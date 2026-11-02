//! NPC shop packets (`0x07xx`), as `CUser::PacketShop` in `ps_game.exe` reads and builds them (`docs/original/shop.md`).

use crate::frame::encode;

/// Client: buy from a merchant. Server: the result.
pub const OP_BUY: u16 = 0x0702;
/// Client: sell to any merchant. Server: the result.
pub const OP_SELL: u16 = 0x0703;

/// Results of `0x0702`; `0x0703` always answers `BOUGHT_OR_SOLD`.
pub mod result {
    pub const BOUGHT_OR_SOLD: u8 = 0;
    pub const NO_MONEY: u8 = 1;
    pub const NO_ROOM: u8 = 2;
}

/// `0x0702` from the client.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuyRequest {
    pub npc_id: u32,
    /// Position in the merchant's list, from 0.
    pub index: u8,
    pub count: u8,
}

/// `u32 npc object id, u8 index, u8 count`.
pub fn parse_buy(payload: &[u8]) -> Option<BuyRequest> {
    match payload {
        [a, b, c, d, index, count] => Some(BuyRequest { npc_id: u32::from_le_bytes([*a, *b, *c, *d]), index: *index, count: *count }),
        _ => None,
    }
}

pub fn buy_request(r: &BuyRequest) -> Vec<u8> {
    let mut p = r.npc_id.to_le_bytes().to_vec();
    p.extend_from_slice(&[r.index, r.count]);
    encode(OP_BUY, &p)
}

/// `0x0703` from the client: `u8 bag, u8 slot, u8 count`.
pub fn parse_sell(payload: &[u8]) -> Option<(u8, u8, u8)> {
    match payload {
        [bag, slot, count] => Some((*bag, *slot, *count)),
        _ => None,
    }
}

pub fn sell_request(bag: u8, slot: u8, count: u8) -> Vec<u8> {
    encode(OP_SELL, &[bag, slot, count])
}

/// The reply to `0x0702` or `0x0703`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShopResult {
    pub result: u8,
    pub bag: u8,
    pub slot: u8,
    pub item_type: u8,
    pub type_id: u8,
    pub count: u8,
    pub money: u32,
}

/// `u8 result, u8 bag, u8 slot, u8 type, u8 type id, u8 count, u32 money` under `0x0702` or `0x0703`.
pub fn shop_result(opcode: u16, r: &ShopResult) -> Vec<u8> {
    let mut p = vec![r.result, r.bag, r.slot, r.item_type, r.type_id, r.count];
    p.extend_from_slice(&r.money.to_le_bytes());
    encode(opcode, &p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::payload;

    #[test]
    fn shop_packets_have_the_original_sizes() {
        // As captured from the US client: NPC 156, index 8, count 10.
        let request = BuyRequest { npc_id: 156, index: 8, count: 10 };
        assert_eq!(parse_buy(&[0x9c, 0, 0, 0, 8, 10]), Some(request));
        assert_eq!(buy_request(&request).len(), 2 + 8);
        assert_eq!(parse_buy(&[0x9c, 0, 0, 0, 8]), None);
        assert_eq!(parse_sell(&[1, 2, 3]), Some((1, 2, 3)));
        assert_eq!(sell_request(1, 2, 3).len(), 2 + 5);

        let reply =
            shop_result(OP_BUY, &ShopResult { result: 0, bag: 1, slot: 2, item_type: 25, type_id: 1, count: 10, money: 0x01020304 });
        // 12 bytes with the opcode, as TP_SHOP_SELL in the PDB and game.exe's handlers.
        assert_eq!(reply.len(), 2 + 12);
        assert_eq!(payload(&reply), [0, 1, 2, 25, 1, 10, 4, 3, 2, 1]);
    }
}
