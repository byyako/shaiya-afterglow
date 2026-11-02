//! Obelisks (`docs/original/obelisks.md`, the client's side in `docs/client/obelisks.md`). States are the packets' countries: 0
//! neutral, 1 light, 2 fury.

use crate::frame::encode;

/// Server: a map's obelisks (`u8 n`, n × `u32 id, u8 state, f32 x, f32 z`), on entering it.
pub const OP_LIST: u16 = 0x2101;
/// Server: an obelisk's new state (`u32 id, u8 state`), to the map.
pub const OP_CHANGE: u16 = 0x2102;
/// Server: an obelisk fell (`u16 monster, u8 from, u8 to`), to the world.
pub const OP_DESTROYED: u16 = 0x2103;

/// Rows per `0x2101` (`CMap::SendObeliskList`).
pub const LIST_ROWS: usize = 100;

/// One `0x2101` row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Row {
    pub id: u32,
    pub state: u8,
    pub x: f32,
    pub z: f32,
}

/// `0x2101` for the rows, 100 to a packet; nothing for none.
pub fn list(rows: &[Row]) -> Vec<Vec<u8>> {
    rows.chunks(LIST_ROWS)
        .map(|chunk| {
            let mut p = vec![chunk.len() as u8];
            for r in chunk {
                p.extend_from_slice(&r.id.to_le_bytes());
                p.push(r.state);
                p.extend_from_slice(&r.x.to_le_bytes());
                p.extend_from_slice(&r.z.to_le_bytes());
            }
            encode(OP_LIST, &p)
        })
        .collect()
}

pub fn change(id: u32, state: u8) -> Vec<u8> {
    let mut p = id.to_le_bytes().to_vec();
    p.push(state);
    encode(OP_CHANGE, &p)
}

pub fn destroyed(mob_id: u16, from: u8, to: u8) -> Vec<u8> {
    let mut p = mob_id.to_le_bytes().to_vec();
    p.extend_from_slice(&[from, to]);
    encode(OP_DESTROYED, &p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_follow_the_original() {
        // Sizes without the length field: 1 + 13 n + 2, 7, 6.
        let rows = vec![Row { id: 1, state: 2, x: 1.5, z: -2.0 }; 101];
        let packets = list(&rows);
        assert_eq!(packets.len(), 2);
        assert_eq!(packets[0].len(), 2 + 3 + 13 * 100);
        assert_eq!(packets[1][4], 1, "the rest in a second packet");
        assert_eq!(&packets[1][5..10], &[1, 0, 0, 0, 2]);
        assert!(list(&[]).is_empty());
        assert_eq!(change(9, 1), encode(OP_CHANGE, &[9, 0, 0, 0, 1]));
        assert_eq!(destroyed(252, 0, 2), encode(OP_DESTROYED, &[252, 0, 0, 2]));
    }
}
