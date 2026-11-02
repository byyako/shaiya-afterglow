//! Fuzz harness for the crypto layer (`docs/server/audit.md` section 5.2).

use std::sync::OnceLock;

use proptest::collection::vec;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;
use shaiya_crypto::{CtrStream, MODULUS_LEN, RsaKeyPair, XOR_SEED, XOR_TABLE_LEN, XorTable, derive_session_keys, game_session_keys};

/// Failing cases go to `proptest-regressions/fuzz.txt`: the default place is found from `lib.rs`, which a test file has no sibling of.
fn config() -> ProptestConfig {
    ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct("proptest-regressions/fuzz.txt"))),
        ..ProptestConfig::default()
    }
}

/// One key for the file: generation takes seconds.
fn key() -> &'static RsaKeyPair {
    static KEY: OnceLock<RsaKeyPair> = OnceLock::new();
    KEY.get_or_init(|| RsaKeyPair::generate(&mut rand::rng()))
}

fn table() -> &'static XorTable {
    static TABLE: OnceLock<XorTable> = OnceLock::new();
    TABLE.get_or_init(|| XorTable::new(&XOR_SEED))
}

proptest! {
    #![proptest_config(config())]
    /// Applying the stream twice with the same start gives back the message, at any length.
    #[test]
    fn ctr_round_trips(key in any::<[u8; 16]>(), iv in any::<[u8; 16]>(), message in vec(any::<u8>(), 0..5000)) {
        let mut data = message.clone();
        CtrStream::new(&key, &iv).apply(&mut data);
        CtrStream::new(&key, &iv).apply(&mut data);
        prop_assert_eq!(data, message);
    }

    /// The keystream runs on across calls: any split gives the bytes of one call.
    #[test]
    fn ctr_does_not_depend_on_how_the_message_is_cut(
        key in any::<[u8; 16]>(),
        iv in any::<[u8; 16]>(),
        message in vec(any::<u8>(), 0..2000),
        sizes in vec(0usize..300, 0..12),
    ) {
        let mut whole = message.clone();
        CtrStream::new(&key, &iv).apply(&mut whole);

        let mut cut = message.clone();
        let mut stream = CtrStream::new(&key, &iv);
        let mut at = 0;
        for size in sizes {
            let end = (at + size).min(cut.len());
            stream.apply(&mut cut[at..end]);
            at = end;
        }
        stream.apply(&mut cut[at..]);
        prop_assert_eq!(cut, whole);
    }

    /// Whatever 128 bytes a client sends back, the server's answer is a refusal or a value, never a panic; a value is below the modulus.
    #[test]
    fn decrypt_never_panics(ciphertext in vec(any::<u8>(), MODULUS_LEN), mostly_ff in any::<bool>()) {
        let mut c: [u8; MODULUS_LEN] = ciphertext.try_into().unwrap();
        if mostly_ff {
            // Near the modulus, where the "not below it" check decides.
            c[8..].fill(0xff);
        }
        if let Some(plain) = key().decrypt_le(&c) {
            let below = |a: &[u8; MODULUS_LEN], b: &[u8; MODULUS_LEN]| a.iter().rev().cmp(b.iter().rev()).is_lt();
            prop_assert!(below(&plain, &key().modulus_le()));
            let keys = derive_session_keys(&plain, &key().modulus_le());
            let _ = game_session_keys(&keys);
        }
    }

    /// Encrypt with the public half, decrypt with the private one.
    #[test]
    fn rsa_round_trips(message in vec(any::<u8>(), MODULUS_LEN - 2)) {
        let mut m = [0u8; MODULUS_LEN];
        m[..MODULUS_LEN - 2].copy_from_slice(&message);
        let c = RsaKeyPair::encrypt_le(&key().modulus_le(), &key().public_exponent_le(), &m).unwrap();
        prop_assert_eq!(key().decrypt_le(&c), Some(m));
    }

    /// The XOR layer is its own inverse for every packet size the frame layer allows after the length field.
    #[test]
    fn xor_table_applies_to_any_packet(packet in vec(any::<u8>(), 0..=2046)) {
        let mut data = packet.clone();
        table().apply(&mut data);
        table().apply(&mut data);
        prop_assert_eq!(data, packet);
    }
}

// The largest payload after the length field is 2048 - 2 bytes; twice that must fit the table.
const _: () = assert!(2 * 2046 <= XOR_TABLE_LEN);
