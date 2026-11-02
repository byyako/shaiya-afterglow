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

The stored procedures `usp_Read_*_R` (and `*_R2`, `usp_Read_MobMaxID_R`, `usp_Read_SkillMaxID_R`, `usp_Read_WorldInfo_E`) are the
queries `ps_game` almost certainly runs at start-up to load these tables (inferred from the names; not yet confirmed in the binary). They show which columns the server actually uses and in which order.
The `*_R2` variants are probably the newer column set.

## `PS_GameData` (runtime state, the main persistence schema)

Characters (`Chars`, 52 columns, plus `CharBacks`, `_DeletedChars`, `_CreatedChars`, `CharRenameLog`), inventory and skills
(`CharItems`, `CharSkills`, `CharApplySkills`, `CharQuickSlots`, `CharQuests`, `CharEvents`, `CharSavePoint`), account storage
(`UserStoredItems`, `UserStoredMoney`, `UserStoredPointItems`, `UserMaxGrow`), social (`FriendChars`, `Keeps`, `BanChars`), guilds
(`Guilds`, `GuildChars`, `GuildDetails`, `GuildStoredItems`, `GuildNpcLv`, `GuildRankLog`), the market (`Market`, `MarketItems`,
`MarketChar*`), and logs (`ProductLog`, `HackLog`, `PointLog`, `EventLog_*`). 173 procedures, naming convention `usp_<Verb>_<Thing>_<R|E|...>`.

## `PS_UserData` (accounts)

`Users_Master`, `Users_Detail`, `Users_Block`, `Users_Deleted`, `UserLoginStatus` and logs for login, leave, online time, pause, block and
point charges. This is probably where the login server authenticates and where the 16-byte session identity is kept (inferred from the table names).
The password column format (plain, hashed, or the same encryption the ini files use) has not been checked yet.

## Next

- Check the `Users_Master` password column definition and the login procedure to learn how the original authenticated.
- Check `usp_Read_*` against the DB-agent traffic and `ps_game.pdb` types for a cross-check of item and skill column meaning.
- Export `PS_GameDefs` to PostgreSQL (convert CP949 to UTF-8, keep the numeric IDs identical so the unmodified client still matches).

## Field meanings from the community guide **[G]**

Source: the original files' `Docs/GuideToShaiyaConfiguration.pdf` (the configuration guide) (ProfNerwosol, 17 pages, mostly EP4 and later). The whole guide has been read; the
full write-up (all item fields, action types, GM commands and permission tiers, map list) is in `docs/original/community-guide-notes.md`.
These are community findings, not verified against the binaries.

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
python3 tools/defs/load_into_postgres.py data/defs-export db/defs/schema.sql
python3 tools/defs/verify.py db/defs/schema.sql docs/original/db/PS_GameDefs.columns.txt
tools/defs/additions.sh                               # the server's own rows, after the check; ends with `data-editor apply` (the operator's edits)
```

- **The server's own rows** (`tools/defs/additions.sql`): changes to the original definitions that the client's data must carry too (each
  names its client patch); updates only, so it can run again. Now: the three Kill Boost Stones 100/212 to 214 and their buffs 200/1 to 3
  (`docs/original/operator-console.md` decision 3; the client's side is `tools/sdata/client_rows.sql`, `docs/plans/done/client-patches.md`). The load replaces the schema, so run it after every load; `verify.py` compares the
  original, so run that before it.
- **The `edits` schema** (migration `0030_data_edits.sql`, `docs/plans/done/data-editing.md` section 3): the data editor's draft, change sets, log,
  the edits in force (`edits.fields` per row key and field with the value and the original, `edits.rows` for rows added or removed) and
  the map pictures and names of its page. A load of the original files replaces `defs` or `world` but never `edits`; `data-editor apply`
  writes the edits back. Each loaded schema gets a one-row marker table `edits_applied` (`applied_at`, `skipped`) from the first apply
  after a load: a schema without it is fresh, so apply first refreshes every edit's original from the new tables and reports what the
  files changed under an edit, rows that are gone, and added rows whose key the import uses now (those stay unwritten, listed in `skipped`).
  **The order after a load**: `load_into_postgres.py`, `verify.py`, `tools/defs/additions.sh` (ends with `data-editor apply`); for `world`,
  `tools/world/import.py` (ends with it too); then restart the game server.
- **The `client` schema** (`db/client/schema.sql`, created by `tools/sdata/load_client.py`, which replaces it): the client's own
  `Item.SData` and `Skill.SData` as shipped (`client.items`, `client.skills`) and what a data patch changes (`client.item_rows`,
  `client.skill_rows`, `client.server_only`, loaded from `tools/sdata/client_rows.sql` by `tools/sdata/client_patch.py`). Not a migration;
  the server never reads it. `docs/plans/done/client-patches.md` sections 2 and 3.

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
`PS_ChatLog.dbo.ChatLog`. No foreign keys (the log outlives deleted characters); `GAME_LOG_DAYS` deletes old rows, otherwise nothing does.

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
