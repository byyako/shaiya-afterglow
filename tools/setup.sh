#!/bin/bash
# Everything from a fresh clone to a running server, in one go:
#   tools/setup.sh <PSM_Data folder> <PS_Game Data folder>
# - writes .env with random passwords when there is none;
# - starts PostgreSQL (compose.yaml) and builds the workspace; with the binaries already in target/release/ it uses them, and
#   without Rust it downloads the release that matches this checkout (tools/fetch-release.sh);
# - reads the game definitions: straight from the PS_GameDefs backup (tools/defs/read_bak.py, no SQL Server), or from a folder of
#   already exported tables (the first argument holds items.psv and the others), or, with --sqlserver, through a SQL Server
#   container (compose profile `import`; the way when the direct reader does not understand your backup);
# - imports the original data ONCE into frozen copies (original_defs, original_world), checks the definitions, and builds the live `defs`
#   and `world` tables from them, the installed extensions and the data editor's edits (data-editor content rebuild);
# - installs the extensions named with --extension, and writes config/game.toml when it is missing;
# - on a terminal, offers to create the first account and the game server's entry and to start both servers.
# Options: --sqlserver  use the SQL Server container for the definitions
#          --no-account  do not offer the account, server entry and start at the end
#          --reset  import the original files again (corrected files): replaces the frozen copies and rebuilds `defs` and `world` from
#                   them, the extensions and the edits (stop the game server first)
#          --extension <folder>  install the extension in that folder after the import (repeatable); skipped when that version is installed
# Stops at the first failure and names the step. Safe to run again: what is imported from the same files is skipped ("already imported,
# nothing to do"), other files are refused until --reset, an extension already installed is skipped, config/game.toml is kept.
# docs/plans/ext-1-foundation.md sections 3.2 and 7.2.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

usage() { echo "usage: $0 [--sqlserver] [--no-account] [--reset] [--extension <folder>]... <PSM_Data folder | exported definitions folder> <PS_Game/Data folder>" >&2; exit 2; }
use_sqlserver=0 offer_account=1 reset=0 args=() extensions=()
while [ $# -gt 0 ]; do
    case "$1" in
        --sqlserver) use_sqlserver=1 ;;
        --no-account) offer_account=0 ;;
        --reset) reset=1 ;;
        --extension) [ $# -ge 2 ] || usage; extensions+=("$2"); shift ;;
        -*) usage ;;
        *) args+=("$1") ;;
    esac
    shift
