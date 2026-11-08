"""Generate the client's Item.SData, Skill.SData and (when extensions add monster records) Monster.SData from the database, check them,
and write a client data patch's hand-over folder
(docs/plans/done/client-patches.md, docs/plans/ext-1-foundation.md section 4).

usage: client_patch.py restore <out dir>
       client_patch.py check
       client_patch.py generate --patch N [--dirty]
       client_patch.py adopt --patch N [--patch-sha256 H --sig-sha256 H --status S]
       client_patch.py rebuild --patch N --out DIR
       client_patch.py record --patch N [--patch-sha256 H] [--sig-sha256 H] [--status S]

Monster.SData is generated only when content.client_monsters has rows (docs/plans/ext-4-pets.md section 6.3): the original records, fillers
(an empty name, model -1, zeros) for the ids between, then each row's record, a copy of the original record `copy_of` with the row's fields
(`day` may be a map from the copied record's day to the new one). With no rows the file is left out of the patch.

Reads the database only: a generated record is the client's original (client.items / client.skills, imported by tools/sdata/load_client.py)
unless it has a client row in content.client_items / content.client_skills (written by `data-editor ext install` from an extension's
rows); then it is the server's fields (defs.items / defs.skills) with the row's client-only fields, resolved as the plan's section 4.1
says: a field given by name, else the look-alike's record, else the record's own original.

  restore   writes the decrypted and encrypted files from client.items / client.skills alone and checks that they are the shipped ones.
  check     the eight checks of section 3 of the plan, one line each; exit 1 on the first error.
  generate  runs the checks, then records the patch in the ledger (content.client_patches, content.client_patch_files) and stamps
            since_patch on the client rows that have none, in one transaction, and writes data/client-patches/psNNNN/ (the encrypted
            files that changed, manifest.json, changes.txt).
  adopt     a patch made before the ledger existed (109): reads its hand-over folder, checks every file against its manifest, writes the
            ledger rows and stamps since_patch on the client rows its manifest lists.
  rebuild   generates the files from the database with only the client rows whose since_patch is at most N and compares each with the
            ledger's sha256 for that patch: MATCH, or the records that differ. The proof that a patch regenerates from the database alone.
  record    the client side's results for a patch: the patch file's and the signature's SHA-256 and its status.
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
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import client_sdata
import dbconn
import load_client
import parse_item
import seed_sdata

ROOT = Path(__file__).resolve().parents[2]
PATCHES = ROOT / "data/client-patches"
PREVIOUS_VERSION = 108  # the originals' patch
# What the client side reported of patch 109 (docs/client/client-patch-0109.md section 2): `adopt` records them, status "staged" (the
# updater passed on a spare client; the in-game check is pending). Later patches come with `record`.
ADOPTED = {109: {"patch_sha256": "9478d66a773db74ac132d8af13a40fb727e6564a9abdf3cb99532e3e2c17bde5",
                 "sig_sha256": "9d27d3f161c3d783ea5e42378d76e73b6edefc99119c92c7508390477f4f9533", "status": "staged"}}

ITEM_CLIENT_ONLY = ("model", "icon", "type2", "type3")
SKILL_CLIENT_ONLY = ("icon", "animation", "toggle_type", "sound")
SKILL_COLUMN_NAMES = {"level": "skilllevel", "prev_skill": "prevskillid", "defense_type": "defencetype", "defense_value": "defencevalue"}
WIDTHS = {"B": (0, 255, 0xFF), "H": (-32768, 65535, 0xFFFF), "I": (0, 4294967295, 0xFFFFFFFF),
          # the signed fields of Monster.SData are kept as they are
          "h": (-32768, 32767, None), "i": (-2147483648, 2147483647, None)}
MEMBERS = {
    "item": {"member": "Item/Item.SData", "us_path": "data/item/item.sdata", "original": "Item.SData"},
    "skill": {"member": "Character/Skill.SData", "us_path": "data/character/skill.sdata", "original": "Skill.SData"},
    "monster": {"member": "Monster/Monster.SData", "us_path": "data/monster/monster.sdata", "original": "Monster.SData"},
}
ITEM_GROUPS = 100
SKILLS = 326
MONSTER_ORIGINALS = load_client.MONSTER_RECORDS  # the records of the shipped Monster.SData; an added record's id is at least this
MONSTER_MAX_ID = 65535  # the u16 of 0x0601 (docs/plans/ext-4-pets.md section 6.3)
SKILL_LEVELS = client_sdata.SKILL_LEVELS


class CheckError(Exception):
    pass


# ---------------------------------------------------------------------------------------------------------------- the database

def psql_run(sql=None, stdin=None):
    result = subprocess.run(dbconn.psql_cmd({"PGCLIENTENCODING": "UTF8"}) + (["-c", sql] if sql else []), input=stdin, capture_output=True)
    if result.returncode != 0:
        sys.exit(f"psql failed: {result.stderr.decode(errors='replace').strip()}")
    return result.stdout.decode("utf-8")


def query(sql):
    """The rows of a SELECT as lists of strings (None for NULL)."""
    out = psql_run(f"COPY ({sql}) TO STDOUT WITH (FORMAT csv, NULL '\\N')")
    # A patch file's bytes as hex (the ledger's `encrypted`) are far longer than the csv module's default field limit of 131072.
    csv.field_size_limit(1 << 30)
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


def monster_names():
    return [n for n, _ in client_sdata.MONSTER_FIELDS]


def read_client_monsters():
    """client.monsters as a list of record dicts in file order (mob_id from 0; the name as cp1252 bytes without its NUL)."""
    rows = query(f"SELECT mob_id, name, {', '.join(monster_names())} FROM client.monsters ORDER BY mob_id")
    records = []
    for n, (mob_id, name, *values) in enumerate(rows):
        if int(mob_id) != n:
            raise CheckError(f"client.monsters {mob_id}: not the next place")
        rec = {"mob_id": n, "name": name.encode(load_client.ENCODING)}
        rec.update({k: int(v) for k, v in zip(monster_names(), values)})
        records.append(rec)
    return records


def write_monsters(records):
    """Monster.SData from records (name without its NUL): u32 count, then per record the name as u32 length + text + NUL and the fields."""
    text = client_sdata.pack_text
    return struct.pack("<I", len(records)) + b"".join(
        text(r["name"] + b"\0") + client_sdata.pack_fields(client_sdata.MONSTER_FIELDS, r) for r in records)


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


def parse_monsters(plain):
    """Monster.SData -> records as read_client_monsters makes them (name without its NUL); every byte must be explained."""
    parsed, end, rebuilt, _ = client_sdata.read_monsters(plain)
    if end != len(plain) or rebuilt != plain:
        raise CheckError("Monster.SData: the records do not explain the whole file")
    records = []
    for r in parsed:
        if r["name"][-1:] != b"\0" or b"\0" in r["name"][:-1]:
            raise CheckError(f"Monster.SData record {r['mob_id']}: name is not text and one NUL")
        records.append({**r, "name": r["name"][:-1]})
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

def has_monsters():
    """Whether client.monsters holds the imported Monster.SData (an import from before the pets has no such table)."""
    # Two statements: one that names the table fails to parse where the table does not exist, whatever the AND says.
    if query("SELECT to_regclass('client.monsters') IS NOT NULL")[0][0] != "t":
        return False
    return query("SELECT EXISTS (SELECT 1 FROM client.monsters)")[0][0] == "t"


def restored():
    """{'item': (plain, encrypted), 'skill': (plain, encrypted)} from client.items / client.skills alone, and 'monster' from
    client.monsters when it is there."""
    out = {"item": parse_item.write_items(read_client_items()), "skill": write_skills(read_client_skills())}
    if has_monsters():
        out["monster"] = write_monsters(read_client_monsters())
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
    return value if mask is None else value & mask


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


def resolve_field(by_name, look, own):
    """A client-only field of a client row (docs/plans/ext-1-foundation.md section 4.1): given by name, else the look-alike's record's,
    else the record's own original."""
    if by_name is not None:
        return by_name
    return look if look is not None else own


