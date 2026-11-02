# Build plan: operator (GM) commands, part 2

Built; the behaviour is described in `docs/original/gm-commands-2.md`.

Spec: `docs/original/gm-commands-2.md`. Read it first (sections 3 to 5
are the rules, section 10 the new server and its deviations), then part 1's spec `docs/original/gm-commands.md` sections 2, 3 and 13 and
its plan `docs/plans/done/gm-commands.md` (the design this part extends: orders, `server/admin.rs`, `zone/admin.rs`, `crate::admin`), then
`docs/client/gm-and-commands.md` ("In short", "Summary", "How a GM command travels"; read only, CRLF, never edit it) and the code named below. Build all fifteen deviations
of the spec's section 10 as written; the maintainer approved them (2026-10-19).

Names as in part 1: **GM** = the staff member sending a command; **level** = `Player::privilege` (1 to 10 staff, 11 player); **target** = the
character a command names; **order** = an `AdminOrder` the GM's connection leaves for the target's connection (`Zones::admin_order`).

## Design in one paragraph

The GM's connection parses each packet and checks what it can see (the target in the world, its map, the quest or map known); what changes
the target becomes an order carried out by the target's connection (`server/admin.rs` `carry_out`), which checks the rest (dead, logout
pending, staff protection by comparing the GM's level in the order with its own `player.privilege`), acts, and answers the GM with
`Zones::send_to_char(gm, F501 …)`. The user questions live in one server-wide `QuestionQueue` (`Context`, behind a `Mutex`), written to the
database as they change. The repeating notices are per GM connection (`WorldState`), with a `sleep_until_some` arm in the select loop as
part 1's ban ends.

## Steps (in this order; `cargo test -p game-server` and `cargo test -p shaiya-proto` green after each)

