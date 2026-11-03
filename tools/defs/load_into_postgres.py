#!/usr/bin/env python3
"""Load the exported PS_GameDefs tables into Postgres schema `original_defs`, the frozen copy of the originals.

usage: load_into_postgres.py <export dir> <schema.sql> [--source TEXT] [--reset | --adopt]

Text columns arrive as hex; they are decoded as GBK (the original database's code page) and every value is checked to encode back to
the exact same bytes, so nothing is silently altered. Runs psql as tools/dbconn.py says (the PostgreSQL container by default).

The original data is imported once (docs/plans/ext-1-foundation.md section 3.2). The import is recorded in content.imports with the
SHA-256 of every .psv file read, in the same transaction that fills the frozen copy. Run again with the same files it does nothing (and
finishes a rebuild that an earlier run left undone); with other files it is refused unless --reset, which replaces the copy and
rebuilds the live `defs` from it, the installed extensions and the data editor's edits (`data-editor content rebuild defs`).
--adopt records a copy loaded from the same files on a box whose `defs` already holds them (the dev box); it leaves `defs` alone.
--source is the text recorded as where the files came from (default: the folder). Prints the verify.py command to run after an import.
"""
import csv
import io
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import content_import  # noqa: E402

ENCODING = "gbk"
SCHEMA = "original_defs"


def table_columns(schema_text):
    """{table: [(column, type), ...]} from the generated CREATE TABLE statements."""
    tables = {}
    for match in re.finditer(r"CREATE TABLE defs\.(\w+) \((.*?)\n\);", schema_text, re.S):
        columns = []
        for line in match.group(2).split("\n"):
            parts = line.strip().rstrip(",").split()
            if parts and parts[0] != "PRIMARY":
                columns.append((parts[0], parts[1]))
        tables[match.group(1)] = columns
    return tables


def decode_text(hexed):
    raw = bytes.fromhex(hexed)
    text = raw.decode(ENCODING)
    if text.encode(ENCODING) != raw:
        raise ValueError(f"text does not round-trip: {raw!r}")
    return text


def main():
    args = sys.argv[1:]
    reset, adopt = "--reset" in args, "--adopt" in args
    source = None
    if "--source" in args:
        at = args.index("--source")
        source = args[at + 1] if at + 1 < len(args) else sys.exit("--source needs a text")
        del args[at:at + 2]
    args = [a for a in args if a not in ("--reset", "--adopt")]
    if len(args) != 2 or (reset and adopt):
        sys.exit(__doc__)
    export_dir, schema_file = Path(args[0]), Path(args[1])
    schema_text = schema_file.read_text()
    tables = table_columns(schema_text)
    frozen = SCHEMA
    files = {f"{table}.psv": content_import.sha256_bytes((export_dir / f"{table}.psv").read_bytes()) for table in tables}
    if content_import.decide("defs", files, "defs", reset=reset, adopt=adopt) == "skip":
        return

    # One psql session, one transaction: the frozen schema, its tables and the import record.
    script = ["BEGIN;", f"DROP SCHEMA IF EXISTS {frozen} CASCADE;",
              schema_text.replace("CREATE SCHEMA IF NOT EXISTS defs;", f"CREATE SCHEMA {frozen};").replace("defs.", f"{frozen}.")]
    for table, columns in tables.items():
        rows = 0
        out = io.StringIO()
        writer = csv.writer(out, lineterminator="\n")
        for number, line in enumerate((export_dir / f"{table}.psv").read_text().split("\n"), 1):
            if not line.strip():
                continue
            fields = line.split("|")
            if len(fields) != len(columns):
                sys.exit(f"{table} line {number}: {len(fields)} fields, expected {len(columns)}")
            values = []
            for (name, ty), value in zip(columns, fields):
                if value == "" and ty != "text":
                    values.append("\\N")  # bcp writes NULL as an empty field
                elif ty == "text":
                    values.append(decode_text(value))
                else:
                    values.append(value)
            writer.writerow(values)
            rows += 1
        script.append(f"COPY {frozen}.{table} FROM STDIN WITH (FORMAT csv, NULL '\\N');\n{out.getvalue()}\\.")
        print(f"{table}: {rows} rows")
    script.append(content_import.record_sql("defs", frozen, list(tables), source or str(export_dir), files, adopted=adopt))
    script.append("COMMIT;")
    content_import.run_psql(stdin="\n".join(script) + "\n")
    print(f"{frozen}: the frozen copy is loaded and recorded (content.imports 'defs')")
    if adopt:
        print("adopted: defs was left as it is (compare it with the frozen copy: docs/plans/ext-1-foundation.md section 9 step 4)")
    else:
        content_import.rebuild("defs")
    print("check the row counts against the original database with: python3 tools/defs/verify.py db/defs/schema.sql "
          "docs/original/db/PS_GameDefs.columns.txt [--counts-only]")


if __name__ == "__main__":
    main()
