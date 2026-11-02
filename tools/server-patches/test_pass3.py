#!/usr/bin/env python3
"""Pass-3 fixes: structural checks and Unicorn emulation; never starts the server.

Needs the exact original executable (located by test_patch_ps_game/PS_GAME_EXE),
Unicorn for the emulation tests and objdump for the branch-target test. These
tests prove local instruction behaviour, not live server behaviour.
"""
import contextlib
import io
import re
import shutil
import struct
import subprocess
import tempfile
import unittest
import test_pass2 as t2
from test_patch_ps_game import DOC, EXE, patch, rb
try:
    from unicorn.x86_const import (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX,
        UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP,
        UC_X86_REG_ESP, UC_X86_REG_EFLAGS)
    from unicorn import UC_HOOK_CODE
    HAVE_UNICORN = True
except ImportError:
    HAVE_UNICORN = False

PREVIOUS = t2.PASS1 + t2.IDS
IDS = ('party-kick-leader', 'repair-shield-slot')
FIXES = t2.FIXES
SP, OBJ, OBJ2, OBJ3 = t2.SP, t2.OBJ, t2.OBJ2, t2.OBJ3
ZF = 0x40


@unittest.skipIf(EXE is None, 'expected ps_game.exe not found; set PS_GAME_EXE')
class Pass3Structure(unittest.TestCase):
    def test_new_fixes_follow_the_old_ones(self):
        ids = [f['id'] for f in DOC['fixes']]
        self.assertEqual(tuple(ids[:len(PREVIOUS)]), PREVIOUS)
        self.assertEqual(tuple(ids[len(PREVIOUS):]), IDS)

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

    def test_composes_with_previous_kit(self):
        original = rb(EXE)
        with tempfile.TemporaryDirectory() as d:
            target = d + '/ps_game.exe'
            shutil.copy2(EXE, target)
            with contextlib.redirect_stdout(io.StringIO()):
                patch.cmd_apply(DOC, target, list(PREVIOUS))
                previous = rb(target)
                patch.cmd_apply(DOC, target, list(IDS))
                for f in DOC['fixes']:
                    self.assertEqual(patch.state(rb(target), f), 'applied', f['id'])
                self.assertEqual(bytes(patch.pristine(DOC, rb(target))), original)
                patch.cmd_revert(DOC, target, list(IDS))
                self.assertEqual(rb(target), previous)
                patch.cmd_revert(DOC, target, [])
            self.assertEqual(rb(target), original)

    def test_each_new_fix_alone_round_trips(self):
        original = rb(EXE)
        with tempfile.TemporaryDirectory() as d:
            target = d + '/ps_game.exe'
            for fid in IDS:
                shutil.copy2(EXE, target)
                with contextlib.redirect_stdout(io.StringIO()):
                    patch.cmd_apply(DOC, target, [fid])
                    self.assertEqual(patch.state(rb(target), FIXES[fid]), 'applied')
                    for f in DOC['fixes']:
                        if f['id'] != fid:
                            self.assertEqual(patch.state(rb(target), f), 'clean', f['id'])
                    patch.cmd_revert(DOC, target, [fid])
                self.assertEqual(rb(target), original, fid)


