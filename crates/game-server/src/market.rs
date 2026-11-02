//! The auction house's rules (`docs/original/auction.md`): the fee and the arithmetic, the auctions and the rows to collect as the
//! server keeps them (`Market`, filled from the store at start), searching and paging, and each request as a plan: the store write
//! that makes it (`MarketWrite`, one transaction) and what follows once it is written (`Market::apply`: the same change in memory,
//! the answer and the notices). The connection and the timer pass do the rest (`crate::server`).

use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

use shaiya_proto::market::{self as mp, EndItem, EndMoney, MarketRow, SellRow};

use crate::backend::{
    ItemUnit, MarketItem, MarketOp, MarketWrite, MarketWritten, NewAuction, NewMoneyRow, StoredAuction, StoredItemRow, StoredMoneyRow,
};
use crate::inventory::{Inventory, ItemDef, ItemDefs, SlotChange, real_type};

/// The Auction House (`CMap +0x1e0`, set for map 42 alone): every `0x1Cxx` request works only there (section 1).
pub const AUCTION_MAP: u16 = 42;
/// The periods 0 to 3 (table 0x5318e8).
pub const HOURS: [u32; 4] = [7, 24, 72, 168];
/// A row to collect stays this long (`GetResultEndDate`: now + 1,209,600 s).
pub const DEADLINE_SECS: i64 = 14 * 86_400;
/// Rows of a search page.
pub const PAGE: usize = 17;
/// The interest list's length.
pub const INTEREST_MAX: usize = 10;
/// The end pass (`CWorldThread::Update`, `+0xac`).
pub const PASS_EVERY: Duration = Duration::from_secs(60);

const MICROS: i64 = 1_000_000;
/// The lowest minimum, the highest minimum and the highest buy-now price (section 5.1).
const MIN_LOWEST: u32 = 500;
const MIN_HIGHEST: u32 = 3_800_000_000;
const DIRECT_HIGHEST: u32 = 4_000_000_000;
/// `bySpecial` refused at registration (none shipped).
const SPECIAL_REFUSED: u8 = 63;
/// Lapis: its country column holds link flags, so the country filter lets it through.
const LAPIS_TYPE: u8 = 30;

/// Result types of the rows (section 2.4).
pub const RESULT_WON: u8 = 0;
pub const RESULT_BACK: u8 = 1;
pub const RESULT_RECALLED: u8 = 2;

/// The registration fee, the auction's deposit (section 5.2): hours × started 500,000 of the minimum × 5.
pub fn fee(period: u8, min: u32) -> u32 {
    HOURS[usize::from(period.min(3))] * min.div_ceil(500_000) * 5
}

/// `trunc(price × 0.02)` (the x87 multiply by the double `0.02` with the chop mode, section 7).
fn commission(price: u32) -> u64 {
    (f64::from(price) * 0.02) as u64
}

/// What a sale pays the seller (sections 7 and 10): the deposit back, less 2 % of the price, plus the price. In 64 bits: the deposit
/// less the commission alone can be negative; the sum always fits a `u32` (a deposit is at most 6,384,000, a price at most 4e9).
pub fn sale_return(deposit: u32, price: u32) -> u32 {
    (u64::from(deposit) + u64::from(price) - commission(price)) as u32
}

/// The lowest next bid, `trunc(current × 1.05)` (`_ftol2`, section 8; Rust's `as` truncates and saturates).
pub fn next_min(price: u32) -> u32 {
    (f64::from(price) * 1.05) as u32
}

/// The packed local time of a time in microseconds since 1970 (section 2.5).
pub fn packed(micros: i64) -> u32 {
    crate::admin::packed_from_unix(micros.div_euclid(MICROS))
}

/// A collection deadline made now.
fn deadline(now: i64) -> i64 {
    now + DEADLINE_SECS * MICROS
}

/// The search category of an item type (`GetItemMType`, section 3.1): the first list holding it, 1 to 25; 0 for none.
pub fn category_of(item_type: u8) -> u8 {
    const LISTS: [&[u8]; 25] = [
        &[2, 4, 46, 48],
        &[1, 3, 45, 47],
        &[5, 49, 50],
        &[6, 51],
        &[7, 8, 53, 54, 55, 56],
        &[9, 15, 57, 65],
        &[10, 58],
        &[12, 60, 61],
        &[13, 62, 63],
        &[11, 14, 59, 64],
        &[16, 31, 66, 72, 81, 87],
        &[17, 32, 67, 73, 82, 88],
        &[18, 33, 68, 74, 83, 89],
        &[20, 35, 70, 76, 85, 91],
        &[21, 36, 71, 77, 86, 92],
        &[24, 39],
        &[19, 34, 69, 75, 84, 90],
        &[23, 38],
        &[22, 37],
        &[40, 41],
        &[30],
        &[95],
        &[25, 44],
        &[42],
        &[100],
    ];
    LISTS.iter().position(|list| list.contains(&item_type)).map_or(0, |i| i as u8 + 1)
}

/// What a search looks for (section 2.7, `+0x30`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    /// `0x1C01` with its filters (the highest level 0 made 255).
    Normal(mp::Search),
    /// `/auctionsearch`: the seller's name, exactly.
    Seller(Vec<u8>),
    /// `0x1C02`: `type × 1000 + type id`; the other filters are ignored.
    Item(u32),
}

/// A character's search and where its page is (`SearchInfo`), kept while it stays on map 42.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Search {
    pub mode: Mode,
    pub prev: bool,
    pub next: bool,
    /// Rows on the current page.
    pub rows: u16,
    pub prev_cursor: u32,
    pub next_cursor: u32,
    /// The current page's highest and lowest market ids (`+0x40`, `+0x44`).
    pub high: u32,
    pub low: u32,
}

impl Search {
    pub fn new(mode: Mode) -> Self {
        let mode = match mode {
            Mode::Normal(mut filter) => {
                if filter.level_max == 0 {
                    filter.level_max = 255;
                }
                Mode::Normal(filter)
            }
            other => other,
        };
        Self { mode, prev: false, next: false, rows: 0, prev_cursor: 0, next_cursor: 0, high: 0, low: 0 }
    }
}

/// `strstr` ignoring ASCII case (deviation 2).
fn contains_ignore_case(haystack: &[u8], needle: &[u8]) -> bool {
    needle.is_empty() || haystack.windows(needle.len()).any(|w| w.eq_ignore_ascii_case(needle))
}

/// Whether an auction matches a search (section 3.1). `def` and `name` are its item's definition and name.
pub fn matches(def: Option<&ItemDef>, name: &[u8], auction: &StoredAuction, mode: &Mode) -> bool {
    let item = &auction.item;
    match mode {
        Mode::Seller(seller) => auction.seller_name.as_bytes() == seller.as_slice(),
        Mode::Item(id) => u32::from(item.item_type) * 1000 + u32::from(item.type_id) == *id,
        Mode::Normal(f) => {
            let Some(def) = def else { return false };
            if f.country != 0 && def.item_type != LAPIS_TYPE && def.country != 6 {
                let hidden = if f.country == 1 { 3..=5 } else { 0..=2 };
                if hidden.contains(&def.country) {
                    return false;
                }
            }
            let level = def.level as u8;
            if level < f.level_min || level > f.level_max {
                return false;
            }
            if f.category != 0 && category_of(def.item_type) != f.category {
                return false;
            }
            if f.grade != 255 && def.req_dex as u8 != f.grade {
                return false;
            }
            f.name.is_empty() || contains_ignore_case(name, &f.name)
        }
    }
}

/// Why a request is not carried out: silently, or with this answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    Ignore,
    Answer(Vec<u8>),
}

/// What follows a write: the answer and the notices to build from the rows it made (`Market::apply`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Then {
    Register {
        bag: u8,
        slot: u8,
        count: u8,
        gold: u32,
    },
    /// `refund`: the outbid bidder.
    Cancel {
        id: u32,
        seller: u32,
        refund: Option<u32>,
    },
    /// The seller's gold row is the first new one, the refund (if any) the second.
    Buy {
        id: u32,
        gold: u32,
        seller: u32,
        refund: Option<u32>,
    },
    Bid {
        id: u32,
        gold: u32,
        seller: u32,
        refund: Option<u32>,
    },
    CollectItem {
        id: u32,
        bag: u8,
        slot: u8,
    },
    CollectMoney {
        id: u32,
        gold: u32,
        gained: u32,
    },
    /// An auction past its end without a bid: the item back to the seller.
    Unsold {
        id: u32,
        seller: u32,
    },
    /// With a bid: the item to the bidder, the gold row (the first new one) to the seller.
    Sold {
        id: u32,
        seller: u32,
        bidder: u32,
    },
    /// Rows past their deadline deleted (deviation 1): `(character, market id)` and `(character, money id, return)`.
    Purge {
        items: Vec<(u32, u32)>,
        money: Vec<(u32, u32, u32)>,
    },
    /// `/auctionrecall`: the items to the GM; per auction the refund (if any) then the seller's deposit row (deviation 3).
    Recall {
        gm: u32,
        ids: Vec<u32>,
        refunds: Vec<Option<u32>>,
    },
}

