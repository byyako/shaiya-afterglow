use serde_json::{Value, json};

use super::scope::CLOCK;
use super::*;
use crate::config::{self, Config, Sources, Startup};

fn config() -> Config {
    let startup =
        Startup { bind: "0.0.0.0:30810".parse().unwrap(), database_url: "postgres://localhost/test".into(), server_id: 1, config: None };
    config::resolve(&startup, &Sources::default()).unwrap().0
}

fn p(level: u16, country: u8) -> Who {
    Who { level, kills: 0, country }
}

/// The rules of a config file, by name.
fn file(rules: &[(&str, Value)]) -> RuleSources {
    let rules = rules
        .iter()
        .enumerate()
        .map(|(i, (name, value))| FileRule { name: (*name).into(), value: value.clone(), line: 10 + i, key_lines: vec![] })
        .collect();
    RuleSources { file: Some(RuleFile { path: "config/game.toml".into(), rules }), ..RuleSources::default() }
}

/// The rules in force of a config file (no settings made rules: the default config has the trade-chat one, which `set` drops).
fn set(rules: &[(&str, Value)]) -> RuleSet {
    resolve(&Config { trade_chat_min_level: 0, ..config() }, &file(rules)).unwrap().0
}

fn error(config: &Config, sources: &RuleSources) -> String {
    format!("{:#}", resolve(config, sources).unwrap_err())
}

fn names(set: &RuleSet) -> Vec<String> {
    set.list().iter().map(|l| l.name.to_string()).collect()
}

#[test]
fn an_empty_set_answers_allow_and_one_at_every_point() {
    let none = RuleSet::none();
    let (a, b) = (p(1, 0), p(255, 1));
    assert!(none.attack(1, &a, &b).is_none() && none.trade(1, &a, &b).is_none() && !none.cross_faction_trade(1, &a, &b));
    assert!(none.denied(Point::PersonalShop, 1, &a).is_none() && none.denied(Point::TradeChat, 1, &a).is_none());
    for source in [ExpSource::Mob, ExpSource::Party, ExpSource::Quest, ExpSource::Pvp] {
        assert_eq!(none.exp_factor(1, &a, source).value, 1.0);
    }
    assert_eq!(none.pvp_kills(1, &a).value, 1);
    assert!(none.pvp_deaths(1, &a));
    assert_eq!(none.enchant_jump(1, &a, false, 999_999).value, 1);
    assert!(none.item_use(1, &a, 100, 215).is_none());
    assert!(none.is_empty() && none.list().is_empty());
}

#[test]
fn deny_attack_refuses_in_its_scope_only() {
    let s = set(&[("safe", json!({ "type": "deny", "point": "attack", "maps": [2], "message": "No fighting here." }))]);
    let r = s.attack(2, &p(30, 0), &p(30, 1)).expect("refused on map 2");
    assert_eq!((&*r.name, r.text.as_deref()), ("safe", Some("No fighting here.")));
    assert!(s.attack(3, &p(30, 0), &p(30, 1)).is_none());
    let default = set(&[("x", json!({ "type": "deny", "point": "attack" }))]);
    assert_eq!(default.attack(1, &p(1, 0), &p(1, 1)).unwrap().text.as_deref(), Some("You cannot attack that player here."));
}

#[test]
fn level_gap_works_both_ways_and_at_the_edge() {
    let s = set(&[("gap", json!({ "type": "level_gap", "gap": 20 }))]);
    let (low, high) = (p(10, 0), p(31, 1));
    assert!(s.attack(1, &low, &high).is_some(), "21 apart: the low one may not attack");
    assert!(s.attack(1, &high, &low).is_some(), "21 apart: the high one may not attack either");
    let (low, edge) = (p(10, 0), p(30, 1));
    assert!(s.attack(1, &low, &edge).is_none() && s.attack(1, &edge, &low).is_none(), "20 apart: both may");
    assert_eq!(s.attack(1, &p(10, 0), &p(40, 1)).unwrap().text.as_deref(), Some("The level difference is too great to fight."));
    let except = set(&[("gap", json!({ "type": "level_gap", "gap": 20, "maps": { "not": [42] } }))]);
    assert!(except.attack(1, &low, &high).is_some() && except.attack(42, &low, &high).is_none());
}

