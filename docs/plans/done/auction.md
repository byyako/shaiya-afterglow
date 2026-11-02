# Build plan: the auction house (`0x1Cxx`)

Built; the behaviour is described in `docs/original/auction.md`.

Spec: `docs/original/auction.md`. Read it first (sections 1 to 12
are the rules, 15 the new server, 16 the deviations), then `docs/client/auction-house.md` (the packets as the client reads them; read
only) and the code named below. Build the three deviations of the spec's section 16 as written there: the maintainer decided them
(2026-10-19: 1 and 2 approved, 3 approved as changed: the original's recall plus the deposit back).

Names used below: **auction** = an item on sale (`CMarket`); **item row** / **gold row** = what waits to be collected (spec 2.2, 2.3);
**deadline** = a row's collection deadline (made "now + 14 days"); **packed** = `shaiya_proto::world::pack_time` of the server's local
time (as `server.rs` ~line 4036 does for `0x0114`).

## Steps (in this order; `cargo test -p shaiya-proto` and `cargo test -p game-server` green after each)

1. **Protocol** (new `crates/shaiya-proto/src/market.rs`, `pub mod market;` in `lib.rs`; model: `my_shop.rs`).
   - Opcodes `OP_SEARCH 0x1C01` … `OP_INTEREST_CLEAR 0x1C17` with one-line doc comments (spec and the client note's tables).
   - Requests: `Search { country, level_min, level_max, category, grade, name: Vec<u8> }` (39 bytes with the opcode; the name cut at its
     first 0 within 31 bytes), `same_item -> (u8, u8)`, `page -> u8`, `Register { bag, slot, count, period, min: u32, direct: u32 }`,
     `id -> u32` (`0x1C09`, `0A`, `0C`, `0D`, `15`, `16`), `bid -> (u32, u32)`. A request shorter than its layout is `None` (ignored).
   - Rows: `ItemRow` (the item's `type, type_id, quality, gems, count, craft_name`) and writers `market_item` (48: id, price, direct,
     end, item), `sell_item` (52: + deposit), `end_item` (41: id, result, deadline, item), `end_money` (53: money id, market id, result,
     return, money, deadline, item). Item part order: `type, type id, quality u16, gems[6], count, craft name[21]` (21 bytes, NUL-padded).
   - Answers: `search(op, prev, next, rows)` (5 + 48n; `op` 0x1C01/02/03); `sell_list`, `bid_list`, `end_items`, `end_moneys`, each
     `-> Vec<Vec<u8>>` chunked **36 / 40 / 46 / 36** with a **last packet always sent** (n may be 0; spec 4); `interest_list` (3 + 48n);
     `register_ok(bag, slot, count, gold, &SellRow)` (62), `register_failed()` (3, result 1); `cancel(Option<&EndRow>)` (44 / 3);
     `buy(Result<(u32, &EndRow), u8>)` (48 / 3); `bid_ok(id, gold, &MarketRow)` (59), `bid_refused(id, r)` (7), `bid_too_low(id, min)`
     (11); `got_item(Option<(id, bag, slot, &ItemRow)>)` (41 / 3; the row order is `id, bag, slot, type, type id, count, quality, gems,
     craft name`: **count before quality** here); `got_money(Option<(money id, gold)>)` (11 / 3); notices `sold(&EndMoney)` 0x1C0E (55),
     `unsold(&EndItem)` 0x1C0F (43), `bid_on(id, price, type, type id, count)` 0x1C10 (13), `won(&EndItem)` 0x1C11 (43), `outbid(&EndMoney)`
     0x1C12 (55); `interest_added(Result<&MarketRow, u8>)` (51 / 3), `interest_removed(id, r)` (7), `interest_cleared(r)` (3).
   - Tests: every size above (with the opcode), a search page of 17 rows (821), the chunking (36 rows → one packet of 1875 then one of
     3; 37 → 1875 + 55), the craft name padding, a short request → `None`.

2. **Migration and storage** (`db/migrations/0025_market.sql`, `backend.rs`, `pg.rs`, `memory.rs`).
   - Tables (comment them after the original's, as other migrations do):
     `market_auctions (id integer PRIMARY KEY DEFAULT nextval('market_id_seq'), server_id smallint NOT NULL, seller_id integer NOT NULL
     REFERENCES characters (id), seller_name varchar(20) NOT NULL, ends_at timestamptz NOT NULL, min_money bigint, direct_money bigint,
     period smallint CHECK (period BETWEEN 0 AND 3), deposit bigint, bidder_id integer NULL REFERENCES characters (id), bidder_name
     varchar(20) NOT NULL DEFAULT '', price bigint NOT NULL` (the current price), then the item's columns exactly as `character_items`
     (`item_id … uid bigint NOT NULL UNIQUE`); money columns `CHECK (… BETWEEN 0 AND 4294967295)`.
     `market_result_items (market_id integer PRIMARY KEY, character_id integer NOT NULL REFERENCES characters (id), result smallint
     CHECK (result BETWEEN 0 AND 2), deadline timestamptz NOT NULL`, + the item's columns, `uid UNIQUE)`.
     `market_result_money (id integer PRIMARY KEY DEFAULT nextval('market_money_id_seq'), market_id integer NOT NULL, character_id …,
     result smallint, money bigint, deposit bigint, return_money bigint, deadline timestamptz,` + `type, type_id, quality, gems, count,
     craft_name)`.
     `market_interest (character_id integer REFERENCES characters (id) ON DELETE CASCADE, position smallint CHECK (position BETWEEN 0 AND
     9), market_id integer NOT NULL, PRIMARY KEY (character_id, position))`. Indexes on `market_auctions (server_id)` and the two result
     tables' `character_id`. The sequences start at 1.
   - Types in `backend.rs`: `StoredAuction`, `StoredItemRow`, `StoredMoneyRow` (times as `i64` microseconds, as `Kept` does), and
     `fn load_market(&self, server_id: u8) -> Result<(Vec<StoredAuction>, Vec<StoredItemRow>, Vec<StoredMoneyRow>)>` (the result rows of
     characters whose `server_id` is this server's).
   - **One write for everything**: `fn market(&self, write: &MarketWrite) -> Result<Option<MarketWritten>>`, one transaction; `None`
     when a check in it fails (then nothing changed). `MarketWrite { character: u32, money: i64, take: Option<(u8, u8, u8, u8)>` (bag,
     slot, the count the server believes is there, count to take: as `drop_item` does, the whole row moves keeping its uid, part of a
     stack makes a new row with a new uid and made now), `place: Option<(u8, u8)>` (an item row going into this bag place, uid kept),
     `ops: Vec<MarketOp> }` with `MarketOp::{NewAuction(..), EndAuction(id), SetBid { id, bidder, name, price }, NewItemRow { market_id,
     character, result, deadline, from: RowSource::Auction | Take }, DeleteItemRow(market_id), NewMoneyRow(..), DeleteMoneyRow(id)}`.
     `MarketWritten { auction_ids: Vec<u32>, money_ids: Vec<u32>, taken: Option<Kept> }` (the ids the inserts got, in op order).
     Moving the item between `character_items`, `market_auctions` and `market_result_items` keeps every column (see `pg.rs`'s
     `guild_bank` for moving a row between tables). The gold change uses `characters.money` with the same range check as `trade`.
   - `fn save_interest(&self, character_id: u32, ids: &[u32]) -> Result<()>` (delete then insert, positions in order); the interest list
     is loaded with the character (`PlayCharacter` gets `interest: Vec<u32>`, in position order).
   - `memory.rs`: the same on its maps (counters for the two ids). Postgres test in `crates/game-server/tests/postgres.rs`: register
     part of a stack (new uid) and a whole one (uid kept), bid, end with a bid (item row for the bidder, gold row for the seller),
     collect both; clean up its rows at the end.

3. **The rules** (new `crates/game-server/src/market.rs`, pure and unit-tested; `pub mod market;` in `lib.rs`).
   - `pub const AUCTION_MAP: u16 = 42; HOURS: [u32; 4] = [7, 24, 72, 168]; DEADLINE_SECS: i64 = 14 * 86_400; PAGE: usize = 17;
     INTEREST_MAX: usize = 10; PASS_EVERY: Duration = 60 s`.
   - `fee(period, min) = HOURS[period] * min.div_ceil(500_000) * 5`; `sale_return(deposit, price) = deposit + price - (price as f64 * 0.02) as
     u32` (in `u64`: `deposit - fee` alone can be negative; the sum always fits a `u32`); `next_min(price) = (price as f64 * 1.05) as u32` (Rust's `as` truncates and saturates, as `_ftol2`
     for these ranges). Tests: fee(0, 500) 35, fee(3, 500_000) 840, fee(1, 500_001) 240; sale_return(120, 10_000) 9_920; next_min(1000)
     1050, next_min(500) 525, next_min(4_000_000_000) saturates (unreachable: a bid must stay below buy-now ≤ 4e9).
   - `Market { auctions: BTreeMap<u32, Auction>, items: HashMap<u32, Vec<ItemRow>>, money: HashMap<u32, Vec<MoneyRow>> }` built from the
     stored rows. Per-character "registered" and "bidding" lists are derived (`auctions` filtered by seller / bidder, in id order).
   - **Search** (spec 3): `pub struct Search { mode: Mode (Normal(filter) | Seller(name) | Item(u32)), prev: bool, next: bool, rows: u16,
     prev_cursor: u32, next_cursor: u32, high: u32, low: u32 }` and `fn matches(def, auction, mode)` (country rule with the type-30
     exception, `(def.level as u8)` in range, `category_of(type)` (the spec's table as a `const` array), `def.grade` (`ReqDex`) low byte,
     the name: an **ASCII-case-insensitive** substring, deviation 2). `fn first(&self, search, defs) ->
     Vec<u8>`, `fn page(&self, search, action, defs) -> Option<Vec<u8>>` implementing 3.2/3.3 exactly with `BTreeMap::range` (the next
     lower / higher live id stands in for the original's `+0x130` walk). Tests: 40 matching auctions → pages 17/17/6 with the flags
     (prev 0/1/1, next 1/1/0), back from page 3 gives page 2's ids in rising order, a page whose next cursor auction ended starts at the
     next lower id, refresh, the country rule (Light hides 3 to 5, lapis passes), level 0 max → 255, category of types 2 and 100, item
     id mode ignores the other filters, seller mode exact name.
   - **Plans**, each returning `Result<Planned, Refusal>` where `Planned { write: MarketWrite, answer: …, notices: Vec<(u32 character,
     Vec<u8>)> }` is completed after the write with the ids (`fn apply(&mut self, planned, written, now) -> (answer, notices)`):
     `register` (spec 5.1 silent cases → `Refusal::Ignore`, the others → result 1; 5.2; 5.3), `cancel` (6), `buy` (7: refusal order
     1, 3, 2), `bid` (8: 1, 2, 3, 4, then 5 with `next_min`, 6 → `Ignore`), `collect_item` (9.1: deadline, first free place via
     `Inventory::free_slots` order bags 1.. slots 0..23), `collect_money` (9.2: the `u32` limit; **refused past
     its deadline**, deviation 1), `interest_add/remove/clear` (11; these touch only the character's list), `expire(now)` (10), `purge(now)` (deviation
     1: rows past their deadline), `recall(gm, name, id)` (12.1 plus deviation 3: a result-1 gold row with the deposit for the seller). Notices are for characters, sent only if online (step 4).
   - Tests per plan: every refusal code in its order; the fee taken and the deposit kept on cancel/unsold, returned on sale; a bid
     refunds the previous bidder with a result-1 gold row and `0x1C12`; the top bidder buying now gets its bid back; a bid ≥ buy-now is
     ignored; the first bid at `min` → 5 with `next_min(min)`; collecting into full bags → result 1; gold that would pass `u32` → 1; a
     row past its deadline is not listed and (item) not collectable; `expire` with and without a bid; interest add with a gone id
     purged first, duplicate 2, eleventh 3; remove answers 0 with a list even for an id not in it, 1 without one.

4. **The server** (new `crates/game-server/src/server/market.rs`; dispatch in `server.rs` next to the `0x2300` line ~1775).
   - `opcode & 0xFF00 == 0x1C00 && state.entered` → `market::packet(ctx, id, state, opcode, payload)`. **Not on map 42 → ignore,
     no answer** (spec 1). Unknown opcodes in the family: debug log, ignore.
   - `Context` gets `market: crate::market::House` = `{ turn: tokio::sync::Mutex<()>, state: std::sync::Mutex<Market> }`; writes take the
     turn (as `guild_bank_turn`), plan under the state lock, release it, `ctx.store.market(&write).await`, then take the lock again to
     apply, then send the answer and the notices. `None` from the store → `bail!` as the guild bank does (the plan was checked).
   - Register first does `ctx.zones.cancel_actions(id)` and `ctx.zones.shop_end(id)` (spec 5.3 step 1), then updates `state.money` and
     `state.inventory` after the write as the item drop does.
   - `WorldState` gets `market_search: Option<market::Search>` (cleared whenever `state.map` changes away from 42) and `interest:
     Vec<u32>` (from the load; saved with `save_interest` after each change).
   - Notices: add `Zones::send_to_character(&self, id: u32, packet: &[u8])` (any map, as `find_by_name` searches; a lingering character
     counts as online, as `CWorld::FindUser`).
   - Log every completed action at info (`market register` / `cancel` / `buy` / `bid` / `collect` / `end` with ids and amounts).

5. **Start and the pass** (`main.rs`, `server.rs`).
   - Load the market (`load_market(server_id)`) into `ctx.market` before the listener starts; log the counts.
   - `pub async fn market_pass<S: Backend>(ctx: Arc<Context<S>>)` every `PASS_EVERY` (as `save_mob_ids`): under the turn, `expire(now)`
     and `purge(now)` (deviation 1) → one `market` write per ended auction (so a failure affects one) → apply → notices. Spawn it
     in `main.rs` with the others.

6. **GM commands** (`server/admin.rs`, `admin.rs`).
   - `FA19` and `FA1A` (`ap::OP_AUCTION_RECALL` / `OP_AUCTION_SEARCH` exist) leave the "not built" list in the test at `admin.rs` ~line
     513 and are handled as spec 12 (map 42 check → `F501` with the opcode): `FA1A` makes the GM's `market_search` a seller search and
     sends the first page as `0x1C02`, then `F501 0`. `FA19` as spec 12.1 (the online seller disconnected,
     the item to the GM as a result-2 row, the `0x1C06` rows to the GM) plus deviation 3 (the deposit to the seller as a gold row). `docs/original/gm-commands.md` 8.8: point to the spec.

7. **Docs and finish.**
   - Spec section 15: "built", with any small choices you made; section 16: "built" for each deviation. `docs/original/personal-shop.md`
     line 9 and `docs/original/gm-commands.md` 8.8: one line each pointing to the spec.
   - `cargo test` (whole workspace) and `cargo clippy -p game-server` clean; the release binaries of **both** `game-server` and
     `login-server` (the login server runs the migrations too and must know 0025).

## Live test

Section 40 of `docs/testing/live-test-checklist.md`.
