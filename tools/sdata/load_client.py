#!/usr/bin/env python3
"""Import the client's Item.SData, Skill.SData and Monster.SData (the encrypted originals, US patch 108) into Postgres schema `client`.

usage: load_client.py [--reset | --adopt] [<Item.SData> <Skill.SData> [<Monster.SData>]]
       (default: the files in the folder CLIENT_ORIGINALS; unset, the client's original files in the checkout's git-ignored `reference` folder;
       Monster.SData is taken from the folder of the other two when no third path is given, and left out when it is not there)

Refuses any file that is not the shipped one (SHA-256 of the encrypted file), decrypts it (SEED constants from Parsec's
SEEDConstants.cs, `seed_sdata.py`), parses it with the readers of `parse_item.py` / `client_sdata.py` (every byte explained), decodes the
text as cp1252 checking it encodes back to the same bytes, then replaces the schema `client` (db/client/schema.sql) and copies both
tables in, recording the import in content.imports in the same transaction. What a patch changes (the client rows) is in the content
schema, written by extensions (docs/plans/ext-1-foundation.md section 4). docs/plans/done/client-patches.md sections 2 and 3.

Imported once (docs/plans/ext-1-foundation.md section 3.2): run again it does nothing (the files' SHA-256 are fixed, so other files
cannot pass the check above anyway); --reset replaces the schema and the record. --adopt checks the files, checks that client.items,
client.skills (and client.monsters, when it exists) restore them (client_patch.restored()), and records the import without touching the
tables.

Monster.SData came with the pets (docs/plans/ext-4-pets.md section 6.3), after the first imports: a box whose import was recorded with
the two other files only gets client.monsters added by a plain run (the table, the rows, and the file's SHA-256 and the table's md5 added
to the record, in one transaction); the other two tables are not touched.
"""
import csv
import hashlib
import io
import json
import os
import re
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import client_sdata
import content_import
import parse_item
import seed_sdata

ROOT = Path(__file__).resolve().parents[2]
# The git-ignored `reference` folder of the checkout holds clones of third-party projects and the client's original files; the two
# environment variables point elsewhere.
REFERENCE = ROOT / "reference"
SEED_CONSTANTS = Path(os.environ.get("PARSEC_SEEDCONSTANTS", REFERENCE / "matigramirez__Parsec" / "src" / "Parsec" / "Cryptography" / "SEEDConstants.cs"))
SCHEMA = ROOT / "db/client/schema.sql"
ORIGINALS = Path(os.environ.get("CLIENT_ORIGINALS", REFERENCE / "client-re__shaiya-ep4-client"))
SHA256 = {
    "Item.SData": "bb1ef0fccfc7d80be854b5376d18e41234920921ed62d791dc4a9ac78321ef48",
    "Skill.SData": "0ff8450aa07ddf1a6eace85bf61435f44e3699ccdaa722fe10703bf8d09a8d92",
    "Monster.SData": "10cd2b353276608b6c64b06d31bd5b21854df9d9bb81c488b978ff106b8a90e4",
}
PLAIN_SHA256 = {
    "Item.SData": "efb6523292ee8e524e379f261abe8138b3816247f94d581e54826159ea03b0ba",
    "Skill.SData": "a7bca7064b86438107fca7a72befad67e9d585b156cfe5befaacad12a0ebc0fa",
    "Monster.SData": "701fed38a1286c93e362574d0b3da654253ef3dd993266df2bce623f0f36e75f",
}
# The two files every import has; Monster.SData is the third, added later (the record of an older import lacks it).
PAIR = ("Item.SData", "Skill.SData")
MONSTER_RECORDS = 2474
ENCODING = "cp1252"


def decode(raw):
    """A client string (bytes, its NUL already removed) as text; it must encode back to the same bytes."""
    text = raw.decode(ENCODING)
    if text.encode(ENCODING) != raw:
        raise ValueError(f"text does not round-trip: {raw!r}")
    return text


