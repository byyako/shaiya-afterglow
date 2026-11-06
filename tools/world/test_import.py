#!/usr/bin/env python3
"""Tests of the pure parts of import.py and of content_import.decide: python3 tools/world/test_import.py (no database needed)."""
import importlib.util
import io
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
import content_import  # noqa: E402

spec = importlib.util.spec_from_file_location("world_import", HERE / "import.py")
world_import = importlib.util.module_from_spec(spec)
spec.loader.exec_module(world_import)

# The layout of the original server patch's file: Korean comments (CP949), a tab before each '=', tabs or spaces after some values.
PATCH_LAYOUT = """;---------------------------------------------------------------------------
;라피시안 조합확률----------------------------------------------------------

[LapisianEnchantPercentRate]
;무기(00 : 0단계에서 1단계로의 조합확률
WeaponStep00\t=900000\t
WeaponStep01\t=800000\t\t
DefenseStep00\t=990000
DefenseStep01\t=980000 \n
[LapisianEnchantAddValue]
;무기(00 : 0단계에서의 추가치)
WeaponStep00\t=0\t
WeaponStep01\t=7\t
defensestep01\t=5 \n
WeaponStep01\t=99
[lapisianenchantaddvalue]
WeaponStep02\t=14
"""


def write(text):
    folder = Path(tempfile.mkdtemp())
    path = folder / "ItemEnchant.ini"
    path.write_bytes(text.encode("cp949"))
    return path


class ItemEnchant(unittest.TestCase):
    def test_the_patch_layout(self):
        rows = world_import.read_item_enchant(write(PATCH_LAYOUT))
        self.assertEqual(len(rows), 21, "a row for every step 0 to 20")
        self.assertEqual(rows[0], [0, 900000, 990000, 0, 0])
        # Tabs and spaces after the values are ignored; names are looked up whatever their case; the first key of a name wins.
        self.assertEqual(rows[1], [1, 800000, 980000, 7, 5])

    def test_missing_keys_and_a_second_section_of_a_name_are_0(self):
        rows = world_import.read_item_enchant(write(PATCH_LAYOUT))
        # WeaponStep02 is only in the second [LapisianEnchantAddValue], which GetPrivateProfileIntA never reaches.
        self.assertEqual(rows[2], [2, 0, 0, 0, 0])
        self.assertEqual(rows[20], [20, 0, 0, 0, 0])

    def test_the_leading_integer(self):
        rows = world_import.read_item_enchant(write("[LapisianEnchantAddValue]\nWeaponStep03=31 attack\nDefenseStep03=x\n"))
        self.assertEqual(rows[3][3:], [31, 0])

    def test_no_file_no_rows(self):
        self.assertEqual(world_import.read_item_enchant(Path(tempfile.mkdtemp()) / "ItemEnchant.ini"), [])

    def test_values_the_original_would_wrap_are_refused(self):
        with self.assertRaises(SystemExit):
            world_import.read_item_enchant(write("[LapisianEnchantPercentRate]\nWeaponStep05=-1\n"))
        with self.assertRaises(SystemExit):
            world_import.read_item_enchant(write("[LapisianEnchantAddValue]\nDefenseStep05=40000\n"))


class Decide(unittest.TestCase):
    """content_import.decide against a recorded import, without a database."""

    def setUp(self):
        self.saved = content_import.import_record
        content_import.import_record = lambda part: {"imported_at": "then", "source": "here", "files": {"Map.ini": "a"}, "due": False}

    def tearDown(self):
        content_import.import_record = self.saved

    def test_a_file_never_read_is_noted_and_the_copy_kept(self):
        out = io.StringIO()
        with redirect_stdout(out):
            self.assertEqual(content_import.decide("world", {"Map.ini": "a", "ItemEnchant.ini": "b"}, "world"), "skip")
        self.assertIn("ItemEnchant.ini not read at the import", out.getvalue())

    def test_a_changed_file_is_still_refused(self):
        with self.assertRaises(SystemExit):
            content_import.decide("world", {"Map.ini": "z", "ItemEnchant.ini": "b"}, "world")

    def test_a_file_gone_is_still_refused(self):
        with self.assertRaises(SystemExit):
            content_import.decide("world", {}, "world")


if __name__ == "__main__":
    unittest.main()
