# Changelog

## Unreleased

**Renamed**
- The project's names inside the setup follow the repository: the containers are `shaiya-afterglow-postgres` and
  `shaiya-afterglow-mssql`, the volumes `shaiya-afterglow-pgdata` and `shaiya-afterglow-mssql-data`, the compose database is `shaiya`
  (was `newshaiya`; the `DATABASE_URL` of `.env.example` follows), the variables are `SHAIYA_PASSWORD` and `SHAIYA_PSQL`, and the
  edits export is `shaiya-afterglow-edits`. The old variable names and old edits files are still read, and the tools find a
  container under its old name when only that one exists. **A setup made before this keeps its data under the old names:** put
  `PG_CONTAINER`, `PG_VOLUME` and the old database name in its `.env`, or dump and restore into the new one.

**Build and tests**
- `rust-version = "1.94"` in the workspace, so an older Rust stops with a clear message instead of a build error in `sqlx`.
- The death helper of `game_flow`'s tests sends the attack again while the monster is not fighting the player and waits by the
  clock, not by a count of passes: a busy CI runner failed `the_death_windows_resurrection_runes` once.

**Documentation**
- Building needs Rust 1.94 or newer (`sqlx` 0.9), not 1.85 as the README, the deploy guide and `tools/setup.sh` said.
- `docs/server/settings.md`: how on/off values and numbers are read, the ranges, `NEWSHAIYA_PASSWORD` and the test variables; the run
  scripts load `.env` over the environment.
- `docs/server/deploy.md`: the backup also dumps the `edits` schema; `mall` is installed with the other tools; the data editor runs
  through the `nsh` helper.
- There is no player alpha: the roadmap and the testing pages describe testing by whoever runs a server; stale "not built" notes fixed.
- The docs site has a "Buy me a coffee" link and the GitHub link in its footer.
- The docs site has a "Server (Shaiya Afterglow)" tab: a landing page, a short "Run your own server" page, and the architecture, settings,
  deployment, deviations and audit pages; backticked repository paths on the site link to GitHub. The server pages no longer
  mention the development machine.

## 0.1.0

The first release: a Shaiya Episode 4.5 server in Rust with PostgreSQL, for the unmodified US client (patch 108).

**Included**
- The login server and the game server, with every game system of the original EP4.5 server built from its analysis notes
  (`docs/original/`): login and the server list, the character screen, the world, items and shops, combat and skills, monsters and
  their AI, parties, trade, guilds and guild houses, quests, PvP, duels and guild wars, obelisks, instance zones, personal shops and the
  auction house, the item mall, the gambling house, the game and chat log, the operator (GM) commands and the operator console.
- The operator's tools: the console, the data editor (a web page with a drawn map, history and undo), the server list, account and
  item mall tools, and the import scripts for the original's data (`tools/`, described in `tools/README.md`).
- A patch kit for the original `ps_game.exe` (`tools/server-patches/`).
- Setup from a fresh clone with `tools/setup.sh` and `compose.yaml`; CI with format, lint, tests and a link check.
- The analysis notes, the research on the client, the plans and the live-test checklist (`docs/`).

**Behaviour**
- The defaults are the original's. Fixes to the original's crashes, duplication holes, exploits and limits are on; gameplay changes
  are settings (`docs/server/settings.md`); additions are off (`docs/server/deviations.md`).
- Not built, by decision: the second game server for map 200 and the original console's live reloads beyond `reload`.

**Known limits**
- Part of the live-test checklist has not been walked with a real client yet (`docs/testing/live-test-checklist.md`).
