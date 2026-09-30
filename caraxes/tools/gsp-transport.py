#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
"""gsp-transport.py: the transport class of every GSP-routed control of 615.71.09.

For each control the kernel RM sends to GSP (controls.tsv, route "gsp"), plus the GSS-legacy
and BINAPI ids that the profiles saw outside the export tables, this tool derives what the
cheapest safe transport would be (docs/caraxes/gsp-transport-classes.md):

  ZT-PAGE   the answer is static per card or per VM (or device-updated) and does not depend
            on per-handle state: it could live in a guest-readable page with a version word;
            for a stock driver that page is the guest RM's own control cache
  ZT-QUEUE  needs the device, not the host: served from the polled message queue without the
            head-write trap (the P24-style reactor)
  HOST      needs the host RM round trip; latency-bound; host_cache says whether a device-side
            cache can remove it (static per card: yes; telemetry: a TTL; per handle: no)
  TRAP      must be validated and applied synchronously, in order, before the reply: the
            request carries addresses, handles or a state change (of a host object or of an
            existing zero-trap window); never cached, batched or deferred
  REFUSE    the feature is absent on TU106, or refused for isolation

Inputs (read only):
  caraxes/abi/gsp-615.71.09/controls.tsv   route, size, flags, privilege, category, provenance
  docs/caraxes/gsp-plan/d-controls.tsv      D rules (REFUSE/DENY/ACK/MODEL) and isolation risk
  docs/caraxes/gsp-plan/b-host-forwardable.toml  B groups (F1/F2/S/G/R/N)
  the 615.71.09 tree (--ogkm): SDK params structs (embedded NvP64 pointers, NvHandle fields,
    address fields), the kernel RM call sites through gsp-surface.py's caller scan, the
    RMCTRL access rights of the export tables
  caraxes/runs/coverage-2026-09-25/{map,extra}/profile/dev.*.tsv.xz  the 610-guest profile
    windows (marks.tsv names them): per control count and device time
  615-guest device logs (transport-615, display-615): per control count, the device's
    rx-to-tx time, and the per-frame window of trap-table.txt

Output: caraxes/abi/gsp-615.71.09/transport.tsv (--out), and with --summary the Markdown
tables of the document on stdout.

Usage:
  python3 caraxes/tools/gsp-transport.py [--ogkm DIR] [--out FILE] [--summary]
  python3 caraxes/tools/gsp-transport.py --rpc-traps \
      caraxes/runs/display-615-2026-09-24/s4-series/traps-weston/device-trace.txt.xz

Python 3.8 or later, standard library only.
"""
import argparse
import collections
import glob
import importlib.util
import lzma
import math
import os
import re
import statistics
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
ABI = os.path.join(ROOT, "caraxes", "abi", "gsp-615.71.09")
PLAN = os.path.join(ROOT, "docs", "vfio", "gsp-plan")
RUNS = os.path.join(ROOT, "caraxes", "runs")

# The 610-guest profile series (coverage-map.md §2): run directory -> kind per window name.
PROFILE_RUNS = ["coverage-2026-09-25/map", "coverage-2026-09-25/extra"]
CUDA_WINDOWS = {"gate-cuda", "bench-init", "bench-short-auto", "bench-rl", "bench-ppo", "graphs",
                "streams", "managed", "texsurf", "zerocopy", "query", "torch-matmul",
                "torch-conv", "torch-attention", "multiproc", "oom", "nvrtc", "after-errors"}
WINDOW_KIND = {"boot": "boot", "setup": "setup", "gate-nvml": "query", "smi-q": "query",
               "smi-dmon": "query", "vulkan": "vulkan", "idle": "idle", "kill9": "error",
               "illegal": "error", "ctlsweep": "sweep"}
# The 615-guest device logs: file -> kind. A trap-table.txt beside a log marks the per-frame
# window inside it ("window 10 s [a, b) us").
EVENT_LOGS = [
    ("transport-615-2026-09-24/boot/device-events.txt", "boot"),
    ("transport-615-2026-09-24/reload-a/device-events.txt", "reload"),
    ("transport-615-2026-09-24/reload-b/device-events.txt", "reload"),
    ("display-615-2026-09-24/s4-series/display-up/device-events.txt", "display"),
    ("display-615-2026-09-24/s4-series/gnome-start-1/device-events.txt", "display"),
    ("display-615-2026-09-24/s4-series/gnome-start-2/device-events.txt", "display"),
    ("display-615-2026-09-24/s4-series/gnome-start-3/device-events.txt", "display"),
    ("display-615-2026-09-24/s4-series/q1-unplug-plug/device-events.txt", "display"),
    ("display-615-2026-09-24/s4-series/traps-weston/device-trace.txt.xz", "display"),
    ("display-615-2026-09-24/s4-series/traps-mutter/device-trace.txt.xz", "display"),
]

F_NON_PRIV, F_ROUTE, F_INTERNAL = 0x8, 0x40, 0x80
F_CACHEABLE, F_CACHEABLE_BY_INPUT = 0x400, 0x20000

RX_SETTER = re.compile(
    r"_SET_|_SET$|SET_|ENABLE|DISABLE|_BIND|BIND_|UNBIND|SCHEDULE|PREEMPT|RESET|REGISTER|"
    r"ALLOC|_FREE|MAP_|UNMAP|INVALIDATE|FLUSH|CLEAR|WRITE|PROMOTE|EVICT|_STOP|_START|RESTORE|"
    r"SAVE|_INIT|UPDATE|_ADD|REMOVE|RELEASE|ACQUIRE|_LOCK|UNLOCK|INJECT|TRIGGER|NOTIF|ARM_|"
    r"KILL|_RC_|RECOVER|_ACK|PUSHBUFFER|CTRL_CMD_\w*_SEND|MODIFY|RESERVE|IDLE|"
    r"SUSPEND|RESUME|ATTACH|DETACH|_PUT|_SWITCH|_LAUNCH|_ALLOW|_DENY|_CREATE|DESTROY")
