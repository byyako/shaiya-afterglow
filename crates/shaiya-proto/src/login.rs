//! Packets of the login connection, layouts as read from `ps_login.exe`.

use crate::frame::encode;

pub const OP_PING: u16 = 0x0003;
/// Sent by the client when it quits on the login connection (Alt+F4 or the Quit button).
pub const OP_LOGIN_TERMINATE: u16 = 0x010B;
pub const OP_HANDSHAKE: u16 = 0xA101;
pub const OP_LOGIN: u16 = 0xA102;
pub const OP_SERVER_LIST: u16 = 0xA201;
pub const OP_SELECT_SERVER: u16 = 0xA202;

/// The original server cuts both credential fields at 12 characters; the username still is.
pub const CREDENTIAL_MAX_LEN: usize = 12;
/// The longest password: the client sends 16 bytes of it (its box takes 19, `docs/original/ps_login.md` section 3). The original cut it at 12.
pub const PASSWORD_MAX_LEN: usize = 16;
/// Privilege byte of an ordinary player. The original server sends 11 for them and the staff
/// level (1 to 5) for GMs; values below 11 are staff.
pub const PRIVILEGE_PLAYER: u8 = 11;
/// Server list status values.
pub const SERVER_STATUS_NORMAL: u8 = 0;
pub const SERVER_STATUS_CLOSED: u8 = 2;
/// A closed server is listed with this capacity, as the original does.
pub const SERVER_CLOSED_CAPACITY: u16 = 0x7FFF;
/// Servers per `0xA201` packet; longer lists are split.
pub const SERVER_LIST_CHUNK: usize = 48;

/// `0xA101` from the server: RSA public key. Exponent and modulus are little-endian.
pub fn handshake_request(modulus: &[u8; 128], exponent: &[u8; 64]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(3 + 64 + 128);
    payload.extend_from_slice(&[0, 64, 128]);
    payload.extend_from_slice(exponent);
    payload.extend_from_slice(modulus);
    encode(OP_HANDSHAKE, &payload)
}

/// `0xA101` from the client: `u8 0x80` and 128 bytes of RSA ciphertext.
/// The original only checked the length byte; this requires the exact size.
pub fn parse_handshake_response(payload: &[u8]) -> Option<[u8; 128]> {
    match payload {
        [0x80, rest @ ..] if rest.len() == 128 => Some(rest.try_into().ok()?),
        _ => None,
    }
}

pub fn handshake_response(ciphertext: &[u8; 128]) -> Vec<u8> {
    let mut payload = vec![0x80];
    payload.extend_from_slice(ciphertext);
    encode(OP_HANDSHAKE, &payload)
}

#[derive(Debug, PartialEq, Eq)]
pub struct LoginRequest {
    pub username: Vec<u8>,
    pub password: Vec<u8>,
}

/// `0xA102` from the client: `char username[32]` followed by the password field.
/// The client sends 16 or 19 password bytes depending on the build, so at least 48 payload bytes are
/// required; the username is read to 12 characters and the password to 16 (a 16-character one has no zero after it).
pub fn parse_login_request(payload: &[u8]) -> Option<LoginRequest> {
    if payload.len() < 32 + PASSWORD_MAX_LEN {
        return None;
    }
    Some(LoginRequest { username: credential(&payload[..32], CREDENTIAL_MAX_LEN), password: credential(&payload[32..], PASSWORD_MAX_LEN) })
}

fn credential(field: &[u8], max: usize) -> Vec<u8> {
    field.iter().take(max).take_while(|&&b| b != 0).copied().collect()
}

/// Client-side builder: fixed 32-byte name and 16-byte password fields, longer input is truncated.
pub fn login_request(username: &str, password: &str) -> Vec<u8> {
    let mut payload = vec![0u8; 32 + 16];
    let name = &username.as_bytes()[..username.len().min(32)];
    let pass = &password.as_bytes()[..password.len().min(16)];
    payload[..name.len()].copy_from_slice(name);
    payload[32..32 + pass.len()].copy_from_slice(pass);
    encode(OP_LOGIN, &payload)
}

