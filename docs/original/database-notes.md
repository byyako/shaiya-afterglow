# Database notes (restored from `PSM_Data/`)

Structure only; this file holds no player data. Full column/index/procedure lists are in `docs/original/db/`
(`<db>.columns.txt` with type, nullability, defaults and row count; `<db>.indexes.txt`; `<db>.procs.txt`).

## How they were restored

- The 10 backups are SQL Server 2005 RTM (9.0.1399, database version 611, compatibility level 80), taken 2010-03-12 on host `SHAIYA`.
- Collation: `Chinese_PRC_Stroke_CI_AS`. Text columns hold Korean, CP949 bytes read through a Chinese collation, so names will look
  garbled unless re-decoded. Expect to convert when exporting.
- Restored into SQL Server 2022 Developer (Docker image `mcr.microsoft.com/mssql/server:2022-latest`, container `MSSQL_CONTAINER`,
  default `shaiya-afterglow-mssql`, localhost only, data in the Docker volume `MSSQL_VOLUME`, default `shaiya-afterglow-mssql-data`, the SA password
  `MSSQL_SA_PASSWORD` in `.env` (see `.env.example`), memory capped at 2.2 GB). SQL 2022
  accepted the 2005 backups and upgraded them. Script: `tools/restore_backups.sh`.
- Gotcha: the server reports "ready for client connections" well before logins work (system databases are still upgrading).
  On this host that takes a few minutes; early logins fail with "An error occurred while evaluating the password", which is not a
  password problem.
- The container is stopped when not in use (the host is short on memory).

## Contents

| database | tables | procs | rows | notes |
|---|---|---|---|---|
| `PS_GameDefs` | 9 | 51 | 45,558 | static game definitions, the key one |
| `PS_GameData` | 54 | 173 | 18 | characters, items, guilds, market, quests; empty template |
| `PS_UserData` | 16 | 57 | 4 | accounts and login logs; template only, no real accounts |
| `PS_GMTool` | 22 | 152 | 0 | GM tool |
| `PS_Billing` | 16 | 34 | 0 | billing (the session config has billing disabled) |
| `PS_GameLog` | 4 | 43 | 0 | game log |
| `PS_ChatLog` | 2 | 34 | 0 | chat log |
| `PS_StatData` / `PS_Statics` | 8 / 4 | 32 / 58 | 0 | statistics |
| `OMG_GameWEB` | 2 | 35 | 0 | website side |

The only populated database is `PS_GameDefs`; the rest are empty schemas, which is exactly what we want.
`dt_*` procedures and `dtproperties` are SQL Server tooling leftovers, ignore them.

## `PS_GameDefs` (static definitions)

| table | rows | cols | content |
|---|---|---|---|
| `Items` | 16,047 | 50 | every item: type/typeId, name, required level and stats, per-class flags (`Attackfighter`, `Defensefighter`, `Patrolrogue`, `Shootrogue`, `Attackmage`, `Defensemage`), grow, range, attack time, quality, `Effect1-4`, `Const*` bonus stats, buy/sell, drop, count |
| `Mobs` | 2,747 | 70 | monsters: level, exp, AI, money, HP/SP/MP, defense, magic, 15 state resistances, 6 skill resistances, movement/chase timings, three attack slots |
| `MobItems` | 24,723 | 5 | mob drop table |
| `Skills` | 1,333 | 84 | skills: class flags, level, prerequisites, weapon requirements, SP/MP cost, ready/reset time, range, damage type and values, state/attribute effects |
| `ExpDefs` | 240 | 4 | experience per level and grow mode |
| `StatusDefs` | 360 | 6 | base HP/SP/MP per level and job |
| `BaseItemsDefs` | 24 | 9 | starting equipment per family and job |
| `ProductList` | 84 | 52 | item mall products |

