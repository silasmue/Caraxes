#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Small SQLite exports exercise explicit unit provenance without native tools."""

import argparse
import hashlib
import importlib.util
import sqlite3
import tempfile
import unittest
from pathlib import Path


SOURCE = Path(__file__).with_name("nsys-metrics-summary.py")
SPEC = importlib.util.spec_from_file_location("nsys_metrics_summary", SOURCE)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)

DEFINITION = "metricSets:\n- alias: tu10x\n  metrics:\n  - name: SM Frequency\n    - name: MHz\n      multiplier: 0.001\n"


class MetricDefinitionTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.db = self.root / "metrics.sqlite"
        db = sqlite3.connect(self.db)
        try:
            db.executescript("""
                CREATE TABLE TARGET_INFO_GPU(id INTEGER, name TEXT, chipName TEXT, busLocation TEXT);
                CREATE TABLE TARGET_INFO_GPU_METRICS(typeId INTEGER, metricId INTEGER, metricName TEXT);
                CREATE TABLE GPU_METRICS(typeId INTEGER, metricId INTEGER, timestamp INTEGER, value REAL);
                CREATE TABLE StringIds(value TEXT);
                INSERT INTO TARGET_INFO_GPU VALUES (0,'GPU','TU106','0000:2d:00.0');
                INSERT INTO TARGET_INFO_GPU_METRICS VALUES (1,2,'SM Frequency [MHz]');
                INSERT INTO GPU_METRICS VALUES (1,2,100,1500000);
            """)
            db.commit()
        finally:
            db.close()

    def args(self, path=None, digest=None):
        return argparse.Namespace(input=self.db, device_index=0, start_ns=None, end_ns=None,
                                  metric_definition=path, metric_definition_sha256=digest)

    def definition(self, text=DEFINITION):
        path = self.root / "definition.config"
        raw = text.encode()
        path.write_bytes(raw)
        return path, hashlib.sha256(raw).hexdigest()

    def embedded(self, text=DEFINITION):
        db = sqlite3.connect(self.db)
        try:
            db.execute("INSERT INTO StringIds VALUES (?)", (text,))
            db.commit()
        finally:
            db.close()

    def test_embedded_default_and_external_mhz(self):
        self.embedded()
        old = MODULE.summarize(self.args())
        self.assertEqual(old['metrics']['SM Frequency [MHz]']['mean'], 1500)
        self.assertEqual(old['metrics']['SM Frequency [MHz]']['unit_status'], 'matched_embedded_metric_set')
        path, digest = self.definition()
        new = MODULE.summarize(self.args(path, digest))
        self.assertEqual(new['metrics']['SM Frequency [MHz]']['mean'], 1500)
        self.assertEqual(new['metric_definition_provenance']['sha256'], digest)
        self.assertEqual(new['metrics']['SM Frequency [MHz]']['unit_status'], 'matched_pinned_external_metric_set')

    def test_missing_external_and_pair(self):
        with self.assertRaises(ValueError): MODULE.summarize(self.args())
        path, digest = self.definition()
        for args in (self.args(path, None), self.args(None, digest),
                     self.args(self.root / 'missing', digest), self.args(path, '0'*64)):
            with self.subTest(args=args), self.assertRaises((ValueError, OSError)):
                MODULE.summarize(args)

    def test_invalid_ambiguous_and_disagree(self):
        for text in (DEFINITION.replace('0.001', 'nan'), DEFINITION.replace('0.001', '0'),
                     DEFINITION + DEFINITION):
            path, digest = self.definition(text)
            with self.subTest(text=text), self.assertRaises(ValueError): MODULE.summarize(self.args(path, digest))
        self.embedded()
        path, digest = self.definition(DEFINITION.replace('0.001', '0.002'))
        with self.assertRaisesRegex(ValueError, 'disagree'): MODULE.summarize(self.args(path, digest))

    def test_bounded_raw_utf8_and_name_match(self):
        for raw in (b'\xff', b'a' * (MODULE.MAX_METRIC_DEFINITION_BYTES + 1),
                    DEFINITION.replace('SM Frequency', 'Other').encode()):
            path = self.root / 'definition.config'
            path.write_bytes(raw)
            with self.subTest(length=len(raw)), self.assertRaises((ValueError, UnicodeError)):
                MODULE.summarize(self.args(path, hashlib.sha256(raw).hexdigest()))


if __name__ == '__main__':
    unittest.main()
