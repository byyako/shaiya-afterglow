# Extensions chunk 1, the foundation (spec and build plan)

**Built (part A 2026-11-02 and 2026-11-03, part B 2026-11-03).** Ready to build as reconciled 2026-11-02: Specified on 2026-10-22, before the install work; reconciled on 2026-11-02 with what 0.2.0
changed (`docs/plans/extensions.md` section 2a: the re-runnable `tools/setup.sh`, `tools/defs/read_bak.py`, the renamed containers,
database and variables, the released binaries, no alpha) and checked against the Lua scripts chunk 5 must serve (section 5.8). The
maintainer's answers of that day are section 1a. Breaking changes are accepted in 0.x; section 11 is the changelog's migration note.
Built as specified; what the build settled where the text left a choice: `game-server config` runs no migrations and tolerates a database
without the `content` schema (the server's start runs them on the one connection); the editor's `setting` target stores a value as the
text the file would give (`12`, `35,36`) and a `set` on a setting without an override adds one; the key of that target is a name, so it
has its own code (`crates/data-editor/src/settings.rs`) beside the numeric key code of the other targets.

Chunk 1 of `docs/plans/extensions.md` (section 6). It makes the database the source of truth for the game's content, gives custom
content a package (an **extension**: a folder with a manifest), and gives the game server's settings layers. The Kill Boost Stone,
today two hand-written SQL files (`tools/defs/additions.sh --with-additions`), becomes the first extension and the proof. Nothing here
changes how the game plays: on the dev box every table holds the same bytes before and after, and patch 109 regenerates byte for byte.

Background: `docs/plans/extensions.md` (decisions 3 to 5, section 5 layer 1, section 2a), `docs/plans/done/client-patches.md` (the client
tables and the generator, patch 109), `docs/plans/done/data-editing.md` (the `edits` overlay, its markers, `reload`),
`docs/client/sdata-generation.md` (the client files' layouts), `docs/client/client-patch-0109.md` (patch 109 packed, signed and installed
by the real updater on a spare client), `docs/plans/install.md` (the 0.2.0 install). Client research entry 11 (the client's content
limits) is **not answered yet**: the id ceilings below are the ones the files already prove, and are replaced when it is (section 5.6).

## 1. The maintainer's decisions (2026-10-22)

1. **Import once.** The original data is imported once. A second run of an importer with **the same files does nothing**; with
   **other files it is refused** (amended 2026-11-02, decision 10); `--reset` is the documented way to bring in corrected original files:
   it re-reads them into the frozen copy, then rebuilds the live tables from the originals, the installed extensions and the data
   editor's edits, in that order.
2. **The config file is TOML** (`config/game.toml`); the extension manifest and its rows use TOML too.
3. **Removing an extension whose items players hold is refused**, listing who holds what (character, place, count). `--force`
   deletes those items, buffs and skills after a confirmation, logged. (Other data pointing at its rows also refuses: decision 16.)
4. **Disabling an extension turns its behaviour off and leaves its rows**: its settings defaults stop counting (later chunks: its
   rules, hooks, scripts and options too); its items and skills stay, so held items keep working. Only remove takes rows out.
5. **Environment variables are for start-up only**: `DATABASE_URL`, `GAME_BIND`, `GAME_SERVER_ID`, `GAME_CONFIG` (the file's path).
   Every game setting lives in the file (and the layers around it). A game setting still set in the environment **stops the server at
   start** with a message naming the file key, so nothing is silently ignored. The dev box's seven `GAME_*` game settings (its `.env`:
   `GAME_GUILD_PARTY_SIZE=2`, the gambling house's three, the two map lists, the kill boosts) move into the file.
6. **Extension rows are TOML data files** (copy a row, set columns, the client fields with a look-alike); SQL only for an extension's
   own tables in its own schema.
7. **Settings take effect at restart.** The data editor marks a setting change "restart". The console's live switches (exp rate,
   kill rate, shop, max users) keep working as now; chunk 2's rules get the live `reload`.
8. **The Kill Boost Stone ships as an extension folder, not installed by default** in a release; installed on the dev box. Without
   it `kill_boost_skills` is empty, which is already the release default since 0.1.0 (`docs/server/deviations.md` section 1): skill 200
   levels 1 to 3 are empty in the originals.
9. The maintainer asked how client-side changes (names, icons, models, art) are **undone** when an extension goes: section 4.3.

## 1a. The maintainer's decisions of 2026-11-02 (the reconcile)

Section 2 and 2a of `docs/plans/extensions.md` stand (the order, 0.3.0 with Lua, breaking changes accepted, the Lua game API only); these
settle what 0.2.0 changed under this spec.

10. **A re-run of `tools/setup.sh` skips what is imported from the same files.** Each importer compares the SHA-256 of the files it
    reads with its import record: the same, it says "already imported, nothing to do" and setup goes on (to the extensions, the
    account and the start); different, it stops and names `tools/setup.sh --reset`. So re-running after a failure stays safe, as in
    0.2.0, and decision 1 holds (section 3.2).
11. **A 0.2.0 server upgrades by running `tools/setup.sh` again** with the same two folders: it finds no import record, imports the
    frozen copies and rebuilds the live tables from them plus the edits (changes made by hand in SQL are replaced, as on any 0.2.0
    re-run); then `data-editor ext install extensions/kill-boost` for whoever used `--with-additions`, and the `GAME_*` settings move to
    `config/game.toml`. Until the re-run, the game server runs as before and `ext install` refuses with that hint. `--adopt` stays a
    flag for the dev box (and later the conversion tool), not documented for operators (section 11).
12. **The dev box moves to the clone, the build is rehearsed on a copy, then done on the live database** (section 9). The dev servers
    still run from the old private tree (`/media/drive0/projects/NewShaiya`, database `newshaiya` in `newshaiya-postgres`); the clone's
    migration 0031 must never reach that database while a binary of the old tree uses it.
13. **Extension-owned options are specified and reserved** (section 7.5): the manifest's `[options]`, the file's `[ext.<name>]`
    tables and the editor's `ext.<name>.<key>` keys are written down and refused with "reserved for chunk 5" until chunk 5 builds them
    with their first consumer.
14. **`tools/setup.sh --extension <folder>`** (repeatable) installs an extension after the import, skipping it when that version is
    installed; without it, "Left to do" lists the bundled extensions with the install line.
15. **`tools/setup.sh` writes `config/game.toml` when it is missing**: a short header, every line a comment (section 7.2). It never
    overwrites the file.
16. **Remove refuses while other data points at an extension's rows**, and lists each reference (table, key, column, or the change set
    of an edit), as it lists holders; `--force` does not clear references: the operator undoes the edit or changes the row (section 5.7).

Decided here (not behaviour, so not asked):
- The origin of a row is kept in a side table, not a column (section 3.3); the frozen copy of the originals lives in the database
  (section 3.1); the extension tool is part of the `data-editor` binary (an operator has no Python beyond `tools/setup.sh`,
  `docs/plans/done/data-editing.md` decision 6); in chunk 1 an extension may add to or change items, player skills and drop entries only
  (section 5.5).
- **`ext install` needs no `client` schema.** Operators never import the client's files (`tools/setup.sh` does not run
  `load_client.py`; it is a dev tool). A client row keeps its look-alike as a reference and `client_patch.py` resolves it when it
  generates (section 4.1); the item type-id ceilings are a built-in table of the US patch 108 client's group sizes (section 5.6).
- **The game server reads the database layers over one connection before it builds its pool** (the pool's size is a setting,
  section 7.3).
- **The environment refusal prints the lines to paste** into `config/game.toml` (section 7.2), the migration aid for 0.2.0 operators.
- **`ext install --skip-installed`** for setup's re-runs (section 5.7); **`content.imports.rebuilt_at`** so a re-run finishes a rebuild
  that an earlier run did not (section 3.2).
- **The release tarball carries `extensions/` and `config/game.toml.example`** beside the binaries (section 5.1).
- **The deploy guide's game-data move dumps the content schemas too** (`original_defs`, `original_world`, `content`, `edits`), since
  a VPS restored from `defs` and `world` alone could not rebuild or install (section 11).
- The patch ledger's commit column is `commit` (the manifest key `newshaiya_commit` of patch 109 is read by `adopt`, written no more).

## 2. What goes away, what stays

| Today (0.2.0) | After chunk 1 |
|---|---|
| `tools/defs/load_into_postgres.py` replaces `defs` on every run | loads the frozen copy `original_defs`; again with the same files: nothing to do; with other files: refused without `--reset` (3.2) |
| `tools/world/import.py` replaces `world` on every run, then `data-editor apply` | the same for `original_world`; ends with `data-editor content rebuild world` |
| `tools/sdata/load_client.py` replaces schema `client` (and with it the client rows) | imports `client.items` / `client.skills` once (a dev tool); the client rows no longer live in `client` |
| `tools/defs/read_bak.py`, `tools/defs/export_from_mssql.sh` | unchanged: they write `data/defs-export/`, the importer's input |
| `tools/setup.sh` replaces `defs` and `world` on every run, then `additions.sh` | runs the importers (a re-run skips what is imported), `--reset`, `--extension <folder>`, writes `config/game.toml` when missing (3.2) |
| `tools/defs/additions.sql`, `additions.sh` (`--with-additions`) | **deleted**: the Kill Boost Stone's server rows are `extensions/kill-boost/rows.toml`, installed by `data-editor ext install` or `setup.sh --extension` |
| `tools/sdata/client_rows.sql`; tables `client.item_rows`, `client.skill_rows`, `client.server_only` | **deleted**: the client fields are in the same `rows.toml`, installed into `content.client_items` / `content.client_skills` / `content.server_only` |
| `tools/defs/verify.py` compares live `defs` with SQL Server (`--counts-only` without it, in setup) | compares the frozen `original_defs` (`--schema`, default `original_defs`); the live tables are checked by `data-editor content check` |
| `client_patch.py` reads `client_rows.sql`, the shipped files in `reference/`, `data/defs-export/*.psv`, earlier patch folders | reads the database only (section 4) |
| `GAME_*` environment variables (`.env`, the deploy's env file) | start-up ones stay; the rest are keys of `config/game.toml` (section 7) |
| the release tarball: seven binaries | the seven binaries, `extensions/`, `config/game.toml.example` (5.1) |
| the `edits` overlay, its markers, `data-editor apply`, the console's `reload` | unchanged; they sit on top (section 8) |

## 3. The database as the source of truth

### 3.1 Schemas

| Schema | Holds | Written by |
|---|---|---|
| `original_defs`, `original_world` | the original data exactly as imported (same DDL as `defs` / `world`) | the importers only, once (or `--reset`) |
| `client` | `client.items`, `client.skills`: the client's shipped `Item.SData` / `Skill.SData`, record by record; only where `load_client.py` ran (a developer's box) | `load_client.py` only, once (or `--reset`) |
| `defs`, `world` | the **live** tables the servers read: originals + extensions + edits | `data-editor content rebuild`, `data-editor ext …`, the data editor's Apply/Undo/apply |
| `content` (new, migration 0031) | imports, installed extensions, the rows each owns, their id ranges, settings defaults, client rows, the patch ledger, the editor's settings layer | `data-editor` (and `client_patch.py` for the ledger) |
| `edits` | the data editor's overlay (unchanged) | `data-editor` |
| `ext_<name>` | one per installed extension that has migrations: its own tables | its migrations |