/// A request checked: the write, then (once written) `then`; with what the requester's own state becomes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Planned {
    pub write: MarketWrite,
    pub then: Then,
    /// The requester's gold after the write.
    pub gold: Option<u32>,
    /// What the requester's bag place becomes (registering), or the item it receives (collecting).
    pub bag_change: Option<SlotChange>,
    pub received: Option<ItemUnit>,
}

impl Planned {
    fn new(write: MarketWrite, then: Then) -> Self {
        Self { write, then, gold: None, bag_change: None, received: None }
    }
}

/// The market's rows a write is about, read before it is applied (`Market::log_view`).
#[derive(Clone, Debug, Default)]
pub struct LogView {
    pub auctions: HashMap<u32, StoredAuction>,
    pub money: Option<StoredMoneyRow>,
}

/// What a write's apply sends: the requester's packets in order and each `(character, packet)` notice, for characters that are
/// online (anywhere on this server).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Applied {
    pub answer: Vec<Vec<u8>>,
    pub notices: Vec<(u32, Vec<u8>)>,
}

/// The requester as the plans need it.
#[derive(Clone, Copy, Debug)]
pub struct Requester<'a> {
    pub id: u32,
    pub name: &'a str,
    pub gold: u32,
}

/// The auctions (`CMarketMgr +0x124`) and the rows to collect, by character in the order they were made (`+0x148`, `+0x154`).
#[derive(Clone, Debug, Default)]
pub struct Market {
    server_id: u8,
    pub auctions: BTreeMap<u32, StoredAuction>,
    pub items: HashMap<u32, Vec<StoredItemRow>>,
    pub money: HashMap<u32, Vec<StoredMoneyRow>>,
    /// The item names the name filter reads (`defs.items.itemname`).
    names: HashMap<(u8, u8), Vec<u8>>,
}

fn market_row(a: &StoredAuction) -> MarketRow {
    MarketRow { id: a.id, price: a.price, direct: a.direct, end: packed(a.ends_at), item: a.item.clone() }
}

fn sell_row(a: &StoredAuction) -> SellRow {
    SellRow { row: market_row(a), deposit: a.deposit }
}

fn end_item(r: &StoredItemRow) -> EndItem {
    EndItem { id: r.market_id, result: r.result, deadline: packed(r.deadline), item: r.item.clone() }
}

fn end_money(r: &StoredMoneyRow) -> EndMoney {
    EndMoney {
        money_id: r.id,
        market_id: r.market_id,
        result: r.result,
        return_money: r.return_money,
        money: r.money,
        deadline: packed(r.deadline),
        item: r.item.clone(),
    }
}

/// A refund of a bid (sections 6.2, 7.3, 8.1): money and deposit 0, the bid returned.
fn refund(id: u32, bidder: u32, bid: u32, deadline: i64, item: &MarketItem) -> MarketOp {
    MarketOp::NewMoneyRow(NewMoneyRow {
        market_id: id,
        character: bidder,
        result: RESULT_BACK,
        money: 0,
        deposit: 0,
        return_money: bid,
        deadline,
        item: item.clone(),
    })
}

/// The seller's gold row of a sale at `price` (sections 7.2 and 10).
fn sale(a: &StoredAuction, price: u32, deadline: i64) -> MarketOp {
    MarketOp::NewMoneyRow(NewMoneyRow {
        market_id: a.id,
        character: a.seller,
        result: RESULT_WON,
        money: price,
        deposit: a.deposit,
        return_money: sale_return(a.deposit, price),
        deadline,
        item: a.item.clone(),
    })
}

impl Market {
    pub fn new(
        server_id: u8,
        names: HashMap<(u8, u8), Vec<u8>>,
        (auctions, items, money): (Vec<StoredAuction>, Vec<StoredItemRow>, Vec<StoredMoneyRow>),
    ) -> Self {
        let mut market = Self { server_id, names, auctions: auctions.into_iter().map(|a| (a.id, a)).collect(), ..Self::default() };
        for row in items {
            market.items.entry(row.character).or_default().push(row);
        }
        for row in money {
            market.money.entry(row.character).or_default().push(row);
        }
        market
    }

    /// A character renamed: its name as seller and as top bidder (`docs/original/small-items.md` deviation 5; the database rows
    /// follow in the rename's own transaction).
    pub fn renamed(&mut self, id: u32, name: &str) {
        for a in self.auctions.values_mut() {
            if a.seller == id {
                a.seller_name = name.to_owned();
            }
            if let Some((bidder, bidder_name)) = &mut a.bidder
                && *bidder == id
            {
                *bidder_name = name.to_owned();
            }
        }
    }

    pub fn counts(&self) -> (usize, usize, usize) {
        (self.auctions.len(), self.items.values().map(Vec::len).sum(), self.money.values().map(Vec::len).sum())
    }

    fn matching(&self, a: &StoredAuction, mode: &Mode, defs: &ItemDefs) -> bool {
        let key = (a.item.item_type, a.item.type_id);
        matches(defs.get(key.0, key.1), self.names.get(&key).map_or(&[][..], Vec::as_slice), a, mode)
    }

    /// The matching auctions from `start` down (or up), at most `PAGE` of them, and the id of a further match if there is one.
    fn walk(&self, search: &Search, defs: &ItemDefs, start: u32, up: bool) -> (Vec<&StoredAuction>, Option<u32>) {
        let mut rows = Vec::new();
        let ids: Box<dyn Iterator<Item = &StoredAuction>> = if up {
            Box::new(self.auctions.range(start..).map(|(_, a)| a))
        } else {
            Box::new(self.auctions.range(..=start).rev().map(|(_, a)| a))
        };
        for a in ids.filter(|a| self.matching(a, &search.mode, defs)) {
            if rows.len() == PAGE {
                return (rows, Some(a.id));
            }
            rows.push(a);
        }
        (rows, None)
    }

    /// The first page (`SearchMarketFirst`, section 3.2): from the highest id down; `0x1C01` for a normal search, `0x1C02` otherwise.
    pub fn first(&self, search: &mut Search, defs: &ItemDefs) -> Vec<u8> {
        search.prev = false;
        search.next = false;
        search.prev_cursor = 0;
        search.next_cursor = 0;
        let (rows, more) = self.walk(search, defs, u32::MAX, false);
        if let Some(last) = rows.last() {
            search.low = last.id;
        }
        if let Some(id) = more {
            search.next_cursor = id;
            search.next = true;
        }
        search.rows = rows.len() as u16;
        let op = if matches!(search.mode, Mode::Normal(_)) { mp::OP_SEARCH } else { mp::OP_SAME_ITEM };
        let rows: Vec<MarketRow> = rows.into_iter().map(market_row).collect();
        mp::search(op, search.prev, search.next, &rows)
    }

