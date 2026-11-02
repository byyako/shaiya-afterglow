# Deploying on a VPS

How to run the login and game servers on one Linux VPS with PostgreSQL on the same machine. The commands assume Debian or Ubuntu and
systemd; paths and names (`/opt/shaiya-afterglow`, `/etc/shaiya-afterglow`, user and database `shaiya` / `shaiya`) are suggestions.

What the servers need at runtime:

- **The binaries only.** The schema migrations are compiled in and run at start; neither server reads files at runtime.
- **The game data in PostgreSQL**: the `defs` schema (items, monsters, skills, ...) and the `world` schema (maps, NPCs, spawns, quests, scripts).
  Copy both from a working database (step 3); the VPS needs no original files, no MSSQL and none of the import tools.
- **The `public` schema** (accounts, characters, sessions, guilds, ...) is created by the migrations and is the data to back up.

## Sizing

Measured on a test server (release build, all maps loaded, about 18,000 monsters spawned, nobody online): game server 38 MB of memory and about
1% of one 2012 desktop core, login server 6 MB, PostgreSQL about 65 MB. The world update (`mob_updater`, every 500 ms) runs on one core, so a
fast core matters more than many; player connections use every core. Per-player costs have not been measured under load.

| use | CPU | memory | disk |
|---|---|---|---|
| testing, private server | 1 vCPU | 1 GB (2 GB to compile on the VPS, or add swap) | 10 GB SSD |
| small public server | 2 vCPUs, fast single core | 2 GB | 20-40 GB SSD |

## 1. PostgreSQL

The README's setup uses PostgreSQL 16; other recent versions should work but have not been tried. `pg_restore` must be the same version as the
dump's `pg_dump` or newer.

```bash
sudo apt install postgresql
sudo -u postgres createuser -P shaiya            # asks for the password
sudo -u postgres createdb -O shaiya shaiya
```

The Debian package listens on localhost only; keep it that way.

## 2. The binaries

Seven binaries: `login-server`, `game-server`, the data editor `data-editor` (`docs/plans/done/data-editing.md`), and the tools `servers`,
`create_account`, `console` and `mall` (the last four are built with the `login-server` package).

**Build on the VPS** (simplest; the build needs about 2 GB of memory, so add swap on a 1 GB VPS). No OpenSSL or database is needed to compile.

```bash
sudo apt install build-essential git
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh     # Rust 1.94 or newer (edition 2024; sqlx 0.9 needs it)
git clone https://github.com/byyako/shaiya-afterglow.git && cd shaiya-afterglow
cargo build --release -p login-server -p game-server -p data-editor
sudo install -D -t /opt/shaiya-afterglow/bin target/release/{login-server,game-server,data-editor,servers,create_account,console,mall}
```

**Or copy them** from a machine that built them: `target/release/{login-server,game-server,data-editor,servers,create_account,console,mall}`. They link against the system
glibc, so the VPS needs the build machine's glibc version or newer (a build on Debian 13, glibc 2.41, will not run on Debian 12 or Ubuntu 24.04,
which are older). If the VPS is older, build there, or build in a container of the VPS's distribution.

## 3. The game data

