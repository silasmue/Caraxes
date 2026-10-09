#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""CPU checks for snapshot identity, loss visibility and overlapping intervals."""

import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location('timeline', Path(__file__).with_name('analyze-backend-timeline.py'))
timeline = importlib.util.module_from_spec(spec)
spec.loader.exec_module(timeline)


class TimelineTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)

    def save(self, name, rows, marker=False, seen=None, dropped=0, anchor=1000, endpoint=100000000):
        label = 'backend observation markers' if marker else 'host ioctl timeline'
        counters = f'overflow={dropped} contention=0 poisoned=0' if marker else f'dropped={dropped}'
        fields = timeline.MARKER_COLUMNS if marker else timeline.HOST_COLUMNS
        text = (f'# SPDX-License-Identifier: MIT\n# {label} schema=1 pid=7 reason=test\n'
                f'# anchor_unix_before_ns={anchor} anchor_unix_after_ns={anchor+1} '
                f'snapshot_mono_ns={endpoint} snapshot_unix_before_ns=1 snapshot_unix_after_ns=2\n'
                f'# capacity=8 seen={len(rows) if seen is None else seen} retained={len(rows)} '
                f'{counters} invalid=0\n' + '\t'.join(fields) + '\n')
        path = self.root / name
        path.write_text(text + ''.join('\t'.join(map(str, row)) + '\n' for row in rows))
        return path

    def test_union_dedup_and_shared_marker_clock(self):
        a = self.save('a', [(1, '0x2080a612', '0x0', 10000000, 30000000, 20000000)])
        b = self.save('b', [(1, '0x2080a612', '0x0', 10000000, 30000000, 20000000),
                            (2, '0x2080a613', '0x56', 20000000, 40000000, 20000000)])
        markers = self.save('m', [(1, 'physical_doorbell_written', 50000000, 4, 1, '-', '-', '-', '-', '-'),
                                  (2, 'physical_doorbell_written', 70000000, 4, 1, '-', '-', '-', '-', '-'),
                                  (3, 'physical_doorbell_written', 90000000, 4, 2, '-', '-', '-', '-', '-')], marker=True)
        report = timeline.analyze([a, b, markers])
        self.assertEqual(report['host_coverage']['unique_rows'], 2)
        self.assertEqual(report['host_control_union_ms'], 30)
        self.assertEqual(sum(x['sum_duration_ms'] for x in report['controls'].values()), 40)
        self.assertEqual(report['controls']['0x2080a613']['statuses'], {'0x56': 1})
        self.assertEqual(report['doorbell_interarrival']['count'], 1)
        self.assertEqual(report['doorbell_interarrival']['p50_ms'], 20)

    def test_clips_union_without_counting_partial_call_as_complete(self):
        p = self.save('a', [(1, '0x2080a613', '0x0', 10000000, 40000000, 30000000)])
        report = timeline.analyze([p], 20000000, 30000000)
        self.assertEqual(report['host_control_union_ms'], 10)
        self.assertEqual(report['controls']['0x2080a613']['count'], 0)

    def test_loss_is_visible_without_claiming_complete_rows(self):
        p = self.save('a', [(3, '0x2080a613', '0x0', 10, 20, 10)], seen=3, dropped=2)
        report = timeline.analyze([p])
        self.assertEqual(report['host_coverage']['unobserved_sequence_ids'], 2)
        self.assertTrue(report['host_coverage']['loss_counters_present'])

    def test_stream_endpoints_and_absence_are_explicit(self):
        host = self.save('h', [], endpoint=10000000)
        markers = self.save('m', [], marker=True, endpoint=20000000)
        report = timeline.analyze([host, markers])
        self.assertTrue(report['host_coverage']['present'])
        self.assertEqual(report['host_coverage']['latest_snapshot_ns'], 10000000)
        self.assertFalse(report['host_coverage']['upper_bound_covered'])
        self.assertTrue(report['marker_coverage']['upper_bound_covered'])
        only_host = timeline.analyze([host])
        self.assertFalse(only_host['marker_coverage']['present'])
        self.assertIsNone(only_host['marker_coverage']['latest_snapshot_ns'])
        self.assertFalse(only_host['marker_coverage']['upper_bound_covered'])

    def test_rejects_conflicting_duplicate_and_clock_identity(self):
        a = self.save('a', [(1, '0x2080a613', '0x0', 10, 20, 10)])
        b = self.save('b', [(1, '0x2080a613', '0x0', 10, 30, 20)])
        with self.assertRaisesRegex(ValueError, 'conflicting'):
            timeline.analyze([a, b])
        c = self.save('c', [], anchor=2000)
        with self.assertRaisesRegex(ValueError, 'different process/clock'):
            timeline.analyze([a, c])

    def test_rejects_truncated_or_inconsistent_intervals(self):
        a = self.save('a', [(1, '0x2080a613', '0x0', 30, 20, 10)])
        with self.assertRaisesRegex(ValueError, 'invalid host interval'):
            timeline.analyze([a])
        b = self.save('b', [(1, '0x2080a613', '0x0', 10, 20)])
        with self.assertRaisesRegex(ValueError, 'truncated'):
            timeline.analyze([b])

    def test_long_paired_capture_deduplicates_more_than_128_snapshots(self):
        paths = []
        for i in range(70):
            paths.append(self.save(f'h{i}', [(1, '0x2080a612', '0x0', 10, 20, 10)]))
            paths.append(self.save(f'm{i}', [(1, 'physical_doorbell_written', 30,
                                            4, 1, '-', '-', '-', '-', '-')], marker=True))
        report = timeline.analyze(paths)
        self.assertEqual(len(report['sources']), 140)
        self.assertEqual(report['host_coverage']['unique_rows'], 1)
        self.assertEqual(report['marker_coverage']['unique_rows'], 1)
        self.assertEqual(report['marker_coverage']['unobserved_sequence_ids'], 0)
        self.assertEqual(report['host_control_union_ms'], 0.00001)

    def test_input_limits_refuse_instead_of_truncating_capture(self):
        with self.assertRaisesRegex(ValueError, 'snapshots'):
            timeline.analyze([self.root / 'missing'] * (timeline.MAX_SNAPSHOTS + 1))
        p = self.save('a', [(1, '0x2080a613', '0x0', 10, 20, 10),
                            (2, '0x2080a613', '0x0', 20, 30, 10)])
        with mock.patch.object(timeline, 'MAX_INPUT_BYTES', p.stat().st_size - 1):
            with self.assertRaisesRegex(ValueError, 'total byte limit'):
                timeline.analyze([p])
        with mock.patch.object(timeline, 'MAX_UNIQUE_ROWS', 1):
            with self.assertRaisesRegex(ValueError, 'unique observations'):
                timeline.analyze([p])


if __name__ == '__main__':
    unittest.main()
