//! The layers of rules (`docs/plans/ext-2-rules.md` sections 4.3 and 7): the rules the three folded settings make, the enabled
//! extensions' rules, the config file's, the data editor's. A rule of a higher layer with the same name replaces the lower one whole.

use std::fmt;

use anyhow::Result;
use serde_json::{Value, json};

use super::parse::{self, Draft, Parsed};
use super::{RuleError, RuleSet, check_name};
use crate::config::Config;

/// Where a rule in force comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuleLayer {
    /// The rule a setting makes: the setting's name, and the layer the setting's value came from (empty when not known).
    Setting(String, String),
    /// An enabled extension's (`content.ext_rules`).
    Extension(String),
    /// `[rules.<name>]` of the config file.
    File,
    /// The data editor's (`content.rule_values`), with the change set that wrote it when known.
    Editor(Option<i32>),
}

impl RuleLayer {
    /// The order of precedence: the higher replaces the lower.
    fn rank(&self) -> u8 {
        match self {
            RuleLayer::Setting(..) => 1,
            RuleLayer::Extension(_) => 2,
            RuleLayer::File => 3,
            RuleLayer::Editor(_) => 4,
        }
    }
}

impl fmt::Display for RuleLayer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuleLayer::Setting(name, via) if via.is_empty() => write!(f, "setting {name}"),
            RuleLayer::Setting(name, via) => write!(f, "setting {name} ({via})"),
            RuleLayer::Extension(name) => write!(f, "extension {name}"),
            RuleLayer::File => write!(f, "config file"),
            RuleLayer::Editor(Some(change_set)) => write!(f, "data editor (change set {change_set})"),
            RuleLayer::Editor(None) => write!(f, "data editor"),
        }
    }
}

/// One `[rules.<name>]` table of the config file: the rule, and the lines of the table and of each of its keys (for the errors).
#[derive(Clone, Debug)]
pub struct FileRule {
    pub name: String,
    pub value: Value,
    pub line: usize,
    pub key_lines: Vec<(String, usize)>,
}

/// The config file's rules and the path they were read from.
#[derive(Clone, Debug, Default)]
pub struct RuleFile {
    pub path: String,
    pub rules: Vec<FileRule>,
}

/// What the layers above the settings hold.
#[derive(Clone, Debug, Default)]
pub struct RuleSources {
    /// `(extension, name, rule)` of the enabled extensions, in install order.
    pub extensions: Vec<(String, String, Value)>,
    pub file: Option<RuleFile>,
    /// `(name, rule, change set)` of the data editor.
    pub editor: Vec<(String, Value, Option<i32>)>,
    /// `(setting, layer text)` of the three folded settings, for the layer the rule they make is listed with.
    pub setting_layers: Vec<(String, String)>,
}

/// What became of a rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuleState {
    InForce,
    /// `enabled = false` in its own table.
    Disabled,
    /// A rule of the same name in a higher layer replaced it.
    Replaced(RuleLayer),
    /// `{ enabled = false }` alone, in a higher layer, turned it off.
    TurnedOff(RuleLayer),
}

/// One rule of the layers, after they were laid over each other.
#[derive(Clone, Debug)]
pub struct ResolvedRule {
    pub name: String,
    pub layer: RuleLayer,
    pub value: Value,
    pub state: RuleState,
}

/// The rules the settings make (section 7): `(setting, rule)`, named `setting.<setting>`.
pub fn from_settings(config: &Config) -> Vec<(&'static str, Value)> {
    let mut rules = Vec::new();
    if !config.cross_faction_trade_maps.is_empty() {
        rules.push(("cross_faction_trade_maps", json!({ "type": "cross_faction_trade", "maps": config.cross_faction_trade_maps })));
    }
    if !config.market_maps.is_empty() {
        rules.push(("market_maps", json!({ "type": "deny", "point": "personal_shop", "maps": { "not": config.market_maps }, "message": "" })));
    }
    if config.trade_chat_min_level >= 2 {
        let top = (config.trade_chat_min_level - 1).min(255);
        rules.push(("trade_chat_min_level", json!({ "type": "deny", "point": "trade_chat", "level": format!("1-{top}"), "message": "" })));
    }
    rules
}

struct Slot {
    name: String,
    layer: RuleLayer,
    value: Value,
    draft: Draft,
}

#[derive(Default)]
struct Resolver {
    active: Vec<Slot>,
    gone: Vec<ResolvedRule>,
}

