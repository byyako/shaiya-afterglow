# Build plan: monster drops part 2 (quest items and sequence items)

Built; the behaviour is described in `docs/original/quest-drops.md`.

Spec: `docs/original/quest-drops.md`. Read it first (sections 1 to 4
are the rules, 6 the new server, 7 the deviations), then `docs/original/combat.md` section 7 and `docs/original/party.md` section 9 (the
drops and loot rules already built) and the code named below. Build the two deviations of the spec's section 7 as written: the maintainer
approved both (2026-10-19).

Names used below: **quest item** = an item of real type 27 (item types 27, 28, 29; `inventory::real_type`); **links** = the item → quest
lists of `NpcQuest.SData` (spec 1.3); **in progress** = `Quests::active()` (a quest waiting for a reward choice included).

## Steps (in this order; `cargo test -p game-server` green after each)

1. **Import** (`tools/world/import.py`, `db/world/schema.sql`).
   - Schema: `CREATE TABLE world.item_quest_links (item_type smallint NOT NULL, item_type_id smallint NOT NULL, started smallint[] NOT
     NULL, ended smallint[] NOT NULL, PRIMARY KEY (item_type, item_type_id));` and `CREATE TABLE world.sequence_groups (group_no integer
     PRIMARY KEY, sequence bigint NOT NULL, grades integer[] NOT NULL CHECK (cardinality(grades) = 10));`.
   - `npcquest.parse_all` already returns the links as its second value (`import.py` ~line 402 throws it away): keep it and copy one row
     per item with a non-empty `started` or `ended` list, lists in file order. Expect **481** items with `ended`, **46** with `started`.
   - `read_sequence_groups(path)`: as `GetPrivateProfileInt` reads `SequenceItemCreate.ini` (CP949, `;` comments, sections and keys
     case-insensitive, a missing key or a line without `=` is 0, the value is its leading `[-]digits`, else 0): `COUNT` from
     `[SEQUENCE_GROUP_COUNT]`, then for `n` = 1 to `COUNT` the section `[SEQUENCE_GROUP_n]`'s `SEQUENCE` and `GRADE_0`..`GRADE_9`. Store
     them as read (no clamping; the server applies the rules). Shipped: 7 rows, the first `30000001`, the seventh `16001` with grade 394
     ×7 then 0 ×3 (the eighth section is not read).
   - Add both to the summary print and the `copy` calls. Run `--dry-run` first, then the real import (it only rebuilds schema `world`;
     the running server read `world` at its start and is not affected).

2. **Mob definitions** (`crates/game-server/src/mobs.rs`, `pg.rs` ~line 2370).
   - `MobDef` gets `pub quest_item: Option<(u8, u8)>`. `pub fn quest_item_of(id: i32) -> Option<(u8, u8)>` in `mobs.rs` with the table of
     spec 1.1 (`0` → `None`; `1..=1000` → `(27, id as u8)`; `1001..=2000` → `(28, (id + 0x18) as u8)`; above → `(29, (id + 0x30) as u8)`;
     negative → `None`). Unit test: 1 → (27, 1), 255 → (27, 255), 256 → (27, 0) (the byte wraps as the original), 1001 → (28, 1), 1145 →
     (28, 145), 2001 → (29, 1).
   - `pg.rs`: select `questitemid` in the mobs query and fill `quest_item`.

