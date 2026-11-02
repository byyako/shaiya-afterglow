#!/usr/bin/env python3
"""Load the exported PS_GameDefs tables into Postgres schema `defs`.

usage: load_into_postgres.py <export dir> <schema.sql>

Text columns arrive as hex; they are decoded as GBK (the original database's code page) and every
value is checked to encode back to the exact same bytes, so nothing is silently altered.
Runs psql as tools/dbconn.py says (the PostgreSQL container by default). Existing `defs` data is replaced.
"""
import csv
import io
import re
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import dbconn  # noqa: E402

ENCODING = "gbk"


def psql(sql=None, stdin=None):
    cmd = dbconn.psql_cmd() + (["-c", sql] if sql else [])
    result = subprocess.run(cmd, input=stdin, capture_output=True, text=True)
    if result.returncode != 0:
        sys.exit(f"psql failed: {result.stderr.strip()}")
    return result.stdout


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
    export_dir, schema_file = Path(sys.argv[1]), Path(sys.argv[2])
    schema_text = schema_file.read_text()
    tables = table_columns(schema_text)

    psql("DROP SCHEMA IF EXISTS defs CASCADE")
    psql(stdin=schema_text)

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
        psql(f"\\copy defs.{table} FROM STDIN WITH (FORMAT csv, NULL '\\N')", stdin=out.getvalue())
        print(f"{table}: {rows} rows")


if __name__ == "__main__":
    main()
