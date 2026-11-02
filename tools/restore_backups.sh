#!/bin/bash
# Restore every SQL Server backup in /backups (mounted read-only) into the
# SQL Server container (MSSQL_CONTAINER in .env, default shaiya-afterglow-mssql: tools/dbconn.sh). Runs INSIDE the container:
#   docker cp tools/restore_backups.sh "$MSSQL_CONTAINER":/tmp/ && docker exec "$MSSQL_CONTAINER" bash /tmp/restore_backups.sh
# (tools/setup.sh does this for you.)
# Existing databases are skipped.
export SQLCMDPASSWORD="$MSSQL_SA_PASSWORD"
SQL="/opt/mssql-tools18/bin/sqlcmd -C -S localhost -U sa -W -h -1 -s |"

for f in /backups/*; do
  db=$(basename "$f")
  exists=$($SQL -Q "SET NOCOUNT ON; SELECT COUNT(*) FROM sys.databases WHERE name='$db'" | tr -d ' \r')
  if [ "$exists" != "0" ]; then echo "skip $db (already restored)"; continue; fi
  moves=""
  while IFS='|' read -r logical physical type rest; do
    [ -z "$logical" ] && continue
    ext=mdf; [ "$type" = "L" ] && ext=ldf
    moves="$moves${moves:+, }MOVE '$logical' TO '/var/opt/mssql/data/${db}_${logical}.${ext}'"
  done < <($SQL -Q "SET NOCOUNT ON; RESTORE FILELISTONLY FROM DISK='$f'")
  echo "== restoring $db"
  $SQL -Q "RESTORE DATABASE [$db] FROM DISK='$f' WITH $moves, REPLACE" 2>&1 | grep -vE 'upgrade step|^$' | tail -3 | cut -c1-200
done
