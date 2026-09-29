#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""cuBLAS, cuDNN and attention through torch, for the coverage map (docs/caraxes/coverage-map.md).

Usage: covtorch.py matmul|conv|attention [seconds]
Each mode runs its kernels for about `seconds` (default 15), checks one result against the
CPU, and prints "COV torch-<mode> ok|fail <detail>" last.
"""
import sys
import time

import torch
import torch.nn.functional as F


def run(fn, seconds):
    n = 0
    t0 = time.time()
    while time.time() - t0 < seconds:
        fn()
        n += 1
    torch.cuda.synchronize()
    return n, time.time() - t0


def matmul(seconds):
    a = torch.randn(1024, 1024, device="cuda")
    b = torch.randn(1024, 1024, device="cuda")
    ah, bh = a.half(), b.half()
    ab = torch.randn(16, 256, 256, device="cuda")

    def step():
        (a @ b).sum()
        (ah @ bh).float().sum()
        torch.bmm(ab, ab).sum()
        torch.addmm(a, a, b, beta=0.5)
    n, dt = run(step, seconds)
    ref = a.cpu() @ b.cpu()
    err = (a @ b).cpu().sub(ref).abs().max().item()
    torch.backends.cuda.matmul.allow_tf32 = True
    tf = (a @ b).cpu().sub(ref).abs().max().item()
    return err < 1e-2, f"{n} rounds of fp32/fp16/bmm/addmm in {dt:.1f} s, max err fp32 {err:.2e}, tf32 {tf:.2e}"


def conv(seconds):
    torch.backends.cudnn.benchmark = True
    x = torch.randn(16, 32, 64, 64, device="cuda", requires_grad=True)
    net = torch.nn.Sequential(
        torch.nn.Conv2d(32, 64, 3, padding=1), torch.nn.BatchNorm2d(64), torch.nn.ReLU(),
        torch.nn.Conv2d(64, 64, 5, padding=2, groups=4), torch.nn.MaxPool2d(2),
        torch.nn.ConvTranspose2d(64, 32, 2, stride=2)).cuda()

    def step():
        y = net(x)
        y.sum().backward()
    n, dt = run(step, seconds)
    net.eval()
    with torch.no_grad():
        y = net(x)
        yc = net.cpu()(x.detach().cpu())
    err = (y.cpu() - yc).abs().max().item()
    return err < 1e-2, f"{n} forward+backward rounds in {dt:.1f} s (cuDNN, benchmark mode), max err {err:.2e}"


def attention(seconds):
    q = torch.randn(8, 8, 512, 64, device="cuda", dtype=torch.half, requires_grad=True)
    k = torch.randn_like(q, requires_grad=True)
    v = torch.randn_like(q, requires_grad=True)

    def step():
        o = F.scaled_dot_product_attention(q, k, v, is_causal=True)
        o.float().sum().backward()
    n, dt = run(step, seconds)
    with torch.no_grad():
        o = F.scaled_dot_product_attention(q, k, v).float().cpu()
        s = (q.float().cpu() @ k.float().cpu().transpose(-1, -2)) / 8.0
        ref = torch.softmax(s, -1) @ v.float().cpu()
    err = (o - ref).abs().max().item()
    return err < 5e-2, f"{n} causal SDPA forward+backward rounds (fp16) in {dt:.1f} s, max err {err:.2e}"


def main():
    mode = sys.argv[1]
    seconds = float(sys.argv[2]) if len(sys.argv) > 2 else 15.0
    ok, detail = {"matmul": matmul, "conv": conv, "attention": attention}[mode](seconds)
    print(f"COV torch-{mode} {'ok' if ok else 'fail'} {detail}")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