RX_GETTER = re.compile(r"_GET_|_GET$|CMD_GET|QUERY|_READ|_IS_|_INFO$|_INFO_V\d|_CAPS|_STATUS$|"
                       r"_DESCRIBE|_LIST$|_SAMPLE")
RX_STATIC_GET = re.compile(r"INFO|CAPS|_GET_(NUM|COUNT|MASK|LIST|CONFIG|SIZE|NAME|ID|VERSION|"
                           r"TYPE|ENGINES|CLASS|TABLE|LIMITS|FEATURES|PROPERTIES|PARTITION|"
                           r"STATIC|BRAND|CHIP|SKU|UUID|ARCH|IP_VERSION)|STATIC|_VERSION|"
                           r"BRAND|UUID|SKU|CLASSLIST|ENGINES|_NAME$|OEM|FALCON_INFO|_IS_\w*(ENABLED|SUPPORTED|PRESENT)|"
                           r"_SUPPORTED|_ENABLED$")
RX_COMPUTE = re.compile(r"IS_MODE_POSSIBLE|VALIDATE|_CHECK|MODE_POSSIBLE|_CALC|_COMPUTE|_TEST|"
                        r"AUXCH|_I2C|EXECUTE|_TRANSACTION|DDC|_CTRL$")
RX_TELEMETRY = re.compile(r"UTIL|SAMPLE|TEMP|THERMAL|COUNTER|CURRENT|CLK|CLOCK|POWER|PERF|ECC|"
                          r"FAN|READING|TIMESTAMP|_TIME|PSTATE|VOLT|USAGE|ACCOUNTING|PIDS|"
                          r"ERROR|VIOLATION|THROTTLE|PERFMON|STATUS|_EVENT|LINK|PEX|PCIE_|"
                          r"_STATE$|_STATE_|MEMORY_INFO|RETIRED|PAGES|ROW_REMAP|GC6|DMON|STATS|HEAP|ILLUM|"
                          r"CALIBRATION|REMAPPED")
RX_WINDOW = re.compile(r"FLUSH|INVALIDATE|_L2_|DOORBELL|WORK_SUBMIT|SCHEDULE|PREEMPT|_BIND|"
                       r"BIND_|PROMOTE|PUSHBUFFER|CHANNEL|NOTIFICATION|_INTR|INTERRUPT|_BAR|"
                       r"BAR1|BAR2|USERD|GPFIFO|RUNLIST|TSG|_VASPACE|PAGE_DIRECTORY|_PDE|"
                       r"EVENT_SET|SET_EVENT|SEMAPHORE|_CTX|CTXSW")
RX_ADDR_FIELD = re.compile(r"(?i)(addr|address|^va$|va$|^pa$|pa$|physaddr|gpuva|virtaddr|"
                           r"^base$|base$|pdb|pde|pte|instptr|inst_ptr|iova|dmaaddr)")

# Caller kind from the name of the function that sends the control (kernel RM, modeset, UVM).
CALLER_RULES = [
    ("on-error", r"Rc|Recovery|Recover|Error|Fault|Xid|Ecc|Timeout|Isr|Intr|Health|Crash|"
                 r"Watchdog|Nonstall|Replay"),
    ("per-frame", r"Flip|Vblank|Scanline|Present|Frame"),
    ("per-submission", r"Schedule|Submit|Kickoff|Doorbell|WorkSubmit|Launch|Push"),
    ("per-allocation", r"memdesc|Memdesc|Memory|Mem[A-Z]|Map|Unmap|Vaspace|VAS|VaSpace|Heap|"
                       r"Pma|Surface|Dma|dma|Fbsr|Vidmem|Sysmem|Page"),
    ("per-object", r"Construct|Destruct|Alloc|Free|Create|Destroy|Bind|Unbind|Register|"
                   r"Unregister|Open|Close|Attach|Detach|Dup|Ctrl|Set[A-Z]|Channel|Tsg|Ctx|"
                   r"Context|Modeset|Head|Dpy|Display|Dp[A-Z]|nvkms|Nvkms|Connector|Hdmi|Disp"),
    ("boot", r"StateInit|StateLoad|StatePreInit|StatePostLoad|StatePreLoad|StateUnload|"
             r"StateDestroy|ConstructEngine|DestructEngine|InitMissing|Init|Load|Build|Post|"
             r"Bootstrap|Probe|Discover|Populate|Setup|Initialize|Cache|Static|Boot|Unload|"
             r"Teardown|Shutdown|Suspend|Resume"),
]

WINDOW_ORDER = ["boot", "reload", "query", "cuda", "vulkan", "display", "frame", "idle", "error",
                "sweep", "setup"]
# The trapped accesses of one 615 RPC (--rpc-traps on traps-weston's trace): 108 us median from
# the QUEUE_HEAD write to the fourth access (the DEBUGINFO read), plus about 30 us for that
# access's own round trip (the median interval between two of the four accesses).
PER_RPC_TRAPS_US = 138


def rel_tree(p, tree):
    return os.path.relpath(p, tree) if p.startswith(tree) else p


def read(p):
    with open(p, encoding="utf-8", errors="replace") as f:
        return f.read()


def load_tsv(p):
    rows, cols = [], None
    for line in read(p).split("\n"):
        if not line or line.startswith("#"):
            continue
        parts = line.split("\t")
        if cols is None:
            cols = parts
            continue
        rows.append(dict(zip(cols, parts)))
    return rows


def find_tree(arg):
    cands = [arg, os.environ.get("CARAXES_OGKM_615")]
    try:
        common = subprocess.run(["git", "-C", ROOT, "rev-parse", "--path-format=absolute",
                                 "--git-common-dir"], capture_output=True, text=True).stdout.strip()
        if common:
            cands.append(os.path.join(common, "..", "..", "Leandro", "vendor",
                                      "open-gpu-kernel-modules"))
    except OSError:
        pass
    cands.append("/mnt/vmstore/caraxes/ogkm-615.71.09")
    for c in cands:
        if c and os.path.isfile(os.path.join(c, "version.mk")) and re.search(
                r"^NVIDIA_VERSION\s*=\s*615\.71\.09\s*$", read(os.path.join(c, "version.mk")), re.M):
            return os.path.abspath(c)
    sys.exit("gsp-transport.py: no 615.71.09 tree (--ogkm, CARAXES_OGKM_615, ../Leandro)")


