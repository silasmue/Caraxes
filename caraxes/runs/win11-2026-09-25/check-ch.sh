#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# P21 acceptance checks (P20s); the P18 base: (P17-owned-compute-cta/ANALYSIS.md "Next GPU run must show").
# Usage: check-p18.sh <run dir>   (gpu.log, gpu-trace.log, host-kernel.log, d3d-probe-status.json)
# Read-only; prints one line per check and writes checks.txt into the run dir.
set -uo pipefail
export LC_ALL=C
R=${1:?run dir}
T=$R/gpu-trace.log G=$R/gpu.log K=$R/host-kernel.log
out=$R/checks.txt
: > "$out"
fail=0
say() { printf '%-5s %s\n' "$1" "$2" | tee -a "$out"; [[ $1 == FAIL ]] && fail=1; return 0; }
n() { grep -c -E "$1" "$2" 2>/dev/null || true; }

for required in "$G" "$K" "$R/start.json"; do
    [[ -r $required ]] || { say FAIL "required evidence missing: $required"; exit 1; }
done
trace_available=0
if [[ -s $T ]]; then
    trace_available=1
elif python3 -c 'import json,sys; sys.exit(not json.load(open(sys.argv[1])).get("scored", False))' "$R/start.json"; then
    say SKIP "trace-dependent checks: scored run intentionally omits gpu-trace.log"
else
    say FAIL "missing or empty gpu-trace.log in a diagnostic run"
fi

if (( trace_available )); then
# 1. Fix 1: every lower-level COPY_SERVER_RESERVED_PDES keeps the root; none is installed.
kept=$(n 'ctrl 0x90f10106: .*not a root' "$T")
installed=$(n 'ctrl 0x90f10106: vaspace [^:]*: root Some\(.*levels \[0x[0-9a-f]+\+0x1000 ap [0-9] shift (29|38)\]' "$T")
[[ $installed == 0 ]] && say PASS "root kept: $kept lower-level call(s) kept the root, 0 installed as root" \
    || say FAIL "root kept: $installed lower-level call(s) installed as the VAS root ($kept kept)"

# 2. Fix 2: no VAS walked from a root the guest's invalidates left; PDBs named vs roots recorded.
watch=$(n 'is walked from root' "$T")
[[ $watch == 0 ]] && say PASS "root watch: no mismatch (pdbwatch ERROR lines 0)" \
    || say FAIL "root watch: $watch mismatch line(s) (doorbells held): grep 'is walked from root' $T"
perl -ne '
    $r{$1} = 1 while /(?:ctrl 0x90f10106|set page directory)[^\n]*root Some\(Fb\((\d+)\)\)/g;
    if (/MEM_OP (?:targeted )?tlb invalidate .* pdb Fb\((\d+)\)/) { $m{$1}++ }
    # Traces before the P18 build carry no decoded PDB: C 0:0 ONE, aperture 11:10 vidmem.
    elsif (/MEM_OP (?:targeted )?tlb invalidate \(op 0x[9a], A \S+ B \S+ C (0x[0-9a-f]+)\)/) {
        my $c = hex($1); $m{$c & 0xfffff000}++ if !($c & 1) && !(($c >> 10) & 3);
    }
    if (/mmu invalidate: pdb (0x[0-9a-f]+)/) { $g{hex($1)}++ }
    END {
        my @m = sort { $a <=> $b } keys %m; my @g = sort { $a <=> $b } keys %g;
        printf "INFO  MEM_OP PDBs: %s\n", join(" ", map { sprintf("%#x%s x%d", $_, $r{$_} ? "=root" : "", $m{$_}) } @m) || "none";
        printf "INFO  register PDBs (roots only): %s\n", join(" ", map { sprintf("%#x x%d", $_, $g{$_}) } grep { $r{$_} } @g) || "none";
        my @x = grep { !$r{$_} } @m;
        printf "INFO  MEM_OP PDBs that are no recorded VAS root: %s\n", @x ? join(" ", map { sprintf("%#x", $_) } @x) : "none";
    }' "$T" | tee -a "$out"

# 3./4. Shadow refusals: none at VA >= 2^40, no host 0x1f refusal.
high=$(perl -ne 'print "$1\n" if /shadow: (?:REFUSED leaf|map) va (0x[0-9a-f]+).*(REFUSED|refused)/' "$T" \
    | perl -ne 'chomp; $c++ if hex($_) >= (1 << 40); END { print $c // 0 }')
[[ $high == 0 ]] && say PASS "no refused shadow leaf at VA >= 2^40" || say FAIL "$high refused shadow leaf/leaves at VA >= 2^40"
r1f=$(n 'refused 0x1f' "$T")
[[ $r1f == 0 ]] && say PASS "no 'refused 0x1f'" || say FAIL "$r1f 'refused 0x1f' shadow map(s)"
mal=$(n 'malformed table entries, neither followed' "$T")
[[ $mal == 0 ]] && say PASS "no malformed table entries (fix 3)" || say FAIL "$mal shadow root(s) with malformed entries (fix 3 refused them): wrong root?"

