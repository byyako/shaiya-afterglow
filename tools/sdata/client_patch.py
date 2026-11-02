#!/usr/bin/env python3
"""Generate the client's Item.SData and Skill.SData from the database, check them, and write a client data patch's hand-over folder
(docs/plans/done/client-patches.md).

usage: client_patch.py restore <out dir>
       client_patch.py check
       client_patch.py generate --patch N [--dirty]

Every command first loads tools/sdata/client_rows.sql into the `client` schema (tools/sdata/load_client.py has imported the shipped
originals there). A generated record is the client's original (client.items / client.skills) unless it has a row in client.item_rows /
client.skill_rows; then it is the server's fields (defs.items / defs.skills) with the row's client-only fields.

  restore   writes the decrypted and encrypted files from client.items / client.skills alone and checks that they are the shipped ones.
  check     the eight checks of section 3 of the plan, one line each; exit 1 on the first error.
  generate  runs the checks, then writes data/client-patches/psNNNN/ (the encrypted files that changed, manifest.json, changes.txt).
"""
import argparse
import csv
import hashlib
import io
import json
import struct
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "defs"))
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import client_sdata
import dbconn
import load_client
import load_into_postgres
import parse_item
import seed_sdata

ROOT = Path(__file__).resolve().parents[2]
ROWS_SQL = ROOT / "tools/sdata/client_rows.sql"
DEFS_EXPORT = ROOT / "data/defs-export"
DEFS_SCHEMA = ROOT / "db/defs/schema.sql"
PATCHES = ROOT / "data/client-patches"
PREVIOUS_VERSION = 108  # the originals' patch

ITEM_CLIENT_ONLY = ("model", "icon", "type2", "type3")
SKILL_CLIENT_ONLY = ("icon", "animation", "toggle_type", "sound")
SKILL_COLUMN_NAMES = {"level": "skilllevel", "prev_skill": "prevskillid", "defense_type": "defencetype", "defense_value": "defencevalue"}
WIDTHS = {"B": (0, 255, 0xFF), "H": (-32768, 65535, 0xFFFF), "I": (0, 4294967295, 0xFFFFFFFF)}
MEMBERS = {
    "item": {"member": "Item/Item.SData", "us_path": "data/item/item.sdata", "original": "Item.SData"},
    "skill": {"member": "Character/Skill.SData", "us_path": "data/character/skill.sdata", "original": "Skill.SData"},
}
ITEM_GROUPS = 100
SKILLS = 326
SKILL_LEVELS = client_sdata.SKILL_LEVELS


class CheckError(Exception):
    pass


# ---------------------------------------------------------------------------------------------------------------- the database

def psql_run(sql=None, stdin=None):
    result = subprocess.run(dbconn.psql_cmd({"PGCLIENTENCODING": "UTF8"}) + (["-c", sql] if sql else []), input=stdin, capture_output=True)
    if result.returncode != 0:
        sys.exit(f"psql failed: {result.stderr.decode(errors='replace').strip()}")
    return result.stdout.decode("utf-8")


def load_rows_file():
    psql_run(stdin=ROWS_SQL.read_bytes())


def query(sql):
    """The rows of a SELECT as lists of strings (None for NULL)."""
    out = psql_run(f"COPY ({sql}) TO STDOUT WITH (FORMAT csv, NULL '\\N')")
    return [[None if v == "\\N" else v for v in row] for row in csv.reader(io.StringIO(out, newline=""))]


def table_columns(table):
    return [name for (name,) in query(
        f"SELECT column_name FROM information_schema.columns WHERE table_schema = 'defs' AND table_name = '{table}' ORDER BY ordinal_position")]


# ------------------------------------------------------------------------------------------------------------------- the files

def fix_names():
    return [n for n, _ in parse_item.FIX]


def skill_names():
    return [n for n, _ in client_sdata.SKILL_FIELDS]


def item_defs_column(name):
    return name  # every FIX name is a defs.items column but the client-only four


def skill_defs_column(name):
    return SKILL_COLUMN_NAMES.get(name, name.replace("_", ""))


