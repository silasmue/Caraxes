#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Summarize bounded backend submission spans from overlapping profile dumps."""

import argparse
from collections import defaultdict
import hashlib
import json
import math
from pathlib import Path
import re


MAX_SNAPSHOTS = 4096
MAX_INPUT_BYTES = 8 * 1024 ** 3
MAX_SNAPSHOT_BYTES = 64 * 1024 ** 2
MAX_UNIQUE_ROWS = 4_000_000
STAGES = frozenset('device_lock private_userd_scan shadow_plan bar2_sync '
                   'bar1_replica_sync fb_flush userd_publish userd_harvest '
                   'ring_handler held_ring_wait physical_doorbell'.split())
SOURCES = frozenset('trapped_write eventfd usermode_page posted_doorbell '
                    'polled_userd held_ring_completer mirror'.split())
COLUMNS = ('sequence tid stage source begin_ns end_ns duration_ns chid generation').split()
FIELDS = ('pid anchor_unix_before_ns anchor_unix_after_ns snapshot_mono_ns '
          'snapshot_unix_before_ns snapshot_unix_after_ns capacity seen retained '
          'overflow contention poisoned invalid record_bytes storage_capacity_bytes').split()


def require(condition, message):
    if not condition:
        raise ValueError(message)


def number(value, bits=128):
    require(re.fullmatch(r'[0-9]+', value) is not None, 'invalid unsigned integer')
    result = int(value)
    require(result < 1 << bits, 'integer out of range')
    return result


def optional(value, bits):
    return None if value == '-' else number(value, bits)


def snapshot(path):
    require(path.stat().st_size <= MAX_SNAPSHOT_BYTES, 'snapshot exceeds 64 MiB')
    raw = path.read_bytes()
    require(len(raw) <= MAX_SNAPSHOT_BYTES, 'snapshot exceeds 64 MiB')
    lines = raw.decode('utf-8').splitlines()
    require(sum(line.startswith('# backend submission timeline ') for line in lines) == 1,
            'missing or duplicate submission schema header')
    schema = next(line for line in lines if line.startswith('# backend submission timeline '))
    require(re.fullmatch(r'# backend submission timeline schema=1 pid=[0-9]+ reason=.*', schema),
            'unsupported submission schema')
    meta = {}
    rows = []
    columns = None
    for line in lines:
        if line.startswith('#'):
            for key, value in re.findall(r'([a-z_]+)=([0-9]+)(?:\s|$)', line):
                require(key not in meta, 'duplicate header value')
                meta[key] = number(value)
        elif columns is None:
            columns = line.split('\t')
            require(columns == COLUMNS, 'wrong submission TSV columns')
        else:
            parts = line.split('\t')
            require(len(parts) == len(COLUMNS), 'truncated submission row')
            seq, tid, stage, source, begin, end, duration, chid, generation = parts
            require(stage in STAGES and source in SOURCES, 'unknown stage or source')
            seq, tid = number(seq, 64), number(tid, 32)
            require(seq > 0 and tid > 0, 'zero sequence or TID')
            begin, end, duration = number(begin), number(end), number(duration)
            require(begin <= end and end - begin == duration, 'invalid submission duration')
            rows.append((seq, tid, stage, source, begin, end,
                         optional(chid, 32), optional(generation, 64)))
    require(columns is not None, 'missing submission columns')
    for key in FIELDS:
        require(key in meta, 'missing header ' + key)
    require(meta['pid'] > 0, 'invalid PID')
    require(meta['anchor_unix_before_ns'] <= meta['anchor_unix_after_ns'],
            'invalid anchor bracket')
    require(meta['snapshot_unix_before_ns'] <= meta['snapshot_unix_after_ns'],
            'invalid snapshot bracket')
    require(0 < meta['capacity'] and len(rows) == meta['retained'] <= meta['capacity'],
            'retained/capacity mismatch')
    require(meta['seen'] >= len(rows), 'seen below retained')
    require(meta['record_bytes'] > 0 and meta['storage_capacity_bytes'] >=
            meta['capacity'] * meta['record_bytes'], 'invalid storage capacity')
    require(len({row[0] for row in rows}) == len(rows), 'duplicate sequence in snapshot')
    require(all(row[0] <= meta['seen'] and row[5] <= meta['snapshot_mono_ns'] for row in rows),
            'row after snapshot or seen count')
    source = {**meta, 'path': str(path), 'sha256': hashlib.sha256(raw).hexdigest(),
              'size_bytes': len(raw)}
    return meta, rows, source


def union_ns(intervals):
    total = 0
    end = 0
    for begin, stop in sorted(intervals):
        if begin >= end:
            total += stop - begin
            end = stop
        elif stop > end:
            total += stop - end
            end = stop
    return total


def percentile(values, fraction):
    position = (len(values) - 1) * fraction
    lo, hi = math.floor(position), math.ceil(position)
    return values[lo] + (values[hi] - values[lo]) * (position - lo)