# P19 (F1): every shadow VAS starts at 0, no guest leaf below 1 MiB refused.
vs=$(grep -oE 'host-managed VAS 0x[0-9a-f]+ for guest root 0x[0-9a-f]+, VAS start 0x[0-9a-f]+' "$T" | awk '{print $NF}' | sort | uniq -c | tr '\n' ' ')
say INFO "shadow VAS starts: ${vs:-none logged}"
nz=$(n 'ALLOW_ZERO_ADDRESS not honoured' "$T")
if [[ -z $vs ]]; then say FAIL "no shadow VAS start logged (a build before F1?)"
elif [[ $nz == 0 ]]; then say PASS "every default-layout shadow VAS starts at 0"
else say FAIL "$nz shadow VAS(es) where the host ignored ALLOW_ZERO_ADDRESS"; fi
low=$(perl -ne 'print "$1\n" if /shadow: (?:REFUSED leaf|map) va (0x[0-9a-f]+) .*(?:REFUSED|refused)/' "$T" \
    | perl -ne 'chomp; $c++ if hex($_) < (1 << 20); END { print $c // 0 }')
[[ $low == 0 ]] && say PASS "no refused leaf below 1 MiB" || say FAIL "$low refused shadow leaf/leaves below 1 MiB"
fi
# P19: host-owned VAs in the guest's kernel-RM range (0x51 NO_MEMORY, F2 open).
h51=$(perl -ne 'if (/VA Space alloc failed! Status Code: 0x51 Size: \S+ RangeLo: (0x[0-9a-f]+)/) { my $v = hex($1); $c++ if $v >= 0x120000000 && $v < 0x122000000 } END { print $c // 0 }' "$K")
[[ $h51 == 0 ]] && say PASS "no 0x51 in 0x1_2000_0000..0x1_2200_0000" || say FAIL "$h51 host VA alloc 0x51 in 0x1_2000_0000..0x1_2200_0000 (host-owned VAs the guest uses; F2 open)"
# F2a: the host's global context sizes (unrounded) and the clipped exclusions.
grep -oE 'global context size: .{0,160}' "$G" | sed 's/^/INFO    /' | tee -a "$out"
cut=$(n 'set-B exclusion cut short' "$G")
say INFO "F2a clips: $cut exclusion(s) cut short at a guest page past the host footprint"
grep -m6 -oE 'WARN: shadow root .{0,220}exclusion cut short.{0,120}' "$G" | sed 's/^/INFO    /' | tee -a "$out"
if (( trace_available )); then
# P20: pages the guest maps inside a footprint are its own copy of the buffer (expected).
c1a=$(grep -cE 'refused: per-leaf VA object 0x1a' "$T" 2>/dev/null || true)
[[ $c1a == 0 ]] && say PASS "no guest leaf refused as a collision with host-owned VAs (0x1a)" || say FAIL "$c1a guest leaf/leaves refused 0x1a (host set B at a guest VA; F2b)"
# P24 (F2b): fenced VASes, holes, and where the host's refusals now fall.
fv=$(n 'fence \[0x120000000, 0x140000000\) held by the host' "$T"); fh=$(n 'a hole in the fence|fence \[.*refused' "$T")
say INFO "fenced shadow VASes: $fv, fence holes/refusals: $fh"
fi
perl -ne 'if (/VA Space alloc failed! Status Code: (0x[0-9a-f]+) Size: \S+ RangeLo: (0x[0-9a-f]+)/) { my $v = hex($2); my $b = $v < 0x120000000 ? "below the fence" : $v < 0x140000000 ? "inside the fence" : "above the fence"; $c{"$1 $b"}++ } END { printf "INFO  host VA alloc failed %s: %d\n", $_, $c{$_} for sort keys %c }' "$K" | tee -a "$out"
# P22: GFXP BINDs by outcome, the distinct refused payloads, held-channel replays.
ok=$(n 'P17 EXPERIMENTAL: owned host GFXP' "$G"); ref=$(n 'P17 native GFXP binding refused' "$G")
say INFO "GFXP BINDs: $ok bound, $ref refused"
grep -oE 'P17 native GFXP binding refused: .*payload [0-9a-f]+' "$G" | sed -E 's/.*payload //' | sort | uniq -c | sed 's/^/INFO    refused payload x/' | tee -a "$out"
if (( trace_available )); then
rp=$(n 'enabled again, its held doorbell runs now' "$T")
say INFO "held-doorbell replays: $rp (P21: several thousand)"
# F3: the shadow fault diagnostics of MMU-fault RCs.
fd=$(n 'shadow fault diagnostic' "$T")
say INFO "shadow fault diagnostics: $fd (grep 'shadow fault diagnostic' $T)"
grep -m5 -oE 'shadow fault diagnostic: .{0,300}' "$T" | sed 's/^/INFO    /' | tee -a "$out"