/// Result byte of `0xA102`, as the original server produces it (see `docs/original/ps_login.md`
/// section 12). The names follow the client's messages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum LoginStatus {
    Success = 0,
    NoSuchAccount = 1,
    CannotConnect = 2,
    BadCredentials = 3,
    /// Account blocked from the game.
    NotAllowed = 4,
    /// Account blocked from the game and the homepage.
    NotAllowedWeb = 5,
    /// Account is being terminated at the user's request.
    Withdrawal = 6,
    Withdrawal3Days = 7,
    Withdrawal15Days = 8,
    /// Stopped at the user's request.
    UserStop = 9,
    Blocked = 10,
    Restricted = 11,
    Restricted2 = 12,
    Restricted3 = 13,
    Restricted4 = 14,
    Expired = 15,
}

/// `0xA102` from the server. Always 22 payload bytes (24 with the opcode); fields after the
/// status are zero on failure.
pub fn login_response(status: LoginStatus, user_id: u32, privilege: u8, identity: &[u8; 16]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(22);
    payload.push(status as u8);
    payload.extend_from_slice(&user_id.to_le_bytes());
    payload.push(privilege);
    payload.extend_from_slice(identity);
    encode(OP_LOGIN, &payload)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerEntry {
    pub id: u8,
    /// 0 normal, 1 locked, 2 closed.
    pub status: u8,
    pub players: u16,
    pub capacity: u16,
    pub name: String,
}

/// `0xA201`: `u8 count` then 38-byte entries; split into chunks of `SERVER_LIST_CHUNK`.
pub fn server_list(entries: &[ServerEntry]) -> Vec<Vec<u8>> {
    entries
        .chunks(SERVER_LIST_CHUNK)
        .map(|chunk| {
            let mut payload = vec![chunk.len() as u8];
            for entry in chunk {
                payload.push(entry.id);
                payload.push(entry.status);
                payload.extend_from_slice(&entry.players.to_le_bytes());
                payload.extend_from_slice(&entry.capacity.to_le_bytes());
                let mut name = [0u8; 32];
                let bytes = entry.name.as_bytes();
                let len = bytes.len().min(31);
                name[..len].copy_from_slice(&bytes[..len]);
                payload.extend_from_slice(&name);
            }
            encode(OP_SERVER_LIST, &payload)
        })
        .collect()
}

/// `0xA202` from the client: `u8 server id, u32 client version`.
pub fn parse_select_server(payload: &[u8]) -> Option<(u8, u32)> {
    let id = *payload.first()?;
    let version = u32::from_le_bytes(payload.get(1..5)?.try_into().ok()?);
    Some((id, version))
}

pub fn select_server_request(id: u8, version: u32) -> Vec<u8> {
    let mut payload = vec![id];
    payload.extend_from_slice(&version.to_le_bytes());
    encode(OP_SELECT_SERVER, &payload)
}

/// Result byte of `0xA202`: 0 success, then the negative codes as bytes.
pub mod select_result {
    pub const SUCCESS: u8 = 0;
    pub const CANNOT_CONNECT: u8 = 0xFE;
    pub const VERSION_MISMATCH: u8 = 0xFD;
    pub const SERVER_FULL: u8 = 0xFC;
}

/// `0xA202` from the server: result and game server IPv4 address (zero on errors).
pub fn select_server_response(result: u8, ip: [u8; 4]) -> Vec<u8> {
    let mut payload = vec![result];
    payload.extend_from_slice(&ip);
    encode(OP_SELECT_SERVER, &payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::{opcode, payload};

    #[test]
    fn handshake_request_matches_the_captured_header() {
        let packet = handshake_request(&[0xaa; 128], &[0xbb; 64]);
        // 0xC7 total length, opcode 0xA101, then 00 40 80, as in the forum capture.
        assert_eq!(&packet[..7], [0xc7, 0x00, 0x01, 0xa1, 0x00, 0x40, 0x80]);
        assert_eq!(packet.len(), 0xc7);
    }

    #[test]
    fn handshake_response_has_the_captured_size() {
        let packet = handshake_response(&[1; 128]);
        // The capture shows total length 0x85 and a leading 0x80.
        assert_eq!(&packet[..5], [0x85, 0x00, 0x01, 0xa1, 0x80]);
        assert_eq!(parse_handshake_response(payload(&packet)), Some([1; 128]));
    }

    #[test]
    fn handshake_response_requires_exact_length() {
        assert!(parse_handshake_response(&[0x80; 100]).is_none());
        let mut short = vec![0x80];
        short.extend_from_slice(&[0; 64]);
        assert!(parse_handshake_response(&short).is_none());
        let mut wrong_byte = vec![0x7f];
        wrong_byte.extend_from_slice(&[0; 128]);
        assert!(parse_handshake_response(&wrong_byte).is_none());
    }

    #[test]
    fn the_username_is_cut_at_twelve_characters_and_the_password_at_sixteen() {
        let long = "abcdefghijklmnopqrs";
        let packet = login_request(long, long);
        let parsed = parse_login_request(payload(&packet)).unwrap();
        assert_eq!(parsed.username, b"abcdefghijkl");
        assert_eq!(parsed.password, b"abcdefghijklmnop", "the 16 bytes the client sends, with no zero after them");
        // A 19-byte password field (another build) is read to 16 as well.
        let mut body = vec![0u8; 32 + 19];
        body[32..51].copy_from_slice(long.as_bytes());
        assert_eq!(parse_login_request(&body).unwrap().password, b"abcdefghijklmnop");
    }

    #[test]
    fn login_request_accepts_both_password_field_sizes() {
        for password_field in [16, 19] {
            let mut body = vec![0u8; 32 + password_field];
            body[..4].copy_from_slice(b"test");
            body[32..36].copy_from_slice(b"pass");
            let parsed = parse_login_request(&body).unwrap();
            assert_eq!(parsed.username, b"test");
            assert_eq!(parsed.password, b"pass");
        }
        assert!(parse_login_request(&[0; 47]).is_none());
    }

    #[test]
    fn login_response_is_24_bytes_with_opcode() {
        let packet = login_response(LoginStatus::Success, 0x01020304, 7, &[9; 16]);
        assert_eq!(packet.len(), 2 + 24);
        assert_eq!(opcode(&packet), OP_LOGIN);
        let body = payload(&packet);
        assert_eq!(body[0], 0);
        assert_eq!(body[1..5], [4, 3, 2, 1]);
        assert_eq!(body[5], 7);
        assert_eq!(body[6..], [9; 16]);
    }

    #[test]
    fn server_list_entries_are_38_bytes_and_chunked_at_48() {
        let entries: Vec<_> = (0..50).map(|i| ServerEntry { id: i, status: 0, players: 1, capacity: 2, name: format!("S{i}") }).collect();
        let packets = server_list(&entries);
        assert_eq!(packets.len(), 2);
        assert_eq!(packets[0].len(), 4 + 1 + 48 * 38);
        assert_eq!(payload(&packets[0])[0], 48);
        assert_eq!(packets[1].len(), 4 + 1 + 2 * 38);
        assert_eq!(payload(&packets[1])[0], 2);
    }

    #[test]
    fn select_server_roundtrip() {
        let packet = select_server_request(3, 108);
        assert_eq!(parse_select_server(payload(&packet)), Some((3, 108)));
        assert_eq!(select_server_response(0xFE, [0; 4]).len(), 4 + 5);
        assert!(parse_select_server(&[1, 2, 3]).is_none());
    }
}
