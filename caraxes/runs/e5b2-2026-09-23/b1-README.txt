# SPDX-License-Identifier: MIT
# E5b part 2 attempt b1 (2026-09-23): cuda-init/context/core/hostreg with mirrored CUDA channels before two fixes: a refused GR mirror (GPFIFO_SCHEDULE INVALID_STATE on an externally-owned VA space) left the host RM's USERD clear in place, so the software fallback saw GPPut 0 and the context hung; cuda-init passed. Stopped early.
