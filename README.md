# Shaiya Afterglow

[![CI](https://github.com/byyako/shaiya-afterglow/actions/workflows/ci.yml/badge.svg)](https://github.com/byyako/shaiya-afterglow/actions/workflows/ci.yml)

A Shaiya Episode 4.5 server written from scratch in Rust with PostgreSQL. It works with the unmodified US client (`game.exe` of
2010-04-26, patch 108) and is built from reverse engineering of the original EP4.5 server programs (`ps_login.exe`, `ps_game.exe`
and their neighbours) and public documentation. No original code, binaries or game data are included: you bring your own copy of
the original server files, and the import tools load them into PostgreSQL.

The aim is the original game as it was, without its bugs: the original's rules, numbers and packets, with its crashes, duplication
holes and exploits fixed. Every place where the new server knowingly behaves differently is written down in the feature's analysis note
(`docs/original/*.md`, the "deviations" lists), and the ones that change gameplay are settings (`docs/server/settings.md`).

- [Quick start](#quick-start)
- [Status](#status)
- [What you need](#what-you-need)
- [Setting up from a fresh clone](#setting-up-from-a-fresh-clone)
- [Running the servers](#running-the-servers)
- [Operator tools](#operator-tools)
- [Editing the game data](#editing-the-game-data)
- [Updating](#updating)
- [Development](#development)
- [Layout](#layout)
- [Documentation](#documentation)
- [Legal](#legal)

## Quick start

You need a Linux machine with Docker (and its Compose plugin) and Python 3, and your own copy of the original EP4.5 server files
(see [What you need](#what-you-need)). Rust is not needed: without it, the setup downloads the binaries of the matching release. Then:

```bash
git clone https://github.com/byyako/shaiya-afterglow.git && cd shaiya-afterglow
tools/setup.sh /path/to/PSM_Data /path/to/PS_Game/Data
```

`tools/setup.sh` does the rest: it writes a `.env` with random passwords, starts PostgreSQL in Docker, builds the server (a few minutes
the first time; with no Rust installed it downloads the release's binaries instead), reads the game definitions straight out of the original backup, imports the original data once and builds the game's tables from it, writes
`config/game.toml`, and then asks for the name and password of your first account and the address your players use, and starts the servers.
Without the build it takes about a minute. If it stops, it names the step; run it again after fixing the cause (a re-run skips what is
already imported). `--extension extensions/kill-boost` installs the Kill Boost Stones on the way (below), `--reset` imports corrected original
files again.

Last, point the client at your server: `python3 tools/patch_client_ip.py <Client folder> <server ip>` writes a copy of
`game.exe` that connects to it ([details](docs/original/client-login.md)). Log in with the account you made. The rest of this README
is the same path step by step, and everything after it.

## Status

Version 0.2.0 ([changelog](CHANGELOG.md)). Every game system of the original EP4.5 server is built: login and the server list; the character screen; the world, maps,
portals and gatekeepers; chat; NPCs and shops; items, the warehouse, repair, lapis and enchanting; stats, experience and death; monsters
with their AI, paths, skills and scripted bosses; combat and every skill type; parties and union parties; trade; guilds, guild houses,
the guild bank and the ranking battle; quests and quest drops; friends and memos; PvP, kill counts, duels and guild wars; obelisks and the
boss spawns of `Obelisk.ini`; party instance zones with their Lua bosses; personal shops and the auction house; mounts; the item mall and
premium accounts; the gambling house; the game and chat log; the operator (GM) commands and the operator console.

Not built, by decision: the second game server for map 200 (`ps_gamewar.exe`; nothing in the shipped data leads there), the original
console's `/reitem`-style live reloads (the `reload` console command covers monsters and drop tables), the obelisks' road portal links
(the shipped `Obelisk.ini` defines none) and a global chat without its item (`docs/server/deviations.md` section 8).

Many of the systems have been tested with the real client; the rest are covered by the test suite and still wait for a test in play.
`docs/testing/live-test-checklist.md` lists each untested item; if you try one, a report of what happened is welcome (an issue,
`CONTRIBUTING.md` "Reporting a bug"). With only `DATABASE_URL` set, the
server behaves as the original apart from its fixes and the operator's tools: `docs/server/deviations.md` lists every deviation, and in its
section 1 the settings that turn on what the original lacks (cross-faction trade, a market-map limit, the Kill Boost Stones). One
addition is on by default: the gambling house, which the original's data places but its server never shows (`GAME_MINI_GAME`, with
the original's odds).

## What you need

- **Linux** (developed on Debian 13; any recent distribution with Docker should do). Windows hosts are not tested.
- **Docker** (with the Compose plugin), for PostgreSQL. SQL Server is no longer needed: the game definitions are read directly from the backup
  (`tools/setup.sh --sqlserver` restores them in a SQL Server container instead, about 20 minutes and 2 GB of memory).
- **Rust 1.94 or newer** (edition 2024; `sqlx` 0.9 needs 1.94), from [rustup](https://rustup.rs); `rustup update` brings an older one up. Building needs about 2 GB of memory. Not needed if you put the binaries of a release in `target/release/` (`tools/fetch-release.sh`).
- **Python 3** (standard library only), for the import scripts.
- **The original EP4.5 server files**: the SQL Server backups in `PSM_Data/` (only `PS_GameDefs` is used) and the game server's data
  folder `PS_Game/Data/` (`Map.ini`, the svmaps, `NpcQuest.SData`, `Obelisk.ini`, `ItemCreate.ini`, the Lua scripts and the rest).
  The tools were written against the widely shared EP4.5 server pack (binaries of mid-2009, pack of 2010-03). They are not
  distributed here.
- **The US client**, patch 108, to play.

Ports: login 30800 and game 30810 (TCP). The client always connects to the game server on 30810, so each game server needs its own
IPv4 address.

## Setting up from a fresh clone

The commands run from the repository root. The import scripts find the databases through `.env`: `DATABASE_URL`, and the
container names `PG_CONTAINER` and `MSSQL_CONTAINER` (default `shaiya-afterglow-postgres` and `shaiya-afterglow-mssql`, the names below;
`docs/server/settings.md`, "Tools").

These are the steps `tools/setup.sh` ([Quick start](#quick-start)) runs, one at a time, for doing them by hand or finding which one failed.
`tools/setup.sh` can be run again: it skips what is imported from the same files, refuses other files until `--reset`, and keeps `config/game.toml`.
`--sqlserver` makes step 4 use SQL Server (see there).

### 1. Clone and configure

```bash
git clone https://github.com/byyako/shaiya-afterglow.git && cd shaiya-afterglow
cp .env.example .env        # then set the passwords in it (letters and digits are safest in the URL)
```

`DATABASE_URL` and `PG_PASSWORD` must hold the same password (the PostgreSQL container is created with `PG_PASSWORD`). Only for the
SQL Server alternative of step 4: `MSSQL_SA_PASSWORD` needs at least 8 characters with upper case, lower case and a digit (SQL Server
refuses weaker ones), and `PSM_DATA_DIR` the folder of the backups.

### 2. PostgreSQL

```bash
docker compose up -d postgres
```

or, without the compose file:

```bash
set -a; . ./.env; set +a
docker run -d --name shaiya-afterglow-postgres --restart unless-stopped \
  -e POSTGRES_DB=shaiya -e POSTGRES_USER=shaiya -e POSTGRES_PASSWORD="$PG_PASSWORD" \
  -p 127.0.0.1:5433:5432 -v shaiya-afterglow-pgdata:/var/lib/postgresql/data postgres:16
```

The first start takes a few seconds to create the database.

### 3. Build

```bash
cargo build --release --workspace
```

The binaries land in `target/release/`: `login-server`, `game-server`, `data-editor` and the tools `create_account`, `servers`,
`console` and `mall`. Each applies the database migrations (`db/migrations/`) when it starts, so the first run of any of them creates the
tables. Do that now:

```bash
target/release/servers list        # creates the tables; prints an empty list
```

### 4. The game definitions (`PS_GameDefs` into the `defs` schema)

The definitions (items, monsters, skills, drops, ...) are in the original SQL Server backup `PSM_Data/PS_GameDefs`. `read_bak.py` reads that
file directly (it is an uncompressed SQL Server 2005 backup) and writes the tables to `data/defs-export/` (git-ignored; it is the game's data):

```bash
python3 tools/defs/read_bak.py /path/to/PSM_Data/PS_GameDefs data/defs-export
python3 tools/defs/load_into_postgres.py data/defs-export db/defs/schema.sql
python3 tools/defs/verify.py db/defs/schema.sql docs/original/db/PS_GameDefs.columns.txt --counts-only   # row counts against the known ones
```

The import is made once, into the frozen copy `original_defs`, and ends with `target/release/data-editor content rebuild defs` (the live `defs` table
is the originals, the installed extensions and your data editor edits, in that order; `docs/original/database-notes.md`). The server's own
extra rows, the Kill Boost Stones, are an extension: `target/release/data-editor ext install extensions/kill-boost`
(`docs/server/deviations.md` section 1).

`load_into_postgres.py` decodes the text columns as GBK, the original database's code page, and checks that every value encodes back to
the same bytes. `docs/original/database-notes.md` describes the backups. If you already have the tables as text (an earlier export, or
a friend's), put them in `data/defs-export/` and skip the first command.

**Through SQL Server instead.** If `read_bak.py` stops with a message (a backup in another version or form than the one it was written
against; please report it), SQL Server restores the backup itself in a container used just for this step (about 20 minutes, 2 GB of memory), and
the export is its own:

```bash
docker compose --profile import up -d mssql              # PSM_DATA_DIR in .env names the folder of the backups
# wait a minute or two: "An error occurred while evaluating the password" or "Login failed" means it is still starting
docker cp tools/restore_backups.sh shaiya-afterglow-mssql:/tmp/ && docker exec shaiya-afterglow-mssql bash /tmp/restore_backups.sh   # about 20 minutes
tools/defs/export_from_mssql.sh                                          # to data/defs-export/
python3 tools/defs/load_into_postgres.py data/defs-export db/defs/schema.sql
python3 tools/defs/verify.py db/defs/schema.sql docs/original/db/PS_GameDefs.columns.txt   # row counts and sums against SQL Server
docker compose stop mssql                                                # or docker stop shaiya-afterglow-mssql; docker rm once the import is checked
```

(Without the compose file, `docker run -d --name shaiya-afterglow-mssql -e ACCEPT_EULA=Y -e MSSQL_PID=Developer -e MSSQL_SA_PASSWORD="$MSSQL_SA_PASSWORD" -e MSSQL_MEMORY_LIMIT_MB=1400 -p 127.0.0.1:14333:1433 -v /path/to/PSM_Data:/backups:ro -v shaiya-afterglow-mssql-data:/var/opt/mssql mcr.microsoft.com/mssql/server:2022-latest`
starts the same container; with other container names, put `$MSSQL_CONTAINER` in place of `shaiya-afterglow-mssql`.)

### 5. The world (`PS_Game/Data` into the `world` schema)

```bash
python3 tools/world/import.py "/path/to/PS_Game/Data"
```

In the EP4.5 pack the folder is `PSM_Server/ShaiyaServerPatch/Server/GameMain/PS_Game/Data`. The import reads the maps, spawns, NPCs,
quests, obelisks, boss groups, guild houses, scripted monsters and Lua scripts, and replaces the `world` schema. Run it again after
editing one of those files, then restart the game server.

### 6. An account and a game server entry

```bash
SHAIYA_PASSWORD='secret123' target/release/create_account myname 16    # 16 = full GM; leave it out (or 0) for a player
target/release/servers add 1 MyServer 203.0.113.10                         # id, name, the address clients connect to
```

A user name has up to 12 characters, a password 6 to 16. The address in `servers add` is the one players' clients reach: a LAN address
for a home test, the public one on a VPS.

### 7. Start

```bash
tools/login-server.sh start
tools/game-server.sh start
tools/game-server.sh logs        # the last lines of logs/game-server.log
```

The game server loads the definitions and the world (a few seconds), then sends a heartbeat. The login server lists it as open while the
heartbeats arrive.

### 8. Connect the client

The client has its login address built in. `tools/patch_client_ip.py` writes a copy of `game.exe` that connects to your server (it checks
the input's hash first and changes only the address string, a 16-byte slot), or a network redirect does the same without touching the client. `docs/original/client-login.md`
explains both and what each error box means. Log in with the account from step 6.

To check the server without a client:

```bash
cargo run --release -p login-server --example probe -- 127.0.0.1:30800 myname secret123            # login and server list
cargo run --release -p game-server --example game_probe -- 127.0.0.1:30800 myname secret123 1      # also the character screen
```

## Running the servers

`tools/login-server.sh` and `tools/game-server.sh` take `start`, `stop`, `restart`, `status` and `logs [lines]`. They read `.env`,
run the release binary in the background and log to `logs/`. `start` builds the binary only when it is missing: after changing the code,
run `cargo build --release --workspace` first.

Stopping the game server (Ctrl+C, SIGTERM or `stop`) saves every character in the world before it exits. With players online, prefer
the countdown: `target/release/console 1 quit 5` announces a stop in five minutes, `restart 5` does the same and exits with status 75, which
the run script and the systemd unit of `docs/server/deploy.md` start again.

All settings are environment variables, listed with their defaults in [`docs/server/settings.md`](docs/server/settings.md). The ones most often
changed: `GAME_EXP_RATE` (experience factor), `GAME_SERVER_ID`, `GAME_BIND` / `LOGIN_BIND`, and `RUST_LOG`.

For a public server on a VPS (systemd units, firewall, backups, updates, copying the game data instead of importing it), see
[`docs/server/deploy.md`](docs/server/deploy.md).

## Operator tools

The tools read `.env` like the servers.

```bash
target/release/create_account <name> [status]          # password from SHAIYA_PASSWORD; status 16 = full GM
target/release/servers list | add <id> <name> <ipv4> [capacity] [version] | remove <id> | enable <id> | disable <id>
target/release/mall points <user> add 1000             # item mall points; also: show, premium <user> <0-3> [days], deliver <user> <code>
target/release/console 1 info                          # the operator console of game server 1
```

`console <server id> <command>` reaches a running game server through PostgreSQL `NOTIFY`. Commands: `help`, `info`, `notice <text>`,
`quit [minutes]` / `quit cancel`, `restart [minutes]`, `reload`, `allout`, `close` / `open`, `maxusers <n>`,
`kick char <name>` (or `user`, `userid`, `charid`), `exp <factor>`, `killrate <1-100>`, `shop on|off|silent`, `killcount on|off`, `zones`,
`map <id>`; `help <command>` explains one (`docs/original/operator-console.md`). Staff
characters can type the same commands in the game as `.help`, `.info` and so on.

In the game, GM accounts (status 16) have the original's slash commands: moves, `/set`, items, monsters and NPCs, bans, silence, kick,
summon, the character sheet, quests and the rest (`docs/original/gm-commands.md`, `docs/original/gm-commands-2.md`).

## Editing the game data

`data-editor` changes the game data in place and keeps every change in the `edits` schema: who, when and why, the value before and
after, Undo for any change set. Edits are written again after every import of the original files, so a re-import does not lose them.
It covers drops and gold, monster stats, item grades, spawn areas, obelisks, boss groups and portal destinations
(`docs/plans/done/data-editing.md`).

**The page:** `tools/data-editor.sh start`, then open `http://localhost:30890/` (from another machine: `ssh -L 30890:127.0.0.1:30890
<host>`). A drawn map of each game map with its spawn areas, NPCs, portals, obelisks and bosses; the monsters and their drop tables;
drafts that are checked as you type and applied as one change set with a message; the history with Undo; each game server's status
with "reload" and "restart in N minutes". There is no login: the page only answers loopback, private and Tailscale addresses.

**The command line:**

```bash
target/release/data-editor draft add set mob_drop 200/1 droprate 45       # one draft line, checked at once
target/release/data-editor apply-draft -m "more drops on map 2" --by me   # the draft as one change set; says what to reload or restart
target/release/console 1 reload                                           # drops, gold and monster stats, live
target/release/console 1 restart 5                                        # spawns, obelisks, bosses and portals need a restart
target/release/data-editor history | show <id> | undo <id>
target/release/data-editor export edits.json | import edits.json [--replace]
```

`data-editor --help` lists the rest (draft targets, `status`, `apply`, map pictures).

## Updating

```bash
git pull
cargo build --release --workspace        # or, without Rust: git checkout <release tag> && tools/fetch-release.sh <release tag>
target/release/console 1 restart 5     # or: tools/game-server.sh restart (players are saved, but not warned)
tools/login-server.sh restart
```

New migrations run when the servers start. The game data is not touched by updates; re-run steps 4 and 5 only when the import tools or
your original files change.

## The game data and extensions

The original data is imported **once**, into frozen copies (`original_defs`, `original_world`); the tables the servers read (`defs`, `world`)
are built from them, the installed **extensions** and the data editor's edits, in that order. Nothing hand-edited in `defs` survives a rebuild,
and `data-editor content check` names every row that was changed by hand.

```bash
target/release/data-editor content status                 # what is imported and installed, and whether a client patch is due
target/release/data-editor content check                  # the live tables against a rebuild from the originals, extensions and edits
target/release/data-editor ext install extensions/kill-boost   # an extension: a folder with extension.toml and rows.toml
target/release/data-editor ext list | show <name> | check <folder> | upgrade <folder> | enable <name> | disable <name>
target/release/data-editor ext remove <name>              # every row goes back to the original; refused while players hold its items
```

An extension adds to or changes items, skills and drop chances without SQL; the database keeps everything it read, so removing it needs no
folder. `extensions/kill-boost` (three stones whose buff multiplies player kills) is the first and the example to copy; restart the game servers
after an install. The format, the checks and the commands are in `docs/plans/ext-1-foundation.md` (sections 5 to 7), the schemas in
`docs/original/database-notes.md`.

## Development

```bash
cargo fmt --all                                         # rustfmt.toml: width 140
cargo clippy --workspace --all-targets -- -D warnings   # no warnings allowed
cargo test --workspace          # the PostgreSQL tests use DATABASE_URL from .env and skip themselves without it
```

CI (`.github/workflows/ci.yml`) runs the same three on every push and pull request (`cargo fmt --all -- --check` there), with the tests
against an empty PostgreSQL 16: the tests that need the imported `defs` or `world` skip themselves without them, so only a local run
against a loaded database covers those.

The full suite takes about 25 minutes, because the PostgreSQL suites take turns on the migration lock; `cargo test -p <crate>` runs one
crate. It includes a proptest harness over the packet decoders, the crypto, the game packet dispatch and the login exchange (about 30
seconds). Deep fuzz runs: `PROPTEST_CASES=5000 cargo test --release -p game-server --test fuzz_dispatch -- --include-ignored`, and the
same for `login-server`'s `fuzz_login` (`docs/server/audit.md` section 5).

How a feature is built: the original's behaviour is read from the binaries first and written up in `docs/original/<feature>.md`
(packets, checks, numbers, with the addresses they come from, and any deviation with its reason). The code follows that note.

## Layout

| path | what |
|---|---|
| `crates/shaiya-crypto` | the RSA key exchange, session key derivation and the AES-CTR stream |
| `crates/shaiya-proto` | packet framing and the packet layouts |
| `crates/login-server` | the login server, the operator tools (`create_account`, `servers`, `console`, `mall`) and the `probe` client |
| `crates/game-server` | the game server and the `game_probe` client |
| `crates/data-editor` | the data editor, its page and command line |
| `db/migrations` | the PostgreSQL schema of the servers (`public`, `edits`), applied at start |
| `db/defs`, `db/world`, `db/client` | the schemas of the imported game definitions, world files and client data files |
| `tools/defs`, `tools/world` | the importers for `PS_GameDefs` and `PS_Game/Data` |
| `tools/sdata` | the client data file tools: reading `*.SData`, generating client data patches (`docs/plans/done/client-patches.md`) |
| `tools/server-patches` | a patch kit for the original `ps_game.exe` (bug fixes for people running the original files) |
| `tools/*.sh` | run scripts for the servers and the data editor |
| `tools/*.py` | analysis helpers (PDB symbols and types, disassembly with names, svmap and SData parsers, the packet catalogue); every script is described in `tools/README.md` |
| `docs/` | the analysis notes and the rest, below |

## Documentation

- [`docs/README.md`](docs/README.md): what each folder of `docs/` holds, where to start, and the status words the notes use.
- [`docs/server/architecture.md`](docs/server/architecture.md): how the login server, the game servers and PostgreSQL fit together.
- `docs/original/*.md`: one note per system of the original: packets, rules, numbers, and what the new server does differently.
  [`docs/original/ps_login.md`](docs/original/ps_login.md) and [`docs/original/ps_game.md`](docs/original/ps_game.md) are the place to start.
- [`docs/original/protocol-notes.md`](docs/original/protocol-notes.md): packet framing and encryption.
- [`docs/server/settings.md`](docs/server/settings.md): every setting.
- [`docs/server/deviations.md`](docs/server/deviations.md): every place where the server knowingly differs from the original, by kind, with its setting.
- [`docs/server/deploy.md`](docs/server/deploy.md): running on a VPS.
- [`docs/original/client-login.md`](docs/original/client-login.md): pointing the client at the server.
- [`docs/original/database-notes.md`](docs/original/database-notes.md): the original databases.
- [`docs/client/`](docs/client/): the client's side of the protocol, from separate research on `game.exe`.
- [`docs/original/community-fixes.md`](docs/original/community-fixes.md) and [`docs/original/server-patch-notes.md`](docs/original/server-patch-notes.md): bugs of the original
  server and the patch kit that fixes them.
- [`docs/plans/roadmap.md`](docs/plans/roadmap.md) and [`docs/plans/release.md`](docs/plans/release.md): what comes next.

## Legal

Independent, non-commercial research and a clean implementation. No game files, original code, client or server binaries, or data
tables are distributed; the importers read your own copy. Shaiya and its trademarks belong to their owners.

License: MIT, see [`LICENSE`](LICENSE). How to contribute: [`CONTRIBUTING.md`](CONTRIBUTING.md) (the original comes first, an analysis note before code,
no original files). Reporting a vulnerability: [`SECURITY.md`](SECURITY.md). What changed between versions: [`CHANGELOG.md`](CHANGELOG.md).
