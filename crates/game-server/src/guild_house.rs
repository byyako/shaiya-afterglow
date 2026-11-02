//! Guild houses (`docs/original/guild-house.md`): `GuildHouse.ini`'s rules, a guild's house state with its eight NPC slots, and the
//! pure answers: an upgrade (section 4), the keep Etin (section 6), going in (section 7) and the house NPCs' rates and locks
//! (section 9). The guilds keep the state (`crate::guild`); the zone and the connection apply the answers.

use std::collections::HashMap;

/// NPC keys (`CGuild +0x47c`): the gatekeeper, the blacksmith, the warehouse; a merchant is `shop type << 8 | 1`.
pub const KEY_GATEKEEPER: u16 = 2;
pub const KEY_SMITH: u16 = 3;
pub const KEY_BANK: u16 = 6;
pub const KEY_WEAPONS: u16 = 0x0101;
pub const KEY_ARMOUR: u16 = 0x0201;
pub const KEY_GENERAL: u16 = 0x0301;
pub const KEY_POTIONS: u16 = 0x0401;

/// The merchant shop types (`stNpc +0x124`) the guild house's rules know.
pub const SHOP_GENERAL: u8 = 3;
pub const SHOP_POTIONS: u8 = 4;

/// The NPC lists' kinds (`0x541cb0`): the gatekeeper, then shop types 1 to 3.
pub const KIND_GATEKEEPER: u8 = 0;

pub const SLOTS: usize = 8;
/// A guild's ranking when it has none.
pub const UNRANKED: u8 = 31;
/// The best ranking the smith's discounts need (repair, lapis), and the blacksmith's upgrade.
pub const SMITH_RANKING: u8 = 14;
/// The best ranking the potion merchant needs.
pub const POTION_RANKING: u8 = 6;
/// Any ranking (general goods, the warehouse).
pub const ANY_RANKING: u8 = 30;

/// `byUseHouse`.
pub const HOUSE_NOT_OPEN: u8 = 0;
pub const HOUSE_OPEN: u8 = 1;
pub const HOUSE_CLOSED: u8 = 2;

/// `0x0D25` results.
pub mod upgrade {
    pub const DONE: u8 = 0;
    pub const NO_ETIN: u8 = 1;
    pub const ONE_STEP: u8 = 2;
    pub const FAILED: u8 = 3;
    pub const RANKING: u8 = 4;
}

/// `0x0D34` results.
pub mod buy {
    pub const BOUGHT: u8 = 0;
    pub const ALREADY: u8 = 1;
    pub const NO_MONEY: u8 = 2;
    pub const NOT_MASTER: u8 = 3;
}

/// `0x0D28` results: 1 the ranking is not high enough, 2 the NPC's level is locked, 4 not authorized (the guild bank).
pub const REFUSED_RANKING: u8 = 1;
pub const REFUSED_LEVEL: u8 = 2;
pub const REFUSED_NOT_AUTHORIZED: u8 = 4;

/// `0x020A` refusals for a house map (section 7).
pub mod portal {
    pub const NO_GUILD: u8 = 0;
    pub const UNRANKED: u8 = 3;
    pub const NOT_BOUGHT: u8 = 6;
    pub const NOT_OPEN: u8 = 7;
    pub const CLOSED: u8 = 8;
}

/// One NPC key's upkeep and upgrade costs, index 0 = level 1.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NpcCosts {
    pub keep: Vec<u32>,
    pub upgrade: Vec<u32>,
}

/// `GuildHouse.ini` (section 2). The rate lists are by level, index 0 = level 0.
#[derive(Clone, Debug, PartialEq)]
pub struct HouseRules {
    pub buy_money: u32,
    pub keep_etin: u32,
    pub gate_rates: Vec<f64>,
    pub shop_rates: Vec<f64>,
    pub potion_rates: Vec<f64>,
    pub repair_rates: Vec<f64>,
    pub mix_rates: Vec<f64>,
    pub mix_bonus: Vec<i32>,
    pub bank_slots: Vec<u32>,
    pub npcs: HashMap<u16, NpcCosts>,
    /// `(country, kind, level 1 to 3)` → the NPC type id that level unlocks.
    pub levels: HashMap<(u8, u8, u8), u16>,
}

