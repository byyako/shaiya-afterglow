# Client data patches (spec and build plan)

Built (the server side, patch 109); the client side's report is `docs/client/client-patch-0109.md`.

When the server changes a definition the client also holds (an item, a skill), every player's client must get the same values, through
the updater's signed patches, made the same way each time. This plan covers `Item.SData` and `Skill.SData`; the other client data
files follow later the same way (section 8). The background is `docs/client/sdata-generation.md` (the files' layouts, the round trips
proven, the SHA-256 of the originals) and research entry 7 (`docs/client/research-gambling-kill-boost.md`, "Entry 7").

The split: **the server side** (this repo) turns the database into finished, encrypted client files plus a manifest, and proves them.
**The client side** (in the client repository) packs them into the next
`psNNNN.patch`, signs it, and tries it on a spare client before anyone else gets it. The server side never edits the client repo; the
client side never edits the database or `tools/`.

## 1. The maintainer's decisions (2026-10-22)

Since release step R1 the stones are an addition, off by default: `GAME_KILL_BOOST_SKILLS` is empty unless set, and their
server rows exist only where the extension `extensions/kill-boost` is installed (`data-editor ext install`; before 0.3.0:
`tools/defs/additions.sh --with-additions`; `docs/server/deviations.md` section 1). **Since 0.3.0 (extensions chunk 1,
`docs/plans/ext-1-foundation.md`)** the server's rows and the client's rows of an addition both come from an extension's `rows.toml`
into the database, and this tool reads the database only: sections 2 and 4 say how, and where they differ from the text below.

1. **Three kill-boost stones**, not one: the Kill Boost Stone (100/212, skill 200/1) counts each player kill **2×**, a **Lv2** stone
   **5×**, a **Lv3** stone **10×**. The two new items take the next unused slots, **100/213** and **100/214** (the empty "???177" in both
   copies; nothing gives or holds them: no shop, quest, drop grade, mall product, starting item, character, bank, guild bank, auction
   or point store row, checked). They give skill **200 levels 2 and 3**.
2. **One hour each**, kept through death and logout (`fixrange` 1, the hour timer), as level 1. The server's existing rules for levels
   of one buff apply (`buffs.rs` `apply` and `item_skill_refused`): a stronger stone used during a weaker one ends it and starts a fresh
   hour; a weaker one is refused while a stronger one runs; the same one again restarts the hour (it does not add an hour).
3. **Names**: items "Kill Boost Stone", "Kill Boost Stone Lv2", "Kill Boost Stone Lv3"; buffs "Kill Boost", "Kill Boost Lv2",
   "Kill Boost Lv3". **Text** (each item and its buff alike), exactly:
   - 200/1: "Player kills count double for 1 hour. The effect remains after the character dies, and the hour keeps running while logged out."
   - 200/2: "Player kills count 5 times for 1 hour. The effect remains after the character dies, and the hour keeps running while logged out."
   - 200/3: "Player kills count 10 times for 1 hour. The effect remains after the character dies, and the hour keeps running while logged out."
4. **Look**: all three items look like the Mini Sun EXP Stone 100/108 (its model, icon and the two client grouping bytes); all three
   buffs use the icon of its buff 250/1 (icon 225).
