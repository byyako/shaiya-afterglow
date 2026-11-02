//! Cryptography used on the Shaiya login connection (EP4 era).
//!
//! The scheme, as read from `ps_login.exe`:
//!
//! 1. The server sends a 1024-bit RSA public key (64-byte exponent, 128-byte modulus).
//! 2. The client replies with a random 128-byte value encrypted with raw RSA.
//! 3. `HMAC-SHA256(key = decrypted value, message = modulus)` gives 32 bytes:
//!    the first 16 are the AES-128 key, the last 16 the initial counter block.
//! 4. Every packet after the 2-byte length is XORed with an AES-128 CTR keystream
//!    that runs continuously across packets, one stream per direction.
//!
//! All big numbers are little-endian on the wire.
//!
//! The game connection reuses the login key with a hashed starting counter, and adds an XOR layer
//! for server-to-client packets once a character is in the world (`game` module).

mod game;
mod rsa;
mod session;
mod stream;

pub use game::{XOR_SEED, XOR_TABLE_LEN, XorTable, game_session_keys};
pub use rsa::{MODULUS_LEN, PUBLIC_EXPONENT_LEN, RsaKeyPair};
pub use session::{SessionKeys, derive_session_keys};
pub use stream::CtrStream;
