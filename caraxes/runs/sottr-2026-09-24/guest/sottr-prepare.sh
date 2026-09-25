#!/bin/bash
# SPDX-License-Identifier: MIT
# Guest (root): the SotTR stage's one-time preparation of a fresh overlay on the 610.57.04 base
# image (docs/caraxes/sottr-stage.md section 3). The steps are those vm0 carries (read from its
# overlay with guestfish --ro): the per-VM driver build of quickstart.md section 2 with all four
# modules (experiments.md, display D0), the modprobe files, the GNOME Wayland switch of
# gdm-wayland-setup.sh and nvidia-egl-setup.sh on (display-e.md section 2); then the games disk
# as v1 mounted it (Leandro-Test scripts/lib/provision.sh:656-742: /games by label, nofail), the
# Steam library registration, Steam's autostart, the user-namespace sysctl of v1
# (provision.sh:1839-1848), and Sunshine's KMS capability. A step with a marker in
# /var/lib/caraxes-sottr is skipped. Inputs in the user's home (up.sh copies them):
# ogkm-610.57.04.tar.zst, gsp_tu10x-610.57.04.bin, caraxes-guest/ (crates/vfio-user-nvgpu/guest).
# SOTTR_DRIVER=615 (vm4-615): the 615 steps at the end instead of the driver build, the firmware
# and /opt/nvrm-gl: the distribution packages (packages615), the module rules (modprobe615) and
# gdm on Wayland (display615), then the shared steps.
# Usage: sudo [SOTTR_DRIVER=615 SOTTR_PKG_VERSION=...] bash sottr-prepare.sh [step...]   (default: all steps in order)
set -u
U=leandro H=/home/leandro M=/var/lib/caraxes-sottr KV=$(uname -r)
LABEL=LEAGAMES MNT=/games
mkdir -p "$M"
as_user() { sudo -u "$U" -H bash -c "$1"; }
todo() { if [ -f "$M/$1.done" ]; then echo "== $1: done before ($(cat "$M/$1.done"))"; return 1; fi; echo "== $1"; }
done_() { date -Iseconds > "$M/$1.done"; sync; }

modules() {
    todo modules || return 0
    [ -f $H/ogkm-610.57.04.tar.zst ] || { echo "missing $H/ogkm-610.57.04.tar.zst"; return 1; }
    as_user "mkdir -p ~/ogkm && tar -C ~/ogkm --zstd -xf ~/ogkm-610.57.04.tar.zst" || return 1
    # nvidia, nvidia-uvm, nvidia-modeset, nvidia-drm, nvidia-peermem against the guest's headers.
    as_user "cd ~/ogkm/open-gpu-kernel-modules && nice make modules -j\$(nproc) SYSSRC=/lib/modules/$KV/build > ~/ogkm-build.log 2>&1" \
        || { tail -20 $H/ogkm-build.log; return 1; }
    (cd $H/ogkm/open-gpu-kernel-modules && make modules_install SYSSRC=/lib/modules/$KV/build > $H/ogkm-install.log 2>&1) \
        || { tail -20 $H/ogkm-install.log; return 1; }
    depmod -a
    find /lib/modules/"$KV" -name 'nvidia*.ko*' | sort
    done_ modules
}

firmware() {
    todo firmware || return 0
    install -D -m 644 $H/gsp_tu10x-610.57.04.bin /lib/firmware/nvidia/610.57.04/gsp_tu10x.bin || return 1
    sha256sum /lib/firmware/nvidia/610.57.04/gsp_tu10x.bin
    done_ firmware
}

modprobe_conf() {
    todo modprobe || return 0
    # vm0's /etc/modprobe.d/caraxes.conf and caraxes-display.conf, verbatim.
    cat > /etc/modprobe.d/caraxes.conf <<'EOF'
options nvidia NVreg_GpuInitOnProbe=1 NVreg_ResmanDebugLevel=0 NVreg_RegistryDwords="RmDisableGlobalCeUtils=1;RMDisableScrubOnFree=1;RmRcWatchdog=0"
blacklist nvidiafb
options nvidia-uvm uvm_disable_hmm=1
blacklist nvidia
EOF
    cat > /etc/modprobe.d/caraxes-display.conf <<'EOF'
# Caraxes display phases A/B (docs/caraxes/display-ab.md): never autoload, load by hand.
blacklist nvidia-drm
blacklist nvidia-modeset
options nvidia-drm modeset=1 fbdev=0
EOF
    done_ modprobe
}

