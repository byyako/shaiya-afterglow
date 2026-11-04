//! Rules (`docs/plans/ext-2-rules.md`): named, scoped allow/deny/change statements the core asks at a few fixed places (the points,
//! section 5). This module is pure: the scope (`scope.rs`), the rule types and their parser (`parse.rs`), the layers and the compiled
//! [`RuleSet`] (`resolve.rs`, here). A [`RuleSet`] holds one `Vec` of compiled rules per point (section 8), so asking a point
//! with no rules costs one branch; the hits are counted with a relaxed atomic add.

mod parse;
mod resolve;
mod scope;

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use anyhow::Result;
use serde_json::Value;

pub use parse::TYPES;
pub use resolve::{EXAMPLE, FileRule, ResolvedRule, RuleFile, RuleLayer, RuleSources, RuleState, check_file, from_settings, resolve, show};
pub use scope::{Who, WhoMode, hour_of_week, refresh_hour_of_week, set_hour_of_week};

use parse::{Draft, Items, Kind};
use scope::Scope;

/// A rule's error in its table: the message and the key it is about. `line_key` is the key whose line the error is reported at
/// (the rule's own line without one); `path_key` is the key the message names (`rules.level-gap.gap: …`).
#[derive(Clone, Debug)]
pub struct RuleError {
    pub line_key: Option<String>,
    pub path_key: Option<String>,
    pub msg: String,
}

impl RuleError {
    fn rule(msg: impl Into<String>) -> Self {
        Self { line_key: None, path_key: None, msg: msg.into() }
    }

    fn unknown(key: &str) -> Self {
        Self { line_key: Some(key.to_owned()), path_key: None, msg: format!("unknown key {key:?}") }
    }

    /// `rules.<name>.<key>: …` or `rules.<name>: …`.
    pub fn describe(&self, name: &str) -> String {
        match &self.path_key {
            Some(key) => format!("rules.{name}.{key}: {}", self.msg),
            None => format!("rules.{name}: {}", self.msg),
        }
    }
}

fn key_err(key: &str, msg: impl Into<String>) -> RuleError {
    RuleError { line_key: Some(key.to_owned()), path_key: Some(key.to_owned()), msg: msg.into() }
}

/// A rule name of the file or the editor: `^[a-z0-9][a-z0-9-]{0,39}$`; `<extension>/<name>` names an extension's rule; names starting
/// with `setting.` belong to the settings (section 4.2).
pub fn check_name(name: &str) -> Result<(), String> {
    fn plain(name: &str) -> bool {
        let mut chars = name.chars();
        name.len() <= 40 && chars.next().is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit()) && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    }
    if let Some(setting) = name.strip_prefix("setting.") {
        return Err(format!("names starting with `setting.` belong to the settings (change the setting `{setting}` instead)"));
    }
    let ok = match name.split_once('/') {
        Some((extension, rule)) => {
            extension.len() >= 2
                && extension.len() <= 40
                && extension.starts_with(|c: char| c.is_ascii_lowercase())
                && extension.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                && plain(rule)
        }
        None => plain(name),
    };
    if ok { Ok(()) } else { Err(format!("{name:?} is not a rule name (^[a-z0-9][a-z0-9-]{{0,39}}$, or <extension>/<name>)")) }
}

/// Whether `value` is a rule the server accepts under `name` (used by `data-editor` for an extension's rules and the editor's `rule`
/// target, as `config::check_setting`). A table of `enabled = false` alone is valid: it turns off the rule of that name in a lower
/// layer, which only `resolve` can check.
pub fn check_rule(name: &str, value: &Value) -> Result<()> {
    check_name(name).map_err(|e| anyhow::anyhow!("rules.{name}: {e}"))?;
    parse::parse(value).map(drop).map_err(|e| anyhow::anyhow!("{}", e.describe(name)))
}

