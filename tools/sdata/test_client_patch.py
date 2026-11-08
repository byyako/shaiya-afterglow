#!/usr/bin/env python3
"""Tests of the pure parts of client_patch.py: python3 tools/sdata/test_client_patch.py (no database needed)."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import client_patch
import client_sdata
import load_client
import seed_sdata

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


class MonsterFile(unittest.TestCase):
    """Monster.SData (docs/plans/ext-4-pets.md section 6.3): the writer is the reader's inverse."""

    @staticmethod
    def record(mob_id, name):
        rec = {"mob_id": mob_id, "name": name}
        rec.update({n: (mob_id * 7 + i) % 100 for i, (n, _) in enumerate(client_sdata.MONSTER_FIELDS)})
        rec["hp"] = 100000 + mob_id
        rec["model"] = -1 if mob_id == 2 else mob_id
        return rec

    def test_write_monsters_parses_back_to_the_same_records(self):
        records = [self.record(0, b"Error Monster"), self.record(1, b"Small Ruined Wolf"), self.record(2, b"")]
        data = client_patch.write_monsters(records)
        parsed, end, rebuilt, _ = client_sdata.read_monsters(data)
        self.assertEqual((end, rebuilt), (len(data), data))
        self.assertEqual([r["name"] for r in parsed], [b"Error Monster\0", b"Small Ruined Wolf\0", b"\0"], "each name carries its NUL in the file")
        self.assertEqual([{k: v for k, v in r.items() if k != "name"} for r in parsed],
                         [{k: v for k, v in r.items() if k != "name"} for r in records])

    def test_the_shipped_file_rebuilds_from_its_rows_when_the_originals_are_there(self):
        path = load_client.ORIGINALS / "Monster.SData"
        if not path.exists() or not load_client.SEED_CONSTANTS.exists():
            self.skipTest("the client originals are not in this checkout")
        plain = seed_sdata.decrypt(path.read_bytes(), seed_sdata.load_consts(load_client.SEED_CONSTANTS))
        self.assertEqual(client_patch.sha(plain), load_client.PLAIN_SHA256["Monster.SData"])
        rows = load_client.monster_rows(plain)
        self.assertEqual(len(rows), load_client.MONSTER_RECORDS)
        records = [dict(zip(load_client.monster_columns(), row)) for row in rows]
        for r in records:
            r["name"] = r["name"].encode(load_client.ENCODING)
        self.assertEqual(client_patch.write_monsters(records), plain)
        self.assertEqual(client_patch.sha(client_patch.encrypt(plain)), load_client.SHA256["Monster.SData"])