impl Default for HouseRules {
    /// The original's built-in values (`InitGuildHouseIniFile`'s tables) with the shipped NPC lists.
    fn default() -> Self {
        let npcs = [
            (KEY_GATEKEEPER, vec![200, 500, 800], vec![1000, 1500, 2000]),
            (KEY_WEAPONS, vec![], vec![100, 200, 500]),
            (KEY_ARMOUR, vec![], vec![100, 200, 500]),
            (KEY_GENERAL, vec![], vec![150, 200, 400]),
            (KEY_SMITH, vec![100, 200, 500, 800, 1500], vec![500, 1000, 2000, 10000, 15000]),
            (KEY_POTIONS, vec![50, 200, 500], vec![2000, 5000, 10000]),
            (KEY_BANK, vec![100, 200, 500, 800, 1000], vec![200, 500, 1000, 2000, 5000]),
        ]
        .into_iter()
        .map(|(key, keep, upgrade)| (key, NpcCosts { keep, upgrade }))
        .collect();
        let lists: [[[u16; 3]; 4]; 2] = [
            [[37, 38, 39], [196, 198, 197], [200, 201, 202], [220, 221, 222]],
            [[41, 42, 43], [207, 209, 208], [210, 212, 213], [223, 224, 225]],
        ];
        let mut levels = HashMap::new();
        for (country, kinds) in lists.iter().enumerate() {
            for (kind, ids) in kinds.iter().enumerate() {
                for (i, &id) in ids.iter().enumerate() {
                    levels.insert((country as u8, kind as u8, i as u8 + 1), id);
                }
            }
        }
        Self {
            buy_money: 50_000_000,
            keep_etin: 2000,
            gate_rates: vec![0.0; 4],
            shop_rates: vec![0.95, 0.90, 0.85, 0.80],
            potion_rates: vec![0.90, 0.80, 0.70, 0.60],
            repair_rates: vec![0.99, 0.95, 0.95, 0.90, 0.90, 0.90],
            mix_rates: vec![0.90, 0.80, 0.70, 0.60, 0.50, 0.10],
            mix_bonus: vec![0, 0, 1, 1, 2, 2],
            bank_slots: vec![40, 80, 120, 160, 200, 240],
            npcs,
            levels,
        }
    }
}

impl HouseRules {
    /// Whether `npc` is a level of `kind` above `level` for the country, and the first such level (the lock of section 9).
    fn locked(&self, country: u8, kind: u8, level: u16, npc: u16) -> Option<u8> {
        let country = country.min(1);
        (level + 1..=3).map(|l| l as u8).find(|&l| self.levels.get(&(country, kind, l)) == Some(&npc))
    }

    fn cost(&self, key: u16, level: u8) -> u32 {
        let costs = self.npcs.get(&key);
        costs.and_then(|c| c.upgrade.get(usize::from(level).wrapping_sub(1))).copied().unwrap_or(0)
    }
}

fn rate(rates: &[f64], level: u16) -> f64 {
    rates.get(usize::from(level)).copied().unwrap_or(0.0)
}

/// `trunc(value × rate)`, as the original's x87 does with its control word set to truncate.
pub fn scaled(value: u32, rate: f64) -> u32 {
    (f64::from(value) * rate) as u32
}

/// A guild's house (section 1).
#[derive(Clone, Debug, PartialEq)]
pub struct House {
    pub ranking: u8,
    pub points: u32,
    pub etin: u32,
    /// Etin returns since the last ranking (`EtinReturnCnt`).
    pub etin_returns: u32,
    pub keep_etin: u32,
    /// `byUseHouse`.
    pub use_house: u8,
    pub bought: bool,
    /// `(key, level)`, key 0 = unused.
    pub slots: [(u16, u16); SLOTS],
    /// The ranking battle's points and entries so far (memory only, `docs/original/guild-ranking.md` sections 5 and 7).
    pub battle_points: u32,
    pub battle_entries: u32,
}

impl Default for House {
    fn default() -> Self {
        Self {
            ranking: UNRANKED,
            points: 0,
            etin: 0,
            etin_returns: 0,
            keep_etin: 2000,
            use_house: HOUSE_NOT_OPEN,
            bought: false,
            slots: [(0, 0); SLOTS],
            battle_points: 0,
            battle_entries: 0,
        }
    }
}

/// What an upgrade did: the slot it changed, the level, the Etin left and the new keep Etin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Upgraded {
    pub slot: u8,
    pub etin: u32,
    pub keep_etin: u32,
}

impl House {
    /// `CGuild::EnableGuildHouse`: ranked, open and bought.
    pub fn enabled(&self) -> bool {
        self.ranking < UNRANKED && self.use_house == HOUSE_OPEN && self.bought
    }

    /// A key's level, 0 without a slot.
    pub fn level(&self, key: u16) -> u16 {
        self.slots.iter().find(|&&(k, _)| k == key).map_or(0, |&(_, level)| level)
    }

