# Run your own server

## In short

- You need a Linux machine with Docker, Rust 1.94 or newer, Python 3, your own copy of the original EP4.5 server files, and the US client (patch 108) to play.
- One script, `tools/setup.sh`, starts PostgreSQL, builds the server and imports the original files (about 25 minutes). Then you make an account, register the game server, start both servers and point the client at them.
- With only `DATABASE_URL` set, the server plays as the original apart from its fixes. Everything else is a setting with a default ([Settings](settings.md)).
- The operator works through a console tool, the GM commands in the game, and a data editor for drops, monsters, spawns and portals.
- This page is the short path. The [README on GitHub](https://github.com/byyako/shaiya-afterglow#readme) has every step and command, and [Deploying on a VPS](deploy.md) covers a public server.

## What you need

| | |
|---|---|
| A machine | Linux (developed on Debian 13; any recent distribution with Docker should do). Windows hosts are not tested. Building needs about 2 GB of memory. |
| Docker | For PostgreSQL, and for SQL Server during the import only |
| Rust | 1.94 or newer, from [rustup](https://rustup.rs) |
| Python 3 | Standard library only, for the import scripts |
| The original server files | The SQL Server backups in `PSM_Data/` (only `PS_GameDefs` is used) and the game server's `PS_Game/Data/` folder, from the widely shared EP4.5 server pack. They are not distributed with the server. |
| The client | The US `game.exe`, patch 108 |
| Network | TCP 30800 (login) and 30810 (game). The client always connects to a game server on 30810, so each game server needs its own IPv4 address. |

## Setting up

From a fresh clone:

```bash
git clone https://github.com/byyako/shaiya-afterglow.git newshaiya && cd newshaiya
cp .env.example .env          # then set the passwords in it
tools/setup.sh /path/to/PSM_Data /path/to/PS_Game/Data
```

`tools/setup.sh` runs the README's steps 2 to 5 in one go: it starts PostgreSQL, builds the workspace, creates the tables, restores the original database backups into a temporary SQL Server, loads and checks the game definitions, imports the world (maps, spawns, NPCs, quests, scripts) and stops SQL Server again. It stops at the first failure with the step's name, and it can be run again. The README describes each step on its own, for doing them by hand.

Then an account and the game server's entry in the server list:

```bash
NEWSHAIYA_PASSWORD='secret123' target/release/create_account myname 16   # 16 = full GM; leave it out for a player
target/release/servers add 1 MyServer 203.0.113.10                        # id, name, the address clients connect to
```

The address is the one players' clients reach: a LAN address for a home test, the public one on a VPS. Then start the servers:

```bash
tools/login-server.sh start
tools/game-server.sh start
tools/game-server.sh logs        # the last lines of the game server's log
```

The game server loads the definitions and the world in a few seconds and starts reporting to the login server, which then lists it as open.

**The client** has its login address built in. `tools/patch_client_ip.py` writes a copy of `game.exe` with your server's address (it checks the file's hash first and changes only the address), or a network redirect does the same without touching the client. Log in with the account you made. [Pointing the client at the server](../original/client-login.md) has the details: what the address is in the client, both ways to change it, and what each error on screen means.

## Running and stopping

The run scripts take `start`, `stop`, `restart`, `status` and `logs`. Stopping the game server saves every character in the world first. With players online, use the countdown instead: `target/release/console 1 quit 5` announces a stop in five minutes, and `restart 5` does the same and starts the server again.

The settings most often changed are `GAME_EXP_RATE` (the experience factor), `GAME_SERVER_ID`, the listen addresses `GAME_BIND` and `LOGIN_BIND`, and the log level `RUST_LOG`. [Settings](settings.md) lists every one; [Deviations](deviations.md) section 1 lists the ones that turn on what the original lacks, such as trade between the factions.

## The operator tools

```bash
target/release/create_account <name> [status]     # a new account; status 16 is full GM
target/release/servers list | add | remove | enable | disable
target/release/mall points <user> add 1000        # item mall points, premium, deliveries
target/release/console 1 info                     # the operator console of game server 1
```

The console reaches a running game server through the database and takes the original console's commands: notices, the stop and restart countdowns, `reload` (monsters and drop tables, live), kicking a player, the experience and kill rates, closing the server to new logins, and more; `console 1 help` lists them ([Operator console](../original/operator-console.md)). Staff characters can type the same commands in the game as `.help`, `.info` and so on.

In the game, GM accounts have the original's slash commands: moving, items, monsters and NPCs, bans, silences, kicks, summons, quests and the rest ([GM commands 1](../original/gm-commands.md), [GM commands 2](../original/gm-commands-2.md)). What players and staff do is written to the [game log](../original/game-log.md) in PostgreSQL.

## The data editor

`data-editor` changes the game data in place and keeps every change, with who made it, when, why, and the value before and after; any change set can be undone. It covers drops and gold, monster stats, item grades, spawn areas, obelisks, boss groups and portal destinations. Edits are kept apart from the imported data and written back after every new import, so a re-import does not lose them.

- **The page:** `tools/data-editor.sh start`, then open `http://localhost:30890/`. It shows a drawn map of each game map with its spawn areas, NPCs, portals, obelisks and bosses; the monsters and their drop tables; a draft that is checked as you type and applied as one change set; the history with Undo; and each game server with Reload and Restart buttons. It has no login and answers only local, private and Tailscale addresses: from another machine, use an SSH tunnel.
- **The command line:** the same drafts, change sets, history, undo, and an export and import of all the edits (`data-editor --help`).

Drops, gold and monster stats take effect with the console's `reload`; spawns, obelisks, boss groups and portals need a restart.

## Updating

```bash
git pull
cargo build --release --workspace
target/release/console 1 restart 5     # warns the players, saves them, restarts
tools/login-server.sh restart
```

Database changes run by themselves when the servers start. Updates do not touch the game data; run the import again only when the import tools or your original files change.

## More on GitHub

- [The README](https://github.com/byyako/shaiya-afterglow#readme): every setup step by hand, the probes that check a server without a client, development and tests.
- [`tools/README.md`](https://github.com/byyako/shaiya-afterglow/blob/main/tools/README.md): every script in `tools/`.
- [CONTRIBUTING](https://github.com/byyako/shaiya-afterglow/blob/main/CONTRIBUTING.md): the original comes first, a note before code, no original files.
- [SECURITY](https://github.com/byyako/shaiya-afterglow/blob/main/SECURITY.md): reporting a vulnerability privately.
- [Issues](https://github.com/byyako/shaiya-afterglow/issues): a bug, or a system that behaves differently from the original.