class MonsterGeneration(unittest.TestCase):
    """docs/plans/ext-4-pets.md section 6.3: fillers, copies with a day map and fields, the width and name checks, the record count."""

    @staticmethod
    def originals(count=client_patch.MONSTER_ORIGINALS):
        out = []
        for mob_id in range(count):
            rec = {"mob_id": mob_id, "name": f"Beast {mob_id}".encode(), **{n: 1 for n in client_patch.monster_names()}}
            rec.update(model=10 + mob_id % 50, day=mob_id % 8, size=1 + mob_id % 3, hp=500 + mob_id)
            out.append(rec)
        return out

    PET = {"day": {"0": 6, "1": 6, "2": 0, "3": 0, "4": 1, "5": 4, "6": 6, "7": 6}, "size": 0, "hp": 1, "quest_item": 0,
           "normal_time": 1000, "normal_step": 4, "chase_time": 1000, "chase_step": 10}

    def generate(self, rows, count=client_patch.MONSTER_ORIGINALS):
        return client_patch.generate_monsters(self.originals(count), rows, 29)

    def test_fillers_come_between_the_originals_and_the_first_row(self):
        records, plain = self.generate({3001: (5, {"size": 0}, None, "n")})
        self.assertEqual(len(records), 3002)
        self.assertEqual(records[client_patch.MONSTER_ORIGINALS:3001], [client_patch.monster_filler(i) for i in range(client_patch.MONSTER_ORIGINALS, 3001)])
        filler = records[2474]
        self.assertEqual((filler["name"], filler["model"], filler["hp"], filler["day"], filler["size"]), (b"", -1, 0, 0, 0))
        parsed = client_patch.parse_monsters(plain)
        self.assertEqual(len(parsed), 3002, "the file's count is the highest id plus one")
        self.assertEqual(parsed[:client_patch.MONSTER_ORIGINALS], client_patch.parse_monsters(client_patch.write_monsters(self.originals())))

    def test_a_copy_keeps_the_originals_name_and_model_and_takes_the_rows_fields(self):
        records, _ = self.generate({3000 + m: (m, self.PET, None, "n") for m in (1, 2, 3, 4, 5, 6, 7)})
        for m in range(1, 8):
            wild, pet = records[m], records[3000 + m]
            self.assertEqual((pet["mob_id"], pet["name"], pet["model"], pet["level"]), (3000 + m, wild["name"], wild["model"], wild["level"]))
            self.assertEqual((pet["size"], pet["hp"], pet["normal_time"], pet["normal_step"], pet["chase_step"]), (0, 1, 1000, 4, 10))
        self.assertEqual([records[3000 + m]["day"] for m in range(0, 8) if m], [6, 0, 0, 1, 4, 6, 6], "the day by the copied record's day")
        self.assertEqual(records[1], self.originals()[1], "the original stays as it was")

    def test_a_day_the_map_does_not_name_is_kept_and_a_plain_number_sets_it(self):
        records, _ = self.generate({3001: (1, {"day": {"7": 6}}, None, "n"), 3002: (2, {"day": 3}, None, "n")})
        self.assertEqual((records[3001]["day"], records[3002]["day"]), (self.originals()[1]["day"], 3))

    def test_the_name_can_be_set(self):
        records, _ = self.generate({3001: (1, {"name": "Small Wolf"}, None, "n")})
        self.assertEqual(records[3001]["name"], b"Small Wolf")

    def test_checks(self):
        for row, needle in [
            ((1, {"size": 256}, None, "n"), "does not fit"),
            ((1, {"hp": -5000000000}, None, "n"), "does not fit"),
            ((1, {"level": 40000}, None, "n"), "does not fit"),
            ((1, {"size": -1}, None, "n"), "does not fit"),
            ((1, {"name": "x" * 30}, None, "n"), "longer than the longest"),
            ((1, {"name": "Naïve"}, None, "n"), "not printable ASCII"),
            ((1, {"colour": 1}, None, "n"), "not a field of Monster.SData"),
            ((1, {"size": "big"}, None, "n"), "not a whole number"),
            ((1, {"size": True}, None, "n"), "not a whole number"),
            ((2474, {}, None, "n"), "not a record of the original"),
            ((-1, {}, None, "n"), "not a record of the original"),
        ]:
            with self.assertRaises(client_patch.CheckError, msg=str(row)) as raised:
                self.generate({3001: row})
            self.assertIn(needle, str(raised.exception))
        for mob_id in (100, 2473, 65536):
            with self.assertRaises(client_patch.CheckError) as raised:
                self.generate({mob_id: (1, {}, None, "n")})
            self.assertIn("an added record's id is", str(raised.exception))
        with self.assertRaises(client_patch.CheckError):
            self.generate({3001: (1, {}, None, "n")}, count=100)

    def test_changes_list_each_added_record_as_one_line(self):
        old = client_patch.parse_monsters(client_patch.write_monsters(self.originals(3)))
        new_records = [dict(r) for r in old] + [client_patch.monster_filler(3), client_patch.monster_filler(4), {**old[1], "mob_id": 5, "size": 0}]
        changes = client_patch.monster_changes(old, new_records)
        self.assertEqual([c[0] for c in changes], ["5"], "a filler is not listed")
        self.assertIn("added", changes[0][3][0][2])
        new_records[1] = {**new_records[1], "hp": 9}
        self.assertEqual([c[0] for c in client_patch.monster_changes(old, new_records)], ["1", "5"])

    def test_monster_rows_of_a_patch(self):
        rows = {3001: (1, {}, 111, "a"), 3002: (2, {}, None, "b"), 3003: (3, {}, 109, "c")}
        self.assertEqual(sorted(client_patch.monster_patch_limit(rows, 110)), [3003])
        self.assertEqual(sorted(client_patch.monster_patch_limit(rows, None)), sorted(rows))


if __name__ == "__main__":
    unittest.main()
