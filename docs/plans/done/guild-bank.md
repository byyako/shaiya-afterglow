# Build plan: the guild bank (guild houses part 3)

Built; the behaviour is described in `docs/original/guild-bank.md`.

Spec: `docs/original/guild-bank.md`. Read it first (sections 5 to 9
are the rules, section 14 the new server), then `docs/original/warehouse.md` (the personal warehouse, built the same way) and the code
named below. Build all four proposed deviations of the spec's section 14 as written; they were approved with the others.

Names used below: **bank** = the guild bank (bag 255); **warehouse** = the personal one (bag 100). The rules' "rank" is the member's guild
rank (1 master, 2 officer, 3 to 9 members, 9 new); "ranking" is the guild's place (31 unranked).

## Steps (in this order; `cargo test -p game-server` and `cargo test -p shaiya-proto` green after each)

1. **Protocol** (`crates/shaiya-proto/src/world.rs`, `guild.rs`).
   - `world.rs`: `pub const GUILD_BANK: u8 = 255;` next to the other bag constants, and move the 33-byte unit writer out of `char_bank`
     (~line 181: `u8 slot, u8 type, u8 type id, u16 quality, u8 gems[6], u8 count, char craft name[21]`) into `pub fn bank_unit(p: &mut
     Vec<u8>, item: &ItemUnit)`, used by `char_bank` and below (the warehouse test at ~line 862 must stay green).
   - `guild.rs`: `OP_BANK_LIST = 0x0D29`, `OP_BANK_IN = 0x0D31`, `OP_BANK_OUT = 0x0D32`. `bank_list(items: &[ItemUnit]) -> Vec<Vec<u8>>`:
     `u8 count` then units, **50 per packet**, items in the order given (the caller sorts by slot), **no packet for none**.
     `bank_item_in(slot: u8, item: &ItemUnit, by: u32)` and `bank_item_out(slot: u8, item: &ItemUnit, by: u32)`: the unit written with
     **`slot` (the bank slot) in place of `item.slot`** (for `0x0D32` the item is the one that left, which now sits in a bag place), then
     `u32 by` (the mover's character id); 39 bytes each with the opcode. Also `pub const DISBAND_FAILED` exists already (0x0D03 0).
   - Tests beside the existing length tests (lengths include the 2-byte opcode, e.g. `guild_added` is 122): one item → 36; 50 → one
     packet of 1653; 51 → 1653 + 36; none → no packet; in/out → 39; the unit's bytes for a known item (count at offset 11 of the unit,
     after the gems).

