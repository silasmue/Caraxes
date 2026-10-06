#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""Summarize one Nsight Systems GPU-metrics SQLite export without changing it."""

import argparse
import json
import math
import re
import sqlite3
import statistics
import sys
from collections import Counter, defaultdict
from pathlib import Path


def percentile(values, fraction):
    ordered = sorted(values)
    position = (len(ordered) - 1) * fraction
    lower = math.floor(position)
    upper = math.ceil(position)
    return ordered[lower] + (ordered[upper] - ordered[lower]) * (position - lower)


def metric_multipliers(db, names):
    """Match exported names to the captured metric-set YAML's submetrics.

    This intentionally recognizes only Nsight's simple metric/submetric layout.
    Unknown layouts fail instead of emitting unscaled values under scaled units.
    """
    candidates = []
    for (definition,) in db.execute("SELECT value FROM StringIds WHERE value LIKE '%metricSets:%'"):
        aliases = re.findall(r"(?m)^- alias: ([^\n]+)$", definition)
        if len(aliases) != 1 or "\n  metrics:\n" not in definition:
            continue
        body = definition.rsplit("\n  metrics:\n", 1)[1]
        factors = {}
        metric = submetric = None
        for line in body.splitlines():
            if line.startswith("  - name: "):
                metric, submetric = line.split(": ", 1)[1], None
            elif line.startswith("    - name: "):
                if metric is None:
                    raise ValueError("submetric precedes metric in captured definition")
                submetric = line.split(": ", 1)[1]
                name = f"{metric} [{submetric}]"
                if name in factors:
                    raise ValueError(f"duplicate submetric in captured definition: {name}")
                factors[name] = 1.0
            elif line.startswith("      multiplier: "):
                if metric is None or submetric is None:
                    raise ValueError("multiplier precedes submetric in captured definition")
                name = f"{metric} [{submetric}]"
                try:
                    factor = float(line.split(": ", 1)[1])
                except ValueError as error:
                    raise ValueError(f"invalid multiplier for {name}") from error
                if not math.isfinite(factor) or factor <= 0:
                    raise ValueError(f"invalid multiplier for {name}: {factor}")
                factors[name] = factor
        if set(names) <= factors.keys():
            candidates.append((aliases[0], factors))
    if len(candidates) != 1:
        raise ValueError(
            "cannot uniquely match every exported metric to captured metric-set "
            f"units/multipliers (matching sets: {len(candidates)})"
        )
    alias, factors = candidates[0]
    return alias, {name: factors[name] for name in names}


