#!/usr/bin/env python3
"""Pass-2 fixes: structural checks and Unicorn emulation; never starts the server.

Needs the exact original executable (located by test_patch_ps_game/PS_GAME_EXE),
Unicorn for the emulation tests and objdump for the branch-target test. These
tests prove local instruction behaviour, not live server behaviour.
"""
import contextlib
import io
import os
import re
import shutil
import struct
import subprocess
import tempfile
import unittest
from test_patch_ps_game import DOC, EXE, patch, rb
try:
    from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE
    from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX,
        UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP,
        UC_X86_REG_ESP, UC_X86_REG_EIP, UC_X86_REG_EFLAGS)
    HAVE_UNICORN = True
except ImportError:
    HAVE_UNICORN = False

PASS1 = ('mob-speed-id', 'gatekeeper-logout', 'time-damage-player', 'illusion-save',
         'myshop-visit-cleanup', 'learn-skill-level', 'enchant-material-bounds',
         'cloak-lapis', 'cloak-lapisia', 'nan-pos', 'bound-trade', 'bound-duel',
         'fire-thorns-aggro', 'op-exclusive', 'party-exp-levelup', 'stealth-equip',
         'bound-shop', 'character-name-filter', 'guild-text-filter',
         'gift-target-filter', 'gm-notice-filter')
IDS = ('myshop-price-wrap', 'npc-shop-price-wrap', 'remake-distinct', 'enchant-seed',
       'quest-end-npc', 'chat-markup-filter', 'guild-bank-bound', 'gm-target-filter',
       'exchange-count-guard')
FIXES = {f['id']: f for f in DOC['fixes']}
STACK, SP, BUF = 0x70000000, 0x70018000, 0x71000000
PKT, OBJ, OBJ2, OBJ3 = BUF + 0x1000, BUF + 0x4000, BUF + 0x8000, BUF + 0xc000
FLAGS = 0x8d5  # CF PF AF ZF SF OF and the fixed bit


def markup_fixed(text, limit):
    """Independent model of chat-markup-filter: up to the first NUL or `limit`,
    keep `<<` NNN `c` CC with 1 <= NNN <= bytes after it; other `<<` lose their
    first `<` (made a space)."""
    out = bytearray(text)
    end = min(limit, len(out))
    if 0 in out[:end]:
        end = out.index(0)
    i = 0
    while i + 1 < end:
        if out[i] == 0x3c and out[i + 1] == 0x3c:
            m = bytes(out[i:i + 8])
            ok = (i + 8 <= end and m[2:5].isdigit() and m[5:6] == b'c' and m[6:8].isdigit()
                  and 1 <= int(m[2:5]) <= end - (i + 8))
            if ok:
                i += 8
                continue
            out[i] = 0x20
        i += 1
    return bytes(out)


MARKUP_CASES = {
    'plain': b'hello world\0',
    'chatcolor': b'<<005c01hello\0',
    'exact-fit-no-nul': b'<<003c02abc',
    'overrun': b'<<999c01hi\0',
    'overrun-by-one': b'<<006c01hello\0',
    'zero-length': b'<<000c01hello\0',
    'at-end': b'abc<<\0',
    'short-marker': b'<<12\0',
    'not-digits': b'<<0a5c01hello\0',
    'no-c': b'<<005x01hello\0',
    'bad-colour': b'<<005c0zhello\0',
    'triple': b'<<<005c01hello\0',
    'nested-bad': b'<<016c01<<999c01abcdefgh\0',
    'nested-good': b'<<012c01<<002c03ab\0',
    'two-good': b'<<002c01ab<<001c02c\0',
    'past-nul': b'<<005c01\0hello',
    'after-nul': b'ok\0<<999c01',
    'many': b'<<' * 20 + b'\0',
    'high-bytes': b'\x80<<\xff\xfe<<001c01x\0',
}


def text_section():
    data = rb(EXE)
    pe = struct.unpack_from('<I', data, 0x3c)[0]
    count = struct.unpack_from('<H', data, pe + 6)[0]
    opt = pe + 24
    optsize = struct.unpack_from('<H', data, pe + 20)[0]
    return data, pe, count, opt, optsize


