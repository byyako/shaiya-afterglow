#!/usr/bin/env python3
"""Emulate distributed x86 filters; never start the server. Optional: pip install unicorn.
Needs the exact original executable located by test_patch_ps_game/PS_GAME_EXE.
These tests prove local instruction behavior, not live database behavior.
"""
import contextlib
import io
import os
import shutil
import struct
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
IDS = ('character-name-filter', 'guild-text-filter', 'gift-target-filter', 'gm-notice-filter')
FIXES = {f['id']: f for f in DOC['fixes']}
STACK, SP, BUFFER = 0x70000000, 0x70018000, 0x71000000
SOURCE, DEST = BUFFER + 0x1000, BUFFER + 0x3000


def filtered(data, width):
    """Independent specification: replace quote; stop at NUL or width."""
    out = bytearray(data)
    for i in range(min(width, len(out))):
        if out[i] == 0:
            break
        if out[i] == 0x27:
            out[i] = 0x20
    return bytes(out)


def cases(width):
    return {'first': b"'ab\0", 'middle': b"a'b\0", 'last': b"ab'\0",
        'all': b"'" * width + b'\0', 'empty': b"\0'untouched\0",
        'maximum': b'x' * (width - 1) + b"'\0", 'high': b"\x80'\xfe\xff\0",
        'embedded-nul': b"a'\0'after\0", 'no-nul': b"'" * (width + 8),
        'beyond-width': b'x' * width + b"'beyond\0"}


@unittest.skipIf(EXE is None, 'expected ps_game.exe not found; set PS_GAME_EXE')
class InputFilterRoundTrips(unittest.TestCase):
    def test_filters_and_partial_selection_round_trip(self):
        original = rb(EXE)
        with tempfile.TemporaryDirectory() as directory:
            target = os.path.join(directory, 'ps_game.exe')
            shutil.copy2(EXE, target)
            for fix_id in IDS:
                self.assertIn(fix_id, FIXES)
                with contextlib.redirect_stdout(io.StringIO()):
                    patch.cmd_apply(DOC, target, [fix_id])
                self.assertEqual(patch.state(rb(target), FIXES[fix_id]), 'applied')
                self.assertEqual(bytes(patch.pristine(DOC, rb(target))), original)
            for i, fix_id in enumerate(IDS):
                with contextlib.redirect_stdout(io.StringIO()):
                    patch.cmd_revert(DOC, target, [fix_id])
                data = rb(target)
                for j, other in enumerate(IDS):
                    self.assertEqual(patch.state(data, FIXES[other]), 'clean' if j <= i else 'applied')
                h = DOC['header']
                want = h['patched' if i + 1 < len(IDS) else 'original']
                self.assertEqual(data[h['off']:h['off'] + 4], bytes.fromhex(want))
                self.assertEqual(bytes(patch.pristine(DOC, data)), original)
            self.assertEqual(rb(target), original)


