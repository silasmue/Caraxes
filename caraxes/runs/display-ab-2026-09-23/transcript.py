# SPDX-License-Identifier: MIT
# Turn the AUX and DP_CTRL lines of a device trace into a dp-sink transcript (the line
# format of crates/dp-sink/src/transcript.rs): every DP_AUXCH_CTRL the guest sent, with
# the reply the device gave, and the link training DP_CTRL did behind the control.
# Usage: python3 transcript.py <device-trace.txt> <display-id hex, e.g. 100>
import re
import sys

path, did = sys.argv[1], sys.argv[2].lower().lstrip("0x") or "0"
aux = re.compile(r"aux 0x" + did + r": (.*) -> (.*?)( \(.*\))? -> status 0x[0-9a-f]+ \[")
trained = re.compile(r"dp ctrl of 0x" + did + r": cmd (0x[0-9a-f]+): trained (\d+) lanes at link bw 0x([0-9a-f]+)( \(sink set to D0 first)?")
down = re.compile(r"dp ctrl of 0x" + did + r": cmd 0x[0-9a-f]+ data 0x[0-9a-f]+: link powered down")
print("! plug")
print("? events plug")
for line in open(path, errors="replace"):
    m = aux.search(line)
    if m:
        print(f"> {m.group(1)}")
        print(f"< {m.group(2)}")
        continue
    m = trained.search(line)
    if m:
        if m.group(4):
            print("# DP_CTRL: the device puts the sink in D0 before training (as GSP does)")
            print("> nw 600 01")
            print("< ack 1")
        ef = " ef" if int(m.group(1), 16) & 0x80 else ""
        print(f"# DP_CTRL: trained through the control")
        print(f"! train {m.group(3)} {m.group(2)}{ef}")
        continue
    if down.search(line):
        print("# DP_CTRL: lane count 0, link powered down")
        print("! drop-link")