The stored procedures `usp_Read_*_R` are the queries `ps_game` runs at start-up to load these tables (confirmed): `InitDB` (0x403f0a)
calls `CGameData::LoadData` (0x4bf320), which runs `LoadStatus`, `LoadExp`, `LoadConstItem`, `LoadBaseItem`, `LoadConstSkill`,
`LoadConstMobSkill`, `LoadConstMob`, `LoadConstMobItem` and `LoadProductList` in that order, each with its `usp_Read_*_R` (the item, mob and
skill loaders also with `usp_Read_MobMaxID_R`, `usp_Read_MobSkillMaxID_R`, `usp_Read_SkillMaxID_R`). They show which columns the server
actually uses and in which order. The `*_R2` variants take an id: they are the console's one-row reloads (`CGameData::ReLoadConst*`, called
only from `CServerApp::OnConsoleCommand`, for example 0x40372e), not a newer column set. `usp_Read_WorldInfo_E` is `ps_dbAgent`'s, not
`ps_game`'s.

## `PS_GameData` (runtime state, the main persistence schema)

Characters (`Chars`, 52 columns, plus `CharBacks`, `_DeletedChars`, `_CreatedChars`, `CharRenameLog`), inventory and skills
(`CharItems`, `CharSkills`, `CharApplySkills`, `CharQuickSlots`, `CharQuests`, `CharEvents`, `CharSavePoint`), account storage
(`UserStoredItems`, `UserStoredMoney`, `UserStoredPointItems`, `UserMaxGrow`), social (`FriendChars`, `Keeps`, `BanChars`), guilds
(`Guilds`, `GuildChars`, `GuildDetails`, `GuildStoredItems`, `GuildNpcLv`, `GuildRankLog`), the market (`Market`, `MarketItems`,
`MarketChar*`), and logs (`ProductLog`, `HackLog`, `PointLog`, `EventLog_*`). 173 procedures, naming convention `usp_<Verb>_<Thing>_<R|E|...>`.

## `PS_UserData` (accounts)

`Users_Master`, `Users_Detail`, `Users_Block`, `Users_Deleted`, `UserLoginStatus` and logs for login, leave, online time, pause, block and
point charges. This is where the login server authenticates (confirmed: `ps_login.ini` names `PS_UserData`, and `GetUserMD5`, ps_login
0x406e50, runs `usp_try_gamelogin_korea` there). The session identity kept in it is the **64-bit** session id the login server passes
(`%I64d`; `UserLoginLog.SessionID`, `UserLoginStatus.LoginSession`, both `bigint`); the 16-byte identity goes to the session server in
`0x701` ([`ps_login.md`](ps_login.md) section 6), not to the database. What the unshipped `_korea` procedure writes is not known; the
shipped `_Taiwan` one logs the session id.
The password column format (plain, hashed, or the same encryption the ini files use) has not been checked yet.

## Next

- Check the `Users_Master` password column definition and the login procedure to learn how the original authenticated.
- Check `usp_Read_*` against the DB-agent traffic and `ps_game.pdb` types for a cross-check of item and skill column meaning.
- Export `PS_GameDefs` to PostgreSQL (convert CP949 to UTF-8, keep the numeric IDs identical so the unmodified client still matches).

## Field meanings from the community guide **[G]**

Source: the original files' `Docs/GuideToShaiyaConfiguration.pdf` (the configuration guide) (ProfNerwosol, 17 pages, mostly EP4 and later). The whole guide has been read; the
full write-up (all item fields, action types, GM commands and permission tiers, map list) is in `docs/original/community-guide-notes.md`.
These are community findings, not verified against the binaries here (not worth it on this page: each needs its column traced through
`CGameData::LoadConstItem` 0x4c6420, which reads the columns into locals before filling `stItemInfo`; [`lapis.md`](lapis.md) confirms
`reqluc`, `reqvg` and `effect1` to `effect4` by their uses).

- `Items.Reqluc` (smallint): the item ID used to enhance the item. IDs above 32767 are stored minus 65536 (for example 44068 is -21468).
- `Items.ReqDex`: item name colour (0 white, 1 worship, 2 dread, 3 legendary, 4 goddess, 5 special, 7 consumable). Also used for sorting.
- `Items.Effect1` / `Effect2`: minimum damage and the extra added for maximum damage. `Effect3` / `Effect4`: defence and resistance of armour.
- `Items.ReqVg`: the chance for the linking hammer.
- `Items.ReqOg` (also in `Item.SData`): 1 means the item cannot be traded or dropped.
- Database values override `Item.SData` for stats; `Item.SData` text only changes what the client shows.
- `PS_GameData.CharItems.CraftName`: ten two-digit values (str, dex, rec, int, wis, luc, HP, MP, SP, lapisia). HP/MP/SP count in hundreds. Weapon
  enhancement runs 1 to 20, armour 50 to 70. (In EP3 this field held the name of the player who linked the lapis.)
