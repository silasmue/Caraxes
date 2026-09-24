#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""transformer: a GPT-style training step loop, workload gate metric `transformer`.

A decoder-only transformer (default 6 layers, d_model 256, 8 heads, sequence 256, batch 32,
vocabulary 4096) trained with AdamW on synthetic token sequences for a fixed wall time. Each
step is one forward pass, one backward pass and one optimizer step: a few hundred kernel
launches of mixed size (GEMMs through cuBLAS, the fused scaled-dot-product attention,
layer norms, elementwise kernels, the fused AdamW), no host transfer inside the step except
the loss read-back that ends it (`loss.item()`, one synchronous D2H of 4 bytes). The step
time is therefore submission plus device time; on a remote path the per-launch cost shows
up directly. See docs/caraxes/workload-gate.md.

Output: progress lines, then one machine-readable line for the harness:
`# TRANSFORMER steps_per_s=... step_med_ms=... step_p10_ms=... step_p90_ms=... tokens_per_s=...
 steps=... first_step_ms=... loss_first=... loss_last=... amp=... batch=... seq=... layers=...
 dmodel=... params_m=... mem_peak_mib=... device=...`

Usage: transformer.py [--seconds 60] [--layers 6] [--dmodel 256] [--heads 8] [--seq 256]
                      [--batch 32] [--vocab 4096] [--amp] [--cpu] [--steps N]
