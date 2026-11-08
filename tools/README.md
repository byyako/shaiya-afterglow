# tools/

Scripts that run the servers, import the original data, read the original files, test the server by hand and keep the repository honest.
Who runs each: the **operator** (someone running a server), a **developer** (someone working on this repository), or **CI**
(`.github/workflows/ci.yml`). Needs: Python 3 with the standard library unless a row says more; `pefile` (`pip install -r tools/requirements.txt`)
for the three tools that read executables; Docker for the database containers; `objdump` and the GNU assembler (`as`) for the patch kit's
developer tools. Every shell script reads `.env` from the repository root (`.env.example` lists the names); `tools/dbconn.sh` and
`tools/dbconn.py` give them (and the importers) the database containers and `psql`.

## Running the servers

| script | what it does | reads, writes | who |
|---|---|---|---|
| `setup.sh` | everything from a fresh clone to a running server: writes `.env` with random passwords, starts PostgreSQL, builds the workspace (or uses the binaries in `target/release/`), reads the definitions from the backup (`defs/read_bak.py`) or an export folder, or with `--sqlserver` through SQL Server, imports the originals once and builds the live tables (a re-run skips what is imported from the same files; `--reset` imports corrected files; `--extension <folder>` installs an extension), writes `config/game.toml` when missing, and on a terminal creates the first account and starts the servers | `.env`, `compose.yaml`, the original files' `PSM_Data` and `PS_Game/Data`; the databases | operator |
| `fetch-release.sh` | downloads the binaries of a release (built by `.github/workflows/release.yml`) into `target/release/`, so nothing needs building; checks the checksum | GitHub; `target/release/` | operator |
| `release-notes.sh` | prints the text of a release: its section of `CHANGELOG.md` and how to start (used by the release workflow) | `CHANGELOG.md` | developer, CI |
| `restore_backups.sh` | restores every SQL Server backup in `/backups` (the original `PSM_Data`, mounted read-only) into the SQL Server container; runs inside it | the backups; SQL Server | operator (`setup.sh` calls it) |
| `login-server.sh`, `game-server.sh`, `data-editor.sh` | start, stop, restart, status and logs of the program in the background (`game-server.sh` restarts it after any exit status but 0; 75 is the console's `restart`) | `.env`; `logs/<program>.log`, `.pid` | operator, developer |
| `content/table_hashes.sql` | one line (table, rows, md5, separated by pipes) per table of `defs`, `world` and `client` (`psql -At -f`): the baseline before a change to the game data and its proof afterwards; `data-editor content hashes` prints the same | PostgreSQL (read only) | developer |
| `content_import.py` | what the three importers share: the import record, the same-files / other-files / `--reset` / `--adopt` rule, the rebuild call | `content.imports` | (imported by the importers) |
| `dbconn.sh`, `dbconn.py` | the shared environment of the scripts: container names, `psql` through Docker or on the host | `.env` | (sourced by the others) |

## Importing the original data

| script | what it does | reads, writes | who |
|---|---|---|---|
| `defs/read_bak.py` | reads the `PS_GameDefs` tables straight out of the original SQL Server backup (no SQL Server) and writes the same pipe-separated files; checked byte for byte against the SQL Server export | the backup; `data/defs-export/*.psv` (git-ignored) | operator |
| `defs/export_from_mssql.sh` | exports the `PS_GameDefs` tables from the restored SQL Server container (the `--sqlserver` way) to pipe-separated files | SQL Server; `data/defs-export/*.psv` (git-ignored) | operator |
| `defs/load_into_postgres.py` | loads those files once into the frozen copy `original_defs` (`db/defs/schema.sql`), records the import in `content.imports`, and ends with `data-editor content rebuild defs`; the same files again do nothing, other files are refused (`--reset` imports again, `--adopt` records a copy on a box that already holds the data, `--source` says where the files came from) | `data/defs-export/`; PostgreSQL | operator |
| `defs/verify.py` | compares the loaded `defs` tables with the original database, field by field (`--counts-only`: only the row counts, against the column list, with no SQL Server) | both databases | operator, developer |
| `defs/generate.py` | makes `db/defs/schema.sql` and the export queries (`export_queries.tsv`) from the column list `docs/original/db/PS_GameDefs.columns.txt` | the column list | developer |
| `defs/describe_mssql.sh` | writes the three description files per original database (`docs/original/db/`), proven to reproduce them byte for byte | SQL Server (read only) | developer |
| `world/import.py` | imports the maps, NPCs, spawns, portals, obelisks, boss groups, monster AI and Lua, and the quests from the original server's `Data` folder once into the frozen copy `original_world`, records every file read with its SHA-256, and ends with `data-editor content rebuild world`; the same flags as the definitions' loader (`--dry-run` only reports) | the `Data` folder; PostgreSQL. It uses `svmap.py` and `npcquest.py` from this folder | operator |
| `sdata/` | the client's data files: see below | | developer |

## The live-test page

| script | what it does | who |
|---|---|---|
| `checklist-server.sh` (`checklist_server.py`) | serves the live-test checklist as a page on port 30880 (`CHECKLIST_BIND`), built from `docs/testing/live-test-checklist.md` at every load, and keeps the results in `logs/live-test-results.json`; `report` prints the marked items as text | developer, testers |
| `checklist_page.py`, `checklist_page.html` | build the same page as one stand-alone HTML file (`checklist_page.py <out.html>`); it does not save results, it has "Copy results" | developer |
| `smoke-list-sync.sh` | copies the body of `docs/testing/smoke-list.md` into section 0 of the checklist | developer |

## The patch kit for the original server files

`server-patches/` is the kit people running the original `ps_game.exe` use (`docs/original/server-patch-notes.md`, the docs site's patch kit page).

| file | what it does | who |
|---|---|---|
| `patch_ps_game.py` (with `fixes.json`) | a checked, reversible byte patcher: refuses any file but the expected build, applies fixes by id, lists, reverts; never runs the executable | operator of an original server |
| `build_fixes.py` | assembles the fixes and writes `fixes.json`; run with the read-only `ps_game.exe` as its argument, it also checks each fix's original bytes. Needs `as` | developer |
| `test_patch_ps_game.py`, `test_input_filters.py`, `test_pass2.py`, `test_pass3.py` | tests of the kit: each fix applied to a copy, byte for byte, and the emulated filters. They need the exact `ps_game.exe` (`PS_GAME_EXE`, or next to the tests); the emulating ones need `unicorn`, one needs `objdump`. They skip without them | developer |

## Reading the original files

All of these only read; none runs anything from the original files.

| script | what it does | reads, writes | needs |
|---|---|---|---|
| `disasm.py` | disassembles functions of an executable by PDB name or address, with call targets named | an executable and a `symbols.tsv` of `docs/original/symbols/`; prints | `objdump` |
| `pdb_symbols.py` | extracts function and data symbols from a PDB and maps them to addresses (`docs/original/symbols/*.tsv`) | `.exe`, `.pdb`; one `.tsv` | `pefile` |
| `pdb_types.py` | prints the struct, class, union and enum layouts of a PDB (`docs/original/symbols/ps_game.packets.txt`) | a `.pdb`; prints | |
| `pdb_check.py` | prints a PDB's identity, to match it with an executable (`docs/original/inventory/server/pdb_identity.txt`) | `.pdb` files; prints | |
| `inventory_server.py` | inventories a directory tree of server files: hashes, PE headers, imports, network strings (`docs/original/inventory/`) | the tree; an output folder | `pefile` |
| `svmap.py`, `npcquest.py` | parse the server's `.svmap` map files and `NpcQuest.SData` as `ps_game.exe` reads them | the files; prints (and imported by `world/import.py`) | |
| `extract_core_packets.py` | made `docs/original/ep6-core-packet-catalogue.md` from the Shaiya-Core headers | the headers; one `.md` | |
| `patch_client_ip.py` | makes a copy of the US client's `game.exe` that connects to a different login server | `game.exe`; the copy | operator |

## The client's data files

`sdata/` (`docs/plans/done/client-patches.md`, `docs/client/sdata-generation.md`): reads the client's `*.SData` files and makes the data patches.

| script | what it does | reads, writes |
|---|---|---|
| `load_client.py` | imports the shipped `Item.SData`, `Skill.SData` and `Monster.SData` into the `client` schema once (an older import gets `client.monsters` added), refusing any other file and recording the import (`--reset`, `--adopt`) | the originals (`CLIENT_ORIGINALS`); PostgreSQL |
| `client_patch.py` | generates the two files from the database alone (the client rows come from the extensions: `content.client_items`, `content.client_skills`), runs the checks of the plan's section 3, writes a patch's hand-over folder and the patch ledger (`restore`, `check`, `generate`, `adopt`, `rebuild`, `record`) | PostgreSQL; `data/client-patches/` |
| `seed_sdata.py`, `parse_item.py`, `client_sdata.py` | the SEED cipher of the files (the tables come from Parsec, github.com/matigramirez/Parsec: `PARSEC_SEEDCONSTANTS`) and the readers of their records | |
| `compare_items.py`, `compare_defs.py`, `compare_cash.py`, `compare_npcquest.py` | made the `docs/client/*-vs-*.md` comparisons of the client's files with `defs` and `world` | TSV files; one report |
| `test_client_patch.py` | tests of the pure parts of `client_patch.py` (no database) | |

## Other

| path | what | who |
|---|---|---|
| `balance/` | the balance model (its own `README.md`): the server's formulas ported, the data, the builds | developer |
| `check_links.py` | checks the Markdown links, the `docs/…`/`tools/…` paths in the text, the section references and the site's nav; exits 1 on any error | developer, CI |