@unittest.skipIf(EXE is None, 'expected ps_game.exe not found; set PS_GAME_EXE')
class Pass2Structure(unittest.TestCase):
    def test_new_fixes_present_and_old_ones_first(self):
        ids = [f['id'] for f in DOC['fixes']]
        self.assertEqual(tuple(ids[:len(PASS1)]), PASS1)
        self.assertEqual(tuple(ids[len(PASS1):len(PASS1) + len(IDS)]), IDS)

    def test_caves_inside_region_and_disjoint(self):
        ranges = []
        for f in DOC['fixes']:
            for p in f['inplace']:
                n = len(bytes.fromhex(p['patched']))
                ranges.append((p['va'], p['va'] + n, f['id']))
                if 'fill' in p:
                    self.assertGreaterEqual(p['va'], DOC['cave']['start'])
                    self.assertLessEqual(p['va'] + n, DOC['cave']['end'])
        ranges.sort()
        for (a0, a1, i0), (b0, b1, i1) in zip(ranges, ranges[1:]):
            self.assertLessEqual(a1, b0, '%s overlaps %s' % (i0, i1))

    def test_no_branch_lands_inside_a_hook(self):
        if shutil.which('objdump') is None:
            self.skipTest('objdump not installed')
        out = subprocess.run(['objdump', '-d', '-M', 'intel', '--no-show-raw-insn',
                              '--start-address=0x401000', '--stop-address=0x520250', EXE],
                             capture_output=True, text=True, check=True).stdout
        targets = set(int(t, 16) for t in re.findall(
            r'\s(?:j[a-z]+|call|loop[a-z]*)\s+0x([0-9a-f]+)\b', out))
        for fid in IDS:
            for p in FIXES[fid]['inplace']:
                if 'fill' in p:
                    continue
                start, end = p['va'], p['va'] + len(bytes.fromhex(p['patched']))
                inside = [hex(t) for t in targets if start < t < end]
                self.assertEqual(inside, [], '%s: branch into hook at %x' % (fid, start))

    def test_pass2_composes_with_previous_kit(self):
        original = rb(EXE)
        with tempfile.TemporaryDirectory() as d:
            target = os.path.join(d, 'ps_game.exe')
            shutil.copy2(EXE, target)
            with contextlib.redirect_stdout(io.StringIO()):
                patch.cmd_apply(DOC, target, list(PASS1))
                previous = rb(target)
                patch.cmd_apply(DOC, target, list(IDS))
                for f in DOC['fixes']:
                    if f['id'] in PASS1 + IDS:
                        self.assertEqual(patch.state(rb(target), f), 'applied', f['id'])
                self.assertEqual(bytes(patch.pristine(DOC, rb(target))), original)
                patch.cmd_revert(DOC, target, list(IDS))
                self.assertEqual(rb(target), previous)
                patch.cmd_revert(DOC, target, [])
            self.assertEqual(rb(target), original)

    def test_each_new_fix_alone_round_trips(self):
        original = rb(EXE)
        with tempfile.TemporaryDirectory() as d:
            target = os.path.join(d, 'ps_game.exe')
            for fid in IDS:
                shutil.copy2(EXE, target)
                with contextlib.redirect_stdout(io.StringIO()):
                    patch.cmd_apply(DOC, target, [fid])
                    self.assertEqual(patch.state(rb(target), FIXES[fid]), 'applied')
                    others = [f for f in DOC['fixes'] if f['id'] != fid]
                    for f in others:
                        self.assertEqual(patch.state(rb(target), f), 'clean', f['id'])
                    patch.cmd_revert(DOC, target, [fid])
                self.assertEqual(rb(target), original, fid)


class Stop(Exception):
    pass


