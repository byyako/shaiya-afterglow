#!/bin/bash
# The README's setup steps 2 to 5 in one go, from a fresh clone with its .env filled in:
#   tools/setup.sh <PSM_Data folder> <PS_Game Data folder>
# Starts PostgreSQL (compose.yaml), builds the workspace, creates the tables, starts SQL Server (the compose `import` profile),
# restores the backups, exports, loads and verifies the game definitions, applies the operator's edits (tools/defs/additions.sh,
# without --with-additions), imports the world and stops SQL Server again. Stops at the first failure and names the step.
# Safe to run again: the `defs` and `world` schemas are replaced, and databases already restored are skipped.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

usage() { echo "usage: $0 <PSM_Data folder> <PS_Game Data folder>" >&2; exit 2; }
[ $# -eq 2 ] || usage

current="checking the prerequisites"
trap 'echo >&2; echo "setup failed at: $current" >&2' ERR
step() { current="$1"; echo; echo "== $1"; }
fail() { echo "$1" >&2; exit 1; }

step "checking the prerequisites"
missing=0
need() { command -v "$1" > /dev/null || { echo "missing: $1 ($2)" >&2; missing=1; }; }
need docker "Docker, https://docs.docker.com/engine/install/"
need cargo "Rust 1.94 or newer, https://rustup.rs"
need python3 "Python 3, from your distribution's packages"
if command -v docker > /dev/null && ! docker compose version > /dev/null 2>&1; then
    echo "missing: docker compose (the Compose plugin, https://docs.docker.com/compose/install/)" >&2; missing=1
fi
[ -f .env ] || { echo "missing: .env (cp .env.example .env, then set the passwords: README step 1)" >&2; missing=1; }
[ "$missing" = 0 ] || fail "install or create what is missing above, then run $0 again"

psm_data="$(cd "$1" 2> /dev/null && pwd)" || fail "$1: not a folder (the PSM_Data folder of the original server files)"
game_data="$(cd "$2" 2> /dev/null && pwd)" || fail "$2: not a folder (the PS_Game/Data folder of the original server files)"
[ -f "$psm_data/PS_GameDefs" ] || fail "$psm_data has no PS_GameDefs backup: give the PSM_Data folder of the original server files"
[ -f "$game_data/Map.ini" ] || fail "$game_data has no Map.ini: give the PS_Game/Data folder of the original server files"

. "$root/tools/dbconn.sh"
[ -n "${DATABASE_URL:-}" ] || fail ".env: DATABASE_URL is not set"
[ -n "${PG_PASSWORD:-}" ] || fail ".env: PG_PASSWORD is not set (the password in DATABASE_URL)"
[ -n "${MSSQL_SA_PASSWORD:-}" ] || fail ".env: MSSQL_SA_PASSWORD is not set"
url_password="$(echo "$DATABASE_URL" | sed -nE 's#^postgres(ql)?://[^:/@]+:([^@]*)@.*#\2#p')"
[ "$url_password" = "$PG_PASSWORD" ] || fail ".env: PG_PASSWORD differs from the password in DATABASE_URL"
export PSM_DATA_DIR="$psm_data"
echo "ok (PostgreSQL container $PG_CONTAINER, SQL Server container $MSSQL_CONTAINER)"

step "starting PostgreSQL (docker compose up -d postgres)"
docker compose up -d postgres
_dbconn_parse_url
pg_ready() { docker exec "$PG_CONTAINER" pg_isready -q -h 127.0.0.1 -p 5432 -U "$PG_USER" -d "$PG_DB" > /dev/null 2>&1; }
for _ in $(seq 60); do
    pg_ready && break
    sleep 2
done
pg_ready || fail "PostgreSQL did not become ready within two minutes (docker logs $PG_CONTAINER)"

step "building the workspace (cargo build --release --workspace)"
cargo build --release --workspace

step "creating the tables (target/release/servers list)"
target/release/servers list

step "starting SQL Server (docker compose --profile import up -d mssql)"
docker compose --profile import up -d mssql
echo "waiting for SQL Server (up to 10 minutes)"
ready=0
for _ in $(seq 60); do
    if docker exec "$MSSQL_CONTAINER" bash -c '/opt/mssql-tools18/bin/sqlcmd -C -S localhost -U sa -P "$MSSQL_SA_PASSWORD" -Q "SELECT 1"' > /dev/null 2>&1; then
        ready=1; break
    fi
    sleep 10
done
[ "$ready" = 1 ] || fail "SQL Server did not answer within 10 minutes (docker logs $MSSQL_CONTAINER)"

step "restoring the SQL Server backups (tools/restore_backups.sh)"
docker cp tools/restore_backups.sh "$MSSQL_CONTAINER":/tmp/
docker exec "$MSSQL_CONTAINER" bash /tmp/restore_backups.sh

step "exporting the game definitions (tools/defs/export_from_mssql.sh)"
tools/defs/export_from_mssql.sh

step "loading the game definitions (tools/defs/load_into_postgres.py)"
python3 tools/defs/load_into_postgres.py data/defs-export db/defs/schema.sql

step "verifying the game definitions against SQL Server (tools/defs/verify.py)"
python3 tools/defs/verify.py db/defs/schema.sql docs/original/db/PS_GameDefs.columns.txt

step "applying the operator's edits (tools/defs/additions.sh)"
tools/defs/additions.sh

step "importing the world (tools/world/import.py)"
python3 tools/world/import.py "$game_data"

step "stopping SQL Server (docker compose stop mssql)"
docker compose stop mssql

trap - ERR
cat <<EOF

== done
Left to do (README step 6), then start the servers (step 7):

  SHAIYA_PASSWORD='secret123' target/release/create_account myname 16    # 16 = full GM; leave it out for a player
  target/release/servers add 1 MyServer 203.0.113.10                         # the address the clients connect to

  tools/login-server.sh start
  tools/game-server.sh start
EOF
