//! Item effects and skill abilities (`docs/plans/ext-3-effects.md`): named, parameterised behaviours written in Rust that an
//! extension's rows name in `rows.toml` (`effect` on an `[[item]]`, `abilities` on a `[[skill]]`). This module is pure: the types, the
//! one parser the data editor and the game server share (section 3.4), the table of effects by item (`ItemEffects`) and the parts of
//! each effect that need no connection (`level_target`, `Rate`, the jump). The handlers are `server/effects.rs` (the use chain and
//! `level_up`), `lapis.rs` (`enchant`, `recreate`) and the places that read an ability (`Zones::kill_boost`).
//!
//! A new effect type is a variant here, an arm in [`check_effect`] and [`fits_row`], and a handler (section 3.5).

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Result, anyhow, bail};
use serde_json::{Map, Value};

use crate::inventory::real_type;

/// The item effect types, in the order the documentation lists them.
pub const EFFECT_TYPES: &[&str] = &["level_up", "enchant", "recreate"];
/// The skill ability types.
pub const ABILITY_TYPES: &[&str] = &["kill_boost"];
/// How many abilities one skill level holds.
pub const MAX_ABILITIES: usize = 4;
/// The Lapisia's item type (`crate::lapis::LAPISIA_TYPE`).
const LAPISIA_TYPE: u8 = 95;
/// The real type of the items the core uses (item types 25, 44 and 100).
const REAL_TYPE_USABLE: u8 = 26;
/// The `special` of a Recreation Rune.
const RECREATION_RUNE: u8 = 62;
/// The highest level (`combat::max_level` of a mode that is not easy).
const LAST_LEVEL: u64 = 60;
/// The enchant roll draws below one million, and a rate of that many always succeeds.
const SURE: u32 = 1_000_000;
/// The first id of the refusals the effects make (the whisper limit is per id): above the rules' ids, which count up from 1, and the
/// scripts', which count up from 1,000,000,000 (`script/points.rs`).
pub const REFUSAL_BASE: u32 = 2_000_000_000;
/// The nine crafted numbers, in the order of a craft name.
pub const STATS: [&str; 9] = ["str", "dex", "rec", "int", "wis", "luc", "hp", "mp", "sp"];

/// The request an effect acts in (section 3.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    /// The item's own use (`0x050A`, `0x0557`, `0x0559`).
    Use,
    /// The Lapisia of an enchant (`0x0805`).
    Enchant,
    /// The rune of a recreation (`0x0806`).
    Recreate,
}

/// How a level stone gives its levels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LevelBy {
    /// This many levels more than the user has.
    Levels(u16),
    /// Up to this level.
    To(u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LevelUp {
    pub by: LevelBy,
    /// The stone gives nothing above this level.
    pub max: Option<u16>,
}

/// A Lapisia's own jump table: `(steps, weight)` and the weights' sum.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Jumps {
    pub list: Vec<(u8, u32)>,
    pub total: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnchantEffect {
    /// The success rate in millionths, replacing the step's; 1,000,000 always succeeds.
    pub rate: u32,
    /// The jump on success; without one the `enchant` rules are asked.
    pub jumps: Option<Jumps>,
    /// A failure never destroys the item.
    pub safe: bool,
}

/// What number a Recreation Rune gives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecreateValue {
    /// The smaller of two draws up to the item's `reqwis`, as the core's rune.
    Random,
    /// The item's `reqwis`.
    Max,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Recreate {
    /// Which of the nine crafted numbers (`STATS`) it redraws.
    pub stats: [bool; 9],
    pub value: RecreateValue,
}

/// An item effect (section 3.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemEffect {
    LevelUp(LevelUp),
    Enchant(EnchantEffect),
    Recreate(Recreate),
}