#[test]
fn who_decides_which_player_a_deny_attack_scope_tests() {
    let rule = |who: &str| set(&[("x", json!({ "type": "deny", "point": "attack", "level": "1-15", "who": who }))]);
    let (low, high) = (p(10, 0), p(60, 1));
    let either = rule("either");
    assert!(either.attack(1, &low, &high).is_some() && either.attack(1, &high, &low).is_some(), "protection both ways");
    let actor = rule("actor");
    assert!(actor.attack(1, &low, &high).is_some() && actor.attack(1, &high, &low).is_none());
    let target = rule("target");
    assert!(target.attack(1, &high, &low).is_some() && target.attack(1, &low, &high).is_none());
    assert!(rule("both").attack(1, &low, &high).is_none() && rule("both").attack(1, &low, &p(12, 1)).is_some());
}

#[test]
fn trade_refusal_and_permission() {
    let s = set(&[
        ("grant", json!({ "type": "cross_faction_trade", "level": "60-" })),
        ("busy", json!({ "type": "deny", "point": "trade", "maps": [7] })),
    ]);
    let (a, b) = (p(60, 0), p(70, 1));
    assert!(s.cross_faction_trade(1, &a, &b), "both at 60 and up");
    assert!(!s.cross_faction_trade(1, &p(59, 0), &b), "who defaults to both: one trader at 59 is refused");
    assert!(!s.cross_faction_trade(1, &a, &p(59, 1)));
    assert!(s.trade(1, &a, &b).is_none());
    let denied = s.trade(7, &a, &b).unwrap();
    assert_eq!(denied.text.as_deref(), Some("Trading is not allowed here."));
    assert!(s.cross_faction_trade(7, &a, &b), "the two answers are independent; the caller lets the refusal win (section 5.2)");
    let either = set(&[("grant", json!({ "type": "cross_faction_trade", "level": "60-", "who": "either" }))]);
    assert!(either.cross_faction_trade(1, &p(59, 0), &b));
    let by_map = set(&[("grant", json!({ "type": "cross_faction_trade", "maps": [42] }))]);
    assert!(by_map.cross_faction_trade(42, &p(1, 0), &p(1, 1)) && !by_map.cross_faction_trade(41, &p(1, 0), &p(1, 1)));
}

#[test]
fn the_one_player_refusals() {
    let s = set(&[
        ("shop", json!({ "type": "deny", "point": "personal_shop", "maps": { "not": [35, 36, 42] } })),
        ("chat", json!({ "type": "deny", "point": "trade_chat", "level": "1-9", "message": "" })),
    ]);
    let a = p(5, 0);
    assert_eq!(s.denied(Point::PersonalShop, 1, &a).unwrap().text.as_deref(), Some("A personal shop cannot be opened here."));
    assert!(s.denied(Point::PersonalShop, 35, &a).is_none() && s.denied(Point::PersonalShop, 42, &a).is_none());
    let silent = s.denied(Point::TradeChat, 1, &a).unwrap();
    assert!(silent.text.is_none() && &*silent.name == "chat", "message \"\" is silent");
    assert!(s.denied(Point::TradeChat, 1, &p(10, 0)).is_none());
}

