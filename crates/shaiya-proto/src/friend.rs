//! Friends, the block list and the party search (`0x22xx`, `docs/original/friends.md`).

use crate::chat::{NAME_LEN, name_field};
use crate::frame::encode;

/// Server: the friend list, 25 entries a packet.
pub const OP_FRIENDS: u16 = 0x2201;
/// Client: ask a player to be friends (`char name[21]`). Server: someone asks (`char name[21]`).
pub const OP_REQUEST: u16 = 0x2202;
/// Client: the answer (`u8` 1 yes). Server: the asker's result (`u8`).
pub const OP_ANSWER: u16 = 0x2203;
/// Server: a friend added (`u32 id, u8 job, char name[21]`).
pub const OP_FRIEND_ADD: u16 = 0x2204;
/// Client and server: a friend removed (`u32 id`).
pub const OP_FRIEND_REMOVE: u16 = 0x2205;
/// Client and server: a friend's memo (`u32 id, u8 n, memo`).
pub const OP_FRIEND_MEMO: u16 = 0x2206;
/// Server: a friend's online state (`u32 id, u8 online`).
pub const OP_ONLINE: u16 = 0x2207;
/// Server: the block list, 25 entries a packet.
pub const OP_BLOCKS: u16 = 0x2208;
/// Client: block a player (`char name[21]`). Server: a block added (`u32 id, char name[21]`).
pub const OP_BLOCK_ADD: u16 = 0x2209;
/// Client and server: a block removed (`u32 id`).
pub const OP_BLOCK_REMOVE: u16 = 0x220A;
/// Client and server: a block's memo (`u32 id, u8 n, memo`).
pub const OP_BLOCK_MEMO: u16 = 0x220B;
/// Client: the party search. Server: registered (`u8 1`).
pub const OP_PARTY_SEARCH: u16 = 0x220C;
/// Server: the party search results.
pub const OP_PARTY_FOUND: u16 = 0x220D;
/// Server: the block's result (`u8`).
pub const OP_BLOCK_RESULT: u16 = 0x220E;

/// `0x2203` to the asker.
pub const REFUSED: u8 = 0;
pub const DONE: u8 = 1;
pub const CANNOT_ASK: u8 = 2;

/// `0x220E`.
pub const BLOCK_FAILED: u8 = 0;
pub const BLOCKED: u8 = 1;

/// Entries in one `0x2201` or `0x2208`.
pub const LIST_CHUNK: usize = 25;
/// Players in `0x220D`.
pub const MAX_FOUND: usize = 30;

/// A friend in `0x2201`.
#[derive(Clone, Debug, PartialEq)]
pub struct FriendUnit {
    pub id: u32,
    pub job: u8,
    pub online: bool,
    pub name: Vec<u8>,
    pub memo: Vec<u8>,
}

/// A blocked player in `0x2208`.
#[derive(Clone, Debug, PartialEq)]
pub struct BlockUnit {
    pub id: u32,
    pub name: Vec<u8>,
    pub memo: Vec<u8>,
}

/// A player in `0x220D`.
#[derive(Clone, Debug, PartialEq)]
pub struct FoundUnit {
    pub level: u16,
    pub job: u8,
    pub name: Vec<u8>,
}

/// The longest memo a list entry carries (`friends::MAX_MEMO`); a longer one a tool wrote is cut, so a list packet stays small
/// (`docs/server/audit.md` R22).
const MEMO_MAX: usize = 50;

fn memo_len(memo: &[u8]) -> u8 {
    memo.len().min(MEMO_MAX) as u8
}

/// `u8 count`, then the units, `LIST_CHUNK` a packet; nothing when there are none.
fn chunked<T>(opcode: u16, units: &[T], unit: impl Fn(&T, &mut Vec<u8>)) -> Vec<Vec<u8>> {
    units
        .chunks(LIST_CHUNK)
        .map(|chunk| {
            let mut p = vec![chunk.len() as u8];
            for u in chunk {
                unit(u, &mut p);
            }
            encode(opcode, &p)
        })
        .collect()
}

/// `0x2201`: `u32 id, u8 job, u8 online, char name[21], u8 n, char memo[n]` each (`CUser::SendCharacterFriend`).
pub fn friends(units: &[FriendUnit]) -> Vec<Vec<u8>> {
    chunked(OP_FRIENDS, units, |u, p| {
        p.extend_from_slice(&u.id.to_le_bytes());
        p.push(u.job);
        p.push(u8::from(u.online));
        p.extend_from_slice(&name_field(&u.name));
        p.push(memo_len(&u.memo));
        p.extend_from_slice(&u.memo[..usize::from(memo_len(&u.memo))]);
    })
}