/// As [`check_rule`], for a rule that must stand alone (an extension's own): `{ enabled = false }` alone is refused.
pub fn check_whole_rule(name: &str, value: &Value) -> Result<()> {
    check_name(name).map_err(|e| anyhow::anyhow!("rules.{name}: {e}"))?;
    match parse::parse(value) {
        Ok(parse::Parsed::Rule(_)) => Ok(()),
        Ok(parse::Parsed::TurnOff) => Err(anyhow::anyhow!("rules.{name}: a rule needs a type")),
        Err(e) => Err(anyhow::anyhow!("{}", e.describe(name))),
    }
}

/// The places the core asks the rules (section 5.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Point {
    Attack,
    Trade,
    PersonalShop,
    TradeChat,
    Exp,
    PvpKills,
    PvpDeaths,
    Enchant,
}

impl Point {
    pub fn name(self) -> &'static str {
        match self {
            Point::Attack => "attack",
            Point::Trade => "trade",
            Point::PersonalShop => "personal_shop",
            Point::TradeChat => "trade_chat",
            Point::Exp => "exp",
            Point::PvpKills => "pvp_kills",
            Point::PvpDeaths => "pvp_deaths",
            Point::Enchant => "enchant",
        }
    }
}

/// Where experience comes from (point 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExpSource {
    Mob,
    Party,
    Quest,
    Pvp,
}

impl ExpSource {
    fn bit(self) -> u8 {
        match self {
            ExpSource::Mob => 1,
            ExpSource::Party => 2,
            ExpSource::Quest => 4,
            ExpSource::Pvp => 8,
        }
    }
}

/// What a refusal tells the connection: which rule, and the text to whisper (`None`: the rule is silent).
#[derive(Clone, Debug)]
pub struct Refusal {
    pub rule: u32,
    pub name: Arc<str>,
    pub text: Option<Arc<str>>,
}

/// A number a rule changed, with the names of the rules that did it (filled only while debug logging is on, section 5.3).
#[derive(Clone, Debug, PartialEq)]
pub struct Changed<T> {
    pub value: T,
    pub rules: Vec<Arc<str>>,
}

/// A rule in force as the console lists it.
#[derive(Clone, Debug)]
pub struct RuleLine {
    pub name: Arc<str>,
    pub ty: &'static str,
    pub layer: String,
    /// The layer without spaces (`RuleLayer::short`).
    pub short: String,
    pub hits: u64,
}

/// What every compiled entry of a rule shares: its id (new at every build), name, layer, refusal text and hit count.
#[derive(Debug)]
struct Meta {
    id: u32,
    name: Arc<str>,
    ty: &'static str,
    layer: String,
    short: String,
    text: Option<Arc<str>>,
    hits: AtomicU64,
}

impl Meta {
    fn hit(&self) {
        self.hits.fetch_add(1, Ordering::Relaxed);
    }

    fn refusal(&self) -> Refusal {
        Refusal { rule: self.id, name: self.name.clone(), text: self.text.clone() }
    }
}

struct Entry<T> {
    meta: Arc<Meta>,
    scope: Scope,
    v: T,
}

enum Attack {
    Deny,
    Gap(u16),
}

struct ExpRule {
    factor: f64,
    sources: u8,
}

struct Gain {
    exp: f64,
    kills: u32,
    deaths: bool,
}

struct EnchantRule {
    /// `(steps, weight)`, with the total weight.
    jumps: Vec<(u8, u32)>,
    total: u64,
    items: Items,
}

static NEXT_ID: AtomicU32 = AtomicU32::new(1);

/// The rules in force, compiled: one `Vec` per point, in the order of section 4.3. Built at start and at `reload`; read through
/// an `Arc` snapshot (section 8).
#[derive(Default)]
pub struct RuleSet {
    all: Vec<Arc<Meta>>,
    attack: Vec<Entry<Attack>>,
    trade_deny: Vec<Entry<()>>,
    trade_grant: Vec<Entry<()>>,
    shop: Vec<Entry<()>>,
    trade_chat: Vec<Entry<()>>,
    exp: Vec<Entry<ExpRule>>,
    gain: Vec<Entry<Gain>>,
    enchant: Vec<Entry<EnchantRule>>,
}

impl std::fmt::Debug for RuleSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.all.iter().map(|m| &*m.name)).finish()
    }
}