def resolve_item_row(raw, own, look):
    """A client_items row as (type, typeid, name, description, model, icon, type2, type3, since_patch, note); `own` and `look` are the
    client.items records (name, description, model, icon, type2, type3) or None. `keep` takes the text from the record's own original."""
    ty, tid, keep, name, description, model, icon, type2, type3, since, note = raw
    pick = lambda i, given: resolve_field(given, look[i] if look else None, own[i] if own else None)
    return (ty, tid, (own[0] if keep else name), (own[1] if keep else description),
            pick(2, model), pick(3, icon), pick(4, type2), pick(5, type3), since, note)


def resolve_skill_row(raw, own, look):
    """The same for client_skills: (skill_id, level, name, description, icon, animation, toggle_type, sound, since_patch, note)."""
    sid, level, keep, name, description, icon, animation, toggle, sound, since, note = raw
    pick = lambda i, given: resolve_field(given, look[i] if look else None, own[i] if own else None)
    return (sid, level, (own[0] if keep else name), (own[1] if keep else description),
            pick(2, icon), pick(3, animation), pick(4, toggle), pick(5, sound), since, note)


def nullable_int(v):
    return None if v is None else int(v)


def client_rows(kind):
    """The client rows of content.client_items / content.client_skills, resolved against client.items / client.skills."""
    if kind == "item":
        raw = query("SELECT type, typeid, keep, name, description, model, icon, type2, type3, since_patch, note, look_type, look_typeid "
                    "FROM content.client_items ORDER BY 1, 2")
        records = query("SELECT grp, idx, name, description, model, icon, type2, type3 FROM client.items")
        resolve = resolve_item_row
    else:
        raw = query("SELECT skill_id, level, keep, name, description, icon, animation, toggle_type, sound, since_patch, note, look_skill, look_level "
                    "FROM content.client_skills ORDER BY 1, 2")
        records = query("SELECT skill_id, slot, name, description, icon, animation, toggle_type, sound FROM client.skills")
        resolve = resolve_skill_row
    by_place = {(int(r[0]), int(r[1])): (r[2], r[3], *[nullable_int(v) for v in r[4:]]) for r in records}
    out = {}
    for r in raw:
        place = (int(r[0]), int(r[1]))
        look = by_place.get((int(r[11]), int(r[12]))) if r[11] is not None else None
        fixed = [int(r[0]), int(r[1]), r[2] == "t", r[3], r[4], *[nullable_int(v) for v in r[5:9]], nullable_int(r[9]), r[10]]
        out[place] = resolve(tuple(fixed), by_place.get(place), look)
    return out


