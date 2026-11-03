#!/usr/bin/env python3
"""Import the client's Item.SData and Skill.SData (the encrypted originals, US patch 108) into Postgres schema `client`.

usage: load_client.py [--reset | --adopt] [<Item.SData> <Skill.SData>]
       (default: the two files in the folder CLIENT_ORIGINALS; unset, the client's original files in the checkout's git-ignored `reference` folder)

Refuses any file that is not the shipped one (SHA-256 of the encrypted file), decrypts it (SEED constants from Parsec's
SEEDConstants.cs, `seed_sdata.py`), parses it with the readers of `parse_item.py` / `client_sdata.py` (every byte explained), decodes the
text as cp1252 checking it encodes back to the same bytes, then replaces the schema `client` (db/client/schema.sql) and copies both
tables in, recording the import in content.imports in the same transaction. What a patch changes (the client rows) is in the content
schema, written by extensions (docs/plans/ext-1-foundation.md section 4). docs/plans/done/client-patches.md sections 2 and 3.

Imported once (docs/plans/ext-1-foundation.md section 3.2): run again it does nothing (the two files' SHA-256 are fixed, so other files
cannot pass the check above anyway); --reset replaces the schema and the record. --adopt checks the two files, checks that client.items
and client.skills restore them (client_patch.restored()), and records the import without touching the tables.
"""
import csv
import hashlib
import io
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
}
PLAIN_SHA256 = {
    "Item.SData": "efb6523292ee8e524e379f261abe8138b3816247f94d581e54826159ea03b0ba",
    "Skill.SData": "a7bca7064b86438107fca7a72befad67e9d585b156cfe5befaacad12a0ebc0fa",
}
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


def schema_columns(table):
    """The columns of CREATE TABLE client.<table> in the schema file, in order."""
    body = re.search(rf"CREATE TABLE client\.{table} \((.*?)\n\);", SCHEMA.read_text(), re.S).group(1)
    return [line.split()[0] for line in body.split("\n") if line.strip() and not line.strip().startswith("PRIMARY")]


def copy_text(table, columns, rows):
    out = io.StringIO()
    csv.writer(out, lineterminator="\n").writerows(rows)
    return f"COPY client.{table} ({', '.join(columns)}) FROM STDIN WITH (FORMAT csv, NULL '\\N');\n{out.getvalue()}\\."


def main():
    args = sys.argv[1:]
    reset, adopt = "--reset" in args, "--adopt" in args
    args = [a for a in args if a not in ("--reset", "--adopt")]
    paths = args or [ORIGINALS / "Item.SData", ORIGINALS / "Skill.SData"]
    if len(paths) != 2 or (reset and adopt):
        sys.exit(__doc__)
    files = {name: hashlib.sha256(Path(path).read_bytes()).hexdigest() for name, path in zip(("Item.SData", "Skill.SData"), paths)}
    if content_import.decide("client", files, None, reset=reset, adopt=adopt) == "skip":
        return
    items = item_rows(original(paths[0], "Item.SData"))
    skills = skill_rows(original(paths[1], "Skill.SData"))
    item_columns = ["grp", "idx", "name", "description", *(n for n, _ in parse_item.FIX)]
    skill_columns = ["skill_id", "slot", "name", "description", *(n for n, _ in client_sdata.SKILL_FIELDS)]
    assert schema_columns("items") == item_columns, "db/client/schema.sql: client.items is not parse_item.py's layout"
    assert schema_columns("skills") == skill_columns, "db/client/schema.sql: client.skills is not client_sdata.py's layout"
    source = str(Path(paths[0]).resolve().parent)
    record = content_import.record_sql("client", "client", ["items", "skills"], source, files, adopted=adopt)
    if adopt:
        import client_patch
        client_patch.check_restore(client_patch.restored())
        print("client.items and client.skills restore both shipped files byte for byte")
        content_import.run_psql(stdin=f"BEGIN;\n{record}COMMIT;\n")
        print("adopted: the import is recorded (content.imports 'client'); the tables were left as they are")
        return
    script = ["BEGIN;", "DROP SCHEMA IF EXISTS client CASCADE;", SCHEMA.read_text(), copy_text("items", item_columns, items),
              copy_text("skills", skill_columns, skills), record, "COMMIT;"]
    content_import.run_psql(stdin="\n".join(script) + "\n")
    print(f"client.items: {len(items)} rows, client.skills: {len(skills)} rows (recorded in content.imports 'client')")


if __name__ == "__main__":
    main()
