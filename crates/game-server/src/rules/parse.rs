//! The rule types (`docs/plans/ext-2-rules.md` section 4.1) and their parser: a rule is a JSON object (the file's TOML table, the
//! database's `jsonb`), checked key by key. Every error names the key, so the caller can name the line.

use std::sync::Arc;

use serde_json::{Map, Value};

use super::scope::{self, Scope, WhoMode};
use super::{ExpSource, Point, RuleError, key_err};

/// The rule types, in the order the documentation lists them.
pub const TYPES: &[&str] = &["deny", "level_gap", "cross_faction_trade", "pvp_gain", "exp", "enchant"];

/// Which items an `enchant` rule is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Items {
    Weapons,
    Armour,
    Both,
}

impl Items {
    pub fn covers(self, armour: bool) -> bool {
        match self {
            Items::Weapons => !armour,
            Items::Armour => armour,
            Items::Both => true,
        }
    }
}

/// What a rule does: the type and its own keys, parsed.
#[derive(Clone, Debug)]
pub(super) enum Kind {
    Deny(Point),
    /// `deny` at `item_use` with its `items`: `(type, first type id, last type id)`, sorted; `None` for every item.
    DenyItemUse(Option<Vec<(u8, u8, u8)>>),
    LevelGap(u16),
    CrossFactionTrade,
    PvpGain { exp: f64, kills: u32, deaths: bool },
    Exp { factor: f64, sources: u8 },
    Enchant { jumps: Vec<(u8, u32)>, items: Items },
}

impl Kind {
    pub fn type_name(&self) -> &'static str {
        match self {
            Kind::Deny(_) | Kind::DenyItemUse(_) => "deny",
            Kind::LevelGap(_) => "level_gap",
            Kind::CrossFactionTrade => "cross_faction_trade",
            Kind::PvpGain { .. } => "pvp_gain",
            Kind::Exp { .. } => "exp",
            Kind::Enchant { .. } => "enchant",
        }
    }
}

/// A rule parsed: scope, kind, the refusal text (`None`: silent, or no refusal) and whether it is on.
#[derive(Clone, Debug)]
pub(super) struct Draft {
    pub enabled: bool,
    pub scope: Scope,
    pub kind: Kind,
    pub text: Option<Arc<str>>,
}

/// A rule as written: a whole rule, or the table `{ enabled = false }` that turns off the rule of that name in a lower layer.
#[derive(Clone, Debug)]
pub(super) enum Parsed {
    TurnOff,
    Rule(Draft),
}

/// The keys that belong to one type or another, to say "not a key of this type" instead of only "unknown".
const OWN_KEYS: &[&str] = &["point", "gap", "exp", "kills", "deaths", "factor", "sources", "jumps", "items", "message"];

const DENY_POINTS: &str = "\"attack\", \"trade\", \"personal_shop\", \"trade_chat\" or \"item_use\"";

fn default_text(kind: &Kind) -> Option<&'static str> {
    Some(match kind {
        Kind::Deny(Point::Attack) => "You cannot attack that player here.",
        Kind::LevelGap(_) => "The level difference is too great to fight.",
        Kind::Deny(Point::Trade) => "Trading is not allowed here.",
        Kind::Deny(Point::PersonalShop) => "A personal shop cannot be opened here.",
        Kind::Deny(Point::TradeChat) => "Trade chat is not allowed here.",
        Kind::DenyItemUse(_) => "You cannot use that here.",
        _ => return None,
    })
}

fn take_whole(obj: &mut Map<String, Value>, key: &str, low: u64, high: u64) -> Result<Option<u64>, RuleError> {
    let Some(value) = obj.remove(key) else { return Ok(None) };
    match (value.as_u64(), value.as_i64()) {
        (Some(n), _) if (low..=high).contains(&n) => Ok(Some(n)),
        (Some(n), _) => Err(key_err(key, format!("{n} is not {low} to {high}"))),
        (None, Some(n)) => Err(key_err(key, format!("{n} is not {low} to {high}"))),
        (None, None) => Err(key_err(key, format!("{value} is not a whole number {low} to {high}"))),
    }
}