def load_surface(tree):
    spec = importlib.util.spec_from_file_location("gsp_surface",
                                                  os.path.join(HERE, "gsp-surface.py"))
    gs = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(gs)
    gs.select_tree(tree)
    return gs


# ---------------------------------------------------------------------------------------
# SDK params structs: embedded pointers, handles, addresses.

def strip_comments(t):
    t = re.sub(r"/\*.*?\*/", " ", t, flags=re.S)
    return re.sub(r"//[^\n]*", " ", t)


def parse_structs(tree):
    """typedef struct/union name -> (body text, "path:line")."""
    sdk = os.path.join(tree, "src", "common", "sdk", "nvidia", "inc")
    out = {}
    for p in glob.glob(os.path.join(sdk, "**", "*.h"), recursive=True):
        raw = read(p)
        t = strip_comments(raw)
        for m in re.finditer(r"typedef\s+(?:struct|union)\s*(\w*)\s*\{", t):
            i, depth = m.end() - 1, 0
            for j in range(i, len(t)):
                if t[j] == "{":
                    depth += 1
                elif t[j] == "}":
                    depth -= 1
                    if depth == 0:
                        break
            body = t[i + 1:j]
            tail = re.match(r"\s*(\w+)\s*;", t[j + 1:j + 200])
            names = {m.group(1)} | ({tail.group(1)} if tail else set())
            # the line number in the raw text: search the typedef's tag or name there
            key = tail.group(1) if tail else m.group(1)
            ln = 0
            mm = re.search(r"\}\s*%s\s*;" % re.escape(key), raw) if key else None
            if mm:
                ln = raw.count("\n", 0, mm.start()) + 1
            for n in names:
                if n and n not in out:
                    out[n] = (body, "%s:%d" % (rel_tree(p, tree), ln))
    return out


def fields_of(name, structs, prefix="", depth=0, seen=None):
    """(pointers, handles, addresses) field paths of the struct `name`, nested three deep."""
    seen = seen or set()
    ptr, hnd, adr = [], [], []
    if name not in structs or depth > 3 or name in seen:
        return ptr, hnd, adr
    seen = seen | {name}
    body = structs[name][0]
    body = re.sub(r"NV_DECLARE_ALIGNED\(\s*([^,]+?)\s*,\s*\d+\s*\)", r"\1", body)
    for decl in body.split(";"):
        d = " ".join(decl.split())
        if not d or d.startswith("#"):
            continue
        m = re.match(r"^(?:const\s+|volatile\s+)*(\w+)\s*(\**)\s*(\w+)", d)
        if not m:
            continue
        ty, star, fname = m.group(1), m.group(2), m.group(3)
        path = prefix + fname
        if ty in ("NvP64",) or star or ty.endswith("_PTR") or re.match(r"P\w+", ty) and ty.isupper() and ty.startswith("PNV"):
            ptr.append(path)
        elif ty in ("NvHandle",):
            hnd.append(path)
        elif ty in ("NvU64", "RmPhysAddr", "NvUPtr", "NvU32") and RX_ADDR_FIELD.search(fname):
            adr.append(path)
        elif ty in structs and ty not in ("NvU8", "NvU16", "NvU32", "NvU64"):
            p2, h2, a2 = fields_of(ty, structs, path + ".", depth + 1, seen)
            ptr += p2
            hnd += h2
            adr += a2
    return ptr, hnd, adr


# ---------------------------------------------------------------------------------------
# Observations: profiles (610 guest) and device logs (615 guest).

def load_dump(p):
    head, rows = "", {}
    with lzma.open(p, "rt") as f:
        for line in f:
            if line.startswith("#"):
                head += line
                continue
            if line.startswith("scope"):
                continue
            c = line.rstrip("\n").split("\t")
            if c[0] != "device" or c[3] not in ("control", "rpc"):
                continue
            rows[(c[3], int(c[4], 16) if c[3] == "control" else c[4])] = (
                int(c[6]), int(c[7]), c[13])
    m = re.search(r"device time (\d+) us", head)
    return (int(m.group(1)) if m else 0), rows


class Obs:
    def __init__(self):
        self.count = collections.defaultdict(collections.Counter)    # id -> window -> n
        self.ns = collections.defaultdict(int)                        # id -> device ns (610)
        self.nsw = collections.defaultdict(collections.Counter)       # id -> window -> ns
        self.n610 = collections.Counter()
        self.us615 = collections.defaultdict(list)                    # id -> rx->tx us
        self.label = collections.defaultdict(collections.Counter)
        self.windows = {}                                             # window -> (kind, secs)
        self.rpc = collections.defaultdict(collections.Counter)       # rpc name -> window -> n
        self.rpc_ns = collections.defaultdict(int)
        self.sources = []

    def add(self, win, kind, secs):
        self.windows[win] = (kind, secs)


def load_profiles(obs):
    for run in PROFILE_RUNS:
        base = os.path.join(RUNS, run)
        marks = [l.split("\t") for l in read(os.path.join(base, "marks.tsv")).split("\n") if l]
        prev_t, prev = 0, {}
        for m in marks:
            n, name = m[0], m[1]
            p = os.path.join(base, "profile", "dev.%s.tsv.xz" % n)
            t, rows = load_dump(p)
            kind = "cuda" if name in CUDA_WINDOWS else WINDOW_KIND.get(name, "other")
            win = "%s:%s" % (run.split("/")[-1], name)
            obs.add(win, kind, (t - prev_t) / 1e6)
            for k, (cnt, ns, lab) in rows.items():
                pc, pns, _ = prev.get(k, (0, 0, ""))
                if cnt - pc <= 0:
                    continue
                if k[0] == "control":
                    obs.count[k[1]][win] += cnt - pc
                    obs.nsw[k[1]][win] += ns - pns
                    if kind != "sweep":
                        obs.ns[k[1]] += ns - pns
                        obs.n610[k[1]] += cnt - pc
                    obs.label[k[1]][lab] += cnt - pc
                else:
                    obs.rpc[k[1]][win] += cnt - pc
                    if kind != "sweep":
                        obs.rpc_ns[k[1]] += ns - pns
            prev_t, prev = t, rows
        obs.sources.append("caraxes/runs/%s/profile (marks.tsv)" % run)


