#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""CPU fixtures for the bounded submission timeline analyzer."""

import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest import mock


SPEC = importlib.util.spec_from_file_location(
    'submission', Path(__file__).with_name('analyze-submission-timeline.py'))
submission = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(submission)


class SubmissionTimelineTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)

    def save(self, name, rows, *, anchor=1000, pid=7, endpoint=1000,
             seen=None, overflow=0, contention=0, invalid=0):
        text = (
            f'# SPDX-License-Identifier: MIT\n'
            f'# backend submission timeline schema=1 pid={pid} reason=test\n'
            f'# anchor_unix_before_ns={anchor} anchor_unix_after_ns={anchor + 1} '
            f'snapshot_mono_ns={endpoint} snapshot_unix_before_ns=2000 '
            f'snapshot_unix_after_ns=2001\n'
            f'# capacity=8 seen={len(rows) if seen is None else seen} '
            f'retained={len(rows)} overflow={overflow} contention={contention} '
            f'poisoned=0 invalid={invalid}\n'
            f'# record_bytes=64 storage_capacity_bytes=512\n'
            + '\t'.join(submission.COLUMNS) + '\n'
            + ''.join('\t'.join(map(str, row)) + '\n' for row in rows)
        )
        path = self.root / name
        path.write_text(text)
        return path

    def test_clipping_union_nested_stages_and_overlapping_dumps(self):
        # An outer handler overlaps two lock spans. The second dump repeats row 1.
        outer = (1, 101, 'ring_handler', 'mirror', 10, 90, 80, 4, 8)
        lock_a = (2, 101, 'device_lock', 'mirror', 20, 50, 30, 4, 8)
        lock_b = (3, 101, 'device_lock', 'mirror', 40, 70, 30, 4, 8)
        a = self.save('a', [outer, lock_a], seen=2)
        b = self.save('b', [outer, lock_a, lock_b], seen=3)
        result = submission.analyze([a, b], 30, 60)
        self.assertEqual(result['selection_ns'], {'begin': 30, 'end': 60, 'end_exclusive': True})
        self.assertEqual(result['coverage']['unique_rows'], 3)
        self.assertEqual(result['all_stage_interval_union_ns'], 30)
        groups = {g['stage']: g for g in result['groups']}
        self.assertEqual((groups['ring_handler']['count'], groups['ring_handler']['sum_duration_ns'],
                          groups['ring_handler']['clipped_count']), (1, 30, 1))
        self.assertEqual((groups['device_lock']['count'], groups['device_lock']['sum_duration_ns'],
                          groups['device_lock']['interval_union_ns']), (2, 40, 30))
        self.assertEqual(groups['device_lock']['p50_duration_ns'], 20)
        self.assertEqual(groups['device_lock']['missing_chid'], 0)
        self.assertEqual(len(result['sources'][0]['sha256']), 64)

    def test_conflicting_duplicate_and_clock_identity_rejected(self):
        row = (1, 101, 'fb_flush', 'mirror', 10, 20, 10, '-', '-')
        a = self.save('a', [row])
        b = self.save('b', [(1, 101, 'fb_flush', 'mirror', 10, 21, 11, '-', '-')])
        with self.assertRaisesRegex(ValueError, 'conflicting'):
            submission.analyze([a, b])
        c = self.save('c', [], anchor=2000)
        with self.assertRaisesRegex(ValueError, 'different process/clock'):
            submission.analyze([a, c])
        d = self.save('d', [], pid=8)
        with self.assertRaisesRegex(ValueError, 'different process/clock'):
            submission.analyze([a, d])

    def test_malformed_duration_unknown_fields_and_snapshot_time_rejected(self):
        bad = self.save('bad', [(1, 101, 'bar2_sync', 'mirror', 10, 20, 11, 4, 8)])
        with self.assertRaisesRegex(ValueError, 'invalid submission duration'):
            submission.analyze([bad])
        unknown = self.save('unknown', [(1, 101, 'gpu_done', 'mirror', 10, 20, 10, 4, 8)])
        with self.assertRaisesRegex(ValueError, 'unknown stage'):
            submission.analyze([unknown])
        late = self.save('late', [(1, 101, 'bar2_sync', 'mirror', 10, 1001, 991, 4, 8)])
        with self.assertRaisesRegex(ValueError, 'after snapshot'):
            submission.analyze([late])
        invalid_tid = self.save('tid', [(1, 0, 'bar2_sync', 'mirror', 10, 20, 10, 4, 8)])
        with self.assertRaisesRegex(ValueError, 'zero sequence or TID'):
            submission.analyze([invalid_tid])
        wrong_schema = self.save('schema', [])
        wrong_schema.write_text(wrong_schema.read_text().replace('schema=1', 'schema=2'))
        with self.assertRaisesRegex(ValueError, 'unsupported submission schema'):
            submission.analyze([wrong_schema])

    def test_loss_missing_ids_and_thread_split_stay_explicit(self):
        a = self.save('a', [(3, 101, 'private_userd_scan', 'polled_userd', 10, 20, 10, '-', '-')],
                      seen=4, overflow=1, contention=1)
        b = self.save('b', [(3, 101, 'private_userd_scan', 'polled_userd', 10, 20, 10, '-', '-'),
                            (5, 202, 'private_userd_scan', 'polled_userd', 30, 40, 10, 7, '-')],
                      seen=5, overflow=2, contention=1)
        result = submission.analyze([a, b])
        self.assertEqual(result['coverage']['unobserved_sequence_ids'], 3)
        self.assertTrue(result['coverage']['loss_counters_present'])
        self.assertEqual(result['coverage']['missing_chid_selected'], 1)
        self.assertEqual(result['coverage']['missing_generation_selected'], 2)
        self.assertEqual({g['tid'] for g in result['groups']}, {101, 202})
        self.assertNotIn('bucket', result)

    def test_input_limits_and_bounds_refuse_instead_of_truncating(self):
        with self.assertRaisesRegex(ValueError, 'snapshots'):
            submission.analyze([self.root / 'missing'] * (submission.MAX_SNAPSHOTS + 1))
        p = self.save('a', [(1, 101, 'fb_flush', 'mirror', 10, 20, 10, '-', '-'),
                            (2, 101, 'bar1_replica_sync', 'mirror', 20, 30, 10, '-', '-')])
        with mock.patch.object(submission, 'MAX_INPUT_BYTES', p.stat().st_size - 1):
            with self.assertRaisesRegex(ValueError, 'total byte limit'):
                submission.analyze([p])
        with mock.patch.object(submission, 'MAX_UNIQUE_ROWS', 1):
            with self.assertRaisesRegex(ValueError, 'unique observations'):
                submission.analyze([p])
        with self.assertRaisesRegex(ValueError, 'outside snapshot coverage'):
            submission.analyze([p], 10, 1001)


if __name__ == '__main__':
    unittest.main()