display() {
    todo display || return 0
    cd $H/caraxes-guest || return 1
    bash gdm-wayland-setup.sh || return 1
    bash nvidia-egl-setup.sh on || return 1
    # gdm3 is started by hand after the display modules are loaded (nvidia is blacklisted from
    # autoloading); the base image boots to graphical.target, vm0 to multi-user.target.
    systemctl set-default multi-user.target
    as_user "cd ~/caraxes-guest && cc -O2 -o kmsgrab kmsgrab.c" || return 1
    done_ display
}

sunshine_prep() {
    todo sunshine || return 0
    # Sunshine's uinput devices (Moonlight's mouse, keyboard and pad), as on vm0.
    echo 'KERNEL=="uinput", GROUP="input", MODE="0660", OPTIONS+="static_node=uinput"' > /etc/udev/rules.d/60-sunshine-uinput.rules
    usermod -aG input "$U"
    udevadm control --reload-rules; udevadm trigger --sysname-match=uinput 2>/dev/null
    # KMS capture needs CAP_SYS_ADMIN; as a file capability Sunshine runs in the user's session
    # (audio, the session's Wayland socket) instead of as root.
    local bin
    bin=$(readlink -f "$(command -v sunshine)")
    getcap "$bin" | grep -q cap_sys_admin || setcap cap_sys_admin+p "$bin"
    getcap "$bin"
    done_ sunshine
}

games() {
    todo games || return 0
    local dev
    dev=$(lsblk -ndo NAME,LABEL,TYPE | awk -v l="$LABEL" '$3=="disk" && $2==l {print $1; exit}')
    [ -n "$dev" ] || { echo "no disk with label $LABEL (CARAXES_GAMES_DISK unset?)"; lsblk -o NAME,SIZE,FSTYPE,LABEL; return 1; }
    mkdir -p "$MNT"
    sed -i "\|^LABEL=$LABEL |d" /etc/fstab
    echo "LABEL=$LABEL $MNT ext4 defaults,nofail,x-systemd.device-timeout=10 0 2" >> /etc/fstab
    systemctl daemon-reload
    mountpoint -q "$MNT" || mount "$MNT" || return 1
    echo "/dev/$dev at $MNT: $(df -h "$MNT" | awk 'NR==2 {print $2" total, "$4" free"}')"
    done_ games
}

# The library on the disk is /games/SteamLibrary (a second library folder the operator added in
# v1). Steam finds library folders through libraryfolders.vdf in its root; the Ubuntu launcher's
# root for a new installation is ~/.steam/debian-installation (/usr/games/steam). Entry 0 is the
# root, entry 1 the disk's folder with its own contentid and the apps of its manifests.
steam_prep() {
    todo steam || return 0
    local R=$H/.steam/debian-installation L=$MNT/SteamLibrary f cid apps=""
    [ -f $L/libraryfolder.vdf ] || { echo "no $L/libraryfolder.vdf (games disk not mounted?)"; return 1; }
    cid=$(sed -n 's/.*"contentid"[[:space:]]*"\([0-9]*\)".*/\1/p' $L/libraryfolder.vdf | head -1)
    for f in $L/steamapps/appmanifest_*.acf; do
        apps="$apps$(awk -F'"' '$2=="appid"{a=$4} $2=="SizeOnDisk"{s=$4} END{printf "\t\t\t\"%s\"\t\t\"%s\"\n", a, s}' "$f")"$'\n'
    done
    mkdir -p $R/steamapps $R/config
    {
        printf '"libraryfolders"\n{\n\t"0"\n\t{\n\t\t"path"\t\t"%s"\n\t\t"label"\t\t""\n\t\t"totalsize"\t\t"0"\n\t\t"apps"\n\t\t{\n\t\t}\n\t}\n' "$R"
        printf '\t"1"\n\t{\n\t\t"path"\t\t"%s"\n\t\t"label"\t\t""\n\t\t"contentid"\t\t"%s"\n\t\t"totalsize"\t\t"0"\n\t\t"apps"\n\t\t{\n%s\t\t}\n\t}\n}\n' "$L" "$cid" "$apps"
    } > $R/steamapps/libraryfolders.vdf
    cp $R/steamapps/libraryfolders.vdf $R/config/libraryfolders.vdf
    # The launcher's control links (it makes them too; made here so that Steam's first start
    # already resolves ~/.steam/steam to the root that holds the registration).
    ln -sfn $R $H/.steam/steam; ln -sfn $R $H/.steam/root
    # Steam in the GNOME session, so the streamed desktop shows its installer and login.
    mkdir -p $H/.config/autostart
    cat > $H/.config/autostart/steam.desktop <<'EOF'
[Desktop Entry]
Type=Application
Name=Steam
Comment=Caraxes SotTR stage: Steam in the session (docs/caraxes/sottr-stage.md)
Exec=/usr/games/steam
Icon=steam
X-GNOME-Autostart-enabled=true
X-GNOME-Autostart-Delay=5
EOF
    chown -R "$U:$U" $H/.steam $H/.config/autostart
    # pressure-vessel's bwrap needs unprivileged user namespaces (v1, OPEN-QUESTIONS nr 11).
    echo "kernel.apparmor_restrict_unprivileged_userns = 0" > /etc/sysctl.d/99-lea-userns.conf
    sysctl -q -p /etc/sysctl.d/99-lea-userns.conf
    cat $R/steamapps/libraryfolders.vdf
    done_ steam
}

