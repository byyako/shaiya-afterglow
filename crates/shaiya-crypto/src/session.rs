use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

use crate::MODULUS_LEN;

/// AES key and initial counter block shared by both directions.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SessionKeys {
    pub key: [u8; 16],
    pub iv: [u8; 16],
}

impl std::fmt::Debug for SessionKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SessionKeys(..)")
    }
}

/// `HMAC-SHA256(key = decrypted, message = modulus)`, split into key and IV.
///
/// The decrypted value is used as the full 128-byte buffer (zero padded at the high end),
/// which HMAC hashes down because it is longer than the block size.
pub fn derive_session_keys(decrypted_le: &[u8; MODULUS_LEN], modulus_le: &[u8; MODULUS_LEN]) -> SessionKeys {
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(decrypted_le).expect("HMAC accepts keys of any length");
    mac.update(modulus_le);
    let digest = mac.finalize().into_bytes();

    let mut key = [0u8; 16];
    let mut iv = [0u8; 16];
    key.copy_from_slice(&digest[..16]);
    iv.copy_from_slice(&digest[16..32]);
    SessionKeys { key, iv }
}
