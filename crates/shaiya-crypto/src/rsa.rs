use num_bigint_dig::{BigUint, ModInverse, RandBigInt, RandPrime};
use num_integer::Integer;
use num_traits::One;
use rand::Rng;

pub const MODULUS_LEN: usize = 128;
pub const PUBLIC_EXPONENT_LEN: usize = 64;

const PRIME_BITS: usize = 512;

/// Raw RSA key pair in the shape the Shaiya client expects:
/// a 1024-bit modulus and a random 512-bit public exponent.
pub struct RsaKeyPair {
    n: BigUint,
    e: BigUint,
    d: BigUint,
}

impl RsaKeyPair {
    pub fn generate<R: Rng>(rng: &mut R) -> Self {
        loop {
            let p = prime_with_top_bits(rng);
            let q = prime_with_top_bits(rng);
            if p == q {
                continue;
            }
            let n = &p * &q;
            let phi = (&p - 1u32) * (&q - 1u32);

            let Some(e) = random_exponent(rng, &phi) else {
                continue;
            };
            let Some(d) = (&e).mod_inverse(&phi).and_then(|d| d.to_biguint()) else {
                continue;
            };
            return Self { n, e, d };
        }
    }

    pub fn modulus_le(&self) -> [u8; MODULUS_LEN] {
        to_le_array(&self.n)
    }

    pub fn public_exponent_le(&self) -> [u8; PUBLIC_EXPONENT_LEN] {
        to_le_array(&self.e)
    }

    /// Decrypts a client reply (raw RSA, no padding). Returns `None` if the
    /// ciphertext is not smaller than the modulus.
    pub fn decrypt_le(&self, ciphertext: &[u8; MODULUS_LEN]) -> Option<[u8; MODULUS_LEN]> {
        let c = BigUint::from_bytes_le(ciphertext);
        if c >= self.n {
            return None;
        }
        Some(to_le_array(&c.modpow(&self.d, &self.n)))
    }

    /// Client-side operation, used by tests and tools.
    pub fn encrypt_le(
        modulus_le: &[u8; MODULUS_LEN],
        exponent_le: &[u8; PUBLIC_EXPONENT_LEN],
        message: &[u8; MODULUS_LEN],
    ) -> Option<[u8; MODULUS_LEN]> {
        let n = BigUint::from_bytes_le(modulus_le);
        let e = BigUint::from_bytes_le(exponent_le);
        let m = BigUint::from_bytes_le(message);
        if m >= n {
            return None;
        }
        Some(to_le_array(&m.modpow(&e, &n)))
    }
}

/// A 512-bit prime with its two top bits set, so the product has exactly 1024 bits.
fn prime_with_top_bits<R: Rng>(rng: &mut R) -> BigUint {
    loop {
        let p: BigUint = rng.gen_prime(PRIME_BITS);
        if (&p >> (PRIME_BITS - 2)) == BigUint::from(3u32) {
            return p;
        }
    }
}

/// A random 512-bit odd exponent with the top bit set, coprime to `phi`.
fn random_exponent<R: Rng>(rng: &mut R, phi: &BigUint) -> Option<BigUint> {
    for _ in 0..1000 {
        let mut e = rng.gen_biguint(PUBLIC_EXPONENT_LEN * 8);
        e |= BigUint::one() << (PUBLIC_EXPONENT_LEN * 8 - 1);
        e |= BigUint::one();
        if e.gcd(phi).is_one() {
            return Some(e);
        }
    }
    None
}

fn to_le_array<const N: usize>(v: &BigUint) -> [u8; N] {
    let bytes = v.to_bytes_le();
    let mut out = [0u8; N];
    out[..bytes.len()].copy_from_slice(&bytes);
    out
}
