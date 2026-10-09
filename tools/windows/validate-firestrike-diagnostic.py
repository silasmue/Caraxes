#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Qualify one instrumented local Fire Strike packet; never print private XML or command lines.

validate(case_runtime_dir) is read-only. CLI requires a create-only --output path.
XML helpers/configuration derive from project validate-firestrike-monitor-on-C187.py.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import math
from pathlib import Path
import re
import sys
import zipfile
import xml.etree.ElementTree as ET

WORKLOADS = ('FireStrikeGt1P', 'FireStrikeGt2P', 'FireStrikePhysicsP', 'FireStrikeCombinedP')
SCORES = ('firestrikegraphicsscorep', 'firestrikephysicsscorep',
          'firestrikecombinedscorep', 'firestrikeoverallscorep')
# C187 declared monitor-on before launch; all other C148 global gates retained.
CONFIG = {'enable_systeminfo_monitor': 'true', 'enable_systeminfo_collect': 'true',
          'preset': 'P', 'screen_width': '1920', 'screen_height': '1080',
          'swapchain_width': '0', 'swapchain_height': '0',
          'enable_vertical_sync': 'false', 'enable_window_mode': 'false',
          'enable_fixed_fps': 'false', 'enable_wireframe': 'false',
          'use_refrast': 'false', 'loop': 'false', 'render_mode': '0',
          'enable_frame_output': 'false', 'mode_single_frame': 'false',
          'mode_frame_sequence': 'false', 'msaa_sample_count': '1', 'gpu_count': '1'}
SHA_RE = re.compile(r'[0-9a-f]{64}\Z')
MAX_XML = 2 * 1024 * 1024
MAX_ARCHIVE = 128 * 1024 * 1024
MAX_BINARY = 512 * 1024 * 1024


