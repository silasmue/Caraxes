# SPDX-License-Identifier: MIT
"""Pure, deliberately narrow parser for Nsight GPU-metrics help and nvidia-smi CSV.

No command execution, freshness claim, ownership claim, or attestation is provided.
"""
import csv
import io
import re
from dataclasses import dataclass

MAX_BYTES = 16384
MAX_DEVICES = 64
HEADER = 'Possible --gpu-metrics-devices values are:'
UNSUPPORTED = 'Some GPUs are not supported:'
ALIASES = {
    'all: Select all supported GPUs',
    'cuda-visible: Select GPUs that match CUDA_VISIBLE_DEVICES',
    'none: Disable GPU Metrics [Default]',
}
INDENT = re.compile(r'(?:    |\t)')
FOOTER = 'See the user guide: https://docs.nvidia.com/nsight-systems/UserGuide/index.html#gpu-metrics'
DEVICE = re.compile(r'(0|[1-9][0-9]*): ([^\r\n\[\]]{1,160}) PCI\[([0-9a-fA-F]{4}(?:[0-9a-fA-F]{4})?:[0-9a-fA-F]{2}:[0-9a-fA-F]{2}\.[0-7])\]')
UUID = re.compile(r'GPU-[0-9a-fA-F]{8}(?:-[0-9a-fA-F]{4}){3}-[0-9a-fA-F]{12}')
BDF = re.compile(r'([0-9a-fA-F]{4}|0000[0-9a-fA-F]{4}):([0-9a-fA-F]{2}):([0-9a-fA-F]{2})\.([0-7])')

@dataclass(frozen=True)
class Device:
    index: int
    pci_bdf: str
    uuid: str
    supported: bool
    label: str

def _bounded(value, *, help_text=False):
    if not isinstance(value, str):
        raise ValueError('invalid or oversized input')
    try:
        size = len(value.encode('utf-8', errors='strict'))
    except UnicodeError as exc:
        raise ValueError('invalid UTF-8 text') from exc
    # The documented CLI example and requested CSV grammar are printable ASCII,
    # with LF or CRLF line endings only. No terminal escapes or hidden controls.
    if size > MAX_BYTES or any(c not in '\r\n' and not 32 <= ord(c) <= 126 and not (help_text and c == '\t') for c in value):
        raise ValueError('invalid or oversized input')
    if help_text and any('\t' in line and (not line.startswith('\t') or line.count('\t') != 1)
                         for line in value.split('\n')):
        raise ValueError('tab outside one leading help indent')
    return value

def pci(value):
    if not isinstance(value, str):
        raise ValueError('invalid PCI BDF')
    match = BDF.fullmatch(value)
    if not match:
        raise ValueError('invalid PCI BDF')
    domain, bus, device, function = match.groups()
    if int(device, 16) > 0x1f:
        raise ValueError('PCI device number outside 0x00..0x1f')
    return f'{domain[-4:].lower()}:{bus.lower()}:{device.lower()}.{function}'

def _lines(value, *, help_text=False):
    value = _bounded(value, help_text=help_text)
    if '\r' in value.replace('\r\n', ''):
        raise ValueError('malformed line ending')
    return value.replace('\r\n', '\n').removesuffix('\n').split('\n')

def parse_help(value):
    lines = _lines(value, help_text=True)
    if not lines or lines[0] != HEADER:
        raise ValueError('missing exact Nsight header')
    state = 'supported'
    rows = []
    seen_indices, seen_bdfs, aliases = set(), set(), set()
    unsupported_seen = footer_seen = False
    for line in lines[1:]:
        raw_line = line
        if line == UNSUPPORTED and state == 'supported' and not unsupported_seen:
            state = 'unsupported'; unsupported_seen = True; continue
        if line == FOOTER and not footer_seen:
            state = 'footer'; footer_seen = True; continue
        if state == 'footer':
            raise ValueError('trailing output')
        if INDENT.match(line):
            line = line[4:] if line.startswith('    ') else line[1:]
        if '\t' in line:
            raise ValueError('tab outside one leading help indent')
        if INDENT.match(raw_line) and line in ALIASES and state == 'supported':
            if line in aliases:
                raise ValueError('duplicate alias')
            aliases.add(line); continue
        match = DEVICE.fullmatch(line) if INDENT.match(raw_line) else None
        if not match:
            raise ValueError('unknown Nsight line or section')
        raw_index, label, address = match.groups()
        index = int(raw_index)
        if index >= MAX_DEVICES or not label.strip() or label != label.strip():
            raise ValueError('invalid Nsight device')
        address = pci(address)
        if index in seen_indices or address in seen_bdfs or len(rows) >= MAX_DEVICES:
            raise ValueError('duplicate or excessive Nsight device')
        seen_indices.add(index); seen_bdfs.add(address)
        rows.append((index, address, state == 'supported', label))
    if aliases != ALIASES or not rows or not any(row[2] for row in rows):
        raise ValueError('incomplete Nsight help')
    if unsupported_seen and all(row[2] for row in rows):
        raise ValueError('empty unsupported section')
    return rows

def parse_inventory(value):
    _lines(value)  # Reject lone CR before csv.reader normalizes line endings.
    try:
        rows = list(csv.reader(io.StringIO(value, newline=''), strict=True))
    except csv.Error as exc:
        raise ValueError('invalid CSV') from exc
    if not 1 <= len(rows) <= MAX_DEVICES:
        raise ValueError('invalid inventory count')
    inventory = {}
    uuids = set()
    for row in rows:
        if len(row) != 2:
            raise ValueError('expected PCI BDF and UUID CSV columns')
        address = pci(row[0].strip())
        uuid = row[1].strip()
        if not UUID.fullmatch(uuid):
            raise ValueError('invalid UUID')
        if address in inventory or uuid.lower() in uuids:
            raise ValueError('duplicate BDF or UUID')
        inventory[address] = uuid
        uuids.add(uuid.lower())
    return inventory

def resolve(help_text, inventory_csv, target_bdf, target_uuid):
    """Return (joined devices, unique supported target); fail closed on mismatch."""
    target_bdf = pci(target_bdf)
    if not isinstance(target_uuid, str) or not UUID.fullmatch(target_uuid):
        raise ValueError('invalid target UUID')
    inventory = parse_inventory(inventory_csv)
    devices = []
    for index, address, supported, label in parse_help(help_text):
        if address not in inventory:
            raise ValueError('Nsight BDF missing from inventory')
        devices.append(Device(index, address, inventory[address], supported, label))
    if set(inventory) != {device.pci_bdf for device in devices}:
        raise ValueError('inventory BDF missing from Nsight help')
    matches = [device for device in devices if device.pci_bdf == target_bdf and device.uuid.lower() == target_uuid.lower()]
    if len(matches) != 1 or not matches[0].supported:
        raise ValueError('target absent, mismatched, or unsupported')
    return devices, matches[0]