1. **Protocol** (`crates/shaiya-proto/src/admin.rs`, tests at the bottom).
   - Constants: `OP_USER_QUESTION = 0x1201`, `OP_UQ_GET = 0xF201`, `OP_UQ_ANSWER = 0xF202`, `OP_UQ_ANSWER_TO = 0xF203`, `OP_UQ_COUNT = 0xF204`,
     `OP_CURE = 0xF801`, `OP_CLEAR_BAGS = 0xF802`, `OP_CLEAR_EQUIPMENT = 0xF803`, `OP_KICK = 0xF804`, `OP_QUEST_ADD = 0xF80B`,
     `OP_QUEST_REMOVE = 0xF80C`, `OP_HEAL = 0xF80D`, `OP_CURE_ALL = 0xF80E`, `OP_QUIRY = 0xF901`, `OP_SUMMON = 0xF904`, `OP_SUMMON_TO =
     0xF905`, `OP_NOTICE_STAFF = 0xF906`, `OP_NOTICE_VIEW = 0xF907`, `OP_NOTICE_TO = 0xF908`, `OP_NOTICE_ZONE = 0xF909`, `OP_NOTICE_COUNTRY =
     0xF90A`, `OP_NOTICE_ALL = 0xF90B`, `OP_SUMMON_PARTY = 0xF90C` (check part 1's names first and keep its style).
   - Parsers (payload = after the opcode, exact lengths, `Option`): `parse_user_question` (`u16 n, text[n]`, 2 ≤ n ≤ 1000, payload 2 + n),
     `parse_uq_answer` (`u8 kind, u32 id, u16 n, text`, 1 ≤ n ≤ 1000, payload 7 + n), `parse_uq_answer_to` (`u32 id, name[21], u16 n, text`,
     payload 27 + n), `parse_name_quest` (23: `name[21], u16 quest`), `parse_summon_to` (31: `name[21], f32 x, f32 z, u16 map`),
     `parse_notice` (`u8 len, text`, 2 ≤ len ≤ 128 — `F906` only), `parse_repeat_notice` (`u16 repeat, u8 len, text`; len 0 or 1 allowed, the
     "off" form, payload 3 + len), `parse_notice_to` (`name[21], u16 repeat, u8 len, text`), `parse_notice_country` (`u8 country, u16 repeat,
     u8 len, text`). The repeat parsers return the length as sent so the caller tells "off" (< 2) from too long (> 128). Texts are kept as
     bytes with their zero. `parse_name` serves `F801` to `F804`, `F80D`, `F80E`, `F901`, `F904`.
   - Builders: `uq_question(waiting_plus_one: u16, id: u32, time: u32, name: &[u8], text: &[u8])` (`F201`, 35 + len), `uq_none()` (`F201 u16 0`),
     `uq_count(n)` (`F204`), `sheet(&Sheet…)` (`F901`, spec 5.1: define a small `QuirySheet` struct with the fields in wire order and `items:
     Vec<QuiryItem { slot, item_type, type_id, quality, gems: [u8; 6], craft_name: [u8; 21] }>`), `summoned(op, id, map, [f32; 3])` (20,
     same layout as `world::position`), `notice(op, text)` (`op u8 len, text`), `gm_cmd_whisper(text)` (`F102 u8 0, "<GM Cmd>"[21], u8 len,
     text`; reuse the part 1 whisper builder with the `F102` opcode).
   - Tests: each parser's bounds (n 1 and 1001 refused for `0x1201`, n 0 refused for `F202`; length one short/long → `None`), `uq_question` is
     35 + len, `sheet` with 2 items is 37 + 64, `summoned` is 20 bytes and equals `world::position` but for the opcode.

2. **The rules** (`crates/game-server/src/admin.rs`, and a new pure module `crates/game-server/src/user_questions.rs`).
   - `is_built`: add every opcode of step 1 but `0x1201` (a player opcode, not an `Fxxx`). Leave `FA0B`, `FA0C`, `FA19`, `FA1A` out.
   - `pub fn may_touch(gm_level: u8, target_level: u8, itself: bool) -> bool` (deviation 7): `itself || target_level > gm_level`
     (players are 11, so always touchable by staff).
   - `pub fn packed_time(year, month, day, hour, minute, second) -> u32` (spec 3.1, `GetSystemTime`): wrapping arithmetic in `u32`, year − 16.
     Unit test: 2009-07-15 12:34:56 against a value worked out by hand in the test's comment.
   - `pub fn answer_lines(text: &[u8]) -> Vec<Vec<u8>>` (deviation 2): the text without its zero cut into pieces of at most 120 bytes, each
     with a zero added; empty text → nothing.
   - `user_questions.rs`: `pub struct Question { id: u32, character_id: u32, name: Vec<u8>, text: Vec<u8>, time: u32 }` and `pub struct
     QuestionQueue { waiting: BTreeMap<u32, Question> /* by id = age */, in_process: HashMap<u32, Question>, held: HashMap<u32 /*gm*/,
     u32 /*question*/> }` with
     `ask(q) -> bool` (false when the character already has 3 unanswered, waiting or in process; deviation 4),
     `take(gm) -> Taken` (`Taken::Again(&Question)` when it holds one, deviation 3; `Taken::New(&Question, waiting_plus_one)` for the oldest;
     `Taken::Empty`), `answer(gm, id) -> Option<Question>` (clears the GM's hold whatever the id; removes `id` from `in_process` and clears
     every hold on it; spec 3.4 with deviation 3), `give_back(gm)` (the held one back to `waiting`: its id puts it first), `waiting_count()`.
     Unit tests for each, including the 4th question refused, `take` twice gives the same question, an answer by GM B frees GM A, `give_back`
     makes it the next `take`.
   - Notices (spec 5.3, 5.4): `pub enum NoticeKind { View, To, Zone, Country, All }` and `pub struct Repeat { due: Instant, period: Duration,
     text: Vec<u8>, target: u32 /* To: character id; Country: the country */ }`, `pub struct Notices([Option<Repeat>; 5])` with `set(kind,
     repeat)`, `stop(kind)`, `next_due() -> Option<Instant>`, `due(now) -> Vec<(NoticeKind, Repeat)>` which moves each due one to `now +
     period` (no catch-up). Tests: due order, stop, a repeat of 0 seconds never stored.

3. **Zone side** (`crates/game-server/src/zone/admin.rs`, `zone.rs`).
   - `AdminOrder` gains (each with `gm: u32` to answer and, where deviation 7 applies, `gm_level: u8`):
     `Cure { gm, op, kind: CureKind /* Debuffs (F801), Heal (F80D), All (F80E) */ }`, `Clear { gm, gm_level, op /* F802 or F803 */ }`,
     `Kick { gm, gm_level, op /* F804, or F709 (part 1's /akick, deviation 6) */ }`, `QuestAdd { gm, gm_level, quest: u16 }`,
     `QuestRemove { gm, gm_level, quest: u16 }`, `Quiry { gm }`, `Summon { gm: Option<u32> /* None for F90C members */, op, zone: u16,
     position: [f32; 3] }`.
   - `pub fn send_staff(&self, packet: &[u8])`: every player whose `Occupant::staff` is set (spec 3.2: who entered as staff), any map.
     `pub fn send_zone(&self, map: u16, packet: &[u8])`: every player of that zone (`CZone::SendAll`), unless such a helper exists already.
   - `pub fn clear_applied(&self, map: u16, id: u32, all: bool) -> Vec<Vec<u8>>`: `all == false` is `cure(&[], true)` (the debuffs, as
     `zone/pvp.rs` `mode_end` does); `all == true` takes every applied skill **but those with `def.keep_on_death`** (deviation 10; add a
     `Buffs::take_all_but_kept` beside `Buffs::cure` in `buffs.rs`), `0x050E` each returned, `ZoneEvent::Buffs` and `reform` as `cure` does.
   - Party members for `F90C`: `self.parties.lock().unwrap().party_of(id)` members, each located with `Zones::locate` (`zone/items.rs` ~line
     118: gives the map and `dead`).
   - Zone tests: `send_staff` reaches a staff occupant only; `clear_applied(all: true)` keeps a `keep_on_death` buff and removes a debuff and
     a buff.

4. **Database** (`backend.rs`, `pg.rs`, `memory.rs`; migration `db/migrations/0023_user_questions.sql`).
   - Migration exactly:
     ```sql
     CREATE TABLE user_questions (
         id serial PRIMARY KEY,
         server_id smallint NOT NULL,
         character_id integer NOT NULL REFERENCES characters (id) ON DELETE CASCADE,
         character_name varchar(20) NOT NULL,
         question bytea NOT NULL CHECK (length(question) BETWEEN 1 AND 1000),
         asked_at timestamptz NOT NULL DEFAULT now(),
         taken_by integer,
         answered_at timestamptz,
         answered_by integer,
         answer_kind smallint CHECK (answer_kind BETWEEN 1 AND 4),
         answer bytea,
         answer_to varchar(20)
     );
     CREATE INDEX user_questions_open ON user_questions (server_id, id) WHERE answered_at IS NULL;
     ```
     (`taken_by`/`answered_by` are character ids without a foreign key, so a deleted GM character does not lose the record.) Check that
     `characters.id` is `integer` and that the server applies migrations at start as for 0022.
   - On `AdminStore` (part 1): `add_question(server_id, character_id, name, text) -> Result<u32 /*id*/>`, `take_question(id, gm) -> Result<()>`
     (`taken_by = gm`), `release_question(id) -> Result<()>` (`taken_by = NULL`), `answer_question(id, gm, kind, text, to: Option<&str>) ->
     Result<()>`, `open_questions(server_id) -> Result<Vec<StoredQuestion>>` (unanswered, by id, with `asked_at` for the packed time) and, at
     start, `UPDATE user_questions SET taken_by = NULL WHERE server_id = $1 AND answered_at IS NULL` inside `open_questions` or beside it.
     The question's id in `F201` is the table's id (`u32`).
   - The character name column follows `characters.name`'s width (check `\d characters`; use the same type).
   - `memory.rs`: the same on the in-memory store.
   - `tests/postgres.rs`: add, take, release, answer round trip; `open_questions` returns only unanswered ones, oldest first, with
     `taken_by` cleared at load; a deleted character's questions go with it.

5. **Quests side** (`crates/game-server/src/quests.rs`).
   - `Quests::drop(&mut self, id: u16) -> bool`: out of the in-progress list without a finished mark (spec 4.4 `F80C`; the original's
     `0x0A02` with 2). `Quests::forget_finished(&mut self, id: u16)`: for `F80B` giving a finished quest again (deviation 9).
   - `pub fn plan_admin_start(def: &QuestDef, inventory: &Inventory, items: &ItemDefs) -> Result<ItemPlan, Refused>`: only the start items
     (`def.start_items`, as the loop at the end of `plan_start`), `Refused::NoRoom` when they do not fit. No other check.
   - Tests: `drop` leaves no finished mark and `may_take` is true again; `plan_admin_start` with full bags refuses.

6. **The GM's connection** (`crates/game-server/src/server/admin.rs`).
   - **`F201`**: `QuestionQueue::take(gm)`: `New` → `take_question` in the database, then `uq_question`; `Again` → `uq_question` again (n =
     waiting + 1); `Empty` → `uq_none`. **`F204`** → `uq_count(waiting)`. **`F202`/`F203`**: `answer(gm, id)`; found → `answer_question` (kind
     1 to 3, or 4 with the name for `F203`), then for `F203` the lines of `answer_lines` as `F102` whispers from the GM's name to the named
     character when `find_by_name` finds it (deviation 2); nothing is answered to the GM (as the original).
   - **`F801`, `F80D`, `F80E`**: `find_by_name`; not found → `F501 op` (deviation 5) and for `F80D`/`F80E` also the "failed" whisper with the
     typed name; another map than the GM's (`state.map`) → the same failure; else order `Cure`.
   - **`F802`, `F803`, `F80B`, `F80C`**: `find_by_name` on the GM's map; the quest known (`ctx.data.quests.get`) for `F80B`/`F80C`; else `F501
     op`; then the order with `gm_level = player.privilege`.
   - **`F804`**: `find_by_name` anywhere; none → `F501 F804`; else order `Kick`. Part 1's **`F709`**: keep its account lookup, but when the
     account's character is in the world send it the `Kick` order instead of `OnlineUsers::kick` so it gets `F709` and the staff check
     (deviations 6 and 7); an account online but not in the world (character screen) is kicked as now.
   - **`F901`**: `find_by_name` anywhere; none → `F501 F901`; else order `Quiry` (the target answers `F501 0` then the sheet).
   - **`F904`**: the GM's map has create type 0 (`world.create_type(state.map)`), else `F501 F904` (deviation 11); `find_by_name`; none →
     `F501 F904`; order `Summon { gm: Some(gm), op, zone: state.map, position: the GM's position }`.
   - **`F905`, `F90C`**: the map known and `!= 200` and x, z in bounds (`world.is_safe_pos(map, [x, 0.0, z])`, as part 1's `FA0A`), create type
     0 — else `F501 op` (deviation 11 for `F90C`); the target by `find_by_name`, none → `F501 op`. `F905`: order `Summon` to the target.
     `F90C`: the target and, with a party, every member located in the world, each an order `Summon { gm: None, … }`; the GM gets `F501 0`
     from its own connection once the orders are left (the original answers once).
   - **`F906`**: `parse_notice` or `F501 F906`; `send_staff(notice(F906, text))`; `F501 0`.
   - **`F907` to `F90B`**: len < 2 → `notices.stop(kind)` (`F908` stops `To`, deviation 14), `F501 0`; len > 128 → `F501 op`; `F908`: the target by
     `find_by_name`, none → `F501 F908`; `F90A`: country 0 or 1, else `F501 F90A` (deviation 15); then repeat 0 → `stop(kind)`, else `set`;
     send once now (`F907`: `send_near(state.map, gm, packet, NORMAL_CHAT_RANGE)` plus the GM itself as part 1's `F101`; `F908`:
     `send_to_char(target)`; `F909`: `send_zone(state.map)`; `F90A`: `send_country(country, None)`; `F90B`: `send_world`); `F501 0`.
   - Every command logs one `info` "gm command" line as in part 1.

7. **The target's connection** (`carry_out` in `server/admin.rs`, the event loop in `server.rs`).
   - `Cure`: dead (`ctx.zones.locate(id).dead` or the connection's own state) → `F501 op` to the GM (and the "failed" whisper for `F80D`/`F80E`
     via `send_to_char(gm, gm_cmd_whisper(…))`). `Debuffs` and `All`: `state.cooldowns.clear()` (spec 4.1: nothing sent about it); `Debuffs` →
     `clear_applied(all: false)`, `All` → `clear_applied(all: true)`, the packets to the player; every kind: `ctx.zones.fill(map, id,
     state.totals.max_health())` (what `0x0505` the zone sends); then `F501 0` to the GM and for `F80D`/`F80E` the "ok" whisper (`"cure ok
     ->Name"` / `"cure, buff,debuff clear ok ->Name"`, spec 4.5, the target's name as stored).
   - `Clear`: `admin::may_touch(gm_level, player.privilege, gm == id)` else `F501 op`; a logout pending → `F501 op`; `ctx.zones.cancel_actions(id)`
     (a trade ends first). `F802`: every item of bags 1 and up → `SlotChange::Remove` in one `trade`, applied, then `bare(F802)` to the
     player (no per-slot packets: the client empties itself, spec 4.2). `F803`: every bag-0 slot removed the same way, with
     `take_unsaved_wear` first as the item move does (server.rs ~line 2094), then `renumber`, `vehicles::mount_slot_changed` when the mount
     slot held something, `change_equipment` for each visible slot (< `VISIBLE_SLOTS`), then `bare(F803)`. `F501 0` to the GM.
   - `Kick`: `may_touch` else `F501 op`; `F501 0` to the GM, `bare(op)` to the player, then end the connection as part 1's `Kick::Operator`
     does (`bail!` or the existing close path; check how `Kick` is handled in server.rs ~line 447 and reuse it).
   - `QuestAdd`: `may_touch`; in progress (`state.quests.find`) → `F501 F80B`; `plan_admin_start`, `NoRoom` → `F501 F80B`; else
     `forget_finished`, `ctx.store.start_quest(id, quest, seconds, &plan.changes)` (it already upserts a finished row, pg.rs ~line 1759),
     `state.quests.take`, then the same packets as `quest_start` (server.rs ~line 3424: the item changes, `qp::started(0, quest)`, the
     inserts); `F501 0`.
   - `QuestRemove`: `may_touch`; not in progress → `F501 F80C`; every item of the quest's start item kinds in bags 1 and up (deviation 8) as
     `SlotChange::Remove`; `ctx.store.end_quest(id, &QuestEnd { quest_id, success: false, keep: false, money: 0, changes })` (the row deleted);
     applied; `state.quests.drop(quest)`; `qp::ended(&Ended { npc_id: 0, quest_id, ..Default::default() })` then the item packets
     (`send_item_changes(…, false)`); `F501 0`.
   - `Quiry`: `F501 0` then `sheet` to the GM (`send_to_char`), built from `character` (mode, level, exp, money, points, HG, VG, CG, OG, IG), the
     totals (`state.totals`, in the order STR, DEX, REC, INT, LUC, WIS) and bag 0's items (slot, type, type id, quality, gems, craft name).
   - `Summon`: dead or a logout or item move pending → `F501 op` to `gm` when it is `Some` (deviation 13), nothing for a party member. Else
     `move_player` with the summon's opcode (deviation 12): give `move_player` (server.rs ~line 2569) a parameter `own: Option<u16>`: `None`
     keeps today's behaviour; `Some(op)` sends `summoned(op, id, map_of(zone), position)` instead of `w::position` on a map change, and after
     the packets of `warp` on the same map. Then `F501 0` to `gm`.
   - **`0x1201`** (the player's world packet `match` in server.rs, beside the chat opcodes): `parse_user_question`, else ignored; a chat ban
     does not stop it (as the original); `QuestionQueue::ask` (false → `debug` log, nothing else); written with `add_question` first (its id
     is the question's).
   - **Leaving the world** (where part 1 clears a staff member's state): `give_back(gm)` and `release_question`.
   - **Notices**: `state.notices: Notices`; a `sleep_until_some(state.notices.next_due())` arm: for each due repeat send as step 6 does
     (from the GM's current map and position); `To` with its target not found (`find` by id: `Zones::locate`) → `stop(To)`.
   - Start: load the open questions into `Context::questions` beside `load_closed_quests` (server.rs ~line 285), with their packed times
     from `asked_at` in local time (`chrono::Local`; the crate is already a dependency of game-server).

8. **Tests** (`crates/game-server/tests/game_flow.rs`, helpers as part 1's: a staff session is `StoredSession { privilege: 1, .. }`).
   - A player sends `0x1201`; staff `F204` → `F204 01 00`; `F201` → the question with n = 1; `F201` again → the same question; `F203` naming
     the player → the player gets a `F102` whisper with the text, the stored row is answered; `F204` → 0.
   - A fourth question from one player is not counted.
   - Staff level 3 `F801` on a player in another zone → `F501 F801`; on a name nobody has → `F501 F801` (not the GM cured).
   - `F802` on a player → it gets `F802`, its stored bags are empty, staff gets `F501 0`; `F804` → it gets `F804` and its connection ends.
   - `F80B` with a test quest → the player gets `0x0902 0, quest` and its start item; again → `F501 F80B`; `F80C` → `0x0903` failure, the item
     gone, the stored row deleted.
   - `F901` → staff gets `F501 0` then `F901` of 37 + 32 k bytes.
   - `F904` → the player gets `F904` with its own id and the GM's position.
   - `F906` → another staff member gets it, the player not; `F907` with repeat 1 → the player in view gets `F907` twice within about 2.5 s; with
     len 0 afterwards → no more.
   - Level-2 staff `F804` on a level-1 staff member → `F501 F804` (deviation 7).

9. **Finish**: `cargo clippy --all-targets` clean as the rest of the crate, `cargo test` (all, with the PostgreSQL tests: `-- --nocapture`
   shows they really ran), release build; the spec's section 10 is marked "built" (the deviations are approved).

## Live test

Section 38 of `docs/testing/live-test-checklist.md`.
