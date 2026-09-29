<!-- SPDX-License-Identifier: MIT -->
# nova regression of the openrender device (f7e9f4c7), 2026-09-29

- PASS: nova-core (Linux 7.2.7, guest ABI 570 detected, Pre610 frame) probed five times ("GPU name: Leandro VFIO RTX 2070-1G"), unloaded four times ("GSP successfully unloaded"), every modprobe/bind/unbind rc=0, no device ERROR line; nova's queues link at the first doorbell ("command queue empty at GSP start"), as before the Booter-Load link of 6715c4e3. Command: README.md "Regression".
