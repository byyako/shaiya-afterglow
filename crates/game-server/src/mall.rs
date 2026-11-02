//! The item mall's rules (`docs/original/item-mall.md`): the product list, where a purchase's items go, what a gift stores, the
//! item a storage row becomes. The connection's side is `server/mall.rs`.

use std::collections::HashMap;
use std::time::Duration;

use crate::backend::{Kept, NewItem};
use crate::inventory::{ItemDef, ItemDefs};
use shaiya_proto::point as pp;

/// Make types (sections 3, 5 and 6): the item mall's items and the billing storage's.
pub const MALL_MAKE_TYPE: u8 = b'X';
pub const BILLING_MAKE_TYPE: u8 = b'B';
/// `0x2605` asks the database once in this long; within it the points held are sent (section 2).
pub const RELOAD_EVERY: Duration = Duration::from_secs(40);

/// A product (`stProductInfo`): its price and the entries used (item id and count both non-zero), in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Product {
    pub code: String,
    pub price: u32,
    pub entries: Vec<(u32, u8)>,
}

/// The product list by code (exact, case-sensitive: `CGameData::GetProductInfo`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Products(HashMap<Vec<u8>, Product>);

impl Products {
    /// From `defs.product_list` rows `(code, price, 24 entries)`; a row whose first item id is 0 is left out
    /// (`CGameData::LoadProductList`).
    pub fn new(rows: impl IntoIterator<Item = (String, u32, Vec<(u32, u8)>)>) -> Self {
        let mut products = HashMap::new();
        for (code, price, entries) in rows {
            if entries.first().is_none_or(|&(item, _)| item == 0) {
                continue;
            }
            let entries = entries.into_iter().filter(|&(item, count)| item != 0 && count != 0).collect();
            products.insert(code.clone().into_bytes(), Product { code, price, entries });
        }
        Self(products)
    }

    pub fn get(&self, code: &[u8]) -> Option<&Product> {
        self.0.get(code)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// What a loading should warn about: entries whose item is not defined or has no stack size, and counts above a stack.
    pub fn problems(&self, defs: &ItemDefs) -> Vec<String> {
        let mut problems = Vec::new();
        for p in self.0.values() {
            for &(item, count) in &p.entries {
                match def_of(defs, item) {
                    None => problems.push(format!("{}: item {item} is not defined", p.code)),
                    Some(d) if count > d.max_count => {
                        problems.push(format!("{}: {count} of item {item} is above its stack of {}", p.code, d.max_count))
                    }
                    Some(_) => {}
                }
            }
        }
        problems.sort();
        problems
    }
}

/// An item id's definition (`CGameData::GetItemInfo`), only with a stack size above 0.
pub fn def_of(defs: &ItemDefs, item_id: u32) -> Option<&ItemDef> {
    let (item_type, type_id) = (u8::try_from(item_id / 1000).ok()?, u8::try_from(item_id % 1000).ok()?);
    defs.get(item_type, type_id).filter(|d| d.max_count > 0)
}

/// A new item of the mall or a storage: the definition's quality, no gems, the make type, zeros for the craft stats when craftable
/// (`CItem::Init`, `InitCraftExpansion`).
pub fn new_item(def: &ItemDef, count: u8, (bag, slot): (u8, u8), make_type: u8) -> NewItem {
    let kept = Kept { make_type, craft_name: crate::lapis::zero_craft_name(def), ..Kept::default() };
    NewItem {
        bag,
        slot,
        item_id: def.item_id,
        item_type: def.item_type,
        type_id: def.type_id,
        quality: def.quality,
        count,
        kept: Some(kept),
    }
}

/// Each entry cut into pieces of at most its stack size, in order; entries without a usable definition are skipped (section 3,
/// step 5).
fn pieces<'d>(product: &Product, defs: &'d ItemDefs) -> Vec<(&'d ItemDef, u8)> {
    let mut pieces = Vec::new();
    for &(item, count) in &product.entries {
        let Some(def) = def_of(defs, item) else { continue };
        let mut left = count;
        while left > 0 {
            let piece = left.min(def.max_count);
            pieces.push((def, piece));
            left -= piece;
        }
    }
    pieces
}

/// A purchase's items in `free` (the empty places, bags 1 up in order), or result 3: no piece at all, fewer places than pieces, or
/// more than 24 pieces (deviation 7; the answer holds 24).
pub fn plan_buy(product: &Product, defs: &ItemDefs, free: &[(u8, u8)]) -> Result<Vec<NewItem>, u8> {
    let pieces = pieces(product, defs);
    if pieces.is_empty() || pieces.len() > free.len() || pieces.len() > pp::BUY_ITEMS {
        return Err(pp::BUY_NO_ROOM);
    }
    Ok(pieces.into_iter().zip(free).map(|((def, count), &place)| new_item(def, count, place, MALL_MAKE_TYPE)).collect())
}

