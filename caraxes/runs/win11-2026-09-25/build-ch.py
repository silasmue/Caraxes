#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""CH kit build: the win-ch-2026-09-29 head as it is (no diagnostic patch), one release binary
for every guest. Refuses a dirty tree or a branch behind dev. CARGO_BUILD_JOBS=2, nice 19.
Usage: build-ch.py <tag> "<purpose>"   ->  CH-build/vfio-user-nvgpu-<tag>, SHA256SUMS (appended),
source-<tag>.json, build-<tag>.log. The cap copy is the operator step printed at the end."""
from pathlib import Path
import json, os, shutil, subprocess, sys

# The tree to build: CH_BUILD_ROOT, else the repository this script lives in.
ROOT = Path(os.environ.get('CH_BUILD_ROOT', str(Path(__file__).resolve().parents[3])))
OUT = Path('/mnt/vmstore/caraxes/alpha-2026-09-27/windows-plain/CH-build')
TARGET = Path(os.environ.get('CARGO_TARGET_DIR', str(ROOT / 'target')))
tag, purpose = sys.argv[1], sys.argv[2]


def git(*a):
    return subprocess.check_output(['git', *a], cwd=ROOT, text=True).strip()


if git('status', '--porcelain', '--untracked-files=no'):
    sys.exit('worktree has tracked changes: commit or stash first')
if subprocess.run(['git', 'merge-base', '--is-ancestor', 'dev', 'HEAD'], cwd=ROOT).returncode:
    sys.exit('branch is behind dev: merge dev first')
binary = OUT / f'vfio-user-nvgpu-{tag}'
if binary.exists():
    sys.exit(f'{binary} exists: choose a new tag')
env = dict(os.environ, CARGO_BUILD_JOBS='4', CARGO_TARGET_DIR=str(TARGET))
with open(OUT / f'build-{tag}.log', 'w') as log:
    subprocess.run(['nice', '-n', '19', 'cargo', 'build', '--locked', '--release', '-p', 'vfio-user-nvgpu'],
                   cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
shutil.copy2(TARGET / 'release/vfio-user-nvgpu', binary)
sha = subprocess.check_output(['sha256sum', binary.name], cwd=OUT, text=True)
with open(OUT / 'SHA256SUMS', 'a') as f:
    f.write(sha)
(OUT / f'source-{tag}.json').write_text(json.dumps({
    'branch': git('rev-parse', '--abbrev-ref', 'HEAD'), 'head': git('rev-parse', 'HEAD'),
    'dev': git('rev-parse', 'dev'), 'features': 'none (one binary for every guest ABI)',
    'purpose': purpose}, indent=2) + '\n')
print(sha, end='')
print(f'next (operator, sudo for setcap only):\n  cp {binary} {binary}-cap && sudo -n setcap cap_sys_admin+p {binary}-cap')
