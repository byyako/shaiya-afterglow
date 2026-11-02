//! Packets of the game connection up to the character screen, layouts as read from `ps_game.exe`
//! and its PDB (`docs/original/ps_game.md`). Struct names in the comments are the original ones.

use crate::frame::encode;

/// System packets, handled in every state.
pub const OP_DISCONNECT: u16 = 0x0002;
pub const OP_PING: u16 = 0x0003;
/// The server ends the connection: `u8` 0 "disconnect due to using logged in ID.", else "system error"; the client quits.
pub const OP_KICKED: u16 = 0x0004;
/// `0x0004`'s value for a newer login of the account (message 10070).
pub const KICKED_LOGGED_IN: u8 = 0;

pub const OP_GAME_CONNECT: u16 = 0xA301;
pub const OP_BILLING_INFO: u16 = 0xB103;

pub const OP_CHAR_LIST: u16 = 0x0101;
pub const OP_CHAR_CREATE: u16 = 0x0102;
pub const OP_CHAR_DELETE: u16 = 0x0103;
pub const OP_CHAR_SELECT: u16 = 0x0104;
/// Leaving the character screen; the original closes the connection.
pub const OP_CHAR_LEAVE: u16 = 0x0107;
pub const OP_CHAR_COUNTRY: u16 = 0x0109;
pub const OP_CHAR_RENAME: u16 = 0x010E;
pub const OP_CHAR_RESTORE: u16 = 0x010F;

/// `byEncMode` of the `0xA301` reply; the original always sends 2.
pub const ENCRYPTION_MODE: u8 = 2;
/// Character slots per account and server.
pub const SLOTS: u8 = 5;
/// Country value meaning "no faction chosen yet".
pub const COUNTRY_NONE: u8 = 2;
/// Bytes of the name field in requests (`szName[21]`); the original keeps at most 20 characters.
pub const NAME_FIELD_LEN: usize = 21;
pub const NAME_MAX_LEN: usize = 20;
/// Name bytes in a character list record; the next two bytes are the delete and rename flags.
pub const LIST_NAME_LEN: usize = 19;

/// `TPC_GAME_CONNECT` from the client: `u32 user id, u8 identity[16]`, sent in plain text.
/// The original never checked the length; this requires exactly 20 payload bytes.
pub fn parse_game_connect(payload: &[u8]) -> Option<(u32, [u8; 16])> {
    if payload.len() != 20 {
        return None;
    }
    let user_id = u32::from_le_bytes(payload[..4].try_into().ok()?);
    Some((user_id, payload[4..20].try_into().ok()?))
}

pub fn game_connect_request(user_id: u32, identity: &[u8; 16]) -> Vec<u8> {
    let mut payload = user_id.to_le_bytes().to_vec();
    payload.extend_from_slice(identity);
    encode(OP_GAME_CONNECT, &payload)
}

/// `TP_GAME_CONNECT`: `u8 success = 0, u8 encryption mode = 2, u8 xor seed[16]`. The original sends it
/// only on success; a refused handshake gets a closed connection.
pub fn game_connect_response(xor_seed: &[u8; 16]) -> Vec<u8> {
    let mut payload = vec![0, ENCRYPTION_MODE];
    payload.extend_from_slice(xor_seed);
    encode(OP_GAME_CONNECT, &payload)
}

/// `0x0004` (`docs/original/client-extras.md` section 2): why the server ends the connection.
pub fn kicked(reason: u8) -> Vec<u8> {
    encode(OP_KICKED, &[reason])
}

/// `0xB103`: `u8 billing type, u32 remaining time`, sent right after the handshake reply.
pub fn billing_info(billing_type: u8, remaining: u32) -> Vec<u8> {
    let mut payload = vec![billing_type];
    payload.extend_from_slice(&remaining.to_le_bytes());
    encode(OP_BILLING_INFO, &payload)
}

