# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# guest script: scrub stress (300 MiB alloc/fill/free x30, each free scrubbed by the guest RM
# through the device's software CE) with vkcube on X11 alongside as the responsiveness check.
sudo modprobe nvidia && sudo modprobe nvidia_modeset && sudo modprobe nvidia_drm modeset=1 && echo modules-ok
bash ~/caraxes-guest/display-up.sh >/dev/null 2>&1; sudo systemctl start gdm3; sleep 12
for i in $(seq 1 20); do XA=$(ls -t /run/user/1000/.mutter-Xwaylandauth.* 2>/dev/null | head -1); [ -n "$XA" ] && break; sleep 1; done
cat > /tmp/scrub.py <<'PY'
import torch, time
t0 = time.time(); worst = 0
for i in range(30):
    a = time.time()
    x = torch.empty(300 * 1024 * 1024, dtype=torch.uint8, device="cuda"); x.fill_(i); torch.cuda.synchronize()
    del x; torch.cuda.empty_cache(); torch.cuda.synchronize()
    worst = max(worst, time.time() - a)
print(f"SCRUB loops=30 size_mib=300 total_s={time.time()-t0:.2f} worst_loop_s={worst:.2f}")
PY
(DISPLAY=:0 XAUTHORITY=$XA timeout --foreground 90s vkcube --c 3000 > /tmp/vkc.log 2>&1; echo "vkcube-during-scrub rc=$?" > /tmp/vkc.rc) &
~/gpu/venv/bin/python /tmp/scrub.py 2>&1 | tail -2
wait; cat /tmp/vkc.rc
echo "guest Xid: $(sudo dmesg | grep -c 'NVRM: Xid')"
