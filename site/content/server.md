# The server: Shaiya Afterglow

Shaiya Afterglow is a Shaiya Episode 4.5 server written from scratch in Rust, with PostgreSQL for its data. It is built from the research on the other tabs of this site: each system was read from the original server programs first, written up as a note, and then coded to that note. It works with the unmodified US client. The code is open source under the MIT license, on GitHub as [byyako/shaiya-afterglow](https://github.com/byyako/shaiya-afterglow).

No original code, binaries or game data are included. You bring your own copy of the original server files, and the import tools load them into PostgreSQL.

!!! info "What the server is built against"
    | | |
    |---|---|
    | Original server | EP4.5 `ps_login.exe`, `ps_game.exe` and their neighbours, from the widely shared server pack (binaries of mid-2009, pack of 2010-03) |
    | Client | US `game.exe`, built 2010-04-26, patch level 108, unmodified (only its login address is pointed at your server) |
    | Version | 0.1.0, the first public release ([changelog](https://github.com/byyako/shaiya-afterglow/blob/main/CHANGELOG.md)) |
    | Runs on | Linux, with Docker for PostgreSQL; Rust 1.94 or newer to build |

    There is no closed alpha and no official test server: the public release is the trial. If you run it and something behaves differently from the original, a report is welcome ([issues](https://github.com/byyako/shaiya-afterglow/issues)).

## In short

- **The original game, without its bugs.** The original's rules, numbers and packets by default, with its crashes, duplication holes and exploits fixed. Every place the server knowingly differs is listed in [Deviations](server/deviations.md); the ones that change gameplay are settings.
- **Every system of the original is built:** login and the server list, the world, items and shops, combat and skills, monsters and their AI, parties, guilds and guild houses, quests, PvP, duels and guild wars, obelisks, instance zones, personal shops and the auction house, the item mall, the gambling house, the game log, and the GM commands. A few things are left out on purpose (the second game server for map 200, some live reloads of the original's console).
- **Two programs players connect to.** A login server and one game server per world, with PostgreSQL as the only shared state; it replaces the original's seven services and SQL Server ([Architecture](server/architecture.md)).
- **Tools for the operator:** a command-line console that reaches a running game server, the GM commands in the game, a data editor (a web page with a drawn map, history and undo) for drops, monsters, spawns and portals, and tools for accounts, the server list and the item mall.
- **Hardened against hostile clients.** A robustness audit of the code found 30 problems (R1 to R30), every one fixed, and a fuzz harness throws random packets at the decoders, the login and the game ([Robustness audit](server/audit.md)).
- **Tested in play, partly.** Many systems have been tried with the real client; the rest are covered by the test suite and still wait for a test in play.

## Learn the server

A reading path, from running it to how it is made:

1. **This page.** What the server is, what it is built against, and the words the pages use.
2. [Run your own server](server/run.md): what you need, the setup in brief, the operator tools, the data editor, updating.
3. [Architecture](server/architecture.md): the programs, how a login reaches the world, the inside of the game server, the database schemas.
4. [Settings](server/settings.md): every environment variable with its default; the ones that change gameplay default to the original.
5. [Deviations from the original](server/deviations.md): every difference, by kind, with the setting that controls it.
6. [Deploying on a VPS](server/deploy.md): a public server with systemd, the firewall, backups and updates.
7. [Pointing the client at the server](original/client-login.md): how the client finds the login server, the two ways to change that, and reading what it shows.
8. [Robustness audit](server/audit.md): what a hostile client could do, what was fixed, and the fuzz harness.
9. The original's side, on the other tabs: [the login server](original/ps_login.md) and [the game server](original/ps_game.md) first, then one system, for example [Trade](original/trade.md). Each note ends with what this server does differently.
10. The operator's side of the original, which the server keeps: [the operator console](original/operator-console.md), [GM commands](original/gm-commands.md) and [the game log](original/game-log.md).

## Sections

| Section | What it covers |
|---|---|
| Start here | This page and [Run your own server](server/run.md) |
| How it works | [Architecture](server/architecture.md): programs, the login path, the game server's tasks and locks, the schemas |
| Running it | [Settings](server/settings.md), [Deploying on a VPS](server/deploy.md) and [Pointing the client at the server](original/client-login.md) |
| Behaviour and safety | [Deviations from the original](server/deviations.md) and the [Robustness audit](server/audit.md) |

The rest is on GitHub: the [README](https://github.com/byyako/shaiya-afterglow#readme) (the full setup), [`tools/README.md`](https://github.com/byyako/shaiya-afterglow/blob/main/tools/README.md) (every script), [CONTRIBUTING](https://github.com/byyako/shaiya-afterglow/blob/main/CONTRIBUTING.md), [SECURITY](https://github.com/byyako/shaiya-afterglow/blob/main/SECURITY.md) (reporting a vulnerability privately), and the plans in [`docs/plans/`](https://github.com/byyako/shaiya-afterglow/tree/main/docs/plans).

## Conventions

- **The original** is the EP4.5 server above; **this server** is Shaiya Afterglow.
- Status words for the server: **built**, **not built** (by decision, or planned), **tested in play** (tried with the real client) and **waiting for a test in play** (covered by the test suite only; the list is the [live test checklist](https://github.com/byyako/shaiya-afterglow/blob/main/docs/testing/live-test-checklist.md)).
- Status words for what is said about the original: **confirmed** (read in the binary or the code, with the address or function), *inferred* (follows from what was read, not seen directly) and **PDB** (from the debug symbols, so names and layouts are exact).
- Deviations have a kind: **fix**, **client**, **robust**, **tooling**, **gameplay**, **addition** or **internal**; [the Deviations page](server/deviations.md) explains each.
- Settings are environment variables, written `GAME_EXP_RATE`; every one has a default and only `DATABASE_URL` is required.
- Audit findings are numbered `R1` to `R30`. Paths such as `crates/game-server` are in the repository, and link to it on GitHub.