def client_monster_rows():
    """content.client_monsters as {id: (copy_of, fields, since_patch, note)}; fields is the row's jsonb as a dict."""
    if query("SELECT to_regclass('content.client_monsters') IS NULL")[0][0] == "t":
        return {}
    return {int(i): (int(copy_of), json.loads(fields), nullable_int(since), note)
            for i, copy_of, fields, since, note in query("SELECT id, copy_of, fields, since_patch, note FROM content.client_monsters ORDER BY id")}


def monster_patch_limit(rows, limit):
    """As patch_limit, for the monster rows (since_patch is the third field)."""
    return rows if limit is None else {k: r for k, r in rows.items() if r[2] is not None and int(r[2]) <= limit}


def patch_limit(rows, limit):
    """The client rows of patches up to `limit` (rebuild): a row with since_patch NULL belongs to no patch yet."""
    if limit is None:
        return rows
    return {k: r for k, r in rows.items() if r[8] is not None and int(r[8]) <= limit}


class Data:
    """Everything one run reads: the originals restored from client.*, the client rows (resolved), and the server's fields for the listed
    records. `max_patch` keeps only the client rows of patches up to it (rebuild)."""

    def __init__(self, max_patch=None):
        record = query("SELECT files FROM content.imports WHERE part = 'client'")
        if not record:
            raise CheckError("content.imports has no 'client' record: run tools/sdata/load_client.py (a developer's box only)")
        self.shipped = json.loads(record[0][0])
        self.restored = restored()
        # The originals the other checks compare with are the restored bytes (check 2 proves them), not the files.
        self.originals = {kind: plain for kind, (plain, _) in self.restored.items()}
        self.item_rows = patch_limit(client_rows("item"), max_patch)
        self.skill_rows = patch_limit(client_rows("skill"), max_patch)
        self.monster_rows = monster_patch_limit(client_monster_rows(), max_patch)
        self.server_only = {(r[0], int(r[1]), int(r[2])): r[3] for r in query("SELECT kind, id, level, reason FROM content.server_only")}
        longest = lambda sql: int(query(sql)[0][0])
        self.longest = {"item name": longest("SELECT max(octet_length(name)) FROM client.items"),
                        "item description": longest("SELECT max(octet_length(description)) FROM client.items"),
                        "skill name": longest("SELECT max(octet_length(name)) FROM client.skills"),
                        "skill description": longest("SELECT max(octet_length(description)) FROM client.skills")}
        if self.monster_rows:
            self.longest["monster name"] = longest("SELECT max(octet_length(name)) FROM client.monsters")
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


def monster_filler(mob_id):
    """A record that draws nothing: an empty name, model -1 (no model), zeros (docs/plans/ext-4-pets.md section 6.2)."""
    return {"mob_id": mob_id, "name": b"", **{n: 0 for n in monster_names()}, "model": -1}


def monster_field(name, value, what, longest):
    """One field of a client_monsters row as the record holds it: the name as printable ASCII bytes, a number inside its width."""
    if name == "name":
        if not isinstance(value, str):
            raise CheckError(f"{what}: name {value!r} is not text")
        return printable(value, f"{what} name", longest)
    fmt = dict(client_sdata.MONSTER_FIELDS).get(name)
    if fmt is None:
        raise CheckError(f"{what}: {name!r} is not a field of Monster.SData ({', '.join(monster_names())}, name)")
    if isinstance(value, bool) or not isinstance(value, int):
        raise CheckError(f"{what} field {name}: {value!r} is not a whole number")
    return width_value(fmt, value, f"{what} field {name}")


def build_monster(mob_id, row, original, longest):
    """The record for a client_monsters row: the original record `copy_of`, then the row's fields. `day` may be a map {copied day: new day}
    (keys as text); a day the map does not name is kept."""
    copy_of, fields, _, _ = row
    what = f"Monster {mob_id}"
    if not MONSTER_ORIGINALS <= mob_id <= MONSTER_MAX_ID:
        raise CheckError(f"{what}: an added record's id is {MONSTER_ORIGINALS} to {MONSTER_MAX_ID} (below that it would change an original record)")
    if not 0 <= copy_of < MONSTER_ORIGINALS:
        raise CheckError(f"{what}: copy_of {copy_of} is not a record of the original Monster.SData (0 to {MONSTER_ORIGINALS - 1})")
    rec = {**original[copy_of], "mob_id": mob_id}
    for name, value in fields.items():
        if name == "day" and isinstance(value, dict):
            mapped = value.get(str(rec["day"]))
            if mapped is not None:
                rec["day"] = monster_field("day", mapped, what, longest)
            continue
        rec[name] = monster_field(name, value, what, longest)
    return rec