# 5. GPFIFOs covered (CARAXES_SHADOW_CHANNEL_DIAG=1 records, joined over their line breaks).
perl -0777 -ne '
    while (/shadow diagnostic: chid (\d+) GPFIFO start (0x[0-9a-f]+).*?bytes: guest (gap at \S+ after \d+|covered \[[^)]*\) by \d+)/sg) {
        my ($c, $s, $g) = ($1, $2, $3); $n++;
        if ($g =~ /^gap/) { $gap++; push @g, "chid $c $s ($g)" } else { $cov++ }
    }
    END { printf "%s GPFIFOs covered: %d record(s), %d covered, %d gap(s)%s\n", ($gap ? "FAIL " : $n ? "PASS " : "SKIP "),
          $n // 0, $cov // 0, $gap // 0, $gap ? ": " . join("; ", @g[0 .. ($#g < 5 ? $#g : 5)]) : "" }' "$T" | tee -a "$out"
grep -q '^FAIL  GPFIFOs\|^FAIL GPFIFOs' "$out" && fail=1
fi

# 6. Host Xid (any; 31 decisive) and VA-space update failures.
# Beside Linux B the host log holds both devices: attribute by pid (start.json win_device_pid).
wpid=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("win_device_pid",""))' "$R/start.json" 2>/dev/null)
if [[ -n $wpid ]]; then
    xid=$(n "NVRM: Xid .*pid=$wpid," "$K"); x31=$(n "NVRM: Xid .*: 31, pid=$wpid," "$K")
    other=$(( $(n 'NVRM: Xid' "$K") - xid ))
    # The live host log by the device's pid too (read-only `sudo -n dmesg`): a host-kernel.log
    # cut by a line count missed C80's 54 Xid 31 lines once the ring buffer had dropped lines.
    live=$(sudo -n dmesg 2>/dev/null | grep -c "NVRM: Xid .*pid=$wpid,")
    live31=$(sudo -n dmesg 2>/dev/null | grep -c "NVRM: Xid .*: 31, pid=$wpid,")
    (( live > xid )) && xid=$live
    (( live31 > x31 )) && x31=$live31
    [[ $other == 0 ]] && say PASS "host: no Xid of another device (Linux B)" || say FAIL "host: $other Xid of another pid (Linux B?): grep 'NVRM: Xid' $K"
else
    xid=$(n 'NVRM: Xid' "$K"); x31=$(n 'NVRM: Xid .*: 31,' "$K")
fi
[[ $xid == 0 ]] && say PASS "host: no Xid of the Windows device${wpid:+ (pid $wpid)}" || say FAIL "host: $xid Xid ($x31 Xid 31) of the Windows device: grep 'NVRM: Xid' $K"
vau=$(n "can't update VA space" "$K")
say INFO "host: $vau \"can't update VA space\" line(s)"

# 7. Frame-0 readback (and the full gate).
st=$R/d3d-probe-status.json
if [[ -s $st ]]; then
    txt=$(python3 -c 'import json,base64,sys; r=json.load(open(sys.argv[1])); print(base64.b64decode(r.get("out-data","")).decode("utf8","replace"))' "$st")
    grep -q 'frame 0 verified' <<<"$txt" && say PASS "frame-0 readback complete" || say FAIL "frame-0 readback not complete: $(grep -E 'stage|TIMEOUT|HRESULT|mismatch' <<<"$txt" | tail -1)"
    grep -q 'PASS: NVIDIA D3D11' <<<"$txt" && say PASS "D3D11 gate: $(grep 'PASS: NVIDIA' <<<"$txt")" || say FAIL "D3D11 gate not passed"
else
    say FAIL "no d3d-probe-status.json (run: run44.py probe start, then probe status)"
fi

# Context: device errors and RCs.
say INFO "gpu.log: $(n 'ERROR' "$G") ERROR line(s)"
if (( trace_available )); then
    say INFO "guest RC: $(n 'RC queued|guest RC' "$T") note(s) in the trace"
else
    say SKIP "guest RC trace count unavailable"
fi
if (( fail )); then
    echo "CH checks: FAILURES (see above)" | tee -a "$out"
elif (( trace_available )); then
    echo "CH checks: AVAILABLE CHECKS PASS (review SKIP lines)" | tee -a "$out"
else
    echo "CH checks: AVAILABLE CHECKS PASS; trace-dependent checks skipped" | tee -a "$out"
fi
exit $fail