    /// The used slots as `0x0D24` lists them.
    pub fn used_slots(&self) -> Vec<(u16, u8, u8)> {
        self.slots.iter().enumerate().filter(|(_, (key, _))| *key != 0).map(|(i, &(key, level))| (key, level as u8, i as u8)).collect()
    }

    /// `CGuild::CalculateKeepEtin`: the house's upkeep plus each used slot's at its level.
    pub fn keep_etin_of(&self, rules: &HouseRules) -> u32 {
        let slots: u32 = self
            .slots
            .iter()
            .filter(|(key, _)| *key != 0)
            .map(|&(key, level)| rules.npcs.get(&key).and_then(|c| c.keep.get(usize::from(level).wrapping_sub(1))).copied().unwrap_or(0))
            .sum();
        rules.keep_etin + slots
    }

    /// `CGuild::NpcUpgrade` (section 4): the refusal, or the upgrade made (the slot set, the keep Etin worked out again, the cost
    /// taken). A full slot table answers 3 (deviation 4).
    pub fn upgrade(&mut self, rules: &HouseRules, key: u16, level: u8) -> Result<Upgraded, u8> {
        if self.ranking > ANY_RANKING || self.use_house != HOUSE_OPEN || !self.bought || level == 0 || level > 5 {
            return Err(upgrade::FAILED);
        }
        let (best_ranking, max_level) = match key {
            KEY_WEAPONS | KEY_ARMOUR | KEY_GATEKEEPER => (ANY_RANKING, 3),
            KEY_SMITH => (SMITH_RANKING, 5),
            KEY_BANK | KEY_GENERAL => (ANY_RANKING, 5),
            KEY_POTIONS => (POTION_RANKING, 3),
            _ => return Err(upgrade::FAILED),
        };
        if self.ranking > best_ranking {
            return Err(upgrade::RANKING);
        }
        if level > max_level {
            return Err(upgrade::FAILED);
        }
        let spendable = i64::from(self.etin) - i64::from(self.keep_etin);
        if spendable < 1 {
            return Err(upgrade::NO_ETIN);
        }
        let cost = rules.cost(key, level);
        if i64::from(cost) > spendable {
            return Err(upgrade::NO_ETIN);
        }
        if cost == 0 {
            return Err(upgrade::FAILED);
        }
        let slot = match self.slots.iter().position(|&(k, _)| k == key) {
            Some(slot) if u16::from(level) != self.slots[slot].1 + 1 => return Err(upgrade::ONE_STEP),
            Some(slot) => slot,
            None if level != 1 => return Err(upgrade::FAILED),
            None => self.slots.iter().position(|&(k, _)| k == 0).ok_or(upgrade::FAILED)?,
        };
        self.slots[slot] = (key, u16::from(level));
        self.keep_etin = self.keep_etin_of(rules);
        self.etin -= cost;
        Ok(Upgraded { slot: slot as u8, etin: self.etin, keep_etin: self.keep_etin })
    }

    /// The `0x020A` refusal for a house map (section 7), `None` to go in.
    pub fn portal_refusal(&self) -> Option<u8> {
        if self.ranking >= UNRANKED {
            Some(portal::UNRANKED)
        } else if !self.bought {
            Some(portal::NOT_BOUGHT)
        } else {
            match self.use_house {
                HOUSE_OPEN => None,
                HOUSE_CLOSED => Some(portal::CLOSED),
                _ => Some(portal::NOT_OPEN),
            }
        }
    }

    /// `CGuild::GetGuildHouseShopBuyRate` (section 9): the rate for a merchant of `shop_type` (its NPC type id `npc`), or the
    /// `0x0D28` refusal.
    pub fn shop_rate(&self, rules: &HouseRules, country: u8, npc: u16, shop_type: u8) -> Result<f64, (u8, u8)> {
        let level = self.level(u16::from(shop_type) << 8 | 1).min(3);
        let rate = if shop_type == SHOP_POTIONS { rate(&rules.potion_rates, level) } else { rate(&rules.shop_rates, level) };
        match shop_type {
            1 | 2 | SHOP_GENERAL => {
                if shop_type == SHOP_GENERAL && self.ranking > ANY_RANKING {
                    return Err((REFUSED_RANKING, ANY_RANKING));
                }
                match rules.locked(country, shop_type, level, npc) {
                    Some(needed) => Err((REFUSED_LEVEL, needed)),
                    None => Ok(rate),
                }
            }
            SHOP_POTIONS if self.ranking > POTION_RANKING => Err((REFUSED_RANKING, POTION_RANKING)),
            _ => Ok(rate),
        }
    }

