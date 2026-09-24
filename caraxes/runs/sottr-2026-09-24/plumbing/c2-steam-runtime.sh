# SPDX-License-Identifier: MIT
# Guest (desktop user), plumbing check 2: the default target vm0 has (sottr-prepare.sh gained
# this line after the first preparation), the game binary's libraries, the Steam Linux Runtime
# of the library (pressure-vessel, which needs the user namespaces sottr-prepare.sh allows),
# and the Ubuntu Steam launcher offline under Xvfb.
sudo systemctl set-default multi-user.target 2>&1
G="/games/SteamLibrary/steamapps/common/Shadow of the Tomb Raider"
echo "== the game binary: resolved libraries, and those left to the Steam runtime"
ldd "$G/bin/ShadowOfTheTombRaider" | grep -c "=> /"
ldd "$G/bin/ShadowOfTheTombRaider" | grep "not found"
echo "== the sniper runtime of the library"
S=/games/SteamLibrary/steamapps/common/SteamLinuxRuntime_sniper
head -3 "$S/VERSIONS.txt" 2>/dev/null
timeout 180 "$S/run" -- sh -c 'head -2 /etc/os-release; echo inside-container-ok' 2>&1 | tail -6
echo "== the Steam launcher, offline, under Xvfb for 20 s"
timeout 20 xvfb-run -a /usr/games/steam > /tmp/steam-try.log 2>&1
echo "launcher rc=$? (124: still waiting at the timeout)"
tail -5 /tmp/steam-try.log
ls ~/.steam/debian-installation
