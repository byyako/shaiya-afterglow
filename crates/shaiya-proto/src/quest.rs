//! Quests (`docs/original/quest.md`).

use crate::frame::encode;

/// Server: the quests in progress at login (`TP_QUEST_LIST`).
pub const OP_LIST: u16 = 0x0901;
/// Client: take a quest (`u32 npc id, u16 quest id`). Server: it was taken (the same).
pub const OP_START: u16 = 0x0902;
/// Client: hand a quest in (`u32 npc id, u16 quest id`). Server: it ended (`TP_QUEST_END`).
pub const OP_END: u16 = 0x0903;
/// Client: as `0x0902` (the client never sends it).
pub const OP_START_TOO: u16 = 0x0904;
/// Server: a counter moved (`u16 quest id, u8 counter, u8 count`).
pub const OP_UPDATE: u16 = 0x0905;
/// Server: the finished quests at login (`TP_QUEST_FINISH_LIST`).
pub const OP_FINISHED: u16 = 0x0906;
/// Server: choose a reward (`u16 quest id`). Client: the reward chosen (`u16 quest id, u8 result index`).
pub const OP_CHOOSE: u16 = 0x0907;
/// Client: give a quest up (`u16 quest id`).
pub const OP_GIVE_UP: u16 = 0x0908;

/// Entries per `0x0901` or `0x0906` packet (a `u8` count, the original stops at 255).
const PER_PACKET: usize = 255;

/// `0x0902` and `0x0903` from the client: `u32 npc id, u16 quest id`.
pub fn parse_request(payload: &[u8]) -> Option<(u32, u16)> {
    let payload: &[u8; 6] = payload.try_into().ok()?;
    Some((u32::from_le_bytes(payload[..4].try_into().unwrap()), u16::from_le_bytes([payload[4], payload[5]])))
}

/// `0x0907` from the client: `u16 quest id, u8 result index`.
pub fn parse_choice(payload: &[u8]) -> Option<(u16, u8)> {
    match payload {
        &[a, b, index] => Some((u16::from_le_bytes([a, b]), index)),
        _ => None,
    }
}

/// `0x0908`: `u16 quest id`.
pub fn parse_quest_id(payload: &[u8]) -> Option<u16> {
    Some(u16::from_le_bytes(payload.try_into().ok()?))
}

/// A quest in progress as `0x0901` carries it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActiveEntry {
    pub quest_id: u16,
    /// Whole minutes left of a time limit; 0 without one.
    pub minutes_left: u16,
    pub counts: [u8; 3],
}

/// `0x0901`: `u8 n`, n × `u16 quest id, u16 minutes left, u8 counts[3]`; 255 per packet, nothing when there are none.
pub fn active_list(entries: &[ActiveEntry]) -> Vec<Vec<u8>> {
    entries
        .chunks(PER_PACKET)
        .map(|chunk| {
            let mut p = vec![chunk.len() as u8];
            for e in chunk {
                p.extend_from_slice(&e.quest_id.to_le_bytes());
                p.extend_from_slice(&e.minutes_left.to_le_bytes());
                p.extend_from_slice(&e.counts);
            }
            encode(OP_LIST, &p)
        })
        .collect()
}

/// `0x0906`: `u8 n`, n × `u16 quest id, u8 success`; 255 per packet, nothing when there are none.
pub fn finished_list(entries: &[(u16, bool)]) -> Vec<Vec<u8>> {
    entries
        .chunks(PER_PACKET)
        .map(|chunk| {
            let mut p = vec![chunk.len() as u8];
            for &(quest_id, success) in chunk {
                p.extend_from_slice(&quest_id.to_le_bytes());
                p.push(u8::from(success));
            }
            encode(OP_FINISHED, &p)
        })
        .collect()
}

/// `0x0902`: the quest was taken, from the NPC with this object id (0 without one).
pub fn started(npc_id: u32, quest_id: u16) -> Vec<u8> {
    let mut p = npc_id.to_le_bytes().to_vec();
    p.extend_from_slice(&quest_id.to_le_bytes());
    encode(OP_START, &p)
}

