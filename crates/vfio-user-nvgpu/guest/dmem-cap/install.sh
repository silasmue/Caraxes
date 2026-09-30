#!/bin/bash
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
# Install caraxes-dmem-cap in a guest (docs/research/dmem-cap-2026-09-29.md): the script, the
# user@.service drop-in (root half), the user unit, enabled for every user (user half), Sunshine
# in session.slice and the settings file (kept if present). With --remove, take all of it out
# again (the settings file stays). Run as root inside the guest: sudo ./install.sh [--remove]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
LIBEXEC=/usr/local/libexec/caraxes
DROPIN=/etc/systemd/system/user@.service.d/caraxes-dmem.conf
UNIT=/etc/systemd/user/caraxes-dmem-cap.service
SUNSHINE=/etc/systemd/user/app-dev.lizardbyte.app.Sunshine.service.d/caraxes-session-slice.conf
CONF=/etc/caraxes/dmem-cap.conf
(( EUID == 0 )) || { echo "run as root: sudo $0 $*" >&2; exit 1; }

user_managers() {  # "UID NAME" of each running user manager
    local d uid
    for d in /sys/fs/cgroup/user.slice/user-*.slice/user@*.service; do
        [[ -d $d ]] || continue
        uid=${d##*/user@}; uid=${uid%.service}
        printf '%s %s\n' "$uid" "$(id -nu "$uid" 2>/dev/null || echo "$uid")"
    done
}

if [[ ${1:-} == --remove ]]; then
    while read -r uid name; do
        systemctl --user --machine="$name@" stop caraxes-dmem-cap.service 2>/dev/null || true
    done < <(user_managers)
    systemctl --global disable caraxes-dmem-cap.service 2>/dev/null || true
    rm -f "$UNIT" "$DROPIN" "$SUNSHINE" "$LIBEXEC/caraxes-dmem-cap"
    rmdir --ignore-fail-on-non-empty "$(dirname "$DROPIN")" "$(dirname "$SUNSHINE")" "$LIBEXEC" 2>/dev/null || true
    systemctl daemon-reload
    while read -r uid name; do
        systemctl --user --machine="$name@" daemon-reload 2>/dev/null || true
    done < <(user_managers)
    # dmem stays enabled in cgroup.subtree_control until the next boot; it limits nothing.
    echo "removed; $CONF kept"
    exit 0
fi

install -D -m 0755 "$here/caraxes-dmem-cap" "$LIBEXEC/caraxes-dmem-cap"
install -D -m 0644 "$here/caraxes-dmem-delegate.conf" "$DROPIN"
install -D -m 0644 "$here/caraxes-dmem-cap.service" "$UNIT"
install -D -m 0644 "$here/sunshine-session-slice.conf" "$SUNSHINE"
[[ -e $CONF ]] || install -D -m 0644 "$here/dmem-cap.conf" "$CONF"
systemctl daemon-reload
systemctl --global enable caraxes-dmem-cap.service
# User managers that already run: the root half now, then let them see the new units. The
# user half starts with the next graphical session (or: systemctl --user start caraxes-dmem-cap).
while read -r uid name; do
    "$LIBEXEC/caraxes-dmem-cap" delegate "$uid"
    systemctl --user --machine="$name@" daemon-reload 2>/dev/null || true
done < <(user_managers)
echo "installed: $LIBEXEC/caraxes-dmem-cap, $DROPIN, $UNIT (enabled globally), $SUNSHINE, $CONF"
"$LIBEXEC/caraxes-dmem-cap" status | head -n 3 || true