5. **No serving yet**: this box does not serve patches for now. The work is to learn to build them and prove them; serving (and the
   updater's URL) is decided later (section 7).
6. **Staging, then live**: a patch is tried on a spare client, fetched through the real updater from a staging folder, before any player
   gets it.
7. Where the client-only fields live and how a new row gets them: decided here (section 2), not asked.

The rest of the kill boost (decision 3 of `docs/original/operator-console.md`) stands: the setting names the skills, the monsters'
level-100 skills of the same ids never count.

## 2. One source for every field

Each byte of a generated record comes from exactly one place:

| What | Where | Edited by |
|---|---|---|
| The server's fields (every column of `defs.items` / `defs.skills`: prices, stacks, levels, effects, durations, the item's skill byte) | `defs.items`, `defs.skills` | the originals, an extension's rows (`data-editor ext install`) and the data editor, in that order |
| The client's own copy of every record, as shipped in US patch 108 (all fields, text included) | `client.items`, `client.skills` (schema `client`) | nobody: imported once from the original files by `tools/sdata/load_client.py` (`--reset` replaces it) |
| The client-only fields of each record a patch changes (name, description; items: model, icon, `type2`, `type3`; skills: icon, animation, toggle byte, sound), the look-alike that gives the rest, and the patch that first shipped it | `content.client_items`, `content.client_skills` | the extension's `rows.toml` (`client = { … }` of an `[[item]]` / `[[skill]]`), written by `ext install`; `since_patch` stamped by `generate` |
| Server changes that need nothing in the client | `content.server_only` | the extension's `server_only = "reason"` |