/// `0x0905`: counter `index` of a quest is now `count`.
pub fn update(quest_id: u16, index: u8, count: u8) -> Vec<u8> {
    let [a, b] = quest_id.to_le_bytes();
    encode(OP_UPDATE, &[a, b, index, count])
}

/// `0x0907`: the player is to choose the reward.
pub fn choose(quest_id: u16) -> Vec<u8> {
    encode(OP_CHOOSE, &quest_id.to_le_bytes())
}

/// What `0x0903` tells of an ended quest.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ended {
    pub npc_id: u32,
    pub quest_id: u16,
    pub success: bool,
    pub result: u8,
    pub exp: u32,
    pub money: u32,
    /// Where the reward item went (bag 0: none), and what it is.
    pub bag: u8,
    pub slot: u8,
    pub item_type: u8,
    pub type_id: u8,
}

/// `0x0903`, 22 bytes: `u32 npc id, u16 quest id, u8 success, u8 result, u32 exp, u32 money, u8 bag, slot, item type, type id`.
pub fn ended(e: &Ended) -> Vec<u8> {
    let mut p = e.npc_id.to_le_bytes().to_vec();
    p.extend_from_slice(&e.quest_id.to_le_bytes());
    p.extend_from_slice(&[u8::from(e.success), e.result]);
    p.extend_from_slice(&e.exp.to_le_bytes());
    p.extend_from_slice(&e.money.to_le_bytes());
    p.extend_from_slice(&[e.bag, e.slot, e.item_type, e.type_id]);
    encode(OP_END, &p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::payload;

    #[test]
    fn packets_have_the_original_sizes() {
        let len = |p: &[u8]| p.len() - 2;
        assert_eq!(len(&started(7, 1)), 8);
        assert_eq!(len(&update(1, 0, 3)), 6);
        assert_eq!(len(&choose(1)), 4);
        assert_eq!(len(&ended(&Ended::default())), 22);
        let entry = ActiveEntry { quest_id: 2, minutes_left: 60, counts: [1, 2, 3] };
        let lists = active_list(&[entry; 3]);
        assert_eq!(lists.len(), 1);
        assert_eq!(len(&lists[0]), 3 + 7 * 3);
        assert_eq!(payload(&lists[0])[..8], [3, 2, 0, 60, 0, 1, 2, 3]);
        assert_eq!(len(&finished_list(&[(5, true), (6, false)])[0]), 3 + 3 * 2);
        assert_eq!(payload(&finished_list(&[(5, true)])[0]), [1, 5, 0, 1]);
    }

    #[test]
    fn long_lists_are_split_and_empty_ones_send_nothing() {
        assert!(active_list(&[]).is_empty());
        assert!(finished_list(&[]).is_empty());
        let finished: Vec<(u16, bool)> = (1..=300).map(|id| (id, true)).collect();
        let packets = finished_list(&finished);
        assert_eq!(packets.len(), 2);
        assert_eq!(payload(&packets[0])[0], 255);
        assert_eq!(payload(&packets[1])[0], 45);
    }

    #[test]
    fn requests_parse() {
        assert_eq!(parse_request(&[7, 0, 0, 0, 2, 1]), Some((7, 0x0102)));
        assert_eq!(parse_request(&[7, 0, 0, 0, 2]), None);
        assert_eq!(parse_choice(&[2, 0, 1]), Some((2, 1)));
        assert_eq!(parse_quest_id(&[2, 0]), Some(2));
        let e =
            Ended { npc_id: 9, quest_id: 3, success: true, result: 1, exp: 100, money: 50, bag: 1, slot: 4, item_type: 22, type_id: 11 };
        assert_eq!(payload(&ended(&e)), [9, 0, 0, 0, 3, 0, 1, 1, 100, 0, 0, 0, 50, 0, 0, 0, 1, 4, 22, 11]);
    }
}