def generate_monsters(records, rows, longest):
    """Monster.SData from the original records (read_client_monsters) and the client_monsters rows: (records, plain). The ids between the
    originals and the first row, and between rows, are fillers; the count is the highest id plus one."""
    records = list(records)
    if len(records) != MONSTER_ORIGINALS:
        raise CheckError(f"client.monsters holds {len(records)} records, the shipped file has {MONSTER_ORIGINALS}")
    originals = list(records)
    for mob_id in range(MONSTER_ORIGINALS, max(rows) + 1):
        records.append(monster_filler(mob_id))
    for mob_id, row in rows.items():
        records[mob_id] = build_monster(mob_id, row, originals, longest)
    return records, write_monsters(records)


def generate(data):
    """{'item': (groups, plain), 'skill': (records, plain), 'monster': (records, plain) when there are monster rows} from client.* and the
    rows (the files, before encryption)."""
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
    files = {"item": (groups, parse_item.write_items(groups)), "skill": (records, write_skills(records))}
    if data.monster_rows:
        files["monster"] = generate_monsters(read_client_monsters(), data.monster_rows, data.longest["monster name"])
    return files


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


def monster_changes(old_records, new_records):
    """The records that differ: an original changed, field by field (this chunk changes none), and every record past the old file's end as
    one line (a filler is not listed): (key, old name, new name, [(field, old, new)])."""
    names = ["name", *monster_names()]
    out = []
    for old, new in zip(old_records, new_records):
        changes = record_changes(old, new, names)
        if changes:
            out.append((str(old["mob_id"]), old["name"], new["name"], changes))
    for new in new_records[len(old_records):]:
        if new["name"] or new["model"] != -1:
            out.append((str(new["mob_id"]), b"", new["name"],
                        [("record", "(none)", f"added: model {new['model']}, level {new['level']}, day {new['day']}, size {new['size']}, hp {new['hp']}")]))
    return out


def defs_differences(table, key_columns):
    """The keys of the rows of a defs table that are not as the frozen original (original_defs) has them (changed, added or removed);
    `updatedate` never counts."""
    keys = ", ".join(f"coalesce(a.{k}, b.{k})" for k in key_columns)
    join = " AND ".join(f"a.{k} = b.{k}" for k in key_columns)
    rows = query(f"SELECT {keys} FROM defs.{table} a FULL JOIN original_defs.{table} b ON {join} "
                 f"WHERE a.{key_columns[0]} IS NULL OR b.{key_columns[0]} IS NULL OR (to_jsonb(a) - 'updatedate') <> (to_jsonb(b) - 'updatedate') "
                 f"ORDER BY {', '.join(str(i + 1) for i in range(len(key_columns)))}")
    return [tuple(int(v) for v in row) for row in rows]


def only_grade_differs(names, original, current):
    """Whether two rows of a defs table (lists of strings) differ in the `grade` column alone (`updatedate` never counts)."""
    skip = {i for i, name in enumerate(names) if name == "updatedate"}
    differing = [names[i] for i, (a, b) in enumerate(zip(original, current)) if i not in skip and a != b]
    return differing == ["grade"]


def grade_only_item_edits():
    """The item ids whose only difference from the original is `grade` and that the data editor has an `item_grade` edit for
    (`docs/plans/done/data-editing.md` section 9): the client does not use the drop grade, so check 4 needs no client row for them."""
    if query("SELECT to_regclass('edits.fields') IS NULL")[0][0] == "t":
        return set()
    edited = [int(k) for (k,) in query("SELECT key FROM edits.fields WHERE target = 'item_grade' AND field = 'grade'")]
    if not edited:
        return set()
    names = table_columns("items")
    ids = ", ".join(str(i) for i in edited)
    original = {int(r[names.index("itemid")]): r for r in query(f"SELECT {', '.join(names)} FROM original_defs.items WHERE itemid IN ({ids})")}
    found = set()
    for row in query(f"SELECT {', '.join(names)} FROM defs.items WHERE itemid IN ({ids})"):
        itemid = int(row[names.index("itemid")])
        if itemid in original and only_grade_differs(names, original[itemid], row):
            found.add(itemid)
    return found


