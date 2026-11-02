use std::sync::Arc;

use rand::Rng;
use shaiya_crypto::RsaKeyPair;

/// Pre-generated RSA keys; each new connection gets a random one, like the original `CKeyMgr`.
#[derive(Clone)]
pub struct KeyPool {
    keys: Vec<Arc<RsaKeyPair>>,
}

impl KeyPool {
    pub fn generate(size: usize) -> Self {
        assert!(size > 0, "key pool must not be empty");
        let threads: Vec<_> = (0..size).map(|_| std::thread::spawn(|| Arc::new(RsaKeyPair::generate(&mut rand::rng())))).collect();
        Self { keys: threads.into_iter().map(|t| t.join().expect("key generation thread")).collect() }
    }

    pub fn pick(&self) -> Arc<RsaKeyPair> {
        self.keys[rand::rng().random_range(0..self.keys.len())].clone()
    }
}
