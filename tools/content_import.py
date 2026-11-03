"""Import records and frozen copies, shared by the importers (docs/plans/ext-1-foundation.md section 3.2). Standard library only.

An importer writes the original data once into a frozen schema (`original_defs`, `original_world`, and `client`) and records, in the
same transaction, what it read (`content.imports`: the SHA-256 of every file, the rows and md5 of every table, the source). A second run
with the same files does nothing; with other files it is refused; `--reset` replaces the copy and rebuilds the live tables from it
(`data-editor content rebuild`); `--adopt` records a copy loaded from the same files on a box that already has live data.

The md5 of a table is the one of tools/content/table_hashes.sql and `data-editor content hashes`.
"""
import hashlib
import json
import subprocess
import sys
from pathlib import Path

import dbconn

ROOT = Path(__file__).resolve().parent.parent
EDITOR = ROOT / "target/release/data-editor"


def sha256_bytes(data):
    return hashlib.sha256(data).hexdigest()


def quote(text):
    """A SQL string literal (standard_conforming_strings is on)."""
    return "'" + str(text).replace("'", "''") + "'"


def run_psql(sql=None, stdin=None):
    result = subprocess.run(dbconn.psql_cmd() + (["-c", sql] if sql else []), input=stdin, capture_output=True, text=True)
    if result.returncode != 0:
        sys.exit(f"psql failed: {result.stderr.strip()}")
    return result.stdout


def query_one(sql):
    """The first column of the first row of a query, as text ('' when there is no row)."""
    out = subprocess.run(dbconn.psql_cmd() + ["-At", "-c", sql], capture_output=True, text=True)
    if out.returncode != 0:
        sys.exit(f"psql failed: {out.stderr.strip()}")
    return out.stdout.strip()


def table_md5_sql(schema, table):
    """A sub-select giving {"rows": n, "md5": "…"} for one table (the same md5 as table_hashes.sql)."""
    return (f"(SELECT jsonb_build_object('rows', count(*), 'md5', coalesce(md5(string_agg(t::text, chr(10) ORDER BY t::text)), md5('')))"
            f" FROM {schema}.{table} t)")


def import_record(part):
    """The content.imports row of a part as a dict, or None. Exits when the content schema does not exist yet."""
    if query_one("SELECT to_regclass('content.imports') IS NOT NULL") != "t":
        sys.exit("the database has no content.imports table: run target/release/servers list (it applies the migrations), then this again")
    out = query_one(f"SELECT row_to_json(r) FROM (SELECT imported_at::text AS imported_at, source, files, "
                    f"(rebuilt_at IS NULL OR rebuilt_at < imported_at) AS due FROM content.imports WHERE part = {quote(part)}) r")
    return json.loads(out) if out else None


def decide(part, files, live_schema, reset=False, adopt=False):
    """'import' or 'skip' (exit with a message when the run is refused or has nothing to do and nothing due).

    files: {file name: sha256} of everything the importer read. live_schema: the part's live schema, or None (the client).
    """
    record = import_record(part)
    if adopt:
        if record:
            sys.exit(f"{part} is recorded as imported on {record['imported_at']}: --adopt only records a copy where none exists")
        return "import"
    if reset:
        return "import"
    if record:
        differing = sorted(name for name in set(files) | set(record["files"]) if files.get(name) != record["files"].get(name))
        if differing:
            shown = ", ".join(differing[:8]) + (f" and {len(differing) - 8} more" if len(differing) > 8 else "")
            sys.exit(f"{part} was imported on {record['imported_at']} from {record['source']}; these files differ: {shown}. "
                     f"tools/setup.sh --reset (or this importer with --reset) re-imports and rebuilds (docs/plans/ext-1-foundation.md 3.2)")
        print(f"{part}: already imported on {record['imported_at']} from the same files; nothing to do")
        if live_schema and record["due"]:
            print(f"{part}: the rebuild after the import did not finish: running it")
            rebuild(part)
        return "skip"
    if live_schema and query_one(f"SELECT to_regnamespace({quote(live_schema)}) IS NOT NULL") == "t":
        print(f"{part} exists without an import record (a database from before 0.3.0): importing the originals and rebuilding {part} "
              f"from them; changes made by hand in SQL are replaced, the data editor's edits are written again")
    return "import"


def record_sql(part, schema, tables, source, files, adopted=False):
    """The INSERT that records the import, computed from the frozen tables inside the same transaction. An adopted import leaves the live
    tables as they are, so it counts as rebuilt (a re-run must not rebuild, and so replace, tables that were not built from the copy)."""
    rebuilt = "now()" if adopted else "NULL"
    tables_json = ", ".join(f"{quote(t)}, {table_md5_sql(schema, t)}" for t in tables)
    return (f"INSERT INTO content.imports (part, source, files, tables, rebuilt_at) VALUES ({quote(part)}, {quote(source)}, "
            f"{quote(json.dumps(files, sort_keys=True))}::jsonb, jsonb_build_object({tables_json}), {rebuilt})\n"
            f"ON CONFLICT (part) DO UPDATE SET imported_at = now(), rebuilt_at = EXCLUDED.rebuilt_at, source = EXCLUDED.source, "
            f"files = EXCLUDED.files, tables = EXCLUDED.tables;\n")


def rebuild(part):
    """data-editor content rebuild <part>, or the command to run when the binary is missing."""
    if not EDITOR.exists():
        print(f"data-editor is not built: run 'cargo build --release -p data-editor' and then 'target/release/data-editor content rebuild {part}'")
        return
    subprocess.run([str(EDITOR), "content", "rebuild", part], cwd=ROOT, check=True)