- `Users_Master.Status`: see `docs/original/ps_login.md` sections 12 and 15.
- Item appearance comes from `<Type>.ITM` files (model/texture pairs); mob and map data are in `ItemCreate.ini` (drop sets by item `Grade`), `Map.ini` and so on.

## Definitions loaded into Postgres (schema `defs`)

`PS_GameDefs` now lives in the project's PostgreSQL as schema `defs` (tables `items`, `mobs`, `mob_items`, `skills`, `exp_defs`, `status_defs`,
`base_items_defs`, `product_list`; 45,558 rows). Column names are the original ones, lowercased; the original `RowID` is kept; primary keys are
`items.itemid`, `mobs.mobid` and `product_list.productcode`. The schema is `db/defs/schema.sql` (generated, committed). The data is proprietary and stays in
the database and in git-ignored `data/`.

Reproduce (needs the SQL Server and PostgreSQL containers, `MSSQL_CONTAINER` and `PG_CONTAINER`, defaults `shaiya-afterglow-mssql` and `shaiya-afterglow-postgres`):

```
python3 tools/defs/generate.py docs/original/db/PS_GameDefs.columns.txt db/defs/schema.sql tools/defs/export_queries.tsv
tools/defs/export_from_mssql.sh                       # bcp export, 1 second
python3 tools/defs/load_into_postgres.py data/defs-export db/defs/schema.sql   # once: the frozen copy original_defs, then `data-editor content rebuild defs`
python3 tools/defs/verify.py db/defs/schema.sql docs/original/db/PS_GameDefs.columns.txt   # compares original_defs with the original database
```

