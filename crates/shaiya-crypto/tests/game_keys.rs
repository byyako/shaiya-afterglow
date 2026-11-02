//! Known answers for the game connection keys. The expected values were computed separately with
//! Python's hashlib from the construction in docs/original/ps_game.md section 5.

use hex_literal::hex;
use sha2::{Digest, Sha256};
use shaiya_crypto::{SessionKeys, XOR_SEED, XOR_TABLE_LEN, XorTable, game_session_keys};

#[test]
fn xor_seed_is_the_constant_from_the_server() {
    assert_eq!(XOR_SEED, hex!("4c454e41534f4654464f524556455221"));
}

#[test]
fn xor_table_matches_known_answers() {
    let table = XorTable::new(&XOR_SEED);
    let bytes = table.bytes();
    assert_eq!(bytes.len(), XOR_TABLE_LEN);
    assert_eq!(bytes[..32], hex!("6b8ad46b8607f6876c5678e55c89f157494108e63d349209f96ebbfbbba1f179"));
    assert_eq!(bytes[4064..], hex!("6b5b0575b36ff8f4bd94d34334e10a0ef8d6a6888c9546fe62b8443a4be79e2e"));
    assert_eq!(Sha256::digest(&bytes[..])[..], hex!("b3fb3fc7f845c90e85087b7dc9b7aede477a0f7fffec0104518f7f0bbec7dcb7"));
}

#[test]
fn xor_offset_depends_on_the_packet_length() {
    let table = XorTable::new(&XOR_SEED);
    // Three bytes after the length field use table bytes 3, 4 and 5.
    let mut data = [0u8; 3];
    table.apply(&mut data);
    assert_eq!(data, table.bytes()[3..6]);
    // Applying twice restores the packet.
    let mut packet = *b"opcode and payload";
    table.apply(&mut packet);
    table.apply(&mut packet);
    assert_eq!(&packet, b"opcode and payload");
    // The largest packet (2048 bytes, 2046 after the length) fits.
    let mut largest = vec![0u8; 2046];
    table.apply(&mut largest);
    assert_eq!(largest[..], table.bytes()[2046..4092]);
}

#[test]
fn game_counter_starts_at_the_hashed_login_iv() {
    let login = SessionKeys { key: [7; 16], iv: hex!("000102030405060708090a0b0c0d0e0f") };
    let game = game_session_keys(&login);
    assert_eq!(game.key, login.key, "the AES key is reused");
    assert_eq!(game.iv, hex!("be45cb2605bf36bebde684841a28f0fd"));
}