def summarize(args):
    if args.start_ns is not None and args.end_ns is not None and args.start_ns > args.end_ns:
        raise ValueError("--start-ns must not exceed --end-ns")
    if not args.input.is_file():
        raise ValueError(f"SQLite input does not exist: {args.input}")
    db = sqlite3.connect(args.input.resolve().as_uri() + "?mode=ro", uri=True)
    try:
        db.execute("PRAGMA query_only=ON")
        required = {"GPU_METRICS", "TARGET_INFO_GPU_METRICS", "TARGET_INFO_GPU", "StringIds"}
        tables = {row[0] for row in db.execute("SELECT name FROM sqlite_master WHERE type='table'")}
        missing = required - tables
        if missing:
            raise ValueError(f"missing Nsight tables: {', '.join(sorted(missing))}")

        devices = list(db.execute("SELECT id, name, chipName, busLocation FROM TARGET_INFO_GPU"))
        device = [row for row in devices if row[0] == args.device_index]
        if len(device) != 1:
            raise ValueError(f"expected exactly one TARGET_INFO_GPU device {args.device_index}, found {len(device)}")
        groups = [row[0] for row in db.execute("SELECT DISTINCT typeId FROM GPU_METRICS")]
        if len(groups) != 1:
            raise ValueError(f"expected one GPU_METRICS group; found {len(groups)} (device mapping ambiguous)")
        if len(devices) != 1:
            raise ValueError(f"expected one TARGET_INFO_GPU device; found {len(devices)} (group mapping ambiguous)")
        group = groups[0]
        metadata = list(db.execute(
            "SELECT metricId, metricName FROM TARGET_INFO_GPU_METRICS WHERE typeId=? ORDER BY metricId", (group,)
        ))
        if not metadata or len({mid for mid, _ in metadata}) != len(metadata):
            raise ValueError("missing or duplicate metric metadata for selected group")
        unknown = db.execute(
            "SELECT count(*) FROM GPU_METRICS AS g LEFT JOIN TARGET_INFO_GPU_METRICS AS m "
            "ON g.typeId=m.typeId AND g.metricId=m.metricId WHERE m.metricId IS NULL"
        ).fetchone()[0]
        if unknown:
            raise ValueError(f"{unknown} metric rows have no matching (typeId, metricId) metadata")
        names = {mid: name for mid, name in metadata}
        if len(set(names.values())) != len(names):
            raise ValueError("duplicate metric names in selected group")
        metric_set, factors = metric_multipliers(db, names.values())

        bounds = ["g.typeId=?"]
        parameters = [group]
        if args.start_ns is not None:
            bounds.append("g.timestamp>=?")
            parameters.append(args.start_ns)
        if args.end_ns is not None:
            bounds.append("g.timestamp<=?")
            parameters.append(args.end_ns)
        query = (
            "SELECT g.metricId, m.metricName, g.timestamp, g.value "
            "FROM GPU_METRICS AS g JOIN TARGET_INFO_GPU_METRICS AS m "
            "ON g.typeId=m.typeId AND g.metricId=m.metricId "
            "WHERE " + " AND ".join(bounds) + " ORDER BY g.metricId, g.timestamp"
        )
        rows = defaultdict(list)
        nulls = Counter()
        nonfinite = Counter()
        for mid, name, timestamp, value in db.execute(query, parameters):
            if value is None:
                nulls[name] += 1
                continue
            if not isinstance(value, (int, float)) or not math.isfinite(value):
                nonfinite[name] += 1
                continue
            rows[name].append((timestamp, float(value) * factors.get(name, 1.0)))

        metrics = {}
        all_timestamps = set()
        for mid, name in metadata:
            entries = rows[name]
            timestamps = [t for t, _ in entries]
            all_timestamps.update(timestamps)
            values = [v for _, v in entries]
            intervals = [b - a for a, b in zip(timestamps, timestamps[1:])]
            positive_intervals = [v for v in intervals if v > 0]
            cadence = statistics.median(positive_intervals) if positive_intervals else None
            gaps = [v for v in positive_intervals if cadence is not None and v > 3 * cadence]
            result = {
                "metric_id": mid,
                "count": len(values),
                "null_count": nulls[name],
                "nonfinite_count": nonfinite[name],
                "first_timestamp_ns": timestamps[0] if timestamps else None,
                "last_timestamp_ns": timestamps[-1] if timestamps else None,
                "coverage_ns": timestamps[-1] - timestamps[0] if len(timestamps) > 1 else 0,
                "cadence_median_ns": cadence,
                "cadence_p95_ns": percentile(positive_intervals, .95) if positive_intervals else None,
                "max_gap_ns": max(positive_intervals) if positive_intervals else None,
                "gaps_over_3x_median": len(gaps),
                "duplicate_or_reversed_timestamps": len(intervals) - len(positive_intervals),
                "mean": statistics.fmean(values) if values else None,
                "median": statistics.median(values) if values else None,
                "p05": percentile(values, .05) if values else None,
                "p95": percentile(values, .95) if values else None,
                "max": max(values) if values else None,
            }
            if name.endswith("[Workloads]") or name.endswith("[Requests]"):
                result["sum"] = sum(values)
            result["applied_multiplier_from_embedded_yaml"] = factors[name]
            result["unit_status"] = "matched_embedded_metric_set"
            metrics[name] = result

        diagnostics = {"severity_counts": {}, "warnings_and_overflow": []}
        if "DIAGNOSTIC_EVENT" in tables:
            severity = dict(db.execute("SELECT id, label FROM ENUM_DIAGNOSTIC_SEVERITY_LEVEL")) \
                if "ENUM_DIAGNOSTIC_SEVERITY_LEVEL" in tables else {}
            counts = Counter()
            for timestamp, level, message in db.execute(
                "SELECT timestamp, severity, text FROM DIAGNOSTIC_EVENT ORDER BY timestamp"
            ):
                label = severity.get(level, str(level))
                counts[label] += 1
                if level in (2, 3) or "overflow" in message.lower():
                    diagnostics["warnings_and_overflow"].append({
                        "timestamp_ns": timestamp, "severity": label, "text": message
                    })
            diagnostics["severity_counts"] = dict(counts)

        warnings = []
        if not all_timestamps:
            warnings.append("No finite GPU metric samples in selected time window")
        for name, result in metrics.items():
            if result["null_count"] or result["nonfinite_count"]:
                warnings.append(f"{name}: null/nonfinite samples present")
            if result["gaps_over_3x_median"]:
                warnings.append(f"{name}: cadence gaps over 3x median")
            if result["duplicate_or_reversed_timestamps"]:
                warnings.append(f"{name}: duplicate or reversed timestamps")
            if all_timestamps and result["count"] != len(all_timestamps):
                warnings.append(f"{name}: sample count differs from unique group timestamps")
        if diagnostics["warnings_and_overflow"]:
            warnings.append("Nsight warning/error/overflow diagnostics present")
        return {
            "input": str(args.input.resolve()),
            "device": {"index": device[0][0], "name": device[0][1],
                       "chip": device[0][2], "bus_location": device[0][3]},
            "metric_group_type_id": group,
            "metric_set_alias": metric_set,
            "time_selection_ns": {"start": args.start_ns, "end": args.end_ns, "inclusive": True},
            "timestamp_coverage": {
                "unique_samples": len(all_timestamps),
                "first_ns": min(all_timestamps) if all_timestamps else None,
                "last_ns": max(all_timestamps) if all_timestamps else None,
                "span_ns": max(all_timestamps) - min(all_timestamps) if all_timestamps else None,
            },
            "metrics": metrics,
            "diagnostics": diagnostics,
            "warnings": warnings,
        }
    finally:
        db.close()


def main():
    parser = argparse.ArgumentParser(description=(
        "Summarize one GPU's Nsight Systems SQLite metrics. Reads the export only; "
        "timestamps are Nsight event nanoseconds, not wall-clock time. "
        "Metrics are device-wide and do not assign work to a process or guest."
    ))
    parser.add_argument("input", type=Path, help="SQLite file produced by nsys export --type=sqlite")
    parser.add_argument("--device-index", type=int, default=0,
                        help="expected GPU index in TARGET_INFO_GPU (default: 0)")
    parser.add_argument("--start-ns", type=int, help="inclusive Nsight event timestamp lower bound")
    parser.add_argument("--end-ns", type=int, help="inclusive Nsight event timestamp upper bound")
    args = parser.parse_args()
    try:
        print(json.dumps(summarize(args), indent=2, sort_keys=True, allow_nan=False))
    except (ValueError, sqlite3.Error, OSError) as error:
        parser.exit(2, f"nsys-metrics-summary: {error}\n")


if __name__ == "__main__":
    main()
