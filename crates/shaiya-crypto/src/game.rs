//! Keys of the game connection, as read from `ps_game.exe` and `ps_login.exe`
//! (`docs/original/ps_game.md` sections 4 and 5).

use sha2::{Digest, Sha256};

use crate::SessionKeys;

/// The XOR seed the original game server uses for every player: a constant in the binary,
/// sent to the client in the `0xA301` reply.
pub const XOR_SEED: [u8; 16] = *b"LENASOFTFOREVER!";

/// Size of the XOR table: 128 SHA-256 blocks.
pub const XOR_TABLE_LEN: usize = 4096;

/// AES key and starting counter of a game connection, from the keys negotiated at login.
///
/// The login server hands the game server `key` and `SHA-256(iv)[0..16]` (`CUserCrypto::GetKeyGen`),
/// so the game connection's counters start at the hashed IV, not at the IV itself.
pub fn game_session_keys(login: &SessionKeys) -> SessionKeys {
    let digest = Sha256::digest(login.iv);
    let mut iv = [0u8; 16];
    iv.copy_from_slice(&digest[..16]);
    SessionKeys { key: login.key, iv }
}

/// The table behind the server-to-client XOR layer used once a character is in the world.
pub struct XorTable(Box<[u8; XOR_TABLE_LEN]>);

impl XorTable {
    /// `block[0] = SHA-256(seed)`, then each block is the SHA-256 of the last 16 bytes of the one before.
    pub fn new(seed: &[u8; 16]) -> Self {
        let mut table = Box::new([0u8; XOR_TABLE_LEN]);
        table[..32].copy_from_slice(&Sha256::digest(seed));
        for start in (32..XOR_TABLE_LEN).step_by(32) {
            let digest = Sha256::digest(&table[start - 16..start]);
            table[start..start + 32].copy_from_slice(&digest);
        }
        Self(table)
    }

    pub fn bytes(&self) -> &[u8; XOR_TABLE_LEN] {
        &self.0
    }

    /// XORs the bytes after a packet's length field. A packet of `n` such bytes uses table bytes `n..2n`,
    /// so there is no state between packets. Packets are at most 2048 bytes, which keeps `2n` inside the table.
    pub fn apply(&self, data: &mut [u8]) {
        let n = data.len();
        assert!(2 * n <= XOR_TABLE_LEN, "packet too large for the XOR table: {n}");
        for (byte, key) in data.iter_mut().zip(&self.0[n..2 * n]) {
            *byte ^= key;
        }
    }
}