The frozen copies are what "original" means everywhere: what `remove` restores, what `content check` compares against, what
`client_patch.py check` 4 compares against (instead of `data/defs-export/*.psv`). Their size is about that of `defs` + `world` (the whole
dev database is 65 MB today). `content.imports` keeps each frozen table's row count and an md5 of its rows, and `content check` fails when
they no longer match (nobody writes them, and this proves it). Nothing an operator runs needs `client`: only `client_patch.py` (a dev
tool) and, when it exists, the install's text-length check (5.3).

### 3.2 Import, re-run, reset

```
first import:      load_into_postgres.py → original_defs  (records content.imports 'defs')  → data-editor content rebuild defs
                   import.py             → original_world (records 'world')                → data-editor content rebuild world
                   load_client.py        → client.items, client.skills (records 'client'; a developer's box only)
again, same files: "defs: already imported on <date> from the same files; nothing to do" (exit 0, nothing written; but a rebuild that
                   did not finish, rebuilt_at NULL or older than imported_at, is run now: "the rebuild after the import did not finish: running it")
again, other files: refused, exit 1: "defs was imported on <date> from <source>; these files differ: items.psv, skills.psv. tools/setup.sh
                   --reset (or this importer with --reset) re-imports and rebuilds (docs/plans/ext-1-foundation.md 3.2)"
--reset:           the importer replaces the frozen copy and its content.imports row, then runs `data-editor content rebuild <part>`
                   (with the same files too: a rebuild through the importer)
--adopt:           (the dev box once, section 9; later the conversion tool) writes the frozen copy and its record and keeps the live
                   tables; refused when a record exists
0.2.0 database:    a `defs` schema without a content.imports row is a first import (decision 11), with the line "defs exists without an
                   import record (a database from before 0.3.0): importing the originals and rebuilding defs from them; changes made by
                   hand in SQL are replaced, the data editor's edits are written again"
```

**The same files**: the SHA-256 of every file the importer reads, kept in `content.imports.files`. For `defs` that is the eight `.psv`
files (the direct reader and the SQL Server export write identical files, `docs/plans/install.md`), so a re-run reads the backup again
(about 40 seconds) and compares what it wrote; for `world` every file of the Data folder it reads (Map.ini, the svmaps, NpcQuest.SData,
the `.ini` files, the AI files, kr_string.DB, the Lua). `source` is a text the caller passes with `--source` (setup: `PS_GameDefs backup
<sha256>, read directly (tools/defs/read_bak.py)`, `… through SQL Server`, or `export folder <path>`); without it, the folder's path.

`data-editor content rebuild [defs|world|all]` (one transaction, the `edits.change_sets` lock as Apply takes it):
1. Refuses a part that has no `content.imports` row.
2. Creates the live schema from the DDL when it is missing (`db/defs/schema.sql`, `db/world/schema.sql`, built into the binary with
   `include_str!`, so a released `data-editor` needs no checkout); otherwise empties every table of the part in one `TRUNCATE`.
3. `INSERT INTO defs.t SELECT * FROM original_defs.t` per table (identical column order: both come from the same DDL file); identity
   sequences set past the highest id, as `import.py` does today.
4. Drops the part's `edits_applied` marker.
5. Writes the rows of every installed extension (enabled or not: decision 4), in `install_seq` order, from `content.ext_rows.row`.
6. Runs the edits' apply for the part (`apply.rs`, the fresh path: originals refreshed from the tables, problems reported).
7. Sets `content.imports.rebuilt_at`, prints counts and the problems; exits 1 on an error (nothing written), 0 otherwise (problems are
   reported, not fatal, as `apply`).

`--into <prefix>` builds into scratch schemas `<prefix>_defs` / `<prefix>_world` inside a transaction that is rolled back after the
caller has compared (used by `content check`, and by the tests).

**`tools/setup.sh`** (decisions 10, 11, 14, 15) becomes:

```
tools/setup.sh [--sqlserver] [--no-account] [--reset] [--extension <folder>]... <PSM_Data folder | export folder> <PS_Game/Data folder>
```

1. The prerequisites, `.env`, PostgreSQL, the binaries and `servers list` as in 0.2.0.
2. The definitions read into `data/defs-export/` as in 0.2.0 (direct reader, `--sqlserver`, or the export folder given).
3. `load_into_postgres.py data/defs-export db/defs/schema.sql --source "…" [--reset]`: imports, skips or refuses (above). Then
   `verify.py --counts-only` (or the full check with `--sqlserver`) only when it imported.
4. `import.py <PS_Game/Data> --source "…" [--reset]`: the same for `world`.
5. Each `--extension <folder>`: `target/release/data-editor ext install --skip-installed <folder>`.
6. `config/game.toml` written when missing (7.2).
7. The account, server entry and start as in 0.2.0; "Left to do" adds one line per folder of `extensions/` (`data-editor ext install
   extensions/kill-boost   # Kill Boost Stones: three stones whose buff multiplies player kills`, the manifest's title and description).

Its header says what a re-run does (skips what is imported from the same files, installs only the extensions not yet installed, keeps
`config/game.toml`). A refusal at step 3 or 4 ends setup with its message and the `--reset` hint; `--reset` with a server running is
allowed (the importers do not check; the hint says "stop the game server first").

### 3.3 The origin of a row: a side table, not a column

Every row of `defs` / `world` is **original** unless an extension owns it (`content.ext_rows`) or an edit changed it (`edits.fields`,
`edits.rows`). `data-editor content origin <table> <key>` prints the layers of one row: the original values, the extension that owns it
(and what it set), the edits in force (and their change sets).

Why not an `origin` column on every table, as design section 5 sketched:
- **A row can have several origins.** The Kill Boost Stone does not add rows: items 100/212 to 214 and skill 200 levels 1 to 3 exist
  in the originals as empty placeholders (`???177`, `??`), and the extension changes them. Then an edit may change one field of them
  again. One column cannot say "original, changed by kill-boost, grade edited in change set 12"; the layers can.
- **Restoring needs the original values anyway**: they are in the frozen copy, keyed like the live row. The side table only needs to
  say who owns which key.
- **No table changes**: the 8 `defs` tables (generated DDL, compared with SQL Server by `verify.py`), the 28 `world` tables, about 50
  loaders in the game server, the editor's whole-row snapshots (`to_jsonb(t)`) and its export keep their shape.
- **One owner per row is a primary key** (`content.ext_rows (tbl, key)`), so two extensions can never claim the same row.
- It is tens of rows, not a column on 150,000.

### 3.4 `verify.py`

Unchanged in what it compares; it takes `--schema` (default `original_defs`), and `--counts-only` (0.2.0) counts `original_defs` too.
The importer prints the command to run it after an import or reset; setup runs it as in 0.2.0. Nothing compares live `defs` with SQL
Server any more: the live tables differ from it on purpose, and `content check` accounts for every difference.

### 3.5 `data-editor content check`, `status` and `hashes`

`content check`: (1) the frozen copies match their `content.imports` md5s; (2) `rebuild --into` a scratch prefix and compare every
table with the live one (`EXCEPT` both ways): every difference is printed as `defs.items 100212: hand edit? itemname live 'X', expected
'Y'`; (3) every `content.ext_rows` key still lies in its extension's ranges; (4) no two enabled extensions set the same setting; (5) the
references of section 5.7 into rows of removed extensions (none should exist; a hand edit could make one). Exit 1 on any finding.

`content status`: the imports (when, source, counts, rebuilt), the extensions (name, version, enabled, rows owned, client rows,
settings), and whether a client patch is due (client rows with `since_patch` NULL, or a removed extension's rows that a shipped patch
carried). Without imports it says so and names `tools/setup.sh` (decision 11).

`content hashes [schema…]` prints `schema.table rows md5` for every table of the schemas (default `defs world client`, the ones that
exist): the baseline and the proof of section 9.

## 4. Client-only fields, owned the same way

### 4.1 Where they live

`content.client_items` and `content.client_skills` replace `client.item_rows` / `client.skill_rows`, and `content.server_only` replaces
`client.server_only`, each with `extension` (the owner). A row is written by `ext install`/`upgrade` from the `client` (or
`server_only`) entry of a row in the extension's `rows.toml`, removed by `ext remove`. The generator's rule is unchanged: a record is the
client's original unless it has a client row; then it is the server's fields from `defs` plus the row's client-only fields.

**What a row holds**: the name and description (or `keep`), the look-alike as a reference (`look_type`/`look_typeid` for items,
`look_skill`/`look_level` for skills; NULL for none), and the client-only fields given by name (`model`, `icon`, `type2`, `type3`;
`icon`, `animation`, `toggle_type`, `sound`), NULL when not given. `client_patch.py` resolves each field: given by name, else the
look-alike's record, else the record's own original (`client.items` / `client.skills`). So an install needs no `client` schema
(section 1a), and the resolved values equal what `client_rows.sql` wrote (the A6 comparison resolves them in SQL).

`since_patch` is no longer written by hand: it is the patch that first shipped the row, stamped by `client_patch.py generate` on the
rows that have none (an extension cannot know an operator's patch numbers). For the Kill Boost Stone it is 109 (`adopt`, section 4.2).

### 4.2 `client_patch.py` reads the database only

- **Check 1 (originals)**: the shipped files' SHA-256 come from `content.imports` `'client'` (`files`), recorded by `load_client.py`,
  not from `reference/`. The counts as now.
- **Check 2 (restore)**: unchanged; `client.items` / `client.skills` rebuild both originals byte for byte. The originals the other
  checks compare with are these restored bytes (check 2 proves them), not the files.
- **Check 3 (only the listed records change)**: unchanged, with the rows from `content.*` (resolved as 4.1).
- **Check 4 (nothing forgotten)**: every `defs.items` / `defs.skills` row that differs from `original_defs` (by key; `updatedate`
  never counts) has a client row or a `server_only` reason, or differs in `grade` alone with an `item_grade` edit (as now). A SQL
  comparison of the two schemas replaces `defs_export_rows`.
- **Checks 5 to 8**: unchanged.
- **The ledger** (new): `content.client_patches` (one per patch) and `content.client_patch_files` (one per member, with the
  encrypted bytes). `generate` writes them in the same transaction that stamps `since_patch`, and still writes the hand-over folder
  `data/client-patches/psNNNN/` (manifest, `changes.txt`, files) for the client side. "The previous version" of a member comes from the
  ledger (the newest earlier patch that carried it), else from the restored originals (108). The manifest's commit key is `commit` (was
  `newshaiya_commit`) and it gains `"extensions": {"kill-boost": "1.0.0"}` (the installed extensions and versions; the content's
  provenance).
- `generate` refuses uncommitted changes in `tools/sdata/`, `db/client/` and `extensions/` (instead of `additions.sql`), `--dirty` as
  now.

New commands:
- `adopt --patch N`: once per patch made before the ledger existed (109 only): reads `data/client-patches/psNNNN/`, checks every
  file's SHA-256 against its manifest, writes the ledger rows (`commit` from the manifest's `newshaiya_commit`: a commit of the old
  private tree, kept as written), and stamps `since_patch = N` on the client rows listed in its `changed` whose `since_patch` is NULL.
  Refuses a patch already in the ledger.
