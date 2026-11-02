use aes::Aes128;
use aes::cipher::{BlockCipherEncrypt, KeyInit};

/// AES-128 counter mode with a continuous keystream.
///
/// Differences from the usual CTR construction, all taken from the original server:
/// the 16-byte counter block is incremented as a little-endian integer, and the
/// keystream is not reset between packets: leftover bytes of one block are used
/// first by the next call. Encryption and decryption are the same operation.
pub struct CtrStream {
    cipher: Aes128,
    counter: u128,
    block: [u8; 16],
    used: usize,
}

impl CtrStream {
    pub fn new(key: &[u8; 16], iv: &[u8; 16]) -> Self {
        Self { cipher: Aes128::new(key.into()), counter: u128::from_le_bytes(*iv), block: [0; 16], used: 16 }
    }

    pub fn apply(&mut self, data: &mut [u8]) {
        for byte in data {
            if self.used == 16 {
                self.next_block();
            }
            *byte ^= self.block[self.used];
            self.used += 1;
        }
    }

    fn next_block(&mut self) {
        let mut block = self.counter.to_le_bytes();
        self.cipher.encrypt_block((&mut block).into());
        self.block = block;
        self.counter = self.counter.wrapping_add(1);
        self.used = 0;
    }
}