/// `TP_CHAR_COUNTRY`: the account's faction on this server (0, 1, or `COUNTRY_NONE`) and the highest mode it may create.
pub fn char_country(country: u8, max_grow: u8) -> Vec<u8> {
    encode(OP_CHAR_COUNTRY, &[country, max_grow])
}

/// A character as the selection screen shows it (`stCharacterList`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CharacterSummary {
    pub slot: u8,
    pub id: u32,
    pub create_time: u32,
    pub level: u16,
    pub family: u8,
    pub grow: u8,
    pub hair: u8,
    pub face: u8,
    pub size: u8,
    pub job: u8,
    pub sex: u8,
    pub map: u16,
    /// str, dex, rec, int, wis, luc
    pub stats: [u16; 6],
    /// hp, mp, sp
    pub health: [u16; 3],
    pub equipment_type: [u8; 8],
    pub equipment_type_id: [u8; 8],
    pub name: Vec<u8>,
    pub delete_pending: bool,
    pub rename_allowed: bool,
    pub cloak: [u8; 6],
}

/// `TP_CHAR_LIST` for an empty slot: `u8 slot, u32 id = 0` (7 bytes with the opcode).
pub fn char_list_empty(slot: u8) -> Vec<u8> {
    let mut payload = vec![slot];
    payload.extend_from_slice(&0u32.to_le_bytes());
    encode(OP_CHAR_LIST, &payload)
}

/// `TP_CHAR_LIST` for a character, 83 bytes with the opcode. The name is cut to 19 bytes; the two bytes after it
/// carry the delete-pending and rename-allowed flags, as the original server writes them.
pub fn char_list_entry(c: &CharacterSummary) -> Vec<u8> {
    let mut p = Vec::with_capacity(81);
    p.push(c.slot);
    p.extend_from_slice(&c.id.to_le_bytes());
    p.extend_from_slice(&c.create_time.to_le_bytes());
    p.extend_from_slice(&c.level.to_le_bytes());
    p.extend_from_slice(&[c.family, c.grow, c.hair, c.face, c.size, c.job, c.sex]);
    p.extend_from_slice(&c.map.to_le_bytes());
    for value in c.stats.iter().chain(&c.health) {
        p.extend_from_slice(&value.to_le_bytes());
    }
    p.extend_from_slice(&c.equipment_type);
    p.extend_from_slice(&c.equipment_type_id);
    let mut name = [0u8; LIST_NAME_LEN];
    let len = c.name.len().min(LIST_NAME_LEN - 1);
    name[..len].copy_from_slice(&c.name[..len]);
    p.extend_from_slice(&name);
    p.push(u8::from(c.delete_pending));
    p.push(u8::from(c.rename_allowed));
    p.extend_from_slice(&c.cloak);
    encode(OP_CHAR_LIST, &p)
}

