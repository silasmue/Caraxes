#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# In-guest GNOME Shell screenshot: own org.gnome.Screenshot (a caller GNOME Shell allows), then
# ask for the focused window (window FILE) or the whole screen (screen FILE); no portal dialog.
# Used for docs/vfio/showcase.md (2026-09-30). Usage: guest-screenshot.py window|screen FILE
import sys
import gi
gi.require_version('Gio', '2.0')
from gi.repository import Gio, GLib
what, path = sys.argv[1], sys.argv[2]
bus = Gio.bus_get_sync(Gio.BusType.SESSION)
bus.call_sync('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'RequestName',
              GLib.Variant('(su)', ('org.gnome.Screenshot', 4)), None, 0, -1, None)
if what == 'window':
    m, args = 'ScreenshotWindow', GLib.Variant('(bbbs)', (True, False, False, path))
else:
    m, args = 'Screenshot', GLib.Variant('(bbs)', (False, False, path))
print(bus.call_sync('org.gnome.Shell.Screenshot', '/org/gnome/Shell/Screenshot', 'org.gnome.Shell.Screenshot',
                    m, args, None, 0, -1, None))