impl Resolver {
    /// Lays one rule over the ones below. `place(key)` names where the rule is written (with the key's line when it has one).
    fn put(&mut self, name: &str, value: &Value, layer: RuleLayer, place: &dyn Fn(Option<&str>) -> String) -> Result<()> {
        // An extension's rule is written without its prefix in the manifest; its errors name it that way.
        let shown = match &layer {
            RuleLayer::Extension(extension) => name.strip_prefix(&format!("{extension}/")).unwrap_or(name),
            _ => name,
        };
        let fail = |e: &RuleError| anyhow::anyhow!("{}: {}", place(e.line_key.as_deref()), e.describe(shown));
        let parsed = parse::parse(value).map_err(|e| fail(&e))?;
        let below = self.active.iter().position(|s| s.name == name);
        match parsed {
            Parsed::TurnOff => {
                let Some(at) = below else {
                    anyhow::bail!("{}: rules.{shown}: no rule of that name in a lower layer to turn off", place(None));
                };
                let slot = self.active.remove(at);
                self.gone.push(ResolvedRule { name: slot.name, layer: slot.layer, value: slot.value, state: RuleState::TurnedOff(layer) });
            }
            Parsed::Rule(draft) => {
                if let Some(at) = below {
                    let slot = self.active.remove(at);
                    self.gone.push(ResolvedRule { name: slot.name, layer: slot.layer, value: slot.value, state: RuleState::Replaced(layer.clone()) });
                }
                self.active.push(Slot { name: name.to_owned(), layer, value: value.clone(), draft });
            }
        }
        Ok(())
    }
}

/// Lays the layers over each other (section 4.3), checking every rule with its parser, and compiles the rules in force. Errors name
/// the layer and the place: `config/game.toml line 14: rules.level-gap.gap: 0 is not 1 to 255`.
pub fn resolve(config: &Config, sources: &RuleSources) -> Result<(RuleSet, Vec<ResolvedRule>)> {
    let mut r = Resolver::default();
    for (setting, rule) in from_settings(config) {
        let via = sources.setting_layers.iter().find(|(name, _)| name == setting).map(|(_, via)| via.clone()).unwrap_or_default();
        r.put(&format!("setting.{setting}"), &rule, RuleLayer::Setting(setting.to_owned(), via), &|_| format!("setting {setting}"))?;
    }
    for (extension, name, rule) in &sources.extensions {
        let full = format!("{extension}/{name}");
        check_name(&full).map_err(|e| anyhow::anyhow!("extension {extension}: rules.{name}: {e}"))?;
        r.put(&full, rule, RuleLayer::Extension(extension.clone()), &|_| format!("extension {extension}"))?;
    }
    if let Some(file) = &sources.file {
        for rule in &file.rules {
            check_name(&rule.name).map_err(|e| anyhow::anyhow!("{} line {}: rules.{}: {e}", file.path, rule.line, rule.name))?;
            let place = |key: Option<&str>| {
                let line = key.and_then(|k| rule.key_lines.iter().find(|(name, _)| name == k)).map_or(rule.line, |&(_, line)| line);
                format!("{} line {line}", file.path)
            };
            r.put(&rule.name, &rule.value, RuleLayer::File, &place)?;
        }
    }
    for (name, rule, change_set) in &sources.editor {
        let layer = RuleLayer::Editor(*change_set);
        let place = layer.to_string();
        check_name(name).map_err(|e| anyhow::anyhow!("{place}: rules.{name}: {e}"))?;
        r.put(name, rule, layer, &|_| place.clone())?;
    }
    // The order of the rules in force: the editor's first, then the file's, the extensions', the settings'; by name within a layer.
    let mut slots = r.active;
    slots.sort_by(|a, b| b.layer.rank().cmp(&a.layer.rank()).then_with(|| a.name.cmp(&b.name)));
    let mut resolved = Vec::new();
    let mut compiled = Vec::new();
    for slot in slots {
        if slot.draft.enabled {
            compiled.push((slot.name.clone(), slot.layer.to_string(), slot.draft));
            resolved.push(ResolvedRule { name: slot.name, layer: slot.layer, value: slot.value, state: RuleState::InForce });
        } else {
            resolved.push(ResolvedRule { name: slot.name, layer: slot.layer, value: slot.value, state: RuleState::Disabled });
        }
    }
    let (in_force, off): (Vec<_>, Vec<_>) = resolved.into_iter().partition(|r| r.state == RuleState::InForce);
    let mut all = in_force;
    all.extend(off);
    all.extend(r.gone);
    Ok((RuleSet::build(compiled), all))
}
