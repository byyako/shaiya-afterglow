# Build plan: operator (GM) commands, part 1

Built; the behaviour is described in `docs/original/gm-commands.md`.

Spec: `docs/original/gm-commands.md`. Read it first (sections 2 to 8
are the rules, section 13 the new server and its eighteen deviations), then `docs/client/gm-and-commands.md` ("In short", "Summary", "How a GM command travels"; what the
client sends and shows; read only, CRLF, never edit it) and the code named below. Build all eighteen deviations of the spec's
section 13 as written; the maintainer approved them (2026-10-19).

Names used below: **GM** = the staff member sending a command; **level** = the staff level (`Player::privilege`, 1 to 10 staff, 11 player);
**target** = the player a command names; **order** = a change the GM's connection asks the target's connection to carry out.

## Design in one paragraph

The GM's connection parses and checks each `Fxxx` packet (`server/admin.rs`, with the pure rules in `crate::admin`). What touches only the GM
(its flags, its moves, items into its own bags, monsters and NPCs around it, chat, guild settings, the accept flag) is done there. What
changes another player's character (`/set`, `/apower`, `/recallitem`, bans) becomes an **order**: the GM pushes it into
`Zones::admin_orders` for the target's character id and wakes the target with `ZoneEvent::Admin` (`ZoneEvent` is `Copy`: the order itself
stays in the map). The target's connection takes its orders in its event loop, applies each one to its own `PlayCharacter`/`WorldState`,
saves, sends its packets, and sends the GM's `F501` through the zone (`Zones::send_to_char`). A GM naming itself goes the same way (its own
event channel), so nothing waits on anything.

## Steps (in this order; `cargo test -p game-server` and `cargo test -p shaiya-proto` green after each)

1. **Protocol** (new `crates/shaiya-proto/src/admin.rs`; `pub mod admin;` in `lib.rs`; tests at the bottom of the file).
   - Constants for every opcode of the spec's section 9 (`OP_ANSWER = 0xF501`, `OP_CHAT_NORMAL = 0xF101` … `OP_CHAT_PARTY = 0xF105`,
     `OP_SET_LEVEL = 0xF601`, `OP_CLEAR_LEVEL = 0xF602`, `OP_SET = 0xF701` … `OP_REMOVE_TARGETED = 0xF70E`, `OP_INVISIBLE = 0xFA01` …
     `OP_AUCTION_SEARCH = 0xFA1A`), and `pub fn family(opcode: u16) -> Option<u8>`: the level limit by the high byte (`0xF1`, `0xF2` → 10,
     `0xF6` → 1, `0xF7` → 2, `0xF8` → 3, `0xF9` → 4, `0xFA` → 5, else `None`).
   - `pub fn answer(value: u16) -> Vec<u8>` (`F501`, 4 bytes). `pub fn name_field(bytes: &[u8]) -> Vec<u8>`: the name up to the first zero
     within the field (21 or 25 bytes), as the original's terminating write.
   - Parsers for deviations 14 to 17 too: `parse_ban` (23) serves `F70A`/`F805` as well, `parse_name` `F70B`/`F806`, `parse_quest`
     (`F809`/`F80A`, 2: `u16 quest`), `parse_event` (`F80F`/`F810`, 42: `name[21], event[21]`; the event 1 to 20 bytes after
     `name_field`, else `None`). Builder `ban_notice(op, minutes)` (`F70A`/`F805 u16 minutes`, 4 bytes).
   - Parsers, each `Option<…>` with the exact payload length (payload = after the opcode): `parse_set_level` (**23 bytes: `name[21],
     u8 level`**, the client's layout, deviation 2), `parse_name` (21: `F602`, `F709`, `FA08`, `FA09`, `FA0E`, `FA10`), `parse_set` (26:
     `u8 stat, u32 value, name[21]`), `parse_make_item` (3), `parse_recall_item` (23: `name[21], u8 type, u8 type id`), `parse_mob` (3:
     `u16 mob, u8 count`), `parse_mob_id` (4), `parse_npc` (4: `u8 type, u16 type id, u8 count`), `parse_named_text` (`FA07`: `name[21], u8
     len, text`, len 2 to 128 and the payload exactly `22 + len`), `parse_ban` (23: `u16 minutes, name[21]`), `parse_go` (`FA0A`, 10: `f32
     x, f32 z, u16 map`), `parse_map` (`FA11`, 2), `parse_guild_go` (`FA12`, 35: `f32 x, f32 z, u16 map, name[25]`), `parse_guild_grade`
     (`FA13`, 47), `parse_guild_rank` (`FA14`, 26), `parse_guild_points` (`FA15`, 29), `parse_call` (`FA18`, 1). Chat: reuse
     `chat::parse_message` and `chat::parse_whisper` (same layouts as `0x1101`/`0x1102`).
   - Builders: `set_answer(stat: u8, value: u32)` (`F701`, 7), `level_answer(level)` (`F601`/`F602`, 3: pass the opcode), `bare(op)`
     (2 bytes: `F802`-style `FA0D`/`FA0E`/`FA0F`/`FA10`), `warning(text)` (`FA07 u8 len, text`), `watch(map, [f32; 3])` (`FA08`, 16). GM chat
     reuses `chat::by_id` (`F101`, `F105`), `chat::by_name` (`F103`, `F104`) and `chat::whisper` with the `F102` opcode: add an
     `opcode` parameter to `chat::whisper` (~line 109) or a `whisper_op(op, to_sender, name, text)` next to it.
   - Tests: every parser's length check (one byte short and long → `None`), `F601` in the client's layout (`b"TestVail\0…"` then 3 → level 3
     for "TestVail"), `family` for each family and `0xF3xx` → `None`, `answer(0xF701)` = `01 F5 01 F7`, `set_answer` is 7 bytes,
     `watch` 16.