#[test]
fn deny_item_use_covers_the_items_it_names_in_its_scope() {
    let s = set(&[
        (
            "stones",
            json!({ "type": "deny", "point": "item_use", "items": ["100/215-217", "25/3", "95"], "maps": [2, 3], "message": "Level stones do not work here." }),
        ),
        ("novices", json!({ "type": "deny", "point": "item_use", "level": "1-9", "maps": [5] })),
    ]);
    let a = p(30, 0);
    let refusal = s.item_use(2, &a, 100, 216).expect("a range of type ids");
    assert_eq!((&*refusal.name, refusal.text.as_deref()), ("stones", Some("Level stones do not work here.")));
    assert!(s.item_use(2, &a, 100, 215).is_some() && s.item_use(2, &a, 100, 217).is_some(), "both ends");
    assert!(s.item_use(2, &a, 100, 214).is_none() && s.item_use(2, &a, 100, 218).is_none(), "just outside");
    assert!(s.item_use(3, &a, 25, 3).is_some() && s.item_use(3, &a, 25, 4).is_none(), "one item");
    assert!(
        s.item_use(3, &a, 95, 1).is_some() && s.item_use(3, &a, 95, 255).is_some() && s.item_use(3, &a, 96, 1).is_none(),
        "a whole type"
    );
    assert!(s.item_use(1, &a, 100, 216).is_none(), "another map");
    // Without `items` every item is covered, within the scope.
    assert!(s.item_use(5, &p(5, 0), 25, 1).is_some() && s.item_use(5, &p(5, 0), 100, 48).is_some());
    assert!(s.item_use(5, &p(30, 0), 25, 1).is_none() && s.item_use(6, &p(5, 0), 25, 1).is_none());
    assert_eq!(s.item_use(5, &p(5, 0), 25, 1).unwrap().text.as_deref(), Some("You cannot use that here."), "the point's default text");
    let hits: Vec<u64> = s.list().iter().map(|l| l.hits).collect();
    assert_eq!(hits.iter().sum::<u64>(), 9, "a hit for every refusal and none for a pass: {hits:?}");
}

#[test]
fn experience_factors_by_source_and_the_product() {
    let s = set(&[
        ("double", json!({ "type": "exp", "factor": 2, "maps": [5] })),
        ("quests", json!({ "type": "exp", "factor": 3, "sources": ["quest", "party"], "level": "1-20" })),
        ("pvp", json!({ "type": "pvp_gain", "exp": 0.5 })),
    ]);
    let a = p(10, 0);
    let f = |map, source| s.exp_factor(map, &a, source).value;
    assert_eq!((f(5, ExpSource::Mob), f(1, ExpSource::Mob)), (2.0, 1.0));
    assert_eq!(f(5, ExpSource::Party), 6.0, "the product of the two rules");
    assert_eq!(f(1, ExpSource::Quest), 3.0);
    assert_eq!(f(5, ExpSource::Pvp), 0.5, "a PvP kill's experience is pvp_gain's, not the exp rules'");
    assert_eq!(s.exp_factor(5, &p(30, 0), ExpSource::Party).value, 2.0, "level 30 is out of the quests rule");
    let zero = set(&[("none", json!({ "type": "exp", "factor": 0 })), ("two", json!({ "type": "exp", "factor": 2 }))]);
    assert_eq!(zero.exp_factor(1, &a, ExpSource::Mob).value, 0.0, "a factor of 0 always wins");
}

#[test]
fn player_kill_gains_kills_and_deaths() {
    let s = set(&[
        ("market", json!({ "type": "pvp_gain", "maps": [35, 36, 42], "exp": 0, "kills": 0, "deaths": false })),
        ("triple", json!({ "type": "pvp_gain", "maps": [9], "kills": 3 })),
        ("triple-again", json!({ "type": "pvp_gain", "maps": [9], "kills": 2 })),
    ]);
    let a = p(40, 0);
    assert_eq!(s.exp_factor(42, &a, ExpSource::Pvp).value, 0.0);
    assert_eq!((s.pvp_kills(42, &a).value, s.pvp_kills(1, &a).value, s.pvp_kills(9, &a).value), (0, 1, 6));
    assert!(!s.pvp_deaths(42, &a) && s.pvp_deaths(1, &a) && s.pvp_deaths(9, &a));
    let only_deaths = set(&[("d", json!({ "type": "pvp_gain", "deaths": false }))]);
    assert_eq!(
        (only_deaths.exp_factor(1, &a, ExpSource::Pvp).value, only_deaths.pvp_kills(1, &a).value, only_deaths.pvp_deaths(1, &a)),
        (1.0, 1, false)
    );
}