fn take_number(obj: &mut Map<String, Value>, key: &str, low: f64, high: f64) -> Result<Option<f64>, RuleError> {
    let Some(value) = obj.remove(key) else { return Ok(None) };
    match value.as_f64() {
        Some(n) if n.is_finite() && n >= low && n <= high => Ok(Some(n)),
        Some(n) => Err(key_err(key, format!("{n} is not {low} to {high}"))),
        None => Err(key_err(key, format!("{value} is not a number {low} to {high}"))),
    }
}

fn take_flag(obj: &mut Map<String, Value>, key: &str) -> Result<Option<bool>, RuleError> {
    match obj.remove(key) {
        None => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(b)),
        Some(other) => Err(key_err(key, format!("{other} is not true or false"))),
    }
}

/// The refusal message: printable ASCII, at most 100 bytes; `""` is silent. Outer `None`: not given.
fn take_message(obj: &mut Map<String, Value>) -> Result<Option<Option<Arc<str>>>, RuleError> {
    let Some(value) = obj.remove("message") else { return Ok(None) };
    let Value::String(text) = value else { return Err(key_err("message", format!("{value} is not a text"))) };
    if text.len() > 100 {
        return Err(key_err("message", format!("{} bytes is more than 100", text.len())));
    }
    if let Some(bad) = text.chars().find(|c| !(' '..='~').contains(c)) {
        return Err(key_err("message", format!("{bad:?} is not printable ASCII")));
    }
    Ok(Some((!text.is_empty()).then(|| Arc::from(text))))
}

fn point_of(name: &str) -> Option<Point> {
    Some(match name {
        "attack" => Point::Attack,
        "trade" => Point::Trade,
        "personal_shop" => Point::PersonalShop,
        "trade_chat" => Point::TradeChat,
        "item_use" => Point::ItemUse,
        _ => return None,
    })
}

fn take_sources(obj: &mut Map<String, Value>) -> Result<u8, RuleError> {
    let Some(value) = obj.remove("sources") else { return Ok(ExpSource::Mob.bit() | ExpSource::Party.bit()) };
    let Value::Array(items) = value else { return Err(key_err("sources", format!("{value} is not a list of \"mob\", \"party\", \"quest\""))) };
    if items.is_empty() {
        return Err(key_err("sources", "an empty list matches nothing"));
    }
    let mut mask = 0;
    for item in &items {
        mask |= match item.as_str() {
            Some("mob") => ExpSource::Mob.bit(),
            Some("party") => ExpSource::Party.bit(),
            Some("quest") => ExpSource::Quest.bit(),
            _ => return Err(key_err("sources", format!("{item} is not \"mob\", \"party\" or \"quest\""))),
        };
    }
    Ok(mask)
}

/// The `jumps` of an `enchant` rule or an `enchant` item effect: `[[steps, weight], …]`, 1 to 19 entries, steps 1 to 19, weights 1 to
/// 1,000,000. The error is the text after the key's name.
pub fn parse_jumps(value: &Value) -> Result<Vec<(u8, u32)>, String> {
    let Value::Array(items) = value else { return Err(format!("{value} is not a list of [steps, weight]")) };
    if items.is_empty() || items.len() > 19 {
        return Err(format!("{} entries is not 1 to 19", items.len()));
    }
    items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let pair = item.as_array().filter(|p| p.len() == 2).ok_or_else(|| format!("entry {}: {item} is not [steps, weight]", i + 1))?;
            let part = |v: &Value, what: &str, low: u64, high: u64| -> Result<u64, String> {
                v.as_u64().filter(|n| (low..=high).contains(n)).ok_or_else(|| format!("entry {}: {what} {v} is not {low} to {high}", i + 1))
            };
            Ok((part(&pair[0], "steps", 1, 19)? as u8, part(&pair[1], "weight", 1, 1_000_000)? as u32))
        })
        .collect()
}