@unittest.skipUnless(HAVE_UNICORN and EXE, 'requires Unicorn and expected ps_game.exe')
class Pass2Emulation(unittest.TestCase):
    REGS = None

    @classmethod
    def setUpClass(cls):
        cls.original = rb(EXE)
        cls.REGS = (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX,
                    UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_ESP,
                    UC_X86_REG_EFLAGS)

    def machine(self, fix_id=None):
        data = bytearray(self.original)
        if fix_id:
            for item in FIXES[fix_id]['inplace']:
                patch.write_patch(data, item)
        pe = struct.unpack_from('<I', data, 0x3c)[0]
        count = struct.unpack_from('<H', data, pe + 6)[0]
        opt = pe + 24
        optsize = struct.unpack_from('<H', data, pe + 20)[0]
        base = struct.unpack_from('<I', data, opt + 28)[0]
        size = struct.unpack_from('<I', data, opt + 56)[0]
        uc = Uc(UC_ARCH_X86, UC_MODE_32)
        uc.mem_map(base, (size + 0xfff) & ~0xfff)
        for i in range(count):
            sec = opt + optsize + i * 40
            va, rawsize, raw = struct.unpack_from('<III', data, sec + 12)
            uc.mem_write(base + va, bytes(data[raw:raw + rawsize]))
        uc.mem_map(STACK, 0x20000)
        uc.mem_map(BUF, 0x10000)
        uc.mem_write(STACK, b'\xa5' * 0x20000)
        uc.mem_write(BUF, b'\xcc' * 0x10000)
        for i, reg in enumerate(self.REGS[:-2]):
            uc.reg_write(reg, 0x10203040 + i * 0x01010101)
        uc.reg_write(UC_X86_REG_ESP, SP)
        uc.reg_write(UC_X86_REG_EFLAGS, FLAGS)
        return uc

    def state(self, uc):
        return tuple(uc.reg_read(r) for r in self.REGS)

    def go(self, uc, start, stops, stubs=None):
        """Run from `start` until EIP reaches any address in `stops`; return it."""
        stops = set(stops)
        stubs = stubs or {}
        hit = []

        def step(m, address, size, unused):
            if address in stubs:
                stubs[address](m)
                return
            if address in stops:
                hit.append(address)
                m.emu_stop()
        h = uc.hook_add(UC_HOOK_CODE, step)
        try:
            uc.emu_start(start, 0xffffffff, count=200000)
        finally:
            uc.hook_del(h)
        self.assertEqual(len(hit), 1, 'did not reach an exit (EIP %x)' % uc.reg_read(UC_X86_REG_EIP))
        return hit[0]

    def put(self, uc, address, payload):
        uc.mem_write(address, payload)

    def get(self, uc, address, n):
        return bytes(uc.mem_read(address, n))

    # --- chat-markup-filter --------------------------------------------------
    def chat_case(self, site, resume, reg, packet, text_off, limit, op=None):
        """Run the dispatch hook on `packet` (text at text_off) in the original and
        the patched image; the text must match the model, everything else the
        original, register for register."""
        old, new = self.machine(), self.machine('chat-markup-filter')
        for uc in (old, new):
            self.put(uc, PKT - 16, b'\xdd' * 16 + packet + b'\xee' * 16)
            uc.reg_write(reg, PKT)
            self.assertEqual(self.go(uc, site, [resume]), resume)
        want = bytearray(packet)
        if limit is not None:
            want[text_off:] = markup_fixed(bytes(packet[text_off:]), limit)
        self.assertEqual(self.get(new, PKT - 16, len(packet) + 32),
                         b'\xdd' * 16 + bytes(want) + b'\xee' * 16)
        self.assertEqual(self.get(old, PKT, len(packet)), packet)
        self.assertEqual(self.state(new), self.state(old))

    def test_chat_cases(self):
        for op in (0x1101, 0x1103, 0x1104, 0x1105, 0x1107, 0x1108, 0x1111, 0x1112):
            for label, text in MARKUP_CASES.items():
                with self.subTest(op=hex(op), case=label):
                    pkt = struct.pack('<HB', op, len(text)) + text
                    self.chat_case(0x47052f, 0x470537, UC_X86_REG_ESI, pkt, 3, len(text))

    def test_chat_whisper_and_untouched(self):
        for label, text in MARKUP_CASES.items():
            with self.subTest(case=label):
                pkt = struct.pack('<H', 0x1102) + b'<<999c01name\0'.ljust(21, b'\0')
                pkt += bytes([len(text)]) + text
                self.chat_case(0x47052f, 0x470537, UC_X86_REG_ESI, pkt, 0x18, len(text))
        bad = b'<<999c01hi\0'
        for op in (0x1106, 0x1109, 0x1110, 0x1113, 0x1100, 0x2201):
            with self.subTest(op=hex(op)):
                self.chat_case(0x47052f, 0x470537, UC_X86_REG_ESI,
                               struct.pack('<HB', op, len(bad)) + bad, 3, None)
        long = b'<<999c01' + b'x' * 0x80
        with self.subTest(case='too-long'):  # the case closes the connection anyway
            self.chat_case(0x47052f, 0x470537, UC_X86_REG_ESI,
                           struct.pack('<HB', 0x1101, 0x81) + long, 3, None)
        with self.subTest(case='limit-shorter-than-buffer'):
            text = b'<<003c01abcdef'
            self.chat_case(0x47052f, 0x470537, UC_X86_REG_ESI,
                           struct.pack('<HB', 0x1101, 10) + text, 3, 10)

    def test_admin_chat(self):
        for op in (0xf101, 0xf103, 0xf104, 0xf105, 0xf108, 0xf10a):
            for label, text in MARKUP_CASES.items():
                with self.subTest(op=hex(op), case=label):
                    pkt = struct.pack('<HB', op, len(text)) + text
                    self.chat_case(0x470f5a, 0x470f62, UC_X86_REG_EDI, pkt, 3, len(text))
        for label, text in MARKUP_CASES.items():
            with self.subTest(op='f102', case=label):
                pkt = struct.pack('<H', 0xf102) + b'name'.ljust(21, b'\0')
                pkt += bytes([len(text)]) + text
                self.chat_case(0x470f5a, 0x470f62, UC_X86_REG_EDI, pkt, 0x18, len(text))
            with self.subTest(op='f106', case=label):
                # u16 len; the handler forces the last byte to NUL afterwards
                pkt = struct.pack('<HH', 0xf106, len(text)) + text
                self.chat_case(0x470f5a, 0x470f62, UC_X86_REG_EDI, pkt, 4, len(text) - 1)
        bad = b'<<999c01hi\0'
        for op in (0xf107, 0xf109, 0xf10b, 0xf100):
            with self.subTest(op=hex(op)):
                self.chat_case(0x470f5a, 0x470f62, UC_X86_REG_EDI,
                               struct.pack('<HB', op, len(bad)) + bad, 3, None)
        with self.subTest(op='f106 too long'):
            self.chat_case(0x470f5a, 0x470f62, UC_X86_REG_EDI,
                           struct.pack('<HH', 0xf106, 0x7d1) + bad, 4, None)

    def test_notices(self):
        offsets = {0xf906: 2, 0xf907: 4, 0xf908: 0x19, 0xf909: 4, 0xf90a: 5, 0xf90b: 4}
        for op, at in offsets.items():
            for label, text in MARKUP_CASES.items():
                with self.subTest(op=hex(op), case=label):
                    pkt = bytearray(struct.pack('<H', op) + b'\x07' * (at - 2))
                    if op == 0xf908:
                        pkt[2:0x17] = b'<<999c01who'.ljust(21, b'\0')
                    pkt += bytes([len(text)]) + text
                    self.chat_case(0x4739d6, 0x4739de, UC_X86_REG_EBX, bytes(pkt), at + 1,
                                   len(text))
        bad = b'\x05<<999c01hi\0'
        for op in (0xf901, 0xf905, 0xf90c):
            with self.subTest(op=hex(op)):
                self.chat_case(0x4739d6, 0x4739de, UC_X86_REG_EBX,
                               struct.pack('<HH', op, 0) + bad, 4, None)

    def test_memos(self):
        table = 0x476280
        for op in (0x2206, 0x220b, 0x2205, 0x220a, 0x2202):
            for label, text in MARKUP_CASES.items():
                with self.subTest(op=hex(op), case=label):
                    pkt = struct.pack('<HIB', op, 1234, len(text)) + text
                    pkt = pkt.ljust(58, b'Q')
                    old, new = self.machine(), self.machine('chat-markup-filter')
                    idx = op - 0x2202
                    dest = struct.unpack('<I', self.get(old, table + 4 * idx, 4))[0]
                    for uc in (old, new):
                        self.put(uc, PKT, pkt)
                        uc.reg_write(UC_X86_REG_EBX, PKT)
                        uc.reg_write(UC_X86_REG_EAX, idx)
                        self.assertEqual(self.go(uc, 0x4758a6, [dest]), dest)
                    want = bytearray(pkt)
                    if op in (0x2206, 0x220b):
                        want[7:] = markup_fixed(bytes(pkt[7:]), 50)
                    self.assertEqual(self.get(new, PKT, 58), bytes(want))
                    # EFLAGS differ at the switch target; every target writes them
                    # (cmp, test or a call) before reading them.
                    self.assertEqual(self.state(new)[:8], self.state(old)[:8])

    def test_shop_title(self):
        for label, text in list(MARKUP_CASES.items()) + [('full', b'<<119c01' + b'x' * 119)]:
            with self.subTest(case=label):
                pkt = struct.pack('<HB', 0x2303, min(len(text), 120)) + text
                pkt = pkt.ljust(124, b'\0')[:124]
                pkt = pkt[:0x7b] + b'\0' + pkt[0x7c:]     # the handler's NUL at 120
                old, new = self.machine(), self.machine('chat-markup-filter')
                for uc in (old, new):
                    self.put(uc, PKT, pkt)
                    uc.reg_write(UC_X86_REG_ESI, PKT)
                    uc.reg_write(UC_X86_REG_EBP, OBJ)
                    self.assertEqual(self.go(uc, 0x478d0a, [0x478d13]), 0x478d13)
                want = pkt[:3] + markup_fixed(pkt[3:], 120)
                self.assertEqual(self.get(new, PKT, 124), want)
                self.assertEqual(self.state(new), self.state(old))

    # --- price products ---------------------------------------------------------
    PRICES = [(1, 0xffffffff), (3, 0x55555555), (3, 0x55555556), (255, 0xffffffff),
              (2, 0x80000000), (2, 0x7fffffff), (255, 0x01010101), (255, 0x01010102),
              (17, 0x0f0f0f0f), (17, 0x0f0f0f10), (1, 0), (200, 123456)]

    def test_myshop_price(self):
        for count, price in self.PRICES:
            with self.subTest(count=count, price=hex(price)):
                old, new = self.machine(), self.machine('myshop-price-wrap')
                for uc in (old, new):
                    self.put(uc, OBJ + 3 * 4 + 0x1644, struct.pack('<I', price))
                    uc.reg_write(UC_X86_REG_EBX, OBJ)
                    uc.reg_write(UC_X86_REG_EAX, 3)
                    uc.reg_write(UC_X86_REG_ECX, 0xabcd00 | count)
                end = self.go(new, 0x4790ed, [0x4790f8, 0x479116])
                self.go(old, 0x4790ed, [0x4790f8])
                if count * price >> 32:
                    self.assertEqual(end, 0x479116)         # result 2
                else:
                    self.assertEqual(end, 0x4790f8)
                    self.assertEqual(new.reg_read(UC_X86_REG_EDX), count * price)
                    self.assertEqual(old.reg_read(UC_X86_REG_EDX), count * price)
                    for r in (UC_X86_REG_EBX, UC_X86_REG_ESI, UC_X86_REG_EDI,
                              UC_X86_REG_EBP, UC_X86_REG_ESP):
                        self.assertEqual(new.reg_read(r), old.reg_read(r))

    def test_npc_shop_price(self):
        for count, price in self.PRICES:
            with self.subTest(count=count, price=hex(price)):
                old, new = self.machine(), self.machine('npc-shop-price-wrap')
                for uc in (old, new):
                    self.put(uc, PKT + 7, bytes([count]))
                    self.put(uc, OBJ + 0x80, struct.pack('<I', price))
                    self.put(uc, SP + 0x10, struct.pack('<I', OBJ2))
                    uc.reg_write(UC_X86_REG_EBX, PKT)
                    uc.reg_write(UC_X86_REG_ESI, OBJ)
                end = self.go(new, 0x46b55a, [0x46b565, 0x46b57e])
                self.go(old, 0x46b55a, [0x46b565])
                self.assertEqual(new.reg_read(UC_X86_REG_EDX), 0x13233343)
                self.assertEqual(new.reg_read(UC_X86_REG_ESP), SP)
                if count * price >> 32:
                    self.assertEqual(end, 0x46b57e)          # result 1
                    self.assertEqual(new.reg_read(UC_X86_REG_EBP), OBJ2)
                    self.assertEqual(self.get(new, SP + 0x20, 2), b'\x02\x07')
                else:
                    self.assertEqual(end, 0x46b565)
                    self.assertEqual(self.state(new), self.state(old)[:8] +
                                     (new.reg_read(UC_X86_REG_EFLAGS),))

    # --- remake ----------------------------------------------------------------
    def test_remake_distinct(self):
        objs = (OBJ, OBJ2, OBJ3)
        for a, b_, c in [(0, 1, 2), (0, 0, 1), (0, 1, 0), (1, 0, 0), (2, 2, 2), (2, 1, 0)]:
            for mtype in (0x51, 0x43):
                with self.subTest(places=(a, b_, c), make=hex(mtype)):
                    old, new = self.machine(), self.machine('remake-distinct')
                    for uc in (old, new):
                        for o in objs:
                            self.put(uc, o + 0x68, bytes([mtype]))
                        uc.reg_write(UC_X86_REG_ESI, objs[a])
                        uc.reg_write(UC_X86_REG_ECX, objs[b_])
                        uc.reg_write(UC_X86_REG_EDX, objs[c])
                    end = self.go(new, 0x460983, [0x460989, 0x461013])
                    if len({a, b_, c}) < 3:
                        self.assertEqual(end, 0x461013)
                    else:
                        self.assertEqual(end, 0x460989)
                        self.go(old, 0x460983, [0x460989])
                        self.assertEqual(self.state(new), self.state(old))

    # --- enchant seed ----------------------------------------------------------
    def test_enchant_seed(self):
        seeds = []
        for _ in range(2):
            uc = self.machine('enchant-seed')
            self.put(uc, OBJ + 0x575c, struct.pack('<I', 0x5a5a1234))
            uc.reg_write(UC_X86_REG_EBX, OBJ)
            uc.reg_write(UC_X86_REG_EAX, 1700000000)
            before = self.state(uc)
            self.assertEqual(self.go(uc, 0x45fc8e, [0x45fc96]), 0x45fc96)
            after = self.state(uc)
            for i in (1, 2, 4, 6, 7):                 # ebx ecx esi ebp esp kept
                self.assertEqual(after[i], before[i])
            edi = uc.reg_read(UC_X86_REG_EDI)
            self.assertEqual(edi ^ uc.reg_read(UC_X86_REG_EAX), 1700000000 ^ 0x5a5a1234)
            seeds.append(edi)
        self.assertNotEqual(seeds[0], seeds[1])

    # --- quest -----------------------------------------------------------------
    def test_quest_end_npc(self):
        for npc in (0, 1, 0x12345):
            for kind in (0, 1, 2, 3, 4, 0xffffffff):
                with self.subTest(npc=npc, kind=kind):
                    old, new = self.machine(), self.machine('quest-end-npc')
                    for uc in (old, new):
                        self.put(uc, SP + 0x18, struct.pack('<I', npc))
                        uc.reg_write(UC_X86_REG_EAX, kind)
                    end = self.go(new, 0x47f2fc, [0x47f301, 0x47f358])
                    if npc == 0 or kind > 3:
                        self.assertEqual(end, 0x47f358)
                    else:
                        self.assertEqual(end, 0x47f301)
                        self.go(old, 0x47f2fc, [0x47f301])
                        self.assertEqual(self.state(new)[:8], self.state(old)[:8])

    # --- guild bank --------------------------------------------------------------
    def test_guild_bank_swap(self):
        lock = 0x0badf00d
        for label, item, rtype, og in (('empty', 0, 0, 0), ('normal', OBJ, 5, 0),
                                       ('quest', OBJ, 0x1b, 0), ('bound', OBJ, 5, 1),
                                       ('bound2', OBJ, 5, 2)):
            with self.subTest(case=label):
                left = []

                def leave(m):
                    esp = m.reg_read(UC_X86_REG_ESP)
                    left.append(struct.unpack('<I', m.mem_read(esp, 4))[0])
                    m.reg_write(UC_X86_REG_ESP, esp + 4)    # stdcall, one argument
                    m.reg_write(UC_X86_REG_EIP, 0x52075d)
                uc = self.machine('guild-bank-bound')
                self.put(uc, SP + 0x2c, struct.pack('<I', lock))
                self.put(uc, SP + 0x22c, struct.pack('<II', 2, 5))     # bag, slot
                slot = OBJ3 + (5 + 2 * 24) * 4 + 0x1c4
                self.put(uc, slot, struct.pack('<I', item))
                self.put(uc, OBJ + 0x30, struct.pack('<I', OBJ2))
                self.put(uc, OBJ2 + 0xa0, struct.pack('<I', rtype))
                self.put(uc, OBJ2 + 0x31, bytes([og]))
                uc.reg_write(UC_X86_REG_EDI, OBJ3)
                end = self.go(uc, 0x42ffc4, [0x430235, 0x42fff1, 0x430624],
                               {0x520757: leave})
                self.assertEqual(uc.reg_read(UC_X86_REG_ESP), SP)
                if not item:
                    self.assertEqual((end, left), (0x430235, []))
                elif rtype == 0x1b or og:
                    self.assertEqual((end, left), (0x430624, [lock]))
                else:
                    self.assertEqual((end, left), (0x42fff1, []))
                    self.assertEqual(uc.reg_read(UC_X86_REG_EAX), 2)   # bag, pushed next
                    self.assertEqual(uc.reg_read(UC_X86_REG_ESI), slot)

    # --- gm target ---------------------------------------------------------------
    def test_gm_target_copy(self):
        for site in (0x471b94, 0x471c94):
            for value in (0, 0x27, 0x20, 0x7f, 0x80, 0xff):
                for flags in (0x202, 0x246, 0x293, 0xa97):
                    with self.subTest(site=hex(site), value=value, flags=hex(flags)):
                        uc = self.machine('gm-target-filter')
                        self.put(uc, PKT, bytes([value]))
                        self.put(uc, OBJ, b'\xcc')
                        uc.reg_write(UC_X86_REG_EAX, PKT)
                        uc.reg_write(UC_X86_REG_EDX, OBJ - PKT)
                        uc.reg_write(UC_X86_REG_ECX, 0x123456ab)
                        uc.reg_write(UC_X86_REG_EFLAGS, flags)
                        want = list(self.state(uc))
                        result = 0x20 if value == 0x27 else value
                        want[2] = 0x12345600 | result
                        self.assertEqual(self.go(uc, site, [site + 5]), site + 5)
                        self.assertEqual(self.state(uc), tuple(want))
                        self.assertEqual(self.get(uc, PKT, 1), bytes([value]))
                        self.assertEqual(self.get(uc, OBJ, 1), bytes([result]))

    def test_gm_target_whole_loop(self):
        for site, after, dest in ((0x471b94, 0x471ba0, 0x5e), (0x471c94, 0x471ca0, 0x66)):
            for name in (b"O'Neil\0", b"''\0", b"\0", b"plain\0", b"x'" * 10 + b"\0"):
                with self.subTest(site=hex(site), name=name):
                    uc = self.machine('gm-target-filter')
                    self.put(uc, PKT, name)
                    uc.reg_write(UC_X86_REG_EBX, PKT)
                    start = site - 8          # lea edx,[esp+dest]; mov eax,ebx; sub edx,ebx
                    self.assertEqual(self.go(uc, start, [after]), after)
                    self.assertEqual(self.get(uc, SP + dest, len(name)), name.replace(b"'", b' '))
                    self.assertEqual(self.get(uc, PKT, len(name)), name)

    # --- exchange guard ------------------------------------------------------------
    def test_exchange_guard(self):
        sites = ((0x46f9db, UC_X86_REG_EDI, 0x46f9e7, 0x4700e6, 0x46fb21),
                 (0x47ccbb, UC_X86_REG_EBX, 0x47ccc3, 0x47d052, 0x47cd0f))
        for site, reg, whole, skip, split in sites:
            for offered, have in ((5, 5), (5, 3), (3, 5), (1, 255), (255, 1), (1, 1), (0, 0)):
                with self.subTest(site=hex(site), offered=offered, have=have):
                    old, new = self.machine(), self.machine('exchange-count-guard')
                    for uc in (old, new):
                        self.put(uc, PKT, bytes([1, 2, offered]))
                        self.put(uc, OBJ + 0x42, bytes([have]))
                        uc.reg_write(reg, PKT)
                        uc.reg_write(UC_X86_REG_EBP, OBJ)
                    end = self.go(new, site, [whole, skip, split])
                    oend = self.go(old, site, [whole, split])
                    if offered > have:
                        self.assertEqual(end, skip)
                        self.assertEqual(oend, split)    # the original subtracts past 0
                    else:
                        self.assertEqual(end, oend)
                        self.assertEqual(end, whole if offered == have else split)
                        self.assertEqual(self.state(new), self.state(old))
                    self.assertEqual(new.reg_read(UC_X86_REG_ESP), SP)
                    self.assertEqual(new.reg_read(reg), PKT)


if __name__ == '__main__':
    unittest.main(verbosity=2)