def read_client_items():
    """client.items as groups of record dicts (name and description as cp1252 bytes without their NUL)."""
    rows = query(f"SELECT grp, idx, name, description, {', '.join(fix_names())} FROM client.items ORDER BY grp, idx")
    groups = []
    for grp, idx, name, description, *values in rows:
        if int(grp) < len(groups):
            raise CheckError(f"client.items: group {grp} out of order")
        while len(groups) < int(grp):  # a group without records has no rows
            groups.append([])
        if int(idx) != len(groups[-1]) + 1:
            raise CheckError(f"client.items {grp}/{idx}: not the next place in its group")
        rec = {"name": name.encode(load_client.ENCODING), "description": description.encode(load_client.ENCODING)}
        rec.update({n: int(v) for n, v in zip(fix_names(), values)})
        groups[-1].append(rec)
    groups += [[] for _ in range(ITEM_GROUPS - len(groups))]  # trailing empty groups
    return groups


def read_client_skills():
    """client.skills as a list of record dicts in file order (skill_id from 1, slot 1 to 3)."""
    rows = query(f"SELECT skill_id, slot, name, description, {', '.join(skill_names())} FROM client.skills ORDER BY skill_id, slot")
    records = []
    for n, (skill_id, slot, name, description, *values) in enumerate(rows):
        if (int(skill_id), int(slot)) != (n // SKILL_LEVELS + 1, n % SKILL_LEVELS + 1):
            raise CheckError(f"client.skills {skill_id}/{slot}: not the next place")
        rec = {"skill_id": int(skill_id), "slot": int(slot), "name": name.encode(load_client.ENCODING),
               "description": description.encode(load_client.ENCODING)}
        rec.update({k: int(v) for k, v in zip(skill_names(), values)})
        records.append(rec)
    return records


def write_skills(records):
    text = client_sdata.pack_text
    return struct.pack("<I", len(records) // SKILL_LEVELS) + b"".join(
        text(r["name"] + b"\0") + text(r["description"] + b"\0") + client_sdata.pack_fields(client_sdata.SKILL_FIELDS, r) for r in records)


def parse_skills(plain):
    """Skill.SData -> records as read_client_skills makes them; every byte must be explained."""
    parsed, end, rebuilt, _ = client_sdata.read_skills(plain)
    if end != len(plain) or rebuilt != plain:
        raise CheckError("Skill.SData: the records do not explain the whole file")
    records = []
    for n, r in enumerate(parsed):
        for key in ("name", "description"):
            if r[key][-1:] != b"\0" or b"\0" in r[key][:-1]:
                raise CheckError(f"Skill.SData record {n}: {key} is not text and one NUL")
        rec = {"skill_id": n // SKILL_LEVELS + 1, "slot": n % SKILL_LEVELS + 1, "name": r["name"][:-1], "description": r["description"][:-1]}
        rec.update({k: r[k] for k in skill_names()})
        records.append(rec)
    return records


def parse_items(plain):
    groups = parse_item.read_items(plain)
    if parse_item.write_items(groups) != plain:
        raise CheckError("Item.SData: the records do not rebuild the file")
    return groups


def encrypt(plain):
    return seed_sdata.encrypt(plain, seed_sdata.load_consts(load_client.SEED_CONSTANTS))


def sha(data):
    return hashlib.sha256(data).hexdigest()


# ------------------------------------------------------------------------------------------------------------------- restore

def restored():
    """{'item': (plain, encrypted), 'skill': (plain, encrypted)} from client.items / client.skills alone."""
    out = {"item": parse_item.write_items(read_client_items()), "skill": write_skills(read_client_skills())}
    return {kind: (plain, encrypt(plain)) for kind, plain in out.items()}


def check_restore(files):
    for kind, (plain, encrypted) in files.items():
        name = MEMBERS[kind]["original"]
        if sha(plain) != load_client.PLAIN_SHA256[name]:
            raise CheckError(f"{name}: rebuilt from client.* it is {sha(plain)}, the shipped file decrypts to {load_client.PLAIN_SHA256[name]}")
        if sha(encrypted) != load_client.SHA256[name]:
            raise CheckError(f"{name}: encrypted it is {sha(encrypted)}, the shipped file is {load_client.SHA256[name]}")


def cmd_restore(out_dir):
    files = restored()
    out_dir = Path(out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    for kind, (plain, encrypted) in files.items():
        name = MEMBERS[kind]["original"]
        (out_dir / f"{name}.decrypted").write_bytes(plain)
        (out_dir / name).write_bytes(encrypted)
        print(f"{name}: encrypted {sha(encrypted)}, decrypted {sha(plain)}")
    check_restore(files)
    print("restore: both files are the shipped ones, byte for byte")


# ------------------------------------------------------------------------------------------------------------------ the checks

def width_value(fmt, value, what):
    """The server's value as the client field holds it (section 3, check 5)."""
    low, high, mask = WIDTHS[fmt]
    if value is None or not low <= value <= high:
        raise CheckError(f"{what}: value {value} does not fit the client's {fmt} field ({low} to {high})")
    return value & mask


def printable(text, what, longest):
    try:
        raw = text.encode("ascii")
    except UnicodeEncodeError:
        raw = b"\0"
    if not all(0x20 <= b <= 0x7E for b in raw):
        raise CheckError(f"{what}: text {text!r} is not printable ASCII (0x20 to 0x7E)")
    if len(raw) > longest:
        raise CheckError(f"{what}: text of {len(raw)} bytes is longer than the longest of its kind in the original ({longest})")
    return raw


class Data:
    """Everything one run reads: the originals as shipped, client.*, the rows, and the server's fields for the listed records."""

    def __init__(self):
        load_rows_file()
        self.originals = {}
        for kind, info in MEMBERS.items():
            path = load_client.ORIGINALS / info["original"]
            if not path.exists():
                raise CheckError(f"{path}: the shipped original is missing")
            self.originals[kind] = load_client.original(path, info["original"])
        self.item_rows = {(int(r[0]), int(r[1])): r for r in query(
            "SELECT type, typeid, name, description, model, icon, type2, type3, since_patch, note FROM client.item_rows ORDER BY 1, 2")}
        self.skill_rows = {(int(r[0]), int(r[1])): r for r in query(
            "SELECT skill_id, level, name, description, icon, animation, toggle_type, sound, since_patch, note FROM client.skill_rows ORDER BY 1, 2")}
        self.server_only = {(r[0], int(r[1]), int(r[2])): r[3] for r in query("SELECT kind, id, level, reason FROM client.server_only")}
        longest = lambda sql: int(query(sql)[0][0])
        self.longest = {"item name": longest("SELECT max(octet_length(name)) FROM client.items"),
                        "item description": longest("SELECT max(octet_length(description)) FROM client.items"),
                        "skill name": longest("SELECT max(octet_length(name)) FROM client.skills"),
                        "skill description": longest("SELECT max(octet_length(description)) FROM client.skills")}
        self.item_columns, self.skill_columns = table_columns("items"), table_columns("skills")
        # A typo in a field name must not silently drop a field.
        for n in fix_names():
            if n not in ITEM_CLIENT_ONLY and item_defs_column(n) not in self.item_columns:
                raise CheckError(f"Item field {n!r}: no column {item_defs_column(n)!r} in defs.items")
        for n in skill_names():
            if n not in SKILL_CLIENT_ONLY and skill_defs_column(n) not in self.skill_columns:
                raise CheckError(f"Skill field {n!r}: no column {skill_defs_column(n)!r} in defs.skills")
        self.server_item_fields = [(n, f, item_defs_column(n)) for n, f in parse_item.FIX if n not in ITEM_CLIENT_ONLY]
        self.server_skill_fields = [(n, f, skill_defs_column(n)) for n, f in client_sdata.SKILL_FIELDS if n not in SKILL_CLIENT_ONLY]

    # The server's fields of a listed record, already in the client's widths.
    def server_item(self, grp, idx):
        cols = ", ".join(c for _, _, c in self.server_item_fields)
        rows = query(f"SELECT {cols} FROM defs.items WHERE itemid = {grp * 1000 + idx}")
        if not rows:
            raise CheckError(f"Item {grp}/{idx}: client row without a defs.items row (itemid {grp * 1000 + idx})")
        rec = {n: width_value(f, None if v is None else int(v), f"Item {grp}/{idx} field {n}") for (n, f, _), v in zip(self.server_item_fields, rows[0])}
        if (rec["type"], rec["typeid"]) != (grp, idx):
            raise CheckError(f"Item {grp}/{idx}: defs.items has type {rec['type']}, typeid {rec['typeid']}")
        return rec

    def server_skill(self, skill_id, level):
        cols = ", ".join(c for _, _, c in self.server_skill_fields)
        rows = query(f"SELECT {cols} FROM defs.skills WHERE skillid = {skill_id} AND skilllevel = {level}")
        if not rows:
            raise CheckError(f"Skill {skill_id}/{level}: client row without a defs.skills row")
        rec = {n: width_value(f, None if v is None else int(v), f"Skill {skill_id}/{level} field {n}") for (n, f, _), v in zip(self.server_skill_fields, rows[0])}
        if rec["level"] != level:
            raise CheckError(f"Skill {skill_id}/{level}: defs.skills has level {rec['level']}")
        return rec


def generate(data):
    """{'item': (groups, plain), 'skill': (records, plain)} from client.* and the rows (the files, before encryption)."""
    groups = read_client_items()
    for (grp, idx), row in data.item_rows.items():
        # Check 6, keys inside the tables.
        if not 1 <= grp <= ITEM_GROUPS or grp > len(groups) or not 1 <= idx <= len(groups[grp - 1]):
            raise CheckError(f"Item {grp}/{idx}: not a place in Item.SData (growing a table is not supported)")
        _, _, name, description, model, icon, type2, type3, *_ = row
        rec = data.server_item(grp, idx)
        rec["name"] = printable(name, f"Item {grp}/{idx} name", data.longest["item name"])
        rec["description"] = printable(description, f"Item {grp}/{idx} description", data.longest["item description"])
        for field, value in zip(ITEM_CLIENT_ONLY, (model, icon, type2, type3)):
            rec[field] = width_value(dict(parse_item.FIX)[field], int(value), f"Item {grp}/{idx} field {field}")
        groups[grp - 1][idx - 1] = rec
    records = read_client_skills()
    for (skill_id, level), row in data.skill_rows.items():
        if not 1 <= skill_id <= SKILLS or not 1 <= level <= SKILL_LEVELS or skill_id > len(records) // SKILL_LEVELS:
            raise CheckError(f"Skill {skill_id}/{level}: not a place in Skill.SData (growing a table is not supported)")
        _, _, name, description, icon, animation, toggle_type, sound, *_ = row
        rec = data.server_skill(skill_id, level)
        rec.update(skill_id=skill_id, slot=level)
        rec["name"] = printable(name, f"Skill {skill_id}/{level} name", data.longest["skill name"])
        rec["description"] = printable(description, f"Skill {skill_id}/{level} description", data.longest["skill description"])
        for field, value in zip(SKILL_CLIENT_ONLY, (icon, animation, toggle_type, sound)):
            rec[field] = width_value(dict(client_sdata.SKILL_FIELDS)[field], int(value), f"Skill {skill_id}/{level} field {field}")
        records[(skill_id - 1) * SKILL_LEVELS + level - 1] = rec
    return {"item": (groups, parse_item.write_items(groups)), "skill": (records, write_skills(records))}


def describe(value):
    return repr(value.decode("cp1252")) if isinstance(value, bytes) else str(value)


def record_changes(old, new, names):
    return [(n, old[n], new[n]) for n in names if old[n] != new[n]]


def item_changes(old_groups, new_groups):
    """[(key 'g/i', old name, new name, [(field, old, new)])] for every record that differs."""
    names = ["name", "description", *fix_names()]
    out = []
    for g, (old_group, new_group) in enumerate(zip(old_groups, new_groups), 1):
        for i, (old, new) in enumerate(zip(old_group, new_group), 1):
            changes = record_changes(old, new, names)
            if changes:
                out.append((f"{g}/{i}", old["name"], new["name"], changes))
    return out


def skill_changes(old_records, new_records):
    names = ["name", "description", *skill_names()]
    out = []
    for old, new in zip(old_records, new_records):
        changes = record_changes(old, new, names)
        if changes:
            out.append((f"{old['skill_id']}/{old['slot']}", old["name"], new["name"], changes))
    return out


def defs_export_rows(table, key_columns):
    """The original export of a defs table (as load_into_postgres.py reads it): {key: [string values; None for NULL]}."""
    columns = load_into_postgres.table_columns(DEFS_SCHEMA.read_text())[table]
    names = [c for c, _ in columns]
    index = [names.index(k) for k in key_columns]
    rows = {}
    for number, line in enumerate((DEFS_EXPORT / f"{table}.psv").read_text().split("\n"), 1):
        if not line.strip():
            continue
        fields = line.split("|")
        if len(fields) != len(columns):
            raise CheckError(f"{table}.psv line {number}: {len(fields)} fields, expected {len(columns)}")
        values = [load_into_postgres.decode_text(v) if ty == "text" else (None if v == "" else v) for (_, ty), v in zip(columns, fields)]
        rows[tuple(int(values[i]) for i in index)] = values
    return names, rows


def defs_differences(table, key_columns):
    """The keys of the rows of a defs table that are not as the original export has them (changed, added or removed)."""
    names, original = defs_export_rows(table, key_columns)
    skip = {names.index(c) for c in ("updatedate",) if c in names}
    current = {}
    for row in query(f"SELECT {', '.join(names)} FROM defs.{table}"):
        current[tuple(int(row[names.index(k)]) for k in key_columns)] = row
    differing = []
    for key in sorted(set(original) | set(current)):
        a, b = original.get(key), current.get(key)
        if a is None or b is None or any(x != y for i, (x, y) in enumerate(zip(a, b)) if i not in skip):
            differing.append(key)
    return differing


def only_grade_differs(names, original, current):
    """Whether two rows of a defs table (lists of strings) differ in the `grade` column alone (`updatedate` never counts)."""
    skip = {i for i, name in enumerate(names) if name == "updatedate"}
    differing = [names[i] for i, (a, b) in enumerate(zip(original, current)) if i not in skip and a != b]
    return differing == ["grade"]


def grade_only_item_edits():
    """The item ids whose only difference from the original export is `grade` and that the data editor has an `item_grade` edit for
    (`docs/plans/done/data-editing.md` section 9): the client does not use the drop grade, so check 4 needs no client row for them."""
    if query("SELECT to_regclass('edits.fields') IS NULL")[0][0] == "t":
        return set()
    edited = [int(k) for (k,) in query("SELECT key FROM edits.fields WHERE target = 'item_grade' AND field = 'grade'")]
    if not edited:
        return set()
    names, original = defs_export_rows("items", ["itemid"])
    found = set()
    ids = ", ".join(str(i) for i in edited)
    for row in query(f"SELECT {', '.join(names)} FROM defs.items WHERE itemid IN ({ids})"):
        itemid = int(row[names.index("itemid")])
        if (itemid,) in original and only_grade_differs(names, original[(itemid,)], row):
            found.add(itemid)
    return found


def run_checks(data, patch=None, report=print):
    """The eight checks of section 3. Returns (generated files, item changes, skill changes) against the originals."""
    # 1. Originals: Data() refused anything but the shipped files; the sizes and counts of what was imported.
    counts = (int(query("SELECT count(*) FROM client.items")[0][0]), int(query("SELECT count(*) FROM client.skills")[0][0]))
    if counts != (16045, 978):
        raise CheckError(f"client.items / client.skills hold {counts} rows, the shipped files have (16045, 978)")
    report(f"1 originals: Item.SData {load_client.SHA256['Item.SData'][:8]}…, Skill.SData {load_client.SHA256['Skill.SData'][:8]}… "
           f"as shipped; client.items {counts[0]} rows, client.skills {counts[1]} rows")
    # 2. Restore.
    check_restore(restored())
    report("2 restore: client.* alone rebuild both files and their encrypted forms byte for byte")
    # 3. Only the listed records change (and their fields are the database's and the rows').
    files = generate(data)
    original_items, original_skills = parse_items(data.originals["item"]), parse_skills(data.originals["skill"])
    new_items, new_skills = parse_items(files["item"][1]), parse_skills(files["skill"][1])
    item_changed, skill_changed = item_changes(original_items, new_items), skill_changes(original_skills, new_skills)
    for rows, changed, kind in ((data.item_rows, item_changed, "Item"), (data.skill_rows, skill_changed, "Skill")):
        listed = {f"{a}/{b}" for a, b in rows}
        extra = [c[0] for c in changed if c[0] not in listed]
        if extra:
            raise CheckError(f"{kind} {extra[0]} differs from the original but has no client row")
    expected = {"item": parse_item.write_items(new_items), "skill": write_skills(new_skills)}
    if any(expected[k] != files[k][1] for k in expected):
        raise CheckError("a generated file does not rebuild from its parsed records")
    for (grp, idx), row in data.item_rows.items():
        server = data.server_item(grp, idx)
        rec = new_items[grp - 1][idx - 1]
        for n, v in server.items():
            if rec[n] != v:
                raise CheckError(f"Item {grp}/{idx} field {n}: the file has {rec[n]}, defs has {v}")
        for n, v in zip(ITEM_CLIENT_ONLY, row[4:8]):
            if rec[n] != int(v):
                raise CheckError(f"Item {grp}/{idx} field {n}: the file has {rec[n]}, the client row has {v}")
        if rec["name"] != row[2].encode("ascii") or rec["description"] != row[3].encode("ascii"):
            raise CheckError(f"Item {grp}/{idx}: name or description is not the client row's")
    for (skill_id, level), row in data.skill_rows.items():
        server = data.server_skill(skill_id, level)
        rec = new_skills[(skill_id - 1) * SKILL_LEVELS + level - 1]
        for n, v in server.items():
            if rec[n] != v:
                raise CheckError(f"Skill {skill_id}/{level} field {n}: the file has {rec[n]}, defs has {v}")
        for n, v in zip(SKILL_CLIENT_ONLY, row[4:8]):
            if rec[n] != int(v):
                raise CheckError(f"Skill {skill_id}/{level} field {n}: the file has {rec[n]}, the client row has {v}")
        if rec["name"] != row[2].encode("ascii") or rec["description"] != row[3].encode("ascii"):
            raise CheckError(f"Skill {skill_id}/{level}: name or description is not the client row's")
    report(f"3 only the listed records change: {len(item_changed)} items and {len(skill_changed)} skills differ from the originals, "
           "each with a client row; the files parse back to exactly their bytes")
    # 4. Nothing forgotten.
    item_diff = defs_differences("items", ["itemid"])
    skill_diff = defs_differences("skills", ["skillid", "skilllevel"])
    grade_edits = grade_only_item_edits()
    for (itemid,) in item_diff:
        key = (itemid // 1000, itemid % 1000)
        if key not in data.item_rows and ("item", itemid, 0) not in data.server_only and itemid not in grade_edits:
            raise CheckError(f"defs.items {itemid} differs from the original export but is in neither client.item_rows nor client.server_only")
    for skill_id, level in skill_diff:
        if level == 100:
            if ("skill", skill_id, level) not in data.server_only:
                raise CheckError(f"defs.skills {skill_id}/100 (a monster skill, NpcSkill.SData, not generated yet) differs from the original "
                                 "export and is not in client.server_only")
        elif (skill_id, level) not in data.skill_rows and ("skill", skill_id, level) not in data.server_only:
            raise CheckError(f"defs.skills {skill_id}/{level} differs from the original export but is in neither client.skill_rows nor client.server_only")
    report(f"4 nothing forgotten: {len(item_diff)} defs.items and {len(skill_diff)} defs.skills rows differ from the original export, "
           f"each with a client row or a server_only reason ({len(grade_edits)} items differ in their drop grade alone, by the data editor)")
    # 5. Widths: checked while generating (every listed record's server values and client-only values).
    report(f"5 widths: all {len(data.item_rows) + len(data.skill_rows)} listed records' values fit their client fields")
    # 6. Keys inside the tables: checked while generating.
    report("6 keys: every client row names an existing place and has a defs row; no table grows")
    # 7. Text: checked while generating.
    report("7 text: all names and descriptions are printable ASCII and no longer than the longest of their kind")
    # 8. Deterministic.
    again = generate(data)
    if any(again[k][1] != files[k][1] for k in files):
        raise CheckError("generating twice gives different bytes")
    report("8 deterministic: generating twice gives the same bytes")
    if patch is not None:
        late = [k for rows in (data.item_rows, data.skill_rows) for k, r in rows.items() if int(r[8]) > patch]
        if late:
            raise CheckError(f"client row {late[0]} is for a later patch than {patch}")
    return files, item_changed, skill_changed


def cmd_check():
    data = Data()
    run_checks(data)
    print("check: all eight pass")


# ------------------------------------------------------------------------------------------------------------------- generate

def git(*args):
    return subprocess.run(["git", "-C", str(ROOT), *args], capture_output=True, text=True, check=True).stdout


def earlier_manifests(patch):
    """[(patch number, manifest)] of the earlier patches, newest first."""
    found = []
    for folder in PATCHES.glob("ps[0-9][0-9][0-9][0-9]"):
        number = int(folder.name[2:])
        if number < patch and (folder / "manifest.json").exists():
            found.append((number, json.loads((folder / "manifest.json").read_text())))
    return sorted(found, key=lambda x: -x[0])


def previous_version(kind, patch):
    """(patch number the file was last shipped in, its encrypted bytes, decrypted bytes) before this patch."""
    info = MEMBERS[kind]
    for number, manifest in earlier_manifests(patch):
        if any(f["member"] == info["member"] for f in manifest["files"]):
            encrypted = (PATCHES / f"ps{number:04d}" / info["member"]).read_bytes()
            return number, encrypted, seed_sdata.decrypt(encrypted, seed_sdata.load_consts(load_client.SEED_CONSTANTS))
    path = load_client.ORIGINALS / info["original"]
    return PREVIOUS_VERSION, path.read_bytes(), data_original(kind)


def data_original(kind):
    return load_client.original(load_client.ORIGINALS / MEMBERS[kind]["original"], MEMBERS[kind]["original"])


def changes_text(patch, item_changed, skill_changed):
    lines = [f"Client data patch {patch}: the records that differ from the previous version (old -> new)", ""]
    for label, changed in (("Item", item_changed), ("Skill", skill_changed)):
        for key, old_name, new_name, fields in changed:
            lines.append(f"{label} {key} {old_name.decode('cp1252')} -> {new_name.decode('cp1252')}")
            lines += [f"    {n}: {describe(a)} -> {describe(b)}" for n, a, b in fields]
            lines.append("")
    return "\n".join(lines)


def cmd_generate(patch, dirty):
    status = git("status", "--porcelain", "--", "tools/defs/additions.sql", "tools/sdata", "db/client")
    if status.strip() and not dirty:
        sys.exit("uncommitted changes in tools/defs/additions.sql, tools/sdata/ or db/client/ (the manifest's commit must describe the "
                 "files); commit them, or pass --dirty for a trial run:\n" + status)
    out_dir = PATCHES / f"ps{patch:04d}"
    if out_dir.exists():
        sys.exit(f"{out_dir} exists; refusing to overwrite a patch")
    data = Data()
    files, _, _ = run_checks(data, patch)
    manifest_files, all_item_changes, all_skill_changes = [], [], []
    previous_numbers = [number for number, _ in earlier_manifests(patch)]
    for kind in ("item", "skill"):
        info = MEMBERS[kind]
        plain = files[kind][1]
        encrypted = encrypt(plain)
        previous_patch, previous_encrypted, previous_plain = previous_version(kind, patch)
        if encrypted == previous_encrypted:
            continue
        if kind == "item":
            changed = item_changes(parse_items(previous_plain), parse_items(plain))
            all_item_changes = changed
        else:
            changed = skill_changes(parse_skills(previous_plain), parse_skills(plain))
            all_skill_changes = changed
        manifest_files.append({
            "member": info["member"], "us_path": info["us_path"], "size": len(encrypted), "sha256": sha(encrypted),
            "plain_sha256": sha(plain), "plain_size": len(plain), "replaces_sha256": sha(previous_encrypted),
            "changed": [c[0] for c in changed], "_encrypted": encrypted})
    if not manifest_files:
        sys.exit("both files are identical to the previous version; nothing to patch")
    manifest = {"patch": patch, "previous": previous_numbers[0] if previous_numbers else PREVIOUS_VERSION,
                "newshaiya_commit": git("rev-parse", "HEAD").strip(), "dirty": bool(status.strip()),
                "files": [{k: v for k, v in f.items() if not k.startswith("_")} for f in manifest_files]}
    out_dir.mkdir(parents=True)
    for f in manifest_files:
        path = out_dir / f["member"]
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(f["_encrypted"])
    (out_dir / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    (out_dir / "changes.txt").write_text(changes_text(patch, all_item_changes, all_skill_changes))
    print(f"wrote {out_dir}")
    for f in manifest["files"]:
        print(f"  {f['member']}: {f['size']} bytes, sha256 {f['sha256']}, changed {', '.join(f['changed'])}")
    if manifest["dirty"]:
        print("DIRTY: a trial run; the client side refuses it")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("restore").add_argument("out_dir")
    sub.add_parser("check")
    gen = sub.add_parser("generate")
    gen.add_argument("--patch", type=int, required=True)
    gen.add_argument("--dirty", action="store_true")
    args = parser.parse_args()
    try:
        if args.command == "restore":
            load_rows_file()
            cmd_restore(args.out_dir)
        elif args.command == "check":
            cmd_check()
        else:
            cmd_generate(args.patch, args.dirty)
    except CheckError as error:
        print(f"FAILED: {error}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