fn take_jumps(obj: &mut Map<String, Value>) -> Result<Vec<(u8, u32)>, RuleError> {
    let Some(value) = obj.remove("jumps") else { return Err(RuleError::rule("an enchant rule needs jumps, e.g. [[1, 70], [2, 30]]")) };
    parse_jumps(&value).map_err(|msg| key_err("jumps", msg))
}

/// The `items` of a `deny item_use` rule (`docs/plans/ext-3-effects.md` section 6): a list of `"type/type_id"`, `"type/low-high"` (a
/// range of type ids) and `"type"` (every item of the type), types and type ids 1 to 255. `None` when the key is not there (every item).
fn take_item_list(obj: &mut Map<String, Value>) -> Result<Option<Vec<(u8, u8, u8)>>, RuleError> {
    let Some(value) = obj.remove("items") else { return Ok(None) };
    let wanted = "\"type\", \"type/type_id\" or \"type/low-high\"";
    let Value::Array(list) = value else { return Err(key_err("items", format!("{value} is not a list of {wanted}"))) };
    if list.is_empty() {
        return Err(key_err("items", "an empty list matches nothing"));
    }
    let mut out = Vec::new();
    for entry in &list {
        let text = entry.as_str().ok_or_else(|| key_err("items", format!("{entry} is not {wanted}")))?;
        let number = |part: &str| -> Result<u8, RuleError> {
            part.trim().parse::<u8>().ok().filter(|n| *n >= 1).ok_or_else(|| key_err("items", format!("{text:?}: {part:?} is not 1 to 255")))
        };
        let (kind, ids) = match text.split_once('/') {
            None => (number(text)?, (1, 255)),
            Some((kind, ids)) => match ids.split_once('-') {
                None => (number(kind)?, (number(ids)?, number(ids)?)),
                Some((low, high)) => (number(kind)?, (number(low)?, number(high)?)),
            },
        };
        if ids.0 > ids.1 {
            return Err(key_err("items", format!("{text:?}: {} is above {}", ids.0, ids.1)));
        }
        out.push((kind, ids.0, ids.1));
    }
    out.sort_unstable();
    out.dedup();
    Ok(Some(out))
}