def analyze(paths, begin_ns=0, end_ns=None):
    require(1 <= len(paths) <= MAX_SNAPSHOTS, f'use 1..{MAX_SNAPSHOTS} snapshots')
    require(begin_ns >= 0, 'negative analysis bound')
    require(sum(Path(p).stat().st_size for p in paths) <= MAX_INPUT_BYTES,
            'snapshot inputs exceed total byte limit')
    identity = None
    latest = 0
    rows = {}
    sources = []
    bytes_read = 0
    for path in paths:
        meta, saved, source = snapshot(Path(path))
        bytes_read += source['size_bytes']
        require(bytes_read <= MAX_INPUT_BYTES, 'snapshot inputs exceed total byte limit')
        current = tuple(meta[key] for key in
                        ('pid', 'anchor_unix_before_ns', 'anchor_unix_after_ns'))
        require(identity in (None, current), 'snapshots have different process/clock anchors')
        identity = current
        latest = max(latest, meta['snapshot_mono_ns'])
        sources.append(source)
        for row in saved:
            require(row[0] not in rows or rows[row[0]] == row,
                    'conflicting overlapping snapshot row')
            rows[row[0]] = row
        require(len(rows) <= MAX_UNIQUE_ROWS, 'too many unique observations')
    end_ns = latest if end_ns is None else end_ns
    require(begin_ns < end_ns <= latest, 'analysis interval outside snapshot coverage')
    groups = defaultdict(lambda: {'durations': [], 'intervals': [], 'clipped': 0,
                                  'missing_chid': 0, 'missing_generation': 0})
    all_intervals = []
    selected = 0
    for _, tid, stage, source, begin, end, chid, generation in rows.values():
        if begin >= end_ns or end <= begin_ns:
            continue
        clipped_begin, clipped_end = max(begin, begin_ns), min(end, end_ns)
        if clipped_end < clipped_begin:
            continue
        selected += 1
        group = groups[(stage, source, tid)]
        group['durations'].append(clipped_end - clipped_begin)
        group['intervals'].append((clipped_begin, clipped_end))
        group['clipped'] += begin < begin_ns or end > end_ns
        group['missing_chid'] += chid is None
        group['missing_generation'] += generation is None
        all_intervals.append((clipped_begin, clipped_end))
    summary = []
    for (stage, source, tid), group in sorted(groups.items()):
        durations = sorted(group['durations'])
        summary.append({
            'stage': stage, 'source': source, 'tid': tid, 'count': len(durations),
            'clipped_count': group['clipped'],
            'sum_duration_ns': sum(durations),
            'interval_union_ns': union_ns(group['intervals']),
            'p50_duration_ns': percentile(durations, .5),
            'p95_duration_ns': percentile(durations, .95),
            'p99_duration_ns': percentile(durations, .99),
            'max_duration_ns': durations[-1],
            'missing_chid': group['missing_chid'],
            'missing_generation': group['missing_generation'],
        })
    last_seen = max(source['seen'] for source in sources)
    return {
        'schema': 'caraxes-submission-timeline-analysis-v1',
        'pid': identity[0], 'anchor_unix_bracket_ns': list(identity[1:]),
        'selection_ns': {'begin': begin_ns, 'end': end_ns, 'end_exclusive': True},
        'latest_snapshot_mono_ns': latest,
        'sources': sources,
        'input_limits': {'max_snapshots': MAX_SNAPSHOTS, 'max_bytes': MAX_INPUT_BYTES,
                         'max_unique_rows': MAX_UNIQUE_ROWS, 'read_bytes': bytes_read},
        'coverage': {'unique_rows': len(rows), 'selected_rows': selected,
                     'last_seen': last_seen,
                     'unobserved_sequence_ids': last_seen - len(rows),
                     'loss_counters_present': True,
                     'nonzero_loss_counters_observed': any(any(s[k] for k in
                         ('overflow', 'contention', 'poisoned', 'invalid')) for s in sources),
                     'missing_chid_selected': sum(g['missing_chid'] for g in groups.values()),
                     'missing_generation_selected': sum(g['missing_generation'] for g in groups.values())},
        'all_stage_interval_union_ns': union_ns(all_intervals),
        'groups': summary,
        'limits': [
            'Backend monotonic spans only; no guest or Nsight clock equivalence and no GPU completion.',
            'Nested stage durations overlap; sums across groups are not exclusive time or saved frame time.',
            'Snapshot loss and unobserved sequence IDs cannot be assigned to a stage, source, thread or interval.',
            'Missing identifiers remain missing; they are not inferred from adjacent rows.',
        ],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('snapshots', nargs='+', type=Path)
    parser.add_argument('--begin-ns', type=int, default=0)
    parser.add_argument('--end-ns', type=int)
    parser.add_argument('--output', required=True, type=Path, help='fresh JSON path')
    args = parser.parse_args()
    report = analyze(args.snapshots, args.begin_ns, args.end_ns)
    with args.output.open('x', encoding='utf-8') as out:
        json.dump(report, out, indent=2)
        out.write('\n')
    print(json.dumps({'coverage': report['coverage'], 'groups': len(report['groups'])}))


if __name__ == '__main__':
    try:
        main()
    except (ValueError, OSError, UnicodeError) as error:
        raise SystemExit('Submission timeline analysis failed: ' + str(error)) from None