/// Collects the names of the rules that changed a number, only while debug logging is on.
fn note(rules: &mut Vec<Arc<str>>, meta: &Meta) {
    if tracing::enabled!(tracing::Level::DEBUG) {
        rules.push(meta.name.clone());
    }
}

impl RuleSet {
    /// A set with no rules: every point answers allow and 1.
    pub fn none() -> Self {
        Self::default()
    }

    /// Compiles the rules, given in the order of section 4.3 (the first is the strongest).
    fn build(rules: Vec<(String, RuleLayer, Draft)>) -> Self {
        let mut set = Self::default();
        for (name, layer, draft) in rules {
            let meta = Arc::new(Meta {
                id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
                name: Arc::from(name),
                ty: draft.kind.type_name(),
                layer: layer.to_string(),
                short: layer.short(),
                text: draft.text.clone(),
                hits: AtomicU64::new(0),
            });
            let (scope, m) = (draft.scope, meta.clone());
            match draft.kind {
                Kind::Deny(Point::Attack) => set.attack.push(Entry { meta: m, scope, v: Attack::Deny }),
                Kind::LevelGap(gap) => set.attack.push(Entry { meta: m, scope, v: Attack::Gap(gap) }),
                Kind::Deny(Point::Trade) => set.trade_deny.push(Entry { meta: m, scope, v: () }),
                Kind::CrossFactionTrade => set.trade_grant.push(Entry { meta: m, scope, v: () }),
                Kind::Deny(Point::PersonalShop) => set.shop.push(Entry { meta: m, scope, v: () }),
                Kind::Deny(Point::TradeChat) => set.trade_chat.push(Entry { meta: m, scope, v: () }),
                Kind::Deny(other) => unreachable!("{other:?} is not a point a deny rule can name"),
                Kind::PvpGain { exp, kills, deaths } => set.gain.push(Entry { meta: m, scope, v: Gain { exp, kills, deaths } }),
                Kind::Exp { factor, sources } => set.exp.push(Entry { meta: m, scope, v: ExpRule { factor, sources } }),
                Kind::Enchant { jumps, items } => {
                    let total = jumps.iter().map(|&(_, weight)| u64::from(weight)).sum();
                    set.enchant.push(Entry { meta: m, scope, v: EnchantRule { jumps, total, items } });
                }
            }
            set.all.push(meta);
        }
        set
    }

    /// The rules of `rules` as a config file would hold them, in the order given: no rule of the settings, no layers. For tests and
    /// tools that need a set without a [`crate::config::Config`]; errors name the rule as `check_rule` does.
    pub fn from_values(rules: &[(&str, Value)]) -> Result<Self> {
        let mut compiled = Vec::new();
        for (name, value) in rules {
            check_name(name).map_err(|e| anyhow::anyhow!("rules.{name}: {e}"))?;
            match parse::parse(value) {
                Ok(parse::Parsed::Rule(draft)) if draft.enabled => compiled.push(((*name).to_owned(), RuleLayer::File, draft)),
                Ok(parse::Parsed::Rule(_)) => {}
                Ok(parse::Parsed::TurnOff) => anyhow::bail!("rules.{name}: a rule needs a type"),
                Err(e) => anyhow::bail!("{}", e.describe(name)),
            }
        }
        Ok(Self::build(compiled))
    }

    /// The number of rules in force.
    pub fn len(&self) -> usize {
        self.all.len()
    }

    pub fn is_empty(&self) -> bool {
        self.all.is_empty()
    }

    /// The rules in force in order, with their hits since this set was built.
    pub fn list(&self) -> Vec<RuleLine> {
        self.all.iter().map(|m| RuleLine { name: m.name.clone(), ty: m.ty, layer: m.layer.clone(), short: m.short.clone(), hits: m.hits.load(Ordering::Relaxed) }).collect()
    }

    /// Point 1: whether `attacker` may not attack `target` on `map` (both players, enemies by faction). The first refusing rule.
    pub fn attack(&self, map: u16, attacker: &Who, target: &Who) -> Option<Refusal> {
        if self.attack.is_empty() {
            return None;
        }
        for e in &self.attack {
            let refuses = match e.v {
                Attack::Deny => true,
                Attack::Gap(gap) => attacker.level.abs_diff(target.level) > gap,
            };
            if refuses && e.scope.two(map, attacker, target) {
                e.meta.hit();
                return Some(e.meta.refusal());
            }
        }
        None
    }

