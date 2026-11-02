# Release plan: the open-source repository

**Done: released as `byyako/shaiya-afterglow`, tag `v0.1.0`.** The goal was a **public repository**: anyone with the original EP4.5
server files and the US client can clone it, follow the README and run a server. Bug hunting with the real client is not part of this
plan; it carries on (`docs/testing/live-test-checklist.md`), now by anyone who runs the server.

**2026-11-02, the maintainer: there is no player alpha.** The plan below was written when a hosted alpha (roadmap B1 to B4) was to
follow the release; instead the server files are released for the community to try. The mentions of the alpha below are kept as they
were decided at the time; `docs/plans/roadmap.md` is the current plan.

## Decisions (the maintainer, 2026-10-31)

1. **"Release" means the open-source repository.** The player alpha follows it (`docs/plans/roadmap.md`, "After the release").
2. **As close to the original EP4.5 as possible.** The release defaults to the original's behaviour, with three kinds of deviation
   treated differently:
   - **fixes stay on**: the original's crashes, duplication holes, exploits, wrong packets for this client, data loss, and the
     robustness limits (rate limits, timeouts, connection caps);
   - **gameplay changes** become settings whose default is the original (they stay available to an operator who wants them);
   - **additions** (new items such as the Kill Boost Stones, new rules, new content) are off by default and move to the extension work
     after the release.
3. **Balance changes wait** (`docs/plans/balance/plan.md`): possibly after the alpha, and then as extension work, not as changes to the core.
4. **The README is written for a stranger with a fresh clone**: from `git clone` to logging in with the real client. The settings
   tables live in `docs/server/settings.md`.
5. **Everything in the repository stays** (the analysis notes, `docs/original/symbols`, `docs/original/db`, `docs/original/inventory`), **after a comprehensive check
   that cleans and organises it** (step R4).
6. **Fresh history**: the public repository starts from one commit of the cleaned tree; the private repository keeps the full history.

## Where things stand

Done: the README rewritten for a fresh clone, `docs/server/settings.md`, `.env.example`.

What a stranger would still hit today, found while writing the README:

| gap | step |
|---|---|
| several defaults differ from the original: the gambling house shown with fairer odds, cross-faction trade on map 42, personal shops limited to three maps, smooth chasing, the Kill Boost Stones and their rows in `tools/defs/additions.sql`; about 200 deviations are written down across the analysis notes, never collected in one list | R1 |
| the import scripts hard-code the container names `newshaiya-postgres` / `newshaiya-mssql` and the database user and name; there is no compose file; the README's install path has never been followed on a clean machine | R2 |
| 14 clippy warnings; `zone.rs` 7,836 lines, `server.rs` 5,308, `pg.rs` 4,117; no CI | R3 |
| personal details in tracked files (LAN addresses, the docs-site host and user, local paths, the test password) in about 19 files; workflow notes that name the AI models used (116 lines in 18 files, mostly the roadmap and its briefs); dated status stamps and known stale claims in docs/; 70 notes in one flat `docs/` folder; the dev box's trial edits in `db/edits/edits.json` | R4 |
| no LICENSE file (`Cargo.toml` says MIT), no CONTRIBUTING or SECURITY notes, no version tag | R5 |
| the public repository itself, its name and first commit; the docs site's link to it | R6 |

## Steps

Each step is a spec that settles the choices with the maintainer, then a build that follows it. Nothing is pushed or published
without the maintainer's OK.

### R1. Fidelity pass: the original as the default

**Spec: done**, `docs/server/deviations.md` (the list, the maintainer's decisions, and its Build section for one build session). What
the spec was asked to cover:

- Collect every deviation into one public list, `docs/server/deviations.md`: from the deviation lists of every `docs/original/*.md`, every
  setting in `docs/server/settings.md` whose default differs from the original, `tools/defs/additions.sql`, and what the code does that no note
  calls a deviation (search for behaviour the original lacks: packets the original never sends, extra checks, extra commands).
- Classify each one with the rule of decision 2: **fix** (on), **gameplay** (setting, default original), **addition** (off; extension
  later), or **operator tooling** that replaces one of the original's separate programs (the data editor, the console tool, the game log in
  PostgreSQL, the mall tool: stays). Ask the maintainer about every borderline item, with AskUserQuestion, explained plainly. Known candidates:
  `GAME_MINI_GAME` (the original hides the type-5 NPCs), `GAME_SLOT_PAYOUT_PERCENT`, `GAME_CARD_WIN_CHANCES`,
  `GAME_CROSS_FACTION_TRADE_MAPS`, `GAME_MARKET_MAPS`, `GAME_SMOOTH_CHASE`, `GAME_KILL_BOOST_SKILLS` and the stone rows, the
  ultimate-mode fall (kept and restorable instead of deleted), monster HP bars with `0x0608` if sent, the in-game `.commands`, the
  `F502` chat copies, `GAME_KICK_NOTICE_SECS`.
- Write the build section: each default to change, each addition to gate (with its setting), what `tools/defs/additions.sh` loads by
  default, the tests to update, `docs/server/settings.md` and the README to update. The dev box's own `.env` may keep today's behaviour.

**Build** (one or two sessions): change the defaults, gate the additions, update the tests and the documents, rebuild and restart the dev
servers. **Built**: the release defaults with a unit test (`config.rs` `the_defaults_are_the_original`), `additions.sh
--with-additions`, `docs/server/settings.md` complete, the dev `.env` keeping the old values. Afterwards (the maintainer, 2026-10-31, with R2):
`GAME_MINI_GAME` back on by default, with the original's odds.

Done when: `docs/server/deviations.md` lists every deviation with its kind, setting and default; a server started with an empty `.env` (apart
from `DATABASE_URL`) behaves as the original except for the fixes and the tooling.

### R2. Fresh-clone setup path and the install drill

**Spec'd; one build session.** Touches `tools/`, `compose.yaml`, `.env.example`, the README; not `crates/`.

Build:

