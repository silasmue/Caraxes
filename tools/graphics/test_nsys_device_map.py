# SPDX-License-Identifier: MIT
import unittest
import hashlib
import importlib.util
from pathlib import Path

_spec = importlib.util.spec_from_file_location('nsys_device_map', Path(__file__).with_name('nsys-device-map.py'))
_mapping = importlib.util.module_from_spec(_spec)
import sys
sys.modules[_spec.name] = _mapping
_spec.loader.exec_module(_mapping)
resolve, parse_help, parse_inventory = _mapping.resolve, _mapping.parse_help, _mapping.parse_inventory

U1 = 'GPU-11111111-2222-3333-4444-555555555555'
U2 = 'GPU-aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee'
ALIASES = '''    all: Select all supported GPUs
    cuda-visible: Select GPUs that match CUDA_VISIBLE_DEVICES
    none: Disable GPU Metrics [Default]'''
HELP = f'''Possible --gpu-metrics-devices values are:
    7: Turing TU104 | GeForce RTX 2070 SUPER PCI[0000:65:00.0]
{ALIASES}
Some GPUs are not supported:
    0: Volta GV100 | Quadro GV100 PCI[0000:17:00.0]
See the user guide: https://docs.nvidia.com/nsight-systems/UserGuide/index.html#gpu-metrics
'''
CSV = f'00000000:17:00.0, {U2}\n0000:65:00.0, {U1}\n'
CAPTURED_HELP = ('Possible --gpu-metrics-devices values are:\n'
                 '\t0: Turing TU106 | NVIDIA GeForce RTX 2070 PCI[0000:2d:00.0]\n'
                 '\tall: Select all supported GPUs\n'
                 '\tcuda-visible: Select GPUs that match CUDA_VISIBLE_DEVICES\n'
                 '\tnone: Disable GPU Metrics [Default]\n')
CAPTURED_CSV = f'00000000:2D:00.0, {U1}\n'

