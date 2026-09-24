#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Decode the first command-queue element a 615.71.09 guest sends (GMCAPI_CMD_GSP_INIT):
# GSP_MSG_QUEUE_ELEMENT (615 message_queue_priv.h, NV_ABI_STABLE), GMCAPI_HEADER
# (interface/gmcapi/gmcapi_base.h:27-45) and the NVKV payload (inc/libraries/nvkv/nvkv.h:
# key 15:0, idx 27:16, opcode 31:28, count or imm32 63:32). Key names come from
# interface/gmcapi/gmcapi_system_info*.h of the tree given.
# Usage: nvkv-decode.py SLOTS.bin OGKM_615_DIR
import re, struct, sys, glob

raw = open(sys.argv[1], 'rb').read()
names = {}
for f in glob.glob(sys.argv[2] + '/src/nvidia/interface/gmcapi/gmcapi_system_info*.h'):
    for m in re.finditer(r'#define\s+(NVGMC_SI_[A-Z0-9_]+)\s+(0x[0-9A-Fa-f]+)', open(f).read()):
        names.setdefault(int(m.group(2), 16), m.group(1))
magic, mctp_size, mctp_hdr, nvdm_hdr, nvdm_size, res = struct.unpack_from('<6I', raw, 0)
print(f'element: mctpMagic {magic:#x} ({magic.to_bytes(4, "big").decode()}) mctpPayloadSize {mctp_size} '
      f'mctpHeader {mctp_hdr:#x} nvdmHeader {nvdm_hdr:#x} (NVDM type {(nvdm_hdr >> 24) & 0xff:#x}) '
      f'nvdmPayloadSize {nvdm_size} reserved {res}')
cmd, size, seq, maxresp = struct.unpack_from('<IIQI', raw, 24)
print(f'GMCAPI_HEADER: command {cmd:#x} size {size} sequence {seq} request.max_response_size {maxresp:#x}')
kv = struct.unpack_from(f'<{size // 8}Q', raw, 24 + 40)
ops = {0: 'IMM32', 1: 'SEQ32', 2: 'SEQ64', 3: 'ARRAY8', 4: 'ARRAY32', 5: 'ARRAY64'}
i = 0
while i < len(kv):
    w = kv[i]; key = w & 0xffff; idx = (w >> 16) & 0xfff; op = (w >> 28) & 0xf; hi = w >> 32
    n = names.get(key, f'key {key:#06x}')
    if op == 0:
        print(f'  [{i:3}] IMM32   {n}[{idx}] = {hi:#x}'); i += 1
    elif op in (1, 2):
        # SEQ32/SEQ64: count consecutive keys starting at `key`; SEQ32 packs two per word
        cnt = hi
        words = (cnt + 1) // 2 if op == 1 else cnt
        vals = kv[i + 1:i + 1 + words]
        if op == 1:
            flat = []
            for v in vals:
                flat += [v & 0xffffffff, v >> 32]
            flat = flat[:cnt]
        else:
            flat = list(vals)
        items = ', '.join(f'{names.get(key + j, hex(key + j))}={v:#x}' for j, v in enumerate(flat))
        print(f'  [{i:3}] {ops[op]}  {items}' + (f' (idx {idx})' if idx else ''))
        i += 1 + words
    elif op in (3, 4, 5):
        cnt = hi; per = {3: 8, 4: 2, 5: 1}[op]
        words = (cnt + per - 1) // per
        vals = kv[i + 1:i + 1 + words]
        if op == 3:
            b = b''.join(struct.pack('<Q', v) for v in vals)[:cnt]
            shown = repr(b.rstrip(b'\0').decode('ascii', 'replace'))
        else:
            shown = ' '.join(f'{v:016x}' for v in vals[:8]) + (' ...' if words > 8 else '')
        print(f'  [{i:3}] {ops[op]} {n}[{idx}] count {cnt}: {shown}')
        i += 1 + words
    else:
        print(f'  [{i:3}] opcode {op} word {w:#018x} (stop)'); break