/// `0x2208`: `u32 id, char name[21], u8 n, char memo[n]` each.
pub fn blocks(units: &[BlockUnit]) -> Vec<Vec<u8>> {
    chunked(OP_BLOCKS, units, |u, p| {
        p.extend_from_slice(&u.id.to_le_bytes());
        p.extend_from_slice(&name_field(&u.name));
        p.push(memo_len(&u.memo));
        p.extend_from_slice(&u.memo[..usize::from(memo_len(&u.memo))]);
    })
}

/// `0x2202` to the one asked: the asker's name.
pub fn request(name: &[u8]) -> Vec<u8> {
    encode(OP_REQUEST, &name_field(name))
}

/// `0x2203` to the asker.
pub fn answer(result: u8) -> Vec<u8> {
    encode(OP_ANSWER, &[result])
}

/// `0x2204`: a friend added.
pub fn friend_added(id: u32, job: u8, name: &[u8]) -> Vec<u8> {
    let mut p = id.to_le_bytes().to_vec();
    p.push(job);
    p.extend_from_slice(&name_field(name));
    encode(OP_FRIEND_ADD, &p)
}

fn by_id(opcode: u16, id: u32) -> Vec<u8> {
    encode(opcode, &id.to_le_bytes())
}

/// `0x2205`: a friend removed.
pub fn friend_removed(id: u32) -> Vec<u8> {
    by_id(OP_FRIEND_REMOVE, id)
}

/// `0x2206` / `0x220B`: `u32 id, u8 n, char memo[n], 0`.
fn memo(opcode: u16, id: u32, memo: &[u8]) -> Vec<u8> {
    let mut p = id.to_le_bytes().to_vec();
    p.push(memo_len(memo));
    p.extend_from_slice(&memo[..usize::from(memo_len(memo))]);
    p.push(0);
    encode(opcode, &p)
}

pub fn friend_memo(id: u32, text: &[u8]) -> Vec<u8> {
    memo(OP_FRIEND_MEMO, id, text)
}

pub fn block_memo(id: u32, text: &[u8]) -> Vec<u8> {
    memo(OP_BLOCK_MEMO, id, text)
}

/// `0x2207`: a friend's online state.
pub fn online(id: u32, online: bool) -> Vec<u8> {
    let mut p = id.to_le_bytes().to_vec();
    p.push(u8::from(online));
    encode(OP_ONLINE, &p)
}

/// `0x2209`: a block added.
pub fn block_added(id: u32, name: &[u8]) -> Vec<u8> {
    let mut p = id.to_le_bytes().to_vec();
    p.extend_from_slice(&name_field(name));
    encode(OP_BLOCK_ADD, &p)
}

/// `0x220A`: a block removed.
pub fn block_removed(id: u32) -> Vec<u8> {
    by_id(OP_BLOCK_REMOVE, id)
}

/// `0x220E`: the block's result.
pub fn block_result(result: u8) -> Vec<u8> {
    encode(OP_BLOCK_RESULT, &[result])
}

/// `0x220C`: registered for the party search.
pub fn party_search_registered() -> Vec<u8> {
    encode(OP_PARTY_SEARCH, &[1])
}

/// `0x220D`: `u8 count`, then `u8 level` (the low byte), `u8 job`, `char name[21]` each, at most `MAX_FOUND`; `None` without anyone.
pub fn party_found(units: &[FoundUnit]) -> Option<Vec<u8>> {
    let units = &units[..units.len().min(MAX_FOUND)];
    if units.is_empty() {
        return None;
    }
    let mut p = vec![units.len() as u8];
    for u in units {
        p.push(u.level as u8);
        p.push(u.job);
        p.extend_from_slice(&name_field(&u.name));
    }
    Some(encode(OP_PARTY_FOUND, &p))
}

/// `0x2202` and `0x2209` from the client: `char name[21]`, cut at its first zero (the original forces the last byte to 0).
pub fn parse_name(payload: &[u8]) -> Option<Vec<u8>> {
    let name = payload.get(..NAME_LEN)?;
    Some(name[..NAME_LEN - 1].iter().copied().take_while(|&b| b != 0).collect())
}

/// `0x2203` from the client: `u8` (1 yes).
pub fn parse_yes(payload: &[u8]) -> Option<bool> {
    payload.first().map(|&b| b == 1)
}

/// `0x2205` and `0x220A` from the client: `u32 id`.
pub fn parse_id(payload: &[u8]) -> Option<u32> {
    Some(u32::from_le_bytes(payload.get(..4)?.try_into().ok()?))
}

