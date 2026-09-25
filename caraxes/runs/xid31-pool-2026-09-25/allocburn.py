#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""allocburn - fresh device allocations written by kernels while the channel is busy.

The window of docs/caraxes/bar1-pool.md §9: the guest maps a fresh allocation (page tables,
MMU invalidate), then publishes work that writes it (GPFIFO entry, GPPut through the USERD,
doorbell). Here every step empties PyTorch's cache, so each allocation is a cudaMalloc with a
fresh mapping, and a long convolution keeps the channel loaded while the next allocation is
made and written (no synchronisation inside a round).

  ALB_ROUNDS  rounds (default 200)
  ALB_STEPS   allocations per round (default 24)
"""
import os
import time

import torch

rounds = int(os.environ.get("ALB_ROUNDS", "200"))
steps = int(os.environ.get("ALB_STEPS", "24"))
torch.manual_seed(3)
dev = torch.device("cuda")
conv = torch.nn.Conv2d(64, 64, 3, padding=1).to(dev)
x = torch.randn(8, 64, 128, 128, device=dev)
t0 = time.monotonic()
total = 0.0
for r in range(rounds):
    y = conv(x)  # keeps the GR channel busy while the allocations below are mapped
    keep = []
    for s in range(steps):
        n = (1 + (r * steps + s) % 7) * (1 << 19)  # 2 MiB .. 14 MiB of float32
        t = torch.empty(n, device=dev)
        t.fill_(float(s))  # a kernel writing the fresh allocation at once
        t.mul_(1.5)
        keep.append(t)
    y = conv(y)
    total += float(sum(k[-1] for k in keep).item()) + float(y.mean().item())
    del keep, y
    torch.cuda.empty_cache()  # the next round's allocations are fresh mappings again
torch.cuda.synchronize()
print(f"allocburn: rounds={rounds} steps={steps} time={time.monotonic() - t0:.2f}s total={total:.6e}")
