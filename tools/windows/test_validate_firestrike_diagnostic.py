# SPDX-License-Identifier: MIT
"""CPU-only saved-packet fixtures; no GPU, guest, or benchmark commands."""

from datetime import datetime, timedelta, timezone
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import xml.etree.ElementTree as ET
import zipfile


SOURCE = Path(__file__).with_name('validate-firestrike-diagnostic.py')
spec = importlib.util.spec_from_file_location('firestrike_validator', SOURCE)
validator = importlib.util.module_from_spec(spec)
spec.loader.exec_module(validator)


class SavedPacketTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)
        self.runtime = self.base / 'C900-runtime'
        self.results = self.runtime / 'results'
        self.results.mkdir(parents=True)
        self.run = self.base / 'saved-run'
        self.run.mkdir()
        self.epoch = datetime(2026, 10, 9, 12, tzinfo=timezone.utc)

    def utc(self, seconds):
        return (self.epoch + timedelta(seconds=seconds)).isoformat().replace('+00:00', 'Z')

    def write(self, path, obj):
        path.write_text(json.dumps(obj))

    def sha(self, path):
        return hashlib.sha256(path.read_bytes()).hexdigest()

    def fixture(self, profile):
        backend, hypervisor = self.base / 'backend', self.base / 'hypervisor'
        backend.write_bytes(b'CPU fixture backend; never executed')
        hypervisor.write_bytes(b'CPU fixture hypervisor; never executed')
        env = {'WIN_HOST_MODE': 'exec', 'WIN_GPU_SERVER': str(backend),
               'WIN_CH': str(hypervisor), 'CARAXES_PROFILE': profile,
               'CARAXES_PROFILE_HOST_TIMELINE': profile}
        if profile == '1':
            env['CARAXES_PROFILE_OUT'] = str(self.results / 'host-profile')
        self.manifest = {'case': 'C900', 'status': 'ROOT_REVIEWED_FROZEN',
                         'env_extra': dict(env), 'policy': {'session_bound_s': 1800},
                         'backend': {'path': str(backend), 'sha256': self.sha(backend)},
                         'cloud_hypervisor': {'path': str(hypervisor), 'sha256': self.sha(hypervisor)}}
        self.ready = {'case': 'C900', 'run_directory': str(self.run),
                      'identities': {'ch': {'exe': str(hypervisor), 'sha256': self.sha(hypervisor)}}}
        self.write(self.run / 'env.json', env)
        self.write(self.run / 'start.json', {'gpu': True, 'scored': False,
                   'hypervisor': 'cloud-hypervisor', 'sha256': self.sha(backend)})
        self.save_manifest()
        screenshot = self.results / 'page.png'
        screenshot.write_bytes(b'CPU fixture only; no actual visual approval')
        self.write(self.results / 'benchmark-armed.json', {
            'case': 'C900', 'demo_off': True, 'preset': 'Fire Strike 1.1 / P',
            'page_screenshot': str(screenshot), 'page_sha256': self.sha(screenshot),
            'visual_verified_utc': self.utc(4), 'previous_archives': 0})
        self.write(self.results / 'trace-ready.json', {
            'schema': 'caraxes-process-trace-v1',
            'watchers': ['Win32_ProcessStartTrace', 'Win32_ProcessStopTrace'],
            'self_test_expected_exit_status': 37, 'self_test_direct_exit_status': 37,
            'self_test_trace_exit_status': 37, 'self_test_start_seen': True,
            'self_test_stop_seen': True, 'self_test_pid': 10,
            'ready_utc': self.utc(3), 'deadline_utc': self.utc(700)})
        rows = []
        for index, (pid, begin, end, name, status) in enumerate([
            (10, 1, 2, 'cmd.exe', 37),
            *[(20 + i, 6 + 11*i, 16 + 11*i, '3DMarkICFWorkload.exe', 0) for i in range(4)]
        ]):
            for kind, seconds in [('start', begin), ('stop', end)]:
                row = {'kind': kind, 'pid': pid, 'process_name': name,
                       'callback_sequence': 2*index + (1 if kind == 'start' else 2),
                       'event_utc': self.utc(seconds), 'parent_pid': 100, 'session_id': 1,
                       'event_filetime_100ns': str(116444736000000000 +
                                                 int(self.epoch.timestamp())*10000000 + seconds*10000000)}
                if kind == 'stop':
                    row.update(exit_status_uint32=str(status), exit_status_hex='0x%08X' % status)
                rows.append(row)
        self.rows = rows
        self.save_rows()
        self.write(self.results / 'trace-finished.json', {
            'schema': 'caraxes-process-trace-v1', 'ready_written': True,
            'reason': 'stop_marker', 'cleanup_errors': [], 'started_utc': self.utc(0),
            'finished_utc': self.utc(65), 'events_written': len(rows)})
        result, arielle = ET.Element('benchmark'), ET.Element('benchmark')
        score = ET.SubElement(ET.SubElement(result, 'results'), 'result')
        for name in validator.SCORES:
            ET.SubElement(score, name).text = '1000'
        settings = ET.SubElement(arielle, 'settings')
        for name, value in dict(validator.CONFIG, show_demo='false').items():
            node = ET.SubElement(settings, 'setting')
            ET.SubElement(node, 'name').text = name
            ET.SubElement(node, 'value').text = value
        sets = ET.SubElement(arielle, 'sets')
        for i, name in enumerate(validator.WORKLOADS):
            node = ET.SubElement(sets, 'set')
            ET.SubElement(node, 'name').text = name
            primary = ET.SubElement(ET.SubElement(ET.SubElement(
                ET.SubElement(ET.SubElement(node, 'workloads'), 'workload'), 'results'), 'result'),
                'primary_result', result_type=name, unit='fps')
            primary.text = str(20 + i)
        info = ET.SubElement(arielle, 'application_info')
        for name, seconds in [('benchmark_start_time', 5), ('benchmark_finish_time', 60)]:
            node = ET.SubElement(info, 'info')
            ET.SubElement(node, 'name').text = name
            ET.SubElement(node, 'value').text = self.utc(seconds)
        archive = self.results / 'FireStrike.3dmark-result'
        with zipfile.ZipFile(archive, 'w') as saved:
            saved.writestr('Result.xml', ET.tostring(result))
            saved.writestr('Arielle.xml', ET.tostring(arielle))
        self.write(self.results / 'render-result.json', {
            'archive': str(archive), 'sha256': self.sha(archive),
            'source': {'Name': '3DMark-FireStrike-fixture.3dmark-result',
                       'Length': archive.stat().st_size, 'Utc': self.utc(61)}})
        self.write(self.results / 'before-archives.json', {'files': []})
        for name, seconds in [('pre-health.json', 0), ('post-health.json', 66)]:
            self.write(self.results / name, {
                'status': 'OK', 'problem_code': 0, 'display_4101': 0,
                'driver_signed': True, 'driver_provider': 'NVIDIA',
                'driver_version': 'fixture', 'device_id': 'fixture', 'utc': self.utc(seconds)})
        self.write(self.results / 'closeout.json', {'errors': [], 'elapsed_seconds': 100})

    def save_manifest(self):
        path = self.runtime / 'PINNED.json'
        self.write(path, self.manifest)
        self.ready['manifest_sha256'] = self.sha(path)
        self.write(self.results / 'ready.json', self.ready)

    def save_rows(self):
        (self.results / 'trace-process-events.jsonl').write_text(
            ''.join(json.dumps(row) + '\n' for row in self.rows))

    def test_default_profiled_packet_still_qualifies(self):
        self.fixture('1')
        report = validator.validate(self.runtime)
        self.assertTrue(report['host_profile_enabled'])
        self.assertEqual(len(report['workload_lifetimes']), 4)

    def test_profile_off_requires_explicit_mode(self):
        self.fixture('0')
        with self.assertRaisesRegex(validator.Invalid, 'profiling not established'):
            validator.validate(self.runtime)
        report = validator.validate(self.runtime, no_host_profile=True)
        self.assertFalse(report['host_profile_enabled'])
        self.assertEqual(report['completion'], 'complete')

    def test_no_profile_rejects_undeclared_or_conflicting_flags(self):
        self.fixture('0')
        baseline = dict(self.manifest['env_extra'])
        variants = [dict(baseline, CARAXES_PROFILE='1'),
                    dict(baseline, CARAXES_PROFILE_HOST_TIMELINE='1'),
                    dict(baseline, CARAXES_PROFILE_OUT='unused')]
        for key in ('CARAXES_PROFILE', 'CARAXES_PROFILE_HOST_TIMELINE'):
            variants.append({k: v for k, v in baseline.items() if k != key})
        for env in variants:
            with self.subTest(env=env):
                self.manifest['env_extra'] = env
                self.write(self.run / 'env.json', env)
                self.save_manifest()
                with self.assertRaisesRegex(validator.Invalid, 'not explicitly disabled'):
                    validator.validate(self.runtime, no_host_profile=True)
        self.manifest['env_extra'] = baseline
        self.save_manifest()
        self.write(self.run / 'env.json', dict(baseline, CARAXES_PROFILE='1'))
        with self.assertRaisesRegex(validator.Invalid, 'flags differ'):
            validator.validate(self.runtime, no_host_profile=True)

    def test_no_profile_retains_workload_health_and_cleanup_checks(self):
        self.fixture('0')
        self.rows[-1].update(exit_status_uint32='1', exit_status_hex='0x00000001')
        self.save_rows()
        with self.assertRaisesRegex(validator.Invalid, 'child exit was nonzero'):
            validator.validate(self.runtime, no_host_profile=True)
        self.rows[-1].update(exit_status_uint32='0', exit_status_hex='0x00000000')
        self.save_rows()
        for filename, update, message in [
            ('post-health.json', {'problem_code': 43}, 'health gate failed'),
            ('closeout.json', {'errors': ['cleanup failed']}, 'teardown or host health failed'),
            ('before-archives.json', {'files': [{'Name': '3DMark-FireStrike-fixture.3dmark-result'}]},
             'archive count or identity mismatch')
        ]:
            path = self.results / filename
            original = path.read_text()
            with self.subTest(filename=filename):
                saved = json.loads(original)
                saved.update(update)
                self.write(path, saved)
                with self.assertRaisesRegex(validator.Invalid, message):
                    validator.validate(self.runtime, no_host_profile=True)
            path.write_text(original)

    def test_cli_no_profile_and_create_only_output(self):
        self.fixture('0')
        output = self.base / 'qualification.json'
        args = [sys.executable, '-B', str(SOURCE), str(self.runtime),
                '--no-host-profile', '--output', str(output)]
        result = subprocess.run(args, capture_output=True, text=True, timeout=5)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(json.loads(output.read_text())['host_profile_enabled'])
        original = output.read_bytes()
        result = subprocess.run(args, capture_output=True, text=True, timeout=5)
        self.assertEqual(result.returncode, 1)
        self.assertEqual(output.read_bytes(), original)


if __name__ == '__main__':
    unittest.main()