    /// `0x1C03` (section 3.3): 0 refresh, 1 previous, 2 next. `None` sends nothing.
    pub fn page(&self, search: &mut Search, action: u8, defs: &ItemDefs) -> Option<Vec<u8>> {
        let had_rows = search.rows > 0;
        let rows: Vec<&StoredAuction> = match action {
            2 if search.next => {
                let start = self.auctions.range(..=search.next_cursor).next_back()?.0;
                search.prev_cursor = search.low;
                search.next_cursor = 0;
                search.next = false;
                let (rows, more) = self.walk(search, defs, *start, false);
                if let (Some(first), Some(last)) = (rows.first(), rows.last()) {
                    search.high = first.id;
                    search.low = last.id;
                }
                if let Some(id) = more {
                    search.next_cursor = id;
                    search.next = true;
                }
                search.prev = had_rows;
                rows
            }
            1 if search.prev => {
                let start = self.auctions.range(search.prev_cursor..).next()?.0;
                search.next_cursor = search.high;
                search.prev_cursor = 0;
                search.prev = false;
                let (rows, more) = self.walk(search, defs, *start, true);
                if let (Some(first), Some(last)) = (rows.first(), rows.last()) {
                    search.low = first.id;
                    search.high = last.id;
                }
                if let Some(id) = more {
                    search.prev_cursor = id;
                    search.prev = true;
                }
                search.next = had_rows;
                rows
            }
            0 if search.prev_cursor == 0 => return Some(self.first(search, defs)),
            0 => {
                if search.high == 0 || search.low == 0 {
                    return None;
                }
                let rows: Vec<&StoredAuction> = self
                    .auctions
                    .range(search.low..=search.prev_cursor)
                    .map(|(_, a)| a)
                    .filter(|a| self.matching(a, &search.mode, defs))
                    // Auction ids rise, so a refresh holds the page and what was listed since; never more than a packet can carry.
                    .take(PAGE + 1)
                    .collect();
                let rows: Vec<MarketRow> = rows.into_iter().map(market_row).collect();
                return Some(mp::search(mp::OP_PAGE, search.prev, search.next, &rows));
            }
            _ => return None,
        };
        search.rows = rows.len() as u16;
        let rows: Vec<MarketRow> = rows.into_iter().map(market_row).collect();
        Some(mp::search(mp::OP_PAGE, search.prev, search.next, &rows))
    }

    /// `0x1C04`: the character's auctions, by id.
    pub fn sell_list(&self, me: u32) -> Vec<Vec<u8>> {
        let rows: Vec<SellRow> = self.auctions.values().filter(|a| a.seller == me).map(sell_row).collect();
        mp::sell_list(&rows)
    }

    /// `0x1C05`: the auctions it is the top bidder of, by id.
    pub fn bid_list(&self, me: u32) -> Vec<Vec<u8>> {
        let rows: Vec<MarketRow> =
            self.auctions.values().filter(|a| a.bidder.as_ref().is_some_and(|b| b.0 == me)).map(market_row).collect();
        mp::bid_list(&rows)
    }

    /// `0x1C06`: its item rows not past their deadline.
    pub fn end_items(&self, me: u32, now: i64) -> Vec<Vec<u8>> {
        let rows: Vec<EndItem> = self.items.get(&me).into_iter().flatten().filter(|r| r.deadline >= now).map(end_item).collect();
        mp::end_items(&rows)
    }

    /// `0x1C07`: its gold rows not past their deadline.
    pub fn end_moneys(&self, me: u32, now: i64) -> Vec<Vec<u8>> {
        let rows: Vec<EndMoney> = self.money.get(&me).into_iter().flatten().filter(|r| r.deadline >= now).map(end_money).collect();
        mp::end_moneys(&rows)
    }

    /// `0x1C14`: the listed ids still auctions, in list order.
    pub fn interest_list(&self, list: &[u32]) -> Vec<u8> {
        let rows: Vec<MarketRow> = list.iter().filter_map(|id| self.auctions.get(id)).map(market_row).collect();
        mp::interest_list(&rows)
    }

    /// `0x1C08` (section 5): the silent cases ignored, the others result 1.
    pub fn register(&self, me: Requester, inventory: &Inventory, defs: &ItemDefs, r: mp::Register, now: i64) -> Result<Planned, Refusal> {
        if r.bag == 0 || !inventory.is_bag((r.bag, r.slot)) || r.count == 0 {
            return Err(Refusal::Ignore);
        }
        let Some(item) = inventory.get(r.bag, r.slot) else { return Err(Refusal::Ignore) };
        if r.count > item.count {
            return Err(Refusal::Ignore);
        }
        let Some(def) = defs.get(item.item_type, item.type_id) else { return Err(Refusal::Ignore) };
        let period = r.period.min(3);
        let deposit = fee(period, r.min);
        let refused = r.min < MIN_LOWEST
            || r.min >= r.direct
            || r.direct > DIRECT_HIGHEST
            || r.min > MIN_HIGHEST
            || deposit > me.gold
            || def.special == SPECIAL_REFUSED
            || real_type(def.item_type) == 27
            || def.bound;
        if refused {
            return Err(Refusal::Answer(mp::register_failed()));
        }
        let whole = r.count == item.count;
        // Part of a stack is a new item: the definition's quality, no gems, `InitCraftExpansion`'s name (section 5.3 step 3).
        let auctioned = if whole {
            MarketItem {
                item_type: item.item_type,
                type_id: item.type_id,
                quality: item.quality,
                gems: item.gems,
                count: item.count,
                craft_name: item.craft_name.clone(),
            }
        } else {
            let name = crate::lapis::zero_craft_name(def);
            let len = name.iter().position(|&b| b == 0).unwrap_or(name.len());
            MarketItem {
                item_type: item.item_type,
                type_id: item.type_id,
                quality: def.quality,
                gems: [0; 6],
                count: r.count,
                craft_name: name[..len].to_vec(),
            }
        };
        let auction = NewAuction {
            server_id: self.server_id,
            seller: me.id,
            seller_name: me.name.to_string(),
            ends_at: now + i64::from(HOURS[usize::from(period)]) * 3600 * MICROS,
            min: r.min,
            direct: r.direct,
            period,
            deposit,
            item: auctioned,
        };
        let write = MarketWrite {
            character: me.id,
            money: -i64::from(deposit),
            take: Some((r.bag, r.slot, item.count, r.count)),
            ops: vec![MarketOp::NewAuction(auction)],
            ..MarketWrite::default()
        };
        let gold = me.gold - deposit;
        let mut planned = Planned::new(write, Then::Register { bag: r.bag, slot: r.slot, count: r.count, gold });
        planned.gold = Some(gold);
        planned.bag_change = Some(if whole {
            SlotChange::Remove { bag: r.bag, slot: r.slot }
        } else {
            SlotChange::SetCount { bag: r.bag, slot: r.slot, count: item.count - r.count }
        });
        Ok(planned)
    }

    /// `0x1C09` (section 6): the item back to the seller, the deposit kept; a bidder refunded.
    pub fn cancel(&self, me: u32, id: u32, now: i64) -> Result<Planned, Refusal> {
        let Some(a) = self.auctions.get(&id).filter(|a| a.seller == me) else { return Err(Refusal::Answer(mp::cancel(None))) };
        let deadline = deadline(now);
        let mut ops = vec![MarketOp::EndAuction { id, character: me, result: RESULT_BACK, deadline }];
        let bidder = a.bidder.as_ref().map(|b| b.0);
        if let Some(bidder) = bidder {
            ops.push(refund(id, bidder, a.price, deadline, &a.item));
        }
        let write = MarketWrite { character: me, ops, ..MarketWrite::default() };
        Ok(Planned::new(write, Then::Cancel { id, seller: me, refund: bidder }))
    }

    /// `0x1C0A` (section 7): refusals 1, 3, 2 in that order. The top bidder may buy too, and gets its bid back. An auction past its end
    /// is closed (1) even before the pass that ends it (audit R20, deviation).
    pub fn buy(&self, me: Requester, id: u32, now: i64) -> Result<Planned, Refusal> {
        let refused = |r| Err(Refusal::Answer(mp::buy(Err(r))));
        let Some(a) = self.auctions.get(&id).filter(|a| a.ends_at > now) else { return refused(1) };
        if me.gold < a.direct {
            return refused(3);
        }
        if a.seller == me.id {
            return refused(2);
        }
        let deadline = deadline(now);
        let mut ops = vec![MarketOp::EndAuction { id, character: me.id, result: RESULT_WON, deadline }, sale(a, a.direct, deadline)];
        let bidder = a.bidder.as_ref().map(|b| b.0);
        if let Some(bidder) = bidder {
            ops.push(refund(id, bidder, a.price, deadline, &a.item));
        }
        let gold = me.gold - a.direct;
        let write = MarketWrite { character: me.id, money: -i64::from(a.direct), ops, ..MarketWrite::default() };
        let mut planned = Planned::new(write, Then::Buy { id, gold, seller: a.seller, refund: bidder });
        planned.gold = Some(gold);
        Ok(planned)
    }