1. **One way to reach the databases, shared by every tool.** A small Python module `tools/dbconn.py` (standard library only) and a
   bash file `tools/dbconn.sh` with the same rules:
   - read `.env` from the repository root (simple `KEY=value` lines, `#` comments, optional quotes) for anything not already in the
     environment;
   - `PG_CONTAINER` (default `newshaiya-postgres`), `MSSQL_CONTAINER` (default `newshaiya-mssql`);
   - user, password, host, port and database name parsed from `DATABASE_URL`;
   - `NEWSHAIYA_PSQL` = `docker` | `host` | `auto` (default `auto`): `docker` runs `docker exec -i [-e VAR=…] $PG_CONTAINER psql -U <user>
     -d <db>` as today; `host` runs the host's `psql "$DATABASE_URL"`; `auto` uses `docker` when that container is running, else `host`
     when `psql` is on the PATH, else stops with a plain message naming both options;
   - a `psql_cmd(extra_env=None)` (Python) / `psql_cmd` (bash) giving the command, plus `-v ON_ERROR_STOP=1 -q` where the callers use
     them today.
   Switch every caller to it: `tools/defs/load_into_postgres.py`, `tools/defs/verify.py` (both the PostgreSQL and the SQL Server side:
   `MSSQL_CONTAINER`), `tools/defs/export_from_mssql.sh`, `tools/defs/additions.sh`, `tools/world/import.py`,
   `tools/sdata/load_client.py`, `tools/sdata/client_patch.py` (keeps its `PGCLIENTENCODING=UTF8`), `tools/balance/data.py`, and the
   comments in `tools/restore_backups.sh`. Behaviour with today's `.env` must not change: run `python3 tools/world/import.py <PS_Game Data>
   --dry-run` and `tools/defs/verify.py` against the dev database before and after.
2. **`.env.example`**: add `PG_PASSWORD` (used by the compose file; must equal the password in `DATABASE_URL`, said in a comment),
   `PG_CONTAINER`, `MSSQL_CONTAINER`, `NEWSHAIYA_PSQL`, `PG_PORT`, `MSSQL_PORT`, `PG_VOLUME`, `MSSQL_VOLUME` (commented, with their
   defaults), `PSM_DATA_DIR` (the folder of the SQL Server backups, for the compose file).
3. **`compose.yaml`** at the root: service `postgres` (`postgres:16`, `container_name: ${PG_CONTAINER:-newshaiya-postgres}`, the
   README's environment, `127.0.0.1:${PG_PORT:-5433}:5432`, a volume with `name: ${PG_VOLUME:-newshaiya-pgdata}`, `restart: unless-stopped`) and service `mssql`
   under `profiles: [import]` (`mcr.microsoft.com/mssql/server:2022-latest`, the README's environment with `MSSQL_SA_PASSWORD`,
   `127.0.0.1:${MSSQL_PORT:-14333}:1433`, `${PSM_DATA_DIR}:/backups:ro`, a volume with `name: ${MSSQL_VOLUME:-newshaiya-mssql-data}`,
   `mem_limit: 2200m`). The defaults equal today's names; **never run it on the dev box with the defaults** (it would adopt the dev volumes).
4. **`tools/setup.sh <PSM_Data folder> <PS_Game Data folder>`**: checks docker, cargo, python3 and `.env` first (each missing one named
   with what to install); `docker compose up -d postgres`; builds the workspace (release); `target/release/servers list` (tables);
   `docker compose --profile import up -d mssql` and waits for SQL Server by retrying a `sqlcmd -Q "SELECT 1"` inside the container every
   10 s for up to 10 minutes; `restore_backups.sh`; the export, load and verify; `additions.sh` (without `--with-additions`); the world
   import; `docker compose stop mssql`; then prints the two commands left to the operator (`create_account`, `servers add`) and the
   start commands. Stops at the first failure with the step's name. Idempotent: a second run replaces `defs` and `world` and skips the
   restore of databases already there (as `restore_backups.sh` does).
5. **README**: step 2 offers `docker compose up -d postgres` beside the `docker run` line; step 4 the compose `import` profile; a short
   "or all at once: `tools/setup.sh <PSM_Data> <PS_Game/Data>`" before step 1's details; the settings for container names in
   `docs/server/settings.md` (a "Tools" section: `PG_CONTAINER`, `MSSQL_CONTAINER`, `NEWSHAIYA_PSQL`, `PG_PASSWORD`, `PSM_DATA_DIR`).
6. **The install drill** (the real check of the README). This box has 8 GB and runs the dev servers; keep it to one SQL Server at a
   time (the dev `newshaiya-mssql` stays stopped).
   - `git clone <the repository> <a scratch folder>/drill` (the committed tree, so uncommitted files cannot help).
   - Its `.env`: `PG_CONTAINER=drill-postgres`, `MSSQL_CONTAINER=drill-mssql`, `PG_PORT=5434`, a `DATABASE_URL` on port 5434, new
     passwords, `PSM_DATA_DIR=<the original files>/Server/PSM_Data`, `COMPOSE_PROJECT_NAME=drill`,
     `PG_VOLUME=drill-pgdata`, `MSSQL_VOLUME=drill-mssql-data`, `MSSQL_PORT=14334`, so **nothing named `newshaiya-*` is created, touched
     or removed**. Before the first `docker compose` call, print `docker compose config` and check every name and port.
   - Follow the README word for word (the compose path or `tools/setup.sh`, then the account and the server entry with `127.0.0.1`).
   - Before starting the drill's servers, check nobody is on the dev server (`target/release/console 1 info` from the dev tree; with a
     player online, stop and ask), stop the dev login and game servers, start the drill's (its `tools/*.sh`), run `probe` and
     `game_probe` to the character screen, create a character with `game_probe ... create Drill`, stop the drill's servers, start the
     dev servers again (output redirected) and check their logs.
   - Clean up: `docker compose -p drill down -v` (drill volumes only), remove the scratch clone.
   - Every place where the README, `.env.example` or a script was wrong or unclear is fixed in the main tree; the drill is repeated
     until it passes without a step outside the README. The drill's outcome (passes, fixes made, times of the long steps) goes in the
     commit message.
7. Commit (no AI mention or trailer), push nothing.

Done when: the drill passes from a clean clone with only the README, and the dev tools still work with the dev `.env`.

**Built**: `tools/dbconn.py` / `tools/dbconn.sh` and every caller switched (the world import's dry run and `verify.py` give
the same output as before on the dev database), `compose.yaml` (both services on Docker's default bridge: the drill box had no address
pool left for a compose network, and the tools need none), `tools/setup.sh`, `.env.example`, the README and `docs/server/settings.md`
"Tools". The drill passed from a clean clone on the second run, with `tools/setup.sh` (about 28 minutes: the build about 4, SQL Server
ready in about 1.5, the restore of the ten backups about 21, the rest under a minute). SQL Server does not stop on `docker compose stop`
within any grace period tried (10 s, 60 s) and is killed (exit 137); harmless after the import, so the default stays. The maintainer also turned `GAME_MINI_GAME` back on by
default (with the original's odds).

### R3. Code health and CI (the old A4)

**Spec'd; one build session, then a review of the diff.** Touches `crates/`, `rustfmt.toml`,
`.github/`. Runs after R1; can run before or after R2 (different files).

Build, each item its own commit, every commit behaviour-neutral, the full unit and `game_flow` tests green after each:

1. **Clippy clean**: today 14 warnings (7 "very complex type" in `data-editor`, one each of collapsible `if`, too many arguments, `>= y +
   1`, negated partial-order comparison, empty format literal, `.err().expect()`, and the rest). Fix them; where a fix would change a
   signature used in many places (too many arguments), an `#[allow(clippy::too_many_arguments)]` with a one-line reason is fine. Then
   `cargo clippy --workspace --all-targets -- -D warnings` passes.
2. **rustfmt**: the code is not rustfmt-formatted (about 5,000 diffs with the default width). Add `rustfmt.toml` with `max_width = 140`
   and `use_small_heuristics = "Max"` (closest to the current style; try `edition = "2024"` too) and run `cargo fmt --all` once, alone in
   its commit ("rustfmt, no other change"). The public history starts after this, so the churn costs nothing.
3. **Split the three big files along their existing seams**, moves only (no renames of public items, no logic edits; `pub(super)` /
   `pub(crate)` added only where a move needs it):
   - `crates/game-server/src/zone.rs` (7,836 lines; the tests start at line 4002): move the `#[cfg(test)] mod tests` into
     `zone/tests.rs` (`#[cfg(test)] mod tests;`). If the rest is still over about 4,000 lines, move the monster helpers (`mob_enter` to
     `fight_turn` and their neighbours, lines ~696-1000) into `zone/mobs.rs`.
   - `crates/game-server/src/pg.rs` (4,117): one file per store impl under `pg/`: `session.rs` (`SessionGate`), `character.rs`
     (`CharacterStore`, `ListRow`, `PlayRow`), `items.rs` (`ItemStore`, `ItemRow`), `skills.rs`, `guild.rs`, `friends.rs`, `quests.rs`,
     `heartbeat.rs` (with whatever lies between it and `AdminStore`), `admin.rs`, `market.rs` (`MarketItemCols`, `MarketStore`),
     `points.rs`; `pg.rs` keeps `PgBackend`, shared helpers and the tests.
   - `crates/game-server/src/server.rs` (5,308): the background tasks and loaders (`serve`, `shut_down`, `start_guilds`,
     `ranking_clock`, the mob-id saves, `heartbeat`, the `load_*` functions, `market_pass`, the save writers, `obelisk_events`, lines
     ~228-700) into `server/tasks.rs`, re-exported from `server.rs` so `main.rs` does not change; the tests into `server/tests.rs`.
   Check after each move: `cargo build`, `cargo test -p game-server --lib`, `game_flow`, clippy.
4. **"Not built" branches stay**: they refuse what the original has and this server does not (an item effect, a skill kind); they are
   not dead code. Remove only what the compiler or clippy reports as unused.
5. **CI**: `.github/workflows/ci.yml`, on push and pull request: `ubuntu-latest`, the stable toolchain with clippy and rustfmt
   (`dtolnay/rust-toolchain@stable`), `Swatinem/rust-cache@v2`, then `cargo fmt --all -- --check`, `cargo clippy --workspace
   --all-targets -- -D warnings`, `cargo test --workspace` with a `postgres:16` service and `DATABASE_URL` pointing at it (the suites
   that need `defs` / `world` skip themselves without those schemas, as `crates/game-server/tests/postgres.rs` does; check the
   login-server and data-editor suites do the same, and make them skip the same way if they do not). Pin action versions by tag. The
   workflow cannot run until a push, which needs the maintainer's OK: run the same commands locally instead, including the test suite against an
   empty scratch PostgreSQL (a throwaway `docker run --rm -d --name ci-postgres -p 127.0.0.1:5435:5432 postgres:16`, removed after) to
   prove the skips work.
6. Update `README.md` "Development" (fmt, clippy with `-D warnings`, CI), rebuild and restart the dev servers (output redirected), commit
   (no AI mention or trailer), push nothing.

**Review** (one session): read the R3 diff (`git diff <before R3>..HEAD -- crates/` without the rustfmt commit, `-M`
for moves) and confirm every change is a move or a lint fix; anything else is reverted or reported.

Done when: fmt and clippy clean with `-D warnings`, the full test suite passes locally (the long PostgreSQL run included), the empty-
database run shows the skips, the review finds no behaviour change.

**Built**: clippy clean with `-D warnings` (type aliases for the wide query
rows, one `too_many_arguments` allow on the data editor's log writer); `rustfmt.toml` and one `cargo fmt --all` (133 files;
`edition = "2024"` gives the same result as without it); `zone.rs` 8,308 lines (after the format) to 3,744 with `zone/tests.rs` and
`zone/monsters.rs` (named `monsters`, since `mobs` clashes with the `crate::mobs` import); `pg.rs` 4,236 to 725 with eleven files under
`pg/` (the helpers several stores share stay in `pg.rs`, the module lines sit below its `macro_rules!`); `server.rs` 5,397 to 4,829 with
`server/tasks.rs` and `server/tests.rs`; a second fmt-only commit for the de-indented tests and new import lists; CI in
`.github/workflows/ci.yml`. Nothing was reported unused, so no code was removed. The CI commands, run from a clean clone (no `.env`)
against an empty PostgreSQL 16: fmt and clippy clean, every suite passes, the `defs` / `world` tests print their skips (about 48
minutes, 36 of them the game-server PostgreSQL suite). The full suite against the dev database: all pass in about 9 minutes. Log
targets of moved code now carry the new module path (for example `game_server::server::tasks`); the run scripts
filter by crate, so nothing changes there. For R4: two stray doc lines (the docs of `load_skill_defs` and `load_quest_defs`) sit above
`QuestRowSaved` in `pg/heartbeat.rs`, as they did in `pg.rs`.

**Reviewed**: no behaviour change found, nothing reverted. The clippy commit: type aliases, one
`allow`, `expect_err`, and three rewrites checked for equivalence (`partial_cmp(..) != Some(Less)` is `!(a < b)` with NaN still
passing; `payload.len() > len` is `>= 1 + len` for a `usize` at most 255; the `heartbeat` header is the same text as a literal; the
let-chain is the nested `if`). The two fmt commits are byte-for-byte what `cargo fmt --all` gives on their
parents. The three splits were matched line by line (whitespace aside) against the old files: every
line moved in its original order, within runs that break only at item boundaries, and the only new lines are module docs, `mod` /
`use` / `pub use` lines and `pub(super)` on the items the commit messages name; the only dropped lines are those items' old
declarations and the `mod tests {` wrappers. Every string literal (the embedded Lua and SQL included) is identical before and after
each split, so the de-indent touched no string. Log targets of moved code are one module deeper; the run scripts filter by crate, which
still matches. fmt and clippy (`-D warnings`) clean; `cargo test --workspace` against the dev database: 1,021 passed, 0 failed, 9
ignored (the deep fuzz twins and the A* timing test).

### R4. Repository review and organisation (the maintainer's "comprehensive check"): built

**Reviewed**: every tracked file (489) was looked at; the decisions are below, then the table, then the build in two parts
(A: the move, the links and the generated data, mechanical; B: the content). Each part is one build session; each ends with its commits
made locally and nothing pushed. Another session may be editing the same checkout: before each commit, `git status`, then stage only
the paths this step changed (never `git add -A`).

#### Decisions (the maintainer, 2026-11-01)

1. **`docs/` by purpose, with short names.** `docs/original/` (what the original EP4.5 server does: the 60 analysis notes lose their
   `analysis-` prefix, so `docs/analysis-shop.md` becomes `docs/original/shop.md`, the same length, which keeps code comments inside
   their width; plus the reference notes and the raw data `symbols/`, `db/`, `inventory/`), `docs/server/` (running this server),
   `docs/plans/` (the roadmap, the plans, `balance/`, `done/`), `docs/testing/` (the live tests and the client research queue),
   `docs/client/` stays where it is. The full map is the "Move map" block below.
2. **The docs site's page addresses change** with the move (`/analysis-shop/` becomes `/original/shop/`); no redirects.
3. **Finished build plans stay as history, trimmed**, in `docs/plans/done/`: a one-line header ("Built; the behaviour is described in
   `docs/original/<note>.md`"), the design reasoning kept, the working rules, session instructions, briefs, page links and test
   accounts cut.
4. **Session briefs**: a brief that has been run is removed; one that has not becomes a plain work item (what to do, the files, what "done"
   means; no model, session or relay wording). Checked one by one:
   - run, removed: the roadmap's R1, R2, R3 builds, the R3 review and the R4 review; the parked-briefs file's 3a, 3b-early, 3b, 3c,
     3c-site and the balance analysis; `docs/design-extensions.md` 7.0 (the client-side session: queue entries 8 to 21 are answered in
     `docs/client/client-research-20261026-extension-features.md` and `-20261027-balance.md`); the client patch 109 brief at the end of
     `docs/plan-client-patches.md` (`docs/client/client-patch-0109.md` records the patch packed, signed and accepted); the part 2 and
     part 3 briefs at the ends of `docs/analysis-balance-1-formulas.md` and `-2-content.md` and the "(Before:)" sentence of `-3-classes.md`;
     the briefs and session steps inside the ten finished plans; the client-side brief at the end of `docs/client-research-queue.md`.
   - not run, work items: A3 (the smoke walk, deferred to the alpha), B1 (web API), B2 (alpha operations), the test bots (from
     the parked-briefs file into the roadmap's "Frozen"), extensions chunk 1 (spec written, `docs/plan-ext-1-foundation.md`, not built)
     and chunks 2 to 9 (`docs/design-extensions.md` 7.2 to 7.9), the balance plan's build (`docs/plan-balance.md`), and this R4 build
     until it is done. the parked-briefs file is then empty and is removed.
5. **Test accounts are generic.** The checklist, the solo list and the smoke list get one "Test setup" section: the accounts and
   characters to create (a GM account, two player accounts; each character's race, class, level, map and position as the items need),
   no password; the items say "the GM account", "the second player". The page's LAN address becomes `http://<the box>:30880/`.
6. **The docs site URL stays** (`tylerd.me/shaiya/`, in the kit README and, at R6, in the README). `site/deploy.sh` loses the default
   key path and the mention of other sites: the key comes from `$SHAIYA_KEY` or the git-ignored `site/.deploy-target`, nothing else.
7. **`docs/client` is cleaned here, by the build**, wording and paths only (no change to findings): the dev PC's paths, the network
   share, the AI-tool mentions, the client repository's tools named as such, the date rule of decision 10, and the six dated file names
   renamed by topic. `docs/client/README.md` gets a short note so the client side writes the same way from now on.
8. **`db/edits/edits.json` is removed** (one trial edit and its undo; it changes nothing) and `db/edits/` is git-ignored, so the dev
   box's `EDITOR_EXPORT_FILE` keeps writing there unseen.
9. **"the maintainer" names the person deciding** (239 places used another word, most in `docs/`), by script with each changed line checked by hand: a
   player, an account or a client's user stays.
10. **Dates: nearly all go.** Kept: decision dates in `docs/plans/` ("Decisions (the maintainer, 2026-10-31)", "decided 2026-10-26").
    Removed everywhere else, `docs/client` included: dates in titles and after "new", "asked", "Built", "Test", "Log review",
    "Date:", "answered" and "updated". Not project
    dates, so kept: facts about the original files (the PE build dates 2009-06-10, 2009-07-14/15, the client's 2010-04-26, the backups'
    2010-03-12) and test fixtures in code. **Commit hashes go everywhere** (the public repository starts from one commit, so every hash
    would point nowhere): "Built in the commit that follows <hash>" becomes "Built".
11. **`docs/db` gets its script back**: `tools/defs/describe_mssql.sh`, proven to reproduce the 30 files byte for byte from the
    restored databases.
12. **`tools/` keeps its layout** (`tools/world/import.py` imports `svmap.py` and `npcquest.py` from `tools/`), with a
    `tools/README.md` and a `tools/requirements.txt`.
13. Small calls, all approved: the two dev LAN addresses in the Rust tests become other private addresses; the local name of the
    original files' folder becomes "the original files" (`Server/`, `Client/`, `Docs/`) and the patch kit test finds
    `ps_game.exe` only through `PS_GAME_EXE` (it already reads it) or next to itself; a tool's local folder leaves `.gitignore` (the dev checkout
    keeps it in `.git/info/exclude`); the live-test page drops its old hosted-page saving mode and its chat wording;
    `docs/architecture.md` ("the game server (not written yet)") is rewritten from the code; made-up test character names
    in code (ProbeHero, TestVail) stay.

#### What the review found

- **Already fixed**: the three known stale claims (the party "stack garbage", the mob-fight `0x0552` note, guild-war's item special 59)
  were corrected by the docs audit, which also merged the 63 server pages into one copy each in `docs/`. Part B only
  confirms them.
- **Workflow and model mentions** (model names, session briefs, working rules, hosted-page links, the old page mode): about 165 lines in 27 files (a few are false hits), mostly
  `docs/design-extensions.md` (47), `docs/roadmap.md` (39), the parked-briefs file (12), `tools/checklist_page.html` (11), the plans,
  `.gitignore`, and three tool mentions in `docs/client/client-patch-0109.md`. None in `crates/`.
- **Personal details**: the dev box's LAN address in `docs/live-test-checklist.md`, `docs/smoke-list.md`,
  `docs/client/client-patch-0109.md` (a network share path) and three Rust tests; the dev box's project paths in 8 notes and
  `tools/server-patches/test_patch_ps_game.py`; the dev PC's Windows paths (15) in `docs/client`, the queue, two plans and
  `site/build.py`'s `redactions`; the local name of the original files' folder in 8 notes and `login-server/src/status.rs`; the
  local clones of other projects' repositories (`reference/...`) in 15 files; the deploy key's path and the maintainer's other sites in
  `site/deploy.sh`; the maintainer's account name in `docs/plan-obelisk-bosses.md` and R6; the private repository's name in
  the parked-briefs file and R6; the test password in 6 files. Not personal, kept: `172.17.10.205` (the original developers' address inside `game.exe`), the
  addresses found in the original files (`docs/server-inventory.md`), "Tailscale" as a kind of address the tools allow, the site's
  dedication on its home page.
- **Links**: of 606 Markdown links, 65 do not resolve as plain files, and all of them resolve on the site: 38 in `docs/` point at
  site-only pages (`server-patches.md`, `../game-exe-patch-kit.zip`), 24 in `site/content/` are relative to the `docs/` root as the site
  stages it, 3 are not links (`10`, `WSAGetLastError(`, the `KIT_ZIP` token). Of 1,227 backticked repository paths, 41 point at
  nothing: 28 are the client repository's own tools (`tools/sah.py`, ...), 7 are files `docs/plan-ext-1-foundation.md` will create,
  `docs/server-patches.md` (renamed `server-patch-notes.md`) twice, `docs/design-extensions-catalogue.md` and
  `docs/plan-ext-9-personal-loot.md` (planned), `docs/client/client-patch-NNNN.md` (a pattern), `tools/README.md` (part B makes it). Of 530 "`docs/x.md` section N" references, 6 name a section that has no numbered heading (all into `docs/client` pages:
  `docs/analysis-operator-console.md:126`, `docs/client/server-notes.md:165`, `docs/plan-client-patches.md:70` and `:279`,
  `docs/plan-gm-commands.md:4`, `docs/plan-gm-commands-2.md:6`).
- **The research queue is behind**: entries 8 to 21 are still under "Open" although the client side answered them; entry 3 waits for
  live test 46.
- **`docs/architecture.md` is from the first day** and describes a server that did not exist yet.
- **`docs/db`**: made from SQL Server's catalogue by queries that were never kept; `tools/defs/generate.py` and `verify.py`
  read `PS_GameDefs.columns.txt` from it. **`docs/symbols`**: made by `tools/pdb_symbols.py` (the five `.tsv`) and `tools/pdb_types.py`
  (`ps_game.packets.txt`, its command in its header) from the original executables and PDBs; current by nature (the originals do not
  change). **`docs/inventory`**: made by `tools/inventory_server.py` (and `pdb_check.py` for `pdb_identity.txt`) from the server pack
  (`server/`) and the Omega repack (`omega/`). None has a README. The PDB tools need `pefile`, which nothing installs.
- **`tools/`**: every script is still used or made something in `docs/` (table below); no README; the three `tools/sdata/compare_*.py`
  are referenced by nothing but made the `docs/client/*-vs-*.md` comparisons.

#### The table

Every tracked file. "Script" = the reference rewrite of part A (paths only). Rows that name a folder cover every file in it unless a
row below names one.

| path | decision | what |
|---|---|---|
| `.env.example` | keep | script (its `docs/settings.md` mention) |
| `.github/workflows/ci.yml` | rewrite | add a step `python3 tools/check_links.py` before the cargo steps |
| `.gitignore` | rewrite | drop a tool's local folder; add `db/edits/` and `notes/` (B12) |
| `Cargo.toml`, `Cargo.lock`, `rustfmt.toml`, `compose.yaml` | keep | |
| `README.md` | rewrite | script; its documentation pointers lead to `docs/README.md`; no other change |
| `crates/**` | keep | script (about 980 `docs/` paths in comments, the data editor's refusal texts and one test message); the rows below |
| `crates/data-editor/src/serve.rs` | rewrite | the dev box's address in the private-address tests becomes `192.168.0.10` |
| `crates/game-server/tests/postgres.rs`, `crates/login-server/tests/postgres.rs` | rewrite | the dev PC's address becomes `192.168.0.20`; any commit hash in a comment goes |
| `crates/login-server/src/status.rs` | rewrite | the comment's local `Docs` folder name becomes "the original files' `Docs/` (the configuration guide)" |
| `crates/game-server/src/config.rs` | rewrite | the `GAME_MINI_GAME` doc names the maintainer's choice instead of a dated remark |
| `crates/game-server/src/pg/heartbeat.rs` | rewrite | the two stray doc lines above `QuestRowSaved` move onto `load_skill_defs` and `load_quest_defs` (R3's note) |
| `crates/game-server/src/mini_game.rs`, `drops.rs`, `crates/login-server/src/pg.rs` | rewrite | commit hashes in comments go (find them with the check of B6) |
| `db/migrations/**`, `db/defs/schema.sql`, `db/world/schema.sql`, `db/client/schema.sql` | keep | script |
| `db/edits/edits.json` | remove | decision 8 |
| `docs/**` | move | the move map; then part B per file |
| `docs/README.md` (new) | add | what each folder holds, where to start, the status words (confirmed / verified / inferred / PDB), how the site picks its pages |
| `docs/original/*.md` (60 analysis notes) | keep | moved; script; dates and hashes out (B6); the maintainer (B5); local paths (B3) |
| `docs/original/protocol-notes.md`, `community-fixes.md`, `server-patch-notes.md`, `community-guide-notes.md`, `ep6-core-packet-catalogue.md` | keep | moved; script; B3, B5, B6 |
| `docs/original/database-notes.md`, `server-inventory.md`, `client-login.md` | rewrite | moved; the "Date:" lines go; the local folder name (B3); the dev container's name and "SA password in `NewShaiya/.env`" become the `.env.example` names |
| `docs/original/symbols/*`, `db/*`, `inventory/**` | keep | moved; regenerated and compared (A5); a README each |
| `docs/server/architecture.md` | rewrite | from the code (B9) |
| `docs/server/settings.md`, `deviations.md`, `deploy.md`, `audit.md` | keep | moved; script; B5, B6; `settings.md`'s `EDITOR_EXPORT_FILE` row loses "(the dev box uses `db/edits/edits.json`, committed ...)" |
| `docs/plans/roadmap.md` | rewrite | B2: no model column, no relay paragraph, no briefs; work items; the "Done" table keeps the old step numbers; R4 marked built |
| `docs/plans/release.md` | rewrite | B1 (R1 to R3's model names become "one build session", "a review of the diff"); R6's private repository name becomes "the private repository"; the R2 drill's dev-box paths become `<the original files>/Server/PSM_Data`; the move map block stays (history) |
| `docs/plans/extensions.md` | rewrite | section 7: 7.0 and 7.1's brief go; 7.1 says "spec: `docs/plans/ext-1-foundation.md`; build: its Build section"; 7.2 to 7.9 become work items; the other model and path mentions (B1, B3) |
| `docs/plans/ext-1-foundation.md` | rewrite | the Build heading loses its model name; the working-rules paragraph goes; Part A / Part B briefs become one line each ("Part A: Build steps 1 to N") |
| `docs/plans/ideas.md` | rewrite | B1 (the lines naming models say "needs a spec first"); the the parked-briefs file pointer goes |
| `docs/plans/balance/1-formulas.md`, `2-content.md`, `3-classes.md` | rewrite | the ready briefs at the ends go (run); B5, B6 |
| `docs/plans/balance/plan.md` | rewrite | the Build heading loses its model name; working rules out; the "Build session" brief becomes a work item line |
| `docs/plans/done/*.md` (10) | rewrite | decision 3 (B7) |
| the parked-briefs file | remove | decision 4; the test bots item goes to the roadmap first |
| `docs/testing/live-test-checklist.md`, `live-test-solo.md`, `smoke-list.md` | rewrite | decision 5 (B4); the sentence about the old hosted page goes; B6 |
| `docs/testing/client-research-queue.md` | rewrite | entries 8 to 21 to "Answered" with where each answer is (B8); its brief goes; the Windows paths (B3) |
| `docs/client/*` | rewrite | decision 7 (B3, B6); six renamed (move map); the 6 broken section references (B10); keep each file's line endings (`README.md`, `client-patch-0109.md` are CRLF) |
| `site/build.py` | rewrite | the docstring's example path; `redactions` removed once `docs/client` has no Windows path (it names the paths itself) |
| `site/mkdocs.yml` | keep | script (the nav paths) |
| `site/README.md` | rewrite | script; the held-back list with the new paths |
| `site/deploy.sh` | rewrite | decision 6 |
| `site/kit/README.txt` | rewrite | "the fixes added on" a date loses the date |
| `site/content/*`, `site/.gitignore`, `site/requirements.txt` | keep | script (the content pages' links) |
| `tools/README.md` (new) | add | B10 |
| `tools/requirements.txt` (new) | add | `pefile` (pinned to the version A5 used), for `pdb_symbols.py`, `pdb_check.py`, `inventory_server.py` |
| `tools/check_links.py` (new) | add | A2 |
| `tools/defs/describe_mssql.sh` (new) | add | A5 |
| `tools/checklist_page.html` | rewrite | the old hosted-page mode (its `window.` storage API, its viewport meta, the sign-in, "paste into the chat" and "ask ... to sort these" texts) goes; the stand-alone copy keeps "Copy results" and says it does not save |
| `tools/checklist_page.py` | rewrite | `ROOT / "docs" / "live-test-checklist.md"` and the solo file get `"testing"`; its docstring's stand-alone note follows the page |
| `tools/server-patches/test_patch_ps_game.py` | rewrite | the two dev-box paths in `SEARCH` go |
| `tools/server-patches/build_fixes.py`, `fixes.json` | keep | script (the credits' paths); `fixes.json` regenerated and compared (A4) |
| `tools/sdata/client_sdata.py`, `load_client.py`, `seed_sdata.py` | rewrite | the local clone path of Parsec becomes "Parsec (github.com/matigramirez/Parsec, `src/Parsec/Shaiya`)" |
| `tools/balance/README.md`, `tools/balance/scenarios.py` | rewrite | the dates go |
| every other file under `tools/` | keep | script; listed in `tools/README.md` |

#### Move map

`git mv` each line's first path to its second (folders: every file in it); `(removed)` = `git rm` after B2 has taken what it needs.
Part A's script reads this block.

```

docs/analysis-auction.md  ->  docs/original/auction.md
docs/analysis-balance-1-formulas.md  ->  docs/plans/balance/1-formulas.md
docs/analysis-balance-2-content.md  ->  docs/plans/balance/2-content.md
docs/analysis-balance-3-classes.md  ->  docs/plans/balance/3-classes.md
docs/analysis-chase-smoothing.md  ->  docs/original/chase-smoothing.md
docs/analysis-client-extras.md  ->  docs/original/client-extras.md
docs/analysis-combat.md  ->  docs/original/combat.md
docs/analysis-duel.md  ->  docs/original/duel.md
docs/analysis-enter-world.md  ->  docs/original/enter-world.md
docs/analysis-friends.md  ->  docs/original/friends.md
docs/analysis-game-log.md  ->  docs/original/game-log.md
docs/analysis-gatekeeper.md  ->  docs/original/gatekeeper.md
docs/analysis-gm-commands-2.md  ->  docs/original/gm-commands-2.md
docs/analysis-gm-commands.md  ->  docs/original/gm-commands.md
docs/analysis-guards.md  ->  docs/original/guards.md
docs/analysis-guild-bank.md  ->  docs/original/guild-bank.md
docs/analysis-guild-house.md  ->  docs/original/guild-house.md
docs/analysis-guild-ranking.md  ->  docs/original/guild-ranking.md
docs/analysis-guild-war.md  ->  docs/original/guild-war.md
docs/analysis-guild.md  ->  docs/original/guild.md
docs/analysis-instances.md  ->  docs/original/instances.md
docs/analysis-item-drop.md  ->  docs/original/item-drop.md
docs/analysis-item-flags.md  ->  docs/original/item-flags.md
docs/analysis-item-mall.md  ->  docs/original/item-mall.md
docs/analysis-item-options.md  ->  docs/original/item-options.md
docs/analysis-item-origins.md  ->  docs/original/item-origins.md
docs/analysis-item-use.md  ->  docs/original/item-use.md
docs/analysis-item-windows.md  ->  docs/original/item-windows.md
docs/analysis-items.md  ->  docs/original/items.md
docs/analysis-lapis.md  ->  docs/original/lapis.md
docs/analysis-maps-npcs.md  ->  docs/original/maps-npcs.md
docs/analysis-mini-game.md  ->  docs/original/mini-game.md
docs/analysis-mob-fight.md  ->  docs/original/mob-fight.md
docs/analysis-mob-skills.md  ->  docs/original/mob-skills.md
docs/analysis-mobs.md  ->  docs/original/mobs.md
docs/analysis-move-check.md  ->  docs/original/move-check.md
docs/analysis-obelisks.md  ->  docs/original/obelisks.md
docs/analysis-operator-console.md  ->  docs/original/operator-console.md
docs/analysis-party.md  ->  docs/original/party.md
docs/analysis-pathing.md  ->  docs/original/pathing.md
docs/analysis-personal-shop.md  ->  docs/original/personal-shop.md
docs/analysis-portals.md  ->  docs/original/portals.md
docs/analysis-ps_game.md  ->  docs/original/ps_game.md
docs/analysis-ps_login.md  ->  docs/original/ps_login.md
docs/analysis-pvp-2.md  ->  docs/original/pvp-2.md
docs/analysis-pvp.md  ->  docs/original/pvp.md
docs/analysis-quest-drops.md  ->  docs/original/quest-drops.md
docs/analysis-quest.md  ->  docs/original/quest.md
docs/analysis-repair.md  ->  docs/original/repair.md
docs/analysis-scripted-mobs.md  ->  docs/original/scripted-mobs.md
docs/analysis-shop.md  ->  docs/original/shop.md
docs/analysis-skills-buffs.md  ->  docs/original/skills-buffs.md
docs/analysis-skills-special.md  ->  docs/original/skills-special.md
docs/analysis-skills-use.md  ->  docs/original/skills-use.md
docs/analysis-skills.md  ->  docs/original/skills.md
docs/analysis-small-items.md  ->  docs/original/small-items.md
docs/analysis-stats.md  ->  docs/original/stats.md
docs/analysis-thorns-fields-pvp.md  ->  docs/original/thorns-fields-pvp.md
docs/analysis-trade.md  ->  docs/original/trade.md
docs/analysis-union.md  ->  docs/original/union.md
docs/analysis-vehicles.md  ->  docs/original/vehicles.md
docs/analysis-warehouse.md  ->  docs/original/warehouse.md
docs/analysis-world.md  ->  docs/original/world.md
docs/architecture.md  ->  docs/server/architecture.md
docs/audit-2026-10.md  ->  docs/server/audit.md
docs/client-login.md  ->  docs/original/client-login.md
docs/client-research-queue.md  ->  docs/testing/client-research-queue.md
docs/client/<date>-operator-listen-install.md  ->  docs/client/operator-listen-install.md
docs/client/client-research-20261021-gambling-kill-boost.md  ->  docs/client/research-gambling-kill-boost.md
docs/client/client-research-20261021.md  ->  docs/client/research-movement-storages-auction.md
docs/client/client-research-20261026-extension-features.md  ->  docs/client/research-extension-features.md
docs/client/client-research-20261026-priest.md  ->  docs/client/research-priest-packets.md
docs/client/client-research-20261027-balance.md  ->  docs/client/research-balance.md
docs/community-fixes.md  ->  docs/original/community-fixes.md
docs/community-guide-notes.md  ->  docs/original/community-guide-notes.md
docs/database-notes.md  ->  docs/original/database-notes.md
docs/deploy.md  ->  docs/server/deploy.md
docs/design-extensions.md  ->  docs/plans/extensions.md
docs/deviations.md  ->  docs/server/deviations.md
docs/ep6-core-packet-catalogue.md  ->  docs/original/ep6-core-packet-catalogue.md
docs/ideas-parked.md  ->  docs/plans/ideas.md
docs/live-test-checklist.md  ->  docs/testing/live-test-checklist.md
docs/live-test-solo.md  ->  docs/testing/live-test-solo.md
docs/plan-auction.md  ->  docs/plans/done/auction.md
docs/plan-balance.md  ->  docs/plans/balance/plan.md
docs/plan-client-patches.md  ->  docs/plans/done/client-patches.md
docs/plan-data-editing.md  ->  docs/plans/done/data-editing.md
docs/plan-ext-1-foundation.md  ->  docs/plans/ext-1-foundation.md
docs/plan-gm-commands-2.md  ->  docs/plans/done/gm-commands-2.md
docs/plan-gm-commands.md  ->  docs/plans/done/gm-commands.md
docs/plan-guild-bank.md  ->  docs/plans/done/guild-bank.md
docs/plan-guild-ranking.md  ->  docs/plans/done/guild-ranking.md
docs/plan-obelisk-bosses.md  ->  docs/plans/done/obelisk-bosses.md
docs/plan-quest-drops.md  ->  docs/plans/done/quest-drops.md
docs/plan-release.md  ->  docs/plans/release.md
docs/plan-small-items.md  ->  docs/plans/done/small-items.md
(the parked-briefs file)  ->  (removed)
docs/protocol-notes.md  ->  docs/original/protocol-notes.md
docs/roadmap.md  ->  docs/plans/roadmap.md
docs/server-inventory.md  ->  docs/original/server-inventory.md
docs/server-patch-notes.md  ->  docs/original/server-patch-notes.md
docs/settings.md  ->  docs/server/settings.md
docs/smoke-list.md  ->  docs/testing/smoke-list.md
docs/symbols/  ->  docs/original/symbols/
docs/db/  ->  docs/original/db/
docs/inventory/  ->  docs/original/inventory/
```

#### Built

Built in two parts, each one session, by the decisions and the table above; the repository's last look is `python3 tools/check_links.py` and the
two greps of the "Done when" checks (kept in `notes/release-checks.txt`, git-ignored).

**Part A** (the move, the links, the generated data): `tools/check_links.py` (links, path references, section references, the site nav; baseline
267 errors: 187 of them the new paths this section names, 74 other missing paths, 6 section references); the 147 files of the move map moved by
`git mv` and 327 files rewritten by a script reading the map (root-relative paths, Markdown links resolved from the old place and written for the new
one, the nav, the six client note names); `fixes.json` regenerated by `build_fixes.py` with the read-only `ps_game.exe`, byte for byte the same;
the generated data reproduced from the original files by their tools and compared with `cmp` / `diff -r`, all byte for byte (the five symbol
`.tsv` and `ps_game.packets.txt`, the two inventories, and the 30 database description files by the new `tools/defs/describe_mssql.sh`), a README
each, `tools/requirements.txt` pinning `pefile 2024.8.26`; the folder READMEs. The checker stood at 40 errors after part A (the client
repository's tools, the removed briefs file, `site/dist/`, six section references) and the site built with 111 pages.

**Part B** (the content): the finished plans trimmed to the design record (ten files in `docs/plans/done/`), the roadmap and the extensions plan
turned from briefs into work items (the parked-briefs file removed, its test bots item under "Frozen"), the balance docs' briefs out, the
research queue's entries 8 to 21 moved to "Answered" with where each is answered, the checklist's Test setup with the three roles (the GM account and
two player accounts; five results of section 44 carried over to the reworded items), "the maintainer" where the person deciding is meant (the
documents of the original server, where the word means the player or the account, stay), dates (decision dates kept in `docs/plans/`, facts about
the original files kept), commit hashes (twelve real ones) and personal details out, `docs/client` cleaned (the dev PC's paths, the share, the tool
mentions, the client repository's tools named as such, the six section references fixed to their headings, a note in its README), the architecture
page rewritten from the code, `tools/README.md`, the live-test page without its hosted-page mode, `site/deploy.sh` without its default key,
`db/edits/edits.json` removed and `db/edits/` git-ignored, CI running the link check. The checker exits 0 (the `planned:` notes aside). The two
greps come out empty apart from three ordinary uses of a common word (`docs/client/graphics.md`, `docs/client/re-open-items.md`, `docs/server/audit.md`) and a
test fixture's timestamp in `crates/shaiya-proto/src/world.rs`.

What differed from this section's text: the zone is not a task per map (there is one `Zones` structure behind mutexes), so the architecture page
says that; `tools/sdata/load_client.py` keeps its default folder for the original client files (the git-ignored `reference` folder) and the two
environment variables `PARSEC_SEEDCONSTANTS` and `CLIENT_ORIGINALS` override it; `tools/check_links.py` skips `.gitignore`; the work items for the
data editor's open parts (map pictures, live test 53, the sections it did not build) are under "Open work items" in the roadmap.

### R5. Public-repository furniture: built

Decided by the maintainer: the copyright holder is `byyako`, and vulnerabilities are reported through GitHub's private vulnerability reporting (the
repository's Security tab; no email address).

Built: `LICENSE` (MIT, as `Cargo.toml` says), `CONTRIBUTING.md` (the original comes first, a change in behaviour is a setting or an extension, an
analysis note before code, tests and docs with the change, no original files in issues, pull requests or commits), `SECURITY.md` (the private route
above, what is in scope, supported versions), `CHANGELOG.md` with 0.1.0 (the version in `Cargo.toml`), and the README's Legal section pointing at
all four. A secret scan of the tracked tree (no gitleaks here, so patterns by `git grep`: private keys, cloud and GitHub tokens, JWTs, password and
token assignments, email addresses, private IP addresses, host names, large and binary files) found nothing to remove; what it listed is deliberate (the
original developers' address inside `game.exe`, addresses in the original's files and in tests, `tylerd.me` for the docs site, the patch kit's zip). The
B1 and B3 greps give the same result as at the end of R4, plus `byyako` in `LICENSE`.

Left for R6, because they need the public repository: the repository URL in the README and the badges, enabling private vulnerability
reporting in the repository's settings, and issue templates if wanted.

### R6. The release cut: built

Decided by the maintainer: the public repository is `byyako/shaiya-afterglow`; the single public commit and the tag are authored by `byyako` with the
GitHub no-reply address; development moves to the public repository afterwards and the private one is archived by the maintainer (nothing in the
private repository is changed by the release); issue templates (a bug report and a feature request, plus a link to the private vulnerability
route) are in `.github/ISSUE_TEMPLATE/`; the docs site is deployed after the push, its note replaced by a link.

Built: the README's clone address and CI badge; the public tree made in a separate directory as an orphan branch with one commit of the cleaned tree
(`git archive` of the private `main`, so no history and no old author), after the checks of R4 and R5 were run again (the two greps of
`notes/release-checks.txt`, `tools/check_links.py`, the site build, `cargo fmt`, clippy with `-D warnings` and the tests); tag `v0.1.0` on that
commit; the push, the repository's private vulnerability reporting switched on (`SECURITY.md` depends on it) and the first CI run; the docs site's
"an open-source server is coming" note replaced by a link to the repository.

## Order

R1 spec, R1 build; then R2 and R3 side by side; then R4 (after the code and tools settle, so the documents describe the final state); then
R5 and R6. About eight to ten sessions in all.

## After the release

Planned at the time: the player alpha (roadmap B1 web API, B2 operations, B3 tester client, B4 launch), dropped on 2026-11-02 (the
release is the community's to try); then the extension work: the balance plan, the
additions moved out in R1, pets and the rest of phase 4 (`docs/plans/roadmap.md`).
