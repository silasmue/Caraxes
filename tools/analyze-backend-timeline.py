#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Summarize overlapping backend snapshots without inventing GPU completion times."""

import argparse
import collections
import hashlib
import json
import math
from pathlib import Path
import re


KINDS = {
    'physical_doorbell_written', 'host_notifier_observed',
    'vector_raise_requested', 'local_irq_cause_published',
}
HOST_COLUMNS = 'sequence cmd status begin_ns end_ns duration_ns'.split()
MARKER_COLUMNS = ('sequence kind at_ns chid channel_generation producer_generation '
                  'vector notifier epoch ticket').split()


def require(ok, message):
    if not ok:
        raise ValueError(message)


def integer(value):
    require(re.fullmatch(r'[0-9]+', value) is not None, 'invalid integer')
    return int(value)


def snapshot(path):
    require(path.stat().st_size <= 32 * 1024 * 1024, 'snapshot exceeds 32 MiB')
    raw = path.read_bytes()
    require(len(raw) <= 32 * 1024 * 1024, 'snapshot exceeds 32 MiB')
    lines = raw.decode('utf-8').splitlines()
    headers = [line for line in lines if line.startswith((
        '# backend observation markers ', '# host ioctl timeline '))]
    require(len(headers) == 1 and 'schema=1 ' in headers[0], 'unsupported snapshot schema')
    marker = headers[0].startswith('# backend observation markers ')
    meta = {}
    rows = []
    columns = None
    for line in lines:
        if line.startswith('#'):
            for key, value in re.findall(r'([a-z_]+)=([0-9]+)(?:\s|$)', line):
                require(key not in meta, 'duplicate header value')
                meta[key] = integer(value)
        elif columns is None:
            columns = line.split('\t')
            require(columns == (MARKER_COLUMNS if marker else HOST_COLUMNS), 'wrong TSV columns')
        else:
            parts = line.split('\t')
            require(len(parts) == len(columns), 'truncated TSV row')
            if marker:
                sequence, kind, at, *identifiers = parts
                require(kind in KINDS, 'unknown marker kind')
                rows.append((integer(sequence), kind, integer(at), *[
                    None if value == '-' else integer(value) for value in identifiers]))
            else:
                sequence, cmd, status, begin, end, duration = parts
                require(cmd in ('0x2080a612', '0x2080a613'), 'unexpected host control')
                require(re.fullmatch(r'0x[0-9a-f]+', status), 'invalid host status')
                begin, end = integer(begin), integer(end)
                require(end >= begin and end - begin == integer(duration), 'invalid host interval')
                rows.append((integer(sequence), cmd, int(status, 16), begin, end))
    require(columns is not None, 'missing columns')
    for key in ('pid', 'anchor_unix_before_ns', 'anchor_unix_after_ns',
                'snapshot_mono_ns', 'capacity', 'seen', 'retained', 'invalid'):
        require(key in meta, 'missing header ' + key)
    require(meta['anchor_unix_before_ns'] <= meta['anchor_unix_after_ns'], 'invalid anchor bracket')
    require(meta['capacity'] > 0 and len(rows) == meta['retained'] <= meta['capacity'], 'retained/capacity mismatch')
    require(len({row[0] for row in rows}) == len(rows), 'duplicate sequence in snapshot')
    require(all(0 < row[0] <= meta['seen'] for row in rows), 'invalid sequence')
    require(all((row[2] if marker else row[4]) <= meta['snapshot_mono_ns'] for row in rows),
            'row after snapshot')
    for key in (('overflow', 'contention', 'poisoned') if marker else ('dropped',)):
        require(key in meta, 'missing loss counter ' + key)
    return marker, meta, rows, {'path': str(path), 'sha256': hashlib.sha256(raw).hexdigest(), **meta}


def distribution(values):
    values = sorted(values)
    if not values:
        return {'count': 0}
    def percentile(p):
        index = (len(values) - 1) * p
        lo, hi = math.floor(index), math.ceil(index)
        return values[lo] + (values[hi] - values[lo]) * (index - lo)
    return {'count': len(values), 'p50_ms': percentile(.5) / 1e6,
            'p95_ms': percentile(.95) / 1e6, 'p99_ms': percentile(.99) / 1e6,
            'max_ms': values[-1] / 1e6}


def merged_intervals(intervals):
    merged = []
    for begin, end in sorted(intervals):
        if merged and begin <= merged[-1][1]:
            merged[-1] = (merged[-1][0], max(end, merged[-1][1]))
        else:
            merged.append((begin, end))
    return merged