    /// `0x1C0B` (section 8): refusals 1 to 4, then 5 below the next minimum, and a price at or above the buy-now price ignored. An
    /// auction past its end is closed (1) even before the pass that ends it (audit R20, deviation).
    pub fn bid(&self, me: Requester, id: u32, price: u32, now: i64) -> Result<Planned, Refusal> {
        let refused = |r| Err(Refusal::Answer(mp::bid_refused(id, r)));
        let Some(a) = self.auctions.get(&id).filter(|a| a.ends_at > now) else { return refused(1) };
        if a.seller == me.id {
            return refused(2);
        }
        if a.bidder.as_ref().is_some_and(|b| b.0 == me.id) {
            return refused(3);
        }
        if me.gold < price {
            return refused(4);
        }
        let min = next_min(a.price);
        if price < min {
            return Err(Refusal::Answer(mp::bid_too_low(id, min)));
        }
        if price >= a.direct {
            return Err(Refusal::Ignore);
        }
        let mut ops = vec![MarketOp::SetBid { id, bidder: me.id, name: me.name.to_string(), price }];
        let previous = a.bidder.as_ref().map(|b| b.0);
        if let Some(previous) = previous {
            ops.push(refund(id, previous, a.price, deadline(now), &a.item));
        }
        let gold = me.gold - price;
        let write = MarketWrite { character: me.id, money: -i64::from(price), ops, ..MarketWrite::default() };
        let mut planned = Planned::new(write, Then::Bid { id, gold, seller: a.seller, refund: previous });
        planned.gold = Some(gold);
        Ok(planned)
    }

    /// `0x1C0C` (section 9.1): an item row not past its deadline, into the first free bag place.
    pub fn collect_item(&self, me: u32, inventory: &Inventory, id: u32, now: i64) -> Result<Planned, Refusal> {
        let refused = || Err(Refusal::Answer(mp::got_item(None)));
        let Some(row) = self.items.get(&me).and_then(|rows| rows.iter().find(|r| r.market_id == id)) else { return refused() };
        if row.deadline < now {
            return refused();
        }
        let Some(&(bag, slot)) = inventory.free_slots().first() else { return refused() };
        let write = MarketWrite { character: me, place: Some((id, bag, slot)), ..MarketWrite::default() };
        let item = &row.item;
        let mut planned = Planned::new(write, Then::CollectItem { id, bag, slot });
        planned.received = Some(ItemUnit {
            bag,
            slot,
            item_type: item.item_type,
            type_id: item.type_id,
            quality: item.quality,
            gems: item.gems,
            count: item.count,
            craft_name: item.craft_name.clone(),
        });
        Ok(planned)
    }

    /// `0x1C0D` (section 9.2): within 32 bits; refused past its deadline too (deviation 1).
    pub fn collect_money(&self, me: Requester, id: u32, now: i64) -> Result<Planned, Refusal> {
        let refused = || Err(Refusal::Answer(mp::got_money(None)));
        let Some(row) = self.money.get(&me.id).and_then(|rows| rows.iter().find(|r| r.id == id)) else { return refused() };
        if row.deadline < now {
            return refused();
        }
        let Some(gold) = me.gold.checked_add(row.return_money) else { return refused() };
        let write = MarketWrite {
            character: me.id,
            money: i64::from(row.return_money),
            ops: vec![MarketOp::DeleteMoneyRow(id)],
            ..MarketWrite::default()
        };
        let mut planned = Planned::new(write, Then::CollectMoney { id, gold, gained: row.return_money });
        planned.gold = Some(gold);
        Ok(planned)
    }

    /// `0x1C15` (section 11): the answer, and whether the list changed and is written back. `list` is `None` while the character
    /// has none.
    pub fn interest_add(&self, list: &mut Option<Vec<u32>>, id: u32) -> (Vec<u8>, bool) {
        let Some(a) = self.auctions.get(&id) else { return (mp::interest_added(Err(1)), false) };
        let list = list.get_or_insert_with(Vec::new);
        list.retain(|id| self.auctions.contains_key(id));
        if list.contains(&id) {
            return (mp::interest_added(Err(2)), false);
        }
        if list.len() >= INTEREST_MAX {
            return (mp::interest_added(Err(3)), false);
        }
        list.push(id);
        (mp::interest_added(Ok(&market_row(a))), true)
    }

    /// The auctions past their end (section 10), from the lowest id: one plan each, so a failed write affects only its own.
    pub fn expire(&self, now: i64) -> Vec<Planned> {
        let deadline = deadline(now);
        self.auctions
            .values()
            .filter(|a| a.ends_at <= now)
            .map(|a| match &a.bidder {
                None => {
                    let write = MarketWrite {
                        ops: vec![MarketOp::EndAuction { id: a.id, character: a.seller, result: RESULT_BACK, deadline }],
                        ..MarketWrite::default()
                    };
                    Planned::new(write, Then::Unsold { id: a.id, seller: a.seller })
                }
                Some((bidder, _)) => {
                    let ops = vec![
                        MarketOp::EndAuction { id: a.id, character: *bidder, result: RESULT_WON, deadline },
                        sale(a, a.price, deadline),
                    ];
                    Planned::new(MarketWrite { ops, ..MarketWrite::default() }, Then::Sold { id: a.id, seller: a.seller, bidder: *bidder })
                }
            })
            .collect()
    }

    /// The rows past their deadline, deleted together (deviation 1); `None` when there are none.
    pub fn purge(&self, now: i64) -> Option<Planned> {
        let mut items: Vec<(u32, u32)> =
            self.items.values().flatten().filter(|r| r.deadline < now).map(|r| (r.character, r.market_id)).collect();
        let mut money: Vec<(u32, u32, u32)> =
            self.money.values().flatten().filter(|r| r.deadline < now).map(|r| (r.character, r.id, r.return_money)).collect();
        if items.is_empty() && money.is_empty() {
            return None;
        }
        items.sort_unstable_by_key(|r| r.1);
        money.sort_unstable_by_key(|r| r.1);
        let ops = items.iter().map(|r| MarketOp::DeleteItemRow(r.1)).chain(money.iter().map(|r| MarketOp::DeleteMoneyRow(r.1))).collect();
        Some(Planned::new(MarketWrite { ops, ..MarketWrite::default() }, Then::Purge { items, money }))
    }

    /// `/auctionrecall` (section 12.1, deviation 3): every auction of the seller `name` (id 0), or that one if it is the seller's. Each
    /// item becomes the GM's (result 2), a bidder is refunded and the seller gets its deposit back. `None` when nothing matches.
    pub fn recall(&self, gm: u32, name: &[u8], id: u32, now: i64) -> Option<Planned> {
        let deadline = deadline(now);
        let recalled: Vec<&StoredAuction> =
            self.auctions.values().filter(|a| (id == 0 || a.id == id) && a.seller_name.as_bytes() == name).collect();
        if recalled.is_empty() {
            return None;
        }
        let mut ops = Vec::new();
        let mut refunds = Vec::new();
        for a in &recalled {
            ops.push(MarketOp::EndAuction { id: a.id, character: gm, result: RESULT_RECALLED, deadline });
            let bidder = a.bidder.as_ref().map(|b| b.0);
            if let Some(bidder) = bidder {
                ops.push(refund(a.id, bidder, a.price, deadline, &a.item));
            }
            refunds.push(bidder);
            ops.push(MarketOp::NewMoneyRow(NewMoneyRow {
                market_id: a.id,
                character: a.seller,
                result: RESULT_BACK,
                money: 0,
                deposit: a.deposit,
                return_money: a.deposit,
                deadline,
                item: a.item.clone(),
            }));
        }
        let ids = recalled.iter().map(|a| a.id).collect();
        Some(Planned::new(MarketWrite { ops, ..MarketWrite::default() }, Then::Recall { gm, ids, refunds }))
    }

    /// What the game log's records need of the market as it is before `planned` is applied: the auctions its ops end or bid on and
    /// the gold row it collects (`docs/original/game-log.md` section 3.2).
    pub fn log_view(&self, planned: &Planned) -> LogView {
        let mut view = LogView::default();
        for op in &planned.write.ops {
            if let MarketOp::EndAuction { id, .. } | MarketOp::SetBid { id, .. } = op
                && let Some(a) = self.auctions.get(id)
            {
                view.auctions.insert(*id, a.clone());
            }
        }
        if let Then::CollectMoney { id, .. } = planned.then {
            view.money = self.money.values().flatten().find(|r| r.id == id).cloned();
        }
        view
    }