@unittest.skipUnless(HAVE_UNICORN and EXE, 'requires Unicorn and expected ps_game.exe')
class Pass3Emulation(unittest.TestCase):
    setUpClass = classmethod(t2.Pass2Emulation.setUpClass.__func__)
    machine = t2.Pass2Emulation.machine
    state = t2.Pass2Emulation.state
    go = t2.Pass2Emulation.go
    put = t2.Pass2Emulation.put
    get = t2.Pass2Emulation.get

    def u32(self, uc, address):
        return struct.unpack('<I', self.get(uc, address, 4))[0]

    # --- party ------------------------------------------------------------------
    def test_kick_refuses_the_leader(self):
        for count in (2, 3, 5):
            for slot in range(count):
                for leader in range(count):
                    with self.subTest(count=count, slot=slot, leader=leader):
                        old, new = self.machine(), self.machine('party-kick-leader')
                        for uc in (old, new):
                            self.put(uc, OBJ + 0xc, struct.pack('<I', leader))
                            self.put(uc, OBJ + 0xdc, struct.pack('<I', 0x5a5a0001))
                            uc.reg_write(UC_X86_REG_EBP, OBJ)
                            uc.reg_write(UC_X86_REG_EDI, OBJ)
                            uc.reg_write(UC_X86_REG_EBX, slot)
                        end = self.go(new, 0x44718d, [0x447193, 0x44716f])
                        oend = self.go(old, 0x44718d, [0x447193])
                        self.assertEqual(oend, 0x447193)
                        if slot == leader:
                            self.assertEqual(end, 0x44716f)
                        else:
                            self.assertEqual(end, 0x447193)
                            # flags are dead after the hook (next use: a call)
                            self.assertEqual(self.state(new)[:8], self.state(old)[:8])
                        self.assertEqual(new.reg_read(UC_X86_REG_ESP), SP)

    def test_kick_adjusts_the_leader_and_the_turn(self):
        for count in (2, 3, 4, 6):
            for slot in range(count):
                for leader in range(count):
                    if leader == slot:
                        continue                      # refused before this point
                    for turn in range(count + 1):
                        with self.subTest(count=count, slot=slot, leader=leader, turn=turn):
                            old, new = self.machine(), self.machine('party-kick-leader')
                            for uc in (old, new):
                                self.put(uc, OBJ + 0xc, struct.pack('<I', leader))
                                self.put(uc, OBJ + 0x10, struct.pack('<I', count))
                                self.put(uc, OBJ + 0x108, struct.pack('<I', turn))
                                uc.reg_write(UC_X86_REG_EBP, OBJ)
                                uc.reg_write(UC_X86_REG_EBX, slot + 0)
                            self.assertEqual(self.go(new, 0x4471d4, [0x4471db]), 0x4471db)
                            self.assertEqual(self.go(old, 0x4471d4, [0x4471db]), 0x4471db)
                            # what LeaveNormalParty does with the same slot
                            want_leader = leader - 1 if slot < leader else leader
                            want_turn = turn - 1 if turn > slot + 1 else turn
                            self.assertEqual(self.u32(new, OBJ + 0xc), want_leader)
                            self.assertEqual(self.u32(new, OBJ + 0x108), want_turn)
                            self.assertEqual(self.u32(new, OBJ + 0x10), count - 1)
                            self.assertEqual(self.u32(old, OBJ + 0xc), leader)
                            self.assertEqual(self.u32(old, OBJ + 0x108), turn)
                            self.assertEqual(new.reg_read(UC_X86_REG_EAX), count - 1)
                            self.assertEqual(old.reg_read(UC_X86_REG_EAX), count - 1)
                            self.assertEqual(bool(new.reg_read(UC_X86_REG_EFLAGS) & ZF),
                                             count == 1)
                            self.assertEqual(new.reg_read(UC_X86_REG_EFLAGS),
                                             old.reg_read(UC_X86_REG_EFLAGS))
                            for r in (UC_X86_REG_EBX, UC_X86_REG_EDX, UC_X86_REG_ESI,
                                      UC_X86_REG_EDI, UC_X86_REG_EBP, UC_X86_REG_ESP):
                                self.assertEqual(new.reg_read(r), old.reg_read(r))

    # --- repair -----------------------------------------------------------------
    def test_repair_saves_the_repaired_slot(self):
        for bag in (0, 1, 3):
            for slot in (0, 3, 6, 9, 12) if bag == 0 else (0, 6, 23):
                for quality in (0, 1, 777, 0xffff):
                    with self.subTest(bag=bag, slot=slot, quality=quality):
                        old, new = self.machine(), self.machine('repair-shield-slot')
                        for uc in (old, new):
                            self.put(uc, OBJ2 + 0x1aa, bytes(range(1, 0x41)))
                            self.put(uc, SP + 0xdc, struct.pack('<I', slot))
                            uc.reg_write(UC_X86_REG_ESI, OBJ)
                            uc.reg_write(UC_X86_REG_EBP, OBJ2)
                            uc.reg_write(UC_X86_REG_EBX, bag)
                            uc.reg_write(UC_X86_REG_EDX, 6)
                            uc.reg_write(UC_X86_REG_EDI, quality)
                        self.assertEqual(self.go(new, 0x4649c7, [0x4649db]), 0x4649db)
                        self.assertEqual(self.go(old, 0x4649c7, [0x4649db]), 0x4649db)
                        before = bytes(range(1, 0x41))
                        for uc in (old, new):           # the item's quality is set either way
                            self.assertEqual(self.get(uc, OBJ + 0x44, 2),
                                             struct.pack('<H', quality))
                        self.assertEqual(self.get(old, OBJ2 + 0x1aa, 0x40)[12:14],
                                         struct.pack('<H', quality))       # old: slot 6
                        copy = bytearray(before)
                        if bag == 0:
                            copy[slot * 2:slot * 2 + 2] = struct.pack('<H', quality)
                        self.assertEqual(self.get(new, OBJ2 + 0x1aa, 0x40), bytes(copy))
                        self.assertEqual(bool(new.reg_read(UC_X86_REG_EFLAGS) & ZF), bag == 0)
                        self.assertEqual(bool(old.reg_read(UC_X86_REG_EFLAGS) & ZF), bag == 0)
                        for r in (UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX,
                                  UC_X86_REG_ESI, UC_X86_REG_EDI, UC_X86_REG_EBP,
                                  UC_X86_REG_ESP):
                            self.assertEqual(new.reg_read(r), old.reg_read(r))


if __name__ == '__main__':
    unittest.main(verbosity=2)
