use hex_literal::hex;
use sha2::{Digest, Sha256};
use shaiya_crypto::{CtrStream, MODULUS_LEN, RsaKeyPair, derive_session_keys};

/// FIPS-197 appendix C.1: AES-128 key 000102..0f, block 00112233..ff.
const FIPS_KEY: [u8; 16] = hex!("000102030405060708090a0b0c0d0e0f");
const FIPS_BLOCK: [u8; 16] = hex!("00112233445566778899aabbccddeeff");
const FIPS_CIPHER: [u8; 16] = hex!("69c4e0d86a7b0430d8cdb78070b4c55a");

#[test]
fn first_keystream_block_is_aes_of_the_iv() {
    let mut stream = CtrStream::new(&FIPS_KEY, &FIPS_BLOCK);
    let mut out = [0u8; 16];
    stream.apply(&mut out);
    assert_eq!(out, FIPS_CIPHER);
}

#[test]
fn counter_is_incremented_little_endian_with_carry() {
    // Counter bytes ff 00 00 ...: a little-endian increment gives 00 01 00 ...
    let mut iv = [0u8; 16];
    iv[0] = 0xff;
    let mut stream = CtrStream::new(&FIPS_KEY, &iv);
    let mut two_blocks = [0u8; 32];
    stream.apply(&mut two_blocks);

    let mut next = [0u8; 16];
    next[1] = 0x01;
    let mut reference = CtrStream::new(&FIPS_KEY, &next);
    let mut expected = [0u8; 16];
    reference.apply(&mut expected);
    assert_eq!(two_blocks[16..], expected);
}

#[test]
fn counter_wraps_after_all_ones() {
    let iv = [0xffu8; 16];
    let mut stream = CtrStream::new(&FIPS_KEY, &iv);
    let mut out = [0u8; 32];
    stream.apply(&mut out);
    let mut zero = CtrStream::new(&FIPS_KEY, &[0u8; 16]);
    let mut expected = [0u8; 16];
    zero.apply(&mut expected);
    assert_eq!(out[16..], expected);
}

#[test]
fn keystream_continues_across_calls() {
    let message: Vec<u8> = (0..100u8).collect();
    let iv = hex!("00112233445566778899aabbccddeeff");

    let mut whole = message.clone();
    CtrStream::new(&FIPS_KEY, &iv).apply(&mut whole);

    // Packets of awkward sizes, as the server would see them.
    let mut chunked = message.clone();
    let mut stream = CtrStream::new(&FIPS_KEY, &iv);
    let mut at = 0;
    for len in [3, 13, 1, 16, 31, 36] {
        stream.apply(&mut chunked[at..at + len]);
        at += len;
    }
    assert_eq!(at, 100);
    assert_eq!(whole, chunked);
}

#[test]
fn applying_twice_restores_the_message() {
    let iv = [7u8; 16];
    let original: Vec<u8> = (0..57u8).collect();
    let mut data = original.clone();
    CtrStream::new(&FIPS_KEY, &iv).apply(&mut data);
    assert_ne!(data, original);
    CtrStream::new(&FIPS_KEY, &iv).apply(&mut data);
    assert_eq!(data, original);
}

/// HMAC-SHA256 written out by hand: keys longer than 64 bytes are hashed first.
fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        k[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let ipad: Vec<u8> = k.iter().map(|b| b ^ 0x36).collect();
    let opad: Vec<u8> = k.iter().map(|b| b ^ 0x5c).collect();
    let inner = Sha256::new().chain_update(&ipad).chain_update(message).finalize();
    Sha256::new().chain_update(&opad).chain_update(inner).finalize().into()
}

#[test]
fn session_keys_match_a_hand_written_hmac_with_long_key_rule() {
    let mut decrypted = [0u8; MODULUS_LEN];
    let mut modulus = [0u8; MODULUS_LEN];
    for i in 0..MODULUS_LEN {
        decrypted[i] = (i as u8).wrapping_mul(7).wrapping_add(3);
        modulus[i] = (i as u8).wrapping_mul(13).wrapping_add(5);
    }
    let keys = derive_session_keys(&decrypted, &modulus);
    let digest = hmac_sha256(&decrypted, &modulus);
    assert_eq!(keys.key, digest[..16]);
    assert_eq!(keys.iv, digest[16..]);
}

#[test]
fn rsa_keys_have_the_shape_the_client_expects() {
    let mut rng = rand::rng();
    let key = RsaKeyPair::generate(&mut rng);
    let n = key.modulus_le();
    let e = key.public_exponent_le();
    assert_eq!(n[MODULUS_LEN - 1] & 0x80, 0x80, "modulus must use all 1024 bits");
    assert_eq!(e[63] & 0x80, 0x80, "exponent is a full-size 512-bit number");
    assert_eq!(e[0] & 1, 1, "exponent is odd");
}

#[test]
fn client_and_server_derive_the_same_session() {
    let mut rng = rand::rng();
    let server = RsaKeyPair::generate(&mut rng);
    let n = server.modulus_le();
    let e = server.public_exponent_le();

    // The client picks a random value smaller than the modulus.
    let mut secret = [0u8; MODULUS_LEN];
    rand::Rng::fill(&mut rng, &mut secret[..MODULUS_LEN - 2]);

    let ciphertext = RsaKeyPair::encrypt_le(&n, &e, &secret).expect("secret is below modulus");
    let recovered = server.decrypt_le(&ciphertext).expect("ciphertext is below modulus");
    assert_eq!(recovered, secret);

    assert_eq!(derive_session_keys(&secret, &n), derive_session_keys(&recovered, &n));
}

#[test]
fn ciphertext_not_below_the_modulus_is_rejected() {
    let mut rng = rand::rng();
    let server = RsaKeyPair::generate(&mut rng);
    assert!(server.decrypt_le(&[0xff; MODULUS_LEN]).is_none());
}