class Invalid(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise Invalid(message)


def one(nodes, label):
    require(len(nodes) == 1, 'missing or duplicate ' + label)
    return nodes[0]


def leaf(node, label):
    require(len(node) == 0 and node.text is not None, 'invalid leaf ' + label)
    return node.text.strip()


def positive(text, label):
    try:
        value = float(text)
    except (ValueError, TypeError):
        raise Invalid('invalid number ' + label) from None
    require(math.isfinite(value) and value > 0, 'nonpositive or nonfinite ' + label)
    return value


def digest(path, limit):
    require(path.is_file() and 0 < path.stat().st_size <= limit, 'missing or oversized input file')
    result = hashlib.sha256()
    with path.open('rb') as stream:
        count = 0
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            count += len(block)
            require(count <= limit, 'input exceeded size bound')
            result.update(block)
    return result.hexdigest()


def unique_fields(pairs):
    obj = {}
    for key, value in pairs:
        require(key not in obj, 'duplicate saved JSON key')
        obj[key] = value
    return obj


def read_json(path):
    require(path.is_file() and path.stat().st_size <= MAX_XML, 'missing or oversized saved JSON')
    data = json.loads(path.read_text(), object_pairs_hook=unique_fields,
                      parse_constant=lambda _: (_ for _ in ()).throw(Invalid('nonfinite JSON')))
    require(isinstance(data, dict), 'saved JSON must be an object')
    return data


def xml_member(archive, name):
    member = one([x for x in archive.infolist() if x.filename == name], name)
    require(0 < member.file_size <= MAX_XML and not member.flag_bits & 1,
            'oversized or encrypted ' + name)
    with archive.open(member) as stream:
        raw = stream.read(MAX_XML + 1)
    require(len(raw) == member.file_size and len(raw) <= MAX_XML, 'invalid XML member size')
    # Entity/DTD definitions are unnecessary for these two known archive members.
    require(b'<!DOCTYPE' not in raw.upper() and b'<!ENTITY' not in raw.upper()
            and b'\x00' not in raw, 'unsupported XML encoding or declaration')
    root = ET.fromstring(raw)
    require(root.tag == 'benchmark', 'unexpected XML root')
    return root


def named_values(parent, child_tag, labels):
    values = {}
    for node in parent.findall('./' + child_tag):
        name = leaf(one(node.findall('./name'), 'named field name'), 'named field name')
        if name in labels:
            require(name not in values, 'duplicate field ' + name)
            values[name] = leaf(one(node.findall('./value'), name), name)
    require(set(labels) == set(values), 'missing required named field')
    return values


def timestamp(text):
    # .NET archives contain up to nine fractional digits; datetime keeps microseconds.
    require(re.fullmatch(r'\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d{1,9})?(?:Z|[+-]\d\d:\d\d)', text),
            'unsupported benchmark timestamp')
    try:
        value = datetime.fromisoformat(text.replace('Z', '+00:00'))
    except ValueError:
        raise Invalid('invalid benchmark timestamp') from None
    require(2000 <= value.year <= 2100 and abs(value.utcoffset().total_seconds()) <= 14 * 3600,
            'benchmark timestamp outside bounds')
    return value.astimezone(timezone.utc)


def events(path):
    require(path.is_file() and path.stat().st_size <= 64 * 1024**2, 'missing or oversized process trace')
    rows = []
    sequences = set()
    for line in path.read_text(encoding='utf-8-sig').splitlines():
        require(len(line) <= MAX_XML, 'oversized process event')
        row = json.loads(line, object_pairs_hook=unique_fields,
                         parse_constant=lambda _: (_ for _ in ()).throw(Invalid('nonfinite event JSON')))
        require(isinstance(row, dict) and row.get('kind') in ('start', 'stop'), 'fatal or unsupported trace event')
        require(type(row.get('pid')) is int and 0 < row['pid'] < 2**32, 'invalid event PID')
        sequence = row.get('callback_sequence')
        require(type(sequence) is int and sequence > 0 and sequence not in sequences, 'duplicate or invalid callback sequence')
        sequences.add(sequence)
        raw_time = row.get('event_filetime_100ns')
        require(isinstance(raw_time, str) and re.fullmatch(r'[0-9]{1,20}', raw_time), 'invalid event FILETIME')
        row['_time'] = int(raw_time)
        unix = (row['_time'] - 116444736000000000) / 10000000
        require(abs(timestamp(row.get('event_utc', '')).timestamp() - unix) < 0.00001, 'event clock mismatch')
        name = row.get('process_name')
        require(isinstance(name, str) and 0 < len(name) <= 260, 'invalid event process name')
        if row['kind'] == 'stop':
            status = row.get('exit_status_uint32')
            require(isinstance(status, str) and re.fullmatch(r'[0-9]{1,10}', status), 'invalid uint32 exit status')
            row['_exit'] = int(status)
            require(row['_exit'] < 2**32 and row.get('exit_status_hex') == '0x%08X' % row['_exit'], 'exit status bit pattern mismatch')
        rows.append(row)
    require(rows, 'empty process trace')
    rows.sort(key=lambda row: (row['_time'], row['callback_sequence']))
    return rows


def lifetimes(rows):
    """Pair chronological lifetimes, never a stop name/PID independent of its start."""
    active, paired = {}, []
    workload = '3dmarkicfworkload.exe'
    for row in rows:
        pid = row['pid']
        name = row['process_name'].casefold()
        if row['kind'] == 'start':
            require(pid not in active, 'intervening same-PID start makes lifetime ambiguous')
            active[pid] = row
            continue
        start = active.get(pid)
        if start is None:
            require(not workload.startswith(name), 'workload stop has no observed start')
            continue  # Preexisting unrelated processes may stop after collector start.
        start_name = start['process_name'].casefold()
        # WMI StopTrace can truncate executable names and reports session/parent=0.
        # Prefix matching is permitted only against this still-open start identity.
        matches = name == start_name or (len(name) >= 13 and start_name.startswith(name))
        require(matches, 'stop name does not belong to the open PID lifetime')
        require(row['_time'] > start['_time'], 'nonpositive process lifetime')
        paired.append((start, row))
        del active[pid]
    require(not any(row['process_name'].casefold() == workload for row in active.values()), 'workload has no paired stop')
    return paired


def validate(case_runtime_dir):
    """Read saved evidence and return a curated diagnostic report, without writes."""
    runtime = Path(case_runtime_dir)
    result_dir = runtime / 'results'
    paths = {name: result_dir / name for name in (
        'trace-ready.json', 'trace-finished.json', 'benchmark-armed.json',
        'before-archives.json', 'render-result.json', 'pre-health.json',
        'post-health.json', 'closeout.json', 'ready.json')}
    saved = {name: read_json(path) for name, path in paths.items()}
    manifest = read_json(runtime / 'PINNED.json')
    case = manifest.get('case')
    require(isinstance(case, str) and re.fullmatch(r'C[0-9]+', case), 'invalid case identity')
    require(manifest.get('status') == 'ROOT_REVIEWED_FROZEN', 'unfrozen runtime')
    ready = saved['ready.json']
    require(ready.get('case') == case and ready.get('manifest_sha256') == digest(runtime / 'PINNED.json', MAX_XML), 'runtime manifest identity mismatch')
    run_path = Path(ready.get('run_directory', ''))
    require(run_path.is_absolute(), 'missing saved run directory')
    env, start = read_json(run_path / 'env.json'), read_json(run_path / 'start.json')
    require(start.get('gpu') is True and start.get('scored') is False, 'not a saved instrumented GPU diagnostic')
    require(start.get('hypervisor') == 'cloud-hypervisor' and env.get('WIN_HOST_MODE') == 'exec', 'not a host-backed virtual GPU diagnostic')
    expected_env = manifest.get('env_extra')
    require(isinstance(expected_env, dict) and all(env.get(key) == value for key, value in expected_env.items()), 'saved runtime flags differ from manifest')
    require(env.get('CARAXES_PROFILE') == '1' and expected_env.get('CARAXES_PROFILE') == '1', 'diagnostic profiling not established')
    backend = manifest.get('backend', {})
    require(isinstance(backend, dict) and isinstance(backend.get('sha256'), str) and SHA_RE.fullmatch(backend['sha256']), 'invalid pinned backend digest')
    require(env.get('WIN_GPU_SERVER') == backend.get('path') and start.get('sha256') == backend['sha256'], 'saved backend identity mismatch')
    require(digest(Path(backend['path']), MAX_BINARY) == backend['sha256'], 'pinned backend bytes changed')
    hypervisor = manifest.get('cloud_hypervisor')
    identities = ready.get('identities')
    require(isinstance(hypervisor, dict) and isinstance(hypervisor.get('sha256'), str)
            and SHA_RE.fullmatch(hypervisor['sha256']), 'invalid pinned hypervisor digest')
    require(isinstance(identities, dict) and isinstance(identities.get('ch'), dict), 'missing saved hypervisor identity')
    require(env.get('WIN_CH') == hypervisor.get('path') and identities['ch'].get('exe') == hypervisor['path']
            and identities['ch'].get('sha256') == hypervisor['sha256'], 'saved hypervisor identity mismatch')
    require(digest(Path(hypervisor['path']), MAX_BINARY) == hypervisor['sha256'], 'pinned hypervisor bytes changed')

    arm = saved['benchmark-armed.json']
    require(arm.get('case') == case and arm.get('demo_off') is True and arm.get('preset') == 'Fire Strike 1.1 / P', 'UI benchmark arm mismatch')
    require(isinstance(arm.get('page_sha256'), str) and SHA_RE.fullmatch(arm['page_sha256']), 'missing reviewed page digest')
    require(digest(Path(arm.get('page_screenshot', '')), MAX_ARCHIVE) == arm['page_sha256'], 'reviewed UI page bytes changed')
    arm_time = timestamp(arm.get('visual_verified_utc', ''))

    trace_ready, finished = saved['trace-ready.json'], saved['trace-finished.json']
    require(trace_ready.get('schema') == finished.get('schema') == 'caraxes-process-trace-v1', 'trace schema mismatch')
    require(trace_ready.get('watchers') == ['Win32_ProcessStartTrace', 'Win32_ProcessStopTrace'], 'trace watcher coverage missing')
    require(all(trace_ready.get(key) == 37 and type(trace_ready.get(key)) is int for key in (
        'self_test_expected_exit_status', 'self_test_direct_exit_status', 'self_test_trace_exit_status')), 'trace exit37 self-test failed')
    require(trace_ready.get('self_test_start_seen') is True and trace_ready.get('self_test_stop_seen') is True, 'trace self-test lifecycle missing')
    require(finished.get('ready_written') is True and finished.get('reason') == 'stop_marker' and finished.get('cleanup_errors') == [], 'trace finished with error or deadline')
    trace_begin = timestamp(finished.get('started_utc', ''))
    trace_ready_time = timestamp(trace_ready.get('ready_utc', ''))
    trace_end = timestamp(finished.get('finished_utc', ''))
    require(trace_begin <= trace_ready_time <= arm_time < trace_end <= timestamp(trace_ready.get('deadline_utc', '')), 'trace/arm coverage unordered')
    trace_path = result_dir / 'trace-process-events.jsonl'
    rows = events(trace_path)
    require(type(finished.get('events_written')) is int and finished['events_written'] == len(rows), 'trace event count mismatch')
    pairs = lifetimes(rows)
    self_test = [(a, b) for a, b in pairs if a['pid'] == trace_ready.get('self_test_pid') and
                 a['process_name'].casefold() == 'cmd.exe' and b['_exit'] == 37 and
                 timestamp(b['event_utc']) <= trace_ready_time]
    require(len(self_test) == 1, 'exit37 self-test has no unique paired lifetime')
    processes = [(a, b) for a, b in pairs if a['process_name'].casefold() == '3dmarkicfworkload.exe']
    require(len(processes) == 4, 'expected exactly four workload lifetimes')
    processes.sort(key=lambda pair: (pair[0]['_time'], pair[0]['callback_sequence']))
    require(all(b['_exit'] == 0 for a, b in processes), 'workload child exit was nonzero')
    require(all(type(a.get('parent_pid')) is int and a['parent_pid'] > 0 and
                type(a.get('session_id')) is int and a['session_id'] > 0 for a, b in processes), 'workload start identity incomplete')
    require(len({a['parent_pid'] for a, b in processes}) == len({a['session_id'] for a, b in processes}) == 1, 'workload parent/session identity changed')

    recovered = saved['render-result.json']
    archive_path = result_dir / 'FireStrike.3dmark-result'
    archive_sha = digest(archive_path, MAX_ARCHIVE)
    require(recovered.get('sha256') == archive_sha and Path(recovered.get('archive', '')).name == archive_path.name, 'recovered archive digest mismatch')
    source = recovered.get('source', {})
    require(isinstance(source, dict) and isinstance(source.get('Name'), str) and re.fullmatch(r'3DMark-FireStrike-[A-Za-z0-9_-]+\.3dmark-result', source['Name']), 'invalid source archive identity')
    require(source.get('Length') == archive_path.stat().st_size, 'archive byte length mismatch')
    before = saved['before-archives.json'].get('files')
    require(isinstance(before, list) and all(isinstance(x, dict) and isinstance(x.get('Name'), str) for x in before), 'invalid before-archive list')
    require(arm.get('previous_archives') == len(before) and len({x['Name'] for x in before}) == len(before), 'before-archive count or identity mismatch')
    require(source['Name'] not in {x['Name'] for x in before}, 'recovered archive is stale')
    source_time = timestamp(source.get('Utc', ''))
    require(arm_time < source_time <= trace_end, 'archive freshness time outside trace')

    with zipfile.ZipFile(archive_path) as archive:
        require(len(archive.infolist()) <= 128, 'too many ZIP members')
        result, arielle = xml_member(archive, 'Result.xml'), xml_member(archive, 'Arielle.xml')
    results = one(result.findall('./results'), 'Result results')
    scores = {}
    for name in SCORES:
        node = one(list(result.iter(name)), name)
        require(node in results.findall('./result/' + name), 'unexpected score location')
        scores[name] = positive(leaf(node, name), name)
    settings = one(arielle.findall('./settings'), 'global settings')
    config = named_values(settings, 'setting', CONFIG)
    require(config == CONFIG, 'archive monitor/collect/preset/fullscreen/VSync configuration mismatch')
    demo = named_values(settings, 'setting', ('show_demo',))['show_demo']
    require(demo in ('true', 'false'), 'invalid archived show_demo')
    sets = one(arielle.findall('./sets'), 'benchmark sets').findall('./set')
    require(len(sets) == 4, 'expected exactly four scored sets')
    names = [leaf(one(node.findall('./name'), 'set name'), 'set name') for node in sets]
    require(names == list(WORKLOADS), 'missing, duplicate, unordered or unexpected workload set')
    all_primary = list(arielle.iter('primary_result'))
    require(len(all_primary) == 4, 'expected exactly four primary FPS results')
    fps = {}
    for node, name in zip(sets, names):
        primary = one(node.findall('./workloads/workload/results/result/primary_result'), name + ' primary_result')
        require(primary in all_primary and primary.attrib.get('result_type') == name and primary.attrib.get('unit') == 'fps', 'primary result type/unit mismatch')
        fps[name] = positive(leaf(primary, name), name)
    require(len(set(fps.values())) == 4, 'identical workload FPS require investigation')
    info = one(arielle.findall('./application_info'), 'application_info')
    times = named_values(info, 'info', ('benchmark_start_time', 'benchmark_finish_time'))
    begin, end = timestamp(times['benchmark_start_time']), timestamp(times['benchmark_finish_time'])
    duration = (end - begin).total_seconds()
    require(math.isfinite(duration) and 1 <= duration <= 600, 'invalid benchmark interval')
    require(arm_time <= begin < end <= source_time <= trace_end, 'benchmark/arm/archive interval mismatch')
    require(all(begin <= timestamp(a['event_utc']) < timestamp(b['event_utc']) <= end for a, b in processes), 'workload lifetime outside benchmark')
    require(all(processes[i][1]['_time'] <= processes[i + 1][0]['_time'] for i in range(3)), 'workload lifetimes overlap')
    for name in ('pre-health.json', 'post-health.json'):
        health = saved[name]
        require(health.get('status') == 'OK' and type(health.get('problem_code')) is int and health['problem_code'] == 0 and
                type(health.get('display_4101')) is int and health['display_4101'] == 0 and health.get('driver_signed') is True and
                health.get('driver_provider') == 'NVIDIA', 'guest NVIDIA health gate failed')
    require(timestamp(saved['pre-health.json'].get('utc', '')) <= arm_time and
            timestamp(saved['post-health.json'].get('utc', '')) >= end, 'health does not bracket benchmark')
    require(saved['pre-health.json'].get('driver_version') == saved['post-health.json'].get('driver_version'), 'driver version changed')
    require(saved['pre-health.json'].get('device_id') == saved['post-health.json'].get('device_id'), 'health device identity changed')
    require(saved['closeout.json'].get('errors') == [], 'owned teardown or host health failed')
    elapsed = saved['closeout.json'].get('elapsed_seconds')
    policy = manifest.get('policy')
    require(isinstance(policy, dict) and type(elapsed) in (int, float) and math.isfinite(elapsed)
            and 0 < elapsed <= policy.get('session_bound_s', 0), 'owned session bound invalid')
    require(digest(archive_path, MAX_ARCHIVE) == archive_sha, 'archive changed during validation')
    evidence = {name: digest(path, MAX_XML) for name, path in paths.items()}
    evidence.update({'PINNED.json': digest(runtime / 'PINNED.json', MAX_XML),
                     'trace-process-events.jsonl': digest(trace_path, 64 * 1024**2),
                     'saved-env.json': digest(run_path / 'env.json', MAX_XML),
                     'saved-start.json': digest(run_path / 'start.json', MAX_XML)})
    return {'schema': 'caraxes-firestrike-diagnostic-qualification-v1', 'case': case,
            'qualification': 'qualified_instrumented_local_run', 'completion': 'complete',
            'scored': False, 'online_validated': False, 'native': False,
            'result_sha256': archive_sha, 'backend_sha256': backend['sha256'],
            'cloud_hypervisor_sha256': hypervisor['sha256'],
            'scores': scores, 'workload_fps': fps, 'duration_seconds': duration,
            'global_config': config, 'ui_demo_off': True, 'archived_show_demo': demo,
            'workload_lifetimes': [{'pid': a['pid'], 'start_utc': a['event_utc'],
                                    'stop_utc': b['event_utc'], 'exit_status': b['_exit']}
                                   for a, b in processes],
            'trace_self_test_exit': 37, 'health_problem_code': 0, 'display_4101': 0,
            'closeout_errors': [], 'evidence_sha256': evidence,
            'limits': ['Instrumented local evidence; not an online/scored/native result.',
                       'Process lifetimes include initialization and do not isolate moving-render time.',
                       'Archived show_demo can differ from the reviewed UI; four scored sets qualified.',
                       'Private archive XML and process command lines are deliberately omitted.']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('case_runtime_dir', type=Path)
    parser.add_argument('--output', required=True, type=Path, help='Create-only curated render-qualification.json')
    args = parser.parse_args()
    try:
        report = validate(args.case_runtime_dir)
        with args.output.open('x', encoding='utf-8') as stream:
            stream.write(json.dumps(report, indent=2, allow_nan=False) + '\n')
        print('FIRESTRIKE_DIAGNOSTIC_QUALIFIED ' + report['case'])
    except Invalid as error:
        print('FIRESTRIKE_DIAGNOSTIC_REJECTED: ' + str(error), file=sys.stderr)
        return 1
    except (OSError, ValueError, KeyError, TypeError, AttributeError, RuntimeError, NotImplementedError, zipfile.BadZipFile, ET.ParseError):
        print('FIRESTRIKE_DIAGNOSTIC_REJECTED: saved evidence unreadable or malformed', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
