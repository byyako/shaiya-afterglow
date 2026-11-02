//! Lapis and enchanting (`docs/original/lapis.md`): linking, extraction, enchanting, new craft stats, remake and cloak badges.

use crate::frame::encode;
use crate::world::ItemUnit;

/// Client: link a lapis (`TPC_GEM_ADD`). Server: the result (`TP_GEM_ADD`).
pub const OP_LINK: u16 = 0x0801;
/// Client: take lapis out (`TPC_GEM_REMOVE`). Server: the result (`TP_GEM_REMOVE`).
pub const OP_EXTRACT: u16 = 0x0802;
/// Client: a badge on a cloak (`TPC_CLOAK_ADD`). Server: the result (`TP_CLOAK_ADD`).
pub const OP_BADGE: u16 = 0x0803;
/// Client and server: a cloak's badge taken off (`u8 bag, u8 slot`).
pub const OP_BADGE_REMOVE: u16 = 0x0804;
/// Client: enchant with a Lapisia (`TPC_LAPISIAN_ADD`). Server: the result (`TP_LAPISIAN_ADD`).
pub const OP_ENCHANT: u16 = 0x0805;
/// Client: new craft stats by Recreation Rune (`TPC_ITEM_COMPOSE`). Server: the result (`TP_ITEM_COMPOSE`).
pub const OP_COMPOSE: u16 = 0x0806;
/// Client: one item from three (`TPC_ITEM_REMAKE`). Server: the result (`TP_ITEM_REMAKE`).
pub const OP_REMAKE: u16 = 0x0807;
/// Client: craft stats for an item without any (`TPC_ITEM_CONSIDER`). Server: the result (`TP_ITEM_CONSIDER`).
pub const OP_CONSIDER: u16 = 0x0808;
const CRAFT_NAME_LEN: usize = 21;

/// `0x0801` from the client: lapis, item and hammer places.
pub fn parse_link(payload: &[u8]) -> Option<[(u8, u8); 3]> {
    let &[a, b, c, d, e, f] = payload else { return None };
    Some([(a, b), (c, d), (e, f)])
}

/// `0x0801`: `u8 success, u8 lapisBag, u8 lapisSlot, u8 lapis left, u8 itemBag, u8 itemSlot, u8 gem position, u8 lapis type id,
/// u8 0 × 4, u32 money, u8 hammerBag, u8 hammerSlot`.
pub fn linked(success: bool, lapis: ((u8, u8), u8), item: (u8, u8), gem: (u8, u8), money: u32, hammer: (u8, u8)) -> Vec<u8> {
    let (((lb, ls), left), (ib, is), (pos, type_id)) = (lapis, item, gem);
    let mut p = vec![u8::from(success), lb, ls, left, ib, is, pos, type_id, 0, 0, 0, 0];
    p.extend_from_slice(&money.to_le_bytes());
    p.extend_from_slice(&[hammer.0, hammer.1]);
    encode(OP_LINK, &p)
}

/// `0x0802` from the client: `u8 bag, u8 slot` (all gems), or with `u8 flag, u8 gemPos, u8 hammerBag, u8 hammerSlot` (the long form,
/// sent only by a client with region feature 0 on, which also reads the long answer).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Extract {
    pub place: (u8, u8),
    pub flag: u8,
    pub position: u8,
    pub hammer: (u8, u8),
    pub long_form: bool,
}

pub fn parse_extract(payload: &[u8]) -> Option<Extract> {
    match *payload {
        [bag, slot] => Some(Extract { place: (bag, slot), flag: 0, position: 0, hammer: (0, 0), long_form: false }),
        [bag, slot, flag, position, hb, hs] => Some(Extract { place: (bag, slot), flag, position, hammer: (hb, hs), long_form: true }),
        _ => None,
    }
}

