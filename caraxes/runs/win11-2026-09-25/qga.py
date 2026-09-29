#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""Talk to the Windows guest's QEMU guest agent over $W/qga.sock (run-qemu-gpu.sh).

  qga.py ping                      guest-ping / guest-info (agent version)
  qga.py ps '<powershell>'         run PowerShell as SYSTEM, print stdout and stderr, exit code
  qga.py cmd '<cmd.exe line>'      run through cmd.exe /c
  qga.py get <guest path> <host path>   copy a file out of the guest
  qga.py put <host path> <guest path>   copy a file into the guest
Commands wait up to QGA_TIMEOUT seconds (default 600)."""
import base64, json, os, socket, sys, time

SOCK = os.environ.get("QGA_SOCK", "/mnt/vmstore/caraxes/win11/qga.sock")
TIMEOUT = int(os.environ.get("QGA_TIMEOUT", "600"))


class Qga:
    def __init__(self, path=SOCK):
        self.s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.s.settimeout(30)
        self.s.connect(path)
        self.buf = b""
        # guest-sync-delimited flushes stale bytes from a previous client
        nonce = int(time.time()) & 0x7FFFFFFF
        self.s.sendall(b"\xff" + json.dumps({"execute": "guest-sync-delimited", "arguments": {"id": nonce}}).encode() + b"\n")
        while True:
            r = self._recv()
            if r.get("return") == nonce:
                break

    def _recv(self):
        while b"\n" not in self.buf:
            d = self.s.recv(65536)
            if not d:
                raise EOFError("agent closed the socket")
            self.buf += d
        line, self.buf = self.buf.split(b"\n", 1)
        line = line.lstrip(b"\xff")
        return json.loads(line) if line.strip() else {}

    def call(self, cmd, **args):
        self.s.sendall(json.dumps({"execute": cmd, "arguments": args}).encode() + b"\n")
        r = self._recv()
        if "error" in r:
            raise RuntimeError(f"{cmd}: {r['error']}")
        return r.get("return")

    def exec_wait(self, path, args):
        pid = self.call("guest-exec", path=path, arg=args, **{"capture-output": True})["pid"]
        t0 = time.time()
        while True:
            st = self.call("guest-exec-status", pid=pid)
            if st.get("exited"):
                out = base64.b64decode(st.get("out-data", "")).decode("utf-8", "replace")
                err = base64.b64decode(st.get("err-data", "")).decode("utf-8", "replace")
                return st.get("exitcode", -1), out, err
            if time.time() - t0 > TIMEOUT:
                raise TimeoutError(f"guest-exec pid {pid} still running after {TIMEOUT} s")
            time.sleep(1)

    def get(self, gpath, hpath):
        h = self.call("guest-file-open", path=gpath, mode="rb")
        try:
            with open(hpath, "wb") as f:
                while True:
                    r = self.call("guest-file-read", handle=h, count=1 << 20)
                    f.write(base64.b64decode(r["buf-b64"]))
                    if r["eof"]:
                        break
        finally:
            self.call("guest-file-close", handle=h)

    def put(self, hpath, gpath):
        h = self.call("guest-file-open", path=gpath, mode="wb")
        try:
            with open(hpath, "rb") as f:
                while True:
                    d = f.read(1 << 20)
                    if not d:
                        break
                    self.call("guest-file-write", handle=h, **{"buf-b64": base64.b64encode(d).decode()})
        finally:
            self.call("guest-file-close", handle=h)


def main(argv):
    if len(argv) < 2:
        print(__doc__); return 2
    q = Qga()
    op = argv[1]
    if op == "ping":
        print(json.dumps(q.call("guest-info"))[:300]); return 0
    if op == "ps":
        code, out, err = q.exec_wait("powershell.exe", ["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", argv[2]])
    elif op == "cmd":
        code, out, err = q.exec_wait("cmd.exe", ["/c", argv[2]])
    elif op == "get":
        q.get(argv[2], argv[3]); print(f"fetched {argv[2]} -> {argv[3]} ({os.path.getsize(argv[3])} bytes)"); return 0
    elif op == "put":
        q.put(argv[2], argv[3]); print(f"sent {argv[2]} -> {argv[3]}"); return 0
    else:
        print(__doc__); return 2
    sys.stdout.write(out)
    if err:
        sys.stderr.write(err)
    print(f"[exit {code}]")
    return 0 if code == 0 else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