On the machine with the working database (with the README's setup, it runs in the `shaiya-afterglow-postgres` container):

```bash
docker exec shaiya-afterglow-postgres pg_dump -U shaiya -n defs -n world -Fc shaiya > gamedata.dump    # about 1.2 MB
```

Without Docker: `pg_dump -h <host> -p <port> -U shaiya -n defs -n world -Fc shaiya > gamedata.dump`. Copy it to the VPS and restore:

```bash
pg_restore -h 127.0.0.1 -U shaiya -d shaiya --no-owner gamedata.dump
```

The migrations do not touch `defs` or `world`, so the order of this step and the first start does not matter. To update the game data later,
stop the game server, drop the two schemas (`DROP SCHEMA defs, world CASCADE`), restore a new dump, run `data-editor apply` (it writes the
operator's edits, which live in the `edits` schema and are not in the dump, back into the fresh tables and reports what the new files
changed under an edit) and start the game server again. A dump taken with `-n defs -n world` already holds the edits of the machine it
came from; to move the edits themselves, export and import them (below).

## 4. Settings

A system user for the services, and one environment file for both servers and the tools:

```bash
sudo useradd --system --no-create-home --shell /usr/sbin/nologin shaiya
sudo install -d -m 750 -o root -g shaiya /etc/shaiya-afterglow
sudoedit /etc/shaiya-afterglow/shaiya-afterglow.env
sudo chown root:shaiya /etc/shaiya-afterglow/shaiya-afterglow.env && sudo chmod 640 /etc/shaiya-afterglow/shaiya-afterglow.env
```

```bash
# /etc/shaiya-afterglow/shaiya-afterglow.env
DATABASE_URL=postgres://shaiya:<password>@127.0.0.1:5432/shaiya
GAME_SERVER_ID=1
RUST_LOG=info
```

The services run at `RUST_LOG=info` and log to the journal, which rotates; `tools/game-server.sh` and `tools/login-server.sh` are meant
for a development machine and default to `debug` in a file that does not (`docs/server/audit.md` R18). Both units below carry `LimitNOFILE=65536`: systemd's
default of 1024 open files is below `GAME_MAX_CONNECTIONS` (5000), and running out of them makes `accept` fail. Open the game port to the
world but keep PostgreSQL on loopback: the game server's pool (`GAME_DB_POOL`, 24) and the login server's (16) plus the notification
listener fit PostgreSQL's default of 100 connections.

The login guard (`docs/server/audit.md` R7) needs no setting: five wrong passwords for one account from one address make that address wait
15 minutes for that account, 30 an hour from all addresses together block the account for everyone until the hour clears (the same "wrong
password" answer, so the names that exist are not revealed); `LOGIN_PAIR_*`, `LOGIN_ACCOUNT_*` and `LOGIN_TRACKED_ACCOUNTS` change the numbers.
A player locked out this way waits it out; the operator can restart the login server to clear the counts (they are in memory only). New
passwords (`create_account`, later the web API) have 6 to 16 characters (16 is all the client sends); accounts made with shorter ones still log in.

Every other setting has a default; the full lists are in `docs/server/settings.md`. The file is read by systemd, not a shell: one `NAME=value` per line, no `export`, no quotes needed.

## 5. Register the game server and create accounts

The tools read `DATABASE_URL` from the environment (or a `.env` in the current folder) and run the migrations themselves, so they work before
the servers have ever started. A helper that runs one as the `shaiya` user with the settings file, read by systemd the same way as for the
services:

```bash
nsh() { sudo systemd-run --quiet --pty --collect -p User=shaiya -p EnvironmentFile=/etc/shaiya-afterglow/shaiya-afterglow.env "/opt/shaiya-afterglow/bin/$1" "${@:2}"; }
```

```bash
nsh servers add 1 MyServer <VPS public IPv4>     # the address the client is sent to; one IPv4 per game server, port 30810 always
nsh servers list
nsh create_account myname 16                     # asks for the password (6 to 16 characters); 16 is full GM, 0 (default) a normal account
```

`servers add` takes an optional capacity and required client version (`servers add <id> <name> <ip> [capacity] [version]`); the version is only
enforced with `LOGIN_CHECK_VERSION=true`.

## 6. systemd units

The two servers do not depend on each other: both only need PostgreSQL. The game server reports itself through the database (a heartbeat every
5 s), and the login server lists it as open while the heartbeat is recent. Logs go to the journal.

`/etc/systemd/system/shaiya-afterglow-login.service`:

```ini
[Unit]
Description=Shaiya Afterglow login server
After=network-online.target postgresql.service
Wants=network-online.target

[Service]
User=shaiya
Group=shaiya
EnvironmentFile=/etc/shaiya-afterglow/shaiya-afterglow.env
Environment=NO_COLOR=1
ExecStart=/opt/shaiya-afterglow/bin/login-server
LimitNOFILE=65536
# The login server shuts down cleanly on Ctrl+C (SIGINT) only.
KillSignal=SIGINT
Restart=on-failure
RestartSec=5
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true

[Install]
WantedBy=multi-user.target
```

`/etc/systemd/system/shaiya-afterglow-game.service`:

```ini
[Unit]
Description=Shaiya Afterglow game server
After=network-online.target postgresql.service
Wants=network-online.target

[Service]
User=shaiya
Group=shaiya
EnvironmentFile=/etc/shaiya-afterglow/shaiya-afterglow.env
Environment=NO_COLOR=1
ExecStart=/opt/shaiya-afterglow/bin/game-server
LimitNOFILE=65536
# SIGTERM (the default) closes every connection the way a lost connection does, saving the players in the world; it waits up to 10 s for that.
TimeoutStopSec=30
Restart=on-failure
RestartSec=5
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true

[Install]
WantedBy=multi-user.target
```

The console's `restart [minutes]` counts down like `quit`, saves everyone and then ends the game server with exit status **75**; `quit`,
SIGTERM and Ctrl+C end it with 0. `Restart=on-failure` starts it again after a non-zero status (so after `restart` and after a crash,
not after `systemctl stop` or `quit`). Without systemd, `tools/game-server.sh` runs the server in a loop that starts it again after any non-zero status, and gives up when the
server exits more than 5 times within a minute. A panic in a connection or a background task does not end the process: the connection ends
and its player is saved as on a lost connection, the task starts again after a second, and both are logged at error.

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now shaiya-afterglow-login shaiya-afterglow-game
journalctl -u shaiya-afterglow-game -f                  # the log; it ends its start with "listening for logins"
nsh servers list                                 # the heartbeat column shows the game server's last report
```

`RUST_LOG=info,game_server=debug` (as `tools/game-server.sh` uses) adds the monster spawn and step counts.

## 6b. The data editor

`data-editor` changes the game's data (drops, gold, monster stats, item grades, spawn areas, obelisks, boss groups, portal destinations) without
SQL. It needs only `DATABASE_URL` (the same file as the servers, `/etc/shaiya-afterglow/shaiya-afterglow.env`), so it runs on the server machine or anywhere
that reaches the database. It is not a service: run it when the data is edited.

```bash
nsh data-editor draft add set mob_drop 200/1 droprate 45 && nsh data-editor apply-draft -m "raise the drop" --by me
nsh console 1 reload                   # drops, gold and most monster stats take effect at once
nsh console 1 restart 5                # spawns, obelisks, boss groups and portal destinations need a restart (5 minutes' notice, then exit status 75)
```

(`nsh` is the helper of section 5: it runs the tool as the `shaiya` user with the settings file, which only root and that group can read.)

- **Edits are kept apart from the game data.** They live in the `edits` schema (migration 0030, created at the first start of any binary),
  which no load of the original files touches; `data-editor apply` writes them into `defs` and `world` again. Run it after every restore of
  a game data dump or import of the original files (the repository's loaders, `tools/defs/additions.sh` and `tools/world/import.py`, end
  with it when `target/release/data-editor` exists).
- **Moving edits to another server, backup, sharing**: `data-editor export edits.json` on one machine, `data-editor import edits.json` on the
  other (`--replace` first reverts the edits in force there). The file holds the history too. Set `EDITOR_EXPORT_FILE=/path/edits.json` and every
  Apply and Undo writes it; keep that file in git or in the backups. `--images` adds the map pictures of the page.
- **The page**: `data-editor serve` (or `tools/data-editor.sh start` from the repository) binds `EDITOR_BIND`, default `127.0.0.1:30890` (the script
  too), and only accepts loopback, private and Tailscale addresses, and only requests addressed to `localhost`, `127.0.0.1` or `[::1]` (or a name
  in `EDITOR_HOSTS`, comma separated), with no login yet: a web page that rebinds its own name to the box is refused (`docs/server/audit.md` R17). It shows the maps drawn from the database (spawn areas, portals, NPCs,
  obelisks, boss spots), the monsters' drop tables and stats, and the draft, History with Undo, and the game servers with Reload and Restart.
  On a VPS reach it through a tunnel, never by opening the port: `ssh -L 30890:127.0.0.1:30890 user@vps`, then browse `http://127.0.0.1:30890`. Run it
  only while data is being edited, or as a unit like the servers (`ExecStart=/opt/shaiya-afterglow/bin/data-editor serve`, same `EnvironmentFile`); it holds no
  state of its own, so stopping it loses nothing (the draft is in the database). **Map pictures**: "Set picture…" on each map, or
  `data-editor images import <folder>` with `<map id>.png` (or `.jpg`), `<map id>.json` (`{"x1", "z1", "x2", "z2"}`) and `names.json`.
- **Every game server sharing the database** takes `reload` and `restart` from the editor through PostgreSQL `NOTIFY`, as the `console` tool does.

## 7. Firewall

Open TCP 30800 (login) and 30810 (game), and SSH; nothing else. With `ufw`:

```bash
sudo ufw allow OpenSSH
sudo ufw allow 30800/tcp
sudo ufw allow 30810/tcp
sudo ufw enable
```

Many VPS providers also have their own firewall in the control panel; open the same two ports there.

## 8. The client

Patch a copy of `game.exe` with the VPS address (`docs/original/client-login.md` has the details and the other option):

```bash
python3 tools/patch_client_ip.py <Client>/game.exe <Client>/game_vps.exe <VPS public IPv4>
```

The login server sends the client the address registered with `servers add`, so that address must be the public one too.

`GAME_CHECK_IP=true` (the default) refuses a game connection from another address than the account's login. Players whose address changes
between the two (some mobile and carrier-grade NAT connections) cannot enter; `GAME_CHECK_IP=false` turns the check off, as in the original.

## 9. Updating

```bash
cd shaiya-afterglow && git pull && cargo build --release -p login-server -p game-server -p data-editor
sudo install -t /opt/shaiya-afterglow/bin target/release/{login-server,game-server,data-editor,servers,create_account,console,mall}
sudo systemctl restart shaiya-afterglow-game shaiya-afterglow-login
```

New migrations run when the servers start. Players in the world are saved on the game server's stop and must log in again.

## 10. Backups

The `public` schema is the player data; `defs` and `world` can always be restored from the game data dump (then `data-editor apply`); the
`edits` schema holds the operator's edits and their history, so the dump below takes both (`-n public -n edits`); the export file of
section 6b is a second copy of the edits. A daily dump, kept for two weeks:

```bash
sudo install -d -o postgres /var/backups/shaiya-afterglow
sudo tee /etc/cron.daily/shaiya-afterglow-backup >/dev/null <<'EOF'
#!/bin/sh
cd /var/backups/shaiya-afterglow || exit 1
runuser -u postgres -- pg_dump -n public -n edits -Fc shaiya > "players-$(date +%F).dump"
find . -name 'players-*.dump' -mtime +14 -delete
EOF
sudo chmod 755 /etc/cron.daily/shaiya-afterglow-backup
```

Restore into an empty database with `pg_restore -d shaiya --no-owner players-<date>.dump`, together with the game data dump. Copy the
backups off the VPS as well.