#[test]
fn enchant_draws_by_weight_at_the_boundaries() {
    let s = set(&[("jumps", json!({ "type": "enchant", "jumps": [[1, 70], [2, 20], [3, 10]] }))]);
    let a = p(40, 0);
    let draw = |d| s.enchant_jump(1, &a, false, d).value;
    assert_eq!((draw(0), draw(699_999), draw(700_000), draw(899_999), draw(900_000), draw(999_999)), (1, 1, 2, 2, 3, 3));
    assert_eq!(draw(5_000_000), 3, "a draw is capped at the last value");
    let always = set(&[("three", json!({ "type": "enchant", "jumps": [[3, 1]] }))]);
    assert_eq!((always.enchant_jump(1, &a, false, 0).value, always.enchant_jump(1, &a, false, 999_999).value), (3, 3));
    let big = set(&[("big", json!({ "type": "enchant", "jumps": [[1, 1_000_000], [19, 1_000_000]] }))]);
    assert_eq!(
        (big.enchant_jump(1, &a, false, 499_999).value, big.enchant_jump(1, &a, false, 500_000).value),
        (1, 19),
        "weights up to 1,000,000 each"
    );
}

#[test]
fn enchant_takes_the_first_matching_rule_alone_and_knows_the_items() {
    let s = set(&[
        ("a-armour", json!({ "type": "enchant", "jumps": [[2, 1]], "items": "armour", "maps": [5] })),
        ("b-weapons", json!({ "type": "enchant", "jumps": [[3, 1]], "items": "weapons" })),
        ("c-all", json!({ "type": "enchant", "jumps": [[4, 1]] })),
    ]);
    let a = p(40, 0);
    let jump = |map, armour| s.enchant_jump(map, &a, armour, 0).value;
    assert_eq!((jump(5, true), jump(5, false), jump(1, true), jump(1, false)), (2, 3, 4, 3), "the first that matches, not a product");
    let hits: Vec<(String, u64)> = s.list().into_iter().map(|l| (l.name.to_string(), l.hits)).collect();
    assert_eq!(hits, [("a-armour".to_owned(), 1), ("b-weapons".to_owned(), 2), ("c-all".to_owned(), 1)]);
}

#[test]
fn hits_count_the_matches_and_the_list_names_the_layer() {
    let s = set(&[("safe", json!({ "type": "deny", "point": "attack", "maps": [2] }))]);
    for _ in 0..3 {
        s.attack(2, &p(1, 0), &p(1, 1));
    }
    s.attack(3, &p(1, 0), &p(1, 1));
    let list = s.list();
    assert_eq!((list[0].hits, list[0].ty, list[0].layer.as_str(), &*list[0].name), (3, "deny", "config file", "safe"));
}

#[test]
fn the_time_scope_reads_the_clock() {
    let _guard = CLOCK.lock().unwrap_or_else(|e| e.into_inner());
    let s = set(&[("evening", json!({ "type": "deny", "point": "attack", "days": [1, 2, 3, 4, 5], "hours": "18-22" }))]);
    set_hour_of_week(24 + 19);
    assert!(s.attack(1, &p(1, 0), &p(1, 1)).is_some());
    set_hour_of_week(24 + 23);
    assert!(s.attack(1, &p(1, 0), &p(1, 1)).is_none());
    set_hour_of_week(19);
    assert!(s.attack(1, &p(1, 0), &p(1, 1)).is_none(), "Sunday");
}

// ------------------------------------------------------------------------------------------------------------------------------
// The layers
// ------------------------------------------------------------------------------------------------------------------------------

fn extension(extension: &str, name: &str, rule: Value) -> (String, String, Value) {
    (extension.into(), name.into(), rule)
}