- `rebuild --patch N --out DIR`: generates the files from the database with only the client rows whose `since_patch` ≤ N, writes them
  to DIR, compares each with the ledger's `sha256` for that patch and member, prints `MATCH` or the records that differ; exit 1 on a
  difference. This is **the proof**: patch 109 regenerates byte for byte from the database alone.
- `record --patch N --patch-sha256 … --sig-sha256 … --status …`: the client side's results.
- `client_patch.py restore` and `check` keep their meaning.

The client side's results (patch file and signature SHA-256, status) go into `content.client_patches`: `adopt` fills them for 109 from
`docs/client/client-patch-0109.md` (`ps0109.patch` `9478d66a…7bde5`, `.sig` `9d27d3f1…f9533`, status `staged`: updater live, in-game
pending); later patches by `record`.

### 4.3 Undoing client changes (the maintainer's question)

A client patch always carries **whole files holding every change so far** (`docs/plans/done/client-patches.md` section 4). So undoing is the
next patch, never an edit of the client:
- **Data records** (names, descriptions, icons, models of items and skills): after `ext remove` the client rows are gone, so the next
  generated `Item.SData` / `Skill.SData` holds the original records again. `changes.txt` lists them as reverted; check 3 accepts a
  record without a client row only when it equals the original. `ext remove` and `content status` say "a client patch is due"
  whenever a removed row had `since_patch` set.
