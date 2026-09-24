#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Host-side, read-only capture of what a 615.71.09 guest driver prepares for GSP before
# it waits for the status queue (GspStatusQueueInit, kernel_gsp_tu102.c:469 of 615.71.09).
# It follows the device trace until the boot model reports "gsp riscv active", then reads
# guest RAM through the device's DMA-mapped memfds (/proc/<device pid>/fd/<n>, the
# regions of the trace's dma_map lines) and dumps: the libos init arguments (from the
# mailbox pair NV_PGSP_FALCON_MAILBOX0/1 the guest wrote), GSP_ARGUMENTS_CACHED (RMARGS),
# the shared queue memory (page table, command-queue header, first command slot, status
# queue header) and the WPR meta. Nothing is written anywhere but OUT.
# Usage: capture615.py DEVICE_PID TRACE OUT_DIR [timeout_s]
import os, re, struct, sys, time

pid, trace, out = int(sys.argv[1]), sys.argv[2], sys.argv[3]
timeout = float(sys.argv[4]) if len(sys.argv) > 4 else 120
os.makedirs(out, exist_ok=True)

regions = []  # (gpa, size, fd, offset)
mbox = {}
wpr_pa = None
start = time.time()
with open(trace, 'r', errors='replace') as t:
    # The dma_map lines are at the start of the trace; then follow it from its end,
    # so that only the next boot (the next probe) counts.
    for line in t:
        m = re.search(r'E dma_map address=(0x[0-9a-f]+) size=(0x[0-9a-f]+) offset=(0x[0-9a-f]+) fd=Some\((\d+)\)', line)
        if m:
            gpa, size, off, fd = (int(x, 0) for x in m.groups())
            regions.append((gpa, size, fd, off))
    while time.time() - start < timeout:
        line = t.readline()
        if not line:
            time.sleep(0.05)
            continue
        m = re.search(r'W bar0 0x00(110040|110044) 4\s+(0x[0-9a-f]+)', line)
        if m:
            mbox[m.group(1)] = int(m.group(2), 16)
        m = re.search(r'wpr meta PA (0x[0-9a-f]+)', line)
        if m:
            wpr_pa = int(m.group(1), 16)
        if 'gsp riscv active' in line:
            break
    else:
        sys.exit('timeout waiting for "gsp riscv active"')
time.sleep(0.5)  # the guest programs nothing more before GspStatusQueueInit

fds = {}
def rd(pa, n):
    for gpa, size, fd, off in regions:
        if gpa <= pa and pa + n <= gpa + size:
            if fd not in fds:
                fds[fd] = open(f'/proc/{pid}/fd/{fd}', 'rb')
            f = fds[fd]
            f.seek(off + pa - gpa)
            return f.read(n)
    raise ValueError(f'PA {pa:#x} not in guest RAM regions')

log = open(os.path.join(out, 'capture.txt'), 'w')
def say(s):
    print(s); log.write(s + '\n')
def dump(name, pa, n):
    b = rd(pa, n)
    open(os.path.join(out, name), 'wb').write(b)
    say(f'{name}: {n} bytes at PA {pa:#x}')
    return b

