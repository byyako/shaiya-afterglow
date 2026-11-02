#!/usr/bin/env python3
"""Compare the loaded Postgres `defs` tables with the original SQL Server database.

usage: verify.py <schema.sql> <PS_GameDefs.columns.txt>

For every table it compares the row count, the sum of each numeric column, and the total byte
length of each text column (re-encoded as GBK in Postgres, DATALENGTH in SQL Server).
Needs PostgreSQL and the SQL Server container (MSSQL_CONTAINER, tools/dbconn.py) running.
"""
import re
import subprocess
import sys
from collections import OrderedDict

sys.path.insert(0, __import__("os").path.dirname(__file__))
sys.path.insert(0, __import__("os").path.join(__import__("os").path.dirname(__file__), ".."))
import dbconn  # noqa: E402
from load_into_postgres import table_columns  # noqa: E402
from generate import TABLES  # noqa: E402

MSSQL_NAME = {pg: ms for ms, pg in TABLES.items()}


def run_pg(sql):
    out = subprocess.run(dbconn.psql_cmd(strict=False) + ["-At", "-F", "|", "-c", sql],
                         capture_output=True, text=True, check=True).stdout
    return out.strip()


def run_mssql(sql):
    script = ('export SQLCMDPASSWORD="$MSSQL_SA_PASSWORD"; /opt/mssql-tools18/bin/sqlcmd -C -S localhost -U sa -d PS_GameDefs '
              '-W -h -1 -s "|" -Q "SET NOCOUNT ON; ' + sql.replace('"', '\\"') + '"')
    return subprocess.run(["docker", "exec", dbconn.MSSQL_CONTAINER, "bash", "-c", script], capture_output=True, text=True, check=True).stdout.strip()


def main():
    schema_text = open(sys.argv[1]).read()
    names = {}  # lowercased -> original-case names from the SQL Server column list
    for line in open(sys.argv[2]):
        parts = line.split("|")
        if len(parts) >= 8:
            names[(parts[0], parts[2].lower())] = parts[2]
    problems = 0
    for pg, columns in table_columns(schema_text).items():
        ms = MSSQL_NAME[pg]
        pg_parts, ms_parts, labels = ["count(*)"], ["COUNT(*)"], ["rows"]
        for col, ty in columns:
            original = names[(ms, col)]
            if ty == "text":
                pg_parts.append(f"sum(octet_length(convert_to({col}, 'GBK')))")
                ms_parts.append(f"SUM(CAST(DATALENGTH([{original}]) AS bigint))")
                labels.append(f"{col}:bytes")
            elif ty == "timestamp":
                pg_parts.append(f"sum(floor(extract(epoch from {col}))::bigint)")
                ms_parts.append(f"SUM(CAST(DATEDIFF(second, '19700101', [{original}]) AS bigint))")
                labels.append(f"{col}:epoch")
            else:
                pg_parts.append(f"sum({col}::bigint)")
                ms_parts.append(f"SUM(CAST([{original}] AS bigint))")
                labels.append(col)
        # one query per side, one column per output line to keep parsing simple
        pg_row = run_pg(f"SELECT {', '.join(pg_parts)} FROM defs.{pg}").split("|")
        ms_row = run_mssql(f"SELECT {', '.join(ms_parts)} FROM [{ms}]").split("|")
        bad = [(l, a, b) for l, a, b in zip(labels, pg_row, ms_row) if a != b]
        status = "OK" if not bad and len(pg_row) == len(ms_row) == len(labels) else "MISMATCH"
        print(f"{pg:16s} {len(labels):3d} checks  {status}")
        for l, a, b in bad:
            problems += 1
            print(f"    {l}: postgres={a} sqlserver={b}")
    sys.exit(1 if problems else 0)


if __name__ == "__main__":
    main()