def original(path, name):
    """The decrypted original: the encrypted file's SHA-256 is the shipped one, the decrypted file's too."""
    data = Path(path).read_bytes()
    if hashlib.sha256(data).hexdigest() != SHA256[name]:
        sys.exit(f"{path} is not the shipped {name} (SHA-256 {hashlib.sha256(data).hexdigest()}, expected {SHA256[name]})")
    plain = seed_sdata.decrypt(data, seed_sdata.load_consts(SEED_CONSTANTS))
    if hashlib.sha256(plain).hexdigest() != PLAIN_SHA256[name]:
        sys.exit(f"{path} decrypts to a different file than the shipped one")
    return plain


def item_rows(plain):
    """(grp, idx, name, description, FIX fields...) per record; the groups are the file's, idx is the place from 1."""
    groups = parse_item.read_items(plain)
    if parse_item.write_items(groups) != plain:
        sys.exit("the parsed items do not rebuild Item.SData")
    return [[g, i, decode(r["name"]), decode(r["description"]), *(r[n] for n, _ in parse_item.FIX)]
            for g, group in enumerate(groups, 1) for i, r in enumerate(group, 1)]


def skill_rows(plain):
    """(skill_id from 1, slot, name, description, SKILL_FIELDS...) per record."""
    records, end, rebuilt, _ = client_sdata.read_skills(plain)
    if end != len(plain) or rebuilt != plain:
        sys.exit("the parsed skills do not rebuild Skill.SData")
    rows = []
    for n, r in enumerate(records):
        texts = []
        for key in ("name", "description"):
            raw = r[key]
            if raw[-1:] != b"\0" or b"\0" in raw[:-1]:
                sys.exit(f"skill {n // client_sdata.SKILL_LEVELS + 1}: {key} is not text and one NUL: {raw!r}")
            texts.append(decode(raw[:-1]))
        rows.append([n // client_sdata.SKILL_LEVELS + 1, n % client_sdata.SKILL_LEVELS + 1, *texts, *(r[f] for f, _ in client_sdata.SKILL_FIELDS)])
    return rows


def monster_rows(plain):
    """(mob_id, name, MONSTER_FIELDS...) per record; the name without its one NUL."""
    records, end, rebuilt, _ = client_sdata.read_monsters(plain)
    if end != len(plain) or rebuilt != plain:
        sys.exit("the parsed monsters do not rebuild Monster.SData")
    if len(records) != MONSTER_RECORDS:
        sys.exit(f"Monster.SData holds {len(records)} records, the shipped file has {MONSTER_RECORDS}")
    rows = []
    for r in records:
        raw = r["name"]
        if raw[-1:] != b"\0" or b"\0" in raw[:-1]:
            sys.exit(f"monster {r['mob_id']}: name is not text and one NUL: {raw!r}")
        rows.append([r["mob_id"], decode(raw[:-1]), *(r[f] for f, _ in client_sdata.MONSTER_FIELDS)])
    return rows


def monster_columns():
    return ["mob_id", "name", *(n for n, _ in client_sdata.MONSTER_FIELDS)]


def schema_block(table):
    """The CREATE TABLE client.<table> statement and its COMMENT in the schema file."""
    return re.search(rf"CREATE TABLE client\.{table} \(.*?\nCOMMENT ON TABLE client\.{table} IS '(?:[^']|'')*';", SCHEMA.read_text(), re.S).group(0)


def schema_columns(table):
    """The columns of CREATE TABLE client.<table> in the schema file, in order."""
    body = re.search(rf"CREATE TABLE client\.{table} \((.*?)\n\);", SCHEMA.read_text(), re.S).group(1)
    return [line.split()[0] for line in body.split("\n") if line.strip() and not line.strip().startswith("PRIMARY")]


def copy_text(table, columns, rows):
    out = io.StringIO()
    csv.writer(out, lineterminator="\n").writerows(rows)
    return f"COPY client.{table} ({', '.join(columns)}) FROM STDIN WITH (FORMAT csv, NULL '\\N');\n{out.getvalue()}\\."


def add_monsters(path):
    """An import recorded without Monster.SData (before the pets): add client.monsters and extend the record, in one transaction."""
    rows = monster_rows(original(path, "Monster.SData"))
    columns = monster_columns()
    assert schema_columns("monsters") == columns, "db/client/schema.sql: client.monsters is not client_sdata.py's layout"
    sha = hashlib.sha256(Path(path).read_bytes()).hexdigest()
    update = (f"UPDATE content.imports SET files = files || {content_import.quote(json.dumps({'Monster.SData': sha}))}::jsonb, "
              f"tables = tables || jsonb_build_object('monsters', {content_import.table_md5_sql('client', 'monsters')}) WHERE part = 'client';")
    script = ["BEGIN;", schema_block("monsters"), copy_text("monsters", columns, rows), update, "COMMIT;"]
    content_import.run_psql(stdin="\n".join(script) + "\n")
    print(f"client.monsters: {len(rows)} rows added to the import of client (content.imports 'client' now records Monster.SData)")


def main():
    args = sys.argv[1:]
    reset, adopt = "--reset" in args, "--adopt" in args
    args = [a for a in args if a not in ("--reset", "--adopt")]
    paths = args or [ORIGINALS / "Item.SData", ORIGINALS / "Skill.SData"]
    if len(paths) not in (2, 3) or (reset and adopt):
        sys.exit(__doc__)
    monster_path = Path(paths[2]) if len(paths) == 3 else Path(paths[0]).resolve().parent / "Monster.SData"
    if len(paths) == 3 and not monster_path.exists():
        sys.exit(f"{monster_path}: no such file")
    with_monsters = monster_path.exists()
    pair = {name: hashlib.sha256(Path(path).read_bytes()).hexdigest() for name, path in zip(PAIR, paths[:2])}
    files = dict(pair)
    if with_monsters:
        files["Monster.SData"] = hashlib.sha256(monster_path.read_bytes()).hexdigest()
    record = content_import.import_record("client")
    if with_monsters and record and "Monster.SData" not in record["files"] and not (reset or adopt):
        # Older import, files as recorded: check them, then take the monsters in.
        if any(pair[n] != record["files"].get(n) for n in PAIR):
            sys.exit(f"client was imported from other files than these ({record['source']}); tools/setup.sh --reset re-imports")
        add_monsters(monster_path)
    if content_import.decide("client", files, None, reset=reset, adopt=adopt) == "skip":
        return
    if adopt and with_monsters:
        import client_patch
        if not client_patch.has_monsters():  # a copy loaded before the pets: adopted as it is, without the monsters
            with_monsters = False
            files.pop("Monster.SData")
    items = item_rows(original(paths[0], "Item.SData"))
    skills = skill_rows(original(paths[1], "Skill.SData"))
    monsters = monster_rows(original(monster_path, "Monster.SData")) if with_monsters else []
    item_columns = ["grp", "idx", "name", "description", *(n for n, _ in parse_item.FIX)]
    skill_columns = ["skill_id", "slot", "name", "description", *(n for n, _ in client_sdata.SKILL_FIELDS)]
    assert schema_columns("items") == item_columns, "db/client/schema.sql: client.items is not parse_item.py's layout"
    assert schema_columns("skills") == skill_columns, "db/client/schema.sql: client.skills is not client_sdata.py's layout"
    assert schema_columns("monsters") == monster_columns(), "db/client/schema.sql: client.monsters is not client_sdata.py's layout"
    source = str(Path(paths[0]).resolve().parent)
    tables = ["items", "skills", *(["monsters"] if with_monsters else [])]
    record = content_import.record_sql("client", "client", tables, source, files, adopted=adopt)
    if adopt:
        import client_patch
        client_patch.check_restore(client_patch.restored())
        print("client.items and client.skills" + (" and client.monsters" if with_monsters else "") + " restore the shipped files byte for byte")
        content_import.run_psql(stdin=f"BEGIN;\n{record}COMMIT;\n")
        print("adopted: the import is recorded (content.imports 'client'); the tables were left as they are")
        return
    script = ["BEGIN;", "DROP SCHEMA IF EXISTS client CASCADE;", SCHEMA.read_text(), copy_text("items", item_columns, items),
              copy_text("skills", skill_columns, skills), *([copy_text("monsters", monster_columns(), monsters)] if with_monsters else []),
              record, "COMMIT;"]
    content_import.run_psql(stdin="\n".join(script) + "\n")
    print(f"client.items: {len(items)} rows, client.skills: {len(skills)} rows" + (f", client.monsters: {len(monsters)} rows" if with_monsters else "")
          + " (recorded in content.imports 'client')")


if __name__ == "__main__":
    main()
