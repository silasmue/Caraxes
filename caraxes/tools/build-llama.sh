#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Build llama.cpp's llama-bench with CUDA on the host for the workload gate (metric llama,
# docs/caraxes/workload-gate.md), and fetch the model. Nothing here touches the GPU: nvcc
# compiles for sm_75 (TU106) without a device, and the download is a plain GET.
#
#   build-llama.sh build     configure and build into $LLAMA_OUT (default
#                            /mnt/vmstore/caraxes/llama), then assemble $LLAMA_OUT/dist
#   build-llama.sh model     download the GGUF model into $LLAMA_MODELS, record its sha256
#   build-llama.sh check     what dist needs from the guest: glibc symbol versions, NEEDED
#                            libraries, RUNPATH; fails if the binary needs a glibc newer
#                            than the guests' 2.39 or a libstdc++ at all
#   build-llama.sh all       build, model, check
#
# Why this shape. The guest images have gcc but no nvcc (guest-driver-versions.md §8), so
# the build runs on the host: CUDA 13.3 in /opt/cuda, host compiler gcc-15 (nvcc 13.3 refuses
# gcc 16). The host has glibc 2.44 and libstdc++ of gcc 16, the guests (Ubuntu 24.04) glibc
# 2.39 and libstdc++ of gcc 13, so libstdc++ and libgcc go in statically and `check` verifies
# that no symbol needs a glibc past 2.39. glibc 2.43 gave sqrtf a new symbol version; with
# -fno-math-errno gcc emits the sqrtss instruction instead of the libm call (the first build
# failed `check` on exactly that one symbol). libcuda stays dynamic (it is the driver: 610.57.04 in
# the 610 guests, the host's own on native). The CUDA runtime and cuBLAS are the toolkit's
# shared libraries, staged next to the binary in dist/lib (RUNPATH $ORIGIN/lib): ggml's
# static-CUDA switch (GGML_STATIC) links everything with -static, which cannot load libcuda.
# Device code: SASS for sm_75 only (75-real). A CUDA 13.3 build runs on an older 13.x driver
# (minor-version compatibility) as long as nothing needs a JIT of 13.3 PTX, which a 610
# driver could not compile; with SASS only, nothing does.
# NCCL and OpenMP are off (a single GPU; the guest has no libgomp guarantee). VMM stays on
# (the default, cuMemCreate/cuMemMap pools); LLAMA_NO_VMM=1 builds dist-novmm with the
# legacy cudaMalloc pool as a fallback if a target refuses the VMM calls.
set -euo pipefail
ROOT=$(cd "$(dirname "$(readlink -f "$0")")/../.." && pwd -P)
SRC=${LLAMA_SRC:-$(cd "$ROOT/.." && pwd -P)/llama.cpp}
OUT=${LLAMA_OUT:-/mnt/vmstore/caraxes/llama}
MODELS=${LLAMA_MODELS:-/mnt/vmstore/caraxes/models}
CUDA=${CUDA_HOME:-/opt/cuda}
HOSTCXX=${LLAMA_HOSTCXX:-g++-15}; HOSTCC=${LLAMA_HOSTCC:-gcc-15}
# The pinned upstream revision if $SRC does not exist (clone over plain https, no credentials).
LLAMA_URL=https://github.com/ggml-org/llama.cpp.git
LLAMA_REV=${LLAMA_REV:-e9fa0781f1c25fc4fe8c86be1edc6970661ad6f0}
# Model: Llama 3.2 1B Instruct, Q4_K_M (808 MB), a public GGUF conversion; fallback Qwen2.5
# 1.5B Instruct Q4_K_M from Qwen's own repository. Plain GETs, no token, no identifiers:
# curl -q (no ~/.curlrc), no custom headers.
MODEL_URLS=(
    "https://huggingface.co/bartowski/Llama-3.2-1B-Instruct-GGUF/resolve/main/Llama-3.2-1B-Instruct-Q4_K_M.gguf"
    "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/qwen2.5-1.5b-instruct-q4_k_m.gguf"
)
GLIBC_MAX=2.39