--cpu runs on the CPU (syntax and shape check only); --steps stops after N timed steps.
If the device runs out of memory, the batch is halved (down to 1) and the run restarts;
the line then reports the batch actually used and `oom_retries`.
"""

import argparse
import math
import statistics
import sys
import time

import torch
import torch.nn as nn
import torch.nn.functional as F


class Block(nn.Module):
    def __init__(self, d, heads):
        super().__init__()
        self.heads = heads
        self.ln1 = nn.LayerNorm(d)
        self.qkv = nn.Linear(d, 3 * d)
        self.proj = nn.Linear(d, d)
        self.ln2 = nn.LayerNorm(d)
        self.fc1 = nn.Linear(d, 4 * d)
        self.fc2 = nn.Linear(4 * d, d)

    def forward(self, x):
        b, t, d = x.shape
        q, k, v = self.qkv(self.ln1(x)).split(d, dim=2)
        q = q.view(b, t, self.heads, d // self.heads).transpose(1, 2)
        k = k.view(b, t, self.heads, d // self.heads).transpose(1, 2)
        v = v.view(b, t, self.heads, d // self.heads).transpose(1, 2)
        # Fused attention: the memory-efficient kernel on sm_75, no t x t score tensor kept.
        y = F.scaled_dot_product_attention(q, k, v, is_causal=True)
        x = x + self.proj(y.transpose(1, 2).reshape(b, t, d))
        return x + self.fc2(F.gelu(self.fc1(self.ln2(x))))


class GPT(nn.Module):
    def __init__(self, vocab, seq, d, layers, heads):
        super().__init__()
        self.tok = nn.Embedding(vocab, d)
        self.pos = nn.Embedding(seq, d)
        self.blocks = nn.ModuleList(Block(d, heads) for _ in range(layers))
        self.ln = nn.LayerNorm(d)
        self.head = nn.Linear(d, vocab, bias=False)
        self.head.weight = self.tok.weight  # tied, as in GPT-2
        self.register_buffer("positions", torch.arange(seq), persistent=False)
        for m in self.modules():
            if isinstance(m, (nn.Linear, nn.Embedding)):
                nn.init.normal_(m.weight, std=0.02)
            if isinstance(m, nn.Linear) and m.bias is not None:
                nn.init.zeros_(m.bias)

    def forward(self, idx):
        x = self.tok(idx) + self.pos(self.positions[: idx.shape[1]])
        for blk in self.blocks:
            x = blk(x)
        return self.head(self.ln(x))


def pct(xs, p):
    s = sorted(xs)
    return s[min(len(s) - 1, max(0, int(round(p / 100 * (len(s) - 1)))))]


def run(args, batch, device):
    torch.manual_seed(args.seed)
    model = GPT(args.vocab, args.seq, args.dmodel, args.layers, args.heads).to(device)
    params = sum(p.numel() for p in model.parameters())
    fused = device.type == "cuda"
    opt = torch.optim.AdamW(model.parameters(), lr=3e-4, weight_decay=0.1, fused=fused)
    use_amp = args.amp and device.type == "cuda"
    scaler = torch.amp.GradScaler("cuda", enabled=use_amp)
    # Synthetic data with learnable structure (a repeating pattern plus noise), generated on
    # the device once: the loop itself moves no data from the host.
    gen = torch.Generator(device="cpu").manual_seed(args.seed)
    n_batches = 8
    base = torch.randint(0, args.vocab, (n_batches, batch, args.seq + 1), generator=gen)
    base[:, :, 1::2] = (base[:, :, 0::2][:, :, : base[:, :, 1::2].shape[2]] * 7 + 3) % args.vocab
    data = base.to(device)

    def step(i):
        seq = data[i % n_batches]
        x, y = seq[:, :-1], seq[:, 1:]
        with torch.autocast(device_type=device.type, dtype=torch.float16, enabled=use_amp):
            logits = model(x)
            loss = F.cross_entropy(logits.float().view(-1, args.vocab), y.reshape(-1))
        opt.zero_grad(set_to_none=True)
        scaler.scale(loss).backward()
        scaler.unscale_(opt)
        nn.utils.clip_grad_norm_(model.parameters(), 1.0)
        scaler.step(opt)
        scaler.update()
        return loss.item()  # the one synchronous read-back per step

    t0 = time.perf_counter()
    loss_first = step(0)
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
            print(f"{s1 - start:7.1f}s | step {len(times):6d} | {len(times) / (s1 - start):7.2f} steps/s | "
                  f"median {statistics.median(times) * 1000:7.2f} ms | loss {loss:.3f}", flush=True)
            last_print = s1
        if s1 - start >= args.seconds or (args.steps and len(times) >= args.steps):
            break
    wall = time.perf_counter() - start
    return dict(times=times, wall=wall, first_ms=first_ms, loss_first=loss_first, loss_last=loss,
                params=params, use_amp=use_amp)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--seconds", type=float, default=60)
    ap.add_argument("--layers", type=int, default=6)
    ap.add_argument("--dmodel", type=int, default=256)
    ap.add_argument("--heads", type=int, default=8)
    ap.add_argument("--seq", type=int, default=256)
    ap.add_argument("--batch", type=int, default=32)
    ap.add_argument("--vocab", type=int, default=4096)
    ap.add_argument("--warmup", type=int, default=5)
    ap.add_argument("--steps", type=int, default=0)
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--amp", action="store_true", help="autocast fp16 with a gradient scaler")
    ap.add_argument("--cpu", action="store_true", help="run on the CPU (shape check)")
    args = ap.parse_args()

    if args.cpu:
        device = torch.device("cpu")
        name = "cpu"
    else:
        if not torch.cuda.is_available():
            raise RuntimeError("CUDA GPU required (--cpu for a shape check)")
        device = torch.device("cuda")
        name = torch.cuda.get_device_name()
        torch.backends.cuda.matmul.allow_tf32 = False
    print(f"device: {name}  torch {torch.__version__}  layers {args.layers} d_model {args.dmodel} "
          f"heads {args.heads} seq {args.seq} batch {args.batch} vocab {args.vocab} amp {args.amp} "
          f"seconds {args.seconds}", flush=True)

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
    peak = torch.cuda.max_memory_allocated() / 2**20 if device.type == "cuda" else 0
    sps = n / r["wall"]
    print("\n=== final ===")
    print(f"timed steps {n} in {r['wall']:.2f} s: {sps:.2f} steps/s, median {med * 1000:.2f} ms, "
          f"{sps * batch * args.seq:.0f} tokens/s, loss {r['loss_first']:.3f} -> {r['loss_last']:.3f}, "
          f"params {r['params'] / 1e6:.2f} M, peak memory {peak:.0f} MiB")
    ok = math.isfinite(r["loss_last"])
    print(f"# TRANSFORMER steps_per_s={sps:.3f} step_med_ms={med * 1000:.3f} "
          f"step_p10_ms={pct(times, 10) * 1000:.3f} step_p90_ms={pct(times, 90) * 1000:.3f} "
          f"tokens_per_s={sps * batch * args.seq:.0f} steps={n} first_step_ms={r['first_ms']:.1f} "
          f"loss_first={r['loss_first']:.4f} loss_last={r['loss_last']:.4f} amp={int(r['use_amp'])} "
          f"batch={batch} seq={args.seq} layers={args.layers} dmodel={args.dmodel} "
          f"params_m={r['params'] / 1e6:.2f} mem_peak_mib={peak:.0f} oom_retries={retries} "
          f"finite={int(ok)} device={name.replace(' ', '_')}")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
