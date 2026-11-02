//! The operator's switches (`docs/original/operator-console.md` sections 2 and 4): what the console commands turn while the server
//! runs, each starting from its setting and lasting until the next start (decision 5). Read from the connections and the zones.

use std::sync::atomic::{AtomicU8, AtomicU32, AtomicU64, Ordering};

/// The item mall is open (`g_bDisableShop` 0).
pub const SHOP_ON: u8 = 0;
/// Purchases and gifts answer their failure (`g_bDisableShop` 1, `GAME_ITEM_MALL=false`).
pub const SHOP_OFF: u8 = 1;
/// Purchases and gifts get no answer at all (`g_bDisableShop` 2).
pub const SHOP_SILENT: u8 = 2;

pub struct Switches {
    /// The monster experience event's factor, as `f64` bits (`g_fExp2XValue`).
    exp_rate: AtomicU64,
    /// What a counted player kill adds.
    kill_rate: AtomicU32,
    shop: AtomicU8,
    /// New connections are refused while this many or more are open (`/setmaxuser`).
    max_users: AtomicU32,
}

impl Switches {
    pub fn new(exp_rate: f64, kill_rate: u32, item_mall: bool, max_users: u32) -> Self {
        Self {
            exp_rate: AtomicU64::new(exp_rate.to_bits()),
            kill_rate: AtomicU32::new(kill_rate),
            shop: AtomicU8::new(if item_mall { SHOP_ON } else { SHOP_OFF }),
            max_users: AtomicU32::new(max_users),
        }
    }

    pub fn exp_rate(&self) -> f64 {
        f64::from_bits(self.exp_rate.load(Ordering::Relaxed))
    }

    pub fn set_exp_rate(&self, rate: f64) {
        self.exp_rate.store(rate.to_bits(), Ordering::Relaxed);
    }

    pub fn kill_rate(&self) -> u32 {
        self.kill_rate.load(Ordering::Relaxed)
    }

    pub fn set_kill_rate(&self, rate: u32) {
        self.kill_rate.store(rate, Ordering::Relaxed);
    }

    /// `SHOP_ON`, `SHOP_OFF` or `SHOP_SILENT`.
    pub fn shop(&self) -> u8 {
        self.shop.load(Ordering::Relaxed)
    }

    pub fn set_shop(&self, shop: u8) {
        self.shop.store(shop, Ordering::Relaxed);
    }

    pub fn max_users(&self) -> u32 {
        self.max_users.load(Ordering::Relaxed)
    }

    pub fn set_max_users(&self, max: u32) {
        self.max_users.store(max, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_switches_start_from_the_settings() {
        let s = Switches::new(1.5, 3, false, 5000);
        assert_eq!((s.exp_rate(), s.kill_rate(), s.shop(), s.max_users()), (1.5, 3, SHOP_OFF, 5000));
        s.set_exp_rate(0.0);
        s.set_kill_rate(2);
        s.set_shop(SHOP_SILENT);
        s.set_max_users(7);
        assert_eq!((s.exp_rate(), s.kill_rate(), s.shop(), s.max_users()), (0.0, 2, SHOP_SILENT, 7));
        assert_eq!(Switches::new(1.0, 1, true, 1).shop(), SHOP_ON);
    }
}
