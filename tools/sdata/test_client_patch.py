#!/usr/bin/env python3
"""Tests of the pure parts of client_patch.py: python3 tools/sdata/test_client_patch.py (no database needed)."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import client_patch

NAMES = ["rowid", "itemid", "itemname", "grade", "drop", "updatedate"]


class OnlyGradeDiffers(unittest.TestCase):
    def test_a_grade_alone_passes(self):
        self.assertTrue(client_patch.only_grade_differs(NAMES, ["1", "30071", "Sword", "5", "0", None], ["1", "30071", "Sword", "77", "0", None]))

    def test_the_update_date_never_counts(self):
        self.assertTrue(client_patch.only_grade_differs(NAMES, ["1", "30071", "Sword", "5", "0", "a"], ["1", "30071", "Sword", "6", "0", "b"]))

    def test_another_column_as_well_is_still_forgotten(self):
        self.assertFalse(client_patch.only_grade_differs(NAMES, ["1", "30071", "Sword", "5", "0", None], ["1", "30071", "Sword", "77", "1", None]))
        self.assertFalse(client_patch.only_grade_differs(NAMES, ["1", "30071", "Sword", "5", "0", None], ["1", "30071", "Axe", "5", "0", None]))

    def test_no_difference_at_all_is_not_a_grade_edit(self):
        self.assertFalse(client_patch.only_grade_differs(NAMES, ["1", "30071", "Sword", "5", "0", None], ["1", "30071", "Sword", "5", "0", None]))


class ResolvingClientFields(unittest.TestCase):
    """docs/plans/ext-1-foundation.md section 4.1: a field given by name, else the look-alike's record's, else the record's own."""

    OWN = ("Own name", "own text", 1, 2, 3, 4)
    LOOK = ("Look name", "look text", 11, 12, 13, 14)

    def item(self, keep=False, name="N", description="D", fields=(None, None, None, None), own=OWN, look=LOOK):
        return client_patch.resolve_item_row((100, 212, keep, name, description, *fields, None, "note"), own, look)

    def test_by_name_wins_then_the_look_alike_then_the_own_record(self):
        row = self.item(fields=(7, None, None, None))
        self.assertEqual(row[4:8], (7, 12, 13, 14), "model by name, the rest from the look-alike")
        self.assertEqual(self.item(look=None)[4:8], (1, 2, 3, 4), "no look-alike: the record's own")
        self.assertEqual(self.item(fields=(0, 0, 0, 0))[4:8], (0, 0, 0, 0), "a field given as 0 is given")

    def test_keep_takes_the_text_from_the_own_record(self):
        row = self.item(keep=True, name=None, description=None)
        self.assertEqual(row[2:4], ("Own name", "own text"))
        self.assertEqual(self.item()[2:4], ("N", "D"))

    def test_skills_resolve_the_same_way(self):
        own, look = ("Own", "text", 1, 2, 3, 4), ("Look", "text", 21, 22, 23, 24)
        row = client_patch.resolve_skill_row((200, 1, False, "Kill Boost", "d", None, 5, None, None, 109, "n"), own, look)
        self.assertEqual(row, (200, 1, "Kill Boost", "d", 21, 5, 23, 24, 109, "n"))

    def test_nothing_anywhere_stays_unresolved(self):
        self.assertEqual(self.item(own=None, look=None)[4:8], (None, None, None, None))


class RowsOfAPatch(unittest.TestCase):
    ROWS = {(100, 212): (0, 0, 0, 0, 0, 0, 0, 0, 109, "a"), (100, 213): (0, 0, 0, 0, 0, 0, 0, 0, None, "b"), (100, 214): (0, 0, 0, 0, 0, 0, 0, 0, 110, "c")}

    def test_rebuild_takes_the_rows_of_patches_up_to_n(self):
        self.assertEqual(sorted(client_patch.patch_limit(self.ROWS, 109)), [(100, 212)])
        self.assertEqual(sorted(client_patch.patch_limit(self.ROWS, 110)), [(100, 212), (100, 214)], "a row without a patch belongs to none")
        self.assertEqual(sorted(client_patch.patch_limit(self.ROWS, None)), sorted(self.ROWS), "generate and check take all")
        self.assertEqual(client_patch.patch_limit(self.ROWS, 108), {})


class PreviousVersionAndManifest(unittest.TestCase):
    def test_the_previous_version_is_the_newest_earlier_patch_that_carried_the_member(self):
        carried = {109: {"item", "skill"}, 110: {"item"}, 111: {"item"}}
        numbers = [111, 110, 109]  # the ledger below 112, newest first
        self.assertEqual(client_patch.newest_carrier(numbers, lambda n: "item" in carried[n]), 111)
        self.assertEqual(client_patch.newest_carrier(numbers, lambda n: "skill" in carried[n]), 109, "the skill file last changed in 109")
        self.assertIsNone(client_patch.newest_carrier([], lambda n: True), "no earlier patch: the restored original (108)")
        self.assertIsNone(client_patch.newest_carrier(numbers, lambda n: False))

    def test_the_manifest_names_the_commit_and_the_extensions(self):
        files = [{"member": "Item/Item.SData", "size": 1, "sha256": "a", "changed": ["100/212"], "_encrypted": b"x"}]
        m = client_patch.build_manifest(110, 109, "abc123", {"kill-boost": "1.0.0"}, False, files)
        self.assertEqual(list(m), ["patch", "previous", "commit", "extensions", "dirty", "files"])
        self.assertEqual((m["commit"], m["extensions"], m["dirty"]), ("abc123", {"kill-boost": "1.0.0"}, False))
        self.assertNotIn("newshaiya_commit", m)
        self.assertNotIn("_encrypted", m["files"][0], "the encrypted bytes are files of the folder, not manifest fields")


if __name__ == "__main__":
    unittest.main()