@unittest.skipUnless(HAVE_UNICORN and EXE, 'requires Unicorn and expected ps_game.exe')
class InputFilterInstructions(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.original = rb(EXE)
        for fix_id in IDS:
            if fix_id not in FIXES:
                raise AssertionError('missing input filter: ' + fix_id)
        cls.regs = (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX,
            UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_EDI,
            UC_X86_REG_EBP, UC_X86_REG_ESP, UC_X86_REG_EFLAGS)

    def machine(self, fix_id=None):
        data = bytearray(self.original)
        if fix_id:
            for item in FIXES[fix_id]['inplace']:
                before = bytes.fromhex(item['original'])
                self.assertEqual(data[item['off']:item['off'] + len(before)], before)
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
        uc.mem_map(BUFFER, 0x10000)
        uc.mem_write(STACK, b'\xa5' * 0x20000)
        uc.mem_write(BUFFER, b'\xcc' * 0x10000)
        for i, reg in enumerate(self.regs[:-2]):
            uc.reg_write(reg, 0x10203040 + i * 0x01010101)
        uc.reg_write(UC_X86_REG_ESP, SP)
        uc.reg_write(UC_X86_REG_EFLAGS, 0x246)
        return uc

    def state(self, uc):
        return tuple(uc.reg_read(r) for r in self.regs)

    def execute(self, uc, start, stop):
        calls = []
        def observe(machine, address, size, unused):
            if address in (0x46c250, 0x504c60, 0x42b800):
                calls.append(address)
            if address == 0x42b800:
                # Stub only the process-dependent guild lock constructor.
                for r, value in ((UC_X86_REG_EAX, 0xabcdef01),
                    (UC_X86_REG_ECX, 0x12345678), (UC_X86_REG_EDX, 0x87654321),
                    (UC_X86_REG_EFLAGS, 0x293)):
                    machine.reg_write(r, value)
                esp = machine.reg_read(UC_X86_REG_ESP)
                ret = struct.unpack('<I', machine.mem_read(esp, 4))[0]
                machine.reg_write(UC_X86_REG_ESP, esp + 4)
                machine.reg_write(UC_X86_REG_EIP, ret)
        handle = uc.hook_add(UC_HOOK_CODE, observe)
        try:
            uc.emu_start(start, stop, count=20000)
        finally:
            uc.hook_del(handle)
        self.assertEqual(uc.reg_read(UC_X86_REG_EIP), stop, 'instruction limit reached')
        return calls

    def buffer(self, uc, address, payload):
        data = payload + b'\xcc' * (128 - len(payload))
        uc.mem_write(address - 16, b'\xdd' * 16 + data + b'\xee' * 16)
        return data

    def check(self, uc, address, expected):
        self.assertEqual(bytes(uc.mem_read(address, len(expected))), expected)
        self.assertEqual(bytes(uc.mem_read(address - 16, 16)), b'\xdd' * 16)
        self.assertEqual(bytes(uc.mem_read(address + len(expected), 16)), b'\xee' * 16)

    def test_character_create_rename_real_trim_before_validation(self):
        for hook, resume in ((0x46c006, 0x46c00b), (0x46c173, 0x46c178)):
            for label, payload in cases(20).items():
                with self.subTest(hook=hex(hook), case=label):
                    old, new = self.machine(), self.machine('character-name-filter')
                    for uc in (old, new):
                        self.buffer(uc, SOURCE, payload)
                        uc.reg_write(UC_X86_REG_EBP, SOURCE)
                        uc.reg_write(UC_X86_REG_EDX, SOURCE)
                        uc.reg_write(UC_X86_REG_ESI, 21)
                        self.assertEqual(self.execute(uc, hook, resume), [0x46c250])
                    self.check(new, SOURCE, filtered(bytes(old.mem_read(SOURCE, 128)), 20))
                    self.assertEqual(self.state(new), self.state(old))

    def test_guild_create_both_fields_after_displaced_call(self):
        for label, payload in cases(24).items():
            for remark_label, remark in cases(64).items():
                with self.subTest(name=label, remark=remark_label):
                    old, new = self.machine(), self.machine('guild-text-filter')
                    packet = bytearray(b'\xcc' * 128)
                    packet[2:2 + min(24, len(payload))] = payload[:24]
                    packet[26] = 0  # original caller terminates both fields
                    packet[27:27 + min(64, len(remark))] = remark[:64]
                    packet[91] = 0
                    for uc in (old, new):
                        self.buffer(uc, SOURCE, bytes(packet))
                        uc.reg_write(UC_X86_REG_EBX, SOURCE)
                        self.assertEqual(self.execute(uc, 0x476507, 0x47650c), [0x42b800])
                    packet[2:26] = filtered(packet[2:26], 24)
                    packet[27:91] = filtered(packet[27:91], 64)
                    self.check(new, SOURCE, bytes(packet))
                    self.assertEqual(self.state(new), self.state(old))

    def test_guild_remark_bound_and_displaced_add_flags(self):
        for label, payload in cases(64).items():
            with self.subTest(case=label):
                old, new = self.machine(), self.machine('guild-text-filter')
                for uc in (old, new):
                    data = self.buffer(uc, SOURCE, payload)
                    uc.reg_write(UC_X86_REG_EBX, SOURCE - 2)
                    self.assertEqual(self.execute(uc, 0x477102, 0x477107), [])
                self.check(new, SOURCE, filtered(data, 64))
                self.assertEqual(self.state(new), self.state(old))
                self.assertEqual(new.reg_read(UC_X86_REG_EBX), SOURCE)
                self.assertEqual(new.reg_read(UC_X86_REG_ECX), SOURCE)

    def test_gift_filters_before_real_strncpy(self):
        for label, payload in cases(20).items():
            with self.subTest(case=label):
                old, new = self.machine(), self.machine('gift-target-filter')
                expected = filtered(self.buffer(new, SOURCE, payload), 20)
                self.buffer(old, SOURCE, expected)
                for uc in (old, new):
                    self.buffer(uc, DEST, b'\xcc' * 128)
                    uc.reg_write(UC_X86_REG_EBP, SOURCE)
                    uc.mem_write(SP, struct.pack('<III', DEST, SOURCE, 21))
                    self.assertEqual(self.execute(uc, 0x479f4a, 0x479f4f), [0x504c60])
                    self.assertEqual(bytes(uc.mem_read(SP, 12)), struct.pack('<III', DEST, SOURCE, 21))
                self.check(new, SOURCE, expected)
                self.check(new, DEST, bytes(old.mem_read(DEST, 128)))
                self.assertEqual(self.state(new), self.state(old))
                copied = expected.split(b'\0', 1)[0][:21]
                self.assertEqual(bytes(new.mem_read(DEST, len(copied))), copied)

    def test_gm_single_copy_preserves_flags_partial_ecx(self):
        for value in (0, 0x27, 0x20, 0x7f, 0x80, 0xff):
            for flags in (0x202, 0x246, 0x293, 0xa97):
                with self.subTest(value=value, flags=hex(flags)):
                    uc = self.machine('gm-notice-filter')
                    self.buffer(uc, SOURCE, bytes([value]))
                    self.buffer(uc, DEST, b'\xcc')
                    uc.reg_write(UC_X86_REG_EAX, SOURCE)
                    uc.reg_write(UC_X86_REG_EDX, DEST - SOURCE)
                    uc.reg_write(UC_X86_REG_ECX, 0x123456ab)
                    uc.reg_write(UC_X86_REG_EFLAGS, flags)
                    expected = list(self.state(uc))
                    result = 0x20 if value == 0x27 else value
                    expected[2] = 0x12345600 | result
                    self.assertEqual(self.execute(uc, 0x471ca8, 0x471cad), [])
                    self.assertEqual(self.state(uc), tuple(expected))
                    self.assertEqual(bytes(uc.mem_read(SOURCE, 1)), bytes([value]))
                    self.assertEqual(bytes(uc.mem_read(DEST, 1)), bytes([result]))

    def test_gm_original_loop_terminates_and_preserves_source(self):
        for label, payload in cases(100).items():
            with self.subTest(case=label):
                uc = self.machine('gm-notice-filter')
                # Original NUL-terminated copy has no newly imposed scan cap.
                message = payload if b'\0' in payload else payload + b'\0'
                original = self.buffer(uc, SOURCE, message)
                self.buffer(uc, DEST, b'\xcc' * 128)
                uc.reg_write(UC_X86_REG_EAX, SOURCE)
                uc.reg_write(UC_X86_REG_EDX, DEST - SOURCE)
                uc.reg_write(UC_X86_REG_ECX, 0x123456ab)
                uc.reg_write(UC_X86_REG_EBP, SOURCE)
                uc.mem_write(SP, struct.pack('<II', 0x87654321, 0x12345678))
                self.execute(uc, 0x471ca8, 0x471cb4)
                copied = message.split(b'\0', 1)[0].replace(b"'", b' ') + b'\0'
                self.check(uc, SOURCE, original)
                self.check(uc, DEST, copied + b'\xcc' * (128 - len(copied)))
                self.assertEqual(uc.reg_read(UC_X86_REG_EAX), SOURCE + len(copied))
                self.assertEqual(uc.reg_read(UC_X86_REG_ECX), 0x12345600)
                self.assertEqual(uc.reg_read(UC_X86_REG_ESP), SP)
                self.assertTrue(uc.reg_read(UC_X86_REG_EFLAGS) & 0x40)
                # Continue through the original strlen and packet-size push.
                # Quotes become spaces of identical length; SendLog must still
                # receive precisely the original text length plus its header.
                self.execute(uc, 0x471cb4, 0x471ccf)
                packet_size = len(copied) - 1 + 0x74
                self.assertEqual(uc.reg_read(UC_X86_REG_EAX), packet_size)
                self.assertEqual(uc.reg_read(UC_X86_REG_ESI), 0x87654321)
                self.assertEqual(uc.reg_read(UC_X86_REG_EBP), 0x12345678)
                self.assertEqual(uc.reg_read(UC_X86_REG_ESP), SP + 4)
                self.assertEqual(struct.unpack('<I', uc.mem_read(SP + 4, 4))[0], packet_size)
                self.check(uc, SOURCE, original)


if __name__ == '__main__':
    unittest.main(verbosity=2)