3. **Drop tables** (`crates/game-server/src/drops.rs`).
   - `DropTables` gets `links: HashMap<(u8, u8), (Vec<u16>, Vec<u16>)>` (started, ended) and `sequences: Vec<(u32, [u16; 10])>`.
   - `pub fn load_quest_links(&mut self, rows: &[(i16, i16, Vec<i16>, Vec<i16>)]) -> usize`: types and type ids **1 to 255** only
     (`GetQuestLink`), quest ids above 0. `pub fn quest_start(&self, t: u8, id: u8) -> bool` (spec 1.2 `bQuestStart`: a non-empty started
     list; the item's existence is checked by the callers, which hold the definition) and `pub fn quest_ends(&self, t: u8, id: u8) -> &[u16]`.
   - `pub fn load_sequences(&mut self, rows: &[(i32, i64, Vec<i32>)]) -> usize`: spec 4.1: groups in `group_no` order, at most **8**; if
     **any** of the kept groups has sequence 0 (or one that doesn't fit a `u32`), **none** are kept (log a warning); grades as `u16`
     (out of range → 0).
   - `pub fn sequence_item(&self, level: u16, mob_object_id: u32, rand) -> Option<(u8, u8)>` (spec 4.2): `level` 1 to 99 and id ≠ 0, else
     `None`; the first group with `id % sequence == 0` decides: `grade[level / 10]`, 0 → `None` (no later group); else `self.pick(grade,
     rand)`.
   - `roll` becomes `roll(def, mob_object_id: u32, tables, items, has_quest: &mut impl FnMut(&[u16]) -> bool, rand) -> Vec<Loot>`, in
     spec section 2's order:
     1. **quest item** (spec 3.1): `def.quest_item` with a definition `d`; `chance = u32::from(d.quality)`. If `tables.quest_start(t, id)`:
        `rand() % 100 < chance` → push it. Else if `has_quest(tables.quest_ends(t, id))`: one roll, push on success. Push as the other
        drops do (`Loot::Item`, count 1, `quality: d.quality`, `kept` with make type `'D'` and `lapis::new_craft_name(d, rand)`: empty for
        quest items, but keep the call for the same `rand` use as the original's `CreateCraftExpansion`).
     2. the entries (unchanged);
     3. **sequence item** when `def.money != 0`: `tables.sequence_item(def.level, mob_object_id, rand)` with a definition → push with
        make type **`'V'`** (`SEQUENCE_MAKE_TYPE`) and `new_craft_name`;
     4. gold (unchanged).
   - Unit tests: a start item drops at 100 % quality without asking `has_quest` (count calls); a linked item drops only when `has_quest`
     says yes, with **one** roll (a failed roll is not tried again, so a `rand` giving 99 first drops nothing at chance 50); quality 0
     never drops; the quest item comes before the entries' items; the sequence rules (id 16001 × 3 at level 5 → group 7
     grade; level 0 and 100 → none; a group whose grade is 0 stops the search even when a later group divides; money 0 → none); a 0
     sequence among the groups → no groups loaded; more than 8 rows → 8.
   - Load both in `main.rs` next to `load_boxes` (`pg::load_quest_links`, `pg::load_sequence_groups`, with a "re-run
     tools/world/import.py" context like the boxes'), log the counts, and in `crates/game-server/tests/game_flow.rs`'s `data()` nothing is
     needed (empty tables = no quest or sequence drops).

4. **Quests in progress at the zone** (`zone.rs`, `server.rs`, `server/admin.rs`).
   - `Zones` gets `quests: Mutex<HashMap<u32, Vec<u16>>>` (character id → quest ids in progress). It is a **leaf lock**: take it last and
     hold nothing else's lock inside it. `pub fn set_quests(&self, id: u32, ids: Vec<u16>)`, `pub fn forget_quests(&self, id: u32)`.
   - `pub(super) fn has_quest(&self, players: &HashMap<u32, Placed>, killer: u32, wanted: &[u16]) -> bool`: false for an empty `wanted`;
     the killer, and with a party (`parties.party_of(killer)`, release that lock before taking `quests`) its members **present in
     `players`** (the killer's zone: deviation 1; the original takes every member); true when any of them has any of `wanted` in
     progress.
   - `server.rs`: `fn sync_quests<S>(ctx: &Context<S>, id: u32, quests: &Quests)` → `ctx.zones.set_quests(id,
     quests.active().iter().map(|a| a.id).collect())`. Call it once when the world loop starts (after the enter-world burst, where
     `state.quests` is ready) and **after every change of the in-progress list**: `quest_start` (~line 3509 `take`), the two `remove`s
     (~3592 success, ~3610 `fail_quest`: failure, give-up and time-outs go through it), `server/admin.rs` ~1077 (F80B) and ~1106 (F80C).
     `grep -n "state.quests\.\(take\|remove\|drop\)"` must show a sync after each. `set_choosing` doesn't change the list.
   - Leaving the world (the block with `party_leave_world` ~line 1211): `ctx.zones.forget_quests(character.id)`.
   - Unit tests in `zone.rs`: `has_quest` for a lone killer, a party member in the zone, a member not in `players` (false), an empty list.

5. **The kill** (`zone.rs` ~line 2283, the `drops::roll` call).
   - Pass `mob_id` (the monster's object id; `kind` is its definition id) and `&mut |wanted| killer_owner != 0 && self.has_quest(players,
     killer_owner, wanted)`. `players` is the map's players already in hand; the closure runs while `ground` is locked, which is fine as
     `parties` is already taken under it a few lines below and `quests` is a leaf.
   - In the `for l in loot` loop, **before** the `if let Some(share) = &share` block: an item whose `real_type` is 27 goes on the ground
     with `owner = killer_owner` and `party = share.as_ref().map_or(0, |s| s.party)`, skipping the loot rules (spec 3.2); `continue` after
     `self.place(...)`.
   - Zone tests (beside the existing drop tests, which use `no_loot()`/default tables): a monster with a start-linked quest item and
     quality 100 drops it with no quests; a monster with an end-linked one drops it only after `set_quests(killer, [q])`; in a party the
     item lands on the ground with the killer as owner and the party id, not in a member's bags (rule 1 would give it away otherwise); a
     member elsewhere (not in the zone) doesn't count.

6. **Picking up** (`shaiya-proto/src/ground.rs`, `zone.rs` `pick_up` ~line 2503, `server.rs` `pick_up` ~line 3669).
   - `pub const PICK_NO_QUEST: u8 = 2;` ("Unavailable quest item" 407 on the client).
   - `Zones::pick_up(map, player, item_id, now, quest_ok: impl Fn(u8, u8) -> bool)`: after the owner check and the distance check, a
     `Loot::Item` of real type 27 with `!quest_ok(type, type_id)` → `Err(PICK_NO_QUEST)`, item left on the ground. Gold is never checked.
   - `server.rs` passes `|t, i| ctx.data.drops.quest_start(t, i) || ctx.data.drops.quest_ends(t, i).iter().any(|&q| state.quests.find(q).is_some())`.
   - Update the existing `pick_up` tests (`|_, _| true`) and add: a quest item refused with 2 without the quest, taken with it, a start
     item taken by anyone, the owner error (0) still coming before the quest error.

7. **Monster ids that survive restarts** (migration, backend, `zone.rs`, `main.rs`).
   - `db/migrations/0024_mob_id.sql`: `ALTER TABLE game_servers ADD COLUMN mob_id bigint NOT NULL DEFAULT 1 CHECK (mob_id BETWEEN 0 AND
     4294967295);` (spec 4.3: a missing `MobID.ini` is written as 1).
   - `Backend`: `fn load_mob_id(&self, server_id: u8) -> Result<u32>` (the row's value; no row → 1) and `fn save_mob_id(&self, server_id:
     u8, id: u32) -> Result<()>`, in `pg.rs`, `memory.rs` (a field) and the forwarding impl in `backend.rs` (~line 671, as `heartbeat`).
   - `Zones::set_last_mob_id(&self, id: u32)` and `pub fn last_mob_id(&self) -> u32` (the existing `last_mob_id` atomic; `new_mob` in
     `zone/obelisk.rs` ~line 52 already counts every monster, GM and Lua summons and instance zones included).
   - `main.rs`: after `Context::new`, load it and `set_last_mob_id` (log it); spawn `save_mob_ids(ctx)`: every **60 s** (deviation 2;
     `const MOB_ID_SAVE_EVERY`) save it, logging a warning on failure; after `shut_down(...)` save it once more.
   - Postgres test in `crates/game-server/tests/postgres.rs` (as the others there, needs `DATABASE_URL`): save then load gives the value
     back for the test's server id; restore the old value at the end.

8. **Docs and finish.**
   - Spec: mark section 7's deviations as built (both approved), and section 6 "built" with any small choices you had to make.
     `docs/original/combat.md` section 7 items 1 and 3: "built, `docs/original/quest-drops.md`". `docs/original/quest.md` and
     `docs/original/item-origins.md`: one line each pointing to the new spec.
   - `cargo test` (whole workspace) and `cargo clippy -p game-server` clean; the release binaries of **both** `game-server` and
     `login-server` (the login server runs the migrations too and must know 0024; on restart the migration adds the column).

## Live test

Section 39 of `docs/testing/live-test-checklist.md`.