# Software cursor for Mutter: the hardware cursor plane (plane-1 on nvidia-drm) is not part of
# the primary framebuffer Sunshine's KMS capture reads, so a hardware cursor is invisible in the
# stream (session of 2026-09-25). Read by systemd --user and gdm's Wayland session. No marker:
# runs every time.
cursor() {
    echo "== cursor"
    mkdir -p /etc/environment.d
    printf 'MUTTER_DEBUG_DISABLE_HW_CURSORS=1\n' > /etc/environment.d/50-software-cursor.conf
    echo "software cursor: /etc/environment.d/50-software-cursor.conf"
}

# 610 only. Steam's runtime container (pressure-vessel) imports the NVIDIA Vulkan and GL driver
# from the host's ld.so cache and the ICD json; the image keeps the driver in /opt/nvrm-gl, which
# was not in the cache, so the game saw only Mesa's ICDs and could not create a Vulkan device
# (session of 2026-09-25, the crash dump lists libvulkan_nouveau/intel/lvp only). No marker.
nvrmgl() {
    echo "== nvrmgl"
    printf '/opt/nvrm-gl/lib\n/opt/nvrm-gl/lib32\n' > /etc/ld.so.conf.d/00-nvrm-gl.conf && ldconfig
    sed -i 's#"library_path": "libGLX_nvidia.so.0"#"library_path": "/opt/nvrm-gl/lib/libGLX_nvidia.so.0"#' /usr/share/vulkan/icd.d/nvidia_icd.json
    echo "nvrm-gl in ld.so.cache; nvidia_icd.json with an absolute path"
}