#[test]
fn the_file_replaces_an_extensions_rule_and_the_editor_the_files() {
    let cfg = Config { trade_chat_min_level: 0, ..config() };
    let mut sources = file(&[("pets/no-capture", json!({ "type": "deny", "point": "attack", "maps": [2] }))]);
    sources.extensions = vec![extension("pets", "no-capture", json!({ "type": "deny", "point": "attack", "maps": [1] }))];
    let (s, all) = resolve(&cfg, &sources).unwrap();
    assert!(
        s.attack(2, &p(1, 0), &p(1, 1)).is_some() && s.attack(1, &p(1, 0), &p(1, 1)).is_none(),
        "the file's rule replaced the extension's whole"
    );
    assert_eq!(names(&s), ["pets/no-capture"]);
    assert_eq!(s.list()[0].layer, "config file");
    let replaced = all.iter().find(|r| r.state != RuleState::InForce).unwrap();
    assert_eq!(
        (replaced.layer.clone(), replaced.state.clone()),
        (RuleLayer::Extension("pets".into()), RuleState::Replaced(RuleLayer::File))
    );

    sources.editor = vec![("pets/no-capture".into(), json!({ "type": "deny", "point": "attack", "maps": [3] }), Some(12))];
    let (s, all) = resolve(&cfg, &sources).unwrap();
    assert!(s.attack(3, &p(1, 0), &p(1, 1)).is_some() && s.attack(2, &p(1, 0), &p(1, 1)).is_none());
    assert_eq!(s.list()[0].layer, "data editor (change set 12)");
    assert_eq!(all.iter().filter(|r| matches!(r.state, RuleState::Replaced(_))).count(), 2);
}

#[test]
fn enabled_false_alone_turns_off_a_lower_rule() {
    let cfg = Config { trade_chat_min_level: 0, ..config() };
    let mut sources = file(&[("pets/no-capture", json!({ "enabled": false }))]);
    sources.extensions = vec![extension("pets", "no-capture", json!({ "type": "deny", "point": "attack" }))];
    let (s, all) = resolve(&cfg, &sources).unwrap();
    assert!(s.is_empty());
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].state, RuleState::TurnedOff(RuleLayer::File));
    // The editor turns the file's own rule off.
    let mut sources = file(&[("mine", json!({ "type": "deny", "point": "attack" }))]);
    sources.editor = vec![("mine".into(), json!({ "enabled": false }), None)];
    assert!(resolve(&cfg, &sources).unwrap().0.is_empty());
    // A rule that says enabled = false itself is listed as disabled and not in force.
    let (s, all) = resolve(&cfg, &file(&[("off", json!({ "type": "deny", "point": "attack", "enabled": false }))])).unwrap();
    assert!(s.is_empty() && all[0].state == RuleState::Disabled);
    // A higher layer may define a turned-off name again, with a type.
    let mut sources = file(&[("mine", json!({ "enabled": false }))]);
    sources.extensions = vec![extension("pets", "x", json!({ "type": "deny", "point": "attack" }))];
    assert!(resolve(&cfg, &sources).is_err(), "nothing named mine to turn off");
}

#[test]
fn a_name_no_layer_has_needs_a_type() {
    let cfg = Config { trade_chat_min_level: 0, ..config() };
    let message = error(&cfg, &file(&[("x", json!({ "enabled": false }))]));
    assert_eq!(message, "config/game.toml line 10: rules.x: no rule of that name in a lower layer to turn off");
    let message = error(&cfg, &file(&[("x", json!({ "maps": [1] }))]));
    assert_eq!(message, "config/game.toml line 10: rules.x: a rule needs a type");
    let sources = RuleSources { editor: vec![("nothing".into(), json!({ "enabled": false }), Some(4))], ..RuleSources::default() };
    assert!(error(&cfg, &sources).starts_with("data editor (change set 4): rules.nothing: no rule of that name"));
}

#[test]
fn setting_names_are_refused_in_the_upper_layers() {
    let cfg = config();
    let message = error(&cfg, &file(&[("setting.market_maps", json!({ "type": "deny", "point": "attack" }))]));
    assert!(message.contains("rules.setting.market_maps") && message.contains("change the setting `market_maps` instead"), "{message}");
    let sources = RuleSources { editor: vec![("setting.x".into(), json!({ "enabled": false }), None)], ..RuleSources::default() };
    assert!(error(&cfg, &sources).contains("belong to the settings"));
    let sources = RuleSources {
        extensions: vec![extension("pets", "setting.x", json!({ "type": "deny", "point": "attack" }))],
        ..RuleSources::default()
    };
    assert!(resolve(&cfg, &sources).is_err());
}

