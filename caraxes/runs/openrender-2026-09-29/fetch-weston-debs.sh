#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# The local apt repository with weston for vm10 (docs/research/open-render-2026-09-29.md, R8):
# Ubuntu noble's weston 13 and its whole dependency closure, fetched on the host so that the
# VM needs no network. Anonymous HTTP only (no credentials, no identifying headers).
#   1. noble, noble-updates, noble-security InRelease: signature checked with the Ubuntu
#      Archive Automatic Signing Key (2018), fingerprint pinned below (fetched from
#      keyserver.ubuntu.com into a private keyring);
#   2. main and universe Packages.xz of each: SHA256 checked against InRelease;
#   3. the closure (apt-closure.py; newest version over the pockets);
#   4. every .deb: SHA256 checked against its index stanza;
#   5. a flat repository: Packages, Packages.gz, Release (unsigned; the guest's source line
#      says trusted=yes, and only this script writes it).
# The guest installs from it with apt, only packages it lacks (openrender-check.sh R8).
# Usage: fetch-weston-debs.sh [out dir] (default /mnt/vmstore/caraxes/openrender-debs).
set -euo pipefail
HERE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd -P)
OUT=${1:-/mnt/vmstore/caraxes/openrender-debs}
MIRROR=http://archive.ubuntu.com/ubuntu
KEY_FPR=F6ECB3762474EDA9D21B7022871920D1991BC93C
PACKAGES=weston
mkdir -p "$OUT/.index"
cd "$OUT/.index"
if [[ ! -d gnupg ]]; then
    mkdir -m 0700 gnupg
    curl -sSf -m 60 -o archive-key.asc "https://keyserver.ubuntu.com/pks/lookup?op=get&search=0x$KEY_FPR"
    gpg --homedir gnupg --quiet --import archive-key.asc
fi
gpg --homedir gnupg --with-colons --fingerprint | grep -q "^fpr:::::::::$KEY_FPR:" ||
    { echo "ERROR: archive key fingerprint is not $KEY_FPR" >&2; exit 1; }
indexes=()
for d in noble noble-updates noble-security; do
    curl -sSf -m 120 -o "$d-InRelease" "$MIRROR/dists/$d/InRelease"
    gpg --homedir gnupg --status-fd 1 --verify "$d-InRelease" 2>/dev/null | grep -q "^\[GNUPG:\] VALIDSIG $KEY_FPR" ||
        { echo "ERROR: $d InRelease is not signed by $KEY_FPR" >&2; exit 1; }
    for c in main universe; do
        curl -sSf -m 300 -o "$d-$c.xz" "$MIRROR/dists/$d/$c/binary-amd64/Packages.xz"
        want=$(awk '/^SHA256:/{f=1;next} /^[A-Za-z]/{f=0} f' "$d-InRelease" |
            awk -v n="$c/binary-amd64/Packages.xz" '$3==n{print $1}')
        [[ $(sha256sum "$d-$c.xz" | cut -d' ' -f1) == "$want" ]] ||
            { echo "ERROR: $d $c Packages.xz does not match InRelease" >&2; exit 1; }
        indexes+=("$PWD/$d-$c.xz")
    done
done
python3 "$HERE/apt-closure.py" "$(IFS=,; echo "${indexes[*]}")" "$PACKAGES" > closure.txt
cd "$OUT"
while read -r _ _ file _ sha; do
    deb=$(basename "$file")
    if [[ ! -s $deb || $(sha256sum "$deb" | cut -d' ' -f1) != "$sha" ]]; then
        curl -sSf -m 300 -o "$deb" "$MIRROR/$file"
    fi
    [[ $(sha256sum "$deb" | cut -d' ' -f1) == "$sha" ]] || { echo "ERROR: $deb checksum" >&2; exit 1; }
done < .index/closure.txt
python3 - "$OUT" <<'PY'
import gzip, hashlib, lzma, os, sys
out = sys.argv[1]
want = {}
for line in open(os.path.join(out, ".index", "closure.txt")):
    n, v, f, _, _ = line.split()
    want[(n, v)] = os.path.basename(f)
stanzas = {}
for name in sorted(os.listdir(os.path.join(out, ".index"))):
    if not name.endswith(".xz"):
        continue
    for block in lzma.open(os.path.join(out, ".index", name), "rt", errors="replace").read().split("\n\n"):
        fields = dict(l.split(":", 1) for l in block.split("\n") if ":" in l and not l.startswith(" "))
        key = (fields.get("Package", "").strip(), fields.get("Version", "").strip())
        if key in want and key not in stanzas:
            stanzas[key] = "\n".join(
                "Filename: ./" + want[key] if l.startswith("Filename:") else l
                for l in block.strip().split("\n"))
assert len(stanzas) == len(want), (len(stanzas), len(want))
data = ("\n\n".join(stanzas[k] for k in sorted(stanzas)) + "\n").encode()
open(os.path.join(out, "Packages"), "wb").write(data)
with gzip.open(os.path.join(out, "Packages.gz"), "wb") as f:
    f.write(data)
import email.utils, time
rel = ["Origin: Caraxes openrender", "Label: Ubuntu noble packages for vm10 (weston closure)",
       "Date: " + email.utils.formatdate(time.time(), usegmt=True),
       "Architectures: amd64 all", "SHA256:"]
for name in ("Packages", "Packages.gz"):
    b = open(os.path.join(out, name), "rb").read()
    rel.append(f" {hashlib.sha256(b).hexdigest()} {len(b)} {name}")
open(os.path.join(out, "Release"), "w").write("\n".join(rel) + "\n")
print(f"{out}: {len(stanzas)} packages")
PY
du -sh "$OUT" | tail -1