RX_RX = re.compile(r"^\s*(\d+) E boot: rx seq \d+ (\w+)")
RX_TX = re.compile(r"^\s*(\d+) E boot: tx seq \d+ reply (\w+)")


def load_events(obs):
    for rel, kind in EVENT_LOGS:
        p = os.path.join(RUNS, rel)
        if not os.path.exists(p):
            continue
        frame = None
        tt = os.path.join(os.path.dirname(p), "trap-table.txt")
        if os.path.exists(tt):
            m = re.search(r"window [\d.]+ s \[(\d+), (\d+)\) us", read(tt))
            if m:
                frame = (int(m.group(1)), int(m.group(2)))
        op = lzma.open if p.endswith(".xz") else open
        ts, pending = [], None
        win = rel.split("/")[0].split("-2026")[0] + ":" + rel.split("/")[-2]
        fwin = win + ":frame-window"
        with op(p, "rt", errors="replace") as f:
            for line in f:
                if " E boot: " not in line:
                    continue
                m = RX_RX.match(line)
                if m:
                    t, fn = int(m.group(1)), m.group(2)
                    key = None
                    if fn == "GSP_RM_CONTROL":
                        mc = re.search(r"cmd=(0x[0-9a-fA-F]+)", line)
                        if mc:
                            key = int(mc.group(1), 16)
                    w = fwin if frame and frame[0] <= t < frame[1] else win
                    if key is not None:
                        obs.count[key][w] += 1
                    else:
                        obs.rpc[fn][w] += 1
                    ts.append(t)
                    pending = (t, key)
                    continue
                m = RX_TX.match(line)
                if m and pending and pending[1] is not None:
                    obs.us615[pending[1]].append(int(m.group(1)) - pending[0])
                    pending = None
        if ts:
            span = (ts[-1] - ts[0]) / 1e6
            if frame:
                obs.add(fwin, "frame", (frame[1] - frame[0]) / 1e6)
            obs.add(win, kind, span)
        obs.sources.append("caraxes/runs/%s" % rel)


# ---------------------------------------------------------------------------------------
# Classification.

def caller_from_sites(sites, lines_cache, gs):
    """Kinds by the enclosing function of each call site (tree paths with line)."""
    kinds, where = [], []
    for s in sites:
        m = re.match(r"(.+?):(\d+)", s)
        if not m:
            continue
        p, ln = m.group(1), int(m.group(2))
        if p not in lines_cache:
            try:
                lines_cache[p] = read(p).split("\n")
            except OSError:
                lines_cache[p] = []
        fn = gs.enclosing_function(lines_cache[p], ln) or ""
        kind = None
        for k, rx in CALLER_RULES:
            if re.search(rx, fn):
                kind = k
                break
        kinds.append(kind or "boot")
        where.append((p, ln, fn))
    return kinds, where


def observed_kind(win_counts, obs):
    kinds = collections.Counter()
    for w, n in win_counts.items():
        kinds[obs.windows.get(w, ("other", 0))[0]] += n
    real = {k for k in kinds if k not in ("sweep", "setup", "other")}
    if not real:
        return None, kinds
    if real == {"boot"} or real <= {"boot", "reload"}:
        return "boot", kinds
    if "frame" in real:
        return "periodic", kinds
    if real <= {"query", "boot", "reload"} and "query" in real:
        return "per-query", kinds
    if real <= {"error"}:
        return "on-error", kinds
    if real <= {"idle", "boot", "reload"} and "idle" in real:
        return "periodic", kinds
    return "per-object", kinds


def gss_name(cid, cats):
    cls, cat, idx = cid >> 16, (cid >> 8) & 0xFF, cid & 0xFF
    if cls == 0x2081:
        return "BINAPI 0x%08x (class 0x2081, outside the export tables)" % cid
    nm = cats.get(cat & 0x3F, "0x%02x" % (cat & 0x3F))
    return "GSS-legacy %s 0x%02x (0x%08x, outside the export tables)" % (nm, idx, cid)