say(f'regions: ' + ', '.join(f'[{g:#x}+{s:#x} fd {fd}]' for g, s, fd, _ in regions))
say(f'mailbox0/1 (libos init args PA): {mbox}')
libos_pa = mbox.get('110040', 0) | (mbox.get('110044', 0) << 32)
args = dump('libos-init-args.bin', libos_pa, 4096)
rmargs = None
for i in range(4096 // 32):
    id8, pa, size, kind, loc = struct.unpack_from('<QQQBB', args, 32 * i)
    if id8 == 0:
        break
    name = id8.to_bytes(8, 'big').lstrip(b'\0').decode('ascii', 'replace')
    say(f'  libos arg {i}: id8 {id8:#018x} "{name}" pa {pa:#x} size {size:#x} kind {kind} loc {loc}')
    if name == 'RMARGS':
        rmargs = (pa, size)
if rmargs:
    ga = dump('gsp-arguments-cached.bin', rmargs[0], min(rmargs[1], 0x1000))
    magic, size, = struct.unpack_from('<IH', ga, 0)
    flags, = struct.unpack_from('<Q', ga, 8)
    mq = struct.unpack_from('<QQQQIIIIII', ga, 16)
    say(f'  GSP_ARGUMENTS_CACHED magic {magic:#x} size {size:#x} flags {flags:#x}')
    keys = 'flags sharedMemPhysAddr cmdQueueOffset statQueueOffset queueElementHdrSize queueElementSizeMin queueElementSizeMax queueHeaderAlign queueElementAlign pageTableEntryCount'.split()
    mqd = dict(zip(keys, mq))
    say('  messageQueueInitArguments ' + ' '.join(f'{k}={v:#x}' for k, v in mqd.items()))
    n = mqd['pageTableEntryCount']
    pt = dump('queue-page-table.bin', mqd['sharedMemPhysAddr'], 8 * n)
    pages = struct.unpack_from(f'<{n}Q', pt)
    say(f'  page table: {n} entries, first {[hex(p) for p in pages[:4]]}')
    def qread(off, ln):
        b = b''
        while ln > 0:
            pg, po = divmod(off, 4096)
            k = min(ln, 4096 - po)
            b += rd(pages[pg] + po, k)
            off += k; ln -= k
        return b
    for name, off in (('cmd', mqd['cmdQueueOffset']), ('stat', mqd['statQueueOffset'])):
        hdr = qread(off, 4096)
        open(os.path.join(out, f'{name}-queue-first-page.bin'), 'wb').write(hdr)
        say(f'  {name} queue header (offset {off:#x}): ' + hdr[:32].hex())
        vmaj, vmin, qsize, msize, mcount, eoff, res = struct.unpack_from('<HHIIIII', hdr, 0)
        say(f'    as 615 msgqTxHeader: versionMajor {vmaj} versionMinor {vmin} size {qsize:#x} msgSize {msize:#x} msgCount {mcount} entryOff {eoff:#x} reserved {res:#x}')
        if name == 'cmd' and eoff:
            slot = qread(off + eoff, 4096)
            open(os.path.join(out, 'cmd-queue-slot0.bin'), 'wb').write(slot)
            say('    cmd slot 0 (first 64 bytes): ' + slot[:64].hex())
if wpr_pa:
    dump('wpr-meta.bin', wpr_pa, 256)

# Optional second stage (LINK=1 in the environment): play the GSP's part of msgqTxCreate
# for the status queue (615 msgq.c:81-160: the tx header of the status queue, versionMajor
# 2), written by the host into guest RAM, because the 610 device does not. The guest's
# msgqRxLink (msgq.c:183-265) then succeeds, and kgspSendInitRpcs (kernel_gsp.c:5313-5334)
# queues GMCAPI_CMD_GSP_INIT and rings NV_PGSP_QUEUE_HEAD(0). The script waits for that
# register write in the trace and dumps the command-queue slots it covers.
if os.environ.get('LINK') == '1' and rmargs:
    stat_off = mqd['statQueueOffset']
    size = mqd['cmdQueueOffset'] and (stat_off - mqd['cmdQueueOffset'])  # 0x40000 each
    msg = mqd['queueElementSizeMin']
    entry = 0x1000  # NV_ALIGN_UP(sizeof(msgqTxHeader), 1 << 12), msgq.c:127
    hdr = struct.pack('<HHIIIIIII', 2, 0, size, msg, (size - entry) // msg, entry, 0, 0, 0)
    pg, po = divmod(stat_off, 4096)
    pa = pages[pg] + po
    for gpa, rsize, fd, off in regions:
        if gpa <= pa < gpa + rsize:
            with open(f'/proc/{pid}/fd/{fd}', 'r+b') as f:
                f.seek(off + pa - gpa)
                f.write(hdr)
            break
    say(f'LINK: wrote 615 status-queue header at PA {pa:#x}: {hdr.hex()}')
    with open(trace, 'r', errors='replace') as t:
        t.seek(0, 2)
        heads = []
        t0 = time.time()
        while time.time() - t0 < 30:
            line = t.readline()
            if not line:
                time.sleep(0.02)
                continue
            if re.search(r'bar0 0x00110c[0-9a-f]{2}', line) or ' E ' in line:
                say('trace: ' + line.rstrip())
            m = re.search(r'W bar0 0x00110c00 4\s+(0x[0-9a-f]+)', line)
            if m and int(m.group(1), 16) > 0:
                heads.append(int(m.group(1), 16))
                break
    if heads:
        n = heads[-1]
        cmd = qread(mqd['cmdQueueOffset'] + 0x1000, n * 4096)
        open(os.path.join(out, 'cmd-queue-slots.bin'), 'wb').write(cmd)
        say(f'cmd queue: head {n}, {n} slot(s) dumped; slot 0 first 256 bytes:')
        for i in range(0, 256, 32):
            say('  ' + cmd[i:i + 32].hex())
log.close()