    /// `CGuild::GetGuildHouseUseGateRate`: the gatekeeper's rate, or the locked level.
    pub fn gate_rate(&self, rules: &HouseRules, country: u8, npc: u16) -> Result<f64, u8> {
        let level = self.level(KEY_GATEKEEPER).min(3);
        match rules.locked(country, KIND_GATEKEEPER, level, npc) {
            Some(needed) => Err(needed),
            None => Ok(rate(&rules.gate_rates, level)),
        }
    }

    fn smith_level(&self) -> u16 {
        self.level(KEY_SMITH).min(5)
    }

    /// The blacksmith's repair rate (`GetGuildHouseRepairRate`).
    pub fn repair_rate(&self, rules: &HouseRules) -> f64 {
        rate(&rules.repair_rates, self.smith_level())
    }

    /// The lapis price rate (`GetGuildHouseRapiceMixDecreRate`).
    pub fn mix_rate(&self, rules: &HouseRules) -> f64 {
        rate(&rules.mix_rates, self.smith_level())
    }

    /// The lapis linking bonus (`GetGuildHouseRapiceMixPercentRate`), only for a guild whose house is enabled.
    pub fn mix_bonus(&self, rules: &HouseRules) -> i32 {
        if self.enabled() { rules.mix_bonus.get(usize::from(self.smith_level())).copied().unwrap_or(0) } else { 0 }
    }
}

/// The Etin an item gives back (section 3): count × 1, 10, 100 or 1000 by its special 52 to 55.
pub fn etin_of(special: u8, count: u8) -> u32 {
    let each = match special {
        52 => 1,
        53 => 10,
        54 => 100,
        55 => 1000,
        _ => 0,
    };
    each * u32::from(count)
}

/// Item type of the Etin.
pub const ETIN_TYPE: u8 = 43;

#[cfg(test)]
mod tests {
    use super::*;

    fn open_house(ranking: u8, etin: u32) -> House {
        House { ranking, etin, use_house: HOUSE_OPEN, bought: true, ..House::default() }
    }

    #[test]
    fn upgrade_rules() {
        let rules = HouseRules::default();
        let mut h = open_house(5, 5000);
        assert_eq!(h.upgrade(&rules, KEY_GATEKEEPER, 2), Err(upgrade::FAILED), "a new slot starts at level 1");
        let done = h.upgrade(&rules, KEY_GATEKEEPER, 1).unwrap();
        assert_eq!(done, Upgraded { slot: 0, etin: 4000, keep_etin: 2200 });
        // The cost is checked before the step: level 3 costs 2000, 1800 to spend.
        assert_eq!(h.upgrade(&rules, KEY_GATEKEEPER, 3), Err(upgrade::NO_ETIN));
        h.etin = 9000;
        assert_eq!(h.upgrade(&rules, KEY_GATEKEEPER, 3), Err(upgrade::ONE_STEP));
        h.etin = 4000;
        // 4000 - 2200 = 1800 to spend; level 2 costs 1500.
        assert_eq!(h.upgrade(&rules, KEY_GATEKEEPER, 2), Ok(Upgraded { slot: 0, etin: 2500, keep_etin: 2500 }));
    }

    #[test]
    fn upgrade_costs_against_spendable_etin() {
        let rules = HouseRules::default();
        let mut h = open_house(5, 2000 + 1500);
        h.slots[0] = (KEY_GATEKEEPER, 1);
        h.keep_etin = h.keep_etin_of(&rules);
        assert_eq!(h.keep_etin, 2200);
        // 3500 - 2200 = 1300 to spend, level 2 costs 1500.
        assert_eq!(h.upgrade(&rules, KEY_GATEKEEPER, 2), Err(upgrade::NO_ETIN));
        h.etin = 3700;
        let done = h.upgrade(&rules, KEY_GATEKEEPER, 2).unwrap();
        assert_eq!(done, Upgraded { slot: 0, etin: 2200, keep_etin: 2500 });
        let mut broke = open_house(5, 2000);
        assert_eq!(broke.upgrade(&rules, KEY_WEAPONS, 1), Err(upgrade::NO_ETIN), "nothing above the keep Etin");
    }

