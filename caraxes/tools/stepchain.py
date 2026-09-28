#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""stepchain: the per-submission chain of one rlprobe environment step (docs/caraxes/launch-path.md).

rlprobe's rollout step (Leandro-Test probe/python/rlprobe.py `rollout`) is: the observation to
the GPU (a pageable H2D), the policy's forward pass and the categorical sample (a handful of small
kernels), and the action back with `.item()` (a D2H and the stream synchronize). This probe runs
that step with the same 4-128-2 policy, first as rlprobe does (timed as a whole), then with a
`torch.cuda.synchronize()` after each phase so each phase's own time shows: h2d, fwd (forward and
sample, including the synchronize that waits for the kernels), d2h (`.item()`). A pass with an
empty synchronize measures the bare synchronize. Output: percentiles in microseconds per phase and
one machine-readable line `# STEPCHAIN step_p50_us=... h2d_p50_us=... fwd_p50_us=...
d2h_p50_us=... sync_p50_us=...`.

Usage: stepchain.py [--steps 3000]
"""

import argparse
import time

import numpy as np
import torch
from torch import nn


def pct(xs, q):
    s = sorted(xs)
    return s[min(len(s) - 1, int(q * len(s)))]


def show(name, xs):
    print(f"{name:<6} p10 {pct(xs, .1):8.1f}  p50 {pct(xs, .5):8.1f}  p90 {pct(xs, .9):8.1f}"
          f"  p99 {pct(xs, .99):8.1f}  mean {sum(xs) / len(xs):8.1f} us")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--steps", type=int, default=3000)
    a = ap.parse_args()
    assert torch.cuda.is_available(), "no CUDA card visible"
    dev = torch.device("cuda:0")
    print("device:", torch.cuda.get_device_name(0))
    torch.manual_seed(0)
    pol = nn.Sequential(nn.Linear(4, 128), nn.Tanh(), nn.Linear(128, 2)).to(dev)
    obs = np.random.default_rng(0).standard_normal(4).astype(np.float32)
    us = lambda t0: (time.perf_counter() - t0) * 1e6
    sync = torch.cuda.synchronize

    def step():
        x = torch.from_numpy(obs).to(dev)
        d = torch.distributions.Categorical(logits=pol(x))
        return int(d.sample().item())

    for _ in range(200):  # warm-up: the caching allocator, the modules, the clocks
        step()
    sync()
    whole = []
    for _ in range(a.steps):
        t0 = time.perf_counter()
        step()
        whole.append(us(t0))
    h2d, fwd, d2h, bare = [], [], [], []
    for _ in range(a.steps):
        t0 = time.perf_counter()
        x = torch.from_numpy(obs).to(dev)
        sync()
        h2d.append(us(t0))
        t0 = time.perf_counter()
        s = torch.distributions.Categorical(logits=pol(x)).sample()
        sync()
        fwd.append(us(t0))
        t0 = time.perf_counter()
        int(s.item())
        d2h.append(us(t0))
        t0 = time.perf_counter()
        sync()
        bare.append(us(t0))
    print(f"steps: {a.steps} per pass")
    for n, xs in (("step", whole), ("h2d", h2d), ("fwd", fwd), ("d2h", d2h), ("sync", bare)):
        show(n, xs)
    print("# STEPCHAIN " + " ".join(f"{n}_p50_us={pct(xs, .5):.1f}" for n, xs in
                                     (("step", whole), ("h2d", h2d), ("fwd", fwd),
                                      ("d2h", d2h), ("sync", bare))))


if __name__ == "__main__":
    main()