2. **The rules** (new `crates/game-server/src/guild_bank.rs`, pure, unit-tested; add `pub mod guild_bank;` to `lib.rs`).
   - `pub const SLOTS: usize = 240;` `pub struct GuildBank { slots: Vec<Option<ItemUnit>> }` with `new(items: &[ItemUnit])` (bag 255,
     slot < 240), `get(slot)`, `items()` (by slot), `is_empty()`, and private `take` / `put` (as `Inventory`'s, which set `bag`/`slot` on the
     unit). In `inventory.rs` make `Inventory::take` (~line 644) and `Inventory::put` (~line 648) `pub(crate)`.
   - `pub fn allowed_slots(house: &House, rules: &HouseRules) -> usize`: `house.level(gh::KEY_BANK).min(5)` picks
     `rules.bank_slots[level]` (0 if the list is shorter). Shipped: 40, 80, …, 240.
   - `pub enum Kind { Deposit, Withdraw, Rearrange }`, `pub struct Planned { pub kind: Kind, pub fee: u32, pub changes: Vec<SlotChange> }`
     and `pub type Refusal = Option<(u8, u8)>` (`None` = nothing happens, no answer; `Some((type, require))` = `0x0D28`). Constants:
     `REFUSED_NOT_AUTHORIZED: u8 = 4` in `guild_house.rs` next to `REFUSED_RANKING` / `REFUSED_LEVEL`; `ANY_RANKING` (30) exists.
   - `pub fn plan(bank: &GuildBank, inventory: &Inventory, defs: &ItemDefs, ranking: u8, allowed: usize, rank: u8, money: u32, from: (u8,
     u8), to: (u8, u8)) -> Result<Planned, Refusal>`, in the spec's order (sections 5 to 8), with deviations 2 and 3:
     - **Deposit** (`from` a bag place by `inventory.is_bag`, `to.0 == 255`): `None` unless `to.1 < 240`, the bag place holds an item, its
       definition exists, its `real_type` is not 27 and it is not `bound`. Then: `from.0 == 0` → `None`; `ranking > 30` → `(1, 30)`; rank
       9 → `(4, 8)`; `to.1 >= allowed` → `(2, to.1 / 40)`; bank slot taken and rank > 2 → **`(4, 2)`** (deviation 3; the original `(2, 2)`);
       bank slot empty → `Move { from, to }`; taken → `Swap { a: from, b: to }`. Fee 0. No combining on the way in.
     - **Withdraw** (`from.0 == 255`, `to` a bag place): `None` unless `from.1 < 240` and the bank slot holds an item. Then: `to.0 == 0` →
       `None`; ranking → `(1, 30)`; rank ≥ 3 → **`(4, 2)`**; `from.1 >= allowed` → `(2, from.1 / 40)`; fee =
       `inventory::withdrawal_fee(def, count)` (item without a definition → `None`); `money < fee` → `None`; bag place empty → `Move { from,
       to }`; taken → **deviation 2**: the bag item must have a definition, real type not 27, not bound, else `None`; then `Swap { a: to,
       b: from }`.
     - **Rearrange** (both 255): `None` unless both slots < 240, they differ and the source holds an item. Then ranking → `(1, 30)`; rank ≥ 3
       → **`(4, 2)`**; `to.1 >= allowed` → `(2, to.1 / 40)` (the source is not checked, as the original); target empty → `Move`;
       both stack and match (`max_count > 1` for both, same type and type id, as `Inventory::combine` ~line 351) → the sum into the target
       when it fits (`SetCount` target, `Remove` source), else the target at the limit and the source the rest (two `SetCount`); otherwise
       `Swap`.
     - Anything else (bag 255 with slot ≥ 240, the warehouse with the bank) → `None`.
   - `pub fn apply(bank: &mut GuildBank, inventory: &mut Inventory, change: &SlotChange)`: `Move`/`Swap` between a bag place and a bank
     place take the unit out of one and put it into the other (fixing `bag`/`slot`); `Move`/`Swap`/`SetCount`/`Remove` at 255 only touch
     the bank; anything else goes to `inventory.apply`.
   - Unit tests (a small `ItemDefs` as in `inventory.rs`'s tests): slot counts per level (0 → 40, 1 → 80, 5 and 7 → 240); each refusal of
     each kind with its numbers (`(1, 30)`, `(4, 8)`, `(2, 1)` for slot 45 at level 0, `(4, 2)`); deposit of a bound item, a real-type-27
     item and from bag 0 → `None`; withdrawal fee (5 × sell 110 → 27; a type-94 bar → 0) and too little money → `None`; deposit and
     withdrawal swaps; withdrawal onto a bound bag item → `None` (deviation 2); rearrange move, swap, combine into one stack (5 + 5, limit
     10) and split (8 + 5 → 10 and 3); `apply` of each change leaves the unit's `bag`/`slot` right.

3. **Guild state** (`crates/game-server/src/backend.rs`, `guild.rs`, `zone/guild.rs`, `zone.rs`).
   - `StoredGuild` (backend.rs ~line 339) gets `pub bank: Vec<ItemUnit>` (bag 255). `Guild` (guild.rs ~line 80) gets `pub bank:
     GuildBank`, built in `Guilds::load` (~line 266).
   - `GuildWrite::BankCount { guild: u32, slot: u8, count: u8 }` (the load's count fix, step 6).
   - `Guilds` methods:
     - `bank_plan(&self, me, inventory, defs, rules: &HouseRules, money, from, to) -> Option<(u32, Result<Planned, Refusal>)>`: `None`
       without a guild (`guild_of`), else the guild id and `guild_bank::plan(&g.bank, …, g.house.ranking, allowed_slots(&g.house, rules),
       rank, …)`.
     - `bank_apply(&mut self, guild, me, inventory: &mut Inventory, planned: &Planned, from, to, money_after: u32) -> Option<Vec<u8>>`:
       applies every change (`guild_bank::apply`); then the packets: **Deposit**: to the other online members (a new
       `send_guild_except(guild, me, packet)` beside `send_guild` ~line 333) `0x0D32` (bank slot `to.1`) with the item now in the bag place
       when it was a swap, then `0x0D31` (bank slot `to.1`) with the item now in the bank slot; returns `w::item_move_result(from, item at
       from, to, item at to)` (70 bytes) for the mover. **Withdraw**: others get `0x0D32` (bank slot `from.1`) with the item now in the bag
       place, then `0x0D31` (bank slot `from.1`) with the bank slot's item when it was a swap; returns `w::bank_to_bag_result(from, bank
       item at from, to, bag item at to, money_after)` (74). **Rearrange**: `w::item_move_result(from, …, to, …)` to **every** online member
       (`send_guild`), returns `None`. If the guild is gone (only a deposit into an empty bank can meet a disband in between, deviation 4
       refuses the rest): take the item out of the inventory as the database did, log a warning ("guild bank move for a guild disbanded
       meanwhile"), and return the mover's answer with the bank place shown empty.
     - `burst` (~line 370): after the member list and requests, **`gp::bank_list(bank items)` before `gp::npc_levels`**. The two join
       paths (`join` ~line 846 and `join_answer` ~line 920): the same, before the NPC levels.
     - **Deviation 4** in `disband_request` (~line 1032), right after the master check: a non-empty bank → `0x0D03 DISBAND_FAILED` to the
       master, info log "disband refused: the guild bank holds items", return. `fire` (~line 1079): when the bank is not empty, one info
       line per item (guild, slot, type, type id, count) "disbanded guild's bank item kept in the database".
   - `zone/guild.rs` wrappers `guild_bank_plan` and `guild_bank_apply` (as `guild_return_etin` ~line 103).
   - `Zones` (zone.rs ~line 576, constructor ~line 1962) gets `guild_bank_turn: tokio::sync::Mutex<()>` and `pub async fn
     guild_bank_turn(&self) -> tokio::sync::MutexGuard<'_, ()>`: guild bank moves run one at a time (held from the plan to the apply,
     across the database write; never taken while holding a std lock).
   - Tests in guild.rs's test module: the burst has `0x0D29` before `0x0D24` when the bank holds items and none when empty; joining sends
     it; a deposit sends `0x0D31` to another online member and not to the mover; a rearrange sends `0x0204` to both; the disband refusal.