/// `0x0802`: `u8 success, u8 bag, u8 slot`, then `u8 gemPos` (255 for all) only in the long form (34 bytes; the US client reads 33
/// without it, `docs/original/lapis.md` section 4), where each lapis taken out went (`u8 bag[6], u8 slot[6], u8 type id[6],
/// u8 count[6]`, in order, the rest 0), `u32 money`.
pub fn extracted(success: bool, place: (u8, u8), position: Option<u8>, lapis: &[((u8, u8), u8)], money: u32) -> Vec<u8> {
    let mut lists = [[0u8; 6]; 4];
    for (i, &((bag, slot), type_id)) in lapis.iter().take(6).enumerate() {
        (lists[0][i], lists[1][i], lists[2][i], lists[3][i]) = (bag, slot, type_id, 1);
    }
    let mut p = vec![u8::from(success), place.0, place.1];
    p.extend(position);
    for list in &lists {
        p.extend_from_slice(list);
    }
    p.extend_from_slice(&money.to_le_bytes());
    encode(OP_EXTRACT, &p)
}

/// `0x0803` from the client: `u8 bag, u8 slot, u8 badge[6]`.
pub fn parse_badge(payload: &[u8]) -> Option<((u8, u8), [u8; 6])> {
    let &[bag, slot, a, b, c, d, e, f] = payload else { return None };
    Some(((bag, slot), [a, b, c, d, e, f]))
}

/// `0x0803` (15 bytes): `u8 success, u8 bag, u8 slot, u8 badge[6], u32 money`.
pub fn badge_result(success: bool, (bag, slot): (u8, u8), badge: [u8; 6], money: u32) -> Vec<u8> {
    let mut p = vec![u8::from(success), bag, slot];
    p.extend_from_slice(&badge);
    p.extend_from_slice(&money.to_le_bytes());
    encode(OP_BADGE, &p)
}

/// `0x0804` from the client, and the answer: `u8 bag, u8 slot`.
pub fn parse_place(payload: &[u8]) -> Option<(u8, u8)> {
    let &[bag, slot] = payload else { return None };
    Some((bag, slot))
}

pub fn badge_removed((bag, slot): (u8, u8)) -> Vec<u8> {
    encode(OP_BADGE_REMOVE, &[bag, slot])
}

/// `0x0805` from the client: Lapisia place, material place, material count, equipment place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Enchant {
    pub lapisia: (u8, u8),
    pub material: (u8, u8),
    pub count: u8,
    pub item: (u8, u8),
}

pub fn parse_enchant(payload: &[u8]) -> Option<Enchant> {
    let &[a, b, c, d, count, e, f] = payload else { return None };
    Some(Enchant { lapisia: (a, b), material: (c, d), count, item: (e, f) })
}

fn craft_name(name: &[u8]) -> [u8; CRAFT_NAME_LEN] {
    let mut out = [0u8; CRAFT_NAME_LEN];
    let len = name.iter().position(|&b| b == 0).unwrap_or(name.len()).min(CRAFT_NAME_LEN - 1);
    out[..len].copy_from_slice(&name[..len]);
    out
}

/// `0x0805` (36 bytes): `u8 success, u8 lapisiaBag, u8 lapisiaSlot, u8 left, u8 materialBag, u8 materialSlot, u8 left, u8 equipBag,
/// u8 equipSlot, u32 money, char craftName[21]`.
pub fn enchanted(success: bool, lapisia: ((u8, u8), u8), material: ((u8, u8), u8), item: (u8, u8), money: u32, name: &[u8]) -> Vec<u8> {
    let ((lb, ls), ll) = lapisia;
    let ((mb, ms), ml) = material;
    let mut p = vec![u8::from(success), lb, ls, ll, mb, ms, ml, item.0, item.1];
    p.extend_from_slice(&money.to_le_bytes());
    p.extend_from_slice(&craft_name(name));
    encode(OP_ENCHANT, &p)
}

/// `0x0806`, `0x0808`: refused (3 bytes, `u8 1`) or done (26 bytes: `u8 0, u8 bag, u8 slot, char craftName[21]`).
pub fn rerolled(op: u16, done: Option<((u8, u8), &[u8])>) -> Vec<u8> {
    match done {
        None => encode(op, &[1]),
        Some(((bag, slot), name)) => {
            let mut p = vec![0, bag, slot];
            p.extend_from_slice(&craft_name(name));
            encode(op, &p)
        }
    }
}

/// `0x0806` from the client: rune place, item place.
pub fn parse_compose(payload: &[u8]) -> Option<((u8, u8), (u8, u8))> {
    let &[a, b, c, d] = payload else { return None };
    Some(((a, b), (c, d)))
}