#[test]
fn the_rules_in_force_are_ordered_editor_file_extension_settings() {
    let cfg = Config { market_maps: vec![35], ..config() };
    let rule = json!({ "type": "deny", "point": "attack" });
    let mut sources = file(&[("b-file", rule.clone()), ("a-file", rule.clone())]);
    sources.extensions = vec![extension("pets", "z", rule.clone()), extension("pets", "a", rule.clone())];
    sources.editor = vec![("b-editor".into(), rule.clone(), Some(2)), ("a-editor".into(), rule.clone(), None)];
    let (s, _) = resolve(&cfg, &sources).unwrap();
    assert_eq!(
        names(&s),
        ["a-editor", "b-editor", "a-file", "b-file", "pets/a", "pets/z", "setting.market_maps", "setting.trade_chat_min_level"]
    );
}

#[test]
fn two_refusals_give_the_first_ones_message() {
    let cfg = Config { trade_chat_min_level: 0, ..config() };
    let mut sources = file(&[("file", json!({ "type": "deny", "point": "attack", "message": "file" }))]);
    sources.editor = vec![("editor".into(), json!({ "type": "deny", "point": "attack", "message": "editor" }), None)];
    sources.extensions = vec![extension("pets", "ext", json!({ "type": "deny", "point": "attack", "message": "ext" }))];
    let (s, _) = resolve(&cfg, &sources).unwrap();
    assert_eq!(s.attack(1, &p(1, 0), &p(1, 1)).unwrap().text.as_deref(), Some("editor"));
}

#[test]
fn errors_name_the_layer_the_line_and_the_key() {
    let cfg = Config { trade_chat_min_level: 0, ..config() };
    let mut sources =
        file(&[("level-gap", json!({ "type": "level_gap", "gap": 0 })), ("safe", json!({ "type": "deny", "point": "attack", "hour": 3 }))]);
    sources.file.as_mut().unwrap().rules[0].key_lines = vec![("gap".into(), 14)];
    sources.file.as_mut().unwrap().rules[1].key_lines = vec![("hour".into(), 20)];
    let message = format!("{:#}", resolve(&cfg, &sources).unwrap_err());
    assert_eq!(message, "config/game.toml line 14: rules.level-gap.gap: 0 is not 1 to 255");
    sources.file.as_mut().unwrap().rules.remove(0);
    let message = format!("{:#}", resolve(&cfg, &sources).unwrap_err());
    assert_eq!(message, "config/game.toml line 20: rules.safe: unknown key \"hour\"");
    let missing = error(&cfg, &file(&[("x", json!({}))]));
    assert_eq!(missing, "config/game.toml line 10: rules.x: a rule needs a type");
    let sources =
        RuleSources { extensions: vec![extension("pets", "x", json!({ "type": "level_gap", "gap": 300 }))], ..RuleSources::default() };
    assert_eq!(error(&cfg, &sources), "extension pets: rules.x.gap: 300 is not 1 to 255");
    let sources = RuleSources { editor: vec![("x".into(), json!({ "type": "exp" }), Some(12))], ..RuleSources::default() };
    assert_eq!(error(&cfg, &sources), "data editor (change set 12): rules.x: an exp rule needs a factor, 0 to 100");
    let bad_name = error(&cfg, &file(&[("Bad Name", json!({ "type": "deny", "point": "attack" }))]));
    assert!(bad_name.starts_with("config/game.toml line 10: rules.Bad Name: ") && bad_name.contains("is not a rule name"), "{bad_name}");
}

#[test]
fn rule_names() {
    for good in ["a", "0", "level-gap", "a-b-c", &"a".repeat(40), "pets/no-capture", "kill-boost/x"] {
        assert!(check_name(good).is_ok(), "{good}");
    }
    for bad in ["", "-a", "A", "a_b", "a b", &"a".repeat(41), "/x", "pets/", "p/x", "9pets/x", "pets/-x", "a/b/c"] {
        assert!(check_name(bad).is_err(), "{bad:?}");
    }
    assert!(check_name("setting.market_maps").unwrap_err().contains("change the setting `market_maps` instead"));
}