def analyze(paths, begin_ns=0, end_ns=None, bucket_ns=1_000_000_000):
    require(1 <= len(paths) <= 128, 'use 1..128 snapshots')
    require(begin_ns >= 0 and bucket_ns > 0, 'invalid analysis interval')
    host, markers, sources = {}, {}, []
    anchor = None
    last_snapshot = 0
    for path in paths:
        marker, meta, rows, source = snapshot(Path(path))
        identity = tuple(meta[key] for key in ('pid', 'anchor_unix_before_ns', 'anchor_unix_after_ns'))
        require(anchor in (None, identity), 'snapshots have different process/clock anchors')
        anchor = identity
        last_snapshot = max(last_snapshot, meta['snapshot_mono_ns'])
        sources.append(source)
        saved = markers if marker else host
        for row in rows:
            require(row[0] not in saved or saved[row[0]] == row, 'conflicting overlapping snapshot row')
            saved[row[0]] = row
    end_ns = last_snapshot if end_ns is None else end_ns
    require(begin_ns < end_ns <= last_snapshot, 'interval outside snapshot coverage')
    require((end_ns - begin_ns + bucket_ns - 1) // bucket_ns <= 10000, 'too many buckets')
    intervals = [(max(begin_ns, row[3]), min(end_ns, row[4]))
                 for row in host.values() if row[3] < end_ns and row[4] > begin_ns]
    union = merged_intervals(intervals)
    controls = {}
    for cmd in ('0x2080a612', '0x2080a613'):
        complete = [row for row in host.values()
                    if row[1] == cmd and begin_ns <= row[3] <= row[4] <= end_ns]
        controls[cmd] = {**distribution([row[4] - row[3] for row in complete]),
                         'sum_duration_ms': sum(row[4] - row[3] for row in complete) / 1e6,
                         'statuses': dict(collections.Counter(hex(row[2]) for row in complete))}
    buckets = collections.defaultdict(collections.Counter)
    for a, b in union:
        while a < b:
            index = (a - begin_ns) // bucket_ns
            limit = min(b, begin_ns + (index + 1) * bucket_ns)
            buckets[index]['host_control_union_ns'] += limit - a
            a = limit
    for row in host.values():
        if begin_ns <= row[4] < end_ns:
            buckets[(row[4] - begin_ns) // bucket_ns][row[1] + '_completed'] += 1
    kinds = collections.Counter()
    channels = collections.defaultdict(list)
    for row in markers.values():
        if begin_ns <= row[2] < end_ns:
            kinds[row[1]] += 1
            buckets[(row[2] - begin_ns) // bucket_ns][row[1]] += 1
            if row[1] == 'physical_doorbell_written':
                require(row[3] is not None and row[4] is not None, 'doorbell identity missing')
                channels[(row[3], row[4])].append(row[2])
    gaps = []
    for times in channels.values():
        times.sort()
        gaps.extend(b - a for a, b in zip(times, times[1:]))
    def coverage(saved, marker):
        matching = [source for source in sources if ('overflow' in source) == marker]
        latest = max((source['snapshot_mono_ns'] for source in matching), default=None)
        return {'present': bool(matching), 'latest_snapshot_ns': latest,
                'upper_bound_covered': latest is not None and end_ns <= latest,
                'unique_rows': len(saved), 'last_seen': max((source['seen'] for source in matching), default=0),
                'unobserved_sequence_ids': max((source['seen'] for source in matching), default=0) - len(saved),
                'loss_counters_present': any(any(source[key] for key in
                    (('overflow', 'contention', 'poisoned', 'invalid') if marker else ('dropped', 'invalid')))
                    for source in matching)}
    return {
        'schema': 'caraxes-backend-timeline-analysis-v1',
        'pid': anchor[0], 'anchor_unix_bracket_ns': list(anchor[1:]),
        'begin_ns': begin_ns, 'end_ns': end_ns, 'bucket_ns': bucket_ns,
        'sources': sources, 'host_coverage': coverage(host, False),
        'marker_coverage': coverage(markers, True), 'controls': controls,
        'host_control_union_ms': sum(b - a for a, b in union) / 1e6,
        'marker_counts': dict(kinds), 'doorbell_interarrival': distribution(gaps),
        'buckets': [{'begin_ns': begin_ns + index * bucket_ns,
                     'end_ns': min(end_ns, begin_ns + (index + 1) * bucket_ns), **counts}
                    for index, counts in sorted(buckets.items())],
        'limits': [
            'Backend monotonic timestamps only; no guest QPC or GPU clock alignment.',
            'Host control duration includes kernel/driver waits, not just firmware execution.',
            'Time union is observed control occupancy, not critical-path time or saved frame time.',
            'Doorbell interarrival gaps are per chid/generation, not queue depth or submission latency.',
            'Notifiers are engine-wide; no GPU completion, guest interrupt delivery or ACK is inferred.',
            'Missing rows can hide events; no FIFO pairing between controls, doorbells and notifiers.',
            'Each stream has its own snapshot endpoint; absent or incomplete coverage is not zero activity.',
        ],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('snapshots', nargs='+', type=Path)
    parser.add_argument('--begin-ns', type=int, default=0)
    parser.add_argument('--end-ns', type=int)
    parser.add_argument('--output', required=True, type=Path, help='fresh private JSON path')
    args = parser.parse_args()
    report = analyze(args.snapshots, args.begin_ns, args.end_ns)
    with args.output.open('x', encoding='utf-8') as out:
        json.dump(report, out, indent=2)
        out.write('\n')
    print(json.dumps({key: report[key] for key in (
        'host_coverage', 'marker_coverage', 'controls', 'marker_counts', 'host_control_union_ms')}))


if __name__ == '__main__':
    try:
        main()
    except (ValueError, OSError, UnicodeError) as error:
        raise SystemExit('Timeline analysis failed: ' + str(error)) from None
