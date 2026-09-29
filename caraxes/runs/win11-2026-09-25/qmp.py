#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""QEMU monitor (QMP) helper for the Windows run: $W/qmp.sock (run-qemu-gpu.sh).

  qmp.py screendump <file.png>   screenshot of the guest display (PNG)
  qmp.py sendkey <key>[,<key>]   e.g. ret, esc, kp_enter, ctrl-alt-delete, f8
  qmp.py powerdown               ACPI power button (clean Windows shutdown)
  qmp.py status                  VM run state
  qmp.py <json>                  raw command, e.g. '{"execute":"query-block"}'"""
import json, os, socket, sys

SOCK = os.environ.get("QMP_SOCK", "/mnt/vmstore/caraxes/win11/qmp.sock")


class Qmp:
    def __init__(self, path=SOCK):
        self.s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.s.settimeout(30)
        self.s.connect(path)
        self.buf = b""
        self._recv()  # greeting
        self.cmd({"execute": "qmp_capabilities"})

    def _recv(self):
        while b"\n" not in self.buf:
            d = self.s.recv(65536)
            if not d:
                raise EOFError("QEMU closed the monitor")
            self.buf += d
        line, self.buf = self.buf.split(b"\n", 1)
        return json.loads(line) if line.strip() else {}

    def cmd(self, obj):
        self.s.sendall(json.dumps(obj).encode() + b"\n")
        while True:
            r = self._recv()
            if "event" in r:
                continue
            if "error" in r:
                raise RuntimeError(r["error"])
            return r.get("return")


def main(argv):
    if len(argv) < 2:
        print(__doc__); return 2
    q = Qmp()
    op = argv[1]
    if op == "screendump":
        print(q.cmd({"execute": "screendump", "arguments": {"filename": os.path.abspath(argv[2]), "format": "png"}}))
    elif op == "sendkey":
        keys = [{"type": "qcode", "data": k} for k in argv[2].split("-")] if "-" in argv[2] and "," not in argv[2] else None
        for combo in argv[2].split(","):
            keys = [{"type": "qcode", "data": k} for k in combo.split("-")]
            print(q.cmd({"execute": "send-key", "arguments": {"keys": keys}}))
    elif op == "powerdown":
        print(q.cmd({"execute": "system_powerdown"}))
    elif op == "status":
        print(q.cmd({"execute": "query-status"}))
    else:
        print(json.dumps(q.cmd(json.loads(op)))[:2000])
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