done
[ ${#args[@]} -eq 2 ] || usage
for folder in "${extensions[@]+"${extensions[@]}"}"; do
    [ -f "$folder/extension.toml" ] || { echo "$folder: not an extension folder (no extension.toml)" >&2; exit 2; }
done

current="checking the prerequisites"
trap 'echo >&2; echo "setup failed at: $current" >&2; echo "(run it again after fixing the cause; finished steps are repeated safely)" >&2' ERR
step() { current="$1"; echo; echo "== $1"; }
fail() { echo "$1" >&2; exit 1; }

# --- the .env ------------------------------------------------------------------------------------------------------------
if [ ! -f .env ]; then
    echo "no .env yet: writing one from .env.example with random passwords"
    command -v python3 > /dev/null || fail "missing: python3 (from your distribution's packages)"
    python3 - <<'PY'
import re, secrets, string
alnum = string.ascii_letters + string.digits
pg = "".join(secrets.choice(alnum) for _ in range(24))
ms = "Aa1" + "".join(secrets.choice(alnum) for _ in range(21))   # SQL Server wants upper, lower and a digit
text = open(".env.example").read()
text = text.replace("postgres://shaiya:change-me@", f"postgres://shaiya:{pg}@")
text = re.sub(r"(?m)^PG_PASSWORD=.*$", f"PG_PASSWORD={pg}", text)
text = re.sub(r"(?m)^MSSQL_SA_PASSWORD=.*$", f"MSSQL_SA_PASSWORD={ms}", text)
open(".env", "w").write(text)
PY
    chmod 600 .env
fi

# --- the prerequisites -----------------------------------------------------------------------------------------------------
step "checking the prerequisites"
missing=0
need() { command -v "$1" > /dev/null || { echo "missing: $1 ($2)" >&2; missing=1; }; }
need docker "Docker, https://docs.docker.com/engine/install/"
need python3 "Python 3, from your distribution's packages"
if command -v docker > /dev/null; then
    docker compose version > /dev/null 2>&1 \
        || { echo "missing: docker compose (the Compose plugin, https://docs.docker.com/compose/install/)" >&2; missing=1; }
    docker info > /dev/null 2>&1 \
        || { echo "Docker is installed but not usable by this user: start the service, or add yourself to the docker group (sudo usermod -aG docker \$USER, then log in again)" >&2; missing=1; }
fi
binaries=(login-server game-server servers create_account console data-editor mall)
have_binaries=1
for b in "${binaries[@]}"; do [ -x "target/release/$b" ] || have_binaries=0; done
if [ "$have_binaries" = 0 ] && ! command -v cargo > /dev/null; then
    # no Rust: take the binaries of the release that matches this checkout
    version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
    echo "Rust (cargo) is not installed: downloading the binaries of release v$version instead (tools/fetch-release.sh)"
    if tools/fetch-release.sh "v$version"; then
        have_binaries=1
        for b in "${binaries[@]}"; do [ -x "target/release/$b" ] || have_binaries=0; done
    fi
    [ "$have_binaries" = 1 ] || need cargo "Rust 1.94 or newer, https://rustup.rs; no release v$version could be downloaded either"
fi
[ "$missing" = 0 ] || fail "install what is missing above, then run $0 again"

first="$(cd "${args[0]}" 2> /dev/null && pwd)" || fail "${args[0]}: not a folder (the PSM_Data folder of the original server files)"
game_data="$(cd "${args[1]}" 2> /dev/null && pwd)" || fail "${args[1]}: not a folder (the PS_Game/Data folder of the original server files)"
[ -f "$game_data/Map.ini" ] || fail "$game_data has no Map.ini: give the PS_Game/Data folder of the original server files (in the EP4.5 pack: PSM_Server/ShaiyaServerPatch/Server/GameMain/PS_Game/Data)"
if [ -f "$first/items.psv" ]; then
    source_kind=export
    use_sqlserver=0
elif [ -f "$first/PS_GameDefs" ]; then
    source_kind=backup
else
    fail "$first has neither a PS_GameDefs backup nor an items.psv export: give the PSM_Data folder of the original server files"
fi

. "$root/tools/dbconn.sh"
[ -n "${DATABASE_URL:-}" ] || fail ".env: DATABASE_URL is not set"
[ -n "${PG_PASSWORD:-}" ] || fail ".env: PG_PASSWORD is not set (the password in DATABASE_URL)"
url_password="$(echo "$DATABASE_URL" | sed -nE 's#^postgres(ql)?://[^:/@]+:([^@]*)@.*#\2#p')"
[ "$url_password" = "$PG_PASSWORD" ] || fail ".env: PG_PASSWORD differs from the password in DATABASE_URL"
if [ "$use_sqlserver" = 1 ]; then
    [ -n "${MSSQL_SA_PASSWORD:-}" ] || fail ".env: MSSQL_SA_PASSWORD is not set (needed for --sqlserver)"
    export PSM_DATA_DIR="$first"
fi
if [ "$source_kind" = export ]; then how="the exported tables"; source_text="export folder $first"
elif [ "$use_sqlserver" = 1 ]; then how="the backup, through SQL Server"; source_text="PS_GameDefs backup $(sha256sum "$first/PS_GameDefs" | cut -d' ' -f1), through SQL Server"
else how="the backup, read directly"; source_text="PS_GameDefs backup $(sha256sum "$first/PS_GameDefs" | cut -d' ' -f1), read directly (tools/defs/read_bak.py)"; fi
echo "ok (PostgreSQL container $PG_CONTAINER; definitions from $how)"

# --- PostgreSQL --------------------------------------------------------------------------------------------------------------
step "starting PostgreSQL (docker compose up -d postgres)"
_dbconn_parse_url
pg_port="${PG_PORT:-5433}"
if ! docker inspect "$PG_CONTAINER" > /dev/null 2>&1 && command -v ss > /dev/null && ss -ltn "sport = :$pg_port" 2> /dev/null | grep -q LISTEN; then
    fail "port $pg_port is already in use on this machine: stop what uses it, or set PG_PORT in .env (and the same port in DATABASE_URL)"
fi
docker compose up -d postgres
pg_ready() { docker exec "$PG_CONTAINER" pg_isready -q -h 127.0.0.1 -p 5432 -U "$PG_USER" -d "$PG_DB" > /dev/null 2>&1; }
for _ in $(seq 60); do
    pg_ready && break
    sleep 2
done
pg_ready || fail "PostgreSQL did not become ready within two minutes (docker logs $PG_CONTAINER)"

# --- the binaries -------------------------------------------------------------------------------------------------------------
if [ "$have_binaries" = 1 ]; then
    step "using the binaries in target/release/"
else
    step "building the workspace (cargo build --release --workspace; a few minutes the first time)"
    cargo build --release --workspace
fi

step "creating the tables (target/release/servers list)"
target/release/servers list

# --- the game definitions -----------------------------------------------------------------------------------------------------
export_dir="$root/data/defs-export"
if [ "$source_kind" = export ]; then
    step "using the exported definitions in $first"
    export_dir="$first"
elif [ "$use_sqlserver" = 0 ]; then
    step "reading the game definitions from the backup (tools/defs/read_bak.py)"
    if ! python3 -I tools/defs/read_bak.py "$first/PS_GameDefs" "$export_dir"; then
        echo >&2
        echo "the direct reader could not read this backup (message above). Run again with --sqlserver to restore it in a" >&2
        echo "SQL Server container instead, and please report the message: an issue on GitHub." >&2
        false
    fi
fi

if [ "$use_sqlserver" = 1 ]; then
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

    step "restoring the SQL Server backups (tools/restore_backups.sh; about 20 minutes)"
    docker cp tools/restore_backups.sh "$MSSQL_CONTAINER":/tmp/
    docker exec "$MSSQL_CONTAINER" bash /tmp/restore_backups.sh

    step "exporting the game definitions (tools/defs/export_from_mssql.sh)"
    tools/defs/export_from_mssql.sh
fi

reset_flag=()
if [ "$reset" = 1 ]; then
    reset_flag=(--reset)
    echo "--reset: the original files are imported again; stop the game server first (tools/game-server.sh stop)"
fi
# When the importer wrote a new import record (rather than finding the same files imported), the checks run.
import_stamp() { psql_cmd -At -c "SELECT coalesce((SELECT imported_at::text FROM content.imports WHERE part = '$1'), '')"; }

step "importing the game definitions once (tools/defs/load_into_postgres.py)"
stamp="$(import_stamp defs)"
python3 tools/defs/load_into_postgres.py "$export_dir" db/defs/schema.sql --source "$source_text" "${reset_flag[@]+"${reset_flag[@]}"}"
if [ "$(import_stamp defs)" != "$stamp" ]; then
    if [ "$use_sqlserver" = 1 ]; then
        step "verifying the game definitions against SQL Server (tools/defs/verify.py)"
        python3 tools/defs/verify.py db/defs/schema.sql docs/original/db/PS_GameDefs.columns.txt
    else
        step "checking the row counts of the game definitions (tools/defs/verify.py --counts-only)"
        python3 tools/defs/verify.py db/defs/schema.sql docs/original/db/PS_GameDefs.columns.txt --counts-only
    fi
fi

step "importing the world once (tools/world/import.py)"
python3 tools/world/import.py "$game_data" --source "$game_data" "${reset_flag[@]+"${reset_flag[@]}"}"

if [ "$use_sqlserver" = 1 ]; then
    step "stopping SQL Server (docker compose stop mssql)"
    docker compose stop mssql
fi

for folder in "${extensions[@]+"${extensions[@]}"}"; do
    step "installing the extension $folder (data-editor ext install)"
    target/release/data-editor ext install --skip-installed "$folder"
done

# config/game.toml: the game server's settings (docs/server/settings.md); written once, never overwritten
if [ ! -f config/game.toml ]; then
    mkdir -p config
    cat > config/game.toml <<'TOML'
# The game server's settings (docs/server/settings.md). With nothing set here, the server behaves as the original.
# Every setting with its default and meaning: config/game.toml.example (or: target/release/game-server config --example).
# What is in force and where each value comes from: target/release/game-server config.
# Changes take effect at the next start: tools/game-server.sh restart.
TOML
    echo "wrote config/game.toml (settings are comments in it until you set them)"
fi

trap - ERR
echo
echo "== the data is in place"

# --- the first account, the server entry, the start ----------------------------------------------------------------------------
lan_ip="$(hostname -I 2> /dev/null | tr ' ' '\n' | grep -E '^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$' | head -1 || true)"
next_steps() {
    cat <<EOF

Left to do:

  SHAIYA_PASSWORD='secret123' target/release/create_account myname 16    # 16 = full GM; leave it out for a player
  target/release/servers add 1 MyServer ${lan_ip:-203.0.113.10}                       # id, name, the address the clients connect to

  tools/login-server.sh start
  tools/game-server.sh start

Then point the client at the server: python3 tools/patch_client_ip.py <Client folder> ${lan_ip:-<server ip>}
EOF
    cat <<'EOF'

Extensions (new items, rules, scripts, pets) are in their own repository, https://github.com/byyako/shaiya-afterglow-extensions:
take the release with the same minor version as this server, then for each one you want
  target/release/data-editor ext install <its folder>    # and restart the game server
EOF
}

if [ "$offer_account" = 0 ] || [ ! -t 0 ]; then
    next_steps
    exit 0
fi

echo
read -r -p "Create your first account and start the servers now? [Y/n] " answer
case "${answer:-y}" in [nN]*) next_steps; exit 0 ;; esac

while :; do
    read -r -p "Account name (up to 12 characters): " account
    [ -n "$account" ] && [ "${#account}" -le 12 ] && break
    echo "1 to 12 characters, please"
done
while :; do
    read -r -s -p "Password (6 to 16 characters): " password; echo
    [ "${#password}" -ge 6 ] && [ "${#password}" -le 16 ] && break
    echo "6 to 16 characters, please"
done
read -r -p "Make it a full GM account? [Y/n] " gm
status=16; case "${gm:-y}" in [nN]*) status=0 ;; esac
read -r -p "The address players' clients connect to [${lan_ip:-127.0.0.1}]: " address
address="${address:-${lan_ip:-127.0.0.1}}"
read -r -p "Name of the game server [Afterglow]: " server_name
server_name="${server_name:-Afterglow}"

SHAIYA_PASSWORD="$password" target/release/create_account "$account" "$status" \
    || echo "(the account was not created, see above; setup goes on)"
if target/release/servers list | grep -Eq '^ *1 '; then
    echo "game server 1 is already in the list"
else
    target/release/servers add 1 "$server_name" "$address"
fi
tools/login-server.sh start
tools/game-server.sh start
cat <<EOF

The servers are running (tools/game-server.sh logs shows the log; stop and status work the same way).
Now point the client at $address:

  python3 tools/patch_client_ip.py <Client folder> $address

and log in as $account. More: README.md, docs/original/client-login.md.
EOF