# ---- The 615 steps (SOTTR_DRIVER=615, vm4-615 on the 615 base image): the driver as Ubuntu
# installs it. The image has nvidia-headless-open, nvidia-dkms-open (modules built for its
# kernel), libnvidia-compute and nvidia-firmware of 615.71.09 from NVIDIA's CUDA repository;
# it lacks the GL, EGL, Vulkan, encode and decode libraries and linux-modules-extra
# (drm_display_helper, which the packaged nvidia-drm needs for its HDCP property). apt fetches
# them from the public repositories the image already lists; the pinning package keeps every
# driver package at 615.71.09.
packages615() {
    todo packages615 || return 0
    local V=${SOTTR_PKG_VERSION:?SOTTR_PKG_VERSION unset} K=${SOTTR_GUEST_KERNEL:-$KV}
    local up=${V%%-*}
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -q || return 1
    apt-get install -y -q "nvidia-driver-pinning-$up=$V" || return 1
    apt-get install -y -q --no-install-recommends "nvidia-driver-open=$V" "libnvidia-gl=$V" \
        "libnvidia-decode=$V" "libnvidia-encode=$V" "libnvidia-fbc1=$V" "linux-modules-extra-$K" || return 1
    # The 32-bit GL and Vulkan driver for Steam's 32-bit parts; the 64-bit game does not need it.
    apt-get install -y -q --no-install-recommends "libnvidia-gl:i386=$V" || echo "note: libnvidia-gl:i386 not installed"
    dpkg -l 'nvidia-*' 'libnvidia-*' "linux-modules-extra-$K" 2>/dev/null | grep '^ii ' | awk '{print $1, $2, $3}'
    echo "-- files"
    ls -la /usr/share/vulkan/icd.d/nvidia_icd.json /usr/share/glvnd/egl_vendor.d/ /usr/share/egl/egl_external_platform.d/ \
        /usr/lib/x86_64-linux-gnu/gbm/ 2>&1
    cat /usr/share/vulkan/icd.d/nvidia_icd.json
    for f in /usr/share/vulkan/icd.d/nvidia_icd.json /usr/share/glvnd/egl_vendor.d/*nvidia* /usr/share/egl/egl_external_platform.d/* \
             /usr/lib/x86_64-linux-gnu/gbm/*; do
        [ -e "$f" ] || continue
        printf '%s: %s\n' "$f" "$(dpkg -S "$f" 2>/dev/null | cut -d: -f1 | head -1 || true)"
    done
    ldconfig -p | grep -E 'libGLX_nvidia|libEGL_nvidia|libnvidia-(eglcore|glcore|encode|allocator)|libnvcuvid|libcuda.so.1' | sort
    modinfo -n drm_display_helper
    modinfo -n nvidia_drm
    done_ packages615
}

modprobe615() {
    todo modprobe615 || return 0
    # up.sh writes caraxes-615-display.conf offline before the first boot; checked here.
    [ -f /etc/modprobe.d/caraxes-615-display.conf ] || { echo "no /etc/modprobe.d/caraxes-615-display.conf (up.sh writes it)"; return 1; }
    systemctl mask nvidia-persistenced.service 2>&1
    grep -H . /etc/modprobe.d/caraxes.conf /etc/modprobe.d/caraxes-615-display.conf
    grep -l nvidia /etc/modprobe.d/* /usr/lib/modprobe.d/* /lib/modprobe.d/* 2>/dev/null | sort -u
    done_ modprobe615
}

# GNOME on Wayland through gdm3 autologin (the part of gdm-wayland-setup.sh that is gdm's, not
# the driver's: no Mesa pin in /etc/environment, no nvidia-egl-setup.sh). gdm's udev rule that
# turns Wayland off on NVIDIA without NVreg_PreserveVideoMemoryAllocations is masked; gdm3 is
# started by hand after the display modules (default target multi-user).
display615() {
    todo display615 || return 0
    local A=/var/lib/AccountsService/users/$U
    sed -i 's/^WaylandEnable=false/WaylandEnable=true/' /etc/gdm3/custom.conf
    grep -q '^WaylandEnable=true' /etc/gdm3/custom.conf || sed -i 's/^\[daemon\]/[daemon]\nWaylandEnable=true/' /etc/gdm3/custom.conf
    grep -q '^AutomaticLoginEnable=true' /etc/gdm3/custom.conf || sed -i "s/^\[daemon\]/[daemon]\nAutomaticLoginEnable=true\nAutomaticLogin=$U/" /etc/gdm3/custom.conf
    ln -sf /dev/null /etc/udev/rules.d/61-gdm.rules
    if [ -f "$A" ]; then
        sed -i 's/^Session=ubuntu-xorg/Session=ubuntu-wayland/; s/^Session=ubuntu$/Session=ubuntu-wayland/; /^XSession=/d' "$A"
        grep -q '^Session=' "$A" || printf 'Session=ubuntu-wayland\n' >> "$A"
    else
        mkdir -p "${A%/*}"; printf '[User]\nSession=ubuntu-wayland\nSystemAccount=false\n' > "$A"
    fi
    systemctl restart accounts-daemon
    systemctl enable gdm3 2>/dev/null || true
    systemctl set-default multi-user.target
    grep -v '^#' /etc/gdm3/custom.conf | grep .
    grep Session "$A"
    as_user "cd ~/caraxes-guest && cc -O2 -o kmsgrab kmsgrab.c" || echo "note: kmsgrab not built"
    done_ display615
}

if [ "${SOTTR_DRIVER:-610}" = 615 ]; then
    [ $# -eq 0 ] && set -- packages615 modprobe615 display615 sunshine games steam cursor
else
    [ $# -eq 0 ] && set -- modules firmware modprobe display sunshine games steam cursor nvrmgl
fi
for s in "$@"; do
    case $s in
        modules) modules ;; firmware) firmware ;; modprobe) modprobe_conf ;; display) display ;;
        sunshine) sunshine_prep ;; games) games ;; steam) steam_prep ;;
        cursor) cursor ;; nvrmgl) nvrmgl ;;
        packages615) packages615 ;; modprobe615) modprobe615 ;; display615) display615 ;;
        *) echo "unknown step $s"; exit 2 ;;
    esac || { echo "step $s failed"; exit 1; }
done
sync
echo "prepared: $(ls $M)"
