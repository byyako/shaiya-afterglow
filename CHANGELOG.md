# Changelog

## Unreleased (0.3.0 in the making)

**The game data is built from the originals, extensions and edits** (extensions chunk 1, part A; `docs/plans/ext-1-foundation.md`)
- The original data is imported **once**, into frozen copies (`original_defs`, `original_world`, and `client` on a developer's box), and
  `content.imports` records the SHA-256 of every file read. A second run of an importer or of `tools/setup.sh` with the same files does
  nothing; other files are refused until `--reset`, which re-imports and rebuilds. The live tables `defs` and `world` are built by
  `data-editor content rebuild` as originals → installed extensions → the data editor's edits.
- `data-editor content status | hashes | rebuild | check | origin`: what is imported and installed; `schema.table|rows|md5` per table; the
  rebuild (also `--into` scratch schemas); a check that names every row changed by hand; the layers of one row.
- **Extensions**: a folder with `extension.toml` and `rows.toml` (and optionally migrations in its own schema). `data-editor ext list | show |
  check | install | upgrade | remove | enable | disable`. An extension adds or changes items, skills (levels 1 to 3) and drop chances, declares
  the ids it owns (checked against the client's ceilings and other extensions), may give defaults for game settings and carries the client
  fields (names, text, look-alike) of its rows. Removing one puts every row back to the original and is refused while players hold its
  items or skills or other data points at its rows; `--force` deletes the held items after a confirmation, logged.
- **The Kill Boost Stones are the first extension** (`extensions/kill-boost`), no longer `additions.sql` and
  `client_rows.sql` (deleted). Installing it on a database that already had the stones changes no byte of any table.
- `tools/sdata/client_patch.py` reads the database only (the client rows are `content.client_items` / `client_skills`), keeps a ledger of the
  patches it generates (`content.client_patches`), and has `adopt`, `rebuild --patch N` (regenerates a patch from the database; patch 109
  matches byte for byte) and `record`.
- `tools/setup.sh` has `--reset` and `--extension <folder>`, writes `config/game.toml` when it is missing and lists the bundled extensions;
  the release tarball carries `extensions/`; `tools/fetch-release.sh --keep <dir>` unpacks the whole tarball.
- Migration 0031 (the `content` schema).

**The game server's settings are a file** (extensions chunk 1, part B)
- Every game setting is a key of `config/game.toml` (the old variable's name without `GAME_`, in lowercase: `GAME_MARKET_MAPS` is
  `market_maps`); the layers, lowest first, are the built-in defaults, the defaults of the enabled extensions, the file, and the data
  editor's overrides. `config/game.toml.example` lists every setting with its default and meaning. `tools/game-server.sh` sets
  `GAME_CONFIG` to the checkout's file; on a VPS `GAME_CONFIG` names `/etc/shaiya-afterglow/game.toml`.
- A game setting still set in the environment (`.env` included) **stops the game server at start** and prints the lines to put in the file;
  a `GAME_` name that is not a setting is listed as such. `DATABASE_URL`, `GAME_BIND`, `GAME_SERVER_ID`, `GAME_CONFIG` and `RUST_LOG` stay
  variables. Errors in the file name the line (`config/game.toml line 4: unknown setting "x"`), and an extension's or the editor's value
  names its layer.
- `game-server config` prints what is in force and which layer each value came from; `game-server config --example` prints the example
  file. At start the server logs every setting that is not at its default, with its layer.
- The data editor has the target `setting` (`draft add set setting <name> value <value>`): an override in `content.setting_values` that
  beats the file, with History, Undo, export and import, checked by the game server's own parser; changes take effect at the next start.
- The game server reads the settings' database layers over one connection, which also runs the migrations, before it builds its pool.

**Rules** (extensions chunk 2; `docs/plans/ext-2-rules.md`, `docs/server/rules.md`)
- `[rules.<name>]` tables in `config/game.toml` allow, deny or change something for a scope: maps, level and kill-count ranges, faction,
  weekdays and hours, `{ not = … }`, and for two-player points which side is tested. Types: `deny` (attack, trade, personal shop, trade
  chat), `level_gap` (neither of two players more than N levels apart may attack the other), `cross_faction_trade`, `pvp_gain`
  (experience, kills and deaths of player kills), `exp` (experience from monsters, parties and quests) and `enchant` (weighted jumps of
  more than one step). A refused player gets a whisper from `Server` (the rule's `message`, once per rule per 10 seconds).
- Layers as the settings: the settings' own rules, the enabled extensions' (`[rules.<name>]` in `extension.toml`), the file, the data
  editor (target `rule`, effect live); a rule replaces a lower one of the same name whole, `{ enabled = false }` turns one off.
- The console's `reload` also reads the rules (the file's and the database's) and puts them in force with the monsters and drops, all or
  nothing; it names the settings changed in the file, which wait for a start. The console's `rules` lists the rules with their hits.
  `game-server rules` prints the rules in force with the layer of each; `game-server rules --check [file]` checks a file alone.
- `cross_faction_trade_maps`, `market_maps` and `trade_chat_min_level` keep working and make a rule each; nothing else changes by default.
- Migration 0032 (`content.ext_rules`, `content.rule_values`) runs at the next start. No step for an operator.

**Lua scripts for extensions** (extensions chunk 5; `docs/plans/ext-5-lua.md`, `docs/server/scripts.md`)
- An extension may carry Lua scripts (`api = 1`, `scripts = ["main.lua"]`, `[options.<key>]` in `extension.toml`). `data-editor ext check`
  compiles them and runs the load in a state that does nothing, naming the file and line of an error and listing what the scripts
  register; `install` and `upgrade` store them in the database; `data-editor ext scripts <folder>` replaces only the scripts and options
  of an installed extension, for the edit loop. The game server reads the scripts from the database and `console <id> reload` puts them
  in force with the rules, all or nothing.
- Scripts react to events (`login`, `logout`, `map_enter`, `map_leave`, `kill`, `boss_kill`, `pvp_kill`, `death`, timers, and players'
  `!commands` typed as chat), act through a small versioned API (items, gold, buffs, teleports, notices, monsters spawned for a while,
  values stored per character and per extension), and are asked after the rules at the points trade, personal shop, trade chat, chat,
  experience, player kills and deaths, enchanting and the use of the items they take over. Events run on one script thread, never inside a
  zone lock; a point runs on the asking connection in 5 ms.
- The sandbox: Lua 5.1 with `math`, `string`, `table` and a reduced `os`; no file, process, network or SQL; instruction, time and memory
  limits; five errors in 60 seconds (or one memory error) disable that extension's scripts and tell the staff; the original's monster Lua is
  untouched.
- Options in layers: the manifest default, `[ext.<name>]` in `config/game.toml`, the data editor's `ext.<name>.<key>`; `game-server config`
  lists them with their layer. New settings `scripts` and `script_memory_mb`; the console's `scripts`; the answer of `reload` names the scripts.
- Four bundled extensions, none installed by default: `kill-streak`, `welcome`, `boss-rewards`, `monster-madness`.
- Migration 0033 (`content.ext_scripts`, `ext_options`, `script_values`, `script_globals`) runs at the next start. No step for an operator.

**Item effects and skill abilities** (extensions chunk 3; `docs/plans/ext-3-effects.md`, `docs/server/effects.md`)
- A `[[item]]` entry of an extension's `rows.toml` may have an `effect`, a `[[skill]]` entry `abilities`: named behaviours written in the
  server that the rows give a parameter set. Item effects: `level_up` (a stone that raises the level by some or to a level), `enchant` (a
  Lapisia's own success rate, jump table and `safe`), `recreate` (a Recreation Rune that redraws chosen crafted numbers, or sets them to
  the maximum). Skill ability: `kill_boost` (a counted player kill counts several times while the skill is applied). They are server-only
  data (`content.ext_item_effects`, `ext_skill_abilities`), never in the client's rows, so patch 109 regenerates unchanged. `ext check`
  prints them and warns for an item that would do something else while its extension is disabled.
- An item's use goes: the core's checks, the rules, the reuse group, the script that took the item over (`game.on_item`), the extension's
  effect, the core's effect. `item.effect` in the Lua `item` table names the effect in force.
- The rules have a ninth point, `item_use`: `type = "deny"`, `point = "item_use"`, optional `items = ["100/215-217", "95"]`.
- Three reference extensions, none installed by default: `level-stones` (100/215 to 217), `perfect-lapisia` (95/3, 95/8) and `stat-runes`
  (100/221 to 226); all grade 999 (no monster drops them). Their client text comes with the next client data patch.
- **The Kill Boost Stones move to version 1.1.0**: the multipliers are `kill_boost` abilities on the skill rows instead of the setting
  `kill_boost_skills`. What the players see and what the kills count does not change. For a server with 1.0.0 installed,
  `data-editor ext upgrade extensions/kill-boost` and a restart is optional: nothing breaks without it, because `kill_boost_skills`
  remains a shorthand that gives the same abilities at start.
- Custom GM status levels moved to a later wave (they touch the login server and the client's privilege byte, not items).
- Migration 0034 (`content.ext_item_effects`, `content.ext_skill_abilities`) runs at the next start. No step for an operator.

**Upgrading from 0.2.0** (breaking changes; nothing is converted automatically):
1. **Settings**: the game settings are no longer environment variables. Stop the game server, start the new one once: it refuses to start
   and prints the `config/game.toml` lines for every `GAME_…` setting it found (in `.env` too). Put them in `config/game.toml` (on a VPS:
   `/etc/shaiya-afterglow/game.toml`, and `GAME_CONFIG=/etc/shaiya-afterglow/game.toml` in the env file), remove the variables.
   `DATABASE_URL`, `GAME_BIND`, `GAME_SERVER_ID` and `RUST_LOG` stay variables.
2. **Game data**: run `tools/setup.sh` again with the same two folders. It imports the original data once into new schemas
   (`original_defs`, `original_world`) and rebuilds `defs` and `world` from them plus your data editor's edits; changes made by hand
   in SQL are replaced, as before on every re-run. Later re-runs skip the import; `--reset` re-imports corrected files. Until then the game
   server runs as before and `data-editor ext install` refuses, saying so.
3. **The Kill Boost Stones**: `additions.sh` is gone. If you used `--with-additions`, install the extension instead
   (`target/release/data-editor ext install extensions/kill-boost`, or `tools/setup.sh --extension extensions/kill-boost`) before
   the game server starts, so held stones stay stones; its kill multipliers come with it, so drop `kill_boost_skills` from your
   settings unless you changed them.
4. **Moving game data to a VPS**: dump `defs`, `world`, `original_defs`, `original_world`, `content` and `edits`
   (`docs/server/deploy.md` section 3).

## 0.2.0 (2026-11-02)

**Easier install**
- `tools/defs/read_bak.py` reads the game definitions straight out of the original `PS_GameDefs` backup, so the setup no longer needs a
  SQL Server container (about 20 minutes and 2 GB of memory); its output was checked byte for byte against the SQL Server export, on
  all eight exported tables. `tools/setup.sh --sqlserver` still restores the backup in SQL Server, and a folder of already exported
  tables is accepted in place of the backups.
- `tools/setup.sh` writes `.env` with random passwords when there is none, checks Docker and the PostgreSQL port, uses the binaries in
  `target/release/` instead of building when they are all there, and on a terminal creates the first account, the game server's entry
  and starts both servers. `defs/verify.py --counts-only` checks the loaded row counts without SQL Server.
- `tools/patch_client_ip.py <Client folder> <ip>` writes `game_local.exe` beside the client's `game.exe`, and says what build it needs when the file is another.
- `tools/fetch-release.sh` downloads the binaries of a release (built by `.github/workflows/release.yml` when a `v*` tag is pushed); `tools/setup.sh` runs it by itself when Rust is not installed. The release text comes from this changelog (`tools/release-notes.sh`).
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
