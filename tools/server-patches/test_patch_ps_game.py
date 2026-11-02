#!/usr/bin/env python3
"""Tests for the server patch kit. Each fix is applied to a copy, its bytes are
checked, and a revert must give back the byte-for-byte original.

The tests need a copy of the exact ps_game.exe build (they never modify it in place
and never run it). Point PS_GAME_EXE at it, or put it next to this file;
without it the tests skip. Run: python3 -m pytest test_patch_ps_game.py
or: python3 test_patch_ps_game.py
"""
import hashlib
import importlib.util
import os
import shutil
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))


def rb(path):
    with open(path, "rb") as fh:
        return fh.read()
spec = importlib.util.spec_from_file_location("patch", os.path.join(HERE, "patch_ps_game.py"))
patch = importlib.util.module_from_spec(spec)
spec.loader.exec_module(patch)
DOC = patch.load()
SHA = DOC["target"]["sha256"]

SEARCH = [
    os.environ.get("PS_GAME_EXE", ""),
    os.path.join(HERE, "ps_game.exe"),
]


def find_exe():
    for p in SEARCH:
        if p and os.path.exists(p) and os.path.getsize(p) == DOC["target"]["size"]:
            if hashlib.sha256(open(p, "rb").read()).hexdigest() == SHA:
                return p
    return None


EXE = find_exe()


@unittest.skipIf(EXE is None, "ps_game.exe (the expected build) not found; set PS_GAME_EXE")
class PatchKit(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.mkdtemp()
        self.exe = os.path.join(self.dir, "ps_game.exe")
        shutil.copy2(EXE, self.exe)
        self.original = rb(self.exe)

    def tearDown(self):
        shutil.rmtree(self.dir)

    def run_tool(self, *args):
        patch.main.__globals__  # noqa: keep import used
        import sys
        argv = sys.argv
        sys.argv = ["patch_ps_game.py", *args]
        try:
            patch.main()
        finally:
            sys.argv = argv

    def test_sha_guard_rejects_tampered(self):
        data = bytearray(self.original)
        data[0x1000] ^= 0xFF  # an edit the kit does not know about
        self.assertIsNone(patch.pristine(DOC, data))

    def test_each_fix_applies_and_reverts(self):
        for fix in DOC["fixes"]:
            shutil.copy2(EXE, self.exe)
            self.run_tool("apply", self.exe, fix["id"])
            data = rb(self.exe)
            for p in fix["inplace"]:
                off, pb = p["off"], bytes.fromhex(p["patched"])
                self.assertEqual(data[off:off + len(pb)], pb,
                                 "%s not applied at 0x%x" % (fix["id"], p["va"]))
            self.assertEqual(patch.state(bytearray(data), fix), "applied", fix["id"])
            self.run_tool("revert", self.exe, fix["id"])
            self.assertEqual(rb(self.exe), self.original,
                             "%s revert did not restore the original" % fix["id"])

    def test_default_apply_then_revert_all_round_trips(self):
        self.run_tool("apply", self.exe)
        after = rb(self.exe)
        self.assertNotEqual(after, self.original)
        # the default set is exactly the clear-bug fixes
        data = bytearray(after)
        for fix in DOC["fixes"]:
            want = "applied" if fix["default"] else "clean"
            self.assertEqual(patch.state(data, fix), want, fix["id"])
        self.run_tool("revert", self.exe, "all")
        self.assertEqual(rb(self.exe), self.original)

    def test_input_filters_compose_with_existing_fixes(self):
        filters = {"character-name-filter", "guild-text-filter",
                   "gift-target-filter", "gm-notice-filter"}
        existing = [f["id"] for f in DOC["fixes"] if f["id"] not in filters]
        # Models an executable carrying the previously distributed kit, including
        # opt-in patches. New caves must not change any of those patch ranges.
        self.run_tool("apply", self.exe, *existing)
        previous = rb(self.exe)
        self.assertEqual(patch.pristine(DOC, previous), self.original)
        for fix_id in sorted(filters):
            self.run_tool("apply", self.exe, fix_id)
        for fix in DOC["fixes"]:
            self.assertEqual(patch.state(rb(self.exe), fix), "applied", fix["id"])
        for fix_id in reversed(sorted(filters)):
            self.run_tool("revert", self.exe, fix_id)
        self.assertEqual(rb(self.exe), previous)
        self.run_tool("revert", self.exe, "all")
        self.assertEqual(rb(self.exe), self.original)
        self.assertEqual(rb(self.exe + ".bak"), self.original)

    def test_apply_is_idempotent(self):
        self.run_tool("apply", self.exe)
        once = rb(self.exe)
        self.run_tool("apply", self.exe)
        self.assertEqual(rb(self.exe), once)

    def test_header_tracks_cave_fixes(self):
        # a cave fix raises VirtualSize; reverting the last cave fix restores it
        h = DOC["header"]
        self.run_tool("apply", self.exe, "nan-pos")  # a cave fix
        data = rb(self.exe)
        self.assertEqual(data[h["off"]:h["off"] + 4], bytes.fromhex(h["patched"]))
        self.run_tool("revert", self.exe, "nan-pos")
        data = rb(self.exe)
        self.assertEqual(data[h["off"]:h["off"] + 4], bytes.fromhex(h["original"]))

    def test_every_patch_original_matches_build(self):
        for fix in DOC["fixes"]:
            for p in fix["inplace"]:
                off, ob = p["off"], bytes.fromhex(p["original"])
                self.assertEqual(self.original[off:off + len(ob)], ob,
                                 "%s original bytes drift at 0x%x" % (fix["id"], p["va"]))


if __name__ == "__main__":
    if EXE is None:
        print("ps_game.exe not found; set PS_GAME_EXE to the expected build. Skipping.")
    unittest.main(verbosity=2)