def classify(r, ctx):
    """One output row for the routed control r (a controls.tsv dict, or a synthetic one)."""
    name = r["name"]
    flags = int(r.get("flags") or "0", 16) if r.get("flags", "-") not in ("", "-") else 0
    prov = r["provenance"]
    cat = r.get("category", "-")
    ab = r.get("answered_by", "-")
    cid = int(r["id"], 16)
    size = int(r["size"]) if str(r.get("size", "")).isdigit() else 0
    d = ctx["d"].get(cid, {})
    b = ctx["b"].get(cid, {})
    obs = ctx["obs"]

    # --- embedded pointers, handles, addresses
    ptr, hnd, adr = fields_of(r.get("params", ""), ctx["structs"])
    emb = []
    if ptr:
        emb.append("ptr:" + ",".join(ptr[:4]) + ("+%d" % (len(ptr) - 4) if len(ptr) > 4 else ""))
    if hnd:
        emb.append("handle:" + ",".join(hnd[:4]) + ("+%d" % (len(hnd) - 4) if len(hnd) > 4 else ""))
    if adr:
        emb.append("addr:" + ",".join(adr[:4]) + ("+%d" % (len(adr) - 4) if len(adr) > 4 else ""))
    emb_s = "yes " + ";".join(emb) if emb else "no"

    # --- observations
    wc = obs.count.get(cid, collections.Counter())
    per_boot = max([n for w, n in wc.items() if obs.windows.get(w, ("", 0))[0] == "boot"] or [0])
    rate = 0.0
    for w, n in wc.items():
        kind, secs = obs.windows.get(w, ("other", 0))
        if kind in ("sweep", "setup") or secs < 1.0:
            continue
        rate = max(rate, n / secs)
    total = sum(n for w, n in wc.items() if obs.windows.get(w, ("", 0))[0] != "sweep")
    swept = sum(n for w, n in wc.items() if obs.windows.get(w, ("", 0))[0] == "sweep")
    lat = ""
    if obs.n610.get(cid):
        lat = "%.0f" % (obs.ns[cid] / obs.n610[cid] / 1000.0)
    elif obs.us615.get(cid):
        lat = "%.0f" % statistics.median(obs.us615[cid])
    lat615 = "%.0f" % statistics.median(obs.us615[cid]) if obs.us615.get(cid) else ""
    label = obs.label[cid].most_common(1)[0][0] if obs.label.get(cid) else ""
    frame_n = sum(n for w, n in wc.items() if obs.windows.get(w, ("", 0))[0] == "frame")

    # --- caller kind
    okind, kinds = observed_kind(wc, obs)
    sites = ctx["explicit"].get(name, []) + ctx["dynamic"].get(r.get("func", ""), [])
    skinds, swhere = caller_from_sites([s.split(" (")[0] for s in sites], ctx["lines"], ctx["gs"])
    refs = ctx["refs"].get(name, collections.Counter())
    ref_where = ctx["ref_where"].get(name, [])
    if prov == "refused" and ab.startswith("ctrl:E family") and not total:
        kind, basis = "never-tu106", "E family absent on TU106 (%s)" % ab.split("family ")[1].split(" ")[0]
    elif okind:
        kind = okind
        basis = "observed: " + ",".join("%s %d" % (k, n) for k, n in sorted(
            kinds.items(), key=lambda kv: WINDOW_ORDER.index(kv[0]) if kv[0] in WINDOW_ORDER else 99))
    elif skinds:
        kind = collections.Counter(skinds).most_common(1)[0][0]
        p, ln, fn = swhere[0]
        basis = "sent by %s (%s:%d)" % (fn or "?", rel_tree(p, ctx["tree"]), ln)
    elif refs.get("display"):
        kind = "per-object"
        basis = "nvidia-modeset/displayport (%s)" % rel_tree(ref_where[0], ctx["tree"]) if ref_where else "display"
    elif refs.get("uvm") or any("nv_gpu_ops.c" in w for w in ref_where):
        kind = "per-object"
        w0 = [w for w in ref_where if "uvm" in w or "nv_gpu_ops.c" in w]
        basis = "UVM (%s)" % rel_tree((w0 or ref_where)[0], ctx["tree"])
    elif r.get("privilege") in ("internal", "kernel") and not refs.get("kernel-rm"):
        kind, basis = "never-sent", "internal, no call site in the 615 tree"
    elif d.get("caller"):
        kind = "per-query" if RX_GETTER.search(name) else "per-object"
        basis = "user space (d-controls.tsv caller: %s)" % d["caller"][:60]
    else:
        kind = "per-query" if RX_GETTER.search(name) and not re.search(r"_SET_|_SET$", name) else "per-object"
        basis = "user space, no kernel caller (name)"
    if cat == "E" and kind in ("per-object", "per-query") and not total and prov in ("refused", "unhandled"):
        kind = "never-tu106"

    # --- answer source class
    lab = label.lower()
    if prov == "refused":
        src = "refused"
    elif prov == "unhandled":
        src = "unhandled"
    elif prov == "host-forwarded" or lab.startswith("host"):
        src = "host-forwarded"
    elif lab.startswith("card") or lab.startswith("chip"):
        src = "static per card"
    elif lab.startswith("profile"):
        src = "static per VM"
    elif lab.startswith("guest") or prov == "guest-derived":
        src = "dynamic per VM state"
    elif prov == "echo":
        src = "static per VM"
    elif ab == "display":
        # the display model's answers: its configuration (caps, heads) or its live state
        src = "static per VM" if RX_STATIC_GET.search(name) else "dynamic per VM state"
    elif ab.startswith("ctrl:D table"):
        src = "static per VM"
    else:
        src = "static per card"

    # --- side effects: a getter by name reads, a setter by name changes state; the rest
    # (AUX transactions, mode checks, execute lists) compute an answer from the request
    getter = bool(RX_GETTER.search(name)) and not re.search(r"_SET_|_SET$", name)
    setter = not getter and bool(RX_SETTER.search(name))
    if prov == "refused" or (prov == "echo" and src != "host-forwarded") or not setter:
        side = "none"
    elif src == "host-forwarded":
        side = "host object"
    elif prov == "unhandled":
        side = "device state (planned)"
    else:
        side = "device state"

    # --- guest cache (the stock driver's own answer page)
    acc = ctx["access"].get(cid, 0)
    gcache = "-"
    if flags & (F_CACHEABLE | F_CACHEABLE_BY_INPUT) and acc == 0:
        gcache = "rmapi-cache" if flags & F_CACHEABLE else "rmapi-cache by input"
    if r.get("gss"):
        gcache = "possible (reply rmctrlFlags CACHEABLE; today 0)"

    # --- verdict
    host_cache = "-"
    risk = d.get("risk", "")
    danswer = d.get("answer", "")
    tele = bool(RX_TELEMETRY.search(name)) or bool(r.get("gss"))
    static_get = bool(RX_STATIC_GET.search(name)) and not RX_COMPUTE.search(name)
    window = bool(RX_WINDOW.search(name))
    if prov == "refused":
        iso = danswer.startswith("DENY") or "isolation" in lab or ab.startswith("public:") \
            or (risk.startswith("high") and not ab.startswith("ctrl:E"))
        verdict, reason = "REFUSE", "ISOLATION" if iso else "ABSENT"
    elif prov == "unhandled":
        if cat == "E":
            verdict, reason = "REFUSE", "PLANNED-ABSENT"
        elif cat == "A":
            verdict, reason = "ZT-PAGE", "PLANNED-STATIC-CARD"
        elif cat == "B":
            verdict, reason = "HOST", "PLANNED-HOST"
            host_cache = "yes (static per card)" if static_get and not tele else "ttl" if tele else "no"
        elif setter and (hnd or adr or window):
            verdict, reason = "TRAP", "PLANNED-" + ("ADDR-VALIDATE" if adr else "HANDLE-STATE")
        else:
            verdict, reason = "ZT-QUEUE", "PLANNED-DEVICE"
    elif src == "host-forwarded":
        if side == "host object":
            verdict, reason = "TRAP", "HOST-STATE"
            host_cache = "no"
        else:
            verdict = "HOST"
            if r.get("gss") and not static_get:
                reason, host_cache = "HOST-GSS-LEGACY", "ttl (or guest cache for static tables)"
            elif static_get and not tele:
                reason, host_cache = "HOST-STATIC", "yes (static per card)"
            elif tele:
                reason, host_cache = "HOST-TELEMETRY", "ttl"
            elif hnd:
                reason, host_cache = "HOST-PER-HANDLE", "no"
            else:
                reason, host_cache = "HOST-STATIC", "yes (static per card)"
    elif side != "none" and adr:
        verdict, reason = "TRAP", "ADDR-VALIDATE"
    elif side != "none" and (hnd or window):
        verdict, reason = "TRAP", "HANDLE-STATE" if hnd else "WINDOW-ORDER"
    elif side == "none" and prov == "echo":
        verdict, reason = "ZT-QUEUE", "ECHO-ACK"
    elif src in ("static per card", "static per VM") and getter and not RX_COMPUTE.search(name):
        verdict, reason = "ZT-PAGE", "STATIC-CARD" if src == "static per card" else "STATIC-VM"
    elif side != "none":
        verdict, reason = "ZT-QUEUE", "DEVICE-STATE-SET"
    elif RX_COMPUTE.search(name) or not getter:
        verdict, reason = "ZT-QUEUE", "DEVICE-COMPUTE"
    else:
        verdict, reason = "ZT-QUEUE", "DEVICE-STATE-GET" if src == "dynamic per VM state" else "DEVICE-STATIC-GET"

    # --- isolation: what an unvalidated zero-trap version could do, and the mitigation
    iso = isolation(verdict, reason, src, side, hnd, adr, risk, ab)

    # request and reply carry the same params (boot.rs resizes the answer to paramsSize);
    # on the queue: MCTP 24 + rpc_message_header_v 32 + rpc_gsp_rm_control_v03_00 40 bytes
    known_size = size > 0 or r.get("params", "-") not in ("", "-")
    slots = str(math.ceil((24 + 32 + 40 + size) / 4096.0)) if known_size else "-"
    size_s = str(size) if known_size else "-"
    return collections.OrderedDict([
        ("id", "0x%08X" % cid), ("name", name), ("class", r.get("nvoc_class", "-")),
        ("privilege", r.get("privilege", "-")), ("caller_kind", kind), ("caller_basis", basis),
        ("per_boot", str(per_boot)), ("max_per_s", "%.1f" % rate), ("seen_total", str(total)),
        ("swept", str(swept)), ("per_frame_window", str(frame_n)),
        ("device_us", lat), ("device_us_615", lat615),
        ("req_bytes", size_s), ("reply_bytes", size_s), ("queue_slots", slots),
        ("embedded", emb_s), ("answer_source", src), ("side_effects", side),
        ("guest_cache", gcache), ("verdict", verdict), ("reason", reason),
        ("host_cache", host_cache), ("isolation", iso),
        ("provenance", prov), ("answered_by", ab), ("category", cat),
        ("label", label.replace("\t", " ")[:120]),
    ])