impl ItemEffect {
    /// The type's name as `rows.toml` spells it.
    pub fn type_name(&self) -> &'static str {
        match self {
            ItemEffect::LevelUp(_) => "level_up",
            ItemEffect::Enchant(_) => "enchant",
            ItemEffect::Recreate(_) => "recreate",
        }
    }

    /// The id of this type's refusals (section 3.3): one per effect type.
    pub fn refusal_id(&self) -> u32 {
        REFUSAL_BASE + EFFECT_TYPES.iter().position(|t| *t == self.type_name()).unwrap_or(0) as u32
    }

    pub fn request(&self) -> Request {
        match self {
            ItemEffect::LevelUp(_) => Request::Use,
            ItemEffect::Enchant(_) => Request::Enchant,
            ItemEffect::Recreate(_) => Request::Recreate,
        }
    }

    /// The type and parameters in a line (`level_up levels = 1`), for `ext check`, `ext show` and the logs.
    pub fn describe(&self) -> String {
        match self {
            ItemEffect::LevelUp(LevelUp { by, max }) => {
                let by = match by {
                    LevelBy::Levels(n) => format!("levels = {n}"),
                    LevelBy::To(n) => format!("to = {n}"),
                };
                format!("level_up {by}{}", max.map_or(String::new(), |m| format!(", max = {m}")))
            }
            ItemEffect::Enchant(e) => {
                let jumps = e.jumps.as_ref().map_or(String::new(), |j| {
                    let list: Vec<String> = j.list.iter().map(|(steps, weight)| format!("[{steps}, {weight}]")).collect();
                    format!(", jumps = [{}]", list.join(", "))
                });
                format!("enchant rate = {}{jumps}, safe = {}", e.rate, e.safe)
            }
            ItemEffect::Recreate(r) => {
                let stats: Vec<String> = STATS.iter().zip(r.stats).filter(|(_, on)| *on).map(|(name, _)| format!("\"{name}\"")).collect();
                let value = match r.value {
                    RecreateValue::Random => "random",
                    RecreateValue::Max => "max",
                };
                format!("recreate stats = [{}], value = \"{value}\"", stats.join(", "))
            }
        }
    }
}

/// A skill ability (section 3.2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Ability {
    /// An empty place of [`Abilities`].
    #[default]
    None,
    /// A counted player kill counts this many times while the skill is applied.
    KillBoost { times: u32 },
}

impl Ability {
    pub fn type_name(&self) -> &'static str {
        match self {
            Ability::None => "none",
            Ability::KillBoost { .. } => "kill_boost",
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Ability::None => "none".to_owned(),
            Ability::KillBoost { times } => format!("kill_boost times = {times}"),
        }
    }
}

/// The abilities of one skill level: a small `Copy` value, at most [`MAX_ABILITIES`], so a `SkillDef` that carries them stays `Copy`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Abilities([Ability; MAX_ABILITIES]);

impl Abilities {
    pub fn is_empty(&self) -> bool {
        self.0[0] == Ability::None
    }

    pub fn len(&self) -> usize {
        self.0.iter().take_while(|a| **a != Ability::None).count()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Ability> {
        self.0.iter().take(self.len())
    }

    /// Adds an ability; `false` when the four places are taken.
    pub fn push(&mut self, ability: Ability) -> bool {
        match self.0.iter_mut().find(|a| **a == Ability::None) {
            Some(place) => {
                *place = ability;
                true
            }
            None => false,
        }
    }

    /// The highest `times` among the `kill_boost` abilities (section 5.4).
    pub fn kill_boost(&self) -> Option<u32> {
        self.iter().filter_map(|a| if let Ability::KillBoost { times } = a { Some(*times) } else { None }).max()
    }

    /// A `kill_boost` of `times` from the `kill_boost_skills` shorthand (section 5.4): raises an existing `kill_boost` to `times` when
    /// that is higher, else adds one. `false` when it would need a fifth place.
    pub fn merge_kill_boost(&mut self, times: u32) -> bool {
        match self.0.iter_mut().find(|a| matches!(a, Ability::KillBoost { .. })) {
            Some(Ability::KillBoost { times: have }) => {
                *have = (*have).max(times);
                true
            }
            _ => self.push(Ability::KillBoost { times }),
        }
    }
}

impl<'a> IntoIterator for &'a Abilities {
    type Item = &'a Ability;
    type IntoIter = std::iter::Take<std::slice::Iter<'a, Ability>>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter().take(self.len())
    }
}

/// The item id of an item type and type id (`defs.items.itemid`).
pub fn item_id(item_type: u8, type_id: u8) -> u32 {
    u32::from(item_type) * 1000 + u32::from(type_id)
}

/// The item effects in force: item id to effect (section 3.4). An empty table is a server without extensions.
#[derive(Clone, Debug, Default)]
pub struct ItemEffects(HashMap<u32, Arc<ItemEffect>>);

impl ItemEffects {
    pub fn new(effects: impl IntoIterator<Item = (u32, ItemEffect)>) -> Self {
        Self(effects.into_iter().map(|(id, effect)| (id, Arc::new(effect))).collect())
    }

