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


if __name__ == "__main__":
    unittest.main()
