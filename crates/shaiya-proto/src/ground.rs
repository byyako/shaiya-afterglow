//! Items on the ground, picking them up and dropping them (`docs/original/combat.md` sections 7 and 8, `docs/original/item-drop.md`).

use crate::frame::encode;
use crate::world::ItemUnit;

/// Server: an item on the ground (`TP_ITEM_ENTER`).
pub const OP_ITEM_ENTER: u16 = 0x0401;
/// Server: the item left the view or the ground (`u32 id`).
pub const OP_ITEM_LEAVE: u16 = 0x0402;
/// Client: pick up (`u32 item id`). Server: where it went, or why not (`TP_ITEM_GET`).
pub const OP_PICK_UP: u16 = 0x0205;
/// Client: drop (`u8 bag, u8 slot, u8 count`). Server: an item left a bag slot, all of it or part (`TP_ITEM_DROP`).
pub const OP_ITEM_DROP: u16 = 0x0206;

/// `0x0401` kinds: how it appears (the client's drop height).
pub const KIND_IN_VIEW: u8 = 0;
pub const KIND_PUT_BACK: u8 = 1;
/// What a player drops comes as kind 1 too (`CUser::ItemDrop`).
pub const KIND_PLAYER_DROP: u8 = 1;
pub const KIND_DROPPED: u8 = 2;

/// `0x0205` error codes.
pub const PICK_NOT_YOURS: u8 = 0;
pub const PICK_NO_ROOM: u8 = 1;
/// "Unavailable quest item" (407 on the client): a quest item the picker has no quest for.
pub const PICK_NO_QUEST: u8 = 2;
pub const PICK_GONE: u8 = 3;
pub const PICK_TOO_FAR: u8 = 4;
pub const PICK_TOO_MUCH_GOLD: u8 = 5;

/// `0x0401`: `u32 id, u8 kind, u8 type, u8 type id, u8 count, f32 x, y, z`.
pub fn item_enter(id: u32, kind: u8, item_type: u8, type_id: u8, count: u8, position: [f32; 3]) -> Vec<u8> {
    let mut p = id.to_le_bytes().to_vec();
    p.extend_from_slice(&[kind, item_type, type_id, count]);
    for v in position {
        p.extend_from_slice(&v.to_le_bytes());
    }
    encode(OP_ITEM_ENTER, &p)
}

pub fn item_leave(id: u32) -> Vec<u8> {
    encode(OP_ITEM_LEAVE, &id.to_le_bytes())
}

/// `0x0205` from the client: `u32 item id`.
pub fn parse_pick_up(payload: &[u8]) -> Option<u32> {
    Some(u32::from_le_bytes(payload.try_into().ok()?))
}

/// `0x0206` from the client: `u8 bag, u8 slot, u8 count`.
pub fn parse_drop(payload: &[u8]) -> Option<(u8, u8, u8)> {
    let &[bag, slot, count] = payload else { return None };
    Some((bag, slot, count))
}

/// `0x0205` refused: `u8 0, u8 code`.
pub fn pick_up_failed(code: u8) -> Vec<u8> {
    encode(OP_PICK_UP, &[0, code])
}

/// `0x0205` for a slot: `u8 bag, u8 slot, u8 type, u8 type id, u8 count, u16 quality, u8 gems[6]`, the craft name with its
/// terminating zero (16 bytes with an empty name).
pub fn picked_up(item: &ItemUnit) -> Vec<u8> {
    let mut p = vec![item.bag, item.slot, item.item_type, item.type_id, item.count];
    p.extend_from_slice(&item.quality.to_le_bytes());
    p.extend_from_slice(&item.gems);
    p.extend_from_slice(&item.craft_name);
    p.push(0);
    encode(OP_PICK_UP, &p)
}

/// `0x0205` for gold: bag 1, slot 0, type 26, the amount in the quality and the first two gems.
pub fn picked_up_gold(type_id: u8, count: u8, amount: u32) -> Vec<u8> {
    let [a, b, c, d] = amount.to_le_bytes();
    let gold = ItemUnit {
        bag: 1,
        slot: 0,
        item_type: 26,
        type_id,
        count,
        quality: u16::from_le_bytes([a, b]),
        gems: [c, d, 0, 0, 0, 0],
        craft_name: Vec::new(),
    };
    picked_up(&gold)
}

/// `0x0206`: `u8 bag, u8 slot, u8 type, u8 type id, u8 count` of what is left in the slot, the last three 0 when nothing is.
pub fn item_left(bag: u8, slot: u8, left: Option<&ItemUnit>) -> Vec<u8> {
    let rest = left.map_or([0; 3], |item| [item.item_type, item.type_id, item.count]);
    encode(OP_ITEM_DROP, &[bag, slot, rest[0], rest[1], rest[2]])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::payload;

    #[test]
    fn a_drop_request_is_three_bytes() {
        assert_eq!(parse_drop(&[1, 2, 3]), Some((1, 2, 3)));
        assert_eq!(parse_drop(&[1, 2]), None);
        assert_eq!(parse_drop(&[1, 2, 3, 4]), None);
    }

    #[test]
    fn packets_have_the_original_sizes() {
        let len = |p: &[u8]| p.len() - 2;
        assert_eq!(len(&item_enter(1, KIND_DROPPED, 26, 0, 1, [1.0, 0.0, 2.0])), 22);
        assert_eq!(len(&item_leave(1)), 6);
        assert_eq!(len(&pick_up_failed(PICK_NO_ROOM)), 4);
        let sword = ItemUnit { bag: 1, slot: 2, item_type: 5, type_id: 1, count: 1, quality: 1200, ..Default::default() };
        assert_eq!(len(&picked_up(&sword)), 16);
        assert_eq!(len(&picked_up(&ItemUnit { craft_name: b"0102".to_vec(), ..sword })), 20);
        assert_eq!(payload(&picked_up_gold(0, 1, 0x0102_0304)), [1, 0, 26, 0, 1, 4, 3, 2, 1, 0, 0, 0, 0, 0]);
        assert_eq!(parse_pick_up(&[7, 0, 0, 0]), Some(7));
    }
}