- **The data is imported once, into frozen copies.** (Since 0.3.0; `docs/plans/ext-1-foundation.md` section 3.) The importers write the
  original data exactly as it came into `original_defs`, `original_world` and (a developer's box only) `client`, and record in `content.imports`
  what they read: the SHA-256 of every file, the rows and md5 of every table, and where it came from. A second run with **the same files
  does nothing** ("already imported on …; nothing to do"); with **other files it is refused**, naming the files that differ; `--reset`
  (`tools/setup.sh --reset`, or the importer's own flag) replaces the frozen copy and rebuilds the live tables. `--adopt` records a copy loaded
  from the same files on a box whose live tables already hold them (the dev box; not for operators).
- **The live tables are built, not loaded.** `defs` and `world` are what the servers read; `data-editor content rebuild [defs|world|all]`
  builds them in one transaction as **originals → installed extensions → edits**: the frozen tables are copied in, the rows of every installed
  extension (`content.ext_rows`, enabled or not) are written over them, then the data editor's edits are applied (below). A database from
  before 0.3.0 (a `defs` schema with no import record) is imported by running `tools/setup.sh` again with the same two folders; changes made by
  hand in SQL are replaced, as on any re-run before. `data-editor content check` proves the live tables equal such a rebuild (every difference
  is printed as `defs.items 100212: hand edit? itemname live 'X', expected 'Y'`), `content status` says what is imported and installed,
  `content hashes` prints `schema.table|rows|md5` per table (`tools/content/table_hashes.sql` does the same in psql: the proof that a change
  wrote no byte), and `content origin <table> <key>` prints the layers of one row.
- **Schemas of the game data:** `original_defs`, `original_world` (the originals, written by the importers only; same DDL as `defs` and
  `world`), `client` (the client's own files, a developer's box), `defs` and `world` (live), `content` (migration 0031: imports, installed
  extensions with the rows, ids, settings and client rows each owns, the client-patch ledger, the editor's settings layer), `edits`, and one
  `ext_<name>` per installed extension that has migrations. Which origin a row has is in `content.ext_rows` (an extension owns it) and in
  `edits.fields` / `edits.rows` (an edit changed it), not in a column.
- **The server's own rows are an extension.** The three Kill Boost Stones 100/212 to 214 and their buffs 200/1 to 3 (once
  `additions.sql`, now `../shaiya-afterglow-extensions/kill-boost`, installed with `data-editor ext install ../shaiya-afterglow-extensions/kill-boost`) change rows that
  exist in the originals as empty placeholders; they are not in the originals, so an import never brings them back, and an install or a
  rebuild writes them (`docs/original/operator-console.md` decision 3, `docs/plans/done/client-patches.md`).
- **The `edits` schema** (migration `0030_data_edits.sql`, `docs/plans/done/data-editing.md` section 3): the data editor's draft, change sets, log,
  the edits in force (`edits.fields` per row key and field with the value and the original, `edits.rows` for rows added or removed) and
  the map pictures and names of its page. A load of the original files replaces `defs` or `world` but never `edits`; `data-editor apply`
  writes the edits back. Each loaded schema gets a one-row marker table `edits_applied` (`applied_at`, `skipped`) from the first apply
  after a load: a schema without it is fresh, so apply first refreshes every edit's original from the new tables and reports what the
  files changed under an edit, rows that are gone, and added rows whose key the import uses now (those stay unwritten, listed in `skipped`).
  **The order after an import** is `load_into_postgres.py`, `verify.py` (counts, or the full check with SQL Server), then for `world`
  `tools/world/import.py`; each ends with `data-editor content rebuild` for its part (which applies the edits last); then restart the game
  server. `tools/setup.sh` does all of it and is safe to run again.
- **The `client` schema** (`db/client/schema.sql`, created by `tools/sdata/load_client.py`, once): the client's own `Item.SData` and
  `Skill.SData` as shipped (`client.items`, `client.skills`). What a data patch changes is in `content.client_items`,
  `content.client_skills`, `content.server_only` and the patch ledger `content.client_patches` / `client_patch_files`, written by
  extensions and by `tools/sdata/client_patch.py`. Not a migration; the server never reads it. `docs/plans/done/client-patches.md` sections 2 and 3.
- **Rollback of a change to the game data** (the 0.3.0 upgrade of a dev box, `docs/plans/ext-1-foundation.md` section 9): take a dump of the
  whole database first (`pg_dump -Fc` to `data/backups/pre-ext1-<date>.dump`) and the table hashes (`tools/content/table_hashes.sql` to
  `…hashes`); rehearse on a copy restored from the dump; to give up before the install, drop the schemas `content`, `original_defs` and
  `original_world` and delete migration 31's row from `_sqlx_migrations`, then run the previous binaries; after the install, stop the game
  server, `pg_restore --clean --if-exists` the dump and run the previous binaries.

- **Text encoding.** The original stores GBK (Chinese code page 936) bytes in `varchar` columns. Text is exported as hex, decoded as GBK, and every value is
  checked to re-encode to the identical bytes before loading. SQL Server's own conversion would have mangled it.
- **Verification.** 288 checks compare row counts, the sum of every numeric column, the byte length of every text column and every timestamp between the two
  databases; all match. Do not use `sqlcmd` for bulk export (it needed minutes for 1 MB); `bcp` is the right tool.
- **What the data is.** This is the **Chinese** localisation: mob and many item names are Chinese (for example mob 100 is 野生山羊, "wild goat"), mixed
  with English ones. About 30% of names are `???` placeholders (4,907 of 16,047 items, 980 of 2,747 mobs): the original text was already lost before the
  backup was taken, so it cannot be recovered. Unused IDs exist as placeholder rows (mob 102 has level 0 and 0 HP). All rows of `skills` were last updated
  2009-10-26, which dates the snapshot. The client shows its own text from `Item.SData`, so names matter here for logs and GM tools only.
- **Cross-check with the community guide.** Item 44068 has type 44 / typeid 68 as the guide says, and `reqluc = -21468` (44068 - 65536) is the enhancement-rune
  reference on a family of armours, as described.
- **Risk to check later:** these definitions are the Chinese 2009 set, the client is the US 2010 build. IDs usually agree but have not been compared with the
  client's `Item.SData` yet, which needs `data.saf` to be read.

## The game log (`game_log`, `chat_log`)

The new servers write what the original's `ps_game` sent to its log server (`docs/original/game-log.md`): `game_log` is `PS_GameLog.dbo.ActionLog` (the
same ActionType numbers in `action`, the same placement of values and texts: `value1` to `value10`, `text1` to `text4`), `chat_log` is
`PS_ChatLog.dbo.ChatLog`. No foreign keys (the log outlives deleted characters); `log_days` deletes old rows, otherwise nothing does.

**Following an item.** Every item record names the row as stored: `value1` is `character_items.uid` (the same number in `bank_items`,
`guild_bank_items` and the market's rows while the item is in them), `value2` the item id, `text1` its name. A row keeps its `uid` while it
changes hands whole, goes to the warehouse, the guild bank or the auction house and comes back; part of a stack that changes hands is a new
row with a new `uid`. So an item's history is

```sql
SELECT at, character_name, action, value1, value2, value3, text2, text3 FROM game_log WHERE value1 = <uid> ORDER BY at, id;
```

and a duplicate shows as a `uid` (or a count) that appears without the record that made it: a new `uid` has an action 111 with `text2`
`'ItemCreate'` (made by the server), 113 (bought), 115 (traded), 118/111 (picked up), 213 and so on. The gem records (119, 120, 212) hold the
lapis's item id and unique id in `value3` and `value4`. Two rows with one `uid` cannot exist (`character_items.uid` is unique).

## Reading a backup without SQL Server

`tools/defs/read_bak.py` reads `PS_GameDefs` straight out of the backup file; `tools/setup.sh` uses it, so the import needs no SQL Server.
Its output was compared with the SQL Server export, table by table, and is byte for byte the same for all eight exported tables
(`tools/defs/export_queries.tsv`). What it relies on, found by reading the file:

- The backup is a Microsoft Tape Format file (it starts with `TAPE`) holding the database's 8 KB pages, uncompressed. The first page, the
  file header page (type 15, page 0), is at offset 6656 in this backup; the pages follow in 8192-byte steps, the extents the backup
  skipped are zero pages, and the page numbers come from the page headers, not from the position.
- These pages have torn-page detection on: the low two bits of the last byte of sectors 1 to 15 of each page are replaced by a
  marker, and the real bits are kept in the 32-bit field at offset 60 of the page header (two bits per sector). Without putting them
  back, every 512-byte sector has one damaged byte (a string character, a slot offset). A page with checksums (flag 0x200) would not
  need this; the flag 0x100 says which.
- The table list and columns are in the database's own catalogue: `sysschobjs` (object 34: id, name, type `U ` for a user table),
  `syscolpars` (object 41: column id, type, length, name), and where each column sits in the record in `sysrowsets` (object 5, rowset
  to table) and `syshobtcolumns` (object 13, the 16-bit leaf offset at byte 27: a positive offset is a fixed column's position from
  the start of the record, `-1`, `-2`, ... is the variable column's slot).
- Records are the usual SQL Server 2005 layout: status bytes, the fixed part, the column count, the null bitmap, the variable column
  count with the end offsets, the variable data. Some tables carry an extra variable slot ahead of the real one (`Items`: slot 1 holds
  the name, slot 0 something left from an earlier column), which only the leaf offsets resolve; reading variable columns in order
  gets the wrong text.
- Only the types the definitions use are read (`tinyint`, `smallint`, `int`, `bigint`, `bit`, `datetime`, `char` and `varchar`,
  `nchar`, `nvarchar`, a few more); another type stops the reader with a message. Text is passed on as bytes (hex in the export);
  the GBK decoding stays in `load_into_postgres.py`.

## Other episodes

The databases shipped with the EP3 and EP5.4 server packs were compared with the EP4.5 backups, read statically like the rest of this
note; overview in [`ep3.md`](ep3.md) and [`ep5.4.md`](ep5.4.md). Their description files are in `docs/original/db/ep3/` and
`docs/original/db/ep5.4/`, which `docs/original/db/README.md` explains.

**Versions** (confirmed, boot pages): all ten EP3 `.MDF` files and nine of the ten EP5.4 ones are database version 539 (SQL Server
2000), which SQL Server 2022 refuses to attach (Msg 950). They were read with a static reader of the SQL Server 2000 catalogue; for the
seven EP5.4 databases whose schema equals EP4.5's, its output is byte for byte what SQL Server 2022 wrote for EP4.5 (confirmed). EP5.4
`PS_GameDefs` is version 661 (SQL Server 2008 R2, created on 2000) and was attached normally. EP3 has `PS_Monitor` and `PS_Statistics`
where EP4.5 has `PS_StatData` and `PS_Statics`.

### EP3

- A thinner set, made 2009-01 to 2009-04: no primary keys or defaults on most tables, different name case (`CHARS`, `charitems`),
  `PS_GameData` with 27 tables against 54 (no market, guild bank, NPC levels, `Guilds`/`GuildDetails`), `PS_Billing` with 3 against 16
  (confirmed). `OMG_GameWeb` and `PS_Monitor` are empty.
- `PS_GameDefs` is a stub: one placeholder column per table and no rows, so there are no EP3 definitions to export (confirmed).
- `PS_UserData`: `Users_Master.Pw` is varchar(32) (EP4.5 12); extra procedures compare or change passwords in plain text (confirmed).
- `PS_GameLog` has only `UserLog`; the action-log insert is in `PS_GameData` (confirmed).
- The item-mall delivery procedure `usp_Insert_Product_User_E` is in clear text in EP3 (encrypted in EP4.5 and EP5.4): at most 240 slots
  per user, one row per item, one transaction (confirmed).

### EP5.4

- The EP4.5 schema, older in places (confirmed). `PS_Billing`, `PS_GMTool`, `PS_StatData` and `PS_Statics` are identical, procedures
  included. `PS_UserData` lacks `CreatedGuilds`, `UserLoginStatus`, `UserOnlineTimeLog`, `WebPointLog` and two procedures, all created in
  the EP4.5 backup after the EP5.4 copy's date; so the play-time point grant of EP4.5's `usp_Try_GameLogout_R` is a 2009 addition to the
  EP4.5 pack.
- `PS_GameData`: `Chars.K1` defaults to 0 (EP4.5 120000); `usp_Read_User_MaxGrow_R` returns the stored value where EP4.5 returns a
  constant 3, and EP5.4 stores 3 at country choice instead; the cash-point read goes through a linked server; `usp_Save_Char_Name_E`
  has its commit commented out, and since `BEGIN TRANSACTION` is commented out in both builds, neither renames atomically (EP4.5's lone
  `COMMIT` fails unless the caller holds a transaction: the caller holds none, since ps_dbAgent's `DBCharacter::RenameCharacter`
  (0x41d6a0) runs it on a pool connection in autocommit, which `SDatabase::Connect` sets (0x43ef08) and only `BeginTrans` (0x43e730) turns
  off, and the rename never calls it (confirmed); that SQL Server then raises its "no corresponding BEGIN TRANSACTION" error after the updates
  have been written is its documented behaviour, not run here. The EP4.5 backup holds two texts of the procedure, one with that `COMMIT`
  and one with it commented out; which is the current one was not settled).
- `PS_GameDefs`: `ProductList.RowID` became `IDENTITY`; more rows (items 16,047 → 19,572, skills 1,333 → 3,738, monsters 2,747 → 3,088).
- `PS_ChatLog.ChatLog.ChatData` is char(100) (EP4.5 char(10)).
- The EP5.4 `ps_dbAgent` calls four item-mall procedures that no shipped backup has (`usp_Save_User_BuyPointItems2`,
  `usp_Read_User_CashPoint_UsersMaster`, `usp_Save_User_GiftPointItems2`, `usp_Rollback_User_GiftPointItems`), so that pack's item mall
  cannot have worked with its own databases (confirmed: the purchase save `SaveBuyPointItem`, EP5.4 ps_dbAgent 0x422c50, is its only
  purchase statement, 0x422e7d, and no procedure list of the three packs has the four). `usp_Rollback_User_GiftPointItems` runs when billing refuses a gift and the target account is not loaded in the agent (`0x0E09`, EP5.4 ps_dbAgent 0x407f71 → 0x423b90; confirmed).

### What operators should know

- EP3's `usp_Try_GameLogin_Taiwan` accepts any password for an existing account, and EP3's `ps_login` calls it (confirmed). Never run
  an EP3 `PS_UserData` with real accounts.
- EP5.4's `PS_ChatLog` ships a trigger on its chat table that grants elevated rights on certain chat lines; the game log server writes
  every chat line there. An operator restoring that database should drop the trigger (and keep `xp_cmdshell` off) (confirmed from the
  trigger text).
- In every episode, the servers write text into SQL statements unescaped (`sprintf` with `'%s'`); the only change is EP5.4 quoting
  floats in one save statement (confirmed, string comparison).