4. **Database** (`db/migrations/0021_guild_bank.sql`, `backend.rs`, `pg.rs`, `memory.rs`, `tests/postgres.rs`).
   - The migration, after `0014_bank_items.sql`'s pattern (comment line: the guild bank, `docs/original/guild-bank.md`, after the
     original's `GuildStoredItems`; bag 255 in the packets; rows of a disbanded guild are kept):
     ```sql
     CREATE TABLE guild_bank_items (
         guild_id   integer     NOT NULL REFERENCES guilds (id),
         slot       smallint    NOT NULL CHECK (slot BETWEEN 0 AND 239),
         item_id    integer     NOT NULL,
         type       smallint    NOT NULL CHECK (type BETWEEN 0 AND 255),
         type_id    smallint    NOT NULL CHECK (type_id BETWEEN 0 AND 255),
         quality    integer     NOT NULL DEFAULT 0 CHECK (quality BETWEEN 0 AND 65535),
         gems       bytea       NOT NULL DEFAULT '\x000000000000' CHECK (length(gems) = 6),
         count      smallint    NOT NULL DEFAULT 1 CHECK (count BETWEEN 1 AND 255),
         craft_name varchar(20) NOT NULL DEFAULT '',
         make_type  varchar(1)  NOT NULL DEFAULT '',
         made_at    timestamptz NOT NULL DEFAULT now(),
         uid        bigint      NOT NULL DEFAULT nextval('item_uid_seq') UNIQUE,
         CONSTRAINT guild_bank_items_pkey PRIMARY KEY (guild_id, slot) DEFERRABLE INITIALLY IMMEDIATE
     );
     ```
     The game server applies it at start (`sqlx::migrate!` in `main.rs`). The login server embeds the same migrations
     (`crates/login-server/src/main.rs`) and sqlx refuses to start against a database with a migration it does not know: rebuild the
     login server too (`cargo build --release -p login-server`) and restart it (`tools/login-server.sh restart`) in step 8.
   - `ItemStore` (backend.rs ~line 302) gets `fn guild_bank(&self, character_id: u32, guild_id: u32, money: i64, changes: &[SlotChange])
     -> impl Future<Output = Result<bool>> + Send;` documented like `trade` ("bag 255 is the guild's bank; an item crosses between a bag and
     the bank by `Move` or `Swap`"); add the forwarding impl next to `trade`'s (~line 524).
   - `pg.rs` (next to `trade` ~line 732 and `apply_bank_change` ~line 970): one transaction: the money as `trade` does; each change
     touching 255 goes to a new `apply_guild_bank_change(tx, character, guild, change)`, the rest to `apply_change`; any change not as
     expected → `Ok(false)` (rolled back). Statements (every one must affect exactly the rows it should; copy the warehouse's shapes with
     `guild_bank_items` / `guild_id` for `bank_items` / `user_id, server_id`): `SetCount`, `Remove`, 255→255 `Move` (with `NOT EXISTS`),
     255↔255 `Swap` (one `UPDATE … CASE`, 2 rows), bag→255 `Move` and 255→bag `Move` (the `WITH moved AS (DELETE … RETURNING …) INSERT …`
     pair, keeping `uid` and `made_at`). The **bag↔255 `Swap`**, in three statements: `DELETE FROM guild_bank_items WHERE guild_id = $1 AND
     slot = $2 RETURNING item_id, type, type_id, quality, gems, count, craft_name, make_type, made_at, uid` (one row, else false); the bag →
     bank move as above (the bank slot is now free); `INSERT INTO character_items (…, uid)` the returned row at the bag place.
   - `load_guilds` (~line 1262): load each live guild's bank (`SELECT guild_id, 255::int2 AS bag, slot, type, type_id, quality, gems, count,
     craft_name FROM guild_bank_items WHERE guild_id = ANY($1) ORDER BY guild_id, slot`, mapped with `ItemRow::into_unit` as the warehouse
     at ~line 355) into `StoredGuild::bank`. `write_guild` (~line 1354): `GuildWrite::BankCount` → `UPDATE guild_bank_items SET count`.
   - `memory.rs`: `guild_banks: Mutex<HashMap<u32, Vec<ItemUnit>>>`; `guild_bank` as `trade` (~line 431): the character's items chained
     with the guild's bank items, `changed_items`, then split back by `bag == 255`; `load_guilds` (~line 596) fills `bank`; `BankCount` in
     the `GuildWrite` match (~line 652).
   - `tests/postgres.rs`: a new test beside `guilds_are_created_changed_and_disbanded` (~line 549): a guild and a character with two items;
     deposit (Move) keeps the uid; a second deposit to a taken slot returns false and changes nothing; withdrawal with a fee lowers the
     money; bag↔bank swap; 255 swap and a combine (`SetCount` + `Remove`); `load_guilds` returns the bank with bag 255; `BankCount`.

5. **The handler** (new `crates/game-server/src/server/guild_bank.rs`, `mod guild_bank;` in `server.rs` beside `mod guild_house;` ~line
   51).
   - In the `w::OP_ITEM_MOVE` arm (server.rs ~line 1975), after `cancel_actions` and **before** the warehouse branch: `if from.0 ==
     w::GUILD_BANK || to.0 == w::GUILD_BANK { guild_bank::item_move(&mut conn, ctx, map, id, state, from, to).await?; continue; }`.
   - `item_move`: **deviation 1**: `ctx.data.world.create_type(map) == create_type::GUILD_HOUSE` and
     `ctx.zones.npc_kind_in_view(map, id, WAREHOUSE_KEEPER)` (the const at server.rs ~line 2692; make it `pub(super)`), else a debug line
     "guild bank move ignored: no house warehouse keeper" and return. Then `let _turn = ctx.zones.guild_bank_turn().await;`, then
     `ctx.zones.guild_bank_plan(id, &state.inventory, &ctx.data.items, ctx.data.world.guild_house(), state.money, from, to)`:
     `None` → debug "guild bank move without a guild"; `Some((_, Err(None)))` → debug "guild bank move ignored"; `Some((_, Err(Some((t,
     n)))))` → `conn.send(gd::house_refused(t, n))`; `Some((guild, Ok(planned)))` → `ctx.store.guild_bank(id, guild,
     -i64::from(planned.fee), &planned.changes)`, false → `bail!` as the warehouse does; then `state.money -= planned.fee`,
     `ctx.zones.guild_bank_apply(guild, id, &mut state.inventory, &planned, from, to, state.money)`, send the returned answer. Info log
     "guild bank move" (character, guild, kind, from, to, fee, money).
   - Equipment is never touched (bag 0 is refused), so no stats refresh is needed.

6. **Loading** (`server.rs` `start_guilds` ~line 164): before `Guilds::load`, for each stored guild's bank: a unit whose `(type, type_id)`
   has no definition in `ctx.data.items` is dropped with an error line ("guild bank item without a definition left out": guild, slot, type,
   type id); an item with `real_type` 1 to 25 whose count is above its definition's `max_count` gets `max_count` and a
   `Queued::Write(GuildWrite::BankCount { … })` on the writer (warn line). Info line at start: how many bank items were loaded.

7. **Docs**: the spec's section 14 says "built" where true (as `docs/original/guild-ranking.md` section 10 does: what `cargo test`
   covers, the live test section); `docs/original/guild-house.md` (its "Not covered" line and section 11's last bullets),
   `docs/original/warehouse.md` ("Not covered" and the table's last row) and `docs/original/guild.md` (last bullet) point to it as
   built.

8. **Finish**: `cargo clippy --all-targets` clean as the rest of the crate, `cargo test` (all, with the PostgreSQL tests:
   `-- --nocapture` shows they really ran), release build.

## Live test

Section 36 of `docs/testing/live-test-checklist.md`.