/// What a gift stores: one row per entry, a count above the stack size split into several (deviation 7); result 6 when an entry's
/// item is not usable (section 4, step 4).
pub fn gift_rows(product: &Product, defs: &ItemDefs) -> Result<Vec<(u32, u8)>, u8> {
    if product.entries.iter().any(|&(item, _)| def_of(defs, item).is_none()) {
        return Err(pp::GIFT_NO_PRODUCT);
    }
    Ok(pieces(product, defs).into_iter().map(|(def, count)| (def.item_id, count)).collect())
}

/// A storage row for the packets: its type and type id from the item id.
pub fn stored_unit(row: &crate::backend::StoredPointItem) -> pp::Stored {
    pp::Stored { slot: row.slot, item_type: (row.item_id / 1000) as u8, type_id: (row.item_id % 1000) as u8, count: row.count }
}

/// The log row of a purchase or gift for the packets.
pub fn log_unit(row: &crate::backend::PointLogRow) -> pp::LogRow {
    pp::LogRow { code: row.code.clone().into_bytes(), time: crate::admin::packed_from_unix(row.at), points: row.points }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defs() -> ItemDefs {
        let item = |item_type: u8, type_id: u8, max_count: u8| ItemDef {
            item_id: u32::from(item_type) * 1000 + u32::from(type_id),
            item_type,
            type_id,
            max_count,
            quality: 77,
            ..ItemDef::default()
        };
        ItemDefs::new([item(100, 42, 1), item(25, 1, 50), item(30, 1, 0)])
    }

    fn product(entries: Vec<(u32, u8)>) -> Product {
        Products::new([("Code".to_owned(), 100, entries)]).get(b"Code").cloned().unwrap()
    }

    #[test]
    fn the_list_skips_rows_and_entries_as_the_original() {
        let mut entries = vec![(0, 0); 24];
        entries[0] = (100042, 1);
        entries[3] = (25001, 0);
        let products = Products::new([("A".to_owned(), 5, entries), ("B".to_owned(), 5, vec![(0, 3), (100042, 1)])]);
        assert_eq!(products.len(), 1, "a row without a first item is left out");
        assert_eq!(products.get(b"A").unwrap().entries, [(100042, 1)]);
        assert!(products.get(b"a").is_none(), "codes are case-sensitive");
        assert_eq!(products.problems(&defs()), Vec::<String>::new());
        let odd = Products::new([("C".to_owned(), 5, vec![(25001, 60), (99001, 1)])]);
        assert_eq!(odd.problems(&defs()).len(), 2);
    }

    #[test]
    fn a_purchase_fills_empty_places_in_pieces() {
        let free = [(1, 3), (1, 7), (2, 0), (2, 1)];
        let items = plan_buy(&product(vec![(100042, 2), (25001, 60)]), &defs(), &free).unwrap();
        let placed: Vec<_> = items.iter().map(|i| (i.bag, i.slot, i.item_id, i.count, i.quality)).collect();
        assert_eq!(placed, [(1, 3, 100042, 1, 77), (1, 7, 100042, 1, 77), (2, 0, 25001, 50, 77), (2, 1, 25001, 10, 77)]);
        assert_eq!(items[0].kept.map(|k| k.make_type), Some(b'X'));
        assert_eq!(plan_buy(&product(vec![(100042, 2), (25001, 60)]), &defs(), &free[..3]), Err(pp::BUY_NO_ROOM));
        assert_eq!(plan_buy(&product(vec![(30001, 1)]), &defs(), &free), Err(pp::BUY_NO_ROOM), "nothing usable: no room, as the original");
        assert_eq!(plan_buy(&product(vec![(100042, 25)]), &defs(), &[(1, 0); 30]), Err(pp::BUY_NO_ROOM), "more than 24 pieces");
    }

    #[test]
    fn a_gift_stores_rows_within_the_stack() {
        assert_eq!(
            gift_rows(&product(vec![(100042, 1), (25001, 120)]), &defs()),
            Ok(vec![(100042, 1), (25001, 50), (25001, 50), (25001, 20)])
        );
        assert_eq!(gift_rows(&product(vec![(100042, 1), (30001, 1)]), &defs()), Err(pp::GIFT_NO_PRODUCT));
        let unit = stored_unit(&crate::backend::StoredPointItem { slot: 4, item_id: 100042, count: 1 });
        assert_eq!((unit.item_type, unit.type_id), (100, 42));
    }
}
