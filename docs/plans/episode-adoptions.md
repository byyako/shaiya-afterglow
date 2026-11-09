# Episode adoptions: what EP3 and EP5.4 suggest for the new server

## In short

The EP3 and EP5.4 server packs were compared with the EP4.5 original (overview: `docs/original/ep3.md`, `docs/original/ep5.4.md`;
detail: the "Other episodes" section at the end of each system note in `docs/original/`). This plan lists what is worth taking into the
new server, grouped by kind and ranked by value over cost within each group:

- **bugfix**: the later or earlier build fixes something EP4.5 gets wrong, or confirms a fix this server should have.
- **feature**: a new system or mechanic. Under `CONTRIBUTING.md` these are settings or extensions, off by default.
- **perf/arch**: structure worth copying into the Rust design.

Sizes: S (a day or less), M (a few days), L (a chunk of its own). The evidence is the static reading of the binaries and data, nothing
was run. An address names its binary ("EP5.4 ps_game 0x478240"). Nothing here is planned on the roadmap yet.

The five with the best value for their cost:

1. Refuse non-finite positions before they are saved and in every position packet (bugfix, S).
2. Ignore movement steps for a short window after a server move (bugfix, S).
3. Cap each connection's receive queue and log when it is hit (perf/arch, S).
4. Drop guild bank rows with count 0 at load (bugfix, S).
5. Answer the item-mall product list request `0xB106` from the database, in pages of 25 rows (feature, S to M).

## 1. Bugfixes

| # | what | evidence | touches | size |
|---|---|---|---|---|
| 1 | **Refuse non-finite positions.** *Built 2026-11-09: the packets already refused them; `save_position` now keeps the last good position (`docs/server/deviations.md`, R21).* EP5.4 tests each coordinate for NaN before the character save and refuses NaN/inf in the movement and save-point packets. The new server already refuses NaN in `World::is_safe_pos`, but `save_position` binds the `f32` values unchecked, and whether `0x0220` passes through the check was not read. | EP5.4 ps_dbAgent 0x427160 to 0x4271db; EP5.4 ps_game 0x478514, 0x477555; `community-fixes.md` (`nan-pos`) | `crates/game-server/src/pg/character.rs` (`save_position`), `server.rs` (`0x0220`) | S |
| 2 | **Ignore steps for 1.1 s after a server move** (teleport, gatekeeper, map change). Steps the client sent before it saw the move are then not judged against the new position. | EP5.4 ps_game 0x455de0 and 0x4148fb set the deadline; 0x478276 drops such steps | `move_check.rs` (a `blocked_until`), the `0x0501` handler, `move_player` | S |
| 3 | **Drop guild bank rows with count 0 at load** *(built 2026-11-09 in `check_loaded`)* instead of loading an empty stack. EP5.4 deletes them. | EP5.4 ps_game, `game54` social reading of the guild bank load and `0x0B15`; `guild-bank.md`, Other episodes | `guild_bank.rs` (`check_loaded`), the start-up save | S |
| 4 | **No quest start once a follow-up quest is in progress or finished.** EP4.5 allows it. A setting or a deviation. | EP5.4 ps_game 0x48e187 | `quests.rs` | S |
| 5 | **One item-mall request at a time per character, with the purchase number as an idempotency key.** EP5.4's answer to the held-back item-mall points problem. The new server already does points and items in one transaction; the per-character rule is the part to copy. | EP5.4 ps_game item mall; `item-mall.md`, Other episodes | `crates/game-server` mall handler | S |
| 6 | **Bound every counted list the server sends** to what the client's buffer holds. The EP5.4 client trusts the counts of `0x030B`, `0x2609` and `0xB106` (it reads `0xB106` into a buffer for about 64 rows). | EP5.4 client decompilation; `protocol-notes.md`, Other episodes | the encoders in `crates/shaiya-proto` | S |
| 7 | **Seller gold overflow check on a personal-shop sale.** *Checked 2026-11-09: the new server already refuses it (`my_shop.rs`).* Check first whether the new server already caps the seller's gold. | EP5.4 ps_game personal shop; `personal-shop.md`, Other episodes | personal shop buy | S |
| 8 | **Character rename in one transaction.** *Checked 2026-11-09: the new server's rename is one transaction.* Neither original build has an active `BEGIN TRANSACTION` in its rename procedure; confirm the Rust rename commits or rolls back as one. | EP4.5 and EP5.4 `usp_Save_Char_Name_E` | the rename path in `crates/game-server` | S |
| 9 | **A fallback for a failed scripted summon.** EP5.4's `Lua/116.lua` retries the boss summon twice, with a comment calling it a temporary fix. In EP4.5 the floor's second boss can fail to appear when its spot is blocked. | EP5.4 `Lua/116.lua` lines 72 to 77; `instances.md` | `zone/lua.rs` (`MobGenEx`: try again in a wider radius) | S |
| 10 | **The original database files: never restore the EP5.4 `PS_ChatLog` trigger or the EP3 login procedure.** The trigger grants elevated rights on certain chat lines; the EP3 procedure accepts any password. The new server uses neither database, but any tool that restores or attaches original databases for reference should drop triggers. | EP5.4 `PS_ChatLog_Data.MDF`, EP3 `PS_UserData_Data.MDF`; `database-notes.md`, Other episodes | `tools/restore_backups.sh`, docs | S |
| 11 | **A tighter speed window, as a setting.** EP5.4 rewrote the move check: 30-unit steps, each window judged after 6 steps and 3 s against the time, the speed and a margin. `move-check.md` holds a tighter check back for lack of calibration; EP5.4 is a calibration its publisher shipped. Off by default. | EP5.4 ps_game 0x478240 | `move_check.rs`, `config.rs`, `docs/server/settings.md` | M |