/// `0x0807` from the client: `u8 bag[3], u8 slot[3]`.
pub fn parse_remake(payload: &[u8]) -> Option<[(u8, u8); 3]> {
    let &[b0, b1, b2, s0, s1, s2] = payload else { return None };
    Some([(b0, s0), (b1, s1), (b2, s2)])
}

/// `0x0807`: refused with a result (3 bytes), or 0 and the new item (37 bytes, `TP_ITEM_UNIT`).
pub fn remade(result: u8, item: Option<&ItemUnit>) -> Vec<u8> {
    let mut p = vec![result];
    if let Some(item) = item {
        p.extend_from_slice(&[item.bag, item.slot, item.item_type, item.type_id]);
        p.extend_from_slice(&item.quality.to_le_bytes());
        p.extend_from_slice(&item.gems);
        p.push(item.count);
        p.extend_from_slice(&craft_name(&item.craft_name));
    }
    encode(OP_REMAKE, &p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::payload;

    #[test]
    fn packets_have_the_original_sizes() {
        let len = |p: &[u8]| p.len() - 2;
        assert_eq!(len(&linked(true, ((1, 2), 3), (1, 4), (0, 7), 100, (0, 0))), 20);
        assert_eq!(payload(&linked(true, ((1, 2), 3), (1, 4), (0, 7), 100, (2, 9)))[..8], [1, 1, 2, 3, 1, 4, 0, 7]);
        assert_eq!(len(&extracted(true, (0, 5), Some(255), &[((1, 3), 7), ((1, 4), 9)], 5)), 34);
        let p = extracted(true, (0, 5), Some(255), &[((1, 3), 7), ((1, 4), 9)], 5);
        assert_eq!(payload(&p)[3..12], [255, 1, 1, 0, 0, 0, 0, 3, 4]);
        assert_eq!(payload(&p)[16..18], [7, 9]);
        assert_eq!(payload(&p)[22..24], [1, 1]);
        // The US client's form: no gemPos, the money last.
        let p = extracted(true, (0, 5), None, &[((1, 3), 7), ((1, 4), 9)], 0x0102_0304);
        assert_eq!(len(&p), 33);
        assert_eq!(payload(&p)[..11], [1, 0, 5, 1, 1, 0, 0, 0, 0, 3, 4]);
        assert_eq!(payload(&p)[15..17], [7, 9]);
        assert_eq!(payload(&p)[21..23], [1, 1]);
        assert_eq!(payload(&p)[27..], [4, 3, 2, 1]);
        assert_eq!(len(&badge_result(false, (0, 7), [1; 6], 9)), 15);
        assert_eq!(len(&badge_removed((0, 7))), 4);
        assert_eq!(len(&enchanted(false, ((1, 1), 0), ((1, 2), 3), (0, 5), 9, b"00000000000000000001")), 36);
        assert_eq!(len(&rerolled(OP_COMPOSE, None)), 3);
        assert_eq!(len(&rerolled(OP_CONSIDER, Some(((1, 1), b"x")))), 26);
        assert_eq!(len(&remade(2, None)), 3);
        let unit = ItemUnit { bag: 1, slot: 2, item_type: 1, type_id: 1, quality: 10, gems: [0; 6], count: 1, craft_name: b"ab".to_vec() };
        assert_eq!(len(&remade(0, Some(&unit))), 37);
        assert_eq!(parse_extract(&[0, 5]), Some(Extract { place: (0, 5), flag: 0, position: 0, hammer: (0, 0), long_form: false }));
        assert_eq!(parse_extract(&[0, 5, 1, 2, 1, 3]).map(|e| (e.flag, e.position, e.hammer, e.long_form)), Some((1, 2, (1, 3), true)));
        assert_eq!(parse_enchant(&[1, 0, 1, 1, 2, 0, 5]), Some(Enchant { lapisia: (1, 0), material: (1, 1), count: 2, item: (0, 5) }));
        assert_eq!(parse_remake(&[1, 1, 2, 3, 4, 5]), Some([(1, 3), (1, 4), (2, 5)]));
    }
}