2. **The rules** (new `crates/game-server/src/admin.rs`, pure, unit-tested; `pub mod admin;` in `lib.rs`).
   - `pub enum Check { Allowed, Refused, Close }` and `pub fn check(level: u8, opcode: u16) -> Check`: a player (level ≥ 11) → `Close` for any
     `Fxxx`; staff above the family's limit, or an opcode of the spec's deviation 1 list (`F70A`, `F70B`, `FA03`, `FA04`, `FA16`, `FA17`,
     `F2xx`, `F8xx` except `F805`, `F806`, `F809`, `F80A`, `F80F`, `F810`, `F9xx`, `FA03`, `FA04`, `FA0B`, `FA0C`, `FA19`, `FA1A`, `F106` to
     `F109`, any `F1xx` above `F10A`, any other `Fxxx` family) → `Refused` (`F501 opcode`); **`FA16`/`FA17` are allowed for every staff level
     1 to 10** (deviation 15); else `Allowed`. `F70A`/`F70B` follow the `F7xx` limit (2), the six `F8xx` ones the `F8xx` limit (3).
   - `pub fn status_of(level: u8) -> i16`: `level * 16` for 1 to 5, else 0. `pub fn may_set_level(level: u8) -> bool`: 1 to 11.
   - `/set` (spec 7.1 with deviations 3 and 4): `pub enum Stat { Grow, Level, Money, StatPoint, SkillPoint, Str, Dex, Rec, Int, Luc, Wis,
     Honor, Vg, Cg, Og, Ig, Exp, Kills, Deaths }` with `Stat::from_u8` (0 to 18). `pub struct Sheet { grow: u8, level: u16, exp: u32,
     money: u32, stat_point: u16, skill_point: u16, stats: [u16; 6] (str, dex, rec, int, wis, luc as stored), honor: u16, vg: i16, cg: u8,
     og: u8, ig: u8, kills: u32, deaths: u32 }` and `pub fn apply_set(sheet: &Sheet, stat: Stat, value: u32, exp: &ExpTable, kill_cap: u32)
     -> Option<SetResult>` returning the new sheet, the value to answer, whether the level changed and the `F701` extras (`F701 16 exp`
     for a level set; the job's main stat for the **target's** job: fighter STR, defender REC, ranger DEX, archer LUC, mage INT, priest WIS,
     with stat numbers 5, 7, 6, 9, 8, 10 as the original's table). Limits exactly as the spec's table; CG/OG/IG ≤ 255 (deviation 4); exp →
     the level whose range holds it (`ExpTable::bounds`, `crates/game-server/src/enter.rs` ~line 43),
     a value beyond the mode's highest level's start → that highest level (`combat::max_level`, deviation 4), above the mode's cap →
     refused; a level set → exp = the new level's start. Note the stored stat order is str, dex, rec, int, wis, luc (`PlayCharacter::stats`)
     while `/set` numbers them STR 5, DEX 6, REC 7, INT 8, LUC 9, WIS 10.
   - `pub fn gm_zone_allowed(create_type: u8) -> bool` for `FA12` (3, 5, 6) and `pub fn amove_allowed(create_type: u8) -> bool` (0 only).
   - `pub fn call_targets(...)` for `FA18`: given the GM `(id, position)`, `country` and the players `(id, level, country, position)`, the ids
     to call (level ≥ 11, not the GM, that country, x/z distance > 10) and a delay each of `1000 + rand % 10000` ms.
   - `pub enum Until { Forever, At(Instant) }` and `pub fn ban_until(minutes: u16, now: Instant) -> Until` (0 → `Forever`).
   - Unit tests: `check` for a player (`Close`), level 2 with `F601` (`Refused`), level 1 with `F601` (`Allowed`), level 5 with `F702`
     (`Refused`), level 5 with `FA16` (`Refused`); `status_of` 1 → 16, 5 → 80, 6 → 0, 11 → 0; every `/set` limit (grow 4, grow 0 at level 31,
     level 61, level 31 in easy, same level, stat 0x7000, same stat, HG 0x10000, CG 256, exp above cap) refused; a level set from 30 to 35
     gives exp = level 35's start and keeps the points; exp inside level 40's range sets level 40; exp past level 60's start sets 60; the main
     stat of each job; `ban_until(0)` is `Forever`; `call_targets` skips staff, the GM, the other country and anyone within 10.

3. **State in the zone** (`crates/game-server/src/zone.rs`, `zone/npc.rs`, `zone/pvp.rs`).
   - `Occupant` (~line 370): `pub visible: bool`, `pub attackable: bool`. Where the occupant is built for entering (server.rs ~line 1393,
     `staff: player.privilege <= enter::STAFF_PRIVILEGE_MAX`): both `!staff` (spec section 4: staff enter invisible and untouchable). Every
     other `Occupant { … }` (tests, ~line 3598) gets `true, true`.
   - The shape: `Placed::form_packet` (~line 434) also sends when `!visible`, and the shape packet gives **13** while `!visible` (the
     original's `GetShape`); a visible illusion or mount stays as now.
   - `Occupant` also gets `pub pvp: bool` (deviation 15: `!staff` on entering). Players' attacks and skills on a player (`may_attack_player`
     and the player areas below) need the target's `visible && pvp`; monsters, guards and the Lua/FSM searches keep `visible && attackable`.
   - `pub fn set_gm_flags(&self, map: u16, id: u32, visible: Option<bool>, attackable: Option<bool>, pvp: Option<bool>) -> Vec<Vec<u8>>`: sets
     them (`FA05`/`FA06` pass the same value for `attackable` and `pvp`, `FA16`/`FA17` only `pvp`); a visibility
     change sends the shape (`0x051D` with 13, or the real shape packet) to the view and returns it for the GM.
   - Targets: `targetable` (~line 657) adds `p.occupant.visible && p.occupant.attackable`; `zone/npc.rs` `find_target` (~line 220) skips such
     players; `zone/pvp.rs` `may_attack_player` (~line 150) refuses a target that is not `visible && pvp` (unless `target == me`); the player area skills
     on players (the zone's `Effect` areas, `grep -n "fn hit_player\|areas" crates/game-server/src/zone.rs`) skip them too; Lua and FSM
     searches go through `targetable` already (check with `grep -n "targetable(" -r crates/game-server/src/zone`).
   - Orders: `pub enum AdminOrder { Set { gm: u32, stat: u8, value: u32 }, StaffLevel { gm: u32, op: u16, level: u8 }, RecallItem { gm: u32,
     item_type: u8, type_id: u8 }, Ban { gm: u32, op: u16, until: Option<Until> /* None lifts */ }, Called { position: [f32; 3], zone: u16,
     due: Instant } }` in `zone.rs` (or a new `zone/admin.rs`, `mod admin;` beside the others), `admin_orders:
     Mutex<HashMap<u32, Vec<AdminOrder>>>` in `Zones`, `pub fn admin_order(&self, map: u16, target: u32, order: AdminOrder) -> bool` (false
     when the target is not on that map; pushes and sends `ZoneEvent::Admin`), `pub fn take_admin_orders(&self, id: u32) -> Vec<AdminOrder>`,
     and `pub fn send_to_char(&self, id: u32, packet: &[u8])` (any map). Add `Admin` to `ZoneEvent` (~line 73).
   - Bans by character (deviation 9): `bans: Mutex<HashMap<u32, Bans>>` with `pub struct Bans { pub chat: Option<Until>, pub moves:
     Option<Until> }`; `pub fn bans_of(&self, id: u32) -> Bans`, `pub fn set_ban(…)`, `pub fn end_ban(…)`; entries with both `None` removed.
   - Monsters: move the body of `zone/lua.rs` `mob_gen_ex` (~line 697) into a `Zones`-level `pub fn mob_gen(&self, zone: u16, mob_id: u16,
     spot: [f32; 3], count: u8, now: Instant, rand) -> usize` that both use (one placement loop, spec 7.3; `usize::MAX` entry = never
     respawns); `pub fn mob_remove_near(&self, zone, mob_id, at, count) -> usize` (cells ±1 around `at`, living, x/z distance ≤ 10, mark
     `dead_until = Some(now)` as `mob_remove2` does, deviation 13) and `pub fn mob_remove_id(&self, zone, object_id) -> bool` (false when dead
     or absent).
   - NPCs (`zone/npc.rs`): `pub fn npc_create(&self, zone: u16, kind: u8, type_id: u16, position: [f32; 3], world: &World) -> bool` (a
     def from `world.npc_def`, the `place` code's fields for one entry with a new id from `last_npc_id`, position y − 0.9, `0x0E01` to the
     players whose view holds its cell; false for an unknown kind: check how `npc_def` signals one, e.g. a default def with type 0) and `pub fn
     npc_remove_near(&self, zone, kind, type_id, at, count) -> usize` (3 × 3 cells, distance ≤ 10, `0x0E02` to the view).
   - Zone tests (`zone.rs` tests module, as the existing ones with `Zones::new` and test players): a staff occupant is not found by
     `find_target` and not hit by a guard; after `set_gm_flags(visible: Some(true), attackable: Some(true))` it is; a player entering the view
     of an invisible staff member gets `0x0201` with kind 2 then `0x051D` with 13; `admin_order` reaches the target's event channel and
     `take_admin_orders` returns it once; `mob_gen` makes `count` monsters within ±10 that never respawn; `mob_remove_near` marks only those
     within 10; `npc_create` then `npc_remove_near` sends `0x0E01` then `0x0E02`.

4. **Database** (`crates/game-server/src/backend.rs`, `pg.rs`, `memory.rs`; migration `db/migrations/0022_gm_commands.sql`).
   - New trait `AdminStore` in `backend.rs` (add it to `Backend`, ~line 472): `fn set_account_status(&self, user_id: u32, status: i16)`,
     `fn account_id(&self, username: &str) -> Result<Option<u32>>`, `fn save_marks(&self, character_id: u32, marks: Marks)` with
     `pub struct Marks { pub grow: u8, pub vg: i16, pub cg: u8, pub og: u8, pub ig: u8 }`.
   - `pg.rs` SQL: `UPDATE accounts SET status = $2 WHERE id = $1`; `SELECT id FROM accounts WHERE lower(username) = lower($1) AND NOT
     deleted`; `UPDATE characters SET grow = $2, vg = $3, cg = $4, og = $5, ig = $6 WHERE id = $1`. Check the column types first (`\d
     characters`, `\d accounts`) and bind with matching widths, as the other queries in `pg.rs`.
   - `PlayCharacter` (backend.rs ~line 177) has no `cg`/`og`/`ig`: add `pub cg: u8, pub og: u8, pub ig: u8` and load them in `pg.rs`'s character
     query (find it with `grep -n "vg" crates/game-server/src/pg.rs`) and in `memory.rs`.
   - Guild writes: `GuildWrite::Points { guild, points }` (`UPDATE guilds SET points = $2 WHERE id = $1`) and `GuildWrite::Standing { guild,
     ranking, use_house }` (`UPDATE guilds SET ranking = $2, use_house = $3 WHERE id = $1`) beside `Ranking` (~line 1636 of `pg.rs`); memory
     backend likewise.
   - Deviations 14, 16, 17 (also on `AdminStore`): `fn character_account(&self, name: &str) -> Result<Option<(u32 /*user*/, i16 /*status*/)>>`
     (`SELECT c.user_id, a.status FROM characters c JOIN accounts a ON a.id = c.user_id WHERE lower(c.name) = lower($1) AND c.deleted_at IS
     NULL`; also give `account_id` the status back: `-> Option<(u32, i16)>`), `fn block_account(&self, user_id: u32, until:
     Option<DateTime>)` (`UPDATE accounts SET status = 2, blocked_until = $2 WHERE id = $1`; `None` = no end), `fn lift_block(&self,
     user_id: u32) -> Result<bool>` (`UPDATE accounts SET status = 0, blocked_until = NULL WHERE id = $1 AND status BETWEEN 2 AND 6`, true when
     a row changed), `fn closed_quests(&self, server_id: u8) -> Result<Vec<u16>>`, `fn set_quest_closed(&self, server_id: u8, quest: u16,
     closed: bool)` (insert / delete), `fn character_id(&self, name: &str) -> Result<Option<u32>>` (live characters), `fn add_event(&self,
     character_id: u32, name: &str) -> Result<bool>` (`INSERT … ON CONFLICT DO NOTHING` with no conflict target, so the unique index on
     `lower(name)` also stops a case variant; true when inserted), `fn remove_event(&self, character_id: u32, name: &str) -> Result<bool>`
     (`DELETE … WHERE character_id = $1 AND lower(name) = lower($2)`).
   - Migration `0022_gm_commands.sql` (spec deviations 16 and 17), exactly:
     ```sql
     CREATE TABLE closed_quests (
         server_id smallint NOT NULL,
         quest_id integer NOT NULL,
         closed_at timestamptz NOT NULL DEFAULT now(),
         PRIMARY KEY (server_id, quest_id)
     );
     CREATE TABLE character_events (
         character_id integer NOT NULL REFERENCES characters (id) ON DELETE CASCADE,
         name varchar(20) NOT NULL CHECK (length(name) BETWEEN 1 AND 20),
         added_at timestamptz NOT NULL DEFAULT now(),
         PRIMARY KEY (character_id, name)
     );
     CREATE UNIQUE INDEX character_events_name ON character_events (character_id, lower(name));
     ```
     Check that `characters.id` is `integer` (as `guild_bank_items` references it) and that the server applies migrations at start as for
     0021; the login server's `accounts.status`/`blocked_until` handling needs no change (status 2 is refused, a passed `blocked_until`
     lifts it).
   - `memory.rs`: the same methods on the in-memory store (accounts by name, status kept per user).
   - `tests/postgres.rs`: `set_account_status` then the login's view of it (read the row back), `account_id` ignoring case and `None` for an
     unknown name, `save_marks` round trip through `load`, `GuildWrite::Points` and `Standing`; `block_account` with an end and without, then
     `lift_block` (true), `lift_block` again (false), `lift_block` on a staff account with status 16 (false); `character_account` for a
     deleted character → `None`; `set_quest_closed` both ways and `closed_quests`; `add_event` twice and with other case (true, false,
     false), `remove_event` (true, then false).

5. **Guild side** (`crates/game-server/src/guild.rs`).
   - `pub fn admin_grade(&mut self, guild_name: &[u8], member_name: &[u8], grade: u8) -> bool` (spec 8.6 `FA13` with deviation 10): the guild
     by name (case-insensitive, live), the member by name; grade 2 to 9 (1 refused), not the current rank; promoting into 1 and 2 needs fewer
     than `MAX_OFFICERS` officers (as `change_rank`, ~line 1091); then the rank, `GuildWrite::Rank`, and `0x0D0C (STATE_RANK + rank, id)` to
     the guild (`send_guild`). An online member's own state follows as for `change_rank` (look at how `change_rank` updates the player).
   - `pub fn admin_ranking(&mut self, guild_name: &[u8], ranking: u8) -> bool` (1 to 31: `house.ranking`, `house.use_house = 1`,
     `GuildWrite::Standing`) and `pub fn admin_points(&mut self, guild_name: &[u8], points: u32) -> bool` (`house.points`,
     `GuildWrite::Points`).
   - `pub fn guild_id_by_name(&self, name: &[u8]) -> Option<u32>` for `FA12`.
   - Tests in `guild.rs`'s module: grade 1 refused, grade to the same rank refused, a promotion to 2 with 7 officers refused, a demotion
     sends `0x0D0C (209, id)` for rank 9; ranking 31 accepted, 32 refused, `use_house` becomes 1; points written.

6. **The GM's connection** (new `crates/game-server/src/server/admin.rs`, `mod admin;` in `server.rs` beside `mod duel;`).
   - In the world packet `match` (server.rs, the fallback ~line 2236): before the fallback, `opcode if opcode >= 0xF000 => admin::packet(conn,
     ctx, player, character, &mut state, opcode, payload).await?`. `admin::packet` runs `crate::admin::check` on the current level:
     `Close` → `bail!` (the connection ends, as the original's `Close(9)`), `Refused` → `F501 opcode` and a `debug` log, `Allowed` → the
     command. Every command logs one `info` line "gm command" (GM id and name, opcode, target, values).
   - **The level now** (spec section 6): `Player::privilege` becomes `AtomicU8` (server.rs ~line 523; its readers ~lines 703, 1393 and
     `server/pvp.rs` ~line 86 use `.load(Ordering::Relaxed)`), so an `F601`/`F602` order can change it for this connection; the zone's staff
     flag and visibility stay as they were until the next login.
   - **Chat** (spec section 5): `F101` → as `0x1101` (the code at server.rs ~line 1921) with the `F101` opcode; `F102` → `FindUserZone` is
     `ctx.zones.find_by_name` (~zone.rs line 2861), same country only, `F102` both ways or `chat::whisper_failed()`; `F103` → every player on
     every map (both countries); `F104` → the guild (`Guilds::chat`, ~guild.rs line 448, with the `F104` packet); `F105` → the party as
     `0x1105`; `F10A` → as the player's union notice (`ctx.zones.union_chat`, ~line 1969). Length > 128 → `bail!`; < 2 ignored. A chat ban
     (step 7) drops the line silently, as for the player opcodes (add the same check to the `0x11xx` handler).
   - **`F601`/`F602`**: `may_set_level`; the target by `find_by_name`; order `StaffLevel`. Unknown name → `F501 op`.
   - **`F701`**: target by `find_by_name` (any map), order `Set`. **`F703`**: target in the GM's zone (`state.map`) via `find_by_name`, no
     logout pending there (the target checks), the item known (`ctx.data.items.get`), order `RecallItem`.
   - **`F702`**: an unknown item → no answer; else for `count` (1 or more), as `open_box` (server/items.rs ~line 311): `NewItem`s with
     `min(left, stack)` each into `state.inventory.free_slots()` in order, make type `'S'` and `lapis::new_craft_name`, saved in **one**
     `ItemStore::trade` (all `SlotChange::Insert`s), then `0x0205` per item (as the box path) and `F501 0`; no room for anything → `F501 F702`,
     room for part → the part placed and `F501 F702`.
   - **`F704`/`F705`/`F70E`/`F70C`/`F70D`**: `ctx.data.mobs.get` (`usable_def`'s rule: id, HP and level non-zero) / `world.npc_def`, then the
     step 3 functions on `state.map` at `state.position`; answers per spec 7.3/7.4.
   - **`F706`/`F707`**: `Context::accepting: AtomicBool` (true at start); `serve` (server.rs ~line 135) drops a new connection while false
     (log "new connection refused: /server off"); `F501 0`. **`F708`**: `F501 F708`. **`F709`**: `ctx.store.account_id(name)`, then
     `OnlineUsers::kick(user_id) -> bool` (new, beside `kick_older` ~line 562: `send_replace(true)` on the account's kick); not found or not
     online → `F501 F709`.
   - **`FA01`/`FA02`/`FA05`/`FA06`**: `set_gm_flags`, send what it returns, `F501 0`.
   - **`FA07`**: target by `find_by_name`, `zones.send_to(map, id, warning)`, `F501 0`. **`FA08`**: `F501 0`, then `watch(map_of(zone), pos)`
     (the target's position: add `Zones::position_of(map, id)` if none exists).
   - **Moves** (spec 8.3, deviations 6 to 8): refused (`F501 op`) while `state.logout` or `state.item_move` is pending; then
     `move_player(conn, ctx, state, id, zone, position)` (server.rs ~line 2481) and `F501 0`.
     - `FA09`: the target's map (`find_by_name`) must have create type 0 (`world.create_type`), its position becomes the GM's.
     - `FA0A`: map 200 or a map the world lacks (`!world.is_safe_pos(map, [x, 0.0, z])`) → `F501 FA0A`; y 0.
     - `FA11`: `0xFFFF` = the GM's map; map 200 or unknown → `F501 FA11`; `world.bind_pos(map, country, state.position, rand)`.
     - `FA12`: bounds as `FA0A`; by `world.create_type(map)`: 3 → the named **character**'s party (`find_by_name`, then its party id: look
       for the party lookup used by `enter_instance`, `zone/instance.rs` ~line 23); 5 → `guild_id_by_name` and the ranking battle open
       (`ranking_open()`); 6 → the guild ranked 1 to 30, bought, `use_house` 1 (`House` fields); others → `F501 FA12`. The zone: a new
       `Zones::gm_zone(map, owner, world, defs) -> Option<u16>` in `zone/instance.rs` that finds the owner's zone or makes it as
       `enter_instance`/`enter_guild_house` do, without their refusals and without booking (the move places the GM).
   - **Bans** (`FA0D` to `FA10`): target by `find_by_name`; order `Ban` with `ban_until(minutes)` (or `None` to lift); `F501` comes from the
     target (step 7).
   - **Guilds** (`FA13`/`FA14`/`FA15`): the step 5 functions, `F501 0` or `F501 op`.
   - **`FA18`**: `call_targets` over the GM's map's players (a `Zones` helper returning `(id, staff, country, position)` for one map; staff ⇔
     level ≤ 10, take `Occupant::staff`), an order `Called` to each, and no answer (as the original).

7. **The target's connection** (server.rs event loop, ~line 1205, and `server/admin.rs`).
   - `ZoneEvent::Admin` → `for order in ctx.zones.take_admin_orders(id) { admin::carry_out(conn, ctx, player, character, state, order).await? }`.
   - `Set`: build a `Sheet` from `character`/`state`, `apply_set`; refused → `ctx.zones.send_to_char(gm, &answer(0xF701))`. Done: write the new
     values into `character`, save (`save_progress` for level, exp, points and stats; `trade(id, delta, &[])` for money, as other money
     changes; `save_honor`; `save_kills`; `save_marks` for grow, VG, CG, OG, IG); kills → `zones.set_kills` (`zone/pvp.rs` ~line 239, the
     `0x0522` to the view); stats and levels → `state.totals = totals(...)` and the level-up announcements of `award_exp`'s `levels > 0`
     branch (server.rs ~line 3291: move that block into a `fn announce_level(...)` both use; the not-own-kill form, `level_up(true, …)`);
     a stat change without a level change → `max_all` to the view and a fill (deviation 3); then `F701` packets to the target (the extras
     first: exp and main stat for a level set, the level for an exp set; then `set_answer(stat, value)`), then `F501 0` to the GM.
   - `StaffLevel`: `player.privilege.store(level)`, `ctx.store.set_account_status(player.user_id, status_of(level))`, `level_answer(op, level)`
     to the target, `F501 0` to the GM.
   - `RecallItem`: every bag place (bags 1 and up, deviation 5) holding that type and type id → `SlotChange::Remove` in one `trade`, then the
     removal packets the warehouse/drop code uses for a slot emptied (find the "item gone" packet with `grep -n "fn removed\|0x0206"
     crates/shaiya-proto/src/item.rs`), `F501 0` to the GM (also when nothing was found).
   - `Ban`: `ctx.zones.set_ban`/`end_ban`; chat → `bare(FA0D or FA0E)`; move → `state.move_banned = …`, the speeds re-sent through the existing
     speed path (`WorldState::speeds` ~line 992 returns move speed 255 while banned; look at how `sent_speeds` is refreshed), then `bare(FA0F
     or FA10)`; `F501 0` to the GM.
   - `Called`: a pending call `state.called = Some((due, zone, position))` and a `sleep_until_some` arm in the select loop (~line 1340) that
     runs `move_player` at its time (skipped while a logout is pending).
   - Ban ends (spec section 4): a `sleep_until_some(next ban end)` arm: chat → `end_ban`, `FA0E`; move → `end_ban`, speeds, `FA10`.
   - Entering the world: read `ctx.zones.bans_of(id)` into `state` (a move ban → speed 255 at once, deviation 9).
   - The chat handler of the player opcodes (~line 1895) drops the packet while `zones.bans_of(id).chat` is current.

8. **The commands the original lacks** (spec deviations 14 to 18; `server/admin.rs`).
   - **Kick with a reason** (deviation 18): `OnlineUser::kick` becomes `watch::Sender<Option<Kick>>` with `pub enum Kick { NewLogin,
     Operator, Banned { op: u16, minutes: u16 } }`; `kick_older` sends `NewLogin`, `/akick` `Operator`, a ban `Banned`. Where the connection
     reacts (`Connection::kick`/`kick_out` in server.rs; find them with `grep -n "kick_out" crates/game-server/src/server.rs`): `NewLogin` →
     `0x0004 0` then FIN as now; `Operator` → FIN only (as the original's `Close(9)`); `Banned` → `ban_notice(op, minutes)` then FIN.
     `OnlineUsers::kick(user_id, kick) -> bool`.
   - **`F70A`/`F805`** (ban): `F70A` → `account_id(name)`, `F805` → `character_account(name)`; refused (`F501 op`) when not found, when it is
     the GM's own account (`player.user_id`), or when its `status` staff level `(status >> 4) & 7` is not 0 and ≤ the GM's level; else
     `block_account(user, if minutes == 0 { None } else { now + minutes })`, `ctx.online.kick(user, Kick::Banned { op, minutes })`, `F501 0`.
   - **`F70B`/`F806`** (lift): the same lookups, `lift_block` → `F501 0`, false → `F501 op`.
   - **`FA16`/`FA17`**: `set_gm_flags(…, None, None, Some(true or false))`, `F501 0`.
   - **`F809`/`F80A`**: the quest must exist (`ctx.data.quests.get`); the closed set is `Zones::closed_quests: Mutex<HashSet<u16>>` (or on
     `Context`), loaded at start beside `load_kill_counts` (server.rs ~line 273) from `closed_quests(server_id)`; closing a closed one or
     opening an open one → `F501 op`; else write (`set_quest_closed`), change the set, `F501 0`. In `quest_start` (server.rs ~line 3337) a
     closed quest is refused before `may_take`, as `may_take` refuses (no answer; a `debug` log "quest start refused: closed by a GM").
   - **`F80F`/`F810`**: `character_id(name)` (online or not); `add_event` / `remove_event`; false → `F501 op`; else `F501 0`. Nothing reads
     the tags.
   - Tests: unit (`crate::admin`): the ban's staff-level refusal (GM level 2: status 16 and 32 refused, 48 and 0 allowed); `check` lets level 9
     send `FA16` and refuses `F805` at level 4. `game_flow.rs`: staff `F70A` on another test account in the world → that client gets
     `F70A 05 00` and its connection ends, the stored status is 2; `F70B` → status 0; after `FA06` then `FA17` a player's attack on the staff
     member does nothing; `F80A` on a test quest, then a player's `0x0902` for it starts nothing, after `F809` it works; `F80F` twice →
     `F501 0` then `F501 F80F`.

9. **Tests** (`crates/game-server/tests/game_flow.rs`, with the helpers there: a staff session is `StoredSession { privilege: 1, .. }`, ~line 227).
   - A player (privilege 11) sending `FA01` is disconnected; a level-5 staff member sending `F702` gets `F501 F702`; sending `FA16` gets
     `F501 FA16` and stays connected.
   - Staff `F702` (an item from the test data) → `0x0205` then `F501 0`; the item is in the stored bags.
   - Staff `F701` on another test player (level) → that player gets `F701 16 …`, the level-up packet, `F701 1 level`; the staff member gets
     `F501 0`; the stored level changed.
   - Staff `FA0D` on another player, who then sends `0x1101`: nobody gets the line; the player got `FA0D`.
   - Staff entering: another player in view gets `0x0201` kind 2 and `0x051D` 13; after `FA02` → `0x051D` with the real shape.
   - Staff `FA0A` on its own map → `0x020B` and `F501 0`; with map 200 → `F501 FA0A`.
   - `/server off` (`F707`): a new connection is closed before the handshake answer; `F706` → accepted again.

10. **Finish**: `cargo clippy --all-targets` clean as the rest of the crate, `cargo test` (all, with the PostgreSQL tests: `-- --nocapture`
   shows they really ran), release build; the spec's section 13 is marked "built" (the deviations are approved).

## Live test

Section 37 of `docs/testing/live-test-checklist.md`.