    /// Point 2: the first `deny trade` rule in scope for the requester `a` and the one asked `b`.
    pub fn trade(&self, map: u16, a: &Who, b: &Who) -> Option<Refusal> {
        let e = self.trade_deny.iter().find(|e| e.scope.two(map, a, b))?;
        e.meta.hit();
        Some(e.meta.refusal())
    }

    /// Point 2's permission: whether a `cross_faction_trade` rule lets the two trade across the factions.
    pub fn cross_faction_trade(&self, map: u16, a: &Who, b: &Who) -> bool {
        let Some(e) = self.trade_grant.iter().find(|e| e.scope.two(map, a, b)) else { return false };
        e.meta.hit();
        true
    }

    /// Points 3 and 4 (the one-player refusals, `Point::PersonalShop` and `Point::TradeChat`): the first `deny` rule in scope.
    pub fn denied(&self, point: Point, map: u16, who: &Who) -> Option<Refusal> {
        let list = match point {
            Point::PersonalShop => &self.shop,
            Point::TradeChat => &self.trade_chat,
            other => unreachable!("{other:?} is not a one-player refusal"),
        };
        let e = list.iter().find(|e| e.scope.one(map, who))?;
        e.meta.hit();
        Some(e.meta.refusal())
    }

    /// Point 5: the factor to multiply the experience of `receiver` from `source` by (1 without a rule).
    pub fn exp_factor(&self, map: u16, receiver: &Who, source: ExpSource) -> Changed<f64> {
        let mut out = Changed { value: 1.0, rules: Vec::new() };
        if source == ExpSource::Pvp {
            for e in &self.gain {
                if e.v.exp != 1.0 && e.scope.one(map, receiver) {
                    e.meta.hit();
                    out.value *= e.v.exp;
                    note(&mut out.rules, &e.meta);
                }
            }
        } else {
            for e in &self.exp {
                if e.v.sources & source.bit() != 0 && e.scope.one(map, receiver) {
                    e.meta.hit();
                    out.value *= e.v.factor;
                    note(&mut out.rules, &e.meta);
                }
            }
        }
        out
    }

    /// Point 6: how many times the killer's counted player kill counts (1 without a rule, 0: not at all).
    pub fn pvp_kills(&self, map: u16, killer: &Who) -> Changed<u32> {
        let mut out = Changed { value: 1u32, rules: Vec::new() };
        for e in &self.gain {
            if e.v.kills != 1 && e.scope.one(map, killer) {
                e.meta.hit();
                out.value = out.value.saturating_mul(e.v.kills);
                note(&mut out.rules, &e.meta);
            }
        }
        out
    }

    /// Point 7: whether the victim's death is counted (true without a rule).
    pub fn pvp_deaths(&self, map: u16, victim: &Who) -> bool {
        if self.gain.is_empty() {
            return true;
        }
        let mut counted = true;
        for e in &self.gain {
            if !e.v.deaths && e.scope.one(map, victim) {
                e.meta.hit();
                counted = false;
            }
        }
        counted
    }

    /// Point 8: how many steps a successful enchant of `who`'s item rises, 1 without a rule. `draw` is below 1,000,000; the first
    /// matching `enchant` rule picks by its weights.
    pub fn enchant_jump(&self, map: u16, who: &Who, armour: bool, draw: u32) -> Changed<u8> {
        let mut out = Changed { value: 1u8, rules: Vec::new() };
        let Some(e) = self.enchant.iter().find(|e| e.v.items.covers(armour) && e.scope.one(map, who)) else { return out };
        let target = u64::from(draw.min(999_999)) * e.v.total / 1_000_000;
        let mut upto = 0u64;
        for &(steps, weight) in &e.v.jumps {
            upto += u64::from(weight);
            if target < upto {
                out.value = steps;
                break;
            }
        }
        e.meta.hit();
        note(&mut out.rules, &e.meta);
        out
    }
}

#[cfg(test)]
mod tests;