    /// The written change in memory, as the store made it, then the answer and the notices.
    pub fn apply(&mut self, planned: &Planned, written: &MarketWritten) -> Applied {
        let mut auction_ids = written.auction_ids.iter();
        let mut new_money = Vec::new();
        let mut money_ids = written.money_ids.iter();
        for op in &planned.write.ops {
            match op {
                MarketOp::NewAuction(n) => {
                    let Some(&id) = auction_ids.next() else { continue };
                    let a = StoredAuction {
                        id,
                        seller: n.seller,
                        seller_name: n.seller_name.clone(),
                        ends_at: n.ends_at,
                        min: n.min,
                        direct: n.direct,
                        period: n.period,
                        deposit: n.deposit,
                        bidder: None,
                        price: n.min,
                        item: n.item.clone(),
                    };
                    self.auctions.insert(id, a);
                }
                MarketOp::EndAuction { id, character, result, deadline } => {
                    let Some(a) = self.auctions.remove(id) else { continue };
                    let row = StoredItemRow { market_id: *id, character: *character, result: *result, deadline: *deadline, item: a.item };
                    self.items.entry(*character).or_default().push(row);
                }
                MarketOp::SetBid { id, bidder, name, price } => {
                    if let Some(a) = self.auctions.get_mut(id) {
                        a.bidder = Some((*bidder, name.clone()));
                        a.price = *price;
                    }
                }
                MarketOp::NewMoneyRow(n) => {
                    let Some(&id) = money_ids.next() else { continue };
                    let row = StoredMoneyRow {
                        id,
                        market_id: n.market_id,
                        character: n.character,
                        result: n.result,
                        money: n.money,
                        deposit: n.deposit,
                        return_money: n.return_money,
                        deadline: n.deadline,
                        item: n.item.clone(),
                    };
                    new_money.push(row.clone());
                    self.money.entry(n.character).or_default().push(row);
                }
                MarketOp::DeleteItemRow(id) => self.remove_item_row(*id),
                MarketOp::DeleteMoneyRow(id) => {
                    for rows in self.money.values_mut() {
                        rows.retain(|r| r.id != *id);
                    }
                    self.money.retain(|_, rows| !rows.is_empty());
                }
            }
        }
        let placed = planned.write.place.map(|(id, ..)| id);
        let placed_row = placed.and_then(|id| self.items.get(&planned.write.character)?.iter().find(|r| r.market_id == id).cloned());
        if let Some(id) = placed {
            self.remove_item_row(id);
        }
        let item_row = |market: &Self, character: u32, id: u32| {
            market.items.get(&character).and_then(|rows| rows.iter().find(|r| r.market_id == id)).map(end_item).unwrap_or_default()
        };
        let money = |i: usize| new_money.get(i).map(end_money).unwrap_or_default();
        let mut out = Applied::default();
        match &planned.then {
            Then::Register { bag, slot, count, gold } => {
                if let Some(a) = written.auction_ids.first().and_then(|id| self.auctions.get(id)) {
                    out.answer.push(mp::register_ok(*bag, *slot, *count, *gold, &sell_row(a)));
                }
            }
            Then::Cancel { id, seller, refund } => {
                out.answer.push(mp::cancel(Some(&item_row(self, *seller, *id))));
                if let Some(bidder) = refund {
                    out.notices.push((*bidder, mp::outbid(&money(0))));
                }
            }
            Then::Buy { id, gold, seller, refund } => {
                out.answer.push(mp::buy(Ok((*gold, &item_row(self, planned.write.character, *id)))));
                out.notices.push((*seller, mp::sold(&money(0))));
                if let Some(bidder) = refund {
                    out.notices.push((*bidder, mp::outbid(&money(1))));
                }
            }
            Then::Bid { id, gold, seller, refund } => {
                if let Some(bidder) = refund {
                    out.notices.push((*bidder, mp::outbid(&money(0))));
                }
                if let Some(a) = self.auctions.get(id) {
                    out.notices.push((*seller, mp::bid_on(*id, a.price, a.item.item_type, a.item.type_id, a.item.count)));
                    out.answer.push(mp::bid_ok(*id, *gold, &market_row(a)));
                }
            }
            Then::CollectItem { id, bag, slot } => {
                if let Some(row) = &placed_row {
                    out.answer.push(mp::got_item(Some((*id, *bag, *slot, &row.item))));
                }
            }
            Then::CollectMoney { id, gold, .. } => out.answer.push(mp::got_money(Some((*id, *gold)))),
            Then::Unsold { id, seller } => out.notices.push((*seller, mp::unsold(&item_row(self, *seller, *id)))),
            Then::Sold { id, seller, bidder } => {
                out.notices.push((*bidder, mp::won(&item_row(self, *bidder, *id))));
                out.notices.push((*seller, mp::sold(&money(0))));
            }
            Then::Purge { .. } => {}
            Then::Recall { gm, ids, refunds } => {
                let rows: Vec<EndItem> = ids.iter().map(|id| item_row(self, *gm, *id)).collect();
                out.answer.extend(mp::end_items(&rows));
                // Each auction made its refund (if any) and then the deposit row.
                let mut at = 0;
                for bidder in refunds {
                    if let Some(bidder) = bidder {
                        out.notices.push((*bidder, mp::outbid(&money(at))));
                        at += 1;
                    }
                    at += 1;
                }
            }
        }
        out
    }

    fn remove_item_row(&mut self, id: u32) {
        for rows in self.items.values_mut() {
            rows.retain(|r| r.market_id != id);
        }
        self.items.retain(|_, rows| !rows.is_empty());
    }
}

/// `0x1C16` (section 11): the id removed if there; result 0 with a list (even unchanged), 1 without one. Whether to write it back.
pub fn interest_remove(list: &mut Option<Vec<u32>>, id: u32) -> (Vec<u8>, bool) {
    match list {
        Some(list) => {
            list.retain(|&i| i != id);
            (mp::interest_removed(id, 0), true)
        }
        None => (mp::interest_removed(id, 1), false),
    }
}

/// `0x1C17`: likewise.
pub fn interest_clear(list: &mut Option<Vec<u32>>) -> (Vec<u8>, bool) {
    match list {
        Some(list) => {
            list.clear();
            (mp::interest_cleared(0), true)
        }
        None => (mp::interest_cleared(1), false),
    }
}

/// The market as the server shares it: writes take the turn (one at a time, from the plan to the apply); the state is locked only
/// to plan, apply or read, never across a store call.
#[derive(Default)]
pub struct House {
    pub turn: tokio::sync::Mutex<()>,
    pub state: std::sync::Mutex<Market>,
}