    #[test]
    fn upgrade_refusals_in_order() {
        let rules = HouseRules::default();
        assert_eq!(open_house(31, 9999).upgrade(&rules, KEY_WEAPONS, 1), Err(upgrade::FAILED), "unranked");
        assert_eq!(House { use_house: HOUSE_CLOSED, ..open_house(5, 9999) }.upgrade(&rules, KEY_WEAPONS, 1), Err(upgrade::FAILED));
        assert_eq!(House { bought: false, ..open_house(5, 9999) }.upgrade(&rules, KEY_WEAPONS, 1), Err(upgrade::FAILED));
        assert_eq!(open_house(5, 99999).upgrade(&rules, KEY_WEAPONS, 4), Err(upgrade::FAILED), "weapons stop at 3");
        assert_eq!(open_house(15, 99999).upgrade(&rules, KEY_SMITH, 1), Err(upgrade::RANKING));
        assert_eq!(open_house(14, 99999).upgrade(&rules, KEY_SMITH, 1).map(|u| u.slot), Ok(0));
        assert_eq!(open_house(7, 99999).upgrade(&rules, KEY_POTIONS, 1), Err(upgrade::RANKING));
        assert_eq!(open_house(6, 99999).upgrade(&rules, KEY_POTIONS, 4), Err(upgrade::FAILED));
        assert_eq!(open_house(1, 99999).upgrade(&rules, 0x0501, 1), Err(upgrade::FAILED), "unknown key");
        let mut general = open_house(1, 99999);
        general.slots[0] = (KEY_GENERAL, 3);
        assert_eq!(general.upgrade(&rules, KEY_GENERAL, 4), Err(upgrade::FAILED), "no cost for level 4");
    }

    #[test]
    fn slots_fill_in_order_and_a_full_table_fails() {
        let rules = HouseRules::default();
        let mut h = open_house(1, 1_000_000);
        h.slots = [(9, 1); SLOTS];
        assert_eq!(h.upgrade(&rules, KEY_WEAPONS, 1), Err(upgrade::FAILED), "deviation 4");
        h.slots[3] = (0, 0);
        assert_eq!(h.upgrade(&rules, KEY_WEAPONS, 1).map(|u| u.slot), Ok(3));
    }

    #[test]
    fn portal_order() {
        let h = open_house(5, 0);
        assert_eq!(h.portal_refusal(), None);
        assert_eq!(House { ranking: 31, ..h.clone() }.portal_refusal(), Some(portal::UNRANKED));
        assert_eq!(House { bought: false, ..h.clone() }.portal_refusal(), Some(portal::NOT_BOUGHT));
        assert_eq!(House { use_house: HOUSE_NOT_OPEN, ..h.clone() }.portal_refusal(), Some(portal::NOT_OPEN));
        assert_eq!(House { use_house: HOUSE_CLOSED, ..h }.portal_refusal(), Some(portal::CLOSED));
    }

    #[test]
    fn shop_rates_and_locks() {
        let rules = HouseRules::default();
        let mut h = open_house(10, 0);
        // Level 0 weapons: 0.95, the base merchant 195 free, 196 (level 1) locked.
        assert_eq!(h.shop_rate(&rules, 0, 195, 1), Ok(0.95));
        assert_eq!(h.shop_rate(&rules, 0, 196, 1), Err((REFUSED_LEVEL, 1)));
        assert_eq!(h.shop_rate(&rules, 0, 197, 1), Err((REFUSED_LEVEL, 3)));
        h.slots[0] = (KEY_WEAPONS, 1);
        assert_eq!(h.shop_rate(&rules, 0, 196, 1), Ok(0.90));
        assert_eq!(h.shop_rate(&rules, 0, 198, 1), Err((REFUSED_LEVEL, 2)));
        // Fury's lists.
        assert_eq!(h.shop_rate(&rules, 1, 209, 1), Err((REFUSED_LEVEL, 2)));
        // Potions need ranking 6.
        assert_eq!(h.shop_rate(&rules, 0, 203, SHOP_POTIONS), Err((REFUSED_RANKING, 6)));
        assert_eq!(open_house(6, 0).shop_rate(&rules, 0, 203, SHOP_POTIONS), Ok(0.90));
        assert_eq!(scaled(1000, 0.95), 950);
        assert_eq!(scaled(333, 0.95), 316, "truncated");
    }

    #[test]
    fn gate_and_smith() {
        let rules = HouseRules::default();
        let mut h = open_house(10, 0);
        assert_eq!(h.gate_rate(&rules, 0, 36), Ok(0.0), "free");
        assert_eq!(h.gate_rate(&rules, 0, 37), Err(1));
        h.slots[1] = (KEY_SMITH, 2);
        assert_eq!(h.repair_rate(&rules), 0.95);
        assert_eq!(h.mix_rate(&rules), 0.70);
        assert_eq!(h.mix_bonus(&rules), 1);
        assert_eq!(House { use_house: HOUSE_CLOSED, ..h }.mix_bonus(&rules), 0);
    }

    #[test]
    fn etin_values() {
        assert_eq!(etin_of(52, 3), 3);
        assert_eq!(etin_of(55, 2), 2000);
        assert_eq!(etin_of(7, 9), 0);
    }
}