/// Parses a rule (section 4.1). `Parsed::TurnOff` for `{ enabled = false }` alone.
pub(super) fn parse(value: &Value) -> Result<Parsed, RuleError> {
    let Value::Object(map) = value else { return Err(RuleError::rule("a rule is a table")) };
    let mut obj = map.clone();
    let enabled = take_flag(&mut obj, "enabled")?.unwrap_or(true);
    let ty = match obj.remove("type") {
        None if !enabled && obj.is_empty() => return Ok(Parsed::TurnOff),
        None => return Err(RuleError::rule("a rule needs a type")),
        Some(Value::String(ty)) => ty,
        Some(other) => return Err(key_err("type", format!("{other} is not a text"))),
    };
    let (scope, kind, message) = match ty.as_str() {
        "deny" => {
            let point = match obj.remove("point") {
                None => return Err(RuleError::rule(format!("a deny rule needs a point ({DENY_POINTS})"))),
                Some(Value::String(p)) => point_of(&p).ok_or_else(|| key_err("point", format!("{p:?} is not {DENY_POINTS}")))?,
                Some(other) => return Err(key_err("point", format!("{other} is not {DENY_POINTS}"))),
            };
            let who = matches!(point, Point::Attack | Point::Trade).then_some(WhoMode::Either);
            let kind = if point == Point::ItemUse { Kind::DenyItemUse(take_item_list(&mut obj)?) } else { Kind::Deny(point) };
            (scope::parse(&mut obj, who, true)?, kind, take_message(&mut obj)?)
        }
        "level_gap" => {
            let Some(gap) = take_whole(&mut obj, "gap", 1, 255)? else { return Err(RuleError::rule("a level_gap rule needs a gap, 1 to 255")) };
            (scope::parse(&mut obj, Some(WhoMode::Either), true)?, Kind::LevelGap(gap as u16), take_message(&mut obj)?)
        }
        "cross_faction_trade" => (scope::parse(&mut obj, Some(WhoMode::Both), true)?, Kind::CrossFactionTrade, None),
        "pvp_gain" => {
            let exp = take_number(&mut obj, "exp", 0.0, 100.0)?;
            let kills = take_whole(&mut obj, "kills", 0, 100)?;
            let deaths = take_flag(&mut obj, "deaths")?;
            if exp.is_none() && kills.is_none() && deaths.is_none() {
                return Err(RuleError::rule("a pvp_gain rule needs at least one of exp, kills, deaths"));
            }
            let kind = Kind::PvpGain { exp: exp.unwrap_or(1.0), kills: kills.map_or(1, |k| k as u32), deaths: deaths.unwrap_or(true) };
            (scope::parse(&mut obj, None, false)?, kind, None)
        }
        "exp" => {
            let Some(factor) = take_number(&mut obj, "factor", 0.0, 100.0)? else { return Err(RuleError::rule("an exp rule needs a factor, 0 to 100")) };
            let sources = take_sources(&mut obj)?;
            (scope::parse(&mut obj, None, true)?, Kind::Exp { factor, sources }, None)
        }
        "enchant" => {
            let jumps = take_jumps(&mut obj)?;
            let items = match obj.remove("items") {
                None => Items::Both,
                Some(Value::String(s)) if s == "weapons" => Items::Weapons,
                Some(Value::String(s)) if s == "armour" => Items::Armour,
                Some(Value::String(s)) if s == "both" => Items::Both,
                Some(other) => return Err(key_err("items", format!("{other} is not \"weapons\", \"armour\" or \"both\""))),
            };
            (scope::parse(&mut obj, None, true)?, Kind::Enchant { jumps, items }, None)
        }
        other => return Err(key_err("type", format!("unknown type {other:?}; the types are {}", TYPES.join(", ")))),
    };
    if let Some(key) = obj.keys().next() {
        let note = if key == "items" && ty == "deny" {
            " (only a deny rule with point = \"item_use\" takes items)".to_owned()
        } else if OWN_KEYS.contains(&key.as_str()) {
            format!(" (not a key of a {ty} rule)")
        } else {
            String::new()
        };
        return Err(RuleError { msg: format!("unknown key {key:?}{note}"), ..RuleError::unknown(key) });
    }
    let kind_text = default_text(&kind);
    let text = match message {
        Some(given) => given,
        None => kind_text.map(Arc::from),
    };
    Ok(Parsed::Rule(Draft { enabled, scope, kind, text }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn draft(v: Value) -> Draft {
        match parse(&v).unwrap_or_else(|e| panic!("{}", e.msg)) {
            Parsed::Rule(d) => d,
            Parsed::TurnOff => panic!("a whole rule"),
        }
    }

    fn error(v: Value) -> String {
        let e = parse(&v).unwrap_err();
        match &e.path_key {
            Some(k) => format!("{k}: {}", e.msg),
            None => e.msg,
        }
    }

    #[test]
    fn every_type_parses() {
        assert!(matches!(draft(json!({ "type": "deny", "point": "attack" })).kind, Kind::Deny(Point::Attack)));
        for point in ["attack", "trade", "personal_shop", "trade_chat"] {
            let d = draft(json!({ "type": "deny", "point": point, "maps": [1] }));
            assert!(d.enabled && d.text.is_some(), "{point}");
        }
        assert!(matches!(draft(json!({ "type": "level_gap", "gap": 20 })).kind, Kind::LevelGap(20)));
        assert!(matches!(draft(json!({ "type": "cross_faction_trade", "maps": [42] })).kind, Kind::CrossFactionTrade));
        let Kind::PvpGain { exp, kills, deaths } = draft(json!({ "type": "pvp_gain", "exp": 0, "kills": 0, "deaths": false })).kind else { panic!() };
        assert_eq!((exp, kills, deaths), (0.0, 0, false));
        let Kind::PvpGain { exp, kills, deaths } = draft(json!({ "type": "pvp_gain", "exp": 0.5 })).kind else { panic!() };
        assert_eq!((exp, kills, deaths), (0.5, 1, true), "the others stay at their defaults");
        let Kind::Exp { factor, sources } = draft(json!({ "type": "exp", "factor": 2 })).kind else { panic!() };
        assert_eq!((factor, sources), (2.0, ExpSource::Mob.bit() | ExpSource::Party.bit()));
        let Kind::Exp { sources, .. } = draft(json!({ "type": "exp", "factor": 2, "sources": ["quest"] })).kind else { panic!() };
        assert_eq!(sources, ExpSource::Quest.bit());
        let Kind::Enchant { jumps, items } = draft(json!({ "type": "enchant", "jumps": [[1, 70], [2, 20], [3, 10]] })).kind else { panic!() };
        assert_eq!((jumps, items), (vec![(1, 70), (2, 20), (3, 10)], Items::Both));
        let Kind::Enchant { items, .. } = draft(json!({ "type": "enchant", "jumps": [[2, 1]], "items": "armour" })).kind else { panic!() };
        assert_eq!(items, Items::Armour);
    }

    #[test]
    fn deny_item_use_takes_items_and_nothing_else_does() {
        let Kind::DenyItemUse(items) = draft(json!({ "type": "deny", "point": "item_use" })).kind else { panic!() };
        assert_eq!(items, None, "every item");
        let d = draft(json!({ "type": "deny", "point": "item_use", "items": ["100/217", "100/215-216", "25", "100/215-216", " 7 / 3 "], "maps": [2] }));
        let Kind::DenyItemUse(items) = d.kind else { panic!() };
        assert_eq!(items, Some(vec![(7, 3, 3), (25, 1, 255), (100, 215, 216), (100, 217, 217)]), "sorted, a repeat once, a type is all its ids");
        assert_eq!(d.text.as_deref(), Some("You cannot use that here."));
        assert!(error(json!({ "type": "deny", "point": "item_use", "items": [] })).starts_with("items: an empty list"));
        assert!(error(json!({ "type": "deny", "point": "item_use", "items": "100/215" })).starts_with("items: \"100/215\" is not a list"));
        assert!(error(json!({ "type": "deny", "point": "item_use", "items": [215] })).starts_with("items: 215 is not \"type\""));
        for bad in ["0", "256", "x", "100/0", "100/256", "100/216-215", "100/a-b", "100/1-2-3", "/5", "100/"] {
            assert!(error(json!({ "type": "deny", "point": "item_use", "items": [bad] })).starts_with("items: "), "{bad:?}");
        }
        assert!(error(json!({ "type": "deny", "point": "item_use", "items": ["100/216-215"] })).contains("216 is above 215"));
        // `items` belongs to deny item_use and the enchant rule (with its own meaning), and to no other deny.
        let e = parse(&json!({ "type": "deny", "point": "attack", "items": ["100"] })).unwrap_err();
        assert!(e.msg.starts_with("unknown key \"items\"") && e.msg.contains("only a deny rule with point = \"item_use\""), "{}", e.msg);
        assert!(error(json!({ "type": "level_gap", "gap": 3, "items": ["100"] })).contains("not a key of a level_gap rule"));
        assert!(error(json!({ "type": "enchant", "jumps": [[2, 1]], "items": ["100"] })).starts_with("items: "), "the enchant rule's own key");
        // A scope and a silent message work as at the other points.
        assert!(draft(json!({ "type": "deny", "point": "item_use", "message": "" })).text.is_none());
        assert!(error(json!({ "type": "deny", "point": "item_use", "who": "both" })).contains("unknown key \"who\""));
        assert!(error(json!({ "type": "deny", "point": "chat" })).contains("\"item_use\""), "the point list names it");
    }

    #[test]
    fn enabled_false_alone_turns_a_rule_off() {
        assert!(matches!(parse(&json!({ "enabled": false })).unwrap(), Parsed::TurnOff));
        assert!(!draft(json!({ "type": "deny", "point": "attack", "enabled": false })).enabled);
        assert_eq!(error(json!({ "enabled": true })), "a rule needs a type");
        assert_eq!(error(json!({ "enabled": false, "maps": [1] })), "a rule needs a type");
        assert_eq!(error(json!({})), "a rule needs a type");
        assert!(error(json!({ "type": "deny", "point": "attack", "enabled": "no" })).starts_with("enabled: "));
        assert_eq!(error(json!(3)), "a rule is a table");
    }

    #[test]
    fn the_default_texts_and_messages() {
        let text = |v: Value| draft(v).text.map(|t| t.to_string());
        assert_eq!(text(json!({ "type": "deny", "point": "attack" })).as_deref(), Some("You cannot attack that player here."));
        assert_eq!(text(json!({ "type": "level_gap", "gap": 5 })).as_deref(), Some("The level difference is too great to fight."));
        assert_eq!(text(json!({ "type": "deny", "point": "trade" })).as_deref(), Some("Trading is not allowed here."));
        assert_eq!(text(json!({ "type": "deny", "point": "personal_shop" })).as_deref(), Some("A personal shop cannot be opened here."));
        assert_eq!(text(json!({ "type": "deny", "point": "trade_chat" })).as_deref(), Some("Trade chat is not allowed here."));
        assert_eq!(text(json!({ "type": "deny", "point": "attack", "message": "No." })).as_deref(), Some("No."));
        assert_eq!(text(json!({ "type": "deny", "point": "attack", "message": "" })), None, "\"\" is silent");
        assert_eq!(text(json!({ "type": "pvp_gain", "exp": 0 })), None);
        assert!(error(json!({ "type": "deny", "point": "attack", "message": "é" })).contains("not printable ASCII"));
        assert!(error(json!({ "type": "deny", "point": "attack", "message": "a".repeat(101) })).contains("101 bytes is more than 100"));
        assert!(parse(&json!({ "type": "deny", "point": "attack", "message": "a".repeat(100) })).is_ok());
        assert!(error(json!({ "type": "deny", "point": "attack", "message": 1 })).starts_with("message: "));
        assert!(error(json!({ "type": "deny", "point": "attack", "message": "line\nbreak" })).contains("not printable ASCII"));
    }

    #[test]
    fn errors_name_the_key() {
        assert_eq!(error(json!({ "type": "level_gap", "gap": 0 })), "gap: 0 is not 1 to 255");
        assert_eq!(error(json!({ "type": "level_gap", "gap": 256 })), "gap: 256 is not 1 to 255");
        assert!(error(json!({ "type": "level_gap", "gap": 1.5 })).starts_with("gap: "));
        assert!(error(json!({ "type": "level_gap" })).contains("needs a gap"));
        assert!(error(json!({ "type": "deny" })).contains("needs a point"));
        assert!(error(json!({ "type": "deny", "point": "chat" })).starts_with("point: \"chat\" is not"));
        assert!(error(json!({ "type": "nothing" })).starts_with("type: unknown type \"nothing\"; the types are deny, level_gap"));
        assert!(error(json!({ "type": 1 })).starts_with("type: "));
        assert!(error(json!({ "type": "pvp_gain" })).contains("at least one of"));
        assert!(error(json!({ "type": "pvp_gain", "exp": 101 })).starts_with("exp: 101 is not 0 to 100"));
        assert!(error(json!({ "type": "pvp_gain", "exp": -1 })).starts_with("exp: "));
        assert!(error(json!({ "type": "pvp_gain", "kills": 101 })).starts_with("kills: 101 is not 0 to 100"));
        assert!(error(json!({ "type": "pvp_gain", "kills": 1.5 })).starts_with("kills: "));
        assert!(error(json!({ "type": "pvp_gain", "deaths": 1 })).starts_with("deaths: "));
        assert!(error(json!({ "type": "exp" })).contains("needs a factor"));
        assert!(error(json!({ "type": "exp", "factor": 101 })).starts_with("factor: "));
        assert!(error(json!({ "type": "exp", "factor": 2, "sources": ["pvp"] })).starts_with("sources: \"pvp\" is not"));
        assert!(error(json!({ "type": "exp", "factor": 2, "sources": [] })).starts_with("sources: "));
        assert!(error(json!({ "type": "enchant" })).contains("needs jumps"));
        assert!(error(json!({ "type": "enchant", "jumps": [] })).starts_with("jumps: 0 entries"));
        assert!(error(json!({ "type": "enchant", "jumps": vec![json!([1, 1]); 20] })).starts_with("jumps: 20 entries"));
        assert!(error(json!({ "type": "enchant", "jumps": [[0, 1]] })).contains("steps 0 is not 1 to 19"));
        assert!(error(json!({ "type": "enchant", "jumps": [[20, 1]] })).contains("steps 20 is not 1 to 19"));
        assert!(error(json!({ "type": "enchant", "jumps": [[1, 0]] })).contains("weight 0 is not 1 to 1000000"));
        assert!(error(json!({ "type": "enchant", "jumps": [[1, 1000001]] })).contains("weight 1000001"));
        assert!(error(json!({ "type": "enchant", "jumps": [[1]] })).contains("is not [steps, weight]"));
        assert!(error(json!({ "type": "enchant", "jumps": [[1, 1]], "items": "rings" })).starts_with("items: "));
        assert!(parse(&json!({ "type": "enchant", "jumps": vec![json!([19, 1_000_000]); 19] })).is_ok(), "the limits themselves");
        // A scope key's error names the scope key.
        assert!(error(json!({ "type": "deny", "point": "attack", "level": "0-5" })).starts_with("level: 0 is not 1 to 255"));
    }

    #[test]
    fn an_unknown_key_is_named_and_so_is_one_of_another_type() {
        let e = parse(&json!({ "type": "deny", "point": "attack", "hour": 3 })).unwrap_err();
        assert_eq!((e.msg.as_str(), e.line_key.as_deref(), e.path_key.as_deref()), ("unknown key \"hour\"", Some("hour"), None));
        let e = parse(&json!({ "type": "deny", "point": "attack", "gap": 3 })).unwrap_err();
        assert!(e.msg.starts_with("unknown key \"gap\"") && e.msg.contains("not a key of a deny rule"), "{}", e.msg);
        assert!(error(json!({ "type": "level_gap", "gap": 3, "factor": 2 })).contains("unknown key \"factor\""));
        assert!(error(json!({ "type": "exp", "factor": 2, "exp": 3 })).contains("unknown key \"exp\""));
        assert!(error(json!({ "type": "cross_faction_trade", "message": "x" })).contains("unknown key \"message\""), "a permission has no message");
        assert!(error(json!({ "type": "pvp_gain", "exp": 0, "message": "x" })).contains("unknown key \"message\""));
    }

    #[test]
    fn who_belongs_to_the_two_player_rules_only() {
        for ok in [
            json!({ "type": "deny", "point": "attack", "who": "target" }),
            json!({ "type": "deny", "point": "trade", "who": "both" }),
            json!({ "type": "level_gap", "gap": 3, "who": "actor" }),
            json!({ "type": "cross_faction_trade", "who": "either" }),
        ] {
            assert!(parse(&ok).is_ok(), "{ok}");
        }
        for bad in [
            json!({ "type": "deny", "point": "personal_shop", "who": "both" }),
            json!({ "type": "deny", "point": "trade_chat", "who": "both" }),
            json!({ "type": "pvp_gain", "exp": 0, "who": "both" }),
            json!({ "type": "exp", "factor": 2, "who": "both" }),
            json!({ "type": "enchant", "jumps": [[2, 1]], "who": "both" }),
        ] {
            assert!(error(bad.clone()).contains("unknown key \"who\""), "{bad}");
        }
    }

    #[test]
    fn kills_is_the_multiplier_of_pvp_gain_and_the_scope_everywhere_else() {
        // On a pvp_gain `kills` is the rule's own key (section 4.1), so that rule cannot be scoped by the kill count.
        let Kind::PvpGain { kills, .. } = draft(json!({ "type": "pvp_gain", "kills": 3 })).kind else { panic!() };
        assert_eq!(kills, 3);
        assert!(error(json!({ "type": "pvp_gain", "kills": "100-" })).starts_with("kills: "));
        let scoped = draft(json!({ "type": "deny", "point": "attack", "kills": "100-" }));
        assert!(scoped.scope.one(1, &scope::Who { level: 30, kills: 100, country: 0 }));
    }
}