/// Now, in microseconds since 1970.
pub fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_micros() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use shaiya_proto::frame::{opcode, payload};

    const NOW: i64 = 1_800_000_000 * MICROS;
    const SELLER: u32 = 1;
    const BUYER: u32 = 2;
    const OTHER: u32 = 3;

    fn potion_def() -> ItemDef {
        ItemDef { item_id: 100_001, item_type: 100, type_id: 1, max_count: 255, country: 6, ..Default::default() }
    }

    fn defs() -> ItemDefs {
        let mut sword = ItemDef {
            item_id: 2001,
            item_type: 2,
            type_id: 1,
            max_count: 1,
            quality: 900,
            level: 30,
            country: 3,
            req_dex: 2,
            ..Default::default()
        };
        let lapis = ItemDef { item_id: 30_001, item_type: 30, type_id: 1, max_count: 1, country: 4, ..Default::default() };
        let bound = ItemDef { item_id: 2002, item_type: 2, type_id: 2, max_count: 1, bound: true, ..Default::default() };
        let quest = ItemDef { item_id: 27_001, item_type: 27, type_id: 1, max_count: 1, ..Default::default() };
        sword.craftable = false;
        ItemDefs::new([potion_def(), sword, lapis, bound, quest])
    }

    fn names() -> HashMap<(u8, u8), Vec<u8>> {
        [((100, 1), b"Etain Potion".to_vec()), ((2, 1), b"Long Sword".to_vec()), ((30, 1), b"Lapis".to_vec())].into_iter().collect()
    }

    fn item(item_type: u8, type_id: u8, count: u8) -> MarketItem {
        MarketItem { item_type, type_id, count, ..Default::default() }
    }

    fn auction(id: u32, seller: u32, item: MarketItem) -> StoredAuction {
        StoredAuction {
            id,
            seller,
            seller_name: format!("Seller{seller}"),
            ends_at: NOW + 3600 * MICROS,
            min: 1000,
            direct: 5000,
            period: 1,
            deposit: 120,
            bidder: None,
            price: 1000,
            item,
        }
    }

    fn market(auctions: Vec<StoredAuction>) -> Market {
        Market::new(1, names(), (auctions, Vec::new(), Vec::new()))
    }

    #[test]
    fn a_renamed_character_sells_and_bids_under_its_new_name() {
        // docs/original/small-items.md deviation 5.
        let bid = StoredAuction { bidder: Some((SELLER, "Seller1".into())), ..auction(2, OTHER, item(100, 1, 1)) };
        let mut m = market(vec![auction(1, SELLER, item(100, 1, 1)), bid]);
        m.renamed(SELLER, "Renamed");
        assert_eq!(m.auctions[&1].seller_name, "Renamed");
        assert_eq!((m.auctions[&2].seller_name.as_str(), m.auctions[&2].bidder.clone()), ("Seller3", Some((SELLER, "Renamed".into()))));
    }

    /// The store's part: ids for the inserts, counting on from what the market has.
    fn write(market: &mut Market, planned: &Planned) -> Applied {
        let mut next_auction = market.auctions.keys().max().copied().unwrap_or(0) + 100;
        let mut next_money = market.money.values().flatten().map(|r| r.id).max().unwrap_or(0) + 1;
        let mut written = MarketWritten::default();
        for op in &planned.write.ops {
            match op {
                MarketOp::NewAuction(_) => {
                    written.auction_ids.push(next_auction);
                    next_auction += 1;
                }
                MarketOp::NewMoneyRow(_) => {
                    written.money_ids.push(next_money);
                    next_money += 1;
                }
                _ => {}
            }
        }
        market.apply(planned, &written)
    }

    fn me(id: u32, gold: u32) -> Requester<'static> {
        Requester { id, name: ["", "Seller1", "Buyer", "Other"][id as usize], gold }
    }

    /// With the opcode, as the PDB sizes count it.
    fn size(packet: &[u8]) -> usize {
        payload(packet).len() + 2
    }

    fn rows(packet: &[u8]) -> Vec<u32> {
        let b = payload(packet);
        (0..usize::from(b[2])).map(|i| u32::from_le_bytes(b[3 + 48 * i..7 + 48 * i].try_into().unwrap())).collect()
    }

    fn flags(packet: &[u8]) -> (u8, u8) {
        (payload(packet)[0], payload(packet)[1])
    }

    fn answer(r: Result<Planned, Refusal>) -> Vec<u8> {
        match r {
            Err(Refusal::Answer(p)) => p,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_arithmetic_follows_the_original() {
        assert_eq!((fee(0, 500), fee(3, 500_000), fee(1, 500_001)), (35, 840, 240));
        assert_eq!(fee(3, 3_800_000_000), 6_384_000);
        assert_eq!(sale_return(120, 10_000), 9_920);
        assert_eq!(sale_return(120, 5000), 5020);
        assert_eq!(sale_return(35, 4_000_000_000), 35 + 4_000_000_000 - 80_000_000, "the deposit less the commission is negative");
        assert_eq!((next_min(1000), next_min(500), next_min(1050)), (1050, 525, 1102));
        assert_eq!(next_min(4_000_000_000), 4_200_000_000);
        assert_eq!(next_min(u32::MAX), u32::MAX, "saturates");
        assert_eq!((category_of(2), category_of(100), category_of(30), category_of(26)), (1, 25, 21, 0));
    }

    #[test]
    fn pages_walk_down_and_back_up() {
        let defs = defs();
        let mut m = market((1..=40).map(|id| auction(id, SELLER, item(100, 1, 1))).collect());
        let mut s = Search::new(Mode::Item(100_001));
        let p1 = m.first(&mut s, &defs);
        assert_eq!(opcode(&p1), mp::OP_SAME_ITEM);
        assert_eq!((rows(&p1), flags(&p1)), ((24..=40).rev().collect(), (0, 1)));
        let p2 = m.page(&mut s, 2, &defs).unwrap();
        assert_eq!(opcode(&p2), mp::OP_PAGE);
        assert_eq!((rows(&p2), flags(&p2)), ((7..=23).rev().collect(), (1, 1)));
        let p3 = m.page(&mut s, 2, &defs).unwrap();
        assert_eq!((rows(&p3), flags(&p3)), ((1..=6).rev().collect(), (1, 0)));
        assert_eq!(m.page(&mut s, 2, &defs), None, "no next");
        let back = m.page(&mut s, 1, &defs).unwrap();
        assert_eq!((rows(&back), flags(&back)), ((7..=23).collect(), (1, 1)), "page 2 in rising order");
        let top = m.page(&mut s, 1, &defs).unwrap();
        assert_eq!((rows(&top), flags(&top)), ((24..=40).collect(), (0, 1)));
        // Refresh: from the page's lowest id up to the previous cursor (none now: a new first page).
        assert_eq!(opcode(&m.page(&mut s, 0, &defs).unwrap()), mp::OP_SAME_ITEM);
        let _ = m.page(&mut s, 2, &defs).unwrap();
        let refreshed = m.page(&mut s, 0, &defs).unwrap();
        assert_eq!((opcode(&refreshed), rows(&refreshed)), (mp::OP_PAGE, (7..=24).collect()));

        // The next cursor's auction gone: the walk starts at the next lower id.
        let mut s = Search::new(Mode::Item(100_001));
        m.first(&mut s, &defs);
        assert_eq!(s.next_cursor, 23);
        m.auctions.remove(&23);
        m.auctions.remove(&22);
        let p2 = m.page(&mut s, 2, &defs).unwrap();
        assert_eq!(rows(&p2)[0], 21);
        assert_eq!(m.page(&mut Search::new(Mode::Item(100_001)), 1, &defs), None, "no previous");
    }

    #[test]
    fn the_filters_follow_the_original() {
        let defs = defs();
        let mut sword = auction(1, SELLER, item(2, 1, 1));
        sword.seller_name = "Alice".into();
        let m = market(vec![sword, auction(2, OTHER, item(100, 1, 5)), auction(3, OTHER, item(30, 1, 1))]);
        let search = |country, level_min, level_max, category, grade, name: &[u8]| {
            let filter = mp::Search { country, level_min, level_max, category, grade, name: name.to_vec() };
            rows(&m.first(&mut Search::new(Mode::Normal(filter)), &defs))
        };
        assert_eq!(search(0, 0, 0, 0, 255, b""), [3, 2, 1], "level max 0 is 255");
        assert_eq!(search(1, 0, 0, 0, 255, b""), [3, 2], "Light hides 3 to 5, lapis passes, 6 passes");
        assert_eq!(search(2, 0, 0, 0, 255, b""), [3, 2, 1]);
        assert_eq!(search(0, 31, 99, 0, 255, b""), Vec::<u32>::new());
        assert_eq!(search(0, 30, 30, 0, 255, b""), [1], "the level's low byte");
        assert_eq!(search(0, 0, 0, 1, 255, b""), [1], "category 1 holds type 2");
        assert_eq!(search(0, 0, 0, 25, 255, b""), [2], "category 25 holds type 100");
        assert_eq!(search(0, 0, 0, 0, 2, b""), [1], "grade: ReqDex");
        assert_eq!(search(0, 0, 0, 0, 255, b"potion"), [2], "any case (deviation 2)");
        assert_eq!(search(0, 0, 0, 0, 255, b"g sw"), [1]);
        let item = |s| rows(&m.first(&mut Search::new(Mode::Item(s)), &defs));
        assert_eq!(item(2001), [1], "the other filters ignored");
        let seller = |name: &[u8]| rows(&m.first(&mut Search::new(Mode::Seller(name.to_vec())), &defs));
        assert_eq!((seller(b"Alice"), seller(b"alice")), (vec![1], vec![]), "exact name");
    }

    #[test]
    fn registering_takes_the_fee_and_the_item() {
        let defs = defs();
        let mut m = market(Vec::new());
        let bags = Inventory::new(&[
            ItemUnit { bag: 1, slot: 0, item_type: 100, type_id: 1, count: 10, ..Default::default() },
            ItemUnit { bag: 1, slot: 1, item_type: 2, type_id: 2, count: 1, ..Default::default() },
            ItemUnit { bag: 1, slot: 2, item_type: 27, type_id: 1, count: 1, ..Default::default() },
            ItemUnit { bag: 1, slot: 3, item_type: 2, type_id: 1, quality: 700, gems: [1, 0, 0, 0, 0, 0], count: 1, ..Default::default() },
        ]);
        let req = |bag, slot, count, min, direct| mp::Register { bag, slot, count, period: 1, min, direct };
        let reg = |r, gold| market(Vec::new()).register(me(SELLER, gold), &bags, &defs, r, NOW);
        for r in [
            req(0, 0, 1, 1000, 5000),
            req(3, 0, 1, 1000, 5000),
            req(1, 9, 1, 1000, 5000),
            req(1, 0, 11, 1000, 5000),
            req(1, 0, 0, 1000, 5000),
        ] {
            assert_eq!(reg(r, 1000), Err(Refusal::Ignore), "{r:?}");
        }
        for (r, gold) in [
            (req(1, 0, 3, 499, 5000), 1000),
            (req(1, 0, 3, 5000, 5000), 1000),
            (req(1, 0, 3, 1000, 4_000_000_001), 1000),
            (req(1, 0, 3, 3_800_000_001, 3_900_000_000), 100_000),
            (req(1, 0, 3, 1000, 5000), 119),
            (req(1, 1, 1, 1000, 5000), 1000),
            (req(1, 2, 1, 1000, 5000), 1000),
        ] {
            assert_eq!(answer(reg(r, gold)), mp::register_failed(), "{r:?}");
        }
        assert!(reg(mp::Register { period: 9, ..req(1, 0, 3, 1000, 5000) }, 840).is_ok(), "a period above 3 is 3: a fee of 840");

        let planned = reg(req(1, 0, 3, 1000, 5000), 1000).unwrap();
        assert_eq!((planned.gold, planned.write.money, planned.write.take), (Some(880), -120, Some((1, 0, 10, 3))));
        assert_eq!(planned.bag_change, Some(SlotChange::SetCount { bag: 1, slot: 0, count: 7 }));
        let applied = write(&mut m, &planned);
        let p = &applied.answer[0];
        assert_eq!(size(p), 62);
        let b = payload(p);
        assert_eq!((&b[..4], u32::from_le_bytes(b[4..8].try_into().unwrap())), (&[0, 1, 0, 3][..], 880));
        let a = m.auctions.values().next().unwrap();
        assert_eq!((a.price, a.deposit, a.item.count, a.ends_at), (1000, 120, 3, NOW + 24 * 3600 * MICROS));

        let whole = reg(req(1, 3, 1, 1000, 5000), 1000).unwrap();
        assert_eq!(whole.bag_change, Some(SlotChange::Remove { bag: 1, slot: 3 }));
        let MarketOp::NewAuction(n) = &whole.write.ops[0] else { panic!() };
        assert_eq!((n.item.quality, n.item.gems[0]), (700, 1), "the whole item as it is");
    }

    #[test]
    fn cancel_keeps_the_deposit_and_refunds_the_bidder() {
        let mut a = auction(5, SELLER, item(100, 1, 3));
        a.bidder = Some((BUYER, "Buyer".into()));
        a.price = 1050;
        let mut m = market(vec![a, auction(6, OTHER, item(100, 1, 1))]);
        assert_eq!(answer(m.cancel(SELLER, 6, NOW)), mp::cancel(None), "not the seller's");
        assert_eq!(answer(m.cancel(SELLER, 7, NOW)), mp::cancel(None));
        let planned = m.cancel(SELLER, 5, NOW).unwrap();
        assert_eq!(planned.write.money, 0, "the deposit is not returned");
        let applied = write(&mut m, &planned);
        assert_eq!(size(&applied.answer[0]), 44);
        assert_eq!(m.items[&SELLER][0].result, RESULT_BACK);
        let refund = &m.money[&BUYER][0];
        assert_eq!((refund.result, refund.money, refund.return_money, refund.deadline), (1, 0, 1050, NOW + DEADLINE_SECS * MICROS));
        assert_eq!(applied.notices, [(BUYER, mp::outbid(&end_money(refund)))]);
        assert!(!m.auctions.contains_key(&5));
    }

    #[test]
    fn buying_now_pays_the_seller_and_refunds_the_bidder() {
        let mut a = auction(5, SELLER, item(100, 1, 3));
        a.bidder = Some((BUYER, "Buyer".into()));
        a.price = 1050;
        let mut m = market(vec![a]);
        assert_eq!(answer(m.buy(me(BUYER, 9000), 6, NOW)), mp::buy(Err(1)));
        assert_eq!(answer(m.buy(me(SELLER, 4999), 5, NOW)), mp::buy(Err(3)), "the gold before the seller");
        assert_eq!(answer(m.buy(me(SELLER, 5000), 5, NOW)), mp::buy(Err(2)));
        // The top bidder buys: it pays the full price and gets its bid back.
        let planned = m.buy(me(BUYER, 8950), 5, NOW).unwrap();
        assert_eq!((planned.gold, planned.write.money), (Some(3950), -5000));
        let applied = write(&mut m, &planned);
        assert_eq!((size(&applied.answer[0]), opcode(&applied.answer[0])), (48, mp::OP_BUY));
        let sale = &m.money[&SELLER][0];
        assert_eq!((sale.result, sale.money, sale.deposit, sale.return_money), (0, 5000, 120, 5020));
        let refund = &m.money[&BUYER][0];
        assert_eq!((refund.result, refund.return_money), (1, 1050));
        assert_eq!(applied.notices, [(SELLER, mp::sold(&end_money(sale))), (BUYER, mp::outbid(&end_money(refund)))]);
        assert_eq!((m.items[&BUYER][0].result, m.items[&BUYER][0].item.count), (0, 3));
    }

    #[test]
    fn an_auction_past_its_end_takes_neither_a_bid_nor_a_buy() {
        // Audit R20: the end pass runs every 60 s; in between the auction is closed already.
        let m = market(vec![auction(5, SELLER, item(100, 1, 3))]);
        let ends = NOW + 3600 * MICROS;
        assert!(m.buy(me(BUYER, 9000), 5, ends - 1).is_ok(), "one microsecond before its end");
        assert_eq!(answer(m.buy(me(BUYER, 9000), 5, ends)), mp::buy(Err(1)));
        assert_eq!(answer(m.buy(me(BUYER, 9000), 5, ends + 30 * MICROS)), mp::buy(Err(1)));
        assert!(m.bid(me(BUYER, 9000), 5, 1050, ends - 1).is_ok());
        assert_eq!(answer(m.bid(me(BUYER, 9000), 5, 1050, ends)), mp::bid_refused(5, 1));
        assert_eq!(answer(m.bid(me(BUYER, 9000), 5, 1050, ends + 30 * MICROS)), mp::bid_refused(5, 1));
    }

    #[test]
    fn bids_follow_the_original_order() {
        let mut m = market(vec![auction(5, SELLER, item(100, 1, 3))]);
        let bid = |m: &Market, who, gold, id, price| m.bid(me(who, gold), id, price, NOW);
        assert_eq!(answer(bid(&m, BUYER, 9000, 6, 2000)), mp::bid_refused(6, 1));
        assert_eq!(answer(bid(&m, SELLER, 9000, 5, 2000)), mp::bid_refused(5, 2));
        assert_eq!(answer(bid(&m, BUYER, 999, 5, 1000)), mp::bid_refused(5, 4));
        assert_eq!(answer(bid(&m, BUYER, 9000, 5, 1000)), mp::bid_too_low(5, 1050), "the minimum itself is too low");
        assert_eq!(bid(&m, BUYER, 9000, 5, 5000), Err(Refusal::Ignore), "at the buy-now price: no answer");
        let planned = bid(&m, BUYER, 9000, 5, 1050).unwrap();
        let applied = write(&mut m, &planned);
        assert_eq!((size(&applied.answer[0]), planned.gold), (59, Some(7950)));
        assert_eq!(applied.notices, [(SELLER, mp::bid_on(5, 1050, 100, 1, 3))]);
        assert_eq!(answer(bid(&m, BUYER, 9000, 5, 2000)), mp::bid_refused(5, 3), "already the top bidder");
        assert_eq!(answer(bid(&m, OTHER, 9000, 5, 1100)), mp::bid_too_low(5, 1102));
        let planned = bid(&m, OTHER, 9000, 5, 1102).unwrap();
        let applied = write(&mut m, &planned);
        let refund = &m.money[&BUYER][0];
        assert_eq!((refund.result, refund.return_money), (1, 1050));
        assert_eq!(applied.notices[0], (BUYER, mp::outbid(&end_money(refund))));
        assert_eq!(m.auctions[&5].bidder, Some((OTHER, "Other".into())));
        assert_eq!(payload(&m.bid_list(OTHER)[0])[0], 1);
        assert_eq!(payload(&m.bid_list(BUYER)[0])[0], 0);
    }

    #[test]
    fn collecting_needs_room_the_deadline_and_32_bits() {
        let row = StoredItemRow { market_id: 5, character: BUYER, result: 0, deadline: NOW, item: item(100, 1, 3) };
        let late = StoredItemRow { market_id: 6, deadline: NOW - 1, ..row.clone() };
        let gold = |id, deadline| StoredMoneyRow {
            id,
            market_id: 5,
            character: BUYER,
            result: 0,
            money: 5000,
            deposit: 120,
            return_money: 5020,
            deadline,
            item: item(100, 1, 3),
        };
        let mut m = Market::new(1, names(), (Vec::new(), vec![row, late], vec![gold(1, NOW), gold(2, NOW - 1)]));
        let full: Vec<ItemUnit> = (1..=2)
            .flat_map(|bag| (0..24).map(move |slot| ItemUnit { bag, slot, item_type: 100, type_id: 1, count: 1, ..Default::default() }))
            .collect();
        assert_eq!(answer(m.collect_item(BUYER, &Inventory::new(&full), 5, NOW)), mp::got_item(None), "full bags");
        let bags = Inventory::new(&full[..3]);
        assert_eq!(answer(m.collect_item(BUYER, &bags, 6, NOW)), mp::got_item(None), "past its deadline");
        assert_eq!(answer(m.collect_item(OTHER, &bags, 5, NOW)), mp::got_item(None), "not its row");
        assert_eq!(payload(&m.end_items(BUYER, NOW)[0])[0], 1, "the late row is not listed");
        let planned = m.collect_item(BUYER, &bags, 5, NOW).unwrap();
        assert_eq!(planned.write.place, Some((5, 1, 3)), "the first free place");
        let applied = write(&mut m, &planned);
        assert_eq!(size(&applied.answer[0]), 41);
        assert_eq!(planned.received.as_ref().map(|i| (i.bag, i.slot, i.count)), Some((1, 3, 3)));
        assert_eq!(m.items[&BUYER].len(), 1);

        assert_eq!(answer(m.collect_money(me(BUYER, u32::MAX - 5019), 1, NOW)), mp::got_money(None), "past u32");
        assert_eq!(answer(m.collect_money(me(BUYER, 0), 2, NOW)), mp::got_money(None), "past its deadline (deviation 1)");
        assert_eq!(payload(&m.end_moneys(BUYER, NOW)[0])[0], 1);
        let planned = m.collect_money(me(BUYER, u32::MAX - 5020), 1, NOW).unwrap();
        assert_eq!((planned.gold, planned.write.money), (Some(u32::MAX), 5020));
        let applied = write(&mut m, &planned);
        assert_eq!(applied.answer, [mp::got_money(Some((1, u32::MAX)))]);
        assert_eq!(m.money[&BUYER].len(), 1);
    }

    #[test]
    fn the_end_pass_ends_and_purges() {
        let mut sold = auction(5, SELLER, item(100, 1, 3));
        sold.bidder = Some((BUYER, "Buyer".into()));
        sold.price = 1050;
        sold.ends_at = NOW;
        let unsold = StoredAuction { ends_at: NOW - 1, ..auction(4, SELLER, item(2, 1, 1)) };
        let running = auction(6, SELLER, item(100, 1, 1));
        let old = StoredItemRow { market_id: 1, character: OTHER, result: 1, deadline: NOW - 1, item: item(2, 1, 1) };
        let mut m = Market::new(1, names(), (vec![sold, unsold, running], vec![old], Vec::new()));
        let plans = m.expire(NOW);
        assert_eq!(
            plans.iter().map(|p| p.then.clone()).collect::<Vec<_>>(),
            [Then::Unsold { id: 4, seller: SELLER }, Then::Sold { id: 5, seller: SELLER, bidder: BUYER }]
        );
        let unsold = write(&mut m, &plans[0]);
        assert_eq!(unsold.notices[0].0, SELLER);
        assert_eq!((opcode(&unsold.notices[0].1), size(&unsold.notices[0].1)), (mp::OP_UNSOLD, 43));
        assert!(!m.money.contains_key(&SELLER), "the deposit is lost");
        let sold = write(&mut m, &plans[1]);
        assert_eq!(sold.notices.iter().map(|(c, p)| (*c, opcode(p))).collect::<Vec<_>>(), [(BUYER, mp::OP_WON), (SELLER, mp::OP_SOLD)]);
        assert_eq!(m.money[&SELLER][0].return_money, 120 + 1050 - 21);
        assert_eq!(m.auctions.keys().collect::<Vec<_>>(), [&6]);

        let purge = m.purge(NOW).unwrap();
        assert_eq!(purge.write.ops, [MarketOp::DeleteItemRow(1)]);
        write(&mut m, &purge);
        assert!(!m.items.contains_key(&OTHER));
        assert_eq!(m.purge(NOW + DEADLINE_SECS * MICROS), None, "deadlines now + 14 days are not past yet");
    }

    #[test]
    fn the_interest_list_follows_the_original() {
        let m = market((1..=12).map(|id| auction(id, SELLER, item(100, 1, 1))).collect());
        let mut list = None;
        assert_eq!(interest_remove(&mut list, 1), (mp::interest_removed(1, 1), false), "never had one");
        assert_eq!(interest_clear(&mut list).0, mp::interest_cleared(1));
        assert_eq!(m.interest_add(&mut list, 99), (mp::interest_added(Err(1)), false));
        assert_eq!(size(&m.interest_add(&mut list, 1).0), 51);
        assert_eq!(m.interest_add(&mut list, 1), (mp::interest_added(Err(2)), false));
        for id in 2..=10 {
            assert!(m.interest_add(&mut list, id).1);
        }
        assert_eq!(m.interest_add(&mut list, 11), (mp::interest_added(Err(3)), false), "an eleventh");
        let mut list = Some(vec![50, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
        assert!(m.interest_add(&mut list, 11).1, "a gone id is removed first");
        assert_eq!(list.as_deref(), Some(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 11][..]));
        assert_eq!(interest_remove(&mut list, 77), (mp::interest_removed(77, 0), true), "0 even for an id not in it");
        let b = payload(&m.interest_list(&[3, 99, 1])).to_vec();
        assert_eq!((b[0], u32::from_le_bytes(b[1..5].try_into().unwrap()), u32::from_le_bytes(b[49..53].try_into().unwrap())), (2, 3, 1));
        assert_eq!(interest_clear(&mut list), (mp::interest_cleared(0), true));
        assert_eq!(list, Some(Vec::new()));
    }

    #[test]
    fn recall_gives_the_gm_the_item_and_the_seller_its_deposit() {
        let mut a = auction(5, SELLER, item(100, 1, 3));
        a.bidder = Some((BUYER, "Buyer".into()));
        a.price = 1050;
        let mut m = market(vec![a, auction(6, SELLER, item(2, 1, 1)), auction(7, OTHER, item(2, 1, 1))]);
        const GM: u32 = 9;
        assert_eq!(m.recall(GM, b"Seller1", 7, NOW), None, "not that seller's");
        assert_eq!(m.recall(GM, b"Nobody", 0, NOW), None);
        let planned = m.recall(GM, b"Seller1", 0, NOW).unwrap();
        let applied = write(&mut m, &planned);
        assert_eq!(applied.answer.len(), 1);
        assert_eq!((opcode(&applied.answer[0]), payload(&applied.answer[0])[0]), (mp::OP_END_ITEMS, 2));
        assert_eq!(m.items[&GM].iter().map(|r| (r.market_id, r.result)).collect::<Vec<_>>(), [(5, 2), (6, 2)]);
        let deposits: Vec<_> = m.money[&SELLER].iter().map(|r| (r.market_id, r.result, r.money, r.return_money)).collect();
        assert_eq!(deposits, [(5, 1, 0, 120), (6, 1, 0, 120)]);
        assert_eq!(m.money[&BUYER][0].return_money, 1050);
        assert_eq!(applied.notices, [(BUYER, mp::outbid(&end_money(&m.money[&BUYER][0])))]);
        assert_eq!(m.auctions.keys().collect::<Vec<_>>(), [&7]);
    }
}
