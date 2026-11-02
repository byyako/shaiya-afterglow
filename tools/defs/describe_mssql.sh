#!/bin/bash
# Describe the structure of the restored original databases (the SQL Server container, MSSQL_CONTAINER, default
# newshaiya-mssql: tools/dbconn.sh), three plain-text files per database, into docs/original/db/ or the directory given:
#   tools/defs/describe_mssql.sh [out dir]
#
#   <db>.columns.txt  table|ordinal|column|type|NOT NULL or NULL|IDENTITY or empty|default|row count
#   <db>.indexes.txt  index|table|primary key (1/0)|unique (1/0)|CLUSTERED or NONCLUSTERED     (heaps are not listed)
#   <db>.procs.txt    the stored procedures, by name
#
# Tables are in name order, columns in ordinal order. Structure only: no player data is read (the row count comes from
# sys.partitions). Read-only: every query is a SELECT on the catalogue. tools/defs/generate.py and verify.py read
# PS_GameDefs.columns.txt.
set -eu
root="$(cd "$(dirname "$0")/../.." && pwd)"
. "$root/tools/dbconn.sh"
out="${1:-$root/docs/original/db}"
mkdir -p "$out"

# one query, run in the container as sa; the password never leaves it. Output without headers, trimmed, '|' between fields.
run() { # database, query
    docker exec -e DB="$1" -e SQL="$2" "$MSSQL_CONTAINER" bash -c \
        '/opt/mssql-tools18/bin/sqlcmd -C -S localhost -U sa -P "$MSSQL_SA_PASSWORD" -d "$DB" -h -1 -W -w 4000 -s "|" -Q "SET NOCOUNT ON; $SQL"' \
        | sed '/^$/d'
}

columns="
SELECT t.name, c.column_id, c.name,
       CASE WHEN ty.name IN ('char','varchar','binary','varbinary') THEN ty.name + '(' + CASE WHEN c.max_length = -1 THEN 'max' ELSE CAST(c.max_length AS varchar(10)) END + ')'
            WHEN ty.name IN ('nchar','nvarchar') THEN ty.name + '(' + CASE WHEN c.max_length = -1 THEN 'max' ELSE CAST(c.max_length / 2 AS varchar(10)) END + ')'
            WHEN ty.name IN ('decimal','numeric') THEN ty.name + '(' + CAST(c.precision AS varchar(10)) + ',' + CAST(c.scale AS varchar(10)) + ')'
            ELSE ty.name END,
       CASE WHEN c.is_nullable = 1 THEN 'NULL' ELSE 'NOT NULL' END,
       CASE WHEN c.is_identity = 1 THEN 'IDENTITY' ELSE '' END,
       ISNULL(dc.definition, ''),
       (SELECT ISNULL(SUM(p.rows), 0) FROM sys.partitions p WHERE p.object_id = t.object_id AND p.index_id IN (0, 1))
FROM sys.tables t
JOIN sys.columns c ON c.object_id = t.object_id
JOIN sys.types ty ON ty.user_type_id = c.user_type_id
LEFT JOIN sys.default_constraints dc ON dc.object_id = c.default_object_id
ORDER BY t.name, c.column_id"

indexes="
SELECT i.name, t.name, CAST(i.is_primary_key AS int), CAST(i.is_unique AS int), i.type_desc
FROM sys.indexes i
JOIN sys.tables t ON t.object_id = i.object_id
WHERE i.type > 0
ORDER BY t.name, i.name"

procs="
SELECT name FROM sys.procedures ORDER BY name"

for db in $(docker exec "$MSSQL_CONTAINER" bash -c '/opt/mssql-tools18/bin/sqlcmd -C -S localhost -U sa -P "$MSSQL_SA_PASSWORD" -h -1 -W -Q "SET NOCOUNT ON; SELECT name FROM sys.databases WHERE database_id > 4 ORDER BY name"' | sed '/^$/d'); do
    run "$db" "$columns" > "$out/$db.columns.txt"
    run "$db" "$indexes" > "$out/$db.indexes.txt"
    run "$db" "$procs"   > "$out/$db.procs.txt"
    echo "$db: $(wc -l < "$out/$db.columns.txt") columns, $(wc -l < "$out/$db.indexes.txt") indexes, $(wc -l < "$out/$db.procs.txt") procedures"
done