Confirmed fixes the new server already has (nothing to build; cite EP5.4 as evidence in `docs/server/deviations.md`): the enchant
material bounds, a remake naming the same item twice, trading with oneself or during a duel or with a shop open, the duel stake count
guard, the party leader index on a kick, the race-faction check at creation, the player put back after a failed speed window.

Corrections to the EP4.5 notes found on the way (recorded in the notes' Other episodes sections): the login key-reply bug is a heap write,
not an over-read; the `0x701` field offsets; the original never stores user questions; the helmet/cloak drop exclusion is EP3 behaviour,
not EP4.5's.

## 2. Features

All off by default. Several need an EP5.4-era client (marked "EP5.4 client"); the patch-108 EP4.5 client never sends those packets.

| # | what | evidence | touches | size |
|---|---|---|---|---|
| 1 | **Answer `0xB106`, the item-mall product list, from the database**, in pages of 25 rows of 76 bytes, once before `0x0201` and again after an operator reload. Prices and names change without a client patch. EP3 and EP5.4 answer it; EP4.5 does not. | EP5.4 ps_game 0x4dbf20, 0x4dbf34; EP3 ps_game; `item-mall.md` | item mall, `crates/shaiya-proto` | S–M |
| 2 | **Hand-over and reconnect windows as settings** (30 s in all three originals; the new server uses 120 s). | EP4.5, EP3, EP5.4 ps_session | `crates/login-server/src/backend.rs`, `config.rs` | S |
| 3 | **Quest experience doubling as a setting** (EP5.4 dropped the doubling below ultimate mode). | EP4.5 ps_game 0x47ee5e, EP5.4 ps_game 0x48de2e | `quests.rs`, `config.rs` | S |
| 4 | **Counted free resets** (3 stat, 3 skill per character, operator-set) and the counted reset item. | EP5.4 ps_game item use, special 70; `item-use.md` | `item_use.rs`, `enter.rs` (`0x021B`), `pg/character.rs` | S |
| 5 | **Staff logins only from listed networks.** | the "Admin IP Check" in the EP3 and EP5.4 login procedure (read from the database files; not checked by a second reader) | `crates/login-server` config and status decision | S |
| 6 | **A per-command "allowed in game" flag for the staff console**, with the results sent to all staff. | EP5.4 ps_game console, `0xF603` | the console command table | S |
| 7 | **Ability types 47 to 49** (double kill count, damage reduction from skills, all stats) and **skill damage types 6 to 11 and 12** (stat-scaled damage, escalating damage over time). | EP5.4 ps_game 0x459705 to 0x459905 and the ability switch; `skills-use.md`, `skills-buffs.md` | skill damage and ability code, the data editor's lists | S each |
| 8 | **Three monster-script functions**: find monsters by type, have one monster use a skill on another, and send scripts a command through an `OnCommand` callback. | EP5.4 ps_game Lua bindings; `scripted-mobs.md` | `zone/lua.rs` | S each |
| 9 | **Multi-item boxes** (`[RandomItemsCreate_n]`, each line rolled on its own) and **box grades above 1024** (EP5.4: 3072). | EP5.4 ps_game 0x4d6df0, 0x473aaa, 0x4d6d82 | the world importer, `drops.rs` (`MAX_GRADE`), `item_use.rs` | S–M |
| 10 | **Link and extraction chance preview** (`0x0809`/`0x080A`). EP5.4 client. | EP5.4 ps_game 0x46f000, 0x46f3d0; the client decompilation agrees on the layout | `lapis.rs`, `crates/shaiya-proto` | S |
| 11 | **Timed server events** (kill count, faction or per-map experience, lapis and enchant rates), with start hour, period and weekday, persisted across restarts. One scheduler serves all of them. | EP5.4 ps_game console and `ServerCmd.ini` | console, a scheduler in `server/tasks.rs`, the formulas | M |
| 12 | **Buff kinds 96 to 98 and debuff kinds 74 to 76** (combo without prerequisite, a shape that dismounts, immunity to state debuffs; a one-at-a-time lock on attacks, skills or items). | EP5.4 ps_game buff code; `skills-buffs.md` | the buff list and its effects | M |
| 13 | **Item-triggered monster summons** (`MobCreate.ini`, special 73), for events. Overlaps the pets extension's summon items. | EP5.4 ps_game 0x4dcf10, 0x4744e8 | a new data table, `item_use.rs`, the zone spawn path | M |
| 14 | **Zone curses** (`MapCurse`: a skill on one country inside a map, with save points refused) and **a skill after a full rebirth**. The shipped data uses neither. | EP5.4 ps_game Map.ini reader, 0x467057 | world import, `zone.rs`, `buffs.rs` | M / S |
| 15 | **Target buff subscription** (`0x0308` to `0x030A` in, `0x030B` to `0x030D` out): buff changes go only to those who selected the target. EP5.4 client. | EP5.4 ps_game 0x456d20; the client decompilation | per-entity watcher sets, `crates/shaiya-proto` | M |
| 16 | **Skill levels up to 9.** The client's skill window must agree. | EP5.4 ps_game skill loader | skill definitions, learning, the defs import | M |
| 17 | **Level cap 70 as a setting.** EP5.4 raised the cap to 70 but left bounds of 80 in places, and its status table has no rows for 61 to 70; copy neither. | EP5.4 ps_game 0x46507d, 0x4651d9 | experience and status tables, level constants | M |
| 18 | **20-slot personal shops.** EP5.4 client. | EP5.4 ps_game personal shop | `personal_shop`, `crates/shaiya-proto` | S |
| 19 | **EP3 behaviours as settings**: the lapis `exp` column as a percent experience bonus (EP3's meaning), and the helmet/cloak drop exclusion. | EP3 ps_game 0x44018b, 0x442133; 0x48763a, 0x44db80 | stats and experience; `drops.rs` | S each |
| 20 | **The level-70 content** (EP5 maps 68 to 72, the relic field, quests, items and monsters). A study source only: the shipped data is a repack with missing monsters and sets, and it needs the EP5 client data. | the EP5.4 `Data/` tree and `PS_GameDefs` | importers, level cap, obelisks | L |
| 21 | **Item-mall points through an external billing service** (EP5.4's PayLetter link through the session server). Only if points ever come from outside. | EP5.4 ps_session | `mall.rs`, a new client module | L |

## 3. Performance and architecture

| # | what | evidence | touches | size |
|---|---|---|---|---|
| 1 | **Cap each connection's receive queue and log when the cap is hit.** EP5.4 drops a connection whose queue passes 100 packets (EP3 and EP4.5: 1000) and logs it. | EP5.4 ps_game 0x474850 | the session read loop or per-tick packet budget | S |
| 2 | **A heartbeat watchdog for long-running tasks.** EP4.5 added (EP3 lacks it) a supervisor that reports a worker thread silent for 15 s once, with a dump. In Rust: a heartbeat per task (world tick, DB writer, sender) checked by a supervisor that logs and raises a metric. | EP4.5 ps_game 0x40532f | `crates/game-server` runtime | S |
| 3 | **Prebuilt packets for static lists, rebuilt on an operator reload** (the item-mall pages), with the feature closed during the reload. | EP5.4 ps_game 0x4dbf20, 0x4dc090 | item mall state behind an `Arc` swapped on reload | S |
| 4 | **A key pool that grows in the background.** The original makes 16 RSA keys at start and adds one at a time up to 1024; the new server makes 8 and stops. | EP4.5 ps_login 0x406af0 (same in EP3) | `crates/login-server/src/keys.rs` | S |
| 5 | **Derive each bound from the loaded table's size**, not from a separate constant (EP5.4 checks levels against 80 on 71-wide tables). | EP5.4 ps_game 0x46515f | every table lookup | S |
| 6 | **Per-map flags computed at load** instead of map-id tests in handlers. | EP5.4 ps_game map config | `world.rs` (a `MapFlags` per map) | S |
| 7 | **A DB queue statistics line** (queue depth, job latency), as EP5.4's DB agent logs. | EP5.4 ps_dbAgent | the backend | S |
| 8 | **A table of outbound requests with ids and deadlines**, for any future call outside PostgreSQL. | EP5.4 ps_session 0x408e40 to 0x409100 | new code only | S |
| 9 | **A referential check of content at import and extension install**: boxes naming missing sets, maps spawning undefined monsters, Lua monsters without a script, maps past the map count. The EP5.4 pack has all of these, and the original loaders drop them silently. | the EP5.4 `Data/` tree and `PS_GameDefs` | `tools/world/import.py`, `crates/data-editor` | M |
| 10 | **A per-character ordered write queue with a periodic save.** The original keeps one queue per user, run by a pool of DB threads, with a 15-minute save timer; the new server writes per change. | EP4.5 and EP5.4 ps_dbAgent | `crates/game-server/src/pg.rs`, `backend.rs` | M |
| 11 | **Hot reload of definitions** from the operator console (an EP4.5 addition over EP3). | EP4.5 ps_game console strings | the defs loader | M |

Not worth copying: the original game log's one statement per record (the new server's batched writer is better), the packet handling
without length checks (unchanged in all three builds), the per-action random reseed in item code.