def isolation(verdict, reason, src, side, hnd, adr, risk, ab):
    short_risk = risk.split(";")[0][:110] if risk else ""
    if verdict == "REFUSE":
        if reason.endswith("ISOLATION"):
            return "threat: %s | mitigation: refused, never forwarded" % (
                short_risk or "host-wide state or hardware of the shared card")
        return "threat: none while absent | mitigation: refusal is static; stays a named NOT_SUPPORTED"
    if verdict == "ZT-PAGE":
        return ("threat: read host state if the page held unfiltered host data | mitigation: "
                "read-only page filled from the VM's labelled answers (D9, costume/profile), "
                "version word, no host telemetry")
    if verdict == "ZT-QUEUE":
        t = "corrupt only this VM's device model"
        if hnd:
            t += "; reach another client's object by handle"
        return ("threat: %s | mitigation: element header and handle namespace validated before "
                "use (msgq/v2.rs checks), bounded work per reactor loop" % t)
    if verdict == "HOST":
        if reason in ("HOST-TELEMETRY", "HOST-GSS-LEGACY"):
            return ("threat: read host-wide telemetry (desktop and other VMs' activity); host RM "
                    "lock contention | mitigation: public-control filter, TTL cache, rate limit "
                    "per VM")
        return ("threat: read host card facts beyond the costume | mitigation: public-control "
                "filter (public.rs), answer cached once and rewritten to the profile (D6)")
    # TRAP
    if reason.endswith("ADDR-VALIDATE"):
        return ("threat: DMA or mappings outside this VM's RAM and reservation | mitigation: "
                "device-side address validation before use (D17/D18 filter), in order after "
                "the memfd sync" + ("; " + short_risk if short_risk else ""))
    if reason == "HOST-STATE":
        return ("threat: reconfigure or reset a host engine, reach the desktop's or another "
                "VM's host objects | mitigation: handle resolved to this VM's mirror object "
                "before the host call (D10/D12), never forwarded raw" + (
                    "; " + short_risk if short_risk else ""))
    return ("threat: act on another client's object or on a window the device keeps "
            "(doorbell tokens, pool, interrupt page) | mitigation: handle and window state "
            "validated synchronously before the reply" + ("; " + short_risk if short_risk else ""))


# ---------------------------------------------------------------------------------------