- **Disable** changes nothing in the client (decision 4: the rows stay).
- **Art** (models, textures, icons as files, sounds): an extension lists its art in the manifest (section 5.2). In chunk 1 the
  server side records it (`content.ext_client`, with each file's SHA-256) and `ext check` validates the paths; it is **not put into a
  patch yet**. The patch format can add, replace and remove archive members (the patch tool lists "archive files, loose files,
  removals"; the updater logged "0 added, 2 replaced, 0 removed" for 109, `docs/client/client-patch-0109.md`), so undoing art is: a
  member the extension **added** is removed by the next patch; a member it **replaced** gets the original back (the client side holds
  every original member in its archive). Which file kinds the updater accepts and what restrictions apply is entry 11 (d); shipping
  art is specified once that is answered (section 10).
- A **removed** item's held copies are gone first (decision 3), and nothing else points at it (decision 16), so no player keeps an item
  whose client record changes under it.

## 5. Extensions

### 5.1 The folder, and the release

```
extensions/<name>/
  extension.toml        the manifest (required)
  rows.toml             rows it adds to or changes in defs (optional; or rows/*.toml, read in file-name order)
  migrations/NNNN_<what>.sql   its own tables, in schema ext_<name> (optional)
  art/                  files the manifest's [client] art names (optional)
  README.md             for people (optional; not read)
```

`extensions/` in this repository holds the bundled extensions (the Kill Boost Stone first). An operator keeps theirs anywhere and
passes the folder. The database keeps everything the install read (the manifest as JSON, every computed row, the settings, the client
rows, the art's hashes), so a server runs and an extension can be removed without its folder.

**The release** (`.github/workflows/release.yml`): the tarball `shaiya-afterglow-vX-linux-x86_64/` holds the seven binaries at its top as
now, plus `extensions/` (the whole folder) and `config/game.toml.example`, so a machine given only the tarball (the deploy guide's "copy
them" path) can install the bundled extensions and see every setting. `tools/fetch-release.sh` installs only the top-level files into
`target/release/` (a checkout has the two folders already; its `install … /*` would fail on a folder) and says where the tarball's
folders are when asked (`--keep <dir>` unpacks the whole tarball there). An older `fetch-release.sh` (v0.2.0's) cannot unpack a 0.3.0
tarball; setup always runs the script of its own checkout, which matches.

### 5.2 The manifest (`extension.toml`)

```toml
format = 1                       # the manifest format; this spec is 1. A higher one is refused ("needs a newer data-editor").
name = "kill-boost"              # ^[a-z][a-z0-9-]{1,39}$; its schema is ext_kill_boost (dashes become underscores)
version = "1.0.0"                # semver; an upgrade must raise it
title = "Kill Boost Stones"
description = "Three stones whose buff multiplies player kills (2x, 5x, 10x) for one hour."
authors = ["Shaiya Afterglow"]   # optional
license = "MIT"                  # optional

[ids]                            # what it owns; checked against the client's ceilings and other extensions (5.6)
items = ["100/212-214"]          # type/typeid, a range of type ids, or one: "101/5"
skills = ["200/1-3"]             # skill id/level, a range of levels, or every level: "201"

[settings]                       # defaults for game settings (section 7); keys are the settings' names
kill_boost_skills = "200/1:2,200/2:5,200/3:10"

[client]
features = []                    # fixes.dll feature names it needs: recorded and listed; checked once the client can say which it has (entry 11 c)
art = []                         # [{ member = "Item/Icon/x.dds", file = "art/x.dds" }]: recorded with SHA-256, not shipped yet (4.3)
```

Reserved keys, refused with "`<key>` is reserved for a later chunk (docs/plans/ext-1-foundation.md 5.2)": `options` (extension-owned
options, section 7.5, chunk 5), `rules` (chunk 2), `scripts` and `api` (chunk 5). Any other key is an error naming it ("unknown key").
A missing required key names the key and the file.

### 5.3 The rows (`rows.toml`)

Three kinds of entry in chunk 1, each a TOML array of tables:

| entry | table | `id` | may | notes |
|---|---|---|---|---|
| `[[item]]` | `defs.items` | `"type/typeid"` (itemid = type × 1000 + typeid) | change an original row; add a row whose place exists in `Item.SData` | must lie in `[ids] items` |
| `[[skill]]` | `defs.skills` | `"skillid/level"`, level 1 to 3 | change an original row; add a missing level 1 to 3 of a skill 1 to 326 | must lie in `[ids] skills` |
| `[[drop]]` | `defs.mob_items` | `"mobid/itemorder"`, order 1 to 9 | change `grade` and `droprate` only | the data editor's `mob_drop` rules; no range needed: ownership is per row |

Fields of an entry:
- `copy = { from = "<id of an original row of the same table>", columns = [ … ] }` (optional): start from those columns of that row
  **in the frozen originals** (never from the live table: an extension's rows depend only on its files and the originals, so a rebuild
  computes the same rows whatever edits or other extensions did). `columns` left out = every column but the key columns, `rowid` and
  `updatedate`.
- `set = { column = value, … }`: `defs` column names (lowercase, as `db/defs/schema.sql`). The key columns and `rowid` are refused.
- The row is: the original row of `id` if there is one (a **change**), else the `copy` row with `id`'s key and the next free `rowid`
  (an **add**; `copy` is then required), then `copy`'s columns, then `set`.
- `client = { name = "…", description = "…", look = "<client id>", … }` (items and skills): the client-only fields. `look` names the
  client record whose model, icon, type2, type3 (items) or icon, animation, toggle_type, sound (skills) are taken (`type/typeid`,
  `skill/level`); any of those fields given by name wins over `look`. `name` and `description` are required. `client = "keep"` takes
  every client-only field, text included, from the record's own original client record (shipping the server's values under the
  client's own look and text: a "listed choice" of `docs/plans/done/client-patches.md` section 2).
- `server_only = "reason"` instead of `client`, for an item or skill change the client needs nothing of.
- An item or skill entry whose row differs from the original in any column needs `client` or `server_only` (the same rule as check 4,
  enforced at install instead of at patch time). When the `client` schema exists, `install` warns when `client = "keep"` or a
  look-alike would also ship server values that differ from the client's own record (it names the fields:
  `docs/client/item-sdata-vs-defs.md` lists 7,729 name differences and some prices).
- `note = "…"` (optional): the client row's note; default `"<extension> <id>"`.

Validation names the file and the entry (`rows.toml [[item]] #2 (100/213): column "rnage" is not a column of defs.items`), checks each
value casts to its column's type (inside the install's transaction), that `defs` text encodes as GBK (the frozen copy's encoding), and
that client text is printable ASCII; when the `client` schema exists, also that the text is no longer than the longest of its kind
(check 7) and that the look-alike record exists (without it, that the look-alike lies inside the ceilings of 5.6). Client field widths
(check 5) are checked by `client_patch.py check`, which `ext install` tells the operator to run when the extension has client rows.

### 5.4 Migrations

`migrations/NNNN_<what>.sql`, `NNNN` from 0001, run in order with `SET LOCAL search_path TO ext_<name>` in the install's transaction,
each recorded in `content.ext_migrations` with its SHA-256. They may only create and fill tables of `ext_<name>` (a foreign key to
`public.characters` with `ON DELETE CASCADE` is allowed: per-character data of an extension goes with the character, section 5.8):
when an extension has migrations, `install`/`upgrade` takes `content hashes defs world client` before and after them and refuses
(rolling everything back) if anything changed. An applied migration whose file changed is refused at upgrade (as sqlx does). `remove`
drops the schema (`DROP SCHEMA ext_<name> CASCADE`), so an extension's own data goes with it; the Kill Boost Stone has no migrations.

### 5.5 What an extension may touch in chunk 1, and why only that

`defs.items`, `defs.skills` (levels 1 to 3) and `defs.mob_items` (grade and chance). Everything else is refused with its reason:
monsters (`defs.mobs`: the client's `Monster.SData` is not generated yet; chunk 4 adds it), monster skills (level 100: `NpcSkill.SData`),
NPCs, shops, gates and quests (`NpcQuest.SData`), the item mall (`Cash.SData`), spawns, portals, obelisks and bosses (the data editor
owns these for now; adding them to extensions waits for a chunk that needs them). The list of extendable tables is one table in the code
(`crates/data-editor/src/ext/tables.rs`: table, key columns, key format, id kind, client file, allowed actions, allowed columns), so a
later chunk adds a line, not a mechanism.

### 5.6 The id-range registry and the client's ceilings

An extension declares what it owns in `[ids]`; stored expanded in `content.ext_ranges (extension, kind, major, minor_low,
minor_high)` (items: major = type, minor = typeid; skills: major = skill id, minor = level). At install and upgrade:
- every range lies inside the ceilings below;
- no range overlaps a range of another installed extension (enabled or not);
- every `[[item]]` / `[[skill]]` entry lies inside the extension's own ranges.

The ceilings (`crates/data-editor/src/ext/ids.rs`, each with its source; entry 11 (a) will replace or extend them):

| kind | ceiling | source, status |
|---|---|---|
| item type | 1 to 100 | `Item.SData` has 100 groups (`docs/client/sdata-generation.md` 1; verified); 74 of them hold records. Whether the client takes more: entry 11 (a) |
| item type id | 1 to the group's record count in the US patch 108 `Item.SData` (4 to 255 in the groups that have records; 0 in the others, so nothing is placed there) | the file's dense arrays; growing a group is refused (`docs/plans/done/client-patches.md` check 6; verified). The 100 counts are a constant table in `ids.rs`, so no `client` schema is needed; a test compares it with `client.items` when that schema exists |
| item `range` (the skill an item gives) | 0 to 255 | one byte in the client (item fixed `+32`, `docs/client/research-gambling-kill-boost.md` "what adding dedicated content takes"; verified) |
| skill id | 1 to 326 | `Skill.SData` holds 326 skills; growing refused (check 6). The loader takes any count (entry 7, inferred): entry 11 (a) |
| skill level | 1 to 3 | players' levels; 100 is the monsters' (`NpcSkill.SData`, not generated) |
| monster, NPC, quest, map, mall product | not extendable in chunk 1 | section 5.5 |

### 5.7 The commands (`data-editor ext …`)

```
data-editor ext list                       installed extensions: name, version, enabled, rows, client rows, settings
data-editor ext show <name>                its manifest, ranges, rows (with what each changes against the original), client rows, settings
data-editor ext check <folder>             everything install checks, writing nothing; prints what it would change, row by row
data-editor ext install [--skip-installed] <folder>
data-editor ext upgrade <folder>
data-editor ext remove <name> [--force [--yes]]
data-editor ext enable <name>
data-editor ext disable <name>
```

**install** (one transaction; `LOCK TABLE edits.change_sets IN EXCLUSIVE MODE` first, as Apply):
1. Refuses unless `defs` is imported (`content.imports`; else "defs has no import record: run tools/setup.sh again with the original
   files first (a database from before 0.3.0: docs/plans/ext-1-foundation.md decision 11)"), the name is not installed (with
   `--skip-installed` the same name and version installed is "already installed", exit 0; another version is refused with the
   `ext upgrade` hint), the manifest and rows parse and validate (5.2, 5.3), the ranges pass 5.6, no row key is owned by another
   extension (`content.ext_rows` primary key), every setting exists and its value passes the game server's own parser
   (`game_server::config::check_setting`), and no enabled extension sets the same setting.
2. Inserts `content.extensions` (`install_seq` gives the order), `content.ext_ranges`, `content.ext_settings`, `content.ext_client`.
3. Runs the migrations (5.4).
4. Computes each row (5.3), writes it into the live table (change: `UPDATE … FROM jsonb_populate_record`; add: `INSERT`), and records
   it in `content.ext_rows` (`row` = the whole row as written, `to_jsonb`), then the client rows into `content.client_items` /
   `client_skills` / `server_only` with `since_patch` NULL.
5. Drops the `defs` marker and runs the edits' apply for `defs` (an edit on a row the extension changed now sits on top of it; a
   changed original under an edit is reported as after a load, with the wording "the content under your edit changed").
6. Logs to `content.ext_log`, commits, and prints what to do next: "restart the game servers" when it wrote items or skills, "`console
   <id> reload`" when it wrote drop entries only; "run `client_patch.py check`, then generate a patch" when it has client rows (with "the
   players' clients show these items as ??? until they have a patch that carries them"); the `fixes.dll` features it needs, if any.

**upgrade**: same name, a strictly higher version (semver), the same checks against everything but its own install; new migrations
only; rows compared by key with the installed ones: a key no longer listed is restored (as remove does, the holder and reference checks
included), a changed one rewritten, a new one claimed; client rows replaced, keeping `since_patch` where neither the client fields nor
the server row changed (else NULL); settings, ranges and client features replaced; then step 5 and 6 as install. A lower or equal
version is refused.

**remove**:
1. The **holder check** for every item it owns (`content.ext_rows` of `defs.items`, change and add alike: a changed placeholder
   turns back into `???177` under the player) and every skill level: `character_items`, `bank_items`, `guild_bank_items`,
   `market_auctions`, `market_result_items`, `point_items`, `billing_items` (item id), `character_skills`, `character_applied_skills`
   (skill id and level), `character_quick_slots` entries that point at them (read its columns and the code that fills it first).
   Found: refused, listing `character <name> (<id>): 3 × 100/212 in the bag`, … and the hint `--force`.
2. The **reference check** (decision 16), rows that are not its own:
   - items: `defs.product_list` (every `itemidN` column of a mall product), `defs.base_items_defs.itemid` (starting items),
     `defs.mobs.questitemid`, `world.npc_shop_items`, `world.quests` (the start item, `start_items`, `end_items` triples),
     `world.quest_results` (its item), `world.item_quest_links`;
   - skills: `defs.items` rows that give an owned skill (`range` = the skill id and `attacktime` = its level, the rule of
     `item_use.rs` (`Effect::Skill`); an item kind 50 names an exp wheel, not a skill);
   - edits in force on an owned row: `edits.fields` / `edits.rows` whose target is a table it owns rows of (`item_grade` on
     `defs.items`, `mob_drop` on `defs.mob_items`) and whose key is one of them, with the change set;
   - found: refused, listing `defs.product_list 2041 itemid3 = 100212`, `edit item_grade 100212 grade (change set #12)`, … with the hint
     "change those rows or undo the edits first; --force does not clear references". Read the item columns of `db/defs/schema.sql` and
     `db/world/schema.sql` when building, and add any table this list misses (a test lists the columns it checks).
3. `--force` (holders only): refused while any game server reports players (`game_servers.players > 0` with a `last_seen` within twice
   the heartbeat: an online player's memory would write the item back); else asks `type the extension's name to confirm` (or `--yes`),
   deletes the held rows, and logs each one in `content.ext_log`.
4. In one transaction: each owned row restored (change: the frozen original row; add: deleted), its `content.*` rows deleted,
   `DROP SCHEMA ext_<name> CASCADE`, the `defs` marker dropped and the edits' apply run; log; commit. Prints "restart the game servers"
   and "a client patch is due" when any of its client rows had been shipped.

**enable / disable**: flips `content.extensions.enabled`, logged. Enable refuses when one of its settings is set by another enabled
extension. Prints "restart the game servers" (decision 7). Rows are untouched (decision 4).

### 5.8 Room for the later chunks (checked against the first Lua scripts)

`docs/plans/extensions.md` section 2a decision 6 names the scripts the Lua API must serve. What each needs from this chunk, and where
chunk 1 leaves the room:

| script | needs from the foundation | chunk 1 |
|---|---|---|
| welcome notice, advert banner, custom chat commands | its own texts and intervals; the extension on or off; no rows | options reserved (7.5), `scripts` and `api` reserved (5.2); disable already means "behaviour off" (decision 4) |
| boss and kill rewards (items or buffs to a party or raid) | reward items and buffs as rows with client rows; held items protected at remove | `[[item]]` / `[[skill]]` rows, client rows, the holder and reference checks; the boss groups are read from `world.boss_groups` as the core reads them |
| event spawns ("monster madness": spawn around a player, caps, clean-up, started by a command or an item) | original monsters spawned at run time; caps as options; an item that starts it | monsters are not extendable and need not be: script spawns live in the zone's memory, never in `world` (so `content check` stays clean); the item is a row now, its use effect is chunk 3's registry |
| kill streak, bounty | per-character values that survive restarts | an extension's own schema with a foreign key to `characters` (5.4); chunk 5 decides whether the API's store is one generated table there (`ext_<name>.values`) or a `content` table; either way remove drops it with the extension |

Rules for the later chunks that follow from this one: the manifest gains its keys only by raising nothing in format 1 (they are
reserved); a script never writes `defs` or `world` (the API is the game, decision 5 of section 2a); an extension's settings, options,
rules and scripts all stop counting when it is disabled; whether scripts are read from the database (copied at install, as rows are)
or from the folder (for live reload while writing one) is chunk 5's to settle (section 10); the release carries `extensions/` either
way (5.1).

## 6. The Kill Boost Stone as the first extension

`tools/defs/additions.sql` and `tools/sdata/client_rows.sql` become these two files, which produce **the same rows, byte for byte**
(the build proves it with the table hashes, section 9):

`extensions/kill-boost/extension.toml`: as the example in 5.2, with `description` and a comment block carrying what `additions.sql`'s
header said (the decisions in `docs/plans/done/client-patches.md` 1 and `docs/original/operator-console.md` decision 3; the items put the
skill id in `range` and its level in `attacktime`; skill 200 level 100 stays the monsters' Weakening Curse).

`extensions/kill-boost/rows.toml`:

```toml
# Skill 200 levels 1 to 3 (empty "??" in both copies): copies of the Mini Sun EXP Stone's buff 250/1 (a kind-4 buff, detail 82)
# without its ability, one hour, kept on death (fixrange 1: keeptime in hours, counting down while offline). They do nothing by
# themselves: the multipliers are the setting kill_boost_skills (extension.toml).

[[skill]]
id = "200/1"
copy = { from = "250/1" }
set = { skillname = "Kill Boost", keeptime = 1, fixrange = 1, abilitytype1 = 0, abilityvalue1 = 0, abilitytype2 = 0, abilityvalue2 = 0, abilitytype3 = 0, abilityvalue3 = 0 }
client = { name = "Kill Boost", look = "250/1", description = "Player kills count double for 1 hour. The effect remains after the character dies, and the hour keeps running while logged out." }
note = "kill-boost buff level 1, icon and look of the Mini Sun EXP Stone buff 250/1"

[[skill]]
id = "200/2"
copy = { from = "250/1" }
set = { skillname = "Kill Boost Lv2", keeptime = 1, fixrange = 1, abilitytype1 = 0, abilityvalue1 = 0, abilitytype2 = 0, abilityvalue2 = 0, abilitytype3 = 0, abilityvalue3 = 0 }
client = { name = "Kill Boost Lv2", look = "250/1", description = "Player kills count 5 times for 1 hour. The effect remains after the character dies, and the hour keeps running while logged out." }
note = "kill-boost buff level 2, icon and look of the Mini Sun EXP Stone buff 250/1"

[[skill]]
id = "200/3"
copy = { from = "250/1" }
set = { skillname = "Kill Boost Lv3", keeptime = 1, fixrange = 1, abilitytype1 = 0, abilityvalue1 = 0, abilitytype2 = 0, abilityvalue2 = 0, abilitytype3 = 0, abilityvalue3 = 0 }
client = { name = "Kill Boost Lv3", look = "250/1", description = "Player kills count 10 times for 1 hour. The effect remains after the character dies, and the hour keeps running while logged out." }
note = "kill-boost buff level 3, icon and look of the Mini Sun EXP Stone buff 250/1"

# Items 100/212 to 214 (the empty "???177" in both copies): the Mini Sun EXP Stone 100/108's trade and use columns; range = the skill
# id, attacktime = its level.

[[item]]
id = "100/212"
copy = { from = "100/108", columns = ["reqlevel", "country", "grow", "special", "slot", "quality", "buy", "sell", "grade", "drop", "server", "count"] }
set = { itemname = "Kill Boost Stone", range = 200, attacktime = 1 }
client = { name = "Kill Boost Stone", look = "100/108", description = "Player kills count double for 1 hour. The effect remains after the character dies, and the hour keeps running while logged out." }
note = "kill-boost stone level 1, looks like the Mini Sun EXP Stone 100/108"

[[item]]
id = "100/213"
copy = { from = "100/108", columns = ["reqlevel", "country", "grow", "special", "slot", "quality", "buy", "sell", "grade", "drop", "server", "count"] }
set = { itemname = "Kill Boost Stone Lv2", range = 200, attacktime = 2 }
client = { name = "Kill Boost Stone Lv2", look = "100/108", description = "Player kills count 5 times for 1 hour. The effect remains after the character dies, and the hour keeps running while logged out." }
note = "kill-boost stone level 2, looks like the Mini Sun EXP Stone 100/108"

[[item]]
id = "100/214"
copy = { from = "100/108", columns = ["reqlevel", "country", "grow", "special", "slot", "quality", "buy", "sell", "grade", "drop", "server", "count"] }
set = { itemname = "Kill Boost Stone Lv3", range = 200, attacktime = 3 }
client = { name = "Kill Boost Stone Lv3", look = "100/108", description = "Player kills count 10 times for 1 hour. The effect remains after the character dies, and the hour keeps running while logged out." }
note = "kill-boost stone level 3, looks like the Mini Sun EXP Stone 100/108"
```

Why it is the same as today: `additions.sql` copies every column of 250/1 but `rowid`, `skillid`, `skilllevel`, `skillname` and
`updatedate` onto 200/1, then sets the name, `keeptime`, `fixrange` and the abilities, then copies 200/1 onto levels 2 and 3 with
their names. `copy` without `columns` takes every column but the keys, `rowid` and `updatedate` (so `skillname` too, which `set`
replaces): the same columns. Levels 2 and 3 copy 250/1 and set the same values as level 1, which is what copying the changed level 1
gave. The items copy the same twelve columns of 100/108 onto the original placeholder rows and set the same three. The notes are the
old tables' notes, so `content.client_*` matches the old tables row for row once resolved (4.1). Check the `updatedate` of the six rows
when building: `additions.sql` never set it, so the live rows keep the placeholders' values, and the install must too.

Behaviour: `kill_boost_skills` has no built-in default (already so since 0.1.0); the extension's `[settings]` gives
`200/1:2,200/2:5,200/3:10`, so on the dev box kills still count 2×, 5×, 10× (until part B the dev box's `.env` sets the same value;
from part B it comes from the extension). Disabled, the stones and buffs stay and count 1×.

## 7. Settings in layers

### 7.1 The layers, lowest first

1. **Built-in defaults** (the code; today's defaults, the original's behaviour as 0.1.0 set them).
2. **Extension defaults**: `content.ext_settings` of the **enabled** extensions, in `install_seq` order (two enabled extensions never
   set the same setting: refused at install and enable).
3. **The config file** (`GAME_CONFIG`; unset, `config/game.toml` in the working folder; `tools/game-server.sh` sets the checkout's;
   deploy: `GAME_CONFIG=/etc/shaiya-afterglow/game.toml` in the env file).
4. **The data editor's edits**: `content.setting_values`, written only through the `edits` overlay (target `setting`, 7.4).

Then, while the server runs, the console's switches (exp rate, kill rate, shop, max users) change their four values until the next
start, as now (`docs/original/operator-console.md` decision 5). Every value is resolved once at start (decision 7).

### 7.2 The file

TOML with top-level keys only in chunk 1. The tables `[ext.<name>]` (7.5) and `[rules]` (chunk 2) are reserved: "line 9: [ext.welcome]
is reserved for chunk 5 (docs/plans/ext-1-foundation.md 7.5)"; any other table is an unknown key. The key is the old variable's name
without `GAME_`, in lowercase: `GAME_TRADE_CHAT_MIN_LEVEL` → `trade_chat_min_level`, `GAME_LOGOUT_SECS` → `logout_secs`, `GAME_LOG` →
`log`, `GAME_CHAT_LOG` → `chat_log`. Values may be a string (exactly today's variable text), an integer, a float, a boolean, or an
array of integers or strings (joined with `,` into today's text); so `market_maps = [35, 36, 42]` and `market_maps = "35,36,42"` are the
same. Each value then goes through today's parser (`flag`, `secs`, `maps`, `kill_boosts`, `ranking_times`, …), so every rule and range
stays as it is.

Errors stop the start and name the place: an unknown key (`config/game.toml line 4: unknown setting "trade_chat_minlevel"`), a value
of the wrong kind or out of range (`line 6: market_maps: map id "x"`), and for the other layers the layer (`extension kill-boost:
kill_boost_skills: …`, `data editor (change set 12): …`). A missing file is fine (an info line says the defaults are used and names
the path) unless `GAME_CONFIG` names it, then it is an error.

The environment: `DATABASE_URL`, `GAME_BIND`, `GAME_SERVER_ID`, `GAME_CONFIG` (and `RUST_LOG`) are read as now. **Any other `GAME_…`
variable stops the start**, with the lines to paste (the value as written, a string):

```
game settings are read from config/game.toml now, not from the environment (docs/server/settings.md). Put these lines in
config/game.toml (GAME_CONFIG names another file), remove the variables (from .env too: the run scripts load it), and start again:
guild_party_size = "2"                 # was GAME_GUILD_PARTY_SIZE
market_maps = "35,36,42"               # was GAME_MARKET_MAPS
```

An unknown `GAME_` name is listed as "not a setting" (a typo the old server ignored). The login server's and the editor's own variables
(`LOGIN_*`, `EDITOR_*`) stay environment variables in chunk 1 (they are start-up and transport settings); a later chunk may give the
login server the same file.

**The file `tools/setup.sh` writes** when `config/game.toml` is missing (decision 15):

```toml
# The game server's settings (docs/server/settings.md). With nothing set here, the server behaves as the original.
# Every setting with its default and meaning: config/game.toml.example (or: target/release/game-server config --example).
# What is in force and where each value comes from: target/release/game-server config.
# Changes take effect at the next start: tools/game-server.sh restart.
```

`config/game.toml.example` is `game-server config --example`'s output, committed; `config/game.toml` is the operator's (git-ignored).

### 7.3 The registry of settings

`crates/game-server/src/config.rs` gets one table, `pub const SETTINGS: &[Setting]`, with per setting: `name` (file key), `default`
(text), `doc` (one line, used for the example file), and the parser it goes through. `pub fn check_setting(name, value) -> Result<()>`
(used by `data-editor`), `pub fn resolve(sources) -> Result<(Config, Vec<Resolved>)>` where each `Resolved` is the name, the value and
the layer it came from. `Config::from_env` becomes `Config::load()`:
1. the start-up variables and the refusal of any other `GAME_` variable (no database yet);
2. the file;
3. **one** `PgConnection` to `DATABASE_URL`: the migrations run on it (as they run on the pool now), then `content.ext_settings` of the
   enabled extensions and `content.setting_values` are read; the connection closes;
4. `resolve`; then `main.rs` builds the pool with the resolved `db_pool` and `db_acquire` (both settings: the reason for step 3).

The tests keep building `Config` directly (`from_lookup` stays for them).

`game-server config` (a command-line word; no world, no listening; the steps 1 to 4 without the pool) prints the resolved settings as
TOML, each with a comment naming its layer and the default when it differs:

```toml
guild_party_size = 2                 # config file (default 7)
kill_boost_skills = "200/1:2,200/2:5,200/3:10"   # extension kill-boost (default "")
trade_chat_min_level = 10            # default
```

`game-server config --example` prints every setting commented out with its default and doc line (no database): that output is
committed as `config/game.toml.example`, and a test checks it is current. `config/game.toml` is in `.gitignore`.

### 7.4 The editor's layer

`data-editor`'s registry gets the target `setting`: table `content.setting_values (name, value)`, key = the setting's name, one field
`value` (a JSON string), add and remove allowed (an override is a row; removing it falls back to the layers below), validated by
`game_server::config::check_setting`, effect **restart**. Keys `ext.<name>.<key>` are refused as reserved (7.5). Its schema is
`content`, which no load ever replaces, so it has no marker: `apply` always writes it. History, Undo, export and import work as for
every target. The page's view of settings (each value, its layer, an override box) belongs to the editor's page work after part B of
`docs/plans/done/data-editing.md` (section 10).

### 7.5 Extension-owned options (specified, reserved; chunk 5 builds them)

An extension's own settings (the welcome text, a banner's interval, kill-streak thresholds, a bounty's amount), read by its scripts
(chunk 5) and Rust hooks (chunk 2), never by the core. The shape, so that chunk 1 refuses it rather than lets it be used wrongly:

```toml
# extension.toml
[options.welcome_text]
type = "string"            # string, integer, float, boolean, maps, strings (a list), integers (a list)
default = "Welcome to Afterglow"
doc = "The notice a player gets at login."
# min = 1, max = 100       # integer and float only
```

```toml
# config/game.toml
[ext.welcome]
welcome_text = "Welcome back"
```

The layers: the manifest's `default`, the file's `[ext.<name>]`, the editor's `setting` target with key `ext.<name>.<key>`; only
for an enabled extension (decision 4); validated by the `type`, `min` and `max` the manifest gives; `game-server config` lists them
under each extension. Chunk 1 refuses all three places: the manifest key `options` (5.2), the file's `[ext.…]` tables (7.2), the
editor key (7.4), each with "reserved for chunk 5".

## 8. On top: the edits overlay and `reload`

The order, every time the live tables are built or changed: **originals → installed extensions (in install order) → edits**. The
edits overlay is untouched: its markers now mean "the edits were written since the content under them last changed", and
`content rebuild`, `ext install`, `upgrade` and `remove` drop the marker of what they changed and run the edits' apply, which refreshes
the edits' `original` values (an edit's original is "what the content below gives", extensions included) and reports any edit whose
row moved under it. An edit on an extension's row is allowed (an `item_grade` edit on a Kill Boost Stone works as on any item), survives
install, upgrade and `content rebuild`, and blocks `remove` of that row until it is undone (decision 16); `content origin` shows both.

`reload` (console) is unchanged: it re-reads the monsters and drop tables from the live tables, so an extension's drop entries are
picked up by `reload`; items, skills and settings need the restart, and the commands say which.

## 9. Order of work on the dev box, the rehearsal, and the rollback

The live dev database (`newshaiya` in `newshaiya-postgres`) holds the test accounts' characters and the operator's edits history;
nothing in this chunk may lose them, and nothing in `defs`, `world` or `client` may change in it, not a byte (decision 12):

0. **The dev box moves to the clone** (no code change; before any build step). The dev servers run from a second worktree of the
   clone, `/media/drive0/projects/shaiya-afterglow-live` (`git worktree add --detach … main`), at the commit before the build, with
   its own `.env`: the old tree's `.env` plus `PG_CONTAINER=newshaiya-postgres`, `MSSQL_CONTAINER=newshaiya-mssql`,
   `PG_VOLUME=newshaiya-pgdata`, `MSSQL_VOLUME=newshaiya-mssql-data` (the database `newshaiya` stays in its `DATABASE_URL`, as
   CHANGELOG 0.2.0 says for setups made before the rename), and symbolic links `data` → the old tree's `data/` and `reference` → its
   `reference/` (both git-ignored as folders, so the links go in `.git/info/exclude`: `/data`, `/reference`) and `db/edits/` made. Its
   binaries built there; the old tree's login and game servers stopped (`tools/game-server.sh stop` there), the worktree's started;
   `servers list` shows game server 1 alive. The clone's migrations are 0001 to 0030 with the same SHA-256 as the old tree's
   (checked 2026-11-02), so this changes nothing in the database. From here on the old tree's binaries never run against `newshaiya`
   (its checklist page, `tools/checklist-server.sh`, reads files only and may stay). The development checkout
   (`/media/drive0/projects/shaiya-afterglow`) never points at `newshaiya`: its `.env` points at the drill (step 2).
1. **Backup and baseline** of the live database: `pg_dump -Fc` of the whole database to `data/backups/pre-ext1-<date>.dump`
   (git-ignored), restored once into a scratch database to prove the dump is good, then dropped; `tools/content/table_hashes.sql` (psql
   `\gexec`; committed) writes the rows and md5 of every `defs`, `world`, `client` table to `data/backups/pre-ext1-<date>.hashes`.
2. **The drill**: a PostgreSQL container of its own from the development checkout (`.env` with `PG_CONTAINER=shaiya-afterglow-ext1`,
   `PG_PORT=5434`, `PG_VOLUME=shaiya-afterglow-ext1-pgdata`, `COMPOSE_PROJECT_NAME=shaiya-afterglow-ext1`, `DATABASE_URL` on port 5434,
   database `shaiya`; `docker compose up -d postgres`), the dump of step 1 restored into it (`--no-owner`), the same symbolic links for
   `data` and `reference`. Its hashes equal the baseline. A game server of the drill, when one is started, binds its own port
   (`GAME_BIND=0.0.0.0:31810`) and server id; the login server is not needed (`game-server config` and a start to "listening" show
   what the build changed).
3. Code and tests against the drill only (the test suites' scratch schemas live there too).
4. **Rehearsal, on the drill**: migration 0031; the frozen copies with `--adopt` (`original_defs`, `original_world` loaded from the same
   files as the live tables; their comparison with the live tables must show **exactly** the six Kill Boost rows in `defs` and nothing
   in `world`; `client` is adopted the same way: `load_client.py --adopt` checks the files' SHA-256 and that `client.*` restores them,
   then records the import); `ext install extensions/kill-boost` (the hashes of `defs`, `world` and `client.items` / `client.skills`
   equal the baseline: **no byte changed**; `content.client_*` resolved equal the old `client.item_rows` / `skill_rows` row for row;
   `content check` clean); `client_patch.py adopt --patch 109` and `rebuild --patch 109` (`MATCH` for both files); only then are
   `client.item_rows`, `client.skill_rows`, `client.server_only` dropped. Every command and its output is kept in the build's notes.
5. **The live database, part A**: the live worktree moved to the built commit (`git -C …-live checkout --detach <commit>`), its binaries
   rebuilt, the servers stopped; the steps of 4 on `newshaiya`, in the same order, with the same results against the step 1 baseline;
   the servers started from the worktree; the Kill Boost live-test items listed.
6. **Part B** in the same way: the drill first (its `.env`'s `GAME_*` settings moved to its `config/game.toml`), then the live worktree:
   `config/game.toml` with the dev box's six game settings (`guild_party_size = 2`, `mini_game = true`, `slot_payout_percent = 220`,
   `card_win_chances = "420,200,130,100,80"`, `cross_faction_trade_maps = [42]`, `market_maps = [35, 36, 42]`; not
   `kill_boost_skills`, which now comes from the extension), the seven `GAME_*` lines removed from its `.env`, `game-server config`
   shows each from its layer, restart.

**Rollback**: until step 5 the live database has not changed at all. In step 5, until the install, nothing the servers read has
changed: drop the schemas `content`, `original_defs`, `original_world`, delete migration 31's row from `_sqlx_migrations`, check out the
commit before the build in the live worktree, rebuild, start. After the install: stop the game server, `pg_restore --clean --if-exists`
the dump of step 1, check out the commit before the build in the live worktree, rebuild, start. To give up the move itself: stop the
worktree's servers and start the old tree's again (the database is the one it knew, once restored). The build writes these procedures
into `docs/original/database-notes.md` with the dump's file name (without the dev box's paths: they go in the build's notes).

## 10. Open for later

- **Entry 11's answer**: the ceilings of 5.6 (growing item groups and the skill table, monsters, NPCs, quests), the handshake that tells
  the server a client's `fixes.dll` features (11 c), and what the updater can carry (11 d), after which art is shipped in patches.
- **More extendable tables** (monsters and `Monster.SData` for pets, chunk 4; NPCs, shops, quests with `NpcQuest.SData`; the mall with
  `Cash.SData`; spawns and portals), each a line in `ext/tables.rs` plus its client generator.
- **An extension adding entries to another's list setting** (two extensions each wanting a kill-boost entry): chunk 3 moves the kill
  boost onto the effect registry, after which settings stay single-owner.
- **For chunk 5** (section 5.8): the options of 7.5 built; where scripts are read from (copied into the database at install, or the
  folder for live reload while writing one); the per-character store (a generated table in `ext_<name>`, or one `content` table keyed by
  extension); whether enable and disable become live for scripts (`reload`) while rows stay restart-only; custom command names against the
  operator's commands.
- **The editor's page for settings and origins**; per-server values of a setting (one database, several game servers with different
  rates).
- **Patch generation without Python** for operators (`client_patch.py` is a dev tool): chunk 8's packaging.
- **The login server's settings in a file.**

## 11. The migration note (for `CHANGELOG.md`, 0.3.0)

The build adds an "Unreleased" section to `CHANGELOG.md` with the changes and this note, then keeps it current until 0.3.0 is tagged:

> **Upgrading from 0.2.0** (breaking changes; nothing is converted automatically):
> 1. **Settings**: the game settings are no longer environment variables. Stop the game server, start the new one once: it refuses
>    to start and prints the `config/game.toml` lines for every `GAME_…` setting it found (in `.env` too). Put them in
>    `config/game.toml` (on a VPS: `/etc/shaiya-afterglow/game.toml`, and `GAME_CONFIG=/etc/shaiya-afterglow/game.toml` in the env
>    file), remove the variables. `DATABASE_URL`, `GAME_BIND`, `GAME_SERVER_ID` and `RUST_LOG` stay variables.
> 2. **Game data**: run `tools/setup.sh` again with the same two folders. It imports the original data once into new schemas
>    (`original_defs`, `original_world`) and rebuilds `defs` and `world` from them plus your data editor's edits; changes made by hand
>    in SQL are replaced, as before on every re-run. Later re-runs skip the import; `--reset` re-imports corrected files.
> 3. **The Kill Boost Stones**: `tools/defs/additions.sh` is gone. If you used `--with-additions`, install the extension instead
>    (`target/release/data-editor ext install extensions/kill-boost`, or `tools/setup.sh --extension extensions/kill-boost`) before
>    the game server starts, so held stones stay stones; its kill multipliers come with it, so drop `kill_boost_skills` from your
>    settings unless you changed them.
> 4. **Moving game data to a VPS**: dump `defs`, `world`, `original_defs`, `original_world`, `content` and `edits`
>    (`docs/server/deploy.md` section 3).

## Build

Two parts, one after the other: **part A** (the dev box moved, the content schema, the frozen copies, extensions, the Kill Boost Stone
moved, the client generator on the database, setup and the release; steps A0 to A10) and **part B** (settings in layers, the editor's
setting target, docs; steps B1 to B6). Each reads sections 1 to 9 first.

Conventions: no `cargo fmt` (wide lines by convention); Python that reads client files runs with `python3 -I`; cargo is
`~/.cargo/bin/cargo`. Match the surrounding code: doc comments naming the docs section, `anyhow`, tests next to the code, PostgreSQL
tests skipped without `DATABASE_URL` (as `crates/data-editor/tests/postgres.rs`). Dependencies: the current `toml` and `semver` crates.
**Which server is "the dev game server"**: during the build, the drill's (rebuilt and restarted after each commit that changes the
server); the live one only at A10 and B6. Stage named paths only (other sessions share the tree), commit without any mention of AI.
Decisions 1 to 16 are the maintainer's; when the plan leaves something open or the code disagrees with it, the question goes to the
maintainer.

### Part A

**A0. The dev box moves to the clone** (section 9 step 0). Nothing committed (everything there is git-ignored or local); the build's
notes record the worktree, the `.env` names (not its passwords) and the check that the game server started and `servers list` shows it.

**A1. Backup, baseline and the drill** (section 9 steps 1 and 2). `tools/content/table_hashes.sql`: for every table of the schemas named
in the psql variable `schemas` (default `defs,world,client`), one line `schema.table|rows|md5`, the md5 over the rows' text in a stable
order (`string_agg(t::text, E'\n' ORDER BY t::text)`), built with `format()` and `\gexec`. Dump the live database, test-restore it into
a scratch database (`createdb`/`pg_restore`/`dropdb` in the container), write the hashes, then make the drill and check its hashes
equal them. Commit the SQL file; report the file names.

**A2. Migration `db/migrations/0031_content.sql`** with a comment on each table as the other migrations:

```sql
CREATE SCHEMA content;

CREATE TABLE content.imports (                    -- one per imported part (3.2)
    part        text PRIMARY KEY CHECK (part IN ('defs', 'world', 'client')),
    imported_at timestamptz NOT NULL DEFAULT now(),
    rebuilt_at  timestamptz,                      -- the last content rebuild of the part; NULL or older than imported_at: due
    source      text NOT NULL,                    -- what the caller said it read (--source), or the folder
    files       jsonb NOT NULL,                   -- {file name: sha256}: "the same files" of a re-run
    tables      jsonb NOT NULL                    -- {table: {"rows": n, "md5": "…"}} of the frozen copy (client: of client.items/skills)
);

CREATE TABLE content.extensions (
    name         text PRIMARY KEY CHECK (name ~ '^[a-z][a-z0-9-]{1,39}$'),
    version      text NOT NULL,
    title        text NOT NULL,
    description  text NOT NULL,
    manifest     jsonb NOT NULL,                  -- extension.toml as read
    enabled      boolean NOT NULL DEFAULT true,
    install_seq  integer GENERATED ALWAYS AS IDENTITY UNIQUE,
    installed_at timestamptz NOT NULL DEFAULT now(),
    upgraded_at  timestamptz
);

CREATE TABLE content.ext_ranges (
    extension  text NOT NULL REFERENCES content.extensions (name) ON DELETE CASCADE,
    kind       text NOT NULL CHECK (kind IN ('item', 'skill')),
    major      integer NOT NULL,                  -- item type / skill id
    minor_low  integer NOT NULL,                  -- typeid / level
    minor_high integer NOT NULL CHECK (minor_high >= minor_low),
    PRIMARY KEY (extension, kind, major, minor_low)
);

CREATE TABLE content.ext_rows (                   -- the rows an extension owns (3.3); one owner per row
    extension text NOT NULL REFERENCES content.extensions (name),
    tbl       text NOT NULL,                      -- 'defs.items', 'defs.skills', 'defs.mob_items'
    key       text NOT NULL,                      -- the data editor's key text: '100212', '200/1', '2133/4'
    action    text NOT NULL CHECK (action IN ('add', 'change')),
    row       jsonb NOT NULL,                     -- the whole row as written
    seq       integer NOT NULL,                   -- its place in the rows files
    PRIMARY KEY (tbl, key)
);

CREATE TABLE content.ext_settings (
    extension text NOT NULL REFERENCES content.extensions (name) ON DELETE CASCADE,
    name      text NOT NULL,
    value     text NOT NULL,
    PRIMARY KEY (extension, name)
);

CREATE TABLE content.ext_migrations (
    extension  text NOT NULL REFERENCES content.extensions (name) ON DELETE CASCADE,
    version    integer NOT NULL,
    name       text NOT NULL,
    sha256     text NOT NULL,
    applied_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (extension, version)
);

CREATE TABLE content.ext_client (                 -- fixes.dll features and art (5.2, 4.3)
    extension text NOT NULL REFERENCES content.extensions (name) ON DELETE CASCADE,
    kind      text NOT NULL CHECK (kind IN ('feature', 'art')),
    name      text NOT NULL,                      -- feature name, or the archive member path
    file      text,                               -- art: the file in the folder
    sha256    text,                               -- art: the file's hash
    PRIMARY KEY (extension, kind, name)
);

CREATE TABLE content.ext_log (
    id        integer GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    at        timestamptz NOT NULL DEFAULT now(),
    extension text NOT NULL,
    action    text NOT NULL CHECK (action IN ('install', 'upgrade', 'remove', 'enable', 'disable', 'force-delete')),
    detail    jsonb NOT NULL
);

-- The client rows (4.1): the owner, the text (or keep), the look-alike as a reference and the fields given by name (NULL: not given);
-- client_patch.py resolves them against the client schema. since_patch stamped by generate.
CREATE TABLE content.client_items (
    type smallint NOT NULL, typeid smallint NOT NULL,
    extension text NOT NULL REFERENCES content.extensions (name),
    keep boolean NOT NULL DEFAULT false,
    name text, description text,
    look_type smallint, look_typeid smallint,
    model smallint, icon smallint, type2 smallint, type3 smallint,
    since_patch integer, note text NOT NULL,
    PRIMARY KEY (type, typeid),
    CHECK (keep OR (name IS NOT NULL AND description IS NOT NULL)),
    CHECK ((look_type IS NULL) = (look_typeid IS NULL))
);
CREATE TABLE content.client_skills (
    skill_id smallint NOT NULL, level smallint NOT NULL,
    extension text NOT NULL REFERENCES content.extensions (name),
    keep boolean NOT NULL DEFAULT false,
    name text, description text,
    look_skill smallint, look_level smallint,
    icon integer, animation integer, toggle_type smallint, sound integer,
    since_patch integer, note text NOT NULL,
    PRIMARY KEY (skill_id, level),
    CHECK (keep OR (name IS NOT NULL AND description IS NOT NULL)),
    CHECK ((look_skill IS NULL) = (look_level IS NULL))
);
CREATE TABLE content.server_only (
    kind text NOT NULL CHECK (kind IN ('item', 'skill')), id integer NOT NULL, level smallint NOT NULL DEFAULT 0,
    extension text NOT NULL REFERENCES content.extensions (name),
    reason text NOT NULL,
    PRIMARY KEY (kind, id, level)
);

-- The patch ledger (4.2).
CREATE TABLE content.client_patches (
    patch            integer PRIMARY KEY,
    previous         integer NOT NULL,
    generated_at     timestamptz NOT NULL DEFAULT now(),
    commit           text NOT NULL,               -- the commit the manifest names (patch 109: the old private tree's)
    extensions       jsonb NOT NULL,              -- {name: version}
    dirty            boolean NOT NULL,
    patch_sha256     text,                        -- the client side's psNNNN.patch and .sig, once packed
    sig_sha256       text,
    status           text NOT NULL DEFAULT 'generated' CHECK (status IN ('generated', 'staged', 'live', 'withdrawn'))
);
CREATE TABLE content.client_patch_files (
    patch           integer NOT NULL REFERENCES content.client_patches (patch),
    member          text NOT NULL,
    us_path         text NOT NULL,
    size            integer NOT NULL,
    sha256          text NOT NULL,
    plain_sha256    text NOT NULL,
    plain_size      integer NOT NULL,
    replaces_sha256 text NOT NULL,
    changed         text[] NOT NULL,
    encrypted       bytea NOT NULL,
    PRIMARY KEY (patch, member)
);

-- The data editor's settings layer (7.4).
CREATE TABLE content.setting_values (
    name  text PRIMARY KEY,
    value text NOT NULL
);
```

Run the whole suite against the drill (the migration applies in the test databases too). Commit.

**A3. `data-editor content`** (`crates/data-editor/src/content.rs`, and the `content …` words in `main.rs`'s usage): `status`,
`hashes`, `rebuild [defs|world|all] [--into <prefix>]`, `check`, `origin <table> <key>` (section 3). `Schemas` gains `content`,
`original_defs`, `original_world`, `client` (defaults as named; scratch names in tests). The DDL for a missing live schema is
`include_str!` of `db/defs/schema.sql` / `db/world/schema.sql` with the schema name substituted. Rebuild calls the existing apply
functions with the connection of its transaction (read `apply.rs`; generalise the "the import changed it" wording to "the content under
your edit changed") and sets `rebuilt_at`. PostgreSQL tests in scratch schemas made from the two DDL files with a few rows: a rebuild of
a part gives the frozen copy plus an `ext_rows` row plus an edit; twice gives the same hashes; a part without an import is refused; a
missing live schema is created; `check` finds a hand edit in the live table and a changed frozen table; `origin` prints the three
layers; `status` without imports names `tools/setup.sh`. Commit.

**A4. The importers** (Python):
- `load_into_postgres.py <export dir> <schema.sql> [--source TEXT] [--reset | --adopt]`: loads into `original_defs` (the DDL text with
  `defs.` → `original_defs.` and its `CREATE SCHEMA` line); with a `content.imports` `'defs'` row: the same files → "nothing to do"
  (and the unfinished rebuild run, 3.2), other files → refused, unless `--reset`; no row but a `defs` schema → the 0.2.0 line, then a
  first import. Records the import (each `.psv`'s SHA-256, each table's rows and md5 by the same SQL as `table_hashes.sql`) in the
  transaction that fills the frozen copy; then, unless `--adopt`, runs `target/release/data-editor content rebuild defs` (or prints the
  command when the binary is missing). Prints the `verify.py` command.
- `verify.py <schema.sql> <columns> [--schema original_defs] [--counts-only]`.
- `tools/world/import.py <Data folder> [--dry-run] [--source TEXT] [--reset | --adopt]`: the same for `original_world` (substitute the
  schema in the DDL and in `copy()`), recording each file read with its SHA-256; ends with `content rebuild world` instead of
  `data-editor apply`.
- `tools/sdata/load_client.py [--reset | --adopt]`: the same rule (the two files' SHA-256 are fixed, so "other files" cannot pass its
  check anyway); `--adopt` checks the two files' SHA-256 (as now) and that `client.items` / `client.skills` restore them
  (`client_patch.restored()`), then records the import without touching the tables. `db/client/schema.sql` loses `item_rows`,
  `skill_rows`, `server_only` (and their comments) at A7, not before (the old tables are compared at A6).
- **On the drill** (section 9 step 4): run all three with `--adopt` (the export is `data/defs-export/`; first prove it is the backup's:
  `read_bak.py /media/drive0/projects/OGShaiyaFiles/Server/PSM_Data/PS_GameDefs <scratch>` writes identical files; the Data folder is
  `/media/drive0/projects/OGShaiyaFiles/Server/PSM_Server/ShaiyaServerPatch/Server/GameMain/PS_Game/Data`, read only; the other
  `PSM_Data` under `Server/New Files/` is another pack, not this one), then compare frozen and live (`EXCEPT` per table): exactly the six
  Kill Boost rows in `defs`, nothing in `world`. Run each importer again without a flag (nothing to do) and with one changed export file in a scratch copy (refused). Run
  `verify.py --schema original_defs` with the SQL Server container: the dev box's `newshaiya-mssql` belongs to the live setup, so start
  the drill's own (`docker compose --profile import up -d mssql` with `MSSQL_CONTAINER=shaiya-afterglow-ext1-mssql`, `MSSQL_PORT=14334`,
  `MSSQL_VOLUME=shaiya-afterglow-ext1-mssql-data`, `PSM_DATA_DIR` the original `PSM_Data`), or skip the full check and run
  `--counts-only` when the restore is not worth its 20 minutes (say which in the notes). Commit.

**A5. Extensions** (`crates/data-editor/src/ext/`: `manifest.rs` parse and validate `extension.toml`; `rows.rs` the rows files and the
computed rows; `tables.rs` the extendable tables of 5.5; `ids.rs` ranges and ceilings of 5.6, with the 100 group counts; `refs.rs` the
holder and reference checks of 5.7; `ops.rs` install, upgrade, remove, enable, disable; `main.rs` the `ext …` words). Settings are
validated with `game_server::config::check_setting`: add that function in `config.rs` now, as a match over the setting names calling
today's parsers (part B turns it into the registry). PostgreSQL tests with fixture extensions under
`crates/data-editor/tests/fixtures/ext/`:
- install a Kill-Boost-like fixture: the rows are exactly as computed; `ext_rows`, client rows (references kept, not resolved),
  settings recorded; install again refused; `--skip-installed` with the same version: exit 0, nothing written; with another version:
  refused naming `ext upgrade`; `content rebuild` afterwards gives the same hashes as the incremental install (the strongest test: both
  paths agree); an install without a `client` schema works and skips the length check;
- refused, each with its message checked: no `content.imports` (the setup hint); a range over another extension's; an entry outside
  its own ranges; a key another extension owns; type 101; typeid past the group's count; a group with no records; skill 327; level 100;
  an unknown column; `rowid` or a key column in `set`; a changed item without `client` or `server_only`; non-ASCII client text;
  too-long client text (with a `client` schema); an unknown setting; a bad setting value; a setting another enabled extension sets;
  `[[drop]]` with a column other than grade and droprate, or out of the editor's ranges; a migration that writes into `defs` (rolled
  back, nothing left behind); `format = 2`; an unknown manifest key; each reserved key (`options`, `rules`, `scripts`, `api`);
- remove: every table's hash equals the one before the install; an added row is gone; with a `character_items` row holding an owned
  item: refused with the holder named; `--force --yes` deletes it and logs it; `--force` refused while a `game_servers` row has players
  and a fresh `last_seen`; with a `defs.product_list` row naming an owned item, a `world.npc_shop_items` row, an original item giving an
  owned skill, and an `item_grade` edit on an owned item: each refused and listed, `--force` refused the same, and remove passes once
  they are gone (the edit undone);
- upgrade: a dropped key restored (and refused while held or referenced), a changed one rewritten, a new one claimed, `since_patch`
  kept for an unchanged client row and cleared for a changed one; the same or a lower version refused; a changed applied migration
  refused;
- disable / enable: the flag; enable refused on a setting clash;
- edits on top: an `item_grade` edit on an owned item survives install, upgrade and `content rebuild`, its original refreshed and the
  change reported.
Commit.

**A6. The Kill Boost Stone extension** (section 6): `extensions/kill-boost/extension.toml`, `rows.toml`, a short `README.md`. `ext check
extensions/kill-boost` lists the six changes. Then on the drill (section 9 step 4): `ext install extensions/kill-boost`; the hashes of
`defs`, `world`, `client.items`, `client.skills` equal the A1 baseline; `content.client_items` / `client_skills`, resolved against
`client.items` / `client.skills` in SQL (4.1), equal the old `client.item_rows` / `skill_rows` (the `extension` and `since_patch` columns
aside); `content check` clean. If a hash differs, stop: find the column (`content origin`), fix the rows file, `ext remove`, install
again. Commit.

**A7. `client_patch.py` on the database** (section 4.2): rows from `content.*` resolved as 4.1 (no `client_rows.sql`); check 1 from
`content.imports`; the originals from the restore; check 4 against `original_defs` (keep the grade-only exception and its test); the
ledger read and written; `since_patch` stamped by `generate`; `adopt`, `rebuild`, `record`; the manifest's `commit` and `extensions`; the
git check paths. Extend `test_client_patch.py` (no database): the previous-version choice from a ledger list, which rows `rebuild --patch
N` takes, the resolution order of a client field (by name, look-alike, own), the manifest's fields. On the drill: `client_patch.py
restore` and `check` pass; `adopt --patch 109` (patch and signature SHA-256 and status `staged` from `docs/client/client-patch-0109.md`
section 2); `rebuild --patch 109 --out <scratch>` prints `MATCH` for both files, and their SHA-256 are
`a92292782764918ec1e6b8b929fe66617d340da183ec4664eba46c80ddf47bea` (Item) and
`6c5b4bc42032949494efd97a226008c6af9f44df9d52b211c0fc5ca5fb54649c` (Skill). Try `check` with an unowned change in a transaction you roll
back (check 4 must fail). Then `DROP TABLE client.item_rows, client.skill_rows, client.server_only` (on the drill; the live database at
A10), remove them from `db/client/schema.sql`, delete `tools/defs/additions.sql`, `additions.sh`, `tools/sdata/client_rows.sql`, and fix
every reference to them (`git grep -n "additions\.\(sql\|sh\)\|client_rows\|with-additions"`: docs, README, `tools/README.md`,
`tools/sdata/compare_defs.py`, `.github/ISSUE_TEMPLATE/feature_request.md`, scripts; dated plans and analyses keep their history).
Commit.

**A8. `tools/setup.sh`, the release and the drills of the install** (sections 3.2, 5.1, 7.2):
- `tools/setup.sh` as section 3.2 (the options `--reset` and `--extension <folder>`; the importers called with `--source`; verify only
  after an import; `ext install --skip-installed`; `config/game.toml` written when missing; "Left to do" with the bundled extensions;
  the header's "safe to run again" sentence rewritten). Until part B the game server ignores `config/game.toml`; the file setup writes
  holds comments only, so nothing changes.
- `.github/workflows/release.yml`: the package step copies `extensions/` and `config/game.toml.example` into the tarball's folder
  (`config/game.toml.example` exists from B1; until then the step copies it only when present). `tools/fetch-release.sh`: installs the
  top-level files only, `--keep <dir>` keeps the unpacked tarball.
- **Drills** (each with its own containers, port and `COMPOSE_PROJECT_NAME`, in a scratch clone or worktree, removed afterwards; never
  the dev box's containers):
  - a fresh install: `tools/setup.sh --extension extensions/kill-boost <PSM_Data> <Data>` from nothing (the direct reader): import
    records written, the six rows in place, `config/game.toml` written; run it again: both parts "nothing to do", the extension "already
    installed", the file kept; `--reset`: both parts rebuilt, the extension's rows and an edit made in between still in place;
  - an upgrade from 0.2.0: `git checkout v0.2.0` in a scratch clone, its `tools/setup.sh` (it downloads the v0.2.0 release), a character
    created with `create_account` and a few rows in `character_items`, `tools/defs/additions.sh --with-additions`; then the new commit:
    the game server starts (migration 0031) and runs as before; `ext install` refuses with the setup hint; `tools/setup.sh` again (the
    0.2.0 line, then the import); `data-editor ext install extensions/kill-boost`; the character and its items unchanged, the six rows
    as before, `content check` clean. This proves section 11.
  Commit.

**A9. Docs for part A**: `docs/original/database-notes.md` (the schemas of 3.1, import once and reset, the order originals → extensions →
edits, the rollback of section 9 with the dump's name), `docs/server/deploy.md` (section 3: the game-data dump with the six schemas, or
`tools/setup.sh` on the VPS itself; section 6b: `content rebuild` in place of `additions.sh` and `apply` after a load; an operator's
`ext install` and `remove`, `content check`; section 9: the upgrade, pointing at the changelog's note), `docs/server/deviations.md`
section 1 (the Kill Boost row: the extension, `data-editor ext install extensions/kill-boost`, in place of `additions.sh
--with-additions`), `docs/plans/done/client-patches.md` (section 2's table: the client rows now come from extensions; section 4: the ledger,
`adopt`, `rebuild`; section 9: patch 109's patch and signature hashes and status "staged, in-game pending"),
`docs/plans/done/data-editing.md` section 3 (the order after a load is now `content rebuild`), `docs/client/sdata-generation.md` is read
only (leave it), `tools/README.md` (the importers' flags, the content folder, the deleted files), README (setup's new options, the `ext`
and `content` commands, the extensions folder, the test count), `CHANGELOG.md` ("Unreleased": part A and the migration note of section
11, steps 2 to 4), `docs/plans/roadmap.md` ("Next: extensions": part A built), `docs/testing/live-test-checklist.md` a section "Extensions
foundation" with the items below and its line in `docs/testing/live-test-solo.md`. Run `tools/check_links.py` and `site/build.py`. Commit.

**A10. The live database, part A** (section 9 step 5): the live worktree to the commit of A9, rebuilt; the servers stopped; migration
0031 (the first start of any binary, or `servers list`); the three `--adopt` imports; the frozen-and-live comparison (six rows, nothing
in `world`); `ext install extensions/kill-boost` with the hashes against the A1 baseline; `client_patch.py adopt --patch 109` and
`rebuild --patch 109` (`MATCH`); `DROP TABLE client.item_rows, client.skill_rows, client.server_only`; `content check` clean; the servers
started from the worktree. The build's notes keep every output. Nothing to commit unless a step disagreed with the drill (then stop and
ask).

Live-test items (what only the real client shows):
- [ ] After the move: `/imake 100 212 1`, `100 213 1`, `100 214 1` give the three stones; each use shows its buff for one hour (with
  patch 109 installed: names, icons and text as before); a player kill with each counts 2×, 5×, 10× (section 51 unchanged).
- [ ] (part B) `data-editor ext disable kill-boost` and a game server restart: a stone still uses and shows its buff, a player kill counts 1×;
  `ext enable kill-boost` and a restart: 2× again.
- [ ] (part B) A guild founded by a party of two (the dev box's `guild_party_size = 2` now from `config/game.toml`); the gambling house's
  payouts and the personal shop maps as before (their settings from the file too).

### Part B

**B1. The settings registry** (`config.rs`, section 7.3): `SETTINGS`, `check_setting` (replacing A5's match), `resolve`, the TOML
reading of 7.2 with line numbers in errors (the `toml` crate's spans), the reserved tables, the environment refusal with its lines to
paste, `Config::load()` (one connection, then the pool), `game-server config` and `config --example`. Unit tests: every setting's
default resolves to today's `Config`; each layer beats the one below; a disabled extension's default does not count; an array and a
string give the same value; an unknown key, a reserved table and a bad value name the line; a `GAME_` variable that is not start-up stops
the start and the message holds its pasteable line; an unknown `GAME_` name is listed as "not a setting"; `config --example` equals
`config/game.toml.example` (commit that file); `config/game.toml` added to `.gitignore`.

**B2. `main.rs` and the scripts**: the server resolves its settings at start and logs one line per value not at its default with its
layer. `tools/game-server.sh` exports `GAME_CONFIG` when unset (`$root/config/game.toml`). `docs/server/deploy.md`'s env file keeps the
start-up variables and gains `GAME_CONFIG=/etc/shaiya-afterglow/game.toml`; the game settings go to that file. The release package now
carries `config/game.toml.example` (A8).

**B3. The drill**: its `.env`'s `GAME_*` game settings moved to its `config/game.toml` (the refusal's own lines, pasted); `game-server
config` shows each from the file and `kill_boost_skills` from the extension; a start to "listening" on its own port; the log agrees.

**B4. The editor's `setting` target** (section 7.4): registry entry (side `content`, no marker), validation through `check_setting`,
effect restart, the reserved `ext.` keys refused; PostgreSQL tests: draft → apply → `content.setting_values` row; `game-server`'s
`resolve` (called with that table's rows) takes it over the file; undo removes it; export and import carry it. A dry run on the drill:
`data-editor draft add set setting trade_chat_min_level value 12`, `apply-draft`, `game-server config` shows it from the editor, `undo`,
shows the default again.

**B5. Docs and the rest**: `docs/server/settings.md` (published on the site) rewritten around the file: the game server's table keyed by
the file keys (the variable names gone, except the start-up ones), the layers, `game-server config`, the refusal; every live document
that tells the operator to set a `GAME_…` game setting (README's settings and quick start, `.env.example`'s examples,
`docs/server/deploy.md`, `docs/server/deviations.md` (its setting names), `docs/testing/live-test-checklist.md`,
`docs/testing/live-test-solo.md`, `docs/original/operator-console.md`, any other `git grep -n "GAME_[A-Z_]*"` outside dated plans and
analyses written before today) uses the file key instead, with one sentence where the habit was "export it before the restart" (now:
put it in `config/game.toml`, restart). `CHANGELOG.md` "Unreleased": part B and step 1 of the migration note. `docs/plans/roadmap.md`
("Next: extensions": chunk 1 built), `docs/plans/extensions.md` section 3 (what exists now) and 7.1 (built). `tools/check_links.py` and
`site/build.py` clean. Commit; ask before pushing or deploying the site.

**B6. The live dev box, part B** (section 9 step 6): the live worktree to the commit of B5, rebuilt; `config/game.toml` with the six
settings; the seven `GAME_*` lines removed from its `.env`; `game-server config` shows each from its layer; restart; the log agrees.
The live-test items of part B listed for the maintainer.

## Parts

Part A: Build steps A0 to A10 (the dev box onto a worktree of the clone, backup, baseline and the drill database, migration 0031 (the
content schema), the data-editor `content` commands, the importers writing the frozen copies (a re-run with the same files does nothing,
`--reset`, `--adopt` on a box that already has data), the extension manager (`ext …`, with the holder and reference checks), the Kill
Boost Stone moved into `extensions/kill-boost` (the live tables must keep every byte: compare with the A1 hashes), `client_patch.py` on
the database with patch 109 regenerated byte for byte, the old additions/client_rows files removed, `tools/setup.sh` (`--reset`,
`--extension`, `config/game.toml`), the release package, the fresh-install and 0.2.0-upgrade drills, the docs, then the same on the
live database). Precondition: no uncommitted changes in `crates/data-editor/`.

Part B: Build steps B1 to B6 (the settings registry and the TOML config file with its layers (defaults, enabled extensions,
`config/game.toml`, the data editor), the start-up-only environment (a game setting left in the environment stops the start and prints
its line for the file), `game-server config` and `--example`, the drill's and then the dev box's `config/game.toml`, the editor's
`setting` target, the docs). Existing behaviour does not change except as this plan says (the whole suite passes).