#[test]
fn check_rule_takes_the_servers_own_parser() {
    assert!(check_rule("level-gap", &json!({ "type": "level_gap", "gap": 15 })).is_ok());
    assert!(check_rule("pets/no-capture", &json!({ "enabled": false })).is_ok(), "turns off a lower rule");
    assert!(check_whole_rule("no-capture", &json!({ "enabled": false })).is_err(), "an extension's own rule stands alone");
    assert!(check_whole_rule("no-capture", &json!({ "type": "deny", "point": "attack" })).is_ok());
    let message = format!("{:#}", check_rule("level-gap", &json!({ "type": "level_gap", "gap": 0 })).unwrap_err());
    assert_eq!(message, "rules.level-gap.gap: 0 is not 1 to 255");
    assert!(check_rule("setting.x", &json!({ "type": "deny", "point": "attack" })).is_err());
}

// ------------------------------------------------------------------------------------------------------------------------------
// The settings folded in (section 7)
// ------------------------------------------------------------------------------------------------------------------------------

fn from(config: &Config) -> RuleSet {
    resolve(config, &RuleSources::default()).unwrap().0
}

#[test]
fn the_default_install_has_one_rule_the_trade_chat_level() {
    let (s, all) = resolve(&config(), &RuleSources::default()).unwrap();
    assert_eq!(names(&s), ["setting.trade_chat_min_level"]);
    assert_eq!(s.list()[0].layer, "setting trade_chat_min_level");
    assert_eq!(all.len(), 1);
    let sources = RuleSources { setting_layers: vec![("trade_chat_min_level".into(), "default".into())], ..Default::default() };
    assert_eq!(resolve(&config(), &sources).unwrap().0.list()[0].layer, "setting trade_chat_min_level (default)");
}

#[test]
fn trade_chat_min_level_is_the_old_test_at_its_edges() {
    for min in [0u16, 1, 2, 10, 11, 255, 256, 1000] {
        let s = from(&Config { trade_chat_min_level: min, ..config() });
        for level in [1u16, 2, 9, 10, 11, 100, 255] {
            let old = level < min;
            let refused = s.denied(Point::TradeChat, 1, &p(level, 0));
            assert_eq!(refused.is_some(), old, "min {min}, level {level}");
            assert!(refused.is_none_or(|r| r.text.is_none()), "silent, as the setting is today");
        }
    }
    assert!(
        from(&Config { trade_chat_min_level: 0, ..config() }).is_empty()
            && from(&Config { trade_chat_min_level: 1, ..config() }).is_empty()
    );
}

#[test]
fn market_maps_is_the_old_test_in_and_out_of_the_list() {
    for maps in [vec![], vec![1u16], vec![35, 36, 42]] {
        let s = from(&Config { market_maps: maps.clone(), trade_chat_min_level: 0, ..config() });
        for map in [0u16, 1, 2, 35, 36, 41, 42, 65535] {
            let old_allows = maps.is_empty() || maps.contains(&map);
            let refused = s.denied(Point::PersonalShop, map, &p(30, 0));
            assert_eq!(refused.is_none(), old_allows, "{maps:?} map {map}");
            assert!(refused.is_none_or(|r| r.text.is_none()));
        }
    }
}

#[test]
fn cross_faction_trade_maps_is_the_old_test_for_both_factions() {
    for maps in [vec![], vec![42u16], vec![1, 42]] {
        let s = from(&Config { cross_faction_trade_maps: maps.clone(), trade_chat_min_level: 0, ..config() });
        for map in [0u16, 1, 41, 42, 43] {
            for (a, b) in [(p(30, 0), p(30, 1)), (p(30, 1), p(30, 0)), (p(1, 0), p(255, 1))] {
                assert_eq!(s.cross_faction_trade(map, &a, &b), maps.contains(&map), "{maps:?} map {map}");
            }
        }
    }
}