def load_b():
    out = {}
    p = os.path.join(PLAN, "b-host-forwardable.toml")
    cur = None
    for line in read(p).split("\n"):
        if line.startswith("[[control]]"):
            cur = {}
            continue
        m = re.match(r'(\w+)\s*=\s*"?([^"]*)"?\s*$', line)
        if m and cur is not None:
            cur[m.group(1)] = m.group(2)
            if m.group(1) == "id":
                out[int(m.group(2), 16)] = cur
    return out


def build(tree):
    gs = load_surface(tree)
    by_name, by_val, _ = gs.parse_sdk_defines()
    refs, ref_where, explicit, dynamic = gs.parse_call_sites(by_name)
    exports = gs.parse_export_tables(by_val)
    access = {e["id"]: e["access"] for e in exports}
    func = {e["id"]: e["func"] for e in exports}
    structs = parse_structs(tree)
    cats = {}
    base = os.path.join(tree, "src", "common", "sdk", "nvidia", "inc", "ctrl", "ctrl2080",
                        "ctrl2080base.h")
    for m in re.finditer(r"#define NV2080_CTRL_(\w+)\s+\((0x[0-9a-fA-F]+)\)", read(base)):
        v = int(m.group(2), 16)
        if v < 0x40 and not m.group(1).endswith("PRIVILEGED"):
            cats.setdefault(v, m.group(1))
    d = {int(r["id"], 16): r for r in load_tsv(os.path.join(PLAN, "d-controls.tsv"))}
    obs = Obs()
    load_profiles(obs)
    load_events(obs)
    ctx = {"gs": gs, "tree": tree, "refs": refs, "ref_where": ref_where, "explicit": explicit,
           "dynamic": dynamic, "access": access, "structs": structs, "d": d, "b": load_b(),
           "obs": obs, "lines": {}}
    rows = []
    ctl = load_tsv(os.path.join(ABI, "controls.tsv"))
    known = {int(r["id"], 16) for r in ctl}
    for r in ctl:
        if r["route"] != "gsp":
            continue
        r = dict(r)
        r["func"] = func.get(int(r["id"], 16), "")
        rows.append(classify(r, ctx))
    # observed ids outside the export tables (GSS legacy, BINAPI): forwarded by the device
    for cid in sorted(obs.count):
        if cid in known:
            continue
        lab = obs.label[cid].most_common(1)[0][0] if obs.label.get(cid) else ""
        prov = "host-forwarded" if lab.startswith("host") else "refused" if "NOT_SUPPORTED" in lab \
            else "synthesized"
        r = {"id": "0x%08X" % cid, "name": gss_name(cid, cats), "nvoc_class": "Subdevice"
             if cid >> 16 == 0x2080 else "-", "privilege": "non-priv", "flags": "-",
             "provenance": prov, "category": "B", "answered_by": "ver LEGACY/BINAPI tables",
             "size": "", "params": "", "gss": True}
        rows.append(classify(r, ctx))
    return rows, obs


def write_tsv(rows, obs, out, tree):
    cols = list(rows[0].keys())
    with open(out, "w", encoding="utf-8") as f:
        f.write("# SPDX-License-Identifier: MIT\n")
        f.write("# transport.tsv of open-gpu-kernel-modules 615.71.09: one row per GSP-routed "
                "control (controls.tsv route gsp) plus the GSS-legacy and BINAPI ids the "
                "profiles saw; the transport verdict of docs/caraxes/gsp-transport-classes.md.\n")
        f.write("# Generated by caraxes/tools/gsp-transport.py; do not edit, rerun it. Observed "
                "columns: the 610-guest profiles and the 615-guest device logs:\n")
        for s in obs.sources:
            f.write("#   %s\n" % s)
        f.write("\t".join(cols) + "\n")
        for r in rows:
            f.write("\t".join(str(r[c]).replace("\t", " ") for c in cols) + "\n")


def summary(rows, obs):
    out = []
    v = collections.Counter(r["verdict"] for r in rows)
    out.append("## Counts per verdict\n")
    out.append("| Verdict | Rows | Observed in a profile or log | Reason codes |")
    out.append("|---|---:|---:|---|")
    for k in ["ZT-PAGE", "ZT-QUEUE", "HOST", "TRAP", "REFUSE"]:
        rs = [r for r in rows if r["verdict"] == k]
        seen = sum(1 for r in rs if int(r["seen_total"]))
        rc = collections.Counter(r["reason"] for r in rs)
        out.append("| %s | %d | %d | %s |" % (k, len(rs), seen, ", ".join(
            "%s %d" % kv for kv in rc.most_common())))
    out.append("| all | %d | %d | |\n" % (len(rows), sum(1 for r in rows if int(r["seen_total"]))))
    out.append("## Caller kinds\n")
    ck = collections.Counter((r["caller_kind"], r["verdict"]) for r in rows)
    kinds = sorted({k for k, _ in ck})
    out.append("| Caller kind | " + " | ".join(["ZT-PAGE", "ZT-QUEUE", "HOST", "TRAP", "REFUSE"]) + " |")
    out.append("|---|" + "---:|" * 5)
    for k in kinds:
        out.append("| %s | %s |" % (k, " | ".join(str(ck[(k, v2)]) for v2 in
                                                ["ZT-PAGE", "ZT-QUEUE", "HOST", "TRAP", "REFUSE"])))
    out.append("")
    for k in ["ZT-PAGE", "ZT-QUEUE", "HOST", "TRAP"]:
        rs = [r for r in rows if r["verdict"] == k and int(r["seen_total"])]
        for r in rs:
            dev = float(r["device_us"] or 0)
            r["_cost_ms"] = int(r["seen_total"]) * (dev + PER_RPC_TRAPS_US) / 1000.0
        rs.sort(key=lambda r: -r["_cost_ms"])
        out.append("## Top of %s by count x (device time + %d us per-RPC traps)\n" % (k, PER_RPC_TRAPS_US))
        out.append("| Id | Name | Caller | Seen | Per boot | Max/s | Device us | Cost ms | Reason | Host cache | Guest cache |")
        out.append("|---|---|---|---:|---:|---:|---:|---:|---|---|---|")
        for r in rs[:12]:
            out.append("| %s | %s | %s | %s | %s | %s | %s | %.1f | %s | %s | %s |" % (
                r["id"], r["name"][:52], r["caller_kind"], r["seen_total"], r["per_boot"],
                r["max_per_s"], r["device_us"], r["_cost_ms"], r["reason"], r["host_cache"],
                r["guest_cache"]))
        out.append("")
    out.append("## Windows: RPCs and controls per verdict (count / device ms; ms only in the profiles)\n")
    out.append("| Window | Kind | Seconds | RPCs | Controls | ZT-PAGE | ZT-QUEUE | HOST | TRAP | REFUSE |")
    out.append("|---|---|---:|---:|---:|---|---|---|---|---|")
    byid = {int(r["id"], 16): r["verdict"] for r in rows}
    for w, (kind, secs) in obs.windows.items():
        if kind == "setup":
            continue
        agg = collections.defaultdict(lambda: [0, 0.0])
        for cid, c in obs.count.items():
            if c.get(w):
                v = byid.get(cid, "?")
                agg[v][0] += c[w]
                agg[v][1] += obs.nsw.get(cid, {}).get(w, 0) / 1e6
        nctl = sum(a[0] for a in agg.values())
        nrpc = nctl + sum(c[w] for k, c in obs.rpc.items() if k not in ("0x4c", "GSP_RM_CONTROL"))
        prof = w.split(":")[0] in ("map", "extra")
        cell = (lambda v: "%d / %.0f" % (agg[v][0], agg[v][1])) if prof else (lambda v: "%d" % agg[v][0])
        out.append("| %s | %s | %.1f | %d | %d | %s |" % (w, kind, secs, nrpc, nctl, " | ".join(
            cell(v) for v in ["ZT-PAGE", "ZT-QUEUE", "HOST", "TRAP", "REFUSE"])))
    return "\n".join(out)


