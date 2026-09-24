#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""llamaprobe: llama.cpp inference throughput, workload gate metric `llama`.

Runs llama.cpp's own llama-bench (built by caraxes/tools/build-llama.sh: CUDA, SASS for
sm_75, libcuda dynamic, the CUDA runtime and cuBLAS as staged .so files in <dist>/lib) with
every layer on the GPU and reports its numbers unchanged: pp<N> (prompt processing, batched
GEMMs over N tokens, the device-bound half) and tg<N> (token generation, one token per step:
a few hundred small kernels and a synchronous read-back per token, the submission-bound
half). See docs/caraxes/workload-gate.md.

Output: llama-bench's Markdown table (its stderr), then one machine-readable line:
`# LLAMA pp512_tps=... pp512_sd=... tg128_tps=... tg128_sd=... reps=... ngl=... fa=...
 model=... model_mib=... wall_s=... build=... gpu=...`

Usage: llamaprobe.py [--dist DIR] [--model FILE] [--reps 5] [-p 512] [-n 128] [--fa auto]
                     [--libs dist|venv] [--dry-run]
Defaults come from LLAMA_DIR and LLAMA_MODEL (set by bench-run.sh per target), else
./llama and ./llama/model.gguf. --libs venv puts the guest venv's CUDA 13.0 runtime and
cuBLAS 13.1 (torch's nvidia/cu13/lib) first instead of the staged 13.3 ones, a fallback if the
guest driver refuses the newer runtime. --dry-run prints the command and exits.
"""

import argparse
import json
import os
import subprocess
import sys
import time
from pathlib import Path


def venv_cu13_lib():
    try:
        import nvidia  # noqa: F401  (torch's CUDA wheels)
        for base in nvidia.__path__:
            p = Path(base) / "cu13" / "lib"
            if p.is_dir():
                return str(p)
    except ImportError:
        pass
    return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dist", default=os.environ.get("LLAMA_DIR", "./llama"))
    ap.add_argument("--model", default=os.environ.get("LLAMA_MODEL", ""))
    ap.add_argument("--reps", type=int, default=5)
    ap.add_argument("-p", "--prompt", default="512")
    ap.add_argument("-n", "--gen", default="128")
    ap.add_argument("--ngl", type=int, default=99)
    ap.add_argument("--fa", default="auto", help="flash attention: on, off, auto")
    ap.add_argument("--libs", choices=("dist", "venv"), default=os.environ.get("LLAMA_LIBS", "dist"))
    ap.add_argument("--timeout", type=int, default=840)
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    dist = Path(args.dist).expanduser()
    model = Path(args.model).expanduser() if args.model else dist / "model.gguf"
    exe = dist / "llama-bench"
    env = dict(os.environ)
    libs = [str(dist / "lib")]
    if args.libs == "venv":
        v = venv_cu13_lib()
        if not v:
            sys.exit("--libs venv: no nvidia/cu13/lib in this python's site-packages")
        libs.insert(0, v)
    # Our libraries first; the driver's directory (/opt/nvrm/lib in the 610 guests) stays in
    # the path after them for libcuda.
    env["LD_LIBRARY_PATH"] = ":".join(libs + [p for p in env.get("LD_LIBRARY_PATH", "").split(":") if p])
    cmd = [str(exe), "-m", str(model), "-p", args.prompt, "-n", args.gen, "-r", str(args.reps),
           "-ngl", str(args.ngl), "-fa", args.fa, "-o", "jsonl", "-oe", "md"]
    print("cmd:", " ".join(cmd), flush=True)
    print("LD_LIBRARY_PATH:", env["LD_LIBRARY_PATH"], flush=True)
    if args.dry_run:
        return
    for f in (exe, model):
        if not f.exists():
            sys.exit(f"missing {f} (bench-run.sh stage-llama, build-llama.sh)")
    t0 = time.perf_counter()
    p = subprocess.run(cmd, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
                       timeout=args.timeout)
    wall = time.perf_counter() - t0
    sys.stderr.flush()
    print(p.stderr, flush=True)  # the Markdown table and llama.cpp's log
    rows = []
    for line in p.stdout.splitlines():
        line = line.strip()
        if line.startswith("{"):
            try:
                rows.append(json.loads(line))
            except json.JSONDecodeError:
                pass
    if p.returncode != 0 or not rows:
        print(f"llama-bench failed: rc={p.returncode}, {len(rows)} result rows")
        print(p.stdout[-2000:])
        print(f"# LLAMA failed=1 rc={p.returncode} wall_s={wall:.1f}")
        sys.exit(1)
    kv = {}
    for r in rows:
        if r.get("n_prompt", 0) > 0 and r.get("n_gen", 0) == 0:
            name = f"pp{r['n_prompt']}"
        elif r.get("n_gen", 0) > 0 and r.get("n_prompt", 0) == 0:
            name = f"tg{r['n_gen']}"
        else:
            name = f"pp{r.get('n_prompt', 0)}+tg{r.get('n_gen', 0)}"
        kv[f"{name}_tps"] = f"{r['avg_ts']:.2f}"
        kv[f"{name}_sd"] = f"{r['stddev_ts']:.2f}"
    r0 = rows[0]
    kv.update(reps=args.reps, ngl=r0.get("n_gpu_layers", args.ngl), fa=r0.get("flash_attn", args.fa),
              model=str(r0.get("model_type", model.name)).replace(" ", "_"),
              model_mib=f"{int(r0.get('model_size', 0)) / 2**20:.0f}", wall_s=f"{wall:.1f}",
              build=str(r0.get("build_commit", "?")), libs=args.libs,
              gpu=str(r0.get("gpu_info", "?")).replace(" ", "_").replace(",", "+"))
    print("# LLAMA " + " ".join(f"{k}={v}" for k, v in kv.items()))


if __name__ == "__main__":
    main()
