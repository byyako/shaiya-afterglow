# Changelog

## Unreleased

**Easier install**
- `tools/defs/read_bak.py` reads the game definitions straight out of the original `PS_GameDefs` backup, so the setup no longer needs a
  SQL Server container (about 20 minutes and 2 GB of memory); its output was checked byte for byte against the SQL Server export, on
  all eight exported tables. `tools/setup.sh --sqlserver` still restores the backup in SQL Server, and a folder of already exported
  tables is accepted in place of the backups.
- `tools/setup.sh` writes `.env` with random passwords when there is none, checks Docker and the PostgreSQL port, uses the binaries in
  `target/release/` instead of building when they are all there, and on a terminal creates the first account, the game server's entry
  and starts both servers. `defs/verify.py --counts-only` checks the loaded row counts without SQL Server.
- `tools/fetch-release.sh` downloads the binaries of a release (built by `.github/workflows/release.yml` when a `v*` tag is pushed).
- The README starts with a quick start; the docs site's Run page follows it.

**Renamed**
- The project's names inside the setup follow the repository: the containers are `shaiya-afterglow-postgres` and
  `shaiya-afterglow-mssql`, the volumes `shaiya-afterglow-pgdata` and `shaiya-afterglow-mssql-data`, the compose database is `shaiya`
  (was `newshaiya`; the `DATABASE_URL` of `.env.example` follows), the variables are `SHAIYA_PASSWORD` and `SHAIYA_PSQL`, and the
  edits export is `shaiya-afterglow-edits`. The old variable names and old edits files are still read. **A setup made before this keeps its data under the old names:** put
  `PG_CONTAINER=newshaiya-postgres`, `MSSQL_CONTAINER`, `PG_VOLUME` and `MSSQL_VOLUME` (the old names) in its `.env`, and keep the old
  database name in its `DATABASE_URL`; `compose.yaml` creates only a new `shaiya` database.

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