err() { echo "ERROR: $*" >&2; exit 1; }

src_rev() {  # the checkout's commit, read from .git without running git
    local h; h=$(cat "$SRC/.git/HEAD" 2>/dev/null) || { echo unknown; return; }
    if [[ $h == ref:* ]]; then
        cat "$SRC/.git/${h#ref: }" 2>/dev/null || grep " ${h#ref: }\$" "$SRC/.git/packed-refs" | cut -d' ' -f1
    else echo "$h"; fi
}

build() {
    local variant=${1:-dist} vmm=OFF bdir
    [[ $variant == dist-novmm ]] && vmm=ON
    bdir=$OUT/build${variant#dist}
    [[ -x $CUDA/bin/nvcc ]] || err "no nvcc at $CUDA/bin/nvcc"
    command -v "$HOSTCXX" >/dev/null || err "no host compiler $HOSTCXX"
    if [[ ! -d $SRC ]]; then
        SRC=$OUT/src
        [[ -d $SRC ]] || { git clone --filter=blob:none "$LLAMA_URL" "$SRC" && git -C "$SRC" checkout "$LLAMA_REV"; }
    fi
    mkdir -p "$OUT"
    echo "llama.cpp $SRC at $(src_rev), build $bdir, GGML_CUDA_NO_VMM=$vmm"
    PATH=$CUDA/bin:$PATH cmake -S "$SRC" -B "$bdir" -G Ninja \
        -DCMAKE_BUILD_TYPE=Release \
        -DCMAKE_C_COMPILER="$HOSTCC" -DCMAKE_CXX_COMPILER="$HOSTCXX" \
        -DCMAKE_CUDA_COMPILER="$CUDA/bin/nvcc" -DCMAKE_CUDA_HOST_COMPILER="$HOSTCXX" \
        -DCMAKE_CUDA_ARCHITECTURES=75-real \
        -DGGML_CUDA=ON -DGGML_CUDA_NCCL=OFF -DGGML_CUDA_NO_VMM=$vmm -DGGML_OPENMP=OFF \
        -DGGML_NATIVE=OFF -DGGML_AVX=ON -DGGML_AVX2=ON -DGGML_FMA=ON -DGGML_F16C=ON \
        -DGGML_STATIC=OFF -DBUILD_SHARED_LIBS=OFF \
        -DLLAMA_CURL=OFF -DLLAMA_OPENSSL=OFF -DLLAMA_BUILD_TESTS=OFF -DLLAMA_BUILD_SERVER=OFF \
        -DCMAKE_C_FLAGS=-fno-math-errno -DCMAKE_CXX_FLAGS=-fno-math-errno \
        -DCMAKE_CUDA_FLAGS=-Xcompiler=-fno-math-errno \
        -DCMAKE_EXE_LINKER_FLAGS="-static-libstdc++ -static-libgcc" \
        -DCMAKE_BUILD_RPATH='$ORIGIN/lib' -DCMAKE_SKIP_RPATH=OFF \
        > "$OUT/cmake${variant#dist}.log" 2>&1 || { tail -30 "$OUT/cmake${variant#dist}.log"; err "cmake failed"; }
    PATH=$CUDA/bin:$PATH cmake --build "$bdir" --target llama-bench -j "$(nproc)" \
        > "$OUT/build${variant#dist}.log" 2>&1 || { grep -m5 -iE 'error' "$OUT/build${variant#dist}.log"; err "build failed ($OUT/build${variant#dist}.log)"; }
    local d=$OUT/$variant
    rm -rf "$d"; mkdir -p "$d/lib"
    cp "$bdir/bin/llama-bench" "$d/"
    # RUNPATH $ORIGIN/lib whatever cmake chose, and nothing else.
    command -v patchelf >/dev/null && patchelf --set-rpath '$ORIGIN/lib' "$d/llama-bench"
    local l
    for l in libcudart.so.13 libcublas.so.13 libcublasLt.so.13; do cp -L "$CUDA/lib64/$l" "$d/lib/$l"; done
    {
        echo "llama.cpp $(src_rev) ($SRC)"
        echo "nvcc $("$CUDA/bin/nvcc" --version | tail -1)"
        echo "host compiler $("$HOSTCXX" --version | head -1)"
        echo "GGML_CUDA_NO_VMM=$vmm CMAKE_CUDA_ARCHITECTURES=75-real built $(date -Iseconds)"
        (cd "$d" && sha256sum llama-bench lib/*)
    } > "$d/BUILD-INFO.txt"
    cat "$d/BUILD-INFO.txt"
}

check() {
    local d=${1:-$OUT/dist} bad=0 v
    [[ -x $d/llama-bench ]] || err "no $d/llama-bench (run build first)"
    echo "NEEDED: $(readelf -d "$d/llama-bench" | grep -oP 'NEEDED.*\[\K[^]]+' | tr '\n' ' ')"
    echo "RUNPATH: $(readelf -d "$d/llama-bench" | grep -oP '(RUNPATH|RPATH).*\[\K[^]]+')"
    readelf -d "$d/llama-bench" | grep -q 'libstdc++' && { echo "FAIL: needs libstdc++.so (the guests' is older)"; bad=1; }
    for f in "$d/llama-bench" "$d"/lib/*; do
        v=$(objdump -T "$f" | grep -oE 'GLIBC_[0-9]+\.[0-9]+' | sort -Vu | tail -1)
        echo "$(basename "$f"): highest glibc symbol $v"
        if [[ $(printf '%s\n%s\n' "${v#GLIBC_}" "$GLIBC_MAX" | sort -V | tail -1) != "$GLIBC_MAX" ]]; then
            echo "FAIL: $(basename "$f") needs $v > GLIBC_$GLIBC_MAX:"
            objdump -T "$f" | grep -E "GLIBC_(2\.(4[0-9]|39\.[0-9]))" | awk '{print "   ", $NF, $(NF-1)}' | head -20
            bad=1
        fi
    done
    [[ $bad == 0 ]] && echo "check: ok for a glibc $GLIBC_MAX guest"
    return $bad
}

model() {
    mkdir -p "$MODELS"
    local u f
    for u in "${MODEL_URLS[@]}"; do
        f=$MODELS/${u##*/}
        if [[ ! -s $f ]]; then
            echo "GET $u"
            curl -q -fL --retry 3 -o "$f.part" "$u" && mv "$f.part" "$f" || { rm -f "$f.part"; echo "download failed: $u"; continue; }
        fi
        # GGUF magic, then the hash next to the file.
        [[ $(head -c4 "$f") == GGUF ]] || { echo "not a GGUF file: $f"; rm -f "$f"; continue; }
        (cd "$MODELS" && sha256sum "${f##*/}" > "${f##*/}.sha256" && cat "${f##*/}.sha256")
        echo "$u" > "$f.source"
        ln -sfn "${f##*/}" "$MODELS/llama-gate.gguf"
        echo "model: $MODELS/llama-gate.gguf -> ${f##*/}"
        return 0
    done
    err "no model downloaded"
}

case ${1:-} in
    build) build dist; [[ ${LLAMA_NO_VMM:-0} == 1 ]] && build dist-novmm; true ;;
    model) model ;;
    check) check "${2:-$OUT/dist}" ;;
    all)   build dist; [[ ${LLAMA_NO_VMM:-0} == 1 ]] && build dist-novmm; model; check "$OUT/dist" ;;
    *) sed -n '2,/^set -euo/p' "$0" | sed '$d'; exit 2 ;;
esac