/// `0x2206` and `0x220B` from the client: `u32 id, u8 n, char memo[n]`. The memo is the `n` bytes sent, cut at a zero
/// (deviation 3); `None` when fewer than `n` bytes came.
pub fn parse_memo(payload: &[u8]) -> Option<(u32, Vec<u8>)> {
    let id = parse_id(payload)?;
    let n = usize::from(*payload.get(4)?);
    let text = payload.get(5..5 + n)?;
    Some((id, text.iter().copied().take_while(|&b| b != 0).collect()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::{opcode, payload};

    fn friend(id: u32, memo: &[u8]) -> FriendUnit {
        FriendUnit { id, job: 3, online: true, name: b"Abc".to_vec(), memo: memo.to_vec() }
    }

    #[test]
    fn packets_have_the_original_sizes() {
        let len = |p: &[u8]| p.len() - 2;
        assert_eq!(len(&request(b"Abc")), 23);
        assert_eq!(len(&answer(DONE)), 3);
        assert_eq!(len(&friend_added(1, 2, b"Abc")), 28);
        assert_eq!(len(&friend_removed(1)), 6);
        assert_eq!(len(&friend_memo(1, b"hello")), 8 + 5);
        assert_eq!(len(&online(1, true)), 7);
        assert_eq!(len(&block_added(1, b"Abc")), 27);
        assert_eq!(len(&block_removed(1)), 6);
        assert_eq!(len(&block_memo(1, b"")), 8);
        assert_eq!(len(&block_result(BLOCKED)), 3);
        assert_eq!(len(&party_search_registered()), 3);
    }

    #[test]
    fn a_memo_longer_than_fifty_bytes_is_cut_in_lists_and_memo_packets() {
        let long = vec![b'm'; 200];
        let list = friends(&vec![friend(1, &long); 25]);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].len(), 4 + 1 + 25 * (28 + 50), "25 friends with 50-byte memos");
        let blocked = blocks(&vec![BlockUnit { id: 9, name: b"X".to_vec(), memo: long.clone() }; 25]);
        assert_eq!(blocked[0].len(), 4 + 1 + 25 * (26 + 50));
        assert_eq!(friend_memo(1, &long).len(), 4 + 4 + 1 + 50 + 1);
    }

    #[test]
    fn lists_go_25_a_packet_with_memos_of_their_own_length() {
        let units: Vec<_> = (0..26).map(|i| friend(i, if i == 0 { b"hi" } else { b"" })).collect();
        let packets = friends(&units);
        assert_eq!(packets.len(), 2);
        assert_eq!(opcode(&packets[0]), OP_FRIENDS);
        // u8 count, then 28 bytes and the memo per friend.
        assert_eq!(payload(&packets[0]).len(), 1 + 25 * 28 + 2);
        assert_eq!(
            payload(&packets[1]),
            &[1, 25, 0, 0, 0, 3, 1, b'A', b'b', b'c', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0][..]
        );
        assert!(friends(&[]).is_empty());
        let b = blocks(&[BlockUnit { id: 9, name: b"X".to_vec(), memo: b"m".to_vec() }]);
        assert_eq!(payload(&b[0]).len(), 1 + 26 + 1);
    }

    #[test]
    fn party_search_results_are_cut_at_30_and_absent_when_empty() {
        let unit = FoundUnit { level: 0x0105, job: 2, name: b"P".to_vec() };
        let p = party_found(&vec![unit.clone(); 40]).unwrap();
        assert_eq!(payload(&p)[0], 30);
        assert_eq!(payload(&p).len(), 1 + 30 * 23);
        assert_eq!(&payload(&p)[1..3], &[0x05, 2], "the level's low byte, then the job");
        assert!(party_found(&[]).is_none());
    }

    #[test]
    fn requests_parse_as_the_client_sends_them() {
        let mut name = [0u8; 21];
        name[..3].copy_from_slice(b"Abc");
        assert_eq!(parse_name(&name), Some(b"Abc".to_vec()));
        assert_eq!(parse_name(&[b'A'; 21]), Some(vec![b'A'; 20]), "the last byte is a zero whatever was sent");
        assert_eq!(parse_name(&[0; 20]), None);
        assert_eq!(parse_yes(&[1]), Some(true));
        assert_eq!(parse_yes(&[2]), Some(false));
        assert_eq!(parse_memo(&[7, 0, 0, 0, 3, b'a', b'b', b'c']), Some((7, b"abc".to_vec())));
        assert_eq!(parse_memo(&[7, 0, 0, 0, 3, b'a', 0, b'c']), Some((7, b"a".to_vec())));
        assert_eq!(parse_memo(&[7, 0, 0, 0, 3, b'a']), None, "fewer bytes than announced");
        assert_eq!(parse_memo(&[7, 0, 0, 0, 0]), Some((7, Vec::new())));
    }
}
