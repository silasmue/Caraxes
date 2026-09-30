#!/usr/bin/env python3
"""KNOWN BUG #1 capture: while a benchmark step of session TAG runs, a window-0 flip gap of 10 s
(no display kick with an UPDATE) triggers a device state dump (`device-ctl`: state), and a
second one 60 s later. One capture per benchmark step. Runs until the session's run stops.
Usage: watch-hang.py TAG [GAP_S]"""
import json, os, re, subprocess, sys, time

K = '/mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain'
TAG = sys.argv[1]
GAP = float(sys.argv[2]) if len(sys.argv) > 2 else 10.0
LOG = f'/mnt/vmstore/caraxes/runs-2026-09-30/win/{TAG}.session.log'
kick = re.compile(rb'^\s*(\d+) E boot: display channel \S+ kick \d+: .* [1-9]\d* UPDATE')


def say(m):
    print(f'{time.strftime("%H:%M:%S")} {m}', flush=True)


def step():
    """The session's current step and when it started, if it is a benchmark still running."""
    try:
        text = open(LOG, 'rb').read().replace(b'\0', b'').decode(errors='replace')
    except OSError:
        return None
    steps = re.findall(r'^=== (\S+) (\d\d:\d\d:\d\d)$', text, re.M)
    if not steps:
        return None
    name, at = steps[-1]
    return (name, at) if name in ('heaven', 'superposition') else None


def last_update_us(trace):
    """Time (us since the device start) of the last flip in the trace's last 16 MiB (0: none
    there, i.e. none for a long while)."""
    try:
        size = os.path.getsize(trace)
        with open(trace, 'rb') as f:
            f.seek(max(0, size - (16 << 20)))
            data = f.read()
    except OSError:
        return None
    for line in reversed(data.splitlines()):
        m = kick.match(line)
        if m:
            return int(m.group(1))
    return 0


def dump(n):
    with open(f'{K}/device-ctl', 'w') as f:
        f.write('state\n')
    say(f'state dump {n} requested')


done = set()
while True:
    cur = step()
    try:
        st = json.load(open(f'{K}/{TAG}/start.json'))
    except (OSError, ValueError):
        st = None
    if st is None or not os.path.exists(f'{K}/gpu-trace.log'):
        if os.path.exists(f'/mnt/vmstore/caraxes/runs-2026-09-30/win/{TAG}/stop.txt'):
            say('session stopped'); break
        time.sleep(2); continue
    if cur and cur not in done:
        started = time.mktime(time.strptime(time.strftime('%Y-%m-%d ') + cur[1], '%Y-%m-%d %H:%M:%S'))
        if time.time() - started > 90:
            last = last_update_us(f'{K}/gpu-trace.log')
            now_us = (time.time() - st['epoch']) * 1e6
            if last is not None and now_us - last > GAP * 1e6:
                say(f'{cur[0]} of {cur[1]}: no flip for {(now_us - last) / 1e6:.1f} s')
                done.add(cur)
                dump(1)
                time.sleep(60)
                dump(2)
    if os.path.exists(f'/mnt/vmstore/caraxes/runs-2026-09-30/win/{TAG}/stop.txt'):
        say('session stopped'); break
    time.sleep(3)