A client row keeps its look-alike as a reference (`look_type`/`look_typeid`, `look_skill`/`look_level`) and the fields given by name; the
generator resolves each field as: given by name, else the look-alike's record in `client.items` / `client.skills`, else the record's own
original. So an install needs no `client` schema (an operator never imports the client's files), and the resolved values are what the
old `client_rows.sql` wrote (`docs/plans/ext-1-foundation.md` section 4.1). Before 0.3.0 these were `client.item_rows`,
`client.skill_rows`, `client.server_only` and `tools/sdata/client_rows.sql` (deleted).

**A generated record** is the original client record (`client.items` / `client.skills`) **unless** its key is in `client.item_rows` /
`client.skill_rows`; then it is the server's fields from `defs` with the client-only fields from that row. So:

- **A new row** (as the kill-boost stones): the extension's `[[item]]` / `[[skill]]` gives the server's row and, in its `client = {…}`,
  the client row: its own name and text and a look-alike (`look = "100/108"`) that gives model, icon, `type2`, `type3` (items) or
  icon, animation, toggle byte, sound (skills).
- **A listed choice** (shipping the server's value of an existing record, e.g. a price from `docs/client/item-sdata-vs-defs.md`): an
  entry with `client = "keep"`, which takes every client-only field, text included, from the record's own original, so only the
  server's fields change. Each choice is the maintainer's (that file's differences are the original's own).
- The names stay the client's: `defs.items.itemname` is the server's (7,729 differ, some Chinese), never written to the client. For a
  new row, the client name is the one the extension gives.

The original files and anything decrypted from them stay out of git (`reference/`, `data/` are ignored); only the extensions' rows
files (`extensions/*/rows.toml`) and the tools are in git. The originals' copies are in the client repository.

## 3. The checks (`client_patch.py check`; every patch passes them before it is handed over)

1. **Originals**: the imported files are the shipped ones (encrypted SHA-256 `bb1ef0fc…ef48` Item, `0ff8450a…8d92` Skill; the full
   values are in `docs/client/sdata-generation.md`, "The other client data files"). Any other file is refused at import.
2. **Restore**: `client.items` / `client.skills` alone rebuild both decrypted files byte for byte (`efb65232…b0ba`, `a7bca706…c0fa`), and
   encrypting them gives the shipped files byte for byte. This proves the schema holds every byte and the writer and encrypter are right.
3. **Only the listed records change**: the generated files are parsed back (exact end of file, rebuilt byte-identical, as the readers
   in `tools/sdata/` do) and compared with the originals record by record. Every record that differs has a client row; every client row's
   record has the server's fields equal to `defs` (after the width rules below) and its client-only fields equal to the row.
4. **Nothing forgotten**: every `defs.items` row and every `defs.skills` row that differs from the original export (`data/defs-export/
   items.psv`, `skills.psv`, read as `tools/defs/load_into_postgres.py` reads them), i.e. every change `additions.sql` made, is in
   `client.item_rows` / `client.skill_rows` or in `client.server_only` with a reason. Skill rows of level 100 (the monsters' skills,
   kept in the client's `NpcSkill.SData`, not generated yet) must be in `client.server_only` until that file is generated. One
   exception (`docs/plans/done/data-editing.md` section 9): a difference in `grade` alone passes when the item has an `edits.fields` row of
   target `item_grade` (the data editor's drop grades; the client does not use the grade).
5. **Widths**: each server value fits its client field: `u8` fields 0 to 255; `u16` fields −32,768 to 65,535 (the database's `smallint`
   is signed; written as the low 16 bits); `u32` fields 0 to 4,294,967,295. Anything else is an error naming the record, field and value
   (for example an item's skill byte above 255: research entry 7's "wide link" would need its own decision first).
6. **Keys inside the tables**: client rows must name an existing record (item groups 1 to 100 with type id up to the group's count;
   skills 1 to 326, levels 1 to 3), and its `defs` row must exist. Growing a table (a new group, a type id past the count, skill 327) is
   not supported yet and is refused.
7. **Text**: printable ASCII only (0x20 to 0x7E; no NUL, tab or line break), each name and description at most as long as the longest
   of its kind in the same original file (names about 29 bytes in both). The client's limits on its own text are not known beyond what
   it already holds.
8. **Deterministic**: generating twice gives the same bytes.

## 4. What the server side hands over

`client_patch.py generate --patch N` writes `data/client-patches/psNNNN/` (git-ignored; the client-side session reads it through the
share, as it reads `docs/client/`):

- `Item/Item.SData`, `Character/Skill.SData`: the encrypted files, under their member paths in `data.sah` (research entry 7's table;
  the US patch history names the same files `data/item/item.sdata`, `data/character/skill.sdata`). A file identical to the previous
  version is left out.
- `manifest.json`:
  ```json
  {
    "patch": 109,
    "previous": 108,
    "newshaiya_commit": "<git rev-parse HEAD>",
    "files": [
      {
        "member": "Item/Item.SData",
        "us_path": "data/item/item.sdata",
        "size": 2960192,
        "sha256": "<encrypted, as packed>",
        "plain_sha256": "<decrypted>",
        "plain_size": 2960114,
        "replaces_sha256": "<the previous version's encrypted file>",
        "changed": ["100/212", "100/213", "100/214"]
      }
    ]
  }
  ```
  `previous` and `replaces_sha256` come from the patch ledger (`content.client_patches`, `content.client_patch_files`: the newest earlier
  patch that carried the file), else from the 108 originals. Since 0.3.0 the manifest's key is `commit` (patch 109's, written before the
  ledger, is `newshaiya_commit` and names a commit of the old private tree) and it gains `"extensions": {"kill-boost": "1.0.0"}`: the
  installed extensions and versions, the content's provenance. Every patch carries **whole files** holding every change so far, so a client at any version ends with the same
  bytes.
- `changes.txt`: per changed record, each field that differs, old → new (`Item 100/212 ???177 → Kill Boost Stone: icon 1 → 8, range 0 →
  200, …`), for a person to read before signing.

`generate` refuses to run when `tools/sdata/`, `db/client/` or `extensions/` has uncommitted changes (the manifest's commit must describe
the files), and runs every check of section 3 first. It then records the patch in the ledger and stamps `since_patch` on the client rows
that have none, in one transaction (a `--dirty` trial is not recorded).

**The ledger and its commands (0.3.0).** `content.client_patches` has a row per patch (previous, commit, extensions, dirty, the patch
file's and the signature's SHA-256 once packed, status `generated`, `staged`, `live` or `withdrawn`); `content.client_patch_files` has one
per member with its encrypted bytes. `client_patch.py adopt --patch N` records a patch made before the ledger existed (109) from its hand-over
folder, checking every file against its manifest; `rebuild --patch N --out DIR` regenerates the files from the database with only the client
rows whose `since_patch` is at most N and compares each with the ledger: `MATCH`, or the records that differ; `record --patch N …` writes the
client side's results. Undoing a client change is the next patch: after `ext remove` the client rows are gone, so the next generated files
hold the original records again (`docs/plans/ext-1-foundation.md` section 4.3). `--dirty` lets a trial run through and writes `"dirty": true`; the client side
refuses a dirty manifest.

## 5. What the client side does

1. **Verify independently**: the manifest is not dirty; each file's SHA-256 matches; decrypted with the client repo's own SEED code and
   parsed with its own readers (exact end of file); compared with its own extraction of the previous version: **only the records in
   `changed` differ**. Two implementations agreeing is the point.
2. **Pack and sign**: build `psNNNN.patch` with the client repo's tools, holding only the listed members (each replaces the archive
   member of the same path; nothing removed, nothing else added); sign it (`shaiya-patch sign`, Ed25519, the key never leaves the dev
   PC) → `psNNNN.patch.sig`; write the `UpdateVersion.ini` it goes with (`[Version] CheckVersion=3`, `PatchFileVersion=N`). Record the
   SHA-256 of the patch and its signature.
3. **Staging**: a spare copy of the client at the previous version (never the everyday install), whose `UpdaterURL.txt` points at a
   static server on the dev PC over a staging folder with the updater's layout (`Shaiya/UpdateVersion.ini`, `Shaiya/patch/psNNNN.patch`
   and `.sig`). Run the real updater. Then: `Version.ini` `CurrentVersion=N`; the archive's member list before and after differs only in
   the listed members' offsets, sizes and contents; the listed members, extracted, match the manifest's `sha256`; a copy of the patch
   with one byte changed is refused by the updater (signature).
4. **In game** (the spare client, against the dev server; GM `/imake 100 212 1`, `100 213 1`, `100 214 1`): the three stones show their
   names, the Mini Sun EXP Stone's icon and their text; each one used shows its buff with icon 225, its name and text and one hour; the
   live test of section 51 of `docs/testing/live-test-checklist.md` (kill counts) can then run. Spot-check a few unchanged records (100/108,
   a weapon, a potion) look as before.
5. **Report**: `docs/client/client-patch-NNNN.md` in Shaiya Afterglow (what was run, the hashes of the patch and signature, the staging and
   in-game results); the patch, signature and `UpdateVersion.ini` stay in the client repo (or its release folder) until serving starts.
   The server side then records the patch in section 9.

**Later, live** (when serving is set up, section 7): copy `psNNNN.patch` and its `.sig` into the live folder first and change
`UpdateVersion.ini` last, so no client ever sees a version whose file is missing.

## 6. The first patch: 109

- `Item.SData`: 100/212, 100/213, 100/214 → the three stones (server fields from `defs`, look of 100/108, decision 3's text).
- `Skill.SData`: 200 levels 1, 2, 3 → the three buffs (server fields from `defs`, icon 225 with 250/1's animation, toggle and sound).
- Nothing else. Expected `changed`: Item `100/212`, `100/213`, `100/214`; Skill `200/1`, `200/2`, `200/3`.

## 7. Open for later

- **Serving** (decision 5): where (`UpdaterURL.txt` names the host; whether the updater accepts `host:port` is a question for the
  client side), and a start/stop script like `tools/checklist-server.sh` over a git-ignored folder, or port 80 (free on this box).
- **The other files** (`Monster.SData`, `NpcSkill.SData`, `NpcQuest.SData`, `Cash.SData`, …): the same pattern (an import into `client`,
  a rows file, the same checks). `NpcSkill.SData` first if a level-100 skill is ever changed (check 4 will say so).
- **Growing a table** (a new item group, type ids past a group's count, skill 327): writer and checks extended, after a client-side look
  at whether the client's loaders accept it (research entry 7 says the skill loader takes any count).
- **The listed choices** of `docs/client/item-sdata-vs-defs.md` and `skill-monster-sdata-vs-defs.md` (prices, stacks, tooltips): none
  shipped until the maintainer picks them.

## 8. What else the client holds (for planning other edits)

A change needs a client patch only when the client keeps its own copy of the data. As far as the comparisons show:

| Data | Server | Client copy | A change needs a patch? |
|---|---|---|---|
| Items, item skills, player skills | `defs.items`, `defs.skills` 1–3 | `Item.SData`, `Skill.SData` | yes: this plan |
| Monster skills | `defs.skills` level 100 | `NpcSkill.SData` | yes (later) |
| Monsters (name, level, model) | `defs.mobs` | `Monster.SData` | for what the client shows (name, level); not for HP (sent by the server) or behaviour |
| Drops (which monster drops what) | `defs.mob_items`, items' `grade` | none | no |
| Spawns (which monsters where, how many) | `world.mob_areas`, `world.mob_area_mobs` | none | no (the monster's own record must exist in `Monster.SData`) |
| Obelisks (positions, guards, owners) | `world.obelisks`, `world.obelisk_guards` | none known (the client gets them as monsters) | no, unless a new monster |
| Shops, NPCs, quests, gate fares | `world.*` | `NpcQuest.SData` | yes (the shop window fills itself from the client's file) |
| Item mall | `defs.product_list` | `Cash.SData` | yes |
| Maps (terrain, portals, safe zones) | `world.maps`, `world.map_portals` (from the svmaps) | the client's own map files (not studied) | unknown |

## 9. Patches (the ledger; filled when a patch is made)

| Patch | Contents | Files (SHA-256 of the encrypted file) | Patch file and signature | Status |
|---|---|---|---|---|
| 109 | the three kill-boost stones and skill 200 levels 1 to 3 | `Item/Item.SData` `a92292782764918ec1e6b8b929fe66617d340da183ec4664eba46c80ddf47bea` (2960592 bytes); `Character/Skill.SData` `6c5b4bc42032949494efd97a226008c6af9f44df9d52b211c0fc5ca5fb54649c` (222368 bytes) | `ps0109.patch` `9478d66a773db74ac132d8af13a40fb727e6564a9abdf3cb99532e3e2c17bde5` (2,052,649 bytes), `ps0109.patch.sig` `9d27d3f161c3d783ea5e42378d76e73b6edefc99119c92c7508390477f4f9533` (129 bytes) (`docs/client/client-patch-0109.md` section 2) | staged: the real updater installed it on a spare client; the in-game check (death and relog persistence, player-kill increments) is pending. In the ledger since 0.3.0 (`client_patch.py adopt --patch 109`; `rebuild --patch 109` regenerates both files byte for byte from the database) |

## Build

Read sections 1 to 6 first, then `tools/sdata/` (all of it; `client_sdata.py`, `parse_item.py`, `seed_sdata.py` are
reused), `tools/defs/additions.sql` and `additions.sh`, `tools/defs/load_into_postgres.py` (`table_columns`, `decode_text`, the psql
pattern), and the kill-boost code: `config.rs` (`skill_matches`, `kill_boost_skills`), `skills.rs` (`SkillMatch`), `zone.rs`
(`has_applied_skill` and its test `the_kill_boost_finds_an_applied_skill_by_id_and_level`), `server/pvp.rs` (`kill_counted`).

### Steps (in this order)

1. **The server: kill boost levels** (`crates/game-server`; `cargo test -p game-server` green).
   - `skills.rs`: `pub struct KillBoost { pub skill: SkillMatch, pub times: u32 }`.
   - `config.rs`: `kill_boost_skills: Vec<KillBoost>`; `GAME_KILL_BOOST_SKILLS` entries are `id[/level][:times]`, comma-separated;
     `times` 1 to 100, **2 when left out** (today's entries keep their meaning); default **`200/1:2,200/2:5,200/3:10`**. Parse errors
     name the entry (`times` 0 or above 100, a bad number). Unit tests: the default, `200` (any level, 2), `200/2:5`, `310:3`, errors.
   - `zone.rs`: `has_applied_skill(map, id, &[SkillMatch]) -> bool` becomes `kill_boost(map, id, &[KillBoost]) -> u32`: the highest
     `times` among the entries matching an applied skill, **1** when none. Extend the test: levels 1, 2, 3 of one id give 2, 5, 10; a
     player with none gets 1; the monster's level 100 still never counts; two matching entries give the higher.
   - `server/pvp.rs`: `let by = ctx.switches.kill_rate() * boost;` (doc comment: "times the kill boost's multiplier").
   - `tests/game_flow.rs` and anything else building `Config`: the new field.
2. **The server's rows** (`tools/defs/additions.sql`, then `tools/defs/additions.sh` and a game-server restart).
   - After the 200/1 updates: copy **every** column of 200/1 (the list of the first `UPDATE`) to 200/2 and 200/3, then
     `skillname = 'Kill Boost Lv2'` / `'Kill Boost Lv3'`. Skill 200 levels 2 and 3 must stay `keeptime` 1, `fixrange` 1, abilities 0.
   - After the 100/212 updates: copy 100/212's `reqlevel, country, grow, special, slot, quality, buy, sell, grade, drop, server, count`
     to 100/213 and 100/214, then `itemname = 'Kill Boost Stone Lv2'`, `range = 200, attacktime = 2` and `'Kill Boost Stone Lv3'`,
     `range = 200, attacktime = 3`.
   - Extend the comment block (decision 1 of this plan, the multipliers live in the setting) and the closing `DO` check (three skill rows
     of 200 with `typeeffect` 4, `typedetail` 82, `fixrange` 1, `keeptime` 1; three items with range 200 and `attacktime` 1, 2, 3).
   - Run it twice (it must change nothing the second time) and check the rows with psql.
3. **The `client` schema**: `db/client/schema.sql` (not a migration; like `db/defs/schema.sql`, created by the import):
   - `client.items`: `grp smallint, idx smallint` (the record's place: group 1 to 100, index 1 to the group's count), `name text,
     description text`, then one `integer` column per fixed field, named as `parse_item.py`'s `FIX` (`type, typeid, model, icon,
     reqlevel, …, count`), in that order. Primary key `(grp, idx)`. Empty records (type 0) are rows too.
   - `client.skills`: `skill_id smallint` (1-based), `slot smallint` (1 to 3, the record's place), `name text, description text`, then
     one `integer` column per `client_sdata.py` `SKILL_FIELDS` entry (`level, icon, animation, toggle_type, sound, req_level, …`), in that
     order. Primary key `(skill_id, slot)`.
   - `client.item_rows (type smallint, typeid smallint, name text, description text, model smallint, icon smallint, type2 smallint,
     type3 smallint, since_patch integer NOT NULL, note text NOT NULL, PRIMARY KEY (type, typeid))`.
   - `client.skill_rows (skill_id smallint, level smallint, name text, description text, icon integer, animation integer, toggle_type
     smallint, sound integer, since_patch integer NOT NULL, note text NOT NULL, PRIMARY KEY (skill_id, level))`.
   - `client.server_only (kind text CHECK (kind IN ('item', 'skill')), id integer, level smallint NOT NULL DEFAULT 0, reason text NOT
     NULL, PRIMARY KEY (kind, id, level))` (items: `id` = `itemid`, `level` 0).
   - A comment on each table saying what fills it (section 2).
4. **`tools/sdata/load_client.py`** — `usage: load_client.py <Item.SData> <Skill.SData>` (the encrypted originals; default
   the client repository's folder of the originals). Checks the SHA-256 of both (refuses others), decrypts (`seed_sdata.decrypt`, constants from
   Parsec, github.com/matigramirez/Parsec, `src/Parsec/Cryptography/SEEDConstants.cs`), parses (factor `parse_item.py`'s reading into an importable
   function, keeping its command line working; `client_sdata.read_skills`), decodes text as cp1252 checking it encodes back to the same
   bytes, strips the one trailing NUL (refuse a string without exactly one, at the end), then `DROP SCHEMA IF EXISTS client CASCADE`,
   runs `db/client/schema.sql`, and copies both tables in (`\copy … FROM STDIN` CSV, as `load_into_postgres.py`). Prints the row counts
   (16,045 items, 978 skills).
5. **`tools/sdata/client_rows.sql`** — the rows of section 6, in one transaction: `TRUNCATE client.item_rows, client.skill_rows,
   client.server_only;` then the inserts. Items: `INSERT INTO client.item_rows SELECT 100, t, n, d, model, icon, type2, type3, 109, '…'
   FROM client.items, (VALUES (212, 'Kill Boost Stone', …), (213, …), (214, …)) v(t, n, d) WHERE grp = 100 AND idx = 108;`. Skills: the
   same from `client.skills` `skill_id = 250 AND slot = 1` for icon, animation, toggle_type and sound, for levels 1, 2, 3 of 200. The texts
   of decision 3 exactly (names without the trailing NUL). A header comment: what the file is (section 2), one block per server change,
   each naming the `additions.sql` block it goes with.
6. **`tools/sdata/client_patch.py`** — one tool, three commands; each first runs `client_rows.sql` (psql, `ON_ERROR_STOP`):
   - `restore <out dir>`: writes the decrypted and encrypted files from `client.*` alone and checks the four SHA-256 of section 3
     check 2.
   - `check`: every check of section 3, printing one line per check and failing (exit 1) on the first error with the record, field and
     values. The changed-records list and per-field differences (for `changes.txt`) come from the same comparison.
   - `generate --patch N [--dirty]`: refuses uncommitted changes in `tools/defs/additions.sql` and `tools/sdata/` (`git status
     --porcelain`) unless `--dirty`; runs `check`; writes section 4's folder (`data/client-patches/ps%04d/`, refuses an existing one).
   - Reading the database: `COPY (SELECT … ORDER BY …) TO STDOUT WITH CSV` through the container's psql; the `defs` columns by name. The
     field ↔ column maps: items, every `FIX` name is a `defs.items` column except `model, icon, type2, type3` (client-only); skills,
     `SKILL_FIELDS` names without underscores are `defs.skills` columns except `level` → `skilllevel`, `prev_skill` → `prevskillid`,
     `defense_type` → `defencetype`, `defense_value` → `defencevalue`, and the client-only `icon, animation, toggle_type, sound`. Assert at
     start that every other name maps to an existing column (so a typo can't silently drop a field).
   - Writing: `u32` group/skill count, then the records in key order, each `u32 len + text + NUL` twice and the fixed fields packed with the
     readers' formats (the item group counts are the rows per `grp`, as `parse_item.py` rebuilds). Encrypt with `seed_sdata.encrypt`.
7. **Run it**: `load_client.py`, `client_patch.py restore` (all four hashes), `check`, then commit (step 9) and `generate --patch 109`.
   Read `changes.txt`: exactly the six records of section 6; for 100/212 the fields that change are name, description, model, icon,
   country, range, attacktime, sell, grade, count (anything else is a question for the maintainer, not a fix). Also try `check` with a
   deliberately unlisted change (e.g. `UPDATE defs.items SET buy = 1 WHERE itemid = 1001` in a transaction you roll back, or a scratch
   copy of the rows file) to see check 4 fail, and a name of 40 bytes to see check 7 fail; don't commit those.
8. **Docs**: section 9's row for 109 (the two files' SHA-256 from the manifest; status "generated, to the client side"); `README.md`
   (the `GAME_KILL_BOOST_SKILLS` row: the new form and default; the tools list if it has one); `docs/original/operator-console.md`
   decision 3 (the three stones and multipliers, pointing here); `docs/client/sdata-generation.md`, "Getting generated files to the client" (steps 3 to 5 built for
   Item and Skill: this plan); `docs/plans/roadmap.md` step 1b (server side built);
   `docs/testing/live-test-checklist.md` section 51's kill-boost item (the three stones: 2×, 5×, 10× the kill rate; a stronger one replaces a
   weaker, a weaker one is refused while a stronger runs; after patch 109 is installed) and `docs/testing/live-test-solo.md` where it lists 51.