    pub fn get(&self, item_id: u32) -> Option<&Arc<ItemEffect>> {
        self.0.get(&item_id)
    }

    /// The effect of the `use` request an item has, if any: what its use does in place of the core's (section 3.3).
    pub fn for_use(&self, item_id: u32) -> Option<&Arc<ItemEffect>> {
        self.get(item_id).filter(|e| e.request() == Request::Use)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// How many items have each type, in the order of [`EFFECT_TYPES`] (every type, with 0 for none).
    pub fn counts(&self) -> Vec<(&'static str, usize)> {
        EFFECT_TYPES.iter().map(|&ty| (ty, self.0.values().filter(|e| e.type_name() == ty).count())).collect()
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The parser (section 3.4)

fn take_whole(obj: &mut Map<String, Value>, key: &str, low: u64, high: u64) -> Result<Option<u64>> {
    let Some(value) = obj.remove(key) else { return Ok(None) };
    match (value.as_u64(), value.as_i64()) {
        (Some(n), _) if (low..=high).contains(&n) => Ok(Some(n)),
        (Some(_), _) | (None, Some(_)) => bail!("{key}: {value} is not {low} to {high}"),
        (None, None) => bail!("{key}: {value} is not a whole number {low} to {high}"),
    }
}

fn take_flag(obj: &mut Map<String, Value>, key: &str) -> Result<Option<bool>> {
    match obj.remove(key) {
        None => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(b)),
        Some(other) => bail!("{key}: {other} is not true or false"),
    }
}

/// The keys that belong to one type or another, to say "not a key of this type" instead of only "unknown".
const OWN_EFFECT_KEYS: &[&str] = &["levels", "to", "max", "rate", "jumps", "safe", "stats", "value"];

fn leftover(obj: &Map<String, Value>, ty: &str, own: &[&str]) -> Result<()> {
    if let Some(key) = obj.keys().next() {
        let note = if own.contains(&key.as_str()) { format!(" (not a key of the {ty} effect)") } else { String::new() };
        bail!("unknown key {key:?}{note}");
    }
    Ok(())
}

/// Parses an item effect (`effect = { type = "level_up", levels = 1 }`); every error names the key and says what is allowed, so the
/// caller adds the entry (`effect: levels: 0 is not 1 to 59`).
pub fn check_effect(value: &Value) -> Result<ItemEffect> {
    let Value::Object(map) = value else { bail!("an effect is a table {{ type = \"…\", … }}") };
    let mut obj = map.clone();
    let ty = match obj.remove("type") {
        None => bail!("an effect needs a type ({})", EFFECT_TYPES.join(", ")),
        Some(Value::String(ty)) => ty,
        Some(other) => bail!("type: {other} is not a text"),
    };
    match ty.as_str() {
        "level_up" => {
            let levels = take_whole(&mut obj, "levels", 1, LAST_LEVEL - 1)?;
            let to = take_whole(&mut obj, "to", 2, LAST_LEVEL)?;
            let max = take_whole(&mut obj, "max", 2, LAST_LEVEL)?.map(|m| m as u16);
            let by = match (levels, to) {
                (Some(n), None) => LevelBy::Levels(n as u16),
                (None, Some(n)) => LevelBy::To(n as u16),
                (Some(_), Some(_)) => bail!("levels and to exclude each other: give one of them"),
                (None, None) => bail!("a level_up effect needs one of levels (1 to 59) and to (2 to 60)"),
            };
            leftover(&obj, &ty, OWN_EFFECT_KEYS)?;
            Ok(ItemEffect::LevelUp(LevelUp { by, max }))
        }
        "enchant" => {
            let rate = take_whole(&mut obj, "rate", 1, u64::from(SURE))?;
            let jumps = match obj.remove("jumps") {
                None => None,
                Some(value) => {
                    let list = crate::rules::parse_jumps(&value).map_err(|msg| anyhow!("jumps: {msg}"))?;
                    let total = list.iter().map(|&(_, weight)| u64::from(weight)).sum();
                    Some(Jumps { list, total })
                }
            };
            let safe = take_flag(&mut obj, "safe")?;
            if rate.is_none() && jumps.is_none() && safe.is_none() {
                bail!("an enchant effect needs at least one of rate, jumps, safe");
            }
            leftover(&obj, &ty, OWN_EFFECT_KEYS)?;
            Ok(ItemEffect::Enchant(EnchantEffect { rate: rate.map_or(SURE, |r| r as u32), jumps, safe: safe.unwrap_or(false) }))
        }
        "recreate" => {
            let Some(list) = obj.remove("stats") else { bail!("a recreate effect needs stats, e.g. [\"str\"]") };
            let Value::Array(list) = list else { bail!("stats: {list} is not a list of {}", quoted_stats()) };
            if list.is_empty() || list.len() > STATS.len() {
                bail!("stats: {} entries is not 1 to {}", list.len(), STATS.len());
            }
            let mut stats = [false; 9];
            for item in &list {
                let Some(at) = item.as_str().and_then(|name| STATS.iter().position(|s| *s == name)) else {
                    bail!("stats: {item} is not one of {}", quoted_stats());
                };
                if std::mem::replace(&mut stats[at], true) {
                    bail!("stats: {item} is listed twice");
                }
            }
            let value = match obj.remove("value") {
                None => RecreateValue::Random,
                Some(Value::String(s)) if s == "random" => RecreateValue::Random,
                Some(Value::String(s)) if s == "max" => RecreateValue::Max,
                Some(other) => bail!("value: {other} is not \"random\" or \"max\""),
            };
            leftover(&obj, &ty, OWN_EFFECT_KEYS)?;
            Ok(ItemEffect::Recreate(Recreate { stats, value }))
        }
        other => bail!("type: unknown type {other:?}; the types are {}", EFFECT_TYPES.join(", ")),
    }
}

fn quoted_stats() -> String {
    STATS.iter().map(|s| format!("\"{s}\"")).collect::<Vec<_>>().join(", ")
}

/// Whether the effect suits the row it is on (the last column of section 3.1): `Err` says what the row must be. `item_type` and
/// `special` are the computed row's, after `copy` and `set`.
pub fn fits_row(effect: &ItemEffect, item_type: u8, special: u8) -> Result<(), String> {
    match effect {
        ItemEffect::LevelUp(_) if real_type(item_type) != REAL_TYPE_USABLE => {
            Err(format!("effect level_up needs an item the core uses (item type 25, 44 or 100); this item is type {item_type}"))
        }
        ItemEffect::Enchant(_) if item_type != LAPISIA_TYPE => {
            Err(format!("effect enchant needs a Lapisia (item type {LAPISIA_TYPE}); this item is type {item_type}"))
        }
        ItemEffect::Recreate(_) if special != RECREATION_RUNE => {
            Err(format!("effect recreate needs a Recreation Rune (special {RECREATION_RUNE}); this item has special {special}"))
        }
        _ => Ok(()),
    }
}

/// Parses a skill level's abilities (`abilities = [{ type = "kill_boost", times = 2 }]`): 1 to 4 tables. Errors name the key as
/// `check_effect`'s do, after the place in the list (`ability 1: times: 0 is not 1 to 100`).
pub fn check_abilities(value: &Value) -> Result<Abilities> {
    let Value::Array(list) = value else { bail!("abilities is a list of tables [{{ type = \"…\", … }}]") };
    if list.is_empty() || list.len() > MAX_ABILITIES {
        bail!("{} abilities is not 1 to {MAX_ABILITIES}", list.len());
    }
    let mut out = Abilities::default();
    for (i, item) in list.iter().enumerate() {
        let ability = one_ability(item).map_err(|e| anyhow!("ability {}: {e}", i + 1))?;
        out.push(ability);
    }
    Ok(out)
}

fn one_ability(value: &Value) -> Result<Ability> {
    let Value::Object(map) = value else { bail!("an ability is a table {{ type = \"…\", … }}") };
    let mut obj = map.clone();
    let ty = match obj.remove("type") {
        None => bail!("an ability needs a type ({})", ABILITY_TYPES.join(", ")),
        Some(Value::String(ty)) => ty,
        Some(other) => bail!("type: {other} is not a text"),
    };
    match ty.as_str() {
        "kill_boost" => {
            let Some(times) = take_whole(&mut obj, "times", 1, 100)? else { bail!("a kill_boost ability needs times, 1 to 100") };
            if let Some(key) = obj.keys().next() {
                bail!("unknown key {key:?}");
            }
            Ok(Ability::KillBoost { times: times as u32 })
        }
        other => bail!("type: unknown type {other:?}; the types are {}", ABILITY_TYPES.join(", ")),
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The pure parts of each effect

/// The level a level stone takes a user at `level` of mode `grow` to (section 5.1): the stone's target, at most its `max` and the
/// mode's cap; `None` when that is not above the level.
pub fn level_target(level: u16, grow: u8, stone: &LevelUp) -> Option<u16> {
    let wanted = match stone.by {
        LevelBy::Levels(n) => level.saturating_add(n),
        LevelBy::To(n) => n,
    };
    let target = wanted.min(stone.max.unwrap_or(u16::MAX)).min(crate::combat::max_level(grow));
    (target > level).then_some(target)
}

/// Whether an enchant succeeds (section 5.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rate {
    /// 1,000,000: no draw is needed (the core's test `material + draw < rate` could fail at the highest draws).
    Always,
    /// Succeeds when the material count plus the draw is below this.
    Below(u32),
}

impl Rate {
    /// Whether an enchant with `material` (at most 100) counted and a `draw` below 1,000,000 succeeds.
    pub fn succeeds(self, material: u32, draw: u32) -> bool {
        match self {
            Rate::Always => true,
            Rate::Below(rate) => material + draw < rate,
        }
    }
}

impl EnchantEffect {
    pub fn rate(&self) -> Rate {
        if self.rate >= SURE { Rate::Always } else { Rate::Below(self.rate) }
    }

    /// The jump on success from a draw below 1,000,000, with the Lapisia's own table; `None` without one (the rules are asked).
    pub fn jump(&self, draw: u32) -> Option<u8> {
        self.jumps.as_ref().map(|j| crate::lapis::weighted_jump(&j.list, j.total, draw))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn effect(v: Value) -> ItemEffect {
        check_effect(&v).unwrap_or_else(|e| panic!("{e}"))
    }

    fn error(v: Value) -> String {
        check_effect(&v).unwrap_err().to_string()
    }

    #[test]
    fn every_type_parses() {
        assert_eq!(
            effect(json!({ "type": "level_up", "levels": 1 })),
            ItemEffect::LevelUp(LevelUp { by: LevelBy::Levels(1), max: None })
        );
        assert_eq!(
            effect(json!({ "type": "level_up", "to": 30, "max": 50 })),
            ItemEffect::LevelUp(LevelUp { by: LevelBy::To(30), max: Some(50) })
        );
        assert!(check_effect(&json!({ "type": "level_up", "levels": 59 })).is_ok());
        assert!(check_effect(&json!({ "type": "level_up", "to": 2 })).is_ok() && check_effect(&json!({ "type": "level_up", "to": 60 })).is_ok());
        let ItemEffect::Enchant(e) = effect(json!({ "type": "enchant", "rate": 1000000, "jumps": [[1, 60], [2, 30], [3, 10]], "safe": true })) else {
            panic!()
        };
        assert_eq!((e.rate, e.safe), (1_000_000, true));
        assert_eq!(e.jumps, Some(Jumps { list: vec![(1, 60), (2, 30), (3, 10)], total: 100 }));
        let ItemEffect::Enchant(only_safe) = effect(json!({ "type": "enchant", "safe": true })) else { panic!() };
        assert_eq!((only_safe.rate, only_safe.jumps), (1_000_000, None), "the rate defaults to always (section 3.1)");
        let ItemEffect::Enchant(only_rate) = effect(json!({ "type": "enchant", "rate": 250000 })) else { panic!() };
        assert!(!only_rate.safe);
        let ItemEffect::Recreate(r) = effect(json!({ "type": "recreate", "stats": ["str", "luc"] })) else { panic!() };
        assert_eq!(r.stats, [true, false, false, false, false, true, false, false, false]);
        assert_eq!(r.value, RecreateValue::Random);
        let ItemEffect::Recreate(r) = effect(json!({ "type": "recreate", "stats": ["hp", "mp", "sp"], "value": "max" })) else { panic!() };
        assert_eq!((r.stats[6..].to_vec(), r.value), (vec![true; 3], RecreateValue::Max));
        let all: Vec<&str> = STATS.to_vec();
        assert!(check_effect(&json!({ "type": "recreate", "stats": all })).is_ok(), "all nine");
        assert_eq!(effect(json!({ "type": "level_up", "levels": 1 })).request(), Request::Use);
        assert_eq!(effect(json!({ "type": "enchant", "safe": false })).request(), Request::Enchant);
        assert_eq!(effect(json!({ "type": "recreate", "stats": ["str"] })).request(), Request::Recreate);
        // Each type refuses under an id of its own, above the rules' and the scripts'.
        let ids: Vec<u32> = [json!({ "type": "level_up", "levels": 1 }), json!({ "type": "enchant", "safe": true }), json!({ "type": "recreate", "stats": ["str"] })]
            .into_iter()
            .map(|v| effect(v).refusal_id())
            .collect();
        assert_eq!(ids, [REFUSAL_BASE, REFUSAL_BASE + 1, REFUSAL_BASE + 2]);
        assert_eq!(item_id(100, 215), 100215);
    }

    #[test]
    fn every_error_names_the_key() {
        assert_eq!(error(json!({ "type": "level_up", "levels": 0 })), "levels: 0 is not 1 to 59");
        assert_eq!(error(json!({ "type": "level_up", "levels": 60 })), "levels: 60 is not 1 to 59");
        assert_eq!(error(json!({ "type": "level_up", "levels": -1 })), "levels: -1 is not 1 to 59");
        assert!(error(json!({ "type": "level_up", "levels": 1.5 })).starts_with("levels: 1.5 is not a whole number"));
        assert_eq!(error(json!({ "type": "level_up", "to": 1 })), "to: 1 is not 2 to 60");
        assert_eq!(error(json!({ "type": "level_up", "to": 61 })), "to: 61 is not 2 to 60");
        assert_eq!(error(json!({ "type": "level_up", "to": 10, "max": 1 })), "max: 1 is not 2 to 60");
        assert!(error(json!({ "type": "level_up", "levels": 1, "to": 5 })).contains("exclude each other"));
        assert!(error(json!({ "type": "level_up" })).contains("needs one of levels"));
        assert_eq!(error(json!({ "type": "level_up", "levels": 1, "rate": 5 })), "unknown key \"rate\" (not a key of the level_up effect)");
        assert_eq!(error(json!({ "type": "level_up", "levels": 1, "colour": 5 })), "unknown key \"colour\"");
        assert!(error(json!({ "type": "enchant" })).contains("at least one of rate, jumps, safe"));
        assert_eq!(error(json!({ "type": "enchant", "rate": 0 })), "rate: 0 is not 1 to 1000000");
        assert_eq!(error(json!({ "type": "enchant", "rate": 1000001 })), "rate: 1000001 is not 1 to 1000000");
        assert!(error(json!({ "type": "enchant", "safe": "yes" })).starts_with("safe: "));
        assert!(error(json!({ "type": "enchant", "jumps": [] })).starts_with("jumps: 0 entries is not 1 to 19"));
        assert!(error(json!({ "type": "enchant", "jumps": [[0, 1]] })).contains("jumps: entry 1: steps 0 is not 1 to 19"));
        assert!(error(json!({ "type": "enchant", "jumps": [[20, 1]] })).contains("steps 20 is not 1 to 19"));
        assert!(error(json!({ "type": "enchant", "jumps": [[1, 0]] })).contains("weight 0 is not 1 to 1000000"));
        assert!(error(json!({ "type": "enchant", "jumps": vec![json!([1, 1]); 20] })).starts_with("jumps: 20 entries"));
        assert!(error(json!({ "type": "enchant", "jumps": 3 })).starts_with("jumps: 3 is not a list"));
        assert!(error(json!({ "type": "enchant", "safe": true, "stats": ["str"] })).contains("not a key of the enchant effect"));
        assert!(error(json!({ "type": "recreate" })).contains("needs stats"));
        assert!(error(json!({ "type": "recreate", "stats": [] })).starts_with("stats: 0 entries is not 1 to 9"));
        assert!(error(json!({ "type": "recreate", "stats": vec!["str"; 10] })).starts_with("stats: 10 entries"));
        assert!(error(json!({ "type": "recreate", "stats": ["strength"] })).starts_with("stats: \"strength\" is not one of \"str\""));
        assert!(error(json!({ "type": "recreate", "stats": ["str", "str"] })).contains("listed twice"));
        assert!(error(json!({ "type": "recreate", "stats": "str" })).starts_with("stats: \"str\" is not a list"));
        assert_eq!(error(json!({ "type": "recreate", "stats": ["str"], "value": "high" })), "value: \"high\" is not \"random\" or \"max\"");
        assert!(error(json!({ "type": "recreate", "stats": ["str"], "rate": 1 })).contains("not a key of the recreate effect"));
        assert!(error(json!({ "type": "nothing" })).starts_with("type: unknown type \"nothing\"; the types are level_up, enchant, recreate"));
        assert!(error(json!({ "levels": 1 })).contains("needs a type"));
        assert!(error(json!({ "type": 3 })).starts_with("type: "));
        assert!(error(json!(3)).contains("is a table"));
    }

    #[test]
    fn an_effect_fits_the_right_row_only() {
        let level_up = effect(json!({ "type": "level_up", "levels": 1 }));
        for item_type in [25, 44, 100] {
            assert_eq!(fits_row(&level_up, item_type, 0), Ok(()), "type {item_type}");
        }
        let wrong = fits_row(&level_up, 95, 0).unwrap_err();
        assert!(wrong.starts_with("effect level_up needs an item the core uses (item type 25, 44 or 100)") && wrong.ends_with("type 95"), "{wrong}");
        assert!(fits_row(&level_up, 16, 0).is_err(), "armour");
        let enchant = effect(json!({ "type": "enchant", "safe": true }));
        assert_eq!(fits_row(&enchant, 95, 0), Ok(()));
        assert!(fits_row(&enchant, 100, 0).unwrap_err().contains("needs a Lapisia"));
        let rune = effect(json!({ "type": "recreate", "stats": ["str"] }));
        assert_eq!(fits_row(&rune, 100, 62), Ok(()));
        assert!(fits_row(&rune, 100, 61).unwrap_err().contains("special 62"));
        assert!(fits_row(&rune, 95, 0).is_err());
    }

    #[test]
    fn the_abilities_parse_and_hold_four() {
        let one = check_abilities(&json!([{ "type": "kill_boost", "times": 2 }])).unwrap();
        assert_eq!(one.len(), 1);
        assert_eq!(one.kill_boost(), Some(2));
        assert_eq!(one.iter().next(), Some(&Ability::KillBoost { times: 2 }));
        assert!(check_abilities(&json!(vec![json!({ "type": "kill_boost", "times": 100 }); 4])).is_ok(), "four");
        let e = |v: Value| check_abilities(&v).unwrap_err().to_string();
        assert!(e(json!(vec![json!({ "type": "kill_boost", "times": 2 }); 5])).starts_with("5 abilities is not 1 to 4"));
        assert!(e(json!([])).starts_with("0 abilities is not 1 to 4"));
        assert_eq!(e(json!([{ "type": "kill_boost", "times": 0 }])), "ability 1: times: 0 is not 1 to 100");
        assert_eq!(e(json!([{ "type": "kill_boost", "times": 101 }])), "ability 1: times: 101 is not 1 to 100");
        assert_eq!(e(json!([{ "type": "kill_boost", "times": 2 }, { "type": "kill_boost" }])), "ability 2: a kill_boost ability needs times, 1 to 100");
        assert_eq!(e(json!([{ "type": "kill_boost", "times": 2, "extra": 1 }])), "ability 1: unknown key \"extra\"");
        assert!(e(json!([{ "type": "haste", "times": 2 }])).starts_with("ability 1: type: unknown type \"haste\"; the types are kill_boost"));
        assert!(e(json!([{ "times": 2 }])).contains("needs a type"));
        assert!(e(json!({ "type": "kill_boost", "times": 2 })).contains("is a list of tables"));
        assert!(e(json!([3])).starts_with("ability 1: an ability is a table"));
    }

    #[test]
    fn the_shorthand_keeps_the_highest_and_fills_four_places() {
        let mut a = Abilities::default();
        assert!(a.is_empty() && a.kill_boost().is_none());
        assert!(a.merge_kill_boost(2));
        assert!(a.merge_kill_boost(5) && a.merge_kill_boost(3));
        assert_eq!((a.len(), a.kill_boost()), (1, Some(5)), "one ability, the highest");
        assert!(Abilities::default().kill_boost().is_none());
        // The places of a full list: a fifth cannot be added, but a kill_boost already there takes the higher value.
        let mut full = Abilities::default();
        full.0 = [Ability::KillBoost { times: 2 }; MAX_ABILITIES];
        assert!(!full.push(Ability::KillBoost { times: 1 }));
        assert!(full.merge_kill_boost(7) && full.kill_boost() == Some(7));
    }

    #[test]
    fn the_effects_table_counts_by_type() {
        let table = ItemEffects::new([
            (100215, effect(json!({ "type": "level_up", "levels": 1 }))),
            (100216, effect(json!({ "type": "level_up", "to": 15 }))),
            (95003, effect(json!({ "type": "enchant", "safe": true }))),
        ]);
        assert_eq!(table.len(), 3);
        assert_eq!(table.counts(), [("level_up", 2), ("enchant", 1), ("recreate", 0)]);
        assert!(table.get(100216).is_some() && table.get(100217).is_none());
        assert!(table.for_use(100216).is_some() && table.for_use(95003).is_none(), "an enchant effect is not a use");
        assert!(ItemEffects::default().is_empty());
    }

    #[test]
    fn the_effects_describe_themselves() {
        assert_eq!(effect(json!({ "type": "level_up", "levels": 1 })).describe(), "level_up levels = 1");
        assert_eq!(effect(json!({ "type": "level_up", "to": 30, "max": 50 })).describe(), "level_up to = 30, max = 50");
        assert_eq!(
            effect(json!({ "type": "enchant", "jumps": [[1, 60], [3, 10]], "safe": true })).describe(),
            "enchant rate = 1000000, jumps = [[1, 60], [3, 10]], safe = true"
        );
        assert_eq!(effect(json!({ "type": "recreate", "stats": ["str", "wis"], "value": "max" })).describe(), "recreate stats = [\"str\", \"wis\"], value = \"max\"");
        assert_eq!(Ability::KillBoost { times: 2 }.describe(), "kill_boost times = 2");
    }

    #[test]
    fn the_level_a_stone_gives() {
        let plus = |n| LevelUp { by: LevelBy::Levels(n), max: None };
        let to = |n| LevelUp { by: LevelBy::To(n), max: None };
        assert_eq!(level_target(10, 1, &plus(1)), Some(11));
        assert_eq!(level_target(59, 1, &plus(1)), Some(60));
        assert_eq!(level_target(60, 1, &plus(1)), None, "at the cap");
        assert_eq!(level_target(55, 3, &plus(30)), Some(60), "capped at 60");
        assert_eq!(level_target(29, 1, &to(30)), Some(30));
        assert_eq!(level_target(30, 1, &to(30)), None, "at or above to");
        assert_eq!(level_target(45, 1, &to(30)), None);
        assert_eq!(level_target(1, 1, &to(60)), Some(60));
        let capped = LevelUp { by: LevelBy::Levels(10), max: Some(50) };
        assert_eq!(level_target(45, 1, &capped), Some(50), "max cuts the jump");
        assert_eq!(level_target(50, 1, &capped), None, "at max");
        assert_eq!(level_target(52, 1, &capped), None, "above max");
        // Easy mode (grow 0) stops at 30.
        assert_eq!(level_target(29, 0, &plus(5)), Some(30));
        assert_eq!(level_target(30, 0, &plus(1)), None);
        assert_eq!(level_target(10, 0, &to(40)), Some(30));
        assert_eq!(level_target(u16::MAX, 1, &plus(1)), None, "no overflow");
    }

    #[test]
    fn a_sure_rate_needs_no_draw() {
        let sure = EnchantEffect { rate: SURE, jumps: None, safe: false };
        assert_eq!(sure.rate(), Rate::Always);
        assert!(sure.rate().succeeds(100, 999_999), "the largest material and draw");
        let half = EnchantEffect { rate: 500_000, ..sure.clone() };
        assert_eq!(half.rate(), Rate::Below(500_000));
        assert!(half.rate().succeeds(0, 499_999) && !half.rate().succeeds(0, 500_000), "the edge");
        assert!(half.rate().succeeds(10, 499_989) && !half.rate().succeeds(10, 499_990), "the material counts");
        assert_eq!(EnchantEffect { rate: 999_999, ..sure }.rate(), Rate::Below(999_999));
    }

    #[test]
    fn the_jump_follows_the_weights_and_the_table_is_the_lapisias() {
        let ItemEffect::Enchant(e) = effect(json!({ "type": "enchant", "jumps": [[1, 60], [2, 30], [3, 10]] })) else { panic!() };
        // 60 %, 30 %, 10 % of the draws below 1,000,000.
        let at = |draw| e.jump(draw).unwrap();
        assert_eq!((at(0), at(599_999)), (1, 1));
        assert_eq!((at(600_000), at(899_999)), (2, 2));
        assert_eq!((at(900_000), at(999_999)), (3, 3));
        assert_eq!(at(5_000_000), 3, "a draw above the range is the last draw");
        let one = EnchantEffect { rate: SURE, jumps: Some(Jumps { list: vec![(19, 1)], total: 1 }), safe: false };
        assert_eq!(one.jump(0), Some(19));
        assert_eq!(EnchantEffect { jumps: None, ..one }.jump(0), None, "no table: the rules are asked");
    }
}
