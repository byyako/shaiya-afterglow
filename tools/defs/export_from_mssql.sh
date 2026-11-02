#!/bin/bash
# Export the PS_GameDefs tables from the restored SQL Server container to data/defs-export/*.psv
# (pipe-separated via bcp, text columns as hex, NULL written as an empty field). Needs the SQL Server container
# (MSSQL_CONTAINER, default newshaiya-mssql: tools/dbconn.sh) running.
# The output is proprietary game data: it stays in data/, which is git-ignored.
set -eu
root="$(cd "$(dirname "$0")/../.." && pwd)"
. "$root/tools/dbconn.sh"
out="$root/data/defs-export"
mkdir -p "$out"
docker cp "$root/tools/defs/export_queries.tsv" "$MSSQL_CONTAINER":/tmp/export_queries.tsv
docker exec "$MSSQL_CONTAINER" bash -c '
  mkdir -p /tmp/defs && rm -f /tmp/defs/*
  while IFS=$'"'"'\t'"'"' read -r name query; do
    /opt/mssql-tools18/bin/bcp "$query" queryout "/tmp/defs/$name.psv" -c -t "|" -r "\n" \
      -S localhost -U sa -P "$MSSQL_SA_PASSWORD" -d PS_GameDefs -u > /dev/null
  done < /tmp/export_queries.tsv
'
docker cp "$MSSQL_CONTAINER":/tmp/defs/. "$out/"
wc -l "$out"/*.psv