class MappingTest(unittest.TestCase):
    def test_c302_captured_tab_help_and_inventory(self):
        self.assertEqual(hashlib.sha256(CAPTURED_HELP.encode()).hexdigest(),
                         '331a10988b0a02b76fbab3974485799c8a9394d110490334acf2ccb05f91e1ba')
        rows, target = resolve(CAPTURED_HELP, CAPTURED_CSV, '0000:2d:00.0',
                               U1)
        self.assertEqual((len(rows), target.index, target.pci_bdf, target.supported),
                         (1, 0, '0000:2d:00.0', True))

    def test_tab_only_at_one_leading_help_indent(self):
        self.refuse(CAPTURED_HELP.replace('\t0:', '\t\t0:'), CAPTURED_CSV,
                    '0000:2d:00.0', U1)
        self.refuse(CAPTURED_HELP.replace('\tall:', 'all:'), CAPTURED_CSV,
                    '0000:2d:00.0', U1)
        self.refuse(CAPTURED_HELP.replace('Turing', 'Tur\ting'), CAPTURED_CSV,
                    '0000:2d:00.0', U1)
        self.refuse(CAPTURED_HELP, CAPTURED_CSV.replace(', ', '\t'),
                    '0000:2d:00.0', U1)

    def refuse(self, help_text=HELP, csv=CSV, bdf='0000:65:00.0', uuid=U1):
        with self.assertRaises(ValueError):
            resolve(help_text, csv, bdf, uuid)

    def test_documented_example_and_inventory_order(self):
        rows, target = resolve(HELP, CSV, '00000000:65:00.0', U1)
        self.assertEqual((target.index, target.pci_bdf, target.uuid, target.supported),
                         (7, '0000:65:00.0', U1, True))
        self.assertFalse(rows[1].supported)

    def test_case_and_domain_normalization(self):
        h = HELP.replace('0000:65:00.0', '00000000:AB:0C.7')
        c = CSV.replace('0000:65:00.0', '0000:ab:0c.7')
        self.assertEqual(resolve(h, c, '00000000:ab:0c.7', U1)[1].pci_bdf, '0000:ab:0c.7')
        self.refuse(h, c, '00010000:ab:0c.7', U1)

    def test_unsupported_and_missing_target(self):
        self.refuse(bdf='0000:17:00.0', uuid=U2)
        self.refuse(uuid=U2)
        self.refuse(csv=f'0000:65:00.0, {U1}\n')

    def test_duplicate_index_bdf_uuid(self):
        self.refuse(HELP.replace('    0: Volta', '    7: Volta'))
        self.refuse(HELP.replace('0000:17:00.0', '00000000:65:00.0'))
        self.refuse(csv=f'0000:17:00.0, {U1}\n0000:65:00.0, {U1}\n')
        self.refuse(csv=CSV + f'00000000:65:00.0, {U2}\n')

    def test_malformed_sections_and_tokens(self):
        for h in (HELP.replace('Possible --gpu-metrics-devices values are:', 'Devices:'),
                  HELP.replace('Some GPUs are not supported:', 'Unknown section:'),
                  HELP.replace('    all: Select all supported GPUs', '    all: everything'),
                  HELP.replace('    7:', '    07:'),
                  HELP.replace('PCI[0000:65:00.0]', 'PCI[ffff0000:65:00.0]'),
                  HELP + 'surprise\n', HELP.replace('    7:', '    64:')):
            with self.subTest(h=h[:40]): self.refuse(h)
        self.refuse(csv=CSV.replace(U1, 'GPU-bad'))
        self.refuse(bdf='0001:65:00.0')
        self.refuse(uuid='not-a-uuid')

    def test_bounds(self):
        self.refuse(HELP + 'x' * 16384)
        self.refuse(csv=CSV + 'x' * 16384)

    def test_full_64_and_65_or_index64_refusal(self):
        devices = '\n'.join(f'    {i}: GPU {i} PCI[0000:{i:02x}:00.0]' for i in range(64))
        inventory = ''.join(f'0000:{i:02x}:00.0, GPU-{i:08x}-2222-3333-4444-555555555555\n' for i in range(64))
        help_text = f'Possible --gpu-metrics-devices values are:\n{devices}\n{ALIASES}\n'
        rows, target = resolve(help_text, inventory, '0000:3f:00.0',
                               'GPU-0000003f-2222-3333-4444-555555555555')
        self.assertEqual((len(rows), target.index), (64, 63))
        self.refuse(help_text.replace(ALIASES, '    64: GPU 64 PCI[0000:40:00.0]\n' + ALIASES),
                    inventory + '0000:40:00.0, GPU-00000040-2222-3333-4444-555555555555\n',
                    '0000:3f:00.0', 'GPU-0000003f-2222-3333-4444-555555555555')
        self.refuse(help_text.replace('    63:', '    64:'), inventory,
                    '0000:3f:00.0', 'GPU-0000003f-2222-3333-4444-555555555555')

    def test_invalid_device_field(self):
        self.refuse(HELP.replace('65:00.0', '65:20.0'), CSV.replace('65:00.0', '65:20.0'))
        self.refuse(HELP.replace('65:00.0', '65:ff.0'), CSV.replace('65:00.0', '65:ff.0'))
        self.refuse(bdf='0000:65:20.0')

    def test_controls_utf8_and_line_endings(self):
        for control in ('\x1b', '\x08', '\x0b', '\t', '\x7f'):
            with self.subTest(control=repr(control)):
                self.refuse(HELP.replace('Turing', 'Tur' + control + 'ing'))
                self.refuse(csv=CSV.replace(U1, U1 + control))
        self.refuse(HELP.replace('Turing', 'Tur\ud800ing'))
        self.refuse(csv=CSV.replace(U1, U1 + '\ud800'))
        self.refuse(HELP.replace('\n', '\r', 1))
        self.refuse(csv=CSV.replace('\n', '\r', 1))
        self.assertEqual(resolve(HELP.replace('\n', '\r\n'), CSV.replace('\n', '\r\n'),
                                 '0000:65:00.0', U1)[1].index, 7)

    def test_footer_and_section_boundaries(self):
        self.refuse(HELP.replace('Some GPUs are not supported:\n    0: Volta GV100 | Quadro GV100 PCI[0000:17:00.0]',
                                 'Some GPUs are not supported:'))
        self.refuse(HELP + 'trailing\n')
        self.refuse(HELP.replace('See the user guide:', 'Unknown footer:'))

if __name__ == '__main__':
    unittest.main()