def run_checks(data, patch=None, report=print):
    """The eight checks of section 3. Returns (generated files, item changes, skill changes, monster changes) against the originals."""
    # 1. Originals: the files the import recorded (content.imports 'client') are the shipped ones; the sizes and counts of what was imported.
    shipped = {name: sha for name, sha in load_client.SHA256.items() if name in data.shipped or name in load_client.PAIR}
    if data.shipped != shipped:
        raise CheckError(f"the import recorded other files than the shipped ones: {data.shipped} (shipped: {shipped})")
    counts = (int(query("SELECT count(*) FROM client.items")[0][0]), int(query("SELECT count(*) FROM client.skills")[0][0]))
    if counts != (16045, 978):
        raise CheckError(f"client.items / client.skills hold {counts} rows, the shipped files have (16045, 978)")
    monsters = f", client.monsters {MONSTER_ORIGINALS} rows" if has_monsters() else ""
    if data.monster_rows and not monsters:
        raise CheckError("content.client_monsters has rows but client.monsters is empty: run tools/sdata/load_client.py")
    if monsters and int(query("SELECT count(*) FROM client.monsters")[0][0]) != MONSTER_ORIGINALS:
        raise CheckError(f"client.monsters does not hold the shipped file's {MONSTER_ORIGINALS} rows")
    report(f"1 originals: Item.SData {load_client.SHA256['Item.SData'][:8]}…, Skill.SData {load_client.SHA256['Skill.SData'][:8]}… "
           f"as recorded by the import; client.items {counts[0]} rows, client.skills {counts[1]} rows{monsters}")
    # 2. Restore.
    check_restore(data.restored)
    report(f"2 restore: client.* alone rebuild {'all three files' if has_monsters() else 'both files'} and their encrypted forms byte for byte")
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
    monster_changed = []
    if "monster" in files:
        original_monsters, new_monsters = parse_monsters(data.originals["monster"]), parse_monsters(files["monster"][1])
        monster_changed = monster_changes(original_monsters, new_monsters)
        expected["monster"] = write_monsters(new_monsters)
        if new_monsters[:MONSTER_ORIGINALS] != original_monsters:
            raise CheckError("an original Monster.SData record differs from the shipped one")
        for mob_id in data.monster_rows:
            if mob_id < MONSTER_ORIGINALS or mob_id >= len(new_monsters):
                raise CheckError(f"Monster {mob_id}: not a record of the generated file")
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
    monster_note = f"; {len(data.monster_rows)} monster records are added past the original {MONSTER_ORIGINALS}, none changed" if data.monster_rows else ""
    report(f"3 only the listed records change: {len(item_changed)} items and {len(skill_changed)} skills differ from the originals, "
           f"each with a client row{monster_note}; the files parse back to exactly their bytes")
    # 4. Nothing forgotten.
    item_diff = defs_differences("items", ["itemid"])
    skill_diff = defs_differences("skills", ["skillid", "skilllevel"])
    grade_edits = grade_only_item_edits()
    for (itemid,) in item_diff:
        key = (itemid // 1000, itemid % 1000)
        if key not in data.item_rows and ("item", itemid, 0) not in data.server_only and itemid not in grade_edits:
            raise CheckError(f"defs.items {itemid} differs from the original but has neither a client row nor a server_only reason")
    for skill_id, level in skill_diff:
        if level == 100:
            if ("skill", skill_id, level) not in data.server_only:
                raise CheckError(f"defs.skills {skill_id}/100 (a monster skill, NpcSkill.SData, not generated yet) differs from the original "
                                 "and has no server_only reason")
        elif (skill_id, level) not in data.skill_rows and ("skill", skill_id, level) not in data.server_only:
            raise CheckError(f"defs.skills {skill_id}/{level} differs from the original but has neither a client row nor a server_only reason")
    report(f"4 nothing forgotten: {len(item_diff)} defs.items and {len(skill_diff)} defs.skills rows differ from the original, "
           f"each with a client row or a server_only reason ({len(grade_edits)} items differ in their drop grade alone, by the data editor)")
    # 5. Widths: checked while generating (every listed record's server values and client-only values).
    report(f"5 widths: all {len(data.item_rows) + len(data.skill_rows) + len(data.monster_rows)} listed records' values fit their client fields")
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
        late = [k for rows in (data.item_rows, data.skill_rows) for k, r in rows.items() if r[8] is not None and int(r[8]) > patch]
        late += [k for k, r in data.monster_rows.items() if r[2] is not None and int(r[2]) > patch]
        if late:
            raise CheckError(f"client row {late[0]} is for a later patch than {patch}")
    return files, item_changed, skill_changed, monster_changed


def cmd_check():
    data = Data()
    run_checks(data)
    print("check: all eight pass")


# ------------------------------------------------------------------------------------------------------------------- generate

def git(*args):
    return subprocess.run(["git", "-C", str(ROOT), *args], capture_output=True, text=True, check=True).stdout


def sql_text(text):
    return "'" + str(text).replace("'", "''") + "'"


def ledger_numbers(below):
    """The patch numbers in the ledger below `below`, newest first."""
    return [int(n) for (n,) in query(f"SELECT patch FROM content.client_patches WHERE patch < {int(below)} ORDER BY patch DESC")]


def ledger_member(patch, member):
    """(encrypted bytes, sha256) of a member the ledger holds for a patch, or None."""
    rows = query(f"SELECT encode(encrypted, 'hex'), sha256 FROM content.client_patch_files WHERE patch = {int(patch)} AND member = {sql_text(member)}")
    return (bytes.fromhex(rows[0][0]), rows[0][1]) if rows else None


def previous_version(kind, patch):
    """(patch number the file was last shipped in, its encrypted bytes, decrypted bytes) before this patch: the newest earlier patch of
    the ledger that carried it, else the restored originals (108)."""
    info = MEMBERS[kind]
    number = newest_carrier(ledger_numbers(patch), lambda n: ledger_member(n, info["member"]) is not None)
    if number is not None:
        encrypted = ledger_member(number, info["member"])[0]
        return number, encrypted, seed_sdata.decrypt(encrypted, seed_sdata.load_consts(load_client.SEED_CONSTANTS))
    plain, encrypted = restored()[kind]
    return PREVIOUS_VERSION, encrypted, plain


def installed_extensions():
    """{extension: version} of the installed extensions (the content's provenance, for the manifest and the ledger)."""
    return {name: version for name, version in query("SELECT name, version FROM content.extensions ORDER BY install_seq")}


def changes_text(patch, item_changed, skill_changed, monster_changed=()):
    lines = [f"Client data patch {patch}: the records that differ from the previous version (old -> new)", ""]
    for label, changed in (("Item", item_changed), ("Skill", skill_changed), ("Monster", monster_changed)):
        for key, old_name, new_name, fields in changed:
            lines.append(f"{label} {key} {old_name.decode('cp1252')} -> {new_name.decode('cp1252')}")
            lines += [f"    {n}: {describe(a)} -> {describe(b)}" for n, a, b in fields]
            lines.append("")
    return "\n".join(lines)


def build_manifest(patch, previous, commit, extensions, dirty, files):
    """The hand-over folder's manifest: the commit it was generated at (`commit`), the installed extensions and their versions, and the
    members (without the encrypted bytes, which are files of the folder)."""
    return {"patch": patch, "previous": previous, "commit": commit, "extensions": extensions, "dirty": dirty,
            "files": [{k: v for k, v in f.items() if not k.startswith("_")} for f in files]}


def newest_carrier(numbers, carried):
    """The newest earlier patch that carried a member: `numbers` are the ledger's patch numbers below this one, newest first, `carried`
    says whether a patch carried it; None when none did (the previous version is then the restored original, 108)."""
    return next((n for n in numbers if carried(n)), None)


def ledger_sql(patch, previous, commit, extensions, dirty, files, patch_sha256=None, sig_sha256=None, status="generated"):
    """The statements that record a generated (or adopted) patch in the ledger and stamp since_patch on the client rows that have none."""
    def text_or_null(v):
        return "NULL" if v is None else sql_text(v)
    out = [f"INSERT INTO content.client_patches (patch, previous, commit, extensions, dirty, patch_sha256, sig_sha256, status) VALUES "
           f"({patch}, {previous}, {sql_text(commit)}, {sql_text(json.dumps(extensions, sort_keys=True))}::jsonb, {'true' if dirty else 'false'}, "
           f"{text_or_null(patch_sha256)}, {text_or_null(sig_sha256)}, {sql_text(status)});"]
    for f in files:
        changed = "ARRAY[" + ", ".join(sql_text(c) for c in f["changed"]) + "]::text[]"
        out.append(f"INSERT INTO content.client_patch_files (patch, member, us_path, size, sha256, plain_sha256, plain_size, replaces_sha256, changed, encrypted) "
                   f"VALUES ({patch}, {sql_text(f['member'])}, {sql_text(f['us_path'])}, {f['size']}, {sql_text(f['sha256'])}, {sql_text(f['plain_sha256'])}, "
                   f"{f['plain_size']}, {sql_text(f['replaces_sha256'])}, {changed}, decode('{f['_encrypted'].hex()}', 'hex'));")
    return out


def cmd_generate(patch, dirty):
    status = git("status", "--porcelain", "--", "tools/sdata", "db/client", "extensions")
    if status.strip() and not dirty:
        sys.exit("uncommitted changes in tools/sdata/, db/client/ or extensions/ (the manifest's commit must describe the "
                 "files); commit them, or pass --dirty for a trial run:\n" + status)
    out_dir = PATCHES / f"ps{patch:04d}"
    if out_dir.exists():
        sys.exit(f"{out_dir} exists; refusing to overwrite a patch")
    if query(f"SELECT 1 FROM content.client_patches WHERE patch = {patch}"):
        sys.exit(f"patch {patch} is in the ledger already; refusing to overwrite a patch")
    data = Data()
    files, _, _, _ = run_checks(data, patch)
    manifest_files, all_item_changes, all_skill_changes, all_monster_changes = [], [], [], []
    previous_numbers = ledger_numbers(patch)
    for kind in files:
        info = MEMBERS[kind]
        plain = files[kind][1]
        encrypted = encrypt(plain)
        previous_patch, previous_encrypted, previous_plain = previous_version(kind, patch)
        if encrypted == previous_encrypted:
            continue
        if kind == "item":
            changed = item_changes(parse_items(previous_plain), parse_items(plain))
            all_item_changes = changed
        elif kind == "skill":
            changed = skill_changes(parse_skills(previous_plain), parse_skills(plain))
            all_skill_changes = changed
        else:
            changed = monster_changes(parse_monsters(previous_plain), parse_monsters(plain))
            all_monster_changes = changed
        manifest_files.append({
            "member": info["member"], "us_path": info["us_path"], "size": len(encrypted), "sha256": sha(encrypted),
            "plain_sha256": sha(plain), "plain_size": len(plain), "replaces_sha256": sha(previous_encrypted),
            "changed": [c[0] for c in changed], "_encrypted": encrypted})
    if not manifest_files:
        sys.exit("both files are identical to the previous version; nothing to patch")
    previous = previous_numbers[0] if previous_numbers else PREVIOUS_VERSION
    extensions = installed_extensions()
    commit = git("rev-parse", "HEAD").strip()
    manifest = build_manifest(patch, previous, commit, extensions, bool(status.strip()), manifest_files)
    if not manifest["dirty"]:
        # The ledger and since_patch in one transaction; a trial (--dirty) run is not recorded.
        script = ["BEGIN;", *ledger_sql(patch, previous, commit, extensions, False, manifest_files),
                  f"UPDATE content.client_items SET since_patch = {patch} WHERE since_patch IS NULL;",
                  f"UPDATE content.client_skills SET since_patch = {patch} WHERE since_patch IS NULL;",
                  *([f"UPDATE content.client_monsters SET since_patch = {patch} WHERE since_patch IS NULL;"] if data.monster_rows else []), "COMMIT;"]
        psql_run(stdin=("\n".join(script) + "\n").encode("utf-8"))
    out_dir.mkdir(parents=True)
    for f in manifest_files:
        path = out_dir / f["member"]
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(f["_encrypted"])
    (out_dir / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    (out_dir / "changes.txt").write_text(changes_text(patch, all_item_changes, all_skill_changes, all_monster_changes))
    print(f"wrote {out_dir}")
    for f in manifest["files"]:
        shown = ", ".join(f["changed"][:12]) + (f" … ({len(f['changed'])} in all)" if len(f["changed"]) > 12 else "")
        print(f"  {f['member']}: {f['size']} bytes, sha256 {f['sha256']}, changed {shown}")
    if manifest["dirty"]:
        print("DIRTY: a trial run; the client side refuses it, and the ledger and since_patch were not touched")
    else:
        print(f"patch {patch} is in the ledger (content.client_patches); the client rows without a patch are stamped {patch}")


def cmd_adopt(patch, patch_sha256, sig_sha256, status):
    """A patch made before the ledger existed: its hand-over folder is checked against its own manifest, then recorded."""
    if query(f"SELECT 1 FROM content.client_patches WHERE patch = {patch}"):
        sys.exit(f"patch {patch} is in the ledger already")
    folder = PATCHES / f"ps{patch:04d}"
    manifest = json.loads((folder / "manifest.json").read_text())
    if manifest["patch"] != patch:
        sys.exit(f"{folder}/manifest.json is patch {manifest['patch']}")
    files = []
    for f in manifest["files"]:
        encrypted = (folder / f["member"]).read_bytes()
        if sha(encrypted) != f["sha256"] or len(encrypted) != f["size"]:
            sys.exit(f"{folder / f['member']}: not the file the manifest describes")
        plain = seed_sdata.decrypt(encrypted, seed_sdata.load_consts(load_client.SEED_CONSTANTS))
        if sha(plain) != f["plain_sha256"] or len(plain) != f["plain_size"]:
            sys.exit(f"{folder / f['member']}: decrypts to another file than the manifest's")
        files.append({**f, "_encrypted": encrypted})
    defaults = ADOPTED.get(patch, {})
    commit = manifest.get("commit") or manifest["newshaiya_commit"]  # patch 109's key is the old private tree's
    owners = {}
    stamps = []
    for f in files:
        kind = "item" if f["member"] == MEMBERS["item"]["member"] else "skill"
        keys = [tuple(int(x) for x in c.split("/")) for c in f["changed"]]
        if kind == "item":
            where = ", ".join(f"({a}, {b})" for a, b in keys)
            stamps.append(f"UPDATE content.client_items SET since_patch = {patch} WHERE (type, typeid) IN ({where}) AND since_patch IS NULL;")
            rows = query(f"SELECT DISTINCT e.name, e.version FROM content.extensions e JOIN content.client_items i ON i.extension = e.name WHERE (i.type, i.typeid) IN ({where})")
        else:
            where = ", ".join(f"({a}, {b})" for a, b in keys)
            stamps.append(f"UPDATE content.client_skills SET since_patch = {patch} WHERE (skill_id, level) IN ({where}) AND since_patch IS NULL;")
            rows = query(f"SELECT DISTINCT e.name, e.version FROM content.extensions e JOIN content.client_skills i ON i.extension = e.name WHERE (i.skill_id, i.level) IN ({where})")
        owners.update({name: version for name, version in rows})
    script = ["BEGIN;", *ledger_sql(patch, manifest["previous"], commit, manifest.get("extensions", owners), manifest["dirty"], files,
                                    patch_sha256 or defaults.get("patch_sha256"), sig_sha256 or defaults.get("sig_sha256"),
                                    status or defaults.get("status", "generated")), *stamps, "COMMIT;"]
    psql_run(stdin=("\n".join(script) + "\n").encode("utf-8"))
    stamped = query(f"SELECT (SELECT count(*) FROM content.client_items WHERE since_patch = {patch}) + (SELECT count(*) FROM content.client_skills WHERE since_patch = {patch})")[0][0]
    print(f"adopted patch {patch}: {len(files)} members in the ledger (status {status or defaults.get('status', 'generated')}), {stamped} client rows stamped {patch}")


def cmd_rebuild(patch, out_dir):
    """The proof of the ledger: the files regenerate from the database alone, with the client rows of patches up to `patch`."""
    ledger = {m: sha256 for m, sha256 in query(f"SELECT member, sha256 FROM content.client_patch_files WHERE patch = {patch}")}
    if not ledger:
        sys.exit(f"patch {patch} is not in the ledger (content.client_patch_files has no member of it)")
    data = Data(max_patch=patch)
    files = generate(data)
    out_dir = Path(out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    differs = 0
    for kind in ("item", "skill", "monster"):
        info = MEMBERS[kind]
        if info["member"] not in ledger:
            continue
        if kind not in files:
            differs += 1
            print(f"{info['member']}: DIFFERS (the ledger has it, the database has no rows for it up to patch {patch})")
            continue
        plain = files[kind][1]
        encrypted = encrypt(plain)
        path = out_dir / info["member"]
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(encrypted)
        if sha(encrypted) == ledger[info["member"]]:
            print(f"{info['member']}: MATCH ({sha(encrypted)})")
            continue
        differs += 1
        print(f"{info['member']}: DIFFERS (rebuilt {sha(encrypted)}, ledger {ledger[info['member']]})")
        carried, _ = ledger_member(patch, info["member"])
        old = seed_sdata.decrypt(carried, seed_sdata.load_consts(load_client.SEED_CONSTANTS))
        changed = {"item": lambda: item_changes(parse_items(old), parse_items(plain)),
                   "skill": lambda: skill_changes(parse_skills(old), parse_skills(plain)),
                   "monster": lambda: monster_changes(parse_monsters(old), parse_monsters(plain))}[kind]()
        for key, old_name, new_name, fields in changed:
            print(f"  {key} {old_name.decode('cp1252')} -> {new_name.decode('cp1252')}: " + ", ".join(f"{n} {describe(a)} -> {describe(b)}" for n, a, b in fields))
    if differs:
        sys.exit(1)
    print(f"patch {patch} regenerates byte for byte from the database")


def cmd_record(patch, patch_sha256, sig_sha256, status):
    sets = [f"{col} = {sql_text(v)}" for col, v in (("patch_sha256", patch_sha256), ("sig_sha256", sig_sha256), ("status", status)) if v]
    if not sets:
        sys.exit("record needs --patch-sha256, --sig-sha256 or --status")
    done = query(f"UPDATE content.client_patches SET {', '.join(sets)} WHERE patch = {patch} RETURNING patch")
    if not done:
        sys.exit(f"patch {patch} is not in the ledger")
    print(f"patch {patch}: " + ", ".join(sets))


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("restore").add_argument("out_dir")
    sub.add_parser("check")
    gen = sub.add_parser("generate")
    gen.add_argument("--patch", type=int, required=True)
    gen.add_argument("--dirty", action="store_true")
    for name in ("adopt", "record"):
        p = sub.add_parser(name)
        p.add_argument("--patch", type=int, required=True)
        p.add_argument("--patch-sha256")
        p.add_argument("--sig-sha256")
        p.add_argument("--status", choices=["generated", "staged", "live", "withdrawn"])
    reb = sub.add_parser("rebuild")
    reb.add_argument("--patch", type=int, required=True)
    reb.add_argument("--out", required=True)
    args = parser.parse_args()
    try:
        if args.command == "restore":
            cmd_restore(args.out_dir)
        elif args.command == "check":
            cmd_check()
        elif args.command == "generate":
            cmd_generate(args.patch, args.dirty)
        elif args.command == "adopt":
            cmd_adopt(args.patch, args.patch_sha256, args.sig_sha256, args.status)
        elif args.command == "rebuild":
            cmd_rebuild(args.patch, args.out)
        else:
            cmd_record(args.patch, args.patch_sha256, args.sig_sha256, args.status)
    except CheckError as error:
        print(f"FAILED: {error}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
