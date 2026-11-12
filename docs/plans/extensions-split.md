# Extensions split: the content leaves the server repository (spec and build plan)

**Specified 2026-11-12; not built.** The server repository keeps the extension *system* and the engine work any extension needs; the
extensions themselves (the folders under `extensions/`) move to a repository of their own and are released on their own.

## 1. The maintainer's decisions (2026-11-12)

1. **Extensions are content, not server work.** They go to their own repository. What an extension needs from the server (an item
   effect, a rule point, a Lua function, a table, a migration) is still built here, as before.
2. **No new extension content in this repository.** `extensions/golden-hunt` (written 2026-11-10, never committed here) and
   `docs/plans/taming.md` start the new repository instead.

3. **The name is `byyako/shaiya-afterglow-extensions`**, started with a fresh first commit that names the server commit the folders
   came from (as the server's own release did, `docs/plans/release.md` R6), not the folders' history.
4. **Kill Boost moves with the rest.** It is the one extension the server's docs present as part of the server
   (`docs/server/deviations.md` section 1 names it among the settings that turn on what the original lacks, and patch 109 carries its
   rows), so those pages change to point at the new repository.

Still open: Q4 (section 4), how the new repository tags its releases.

## 2. The line: engine or content

| stays in the server repository (engine) | moves to the extensions repository (content) |
|---|---|
| the extension system: `crates/data-editor/src/ext/` (manifest, ids, rows, ops), `data-editor ext …`, `content rebuild` | the ten folders: `boss-rewards`, `golden-hunt`, `kill-boost`, `kill-streak`, `level-stones`, `monster-madness`, `perfect-lapisia`, `pets`, `stat-runes`, `welcome` |
| the item effects and skill abilities (`capture`, `pet_egg`, `enchant`, `recreate`, the level-up effect, `kill_boost`), the rule points, the Lua API (`crates/game-server/src/script/`) | each folder's `README.md` (the player- and operator-facing description of that extension) |
| the migrations (`0031` to `0036` and later) and the `content` schema | `docs/plans/taming.md` and any later content idea |
| `tools/sdata/client_patch.py`, `load_client.py`, `tools/setup.sh --extension <folder>` | a CI job that checks every folder against the latest server release (section 5) |
| the reference pages for authors and operators: `docs/server/effects.md`, `rules.md`, `scripts.md`, and `pets.md`'s engine part (the two effects, the two rule points, `character_pets`, `[[monster_look]]`) | `pets.md`'s description of the Pets extension as a player sees it, folded into `pets/README.md` |
| the design record: `docs/plans/extensions.md`, `ext-1` to `ext-5` | |
| frozen copies of the folders the tests read, as test fixtures (section 3.2) | |

A running server is not affected by where the folders are: `ext install` copies an extension's rows, rules, options and scripts into
the database, and the game server reads its scripts from there (`read_script_sources`), not from the folder. A folder is needed again
only for `ext upgrade` and `ext check`.

## 3. What changes in the server repository

### 3.1 The manifest says which servers an extension works with

Once the two repositories release separately, an extension needs to say which server versions it was written for, while 0.x still
allows breaking changes. Today the manifest has a Lua `api` number but nothing for the rest (effects, rule points, tables).

- A new optional key in format 1: `server = ">=0.4.0, <0.5.0"`, a semver requirement (`semver::VersionReq`), checked against the
  data editor's own version (`CARGO_PKG_VERSION`; the workspace has one version).
- `ext check`, `ext install` and `ext upgrade` refuse an extension whose `server` excludes this version: `pets 1.0.0 needs server
  >=0.4.0, <0.5.0; this is 0.5.0`. Without the key: a warning from `ext check` ("no `server` range"), no refusal.
- A data editor older than this change refuses the key as unknown (`manifest.rs` refuses unknown keys), which is the right outcome:
  that editor predates the range.
- The game server, at start, logs a warning for each installed and enabled extension whose stored manifest's `server` excludes the
  running version (it does not stop: the effects and rules it loads are checked at start anyway, and an operator who upgraded the
  server needs it running to fix things). `ext list` shows such extensions as `needs server …`.
- `docs/server/` gains the key in the manifest reference; `ext-1-foundation.md` section 5.2 gets a note.

### 3.2 The tests keep their folders, as fixtures

Four test files read `extensions/` directly:

| test | reads |
|---|---|
| `crates/data-editor/tests/scripts.rs` `every_bundled_extension_passes_check` | every folder; asserts the scripted ones are exactly `boss-rewards`, `kill-streak`, `monster-madness`, `welcome` (it fails today because the uncommitted `golden-hunt` is a fifth) |
| `crates/data-editor/tests/effects.rs` (Kill Boost 1.0.0 → 1.1.0) | `kill-boost` |
| `crates/game-server/tests/game_flow.rs` (`folder_effects`, `put_extensions`) | `level-stones`, `perfect-lapisia`, `stat-runes`, `kill-streak`, `monster-madness`, `welcome` |
| `crates/game-server/tests/postgres.rs` (the kill-boost rows, skipped when not installed) | names `extensions/kill-boost` in its messages |

They test the **engine** through real folders, which is worth keeping. The folders they read move to `tests/fixtures/extensions/` at
the workspace root (`boss-rewards`, `kill-boost`, `kill-streak`, `level-stones`, `monster-madness`, `perfect-lapisia`, `stat-runes`,
`welcome`), with a `README.md` saying: frozen copies for the engine's tests; the real extensions are in the extensions repository; do
not add content here. The paths in the four files change; `every_bundled_extension_passes_check` becomes
`every_fixture_extension_passes_check`. `pets` and `golden-hunt` are read by no test (`monster_look.rs` builds its own folder) and get
no fixture.

When an engine change alters what a fixture must say, the fixture changes here and the real extension changes in its repository; the
extensions repository's CI (section 5) is what notices a real extension left behind.

### 3.3 Setup, release and the client patch

- **`tools/setup.sh`**: `bundled_extensions` (the list of `extensions/*/` not installed yet) goes. `next_steps` prints instead where
  extensions are found (the repository's URL) and the one install line, `target/release/data-editor ext install <folder>`.
  `--extension <folder>` is unchanged (it already takes any path).
- **`.github/workflows/release.yml`**: the tarball no longer carries `extensions/` (`cp -r extensions` goes); `config/game.toml.example`
  stays. `tools/fetch-release.sh`'s comments about `extensions/` change to match.
- **`tools/sdata/client_patch.py`**: `cmd_generate` refuses uncommitted changes in `tools/sdata/`, `db/client/` or `extensions/`. The
  `extensions/` part goes: the ledger already records each installed extension's name and version (`content.client_patches.extensions`),
  which is what describes a patch once the folders live elsewhere. The message changes to match.

### 3.4 The docs

| file | change |
|---|---|
| `README.md` (8 mentions) | the extensions section points at the extensions repository; the install line takes a folder of that repository |
| `docs/server/deviations.md` section 1, `settings.md` | the Kill Boost Stones are an extension from the extensions repository |
| `docs/server/effects.md`, `scripts.md`, `rules.md`, `pets.md` | examples keep their text; paths `extensions/<name>` become `<extensions repository>/<name>`; `pets.md` keeps the engine part (section 2) |
| `docs/server/deploy.md` (2 mentions) | installing an extension on a VPS: clone the extensions repository (or unpack its release) and run `ext install` |
| `docs/testing/live-test-checklist.md` sections 58 to 63 | install lines name the extensions repository's folders |
| `docs/plans/roadmap.md`, `docs/plans/README.md` | this plan; "Next: extensions" says the content is in the other repository |
| `site/content/` | the Server tab's extension pages (roadmap "Open work items") are written for this layout |
| `CHANGELOG.md` | the 0.4.0 entry says the pets *engine* ships with the server and the Pets extension from the extensions repository; older entries stay as written |

`docs/plans/ext-*.md` and `CHANGELOG.md`'s past entries are the record of what was built and keep their paths.

## 4. The extensions repository

```
shaiya-afterglow-extensions/
  README.md            what the extensions are, which server versions this release targets, how to install one
  CHANGELOG.md         per extension, by its own version
  boss-rewards/ golden-hunt/ kill-boost/ kill-streak/ level-stones/ monster-madness/ perfect-lapisia/ pets/ stat-runes/ welcome/
  ideas/taming.md
  .github/workflows/check.yml
  LICENSE              MIT, as the server and each manifest's `license`
```

- **Versions.** Each extension keeps its own semver (`version` in its manifest); a repository release (`v2026.11.x` or `v0.4.0`, Q4
  below) is a tarball of all folders for one server release line. Every manifest gains `server = ">=0.4.0, <0.5.0"` at the move.
- **Dev box.** The live worktree (`../shaiya-afterglow-live`) and this checkout get a sibling clone, `../shaiya-afterglow-extensions`;
  installed extensions keep running (section 2), and the next `ext upgrade` names the new path.
- **Q4 (to confirm)**: release tags that follow the server's minor version (`v0.4.0` = "for server 0.4.x"), or dates. Following the
  server is assumed: an operator then picks the extensions release with the same minor version as their server.

## 5. The extensions repository's CI

One job on every push and pull request: download the latest server release for the targeted line (`tools/fetch-release.sh`'s
logic, or the release asset directly), then for each folder run `data-editor ext check <folder>` (manifest, id ranges against each
other, rows, rules, the scripts' dry run: what `every_bundled_extension_passes_check` does today, needing no database). A second job,
later: install every extension into a throwaway PostgreSQL with the server's migrations and start the game server once (the effects and
rule links are checked at start). This is what tells the maintainer that an engine change left an extension behind.

## 6. Build

One session for the server side, one for the new repository. Nothing is pushed or created on GitHub without the maintainer's go-ahead.

1. **Manifest `server` key** (3.1): `manifest.rs` (parse, `only_keys`, tests for a good range, a bad one, an excluding one, none),
   `ops.rs` (refusal in check, install, upgrade; `ext list` column), the game server's start-up warning, the docs.
2. **Fixtures** (3.2): `git mv` the eight folders to `tests/fixtures/extensions/`, their README, the four test files' paths, the renamed
   test. `cargo test --workspace` passes.
3. **Setup, release, client patch** (3.3).
4. **Docs** (3.4), `tools/check_links.py` clean.
5. **Leave the server repository**: `pets` and `golden-hunt` are removed from `extensions/` (the eight others already moved as
   fixtures); `extensions/` is gone. `docs/plans/taming.md` moves out. CHANGELOG 0.4.0.
6. **The new repository** (section 4): the ten folders from the server commit before step 2 (golden-hunt from the working tree),
   `server` ranges in every manifest, README, CHANGELOG, LICENSE, the check workflow. Run the check locally against a build of this
   repository.
7. **Dev box**: clone the new repository beside both worktrees; `data-editor ext list` on the live database unchanged; one `ext upgrade`
   of an extension from the new path on the drill.

**Done when**: the server repository has no `extensions/` and its tests pass; the extensions repository's check passes against the
current server build; the live dev box runs as before with its extensions installed from the new path; the docs name the new repository.

## 7. Effect on the 1.0 plan

The extensions repository is the first outside user of the extension API. Chunk 8 (the author kit, `docs/plans/extensions.md` 7.8) was
meant to show that an author outside the server can write and install an extension; the split does most of that by construction, and
what it finds wrong with the API (a missing hook, a manifest key, an error message) is fixed here before the API is frozen.