/// The original's name clean-up (`TrimString(name, 21)`): at most 20 bytes, every space removed, cut at the first NUL.
pub fn clean_name(field: &[u8]) -> Vec<u8> {
    field.iter().take(NAME_MAX_LEN).take_while(|&&b| b != 0).filter(|&&b| b != b' ').copied().collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateRequest {
    pub slot: u8,
    pub family: u8,
    pub grow: u8,
    pub hair: u8,
    pub face: u8,
    pub size: u8,
    pub job: u8,
    pub sex: u8,
    /// After `clean_name`.
    pub name: Vec<u8>,
    /// Payload bytes the client actually sent, kept for logging. The US client sends the name and its zero only
    /// (8 + name length + 1), not the struct's 21-byte field.
    pub sent_len: usize,
}

/// `TP_CHAR_CREATE`: eight `u8` fields then `char name[21]`. Shorter name fields are accepted (the US client sends
/// only the name and its zero); the eight fields must be present.
pub fn parse_char_create(payload: &[u8]) -> Option<CreateRequest> {
    let [slot, family, grow, hair, face, size, job, sex] = *payload.first_chunk::<8>()?;
    let name = clean_name(&payload[8..payload.len().min(8 + NAME_FIELD_LEN)]);
    Some(CreateRequest { slot, family, grow, hair, face, size, job, sex, name, sent_len: payload.len() })
}

pub fn char_create_request(r: &CreateRequest) -> Vec<u8> {
    let mut payload = vec![r.slot, r.family, r.grow, r.hair, r.face, r.size, r.job, r.sex];
    let mut name = [0u8; NAME_FIELD_LEN];
    let len = r.name.len().min(NAME_MAX_LEN);
    name[..len].copy_from_slice(&r.name[..len]);
    payload.extend_from_slice(&name);
    encode(OP_CHAR_CREATE, &payload)
}

/// Result of `0x0102`: 0 success (the list follows), 1 refused.
pub mod create_result {
    pub const SUCCESS: u8 = 0;
    pub const FAILED: u8 = 1;
}

pub fn char_create_response(result: u8) -> Vec<u8> {
    encode(OP_CHAR_CREATE, &[result])
}

/// Delete (`0x0103`), select (`0x0104`) and restore (`0x010F`) requests carry just `u32 character id`.
pub fn parse_char_id(payload: &[u8]) -> Option<u32> {
    Some(u32::from_le_bytes(*payload.first_chunk::<4>()?))
}

pub fn char_id_request(opcode: u16, id: u32) -> Vec<u8> {
    encode(opcode, &id.to_le_bytes())
}

/// `TP_CHAR_SUCCESS` (`u8 0, u32 id`), the reply to delete, select and restore once done. The original sends nothing on failure.
pub fn char_success(opcode: u16, id: u32) -> Vec<u8> {
    let mut payload = vec![0];
    payload.extend_from_slice(&id.to_le_bytes());
    encode(opcode, &payload)
}

/// `TPC_CHAR_COUNTRY` from the client: `u8 country`, which must be 0 or 1.
pub fn parse_char_country(payload: &[u8]) -> Option<u8> {
    payload.first().copied().filter(|&c| c < 2)
}

/// `TPC_CHAR_RENAME`: `u32 character id, char name[21]`.
pub fn parse_char_rename(payload: &[u8]) -> Option<(u32, Vec<u8>)> {
    let id = parse_char_id(payload)?;
    Some((id, clean_name(&payload[4..payload.len().min(4 + NAME_FIELD_LEN)])))
}

/// Result of `0x010E`: 1 renamed, 2 refused.
pub mod rename_result {
    pub const SUCCESS: u8 = 1;
    pub const FAILED: u8 = 2;
}

pub fn char_rename_response(result: u8) -> Vec<u8> {
    encode(OP_CHAR_RENAME, &[result])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::{opcode, payload};

    #[test]
    fn game_connect_sizes_match_the_original_structs() {
        // TPC_GAME_CONNECT is 22 bytes and TP_GAME_CONNECT 20, both counted with the opcode but without the length field.
        let request = game_connect_request(0x01020304, &[9; 16]);
        assert_eq!(request.len(), 2 + 22);
        assert_eq!(parse_game_connect(payload(&request)), Some((0x01020304, [9; 16])));
        assert!(parse_game_connect(&[0; 19]).is_none());
        assert!(parse_game_connect(&[0; 21]).is_none());

        let reply = game_connect_response(b"LENASOFTFOREVER!");
        assert_eq!(reply.len(), 2 + 20);
        assert_eq!(opcode(&reply), OP_GAME_CONNECT);
        assert_eq!(payload(&reply)[..2], [0, 2]);
        assert_eq!(&payload(&reply)[2..], b"LENASOFTFOREVER!");
    }

    #[test]
    fn small_packets_have_the_original_sizes() {
        assert_eq!(billing_info(0, 0).len(), 2 + 7);
        assert_eq!(char_country(2, 3).len(), 2 + 4);
        assert_eq!(char_list_empty(4), [9, 0, 0x01, 0x01, 4, 0, 0, 0, 0]);
        assert_eq!(char_success(OP_CHAR_DELETE, 7).len(), 2 + 7);
        assert_eq!(char_create_response(0).len(), 2 + 3);
    }

    #[test]
    fn list_entry_is_83_bytes_with_fields_at_the_original_offsets() {
        let c = CharacterSummary {
            slot: 3,
            id: 0x11223344,
            create_time: 0x55667788,
            level: 0x0102,
            family: 1,
            grow: 2,
            hair: 3,
            face: 4,
            size: 5,
            job: 5,
            sex: 1,
            map: 0x0a0b,
            stats: [10, 11, 12, 13, 14, 15],
            health: [100, 200, 300],
            equipment_type: [1; 8],
            equipment_type_id: [2; 8],
            name: b"ABCDEFGHIJKLMNOPQRSTU".to_vec(),
            delete_pending: true,
            rename_allowed: true,
            cloak: [7; 6],
        };
        let packet = char_list_entry(&c);
        assert_eq!(packet.len(), 2 + 83);
        // Offsets below are from the start of the struct (the opcode), as in docs/original/ps_game.md section 6.1.
        let s = &packet[2..];
        assert_eq!(s[2], 3);
        assert_eq!(s[3..7], 0x11223344u32.to_le_bytes());
        assert_eq!(s[11..13], [2, 1]);
        assert_eq!(s[13..20], [1, 2, 3, 4, 5, 5, 1]);
        assert_eq!(s[20..22], [0x0b, 0x0a]);
        assert_eq!(s[22..24], [10, 0]);
        assert_eq!(s[32..34], [15, 0]);
        assert_eq!(s[38..40], 300u16.to_le_bytes());
        assert_eq!(s[40..48], [1; 8]);
        assert_eq!(s[48..56], [2; 8]);
        assert_eq!(&s[56..74], b"ABCDEFGHIJKLMNOPQR", "18 name bytes and a terminating zero");
        assert_eq!(s[74], 0);
        assert_eq!(s[75..77], [1, 1], "delete pending, rename allowed");
        assert_eq!(s[77..83], [7; 6]);
    }

    #[test]
    fn names_are_cleaned_like_the_original() {
        assert_eq!(clean_name(b"Al ice\0junk"), b"Alice");
        assert_eq!(clean_name(&[b'x'; 21]), vec![b'x'; 20]);
        assert_eq!(clean_name(b"   \0"), b"");
    }

    #[test]
    fn create_request_roundtrip_and_short_name_fields() {
        let request =
            CreateRequest { slot: 1, family: 2, grow: 3, hair: 4, face: 5, size: 6, job: 5, sex: 0, name: b"Hero".to_vec(), sent_len: 29 };
        let packet = char_create_request(&request);
        assert_eq!(packet.len(), 2 + 31, "TP_CHAR_CREATE is 31 bytes");
        assert_eq!(parse_char_create(payload(&packet)), Some(request));
        // A client that sends a 19-byte name field still parses.
        let mut short = payload(&packet)[..8 + 19].to_vec();
        short[8 + 18] = b'Z';
        let parsed = parse_char_create(&short).unwrap();
        assert_eq!(parsed.sent_len, 27);
        assert_eq!(parsed.name, b"Hero");
        assert!(parse_char_create(&[0; 7]).is_none());
    }

    #[test]
    fn small_requests_parse() {
        assert_eq!(parse_char_id(&5u32.to_le_bytes()), Some(5));
        assert_eq!(parse_char_id(&[1, 2, 3]), None);
        assert_eq!(parse_char_country(&[1]), Some(1));
        assert_eq!(parse_char_country(&[2]), None);
        assert_eq!(parse_char_country(&[]), None);
        let mut rename = 9u32.to_le_bytes().to_vec();
        rename.extend_from_slice(b"New Name\0");
        assert_eq!(parse_char_rename(&rename), Some((9, b"NewName".to_vec())));
    }
}
