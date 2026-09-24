#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""convnet: ResNet-18 training on synthetic images, workload gate metric `convnet`.

ResNet-18 (the torchvision layout, written out here because the venvs have no torchvision)
trained with SGD and momentum on synthetic 224 x 224 RGB images with 1000 classes, batch 32,
for a fixed wall time, cuDNN autotuning on (`cudnn.benchmark`). Each step is dominated by
cuDNN convolution kernels (forward, data gradient, weight gradient) and batch norm, a few
hundred launches of which many run for hundreds of microseconds: this is the workload where
the device, not the submission path, should be the limit. The images live on the device;
one fresh batch of labels is copied from the host every step (a small H2D) and the loss is
read back (one synchronous D2H). See docs/caraxes/workload-gate.md.

Output: progress lines, then one machine-readable line for the harness:
`# CONVNET images_per_s=... step_med_ms=... step_p10_ms=... step_p90_ms=... steps=...
 first_step_ms=... loss_first=... loss_last=... amp=... batch=... size=... mem_peak_mib=... device=...`

Usage: convnet.py [--seconds 60] [--batch 32] [--size 224] [--amp] [--channels-last]
                  [--cpu] [--steps N]
--cpu runs on the CPU (syntax and shape check only). If the device runs out of memory the
batch is halved and the run restarts; the line reports the batch used and `oom_retries`.
"""

import argparse
import math
import statistics
import sys
import time

import torch
import torch.nn as nn
import torch.nn.functional as F


class BasicBlock(nn.Module):
    def __init__(self, cin, cout, stride):
        super().__init__()
        self.conv1 = nn.Conv2d(cin, cout, 3, stride, 1, bias=False)
        self.bn1 = nn.BatchNorm2d(cout)
        self.conv2 = nn.Conv2d(cout, cout, 3, 1, 1, bias=False)
        self.bn2 = nn.BatchNorm2d(cout)
        self.down = None
        if stride != 1 or cin != cout:
            self.down = nn.Sequential(nn.Conv2d(cin, cout, 1, stride, bias=False), nn.BatchNorm2d(cout))

    def forward(self, x):
        y = F.relu(self.bn1(self.conv1(x)), inplace=True)
        y = self.bn2(self.conv2(y))
        return F.relu(y + (x if self.down is None else self.down(x)), inplace=True)


class ResNet18(nn.Module):
    def __init__(self, classes=1000):
        super().__init__()
        self.stem = nn.Sequential(nn.Conv2d(3, 64, 7, 2, 3, bias=False), nn.BatchNorm2d(64),
                                  nn.ReLU(inplace=True), nn.MaxPool2d(3, 2, 1))
        layers, cin = [], 64
        for cout, stride in ((64, 1), (128, 2), (256, 2), (512, 2)):
            layers += [BasicBlock(cin, cout, stride), BasicBlock(cout, cout, 1)]
            cin = cout
        self.layers = nn.Sequential(*layers)
        self.fc = nn.Linear(512, classes)
        for m in self.modules():
            if isinstance(m, nn.Conv2d):
                nn.init.kaiming_normal_(m.weight, mode="fan_out", nonlinearity="relu")

    def forward(self, x):
        x = self.layers(self.stem(x))
        return self.fc(torch.flatten(F.adaptive_avg_pool2d(x, 1), 1))


def pct(xs, p):
    s = sorted(xs)
    return s[min(len(s) - 1, max(0, int(round(p / 100 * (len(s) - 1)))))]


def run(args, batch, device):
    torch.manual_seed(args.seed)
    fmt = torch.channels_last if args.channels_last else torch.contiguous_format
    model = ResNet18(args.classes).to(device, memory_format=fmt)
    opt = torch.optim.SGD(model.parameters(), lr=0.01, momentum=0.9, weight_decay=1e-4)
    use_amp = args.amp and device.type == "cuda"
    scaler = torch.amp.GradScaler("cuda", enabled=use_amp)
    n_batches = 4
    gen = torch.Generator(device="cpu").manual_seed(args.seed)
    images = torch.randn(n_batches, batch, 3, args.size, args.size, generator=gen).to(device)
    images = [images[i].contiguous(memory_format=fmt) for i in range(n_batches)]
    labels = torch.randint(0, args.classes, (n_batches, batch), generator=gen)
    if device.type == "cuda":
        labels = labels.pin_memory()

    def step(i):
        x = images[i % n_batches]
        y = labels[i % n_batches].to(device, non_blocking=True)  # the per-step H2D
        with torch.autocast(device_type=device.type, dtype=torch.float16, enabled=use_amp):
            loss = F.cross_entropy(model(x), y)
        opt.zero_grad(set_to_none=True)
        scaler.scale(loss).backward()
        scaler.step(opt)
        scaler.update()
        return loss.item()

    t0 = time.perf_counter()
    loss_first = step(0)  # includes cuDNN autotuning of every convolution shape
    first_ms = (time.perf_counter() - t0) * 1000
    for i in range(1, args.warmup + 1):
        step(i)
    if device.type == "cuda":
        torch.cuda.synchronize()
    times, loss = [], loss_first
    start = time.perf_counter()
    last_print = start
    i = args.warmup + 1
    while True:
        s0 = time.perf_counter()
        loss = step(i)
        s1 = time.perf_counter()
        times.append(s1 - s0)
        i += 1
        if s1 - last_print >= 10:
            print(f"{s1 - start:7.1f}s | step {len(times):6d} | {len(times) * batch / (s1 - start):8.1f} img/s | "
                  f"median {statistics.median(times) * 1000:7.2f} ms | loss {loss:.3f}", flush=True)
            last_print = s1
        if s1 - start >= args.seconds or (args.steps and len(times) >= args.steps):
            break
    wall = time.perf_counter() - start
    return dict(times=times, wall=wall, first_ms=first_ms, loss_first=loss_first, loss_last=loss,
                use_amp=use_amp)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--seconds", type=float, default=60)
    ap.add_argument("--batch", type=int, default=32)
    ap.add_argument("--size", type=int, default=224)
    ap.add_argument("--classes", type=int, default=1000)
    ap.add_argument("--warmup", type=int, default=5)
    ap.add_argument("--steps", type=int, default=0)
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--amp", action="store_true", help="autocast fp16 with a gradient scaler")
    ap.add_argument("--channels-last", action="store_true", help="NHWC tensors (tensor cores with --amp)")
    ap.add_argument("--cpu", action="store_true", help="run on the CPU (shape check)")
    args = ap.parse_args()

    if args.cpu:
        device, name = torch.device("cpu"), "cpu"
    else:
        if not torch.cuda.is_available():
            raise RuntimeError("CUDA GPU required (--cpu for a shape check)")
        device, name = torch.device("cuda"), torch.cuda.get_device_name()
        torch.backends.cudnn.benchmark = True
        torch.backends.cudnn.allow_tf32 = False
        torch.backends.cuda.matmul.allow_tf32 = False
    print(f"device: {name}  torch {torch.__version__}  cudnn "
          f"{torch.backends.cudnn.version() if device.type == 'cuda' else '-'}  batch {args.batch} "
          f"size {args.size} amp {args.amp} channels_last {args.channels_last} seconds {args.seconds}", flush=True)

    batch, retries, r = args.batch, 0, None
    while r is None:
        try:
            r = run(args, batch, device)
        except torch.OutOfMemoryError as e:
            if batch <= 1:
                raise
            print(f"out of memory at batch {batch}: {str(e).splitlines()[0]}; retrying with {batch // 2}", flush=True)
            batch //= 2
            retries += 1
        if r is None:  # outside the handler, so the failed attempt's tensors are gone
            torch.cuda.empty_cache()
            torch.cuda.reset_peak_memory_stats()

    times = r["times"]
    n = len(times)
    med = statistics.median(times)
    ips = n * batch / r["wall"]
    peak = torch.cuda.max_memory_allocated() / 2**20 if device.type == "cuda" else 0
    print("\n=== final ===")
    print(f"timed steps {n} in {r['wall']:.2f} s: {ips:.1f} images/s, median step {med * 1000:.2f} ms, "
          f"loss {r['loss_first']:.3f} -> {r['loss_last']:.3f}, first step {r['first_ms']:.0f} ms, "
          f"peak memory {peak:.0f} MiB")
    ok = math.isfinite(r["loss_last"])
    print(f"# CONVNET images_per_s={ips:.2f} step_med_ms={med * 1000:.3f} "
          f"step_p10_ms={pct(times, 10) * 1000:.3f} step_p90_ms={pct(times, 90) * 1000:.3f} steps={n} "
          f"first_step_ms={r['first_ms']:.1f} loss_first={r['loss_first']:.4f} loss_last={r['loss_last']:.4f} "
          f"amp={int(r['use_amp'])} channels_last={int(args.channels_last)} batch={batch} size={args.size} "
          f"mem_peak_mib={peak:.0f} oom_retries={retries} finite={int(ok)} device={name.replace(' ', '_')}")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