def rpc_traps(path):
    """Per RPC of a device trace: the trapped accesses on page 0x110000.

    An RPC starts at the trapped QUEUE_HEAD write, or, with CARAXES_RPC_IOEVENTFD=1, at the
    device's "rpc ioeventfd: head write" event that served an element (the guest's write
    itself no longer reaches the trace; the event is when the device took it)."""
    rows, cur, regs = [], None, collections.Counter()
    op = lzma.open if path.endswith(".xz") else open
    with op(path, "rt", errors="replace") as f:
        for line in f:
            m = re.match(r"^\s*(\d+) (\w) (.*)$", line)
            if not m:
                continue
            t, k, rest = int(m.group(1)), m.group(2), m.group(3)
            if k in "RW" and rest.startswith("bar0 0x00110"):
                regs[(k, rest.split()[1])] += 1
                if cur is not None and not rest.startswith("bar0 0x00110c00"):
                    cur["traps"] = cur.get("traps", 0) + 1
            fd_head = k == "E" and "rpc ioeventfd: head write" in rest and (
                "QUEUE_HEAD" in rest or "610 doorbell" in rest)
            if (k == "W" and rest.startswith("bar0 0x00110c00")) or fd_head:
                if cur:
                    rows.append(cur)
                cur = {"head": t, "traps": 1 if k == "W" else 0, "fd": fd_head}
            elif cur is None:
                continue
            elif k == "E" and " rx seq " in rest and "rx" not in cur:
                cur["rx"] = t
            elif k == "E" and " tx seq " in rest and "reply" in rest:
                cur["tx"] = t
            elif k == "R" and rest.startswith("bar0 0x00110c80") and "mh" not in cur:
                cur["mh"] = t
            elif k == "W" and rest.startswith("bar0 0x00110c84") and "mt" not in cur:
                cur["mt"] = t
            elif k == "R" and rest.startswith("bar0 0x00110094") and "dbg" not in cur:
                cur["dbg"] = t
    if cur:
        rows.append(cur)
    fd = sum(1 for r in rows if r.get("fd"))
    print("%s: %d RPCs (%d started by the rpc ioeventfd, %d by a trapped QUEUE_HEAD write)" % (
        path, len(rows), fd, len(rows) - fd))
    if rows:
        t = sorted(r["traps"] for r in rows)
        print("trapped accesses on page 0x110000 per RPC: mean %.2f, median %d, p90 %d" % (
            sum(t) / len(t), t[len(t) // 2], t[len(t) * 9 // 10]))
    print("accesses on page 0x110000: " + ", ".join(
        "%s %s %d" % (k, r, n) for (k, r), n in regs.most_common()))
    for name, a, b in [("QUEUE_HEAD write -> rx", "head", "rx"),
                       ("rx -> tx (device work)", "rx", "tx"),
                       ("tx -> MSGQ_HEAD read", "tx", "mh"),
                       ("MSGQ_HEAD read -> MSGQ_TAIL write", "mh", "mt"),
                       ("MSGQ_TAIL write -> DEBUGINFO read", "mt", "dbg"),
                       ("QUEUE_HEAD write -> DEBUGINFO read", "head", "dbg")]:
        v = sorted(r[b] - r[a] for r in rows if a in r and b in r)
        if v:
            print("  %-36s median %4d us  p10 %4d  p90 %4d  (n %d)" % (
                name, statistics.median(v), v[len(v) // 10], v[len(v) * 9 // 10], len(v)))


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--ogkm", help="the 615.71.09 open-gpu-kernel-modules tree")
    ap.add_argument("--out", default=os.path.join(ABI, "transport.tsv"))
    ap.add_argument("--summary", action="store_true", help="Markdown tables on stdout")
    ap.add_argument("--rpc-traps", metavar="TRACE", help="per-RPC trap timing of a 615 device "
                    "trace (device-trace.txt.xz), then exit")
    a = ap.parse_args()
    if a.rpc_traps:
        rpc_traps(a.rpc_traps)
        return
    tree = find_tree(a.ogkm)
    rows, obs = build(tree)
    write_tsv(rows, obs, a.out, tree)
    if a.summary:
        print(summary(rows, obs))
    else:
        print("%s: %d rows" % (os.path.relpath(a.out, ROOT), len(rows)))


if __name__ == "__main__":
    main()
