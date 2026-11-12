//! The game server's settings (`docs/plans/ext-1-foundation.md` section 7): one registry of settings, four layers (the built-in
//! defaults, the enabled extensions' defaults, the config file, the data editor's overrides), and the start-up environment.

use std::collections::BTreeMap;
use std::fmt;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::rules::{FileRule, RuleSources};
use crate::script::sources::ScriptSources;
use crate::skills::{KillBoost, SkillMatch};

#[derive(Clone, Debug)]
pub struct Config {
    pub bind: SocketAddr,
    pub database_url: String,
    /// This server's id in `game_servers`; sessions must be bound to it.
    pub server_id: u8,
    /// Require the game connection to come from the address the account logged in from.
    /// The original did not check; on by default here.
    pub check_ip: bool,
    /// How often the player count and liveness are written to `game_servers`.
    pub heartbeat_interval: Duration,
    /// How long a new connection may take to send the handshake (the client sends it at once).
    pub handshake_timeout: Duration,
    /// How long a connection may stay silent once authenticated. The client sends a keepalive (`0x0003`) regularly.
    pub idle_timeout: Duration,
    /// A deletion request takes effect after this long (the original: 2 hours).
    pub delete_delay: Duration,
    /// A logout request takes effect after this long, unless the player stands up or moves (the original: 10 seconds).
    pub logout_delay: Duration,
    /// How long a character stays in the world after its connection ended with no logout pending (the original: 10 seconds).
    pub linger: Duration,
    /// How long the socket stays open after an operator's kick or ban notice, the character already out of the world, so the
    /// client shows the notice before its disconnect box (`docs/original/small-items.md` deviation 1). Zero closes at once.
    pub kick_notice: Duration,
    /// How long `/town` waits before the move (the original: 10 seconds).
    pub town_delay: Duration,
    /// How long a handshake waits for the account's older connection to go, a newer login having ended it
    /// (`docs/original/client-extras.md` deviation 3; the stay above and some more, inside the client's 30 s).
    pub handshake_wait: Duration,
    /// Trade chat (`0x1103`, the `$` channel, sent to the whole faction): lowest level and time between two messages.
    /// The US client enforces level 10 and 5 seconds itself; the original server did not check. 0 turns a limit off.
    pub trade_chat_min_level: u16,
    pub trade_chat_cooldown: Duration,
    /// Maps where players of both factions may trade with each other (`docs/original/trade.md` section 5; the original allows it
    /// nowhere). Default: none, as the original (`docs/server/deviations.md` section 1); the Auction House is 42.
    pub cross_faction_trade_maps: Vec<u16>,
    /// Maps where a personal shop may be set up (`docs/original/personal-shop.md` deviation 1; the original checks none, the client
    /// offers it in the capitals and the Auction House). Empty, the default: everywhere, as the original.
    pub market_maps: Vec<u16>,
    /// The party size needed to create a guild (`docs/original/guild.md` deviation 1; the original: exactly 7). 1 or 0: alone.
    pub guild_party_size: usize,
    /// The factions' kill counts move with player kills (`docs/original/pvp-2.md` section 9; the original's console turns
    /// `g_bDisableKillCount` on and off, off by default).
    pub kill_count: bool,
    /// The guild ranking battle's open times as `(weekday, hour, hours)` (`docs/original/guild-ranking.md` section 10); `None`
    /// keeps map 50's `CreateTime`. Test setting, e.g. `"(1,14,1)"`.
    pub ranking_times: Option<Vec<[u16; 3]>>,
    /// How long after the battle closes the ranking is updated (the original: 30 minutes).
    pub ranking_update_delay: Duration,
    /// Running steps (monsters chasing and going home, guards, walking NPCs, the Lua moves) are paced to the client's walking, with
    /// a step between the passes every 100 ms (`docs/original/chase-smoothing.md` deviations 1 to 5); off: the original's waits.
    pub smooth_chase: bool,
    /// The move speed check (`docs/original/move-check.md`; deviation 2): off leaves only the 35-unit step check.
    pub move_check: bool,
    /// The item mall sells and sends gifts (`docs/original/item-mall.md` section 10); off is the original's `g_bDisableShop` 1:
    /// purchases answer 1, gifts 6.
    pub item_mall: bool,
    /// The game log (`docs/original/game-log.md`): off writes nothing.
    pub game_log: bool,
    /// The chat log, every chat line and whisper (deviation 2; on as the original).
    pub game_chat_log: bool,
    /// Rows of the game log and the chat log older than this many days are deleted daily; 0 keeps them (deviation 5).
    pub game_log_days: u32,
    pub max_connections: usize,
    /// Simultaneous connections from one address (`docs/server/audit.md` R2); the check follows `accept` at once.
    pub max_per_ip: usize,
    /// New connections one address may open in a minute; the rest are closed unread.
    pub connects_per_minute: u32,
    /// A write to a client that has not been taken in this long ends the connection (R1).
    pub write_timeout: Duration,
    /// Bytes of packets queued for one connection before it counts as not keeping up and ends (R1).
    pub outbox_limit: usize,
    /// How often the `stats` line is logged (`docs/plans/test-bots.md` section 7); zero, the default, logs none.
    pub stats_every: Duration,
    /// The chat channels without a cooldown: lines a second a connection may say, and the burst; the rest are dropped (R4).
    pub chat_per_sec: u32,
    pub chat_burst: u32,
    /// Packets a second a connection may send, with a burst of twice that; over it for 5 seconds in a row ends the connection (R4).
    pub packets_per_sec: u32,
    /// The database pool's size and how long a query waits for a connection from it (R11).
    pub db_pool: u32,
    pub db_acquire: Duration,
    /// The monster experience event's factor at start (`docs/original/operator-console.md` section 2; the original's
    /// `g_fExp2XValue`, 1 at start): 0 or more, 1 is normal. The console's `exp` changes it until the next start.
    pub exp_rate: f64,
    /// What each counted player kill adds to the killer's kills at start (section 4, decision 2; replaces `[NEW_SERVER]`): 1 to 100.
    pub kill_rate: u32,
    /// Applied skills that multiply a counted player kill (section 4, decision 3; `docs/plans/done/client-patches.md` decision 1), by skill id
    /// or id and level, each with its multiplier; empty for none, the default (an addition: `docs/server/deviations.md` section 1).
    pub kill_boost_skills: Vec<KillBoost>,
    /// The gambling house (`docs/original/mini-game.md` decision 1): places the type-5 NPCs and handles `0x31xx`; on by default (the
    /// maintainer's choice), with the original's odds; off keeps them hidden, as the original does (an addition: `docs/server/deviations.md`
    /// section 1).
    pub mini_game: bool,
    /// Each winning slot line pays `multiplier × the table's bet × this / 100` (decision 2; 100, the default, is the original), 1 to 1000.
    pub slot_payout_percent: u32,
    /// The chance in per mille that the double-up's card falls on the guessed side, rounds 1 to 5 (the default is the original's,
    /// `400,200,100,50,10`).
    pub card_win_chances: [u32; 5],
    /// The staff levels that may type console commands in the game, 1 to this level (section 6 step 5).
    pub console_level: u8,
    /// The extensions' Lua scripts load (`docs/plans/ext-5-lua.md` section 8.2); false loads none at all.
    pub scripts: bool,
    /// The memory limit of one extension's Lua state, MB (1 to 256).
    pub script_memory_mb: u32,
    /// `GAME_CONFIG`, the config file's path as named at start (`None`: `config/game.toml`); `reload` reads the file again from it
    /// (`docs/plans/ext-2-rules.md` section 4.6).
    pub config_path: Option<String>,
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The registry (section 7.3)
// ---------------------------------------------------------------------------------------------------------------------------------

/// How a setting's value is written in the file and shown by `game-server config`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `true` or `false`.
    Flag,
    /// A number, written bare.
    Number,
    /// Anything else (a list, a time table): written as a string, exactly the text the parser reads.
    Text,
}

/// One game setting: its key in the config file (an old `GAME_` variable's name without the prefix, in lowercase), the default (the
/// original's behaviour, as 0.1.0 set it), one line of documentation for the example file, and the parser it goes through.
pub struct Setting {
    pub name: &'static str,
    pub default: &'static str,
    pub doc: &'static str,
    pub kind: Kind,
    check: fn(&str) -> Result<()>,
}

const fn s(name: &'static str, default: &'static str, kind: Kind, doc: &'static str, check: fn(&str) -> Result<()>) -> Setting {
    Setting { name, default, doc, kind, check }
}

/// Every game setting, in the order the example file lists them. Any other name is refused wherever it appears.
pub const SETTINGS: &[Setting] = &[
    s(
        "check_ip",
        "true",
        Kind::Flag,
        "The game connection must come from the address the account logged in from (the original did not check).",
        any,
    ),
    s("heartbeat_secs", "5", Kind::Number, "Seconds between the writes of the player count and liveness to game_servers.", uint::<u64>),
    s(
        "handshake_secs",
        "30",
        Kind::Number,
        "Seconds allowed for the whole handshake from the connect (the original: no deadline).",
        uint::<u64>,
    ),
    s(
        "idle_secs",
        "600",
        Kind::Number,
        "Seconds of silence allowed after the handshake, counted from the client's last bytes.",
        uint::<u64>,
    ),
    s("max_per_ip", "8", Kind::Number, "Simultaneous connections from one address; the rest are closed unread.", uint::<usize>),
    s(
        "connects_per_minute",
        "30",
        Kind::Number,
        "New connections one address may open in a minute; the rest are closed unread.",
        uint::<u32>,
    ),
    s(
        "write_timeout_secs",
        "20",
        Kind::Number,
        "A client that does not take a packet in this many seconds is disconnected as a lost connection.",
        uint::<u64>,
    ),
    s(
        "outbox_limit",
        "4194304",
        Kind::Number,
        "Bytes of packets queued for one connection before it counts as not keeping up and ends.",
        uint::<usize>,
    ),
    s(
        "stats_every",
        "0",
        Kind::Number,
        "Seconds between the stats lines (info, target stats: traffic, outboxes, the monster pass, lock waits, saves, memory). 0 logs none.",
        uint::<u64>,
    ),
    s(
        "chat_per_sec",
        "3",
        Kind::Number,
        "Lines a second for the chat channels without a cooldown; extra lines are dropped. 0 turns the limit off.",
        uint::<u32>,
    ),
    s("chat_burst", "5", Kind::Number, "The burst of the chat limit above.", uint::<u32>),
    s(
        "packets_per_sec",
        "100",
        Kind::Number,
        "Packets a second a connection may send, with a burst of twice that; over it for 5 seconds ends the connection. 0 turns the limit off.",
        uint::<u32>,
    ),
    s("db_pool", "24", Kind::Number, "The database pool's size (PostgreSQL allows 100 connections by default).", uint::<u32>),
    s("db_acquire_secs", "10", Kind::Number, "Seconds a query waits for a connection from the pool.", uint::<u64>),
    s(
        "delete_delay_secs",
        "7200",
        Kind::Number,
        "A character deletion request takes effect after this many seconds (as in the original).",
        uint::<u64>,
    ),
    s(
        "logout_secs",
        "10",
        Kind::Number,
        "A logout request (exit to character selection or quit) takes effect after this many seconds (as in the original).",
        uint::<u64>,
    ),
    s(
        "linger_secs",
        "10",
        Kind::Number,
        "A character whose connection ended in the world stays there this many seconds (as in the original).",
        uint::<u64>,
    ),
    s(
        "kick_notice_secs",
        "3",
        Kind::Number,
        "After an operator's kick or ban notice the connection stays open this many seconds so the client shows it; 0 closes at once.",
        uint::<u64>,
    ),
    s("move_check", "true", Kind::Flag, "The move speed check, as the original; false leaves only the 35-unit step check.", any),
    s(
        "smooth_chase",
        "true",
        Kind::Flag,
        "Running steps (chasing, guards, walking NPCs) wait for the client's own walking time, so they move without stops; false keeps the original's waits.",
        any,
    ),
    s("log", "true", Kind::Flag, "The game log: what the original sent to its log server is written to PostgreSQL (game_log).", any),
    s(
        "chat_log",
        "true",
        Kind::Flag,
        "The chat log: every chat line of types 1 to 7, whispers and staff chat included, as the original.",
        any,
    ),
    s(
        "log_days",
        "0",
        Kind::Number,
        "Rows of the game log and the chat log older than this many days are deleted daily; 0 keeps them.",
        uint::<u32>,
    ),
    s("item_mall", "true", Kind::Flag, "The item mall sells and sends gifts; false refuses both, as the original's g_bDisableShop.", any),
    s(
        "town_secs",
        "10",
        Kind::Number,
        "/town moves the player to the map's bind position after this many seconds standing still (as in the original).",
        uint::<u64>,
    ),
    s(
        "handshake_wait_secs",
        "15",
        Kind::Number,
        "A new connection of an account whose older one is still leaving waits this many seconds for it (the original: refused at once).",
        uint::<u64>,
    ),
    s(
        "trade_chat_min_level",
        "10",
        Kind::Number,
        "Trade chat ($, faction-wide): the lowest level, as the US client limits it; 0 turns the limit off.",
        uint::<u16>,
    ),
    s("trade_chat_cooldown_secs", "5", Kind::Number, "Trade chat: seconds between two messages; 0 turns the limit off.", uint::<u64>),
    s(
        "cross_faction_trade_maps",
        "",
        Kind::Text,
        "Maps (ids, e.g. [42] for the Auction House) where players of both factions may trade; none, as the original.",
        maps_ok,
    ),
    s(
        "market_maps",
        "",
        Kind::Text,
        "Maps (ids, e.g. [35, 36, 42]) where a personal shop may be set up; empty for every map, as the original.",
        maps_ok,
    ),
    s(
        "guild_party_size",
        "7",
        Kind::Number,
        "The party size needed to create a guild (the original: exactly 7); 1 or 0 lets a player create one alone.",
        uint::<usize>,
    ),
    s("kill_count", "true", Kind::Flag, "The factions' kill counts move with player kills, as the original's console default.", any),
    s(
        "ranking_times",
        "",
        Kind::Text,
        "The guild ranking battle's open times as (weekday,hour,hours) entries, e.g. \"(1,14,1),(3,10,2)\"; empty keeps map 50's own. A test override.",
        ranking_times_ok,
    ),
    s(
        "ranking_update_delay_secs",
        "1800",
        Kind::Number,
        "Seconds after the ranking battle closes until the ranking is updated (as in the original).",
        uint::<u64>,
    ),
    s(
        "exp_rate",
        "1",
        Kind::Number,
        "The monster experience event's factor at start, 0 or more; the console's exp changes it until the next start.",
        exp_rate_ok,
    ),
    s(
        "kill_rate",
        "1",
        Kind::Number,
        "What each counted player kill adds to the killer's kills, 1 to 100; the console's killrate changes it.",
        kill_rate_ok,
    ),
    s(
        "kill_boost_skills",
        "",
        Kind::Text,
        "A shorthand for the kill boost: id[/level][:times], comma-separated, e.g. \"200/1:2,200/2:5\". At start each entry gives those skills a kill_boost ability, which multiplies a counted kill while the skill is applied; an extension's rows can give the same ability (docs/server/effects.md).",
        kill_boosts_ok,
    ),
    s(
        "mini_game",
        "true",
        Kind::Flag,
        "The gambling house: places its NPCs and handles the slot machine and the card double-up; false hides the NPCs, as the original does.",
        any,
    ),
    s(
        "slot_payout_percent",
        "100",
        Kind::Number,
        "Each winning slot line pays multiplier x bet x this / 100, 1 to 1000; 100 is the original's table (about 41 % back), 220 about 90 %.",
        slot_payout_ok,
    ),
    s(
        "card_win_chances",
        "400,200,100,50,10",
        Kind::Text,
        "The double-up's chance in per mille, rounds 1 to 5, that the card falls on the guessed side; the default is the original's.",
        card_win_ok,
    ),
    s("console_level", "1", Kind::Number, "1 to 5: staff levels 1 to this may type .command lines in the game.", console_level_ok),
    s("max_connections", "5000", Kind::Number, "Total connections.", uint::<usize>),
    s("scripts", "true", Kind::Flag, "The enabled extensions' Lua scripts run; false loads no script at all (an operator's switch).", any),
    s(
        "script_memory_mb",
        "16",
        Kind::Number,
        "The memory limit of one extension's Lua state in MB, 1 to 256; a script past it is an error and disables its extension.",
        script_memory_ok,
    ),
];

/// The setting called `name`.
pub fn setting(name: &str) -> Option<&'static Setting> {
    SETTINGS.iter().find(|s| s.name == name)
}

/// A setting's resolved text, trimmed; the default when `values` has none.
fn text_of(values: &BTreeMap<&str, String>, name: &str) -> String {
    values.get(name).map_or_else(|| default_of(name), String::as_str).trim().to_owned()
}

fn number<T: FromStr>(values: &BTreeMap<&str, String>, name: &str) -> Result<T>
where
    T::Err: std::error::Error + Send + Sync + 'static,
{
    text_of(values, name).parse().with_context(|| name.to_owned())
}

/// The default of a setting that exists.
fn default_of(name: &str) -> &'static str {
    setting(name).unwrap_or_else(|| panic!("{name} is not in config::SETTINGS")).default
}

/// Whether `name` is a game setting and `value` passes the parser the server reads it with. Used by `data-editor` for an extension's
/// setting defaults and the editor's `setting` target (`docs/plans/ext-1-foundation.md` sections 5.7 and 7.4). The start-up variables
/// (`DATABASE_URL`, `GAME_BIND`, `GAME_SERVER_ID`, `GAME_CONFIG`) are not game settings.
pub fn check_setting(name: &str, value: &str) -> Result<()> {
    let Some(setting) = setting(name) else { anyhow::bail!("{name:?} is not a game setting (docs/server/settings.md)") };
    (setting.check)(value)
}

fn any(_: &str) -> Result<()> {
    Ok(())
}

fn uint<T: FromStr>(text: &str) -> Result<()> {
    text.trim().parse::<T>().map(drop).map_err(|_| anyhow::anyhow!("{text:?} is not a whole number in range"))
}

fn maps_ok(text: &str) -> Result<()> {
    maps(text).map(drop)
}
fn ranking_times_ok(text: &str) -> Result<()> {
    ranking_times(text).map(drop)
}
fn exp_rate_ok(text: &str) -> Result<()> {
    exp_rate(text).map(drop)
}
fn kill_rate_ok(text: &str) -> Result<()> {
    kill_rate(text).map(drop)
}
fn kill_boosts_ok(text: &str) -> Result<()> {
    kill_boosts(text).map(drop)
}
fn slot_payout_ok(text: &str) -> Result<()> {
    slot_payout_percent(text).map(drop)
}
fn card_win_ok(text: &str) -> Result<()> {
    card_win_chances(text).map(drop)
}
fn script_memory_ok(text: &str) -> Result<()> {
    script_memory_mb(text).map(drop)
}

fn console_level_ok(text: &str) -> Result<()> {
    console_level(text).map(drop)
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The layers (section 7.1)
// ---------------------------------------------------------------------------------------------------------------------------------

/// Where a resolved value came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Layer {
    Default,
    /// The defaults of an enabled extension (`content.ext_settings`).
    Extension(String),
    /// The config file.
    File,
    /// The data editor's overrides (`content.setting_values`), with the change set that wrote it when it is known.
    Editor(Option<i32>),
}

impl fmt::Display for Layer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Layer::Default => write!(f, "default"),
            Layer::Extension(name) => write!(f, "extension {name}"),
            Layer::File => write!(f, "config file"),
            Layer::Editor(Some(change_set)) => write!(f, "data editor (change set {change_set})"),
            Layer::Editor(None) => write!(f, "data editor"),
        }
    }
}

/// One setting after the layers are laid over each other.
#[derive(Clone, Debug)]
pub struct Resolved {
    pub name: &'static str,
    /// The text the parser reads.
    pub value: String,
    pub layer: Layer,
    pub default: &'static str,
}

/// The file's settings: the path it was read from, and each `(name, value text, line)`.
#[derive(Clone, Debug, Default)]
pub struct FileLayer {
    pub path: String,
    pub values: Vec<(String, String, usize)>,
    /// The `[rules.<name>]` tables (`docs/plans/ext-2-rules.md` section 4.2).
    pub rules: Vec<FileRule>,
    /// The `[ext.<name>]` tables: the extensions' option values (`docs/plans/ext-5-lua.md` section 3.4).
    pub options: Vec<FileOption>,
}

/// One `<key> = <value>` of an `[ext.<extension>]` table, with its line; checked against the option's spec by
/// `script::resolve_options`.
#[derive(Clone, Debug)]
pub struct FileOption {
    pub extension: String,
    pub key: String,
    pub value: serde_json::Value,
    pub line: usize,
}

/// An override of the data editor: `(name, value, change set)`.
pub type EditorValue = (String, String, Option<i32>);

/// What the layers above the defaults hold.
#[derive(Clone, Debug, Default)]
pub struct Sources {
    /// `(extension, name, value)` of the enabled extensions, in install order.
    pub extensions: Vec<(String, String, String)>,
    pub file: Option<FileLayer>,
    pub editor: Vec<EditorValue>,
}

/// The start-up environment: `DATABASE_URL`, `GAME_BIND`, `GAME_SERVER_ID` and `GAME_CONFIG` (`RUST_LOG` is the logger's).
#[derive(Clone, Debug)]
pub struct Startup {
    pub bind: SocketAddr,
    pub database_url: String,
    pub server_id: u8,
    /// `GAME_CONFIG`: the config file's path; unset, `config/game.toml` in the working folder.
    pub config: Option<String>,
}

/// The config file's default path.
pub const DEFAULT_CONFIG: &str = "config/game.toml";

impl Startup {
    pub fn from_env() -> Result<Self> {
        let vars: Vec<(String, String)> =
            std::env::vars_os().filter_map(|(k, v)| Some((k.into_string().ok()?, v.into_string().ok()?))).collect();
        Self::from_vars(&vars)
    }

    /// Reads the start-up variables from `vars`; any other `GAME_` variable stops the start with the lines to paste into the file.
    pub fn from_vars(vars: &[(String, String)]) -> Result<Self> {
        let get = |name: &str| vars.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone());
        let mut stray: Vec<&(String, String)> = vars
            .iter()
            .filter(|(k, _)| k.starts_with("GAME_") && !matches!(k.as_str(), "GAME_BIND" | "GAME_SERVER_ID" | "GAME_CONFIG"))
            .collect();
        stray.sort();
        if !stray.is_empty() {
            let mut lines = String::new();
            for (variable, value) in stray {
                match setting(&variable["GAME_".len()..].to_ascii_lowercase()) {
                    Some(setting) => {
                        let line = format!("{} = {:?}", setting.name, value);
                        lines.push_str(&format!("{line:<40} # was {variable}\n"));
                    }
                    None => lines.push_str(&format!("# {variable} is not a setting (a typo? the old server ignored it); remove it\n")),
                }
            }
            anyhow::bail!(
                "game settings are read from config/game.toml now, not from the environment (docs/server/settings.md). Put these lines in\n\
                 config/game.toml (GAME_CONFIG names another file), remove the variables (from .env too: the run scripts load it), and start again:\n{lines}"
            );
        }
        Ok(Self {
            bind: get("GAME_BIND").unwrap_or_else(|| "0.0.0.0:30810".into()).parse().context("GAME_BIND")?,
            database_url: get("DATABASE_URL").context("DATABASE_URL is required")?,
            server_id: get("GAME_SERVER_ID").unwrap_or_else(|| "1".into()).parse().context("GAME_SERVER_ID")?,
            config: get("GAME_CONFIG").filter(|c| !c.is_empty()),
        })
    }
}

/// The text of a TOML value as the parsers read it: a string as written, a number or boolean as its text, a list joined with `,`.
fn setting_text(value: &toml::Value) -> Result<String, String> {
    Ok(match value {
        toml::Value::String(s) => s.clone(),
        toml::Value::Integer(i) => i.to_string(),
        toml::Value::Float(f) => f.to_string(),
        toml::Value::Boolean(b) => b.to_string(),
        toml::Value::Array(items) => items
            .iter()
            .map(|v| match v {
                toml::Value::String(s) => Ok(s.clone()),
                toml::Value::Integer(i) => Ok(i.to_string()),
                other => Err(format!("a list holds integers or strings, not {}", other.type_str())),
            })
            .collect::<Result<Vec<_>, _>>()?
            .join(","),
        other => return Err(format!("a {} cannot be a setting", other.type_str())),
    })
}

/// A TOML value as JSON, for the rules (`docs/plans/ext-2-rules.md` section 1: one parser serves every layer).
fn toml_to_json(value: &toml::Value) -> Result<serde_json::Value, String> {
    use serde_json::Value;
    Ok(match value {
        toml::Value::String(s) => Value::String(s.clone()),
        toml::Value::Integer(i) => Value::from(*i),
        toml::Value::Float(f) => {
            Value::Number(serde_json::Number::from_f64(*f).ok_or_else(|| format!("{f} is not a number a rule can hold"))?)
        }
        toml::Value::Boolean(b) => Value::Bool(*b),
        toml::Value::Array(items) => Value::Array(items.iter().map(toml_to_json).collect::<Result<_, _>>()?),
        toml::Value::Table(table) => {
            Value::Object(table.iter().map(|(k, v)| Ok((k.clone(), toml_to_json(v)?))).collect::<Result<_, String>>()?)
        }
        toml::Value::Datetime(_) => return Err("a date cannot be in a rule".into()),
    })
}

fn valid_extension_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    (2..=40).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes.iter().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
}

fn line_of(text: &str, offset: usize) -> usize {
    text[..offset.min(text.len())].bytes().filter(|&b| b == b'\n').count() + 1
}

/// Reads the config file's text (`docs/plans/ext-1-foundation.md` section 7.2). Errors name the file and the line; a value is not
/// checked here (see [`resolve`]), only its kind and the key.
pub fn read_file(path: &str, text: &str) -> Result<FileLayer> {
    let fail = |line: usize, message: String| anyhow::anyhow!("{path} line {line}: {message}");
    let spans = toml::de::DeTable::parse(text).map_err(|e| fail(e.span().map_or(1, |s| line_of(text, s.start)), e.message().to_owned()))?;
    let values: toml::Table =
        toml::from_str(text).map_err(|e| fail(e.span().map_or(1, |s| line_of(text, s.start)), e.message().to_owned()))?;
    let mut file = FileLayer { path: path.to_owned(), values: Vec::new(), rules: Vec::new(), options: Vec::new() };
    for (key, value) in spans.get_ref().iter() {
        let name: &str = key.get_ref();
        let line = line_of(text, key.span().start);
        let parsed = &values[name];
        match name {
            "ext" => {
                let (toml::Value::Table(tables), toml::de::DeValue::Table(spanned)) = (parsed, value.get_ref()) else {
                    return Err(fail(line, "ext: write one table per extension, [ext.<name>] (docs/server/scripts.md)".into()));
                };
                for (ext_key, ext_value) in spanned.iter() {
                    let extension: &str = ext_key.get_ref();
                    let ext_line = line_of(text, ext_key.span().start);
                    if !valid_extension_name(extension) {
                        return Err(fail(ext_line, format!("ext.{extension}: not an extension name (^[a-z][a-z0-9-]{{1,39}}$)")));
                    }
                    let (toml::Value::Table(body), toml::de::DeValue::Table(keys)) = (&tables[extension], ext_value.get_ref()) else {
                        return Err(fail(ext_line, format!("ext.{extension}: an extension's options are a table, [ext.{extension}]")));
                    };
                    for (opt_key, opt_value) in keys.iter() {
                        let key: &str = opt_key.get_ref();
                        let key_line = line_of(text, opt_key.span().start);
                        if matches!(opt_value.get_ref(), toml::de::DeValue::Table(_)) {
                            return Err(fail(key_line, format!("ext.{extension}.{key}: an option is a value, not a table")));
                        }
                        let json = toml_to_json(&body[key]).map_err(|e| fail(key_line, format!("ext.{extension}.{key}: {e}")))?;
                        file.options.push(FileOption { extension: extension.to_owned(), key: key.to_owned(), value: json, line: key_line });
                    }
                }
                continue;
            }
            "rules" => {
                let (toml::Value::Table(tables), toml::de::DeValue::Table(spanned)) = (parsed, value.get_ref()) else {
                    return Err(fail(line, "rules: write one table per rule, [rules.<name>] (docs/server/rules.md)".into()));
                };
                for (rule_key, rule_value) in spanned.iter() {
                    let rule_name: &str = rule_key.get_ref();
                    let rule_line = line_of(text, rule_key.span().start);
                    let toml::Value::Table(body) = &tables[rule_name] else {
                        return Err(fail(rule_line, format!("rules.{rule_name}: a rule is a table, [rules.{rule_name}]")));
                    };
                    let key_lines = match rule_value.get_ref() {
                        toml::de::DeValue::Table(keys) => {
                            keys.iter().map(|(k, _)| (k.get_ref().to_string(), line_of(text, k.span().start))).collect()
                        }
                        _ => Vec::new(),
                    };
                    let json =
                        toml_to_json(&toml::Value::Table(body.clone())).map_err(|e| fail(rule_line, format!("rules.{rule_name}: {e}")))?;
                    file.rules.push(FileRule { name: rule_name.to_owned(), value: json, line: rule_line, key_lines });
                }
                continue;
            }
            _ => {}
        }
        if setting(name).is_none() {
            let table = if matches!(parsed, toml::Value::Table(_)) { " (a table: the file has top-level keys only)" } else { "" };
            return Err(fail(line, format!("unknown setting {name:?}{table}")));
        }
        let text = setting_text(parsed).map_err(|e| fail(line, format!("{name}: {e}")))?;
        file.values.push((name.to_owned(), text, line));
    }
    Ok(file)
}

/// Lays the layers over the defaults, checking every value with its setting's parser, and builds the [`Config`]. Errors name the layer
/// (and for the file the line): `config/game.toml line 6: market_maps: …`, `extension kill-boost: kill_boost_skills: …`,
/// `data editor (change set 12): …`.
pub fn resolve(startup: &Startup, sources: &Sources) -> Result<(Config, Vec<Resolved>)> {
    let mut resolved: Vec<Resolved> = SETTINGS
        .iter()
        .map(|s| Resolved { name: s.name, value: s.default.to_owned(), layer: Layer::Default, default: s.default })
        .collect();
    let mut put = |name: &str, value: &str, layer: Layer, place: String| -> Result<()> {
        let Some(slot) = resolved.iter_mut().find(|r| r.name == name) else {
            anyhow::bail!("{place}: {name:?} is not a game setting (docs/server/settings.md)")
        };
        check_setting(name, value).map_err(|e| anyhow::anyhow!("{place}: {name}: {e:#}"))?;
        slot.value = value.to_owned();
        slot.layer = layer;
        Ok(())
    };
    for (extension, name, value) in &sources.extensions {
        put(name, value, Layer::Extension(extension.clone()), format!("extension {extension}"))?;
    }
    if let Some(file) = &sources.file {
        for (name, value, line) in &file.values {
            put(name, value, Layer::File, format!("{} line {line}", file.path))?;
        }
    }
    for (name, value, change_set) in &sources.editor {
        let place = match change_set {
            Some(n) => format!("data editor (change set {n})"),
            None => "data editor".to_owned(),
        };
        put(name, value, Layer::Editor(*change_set), place)?;
    }
    let values: BTreeMap<&str, String> = resolved.iter().map(|r| (r.name, r.value.clone())).collect();
    Ok((Config::from_values(startup, &values)?, resolved))
}

impl Config {
    /// The settings as the layers resolved them: `values` holds the text of every setting by name.
    fn from_values(startup: &Startup, values: &BTreeMap<&str, String>) -> Result<Self> {
        let var = |name: &str| text_of(values, name);
        let secs = |name: &str| -> Result<Duration> { Ok(Duration::from_secs(number(values, name)?)) };
        Ok(Self {
            bind: startup.bind,
            database_url: startup.database_url.clone(),
            server_id: startup.server_id,
            check_ip: flag(&var("check_ip")),
            heartbeat_interval: secs("heartbeat_secs")?,
            handshake_timeout: secs("handshake_secs")?,
            idle_timeout: secs("idle_secs")?,
            delete_delay: secs("delete_delay_secs")?,
            logout_delay: secs("logout_secs")?,
            linger: secs("linger_secs")?,
            kick_notice: secs("kick_notice_secs")?,
            town_delay: secs("town_secs")?,
            handshake_wait: secs("handshake_wait_secs")?,
            trade_chat_min_level: number::<u16>(values, "trade_chat_min_level")?,
            trade_chat_cooldown: secs("trade_chat_cooldown_secs")?,
            cross_faction_trade_maps: maps(&var("cross_faction_trade_maps")).context("cross_faction_trade_maps")?,
            market_maps: maps(&var("market_maps")).context("market_maps")?,
            guild_party_size: number::<usize>(values, "guild_party_size")?,
            kill_count: flag(&var("kill_count")),
            ranking_times: ranking_times(&var("ranking_times")).context("ranking_times")?,
            ranking_update_delay: secs("ranking_update_delay_secs")?,
            smooth_chase: flag(&var("smooth_chase")),
            move_check: flag(&var("move_check")),
            item_mall: flag(&var("item_mall")),
            game_log: flag(&var("log")),
            game_chat_log: flag(&var("chat_log")),
            game_log_days: number::<u32>(values, "log_days")?,
            max_connections: number::<usize>(values, "max_connections")?,
            max_per_ip: number::<usize>(values, "max_per_ip")?,
            connects_per_minute: number::<u32>(values, "connects_per_minute")?,
            write_timeout: secs("write_timeout_secs")?,
            outbox_limit: number::<usize>(values, "outbox_limit")?,
            stats_every: secs("stats_every")?,
            chat_per_sec: number::<u32>(values, "chat_per_sec")?,
            chat_burst: number::<u32>(values, "chat_burst")?,
            packets_per_sec: number::<u32>(values, "packets_per_sec")?,
            db_pool: number::<u32>(values, "db_pool")?,
            db_acquire: secs("db_acquire_secs")?,
            exp_rate: exp_rate(&var("exp_rate")).context("exp_rate")?,
            kill_rate: kill_rate(&var("kill_rate")).context("kill_rate")?,
            kill_boost_skills: kill_boosts(&var("kill_boost_skills")).context("kill_boost_skills")?,
            mini_game: flag(&var("mini_game")),
            slot_payout_percent: slot_payout_percent(&var("slot_payout_percent")).context("slot_payout_percent")?,
            card_win_chances: card_win_chances(&var("card_win_chances")).context("card_win_chances")?,
            console_level: console_level(&var("console_level")).context("console_level")?,
            scripts: flag(&var("scripts")),
            script_memory_mb: script_memory_mb(&var("script_memory_mb")).context("script_memory_mb")?,
            config_path: startup.config.clone(),
        })
    }

    /// Reads the start-up environment, the config file and the database layers (one connection, which also runs the migrations; the
    /// pool is built after, with the resolved `db_pool`) and resolves the settings (section 7.3). The rules' layers come with them
    /// (`docs/plans/ext-2-rules.md` section 4.3); `rules::resolve` puts them together.
    pub async fn load() -> Result<(Config, Vec<Resolved>, RuleSources, ScriptSources)> {
        Self::read(true).await
    }

    /// As [`Config::load`] for `game-server config` and `game-server rules`: no migrations, and a database without the `content`
    /// schema (before 0.3.0) has no database layers.
    pub async fn inspect() -> Result<(Config, Vec<Resolved>, RuleSources, ScriptSources)> {
        Self::read(false).await
    }

    async fn read(migrate: bool) -> Result<(Config, Vec<Resolved>, RuleSources, ScriptSources)> {
        use sqlx::Connection;
        let startup = Startup::from_env()?;
        let mut sources = Sources { file: read_config_file(startup.config.as_deref())?, ..Sources::default() };
        let mut rules = RuleSources {
            file: sources.file.as_ref().map(|f| crate::rules::RuleFile { path: f.path.clone(), rules: f.rules.clone() }),
            ..RuleSources::default()
        };
        let mut conn = sqlx::PgConnection::connect(&startup.database_url).await.context("connecting to DATABASE_URL")?;
        if migrate {
            sqlx::migrate!("../../db/migrations").run(&mut conn).await?;
        }
        let present: bool = sqlx::query_scalar(
            "SELECT to_regclass('content.ext_settings') IS NOT NULL AND to_regclass('content.setting_values') IS NOT NULL",
        )
        .fetch_one(&mut conn)
        .await?;
        if present {
            sources.extensions = sqlx::query_as(
                "SELECT s.extension, s.name, s.value FROM content.ext_settings s JOIN content.extensions e ON e.name = s.extension \
                 WHERE e.enabled ORDER BY e.install_seq, s.name",
            )
            .fetch_all(&mut conn)
            .await?;
            let edits: bool = sqlx::query_scalar("SELECT to_regclass('edits.fields') IS NOT NULL").fetch_one(&mut conn).await?;
            // The extensions' option values (`ext.<extension>.<option>`) are the scripts' layer, not a game setting.
            let query = if edits {
                "SELECT v.name, v.value, (SELECT max(f.change_set) FROM edits.fields f WHERE f.target = 'setting' AND f.key = v.name) \
                 FROM content.setting_values v WHERE v.name NOT LIKE 'ext.%' ORDER BY v.name"
            } else {
                "SELECT name, value, NULL::integer FROM content.setting_values WHERE name NOT LIKE 'ext.%' ORDER BY name"
            };
            sources.editor = sqlx::query_as(query).fetch_all(&mut conn).await?;
        }
        let layers = read_rule_layers(&mut conn).await?;
        rules.extensions = layers.extensions;
        rules.editor = layers.editor;
        let mut scripts = read_script_sources(&mut conn).await?;
        scripts.file =
            sources.file.as_ref().map(|f| crate::script::sources::FileOptions { path: f.path.clone(), options: f.options.clone() });
        conn.close().await?;
        let (config, resolved) = resolve(&startup, &sources)?;
        rules.setting_layers = resolved
            .iter()
            .filter(|r| matches!(r.name, "cross_faction_trade_maps" | "market_maps" | "trade_chat_min_level"))
            .map(|r| (r.name.to_owned(), r.layer.to_string()))
            .collect();
        Ok((config, resolved, rules, scripts))
    }
}

/// The database's side of the scripts (`docs/plans/ext-5-lua.md` section 9.1): the enabled extensions' scripts and option specs in
/// install order, the data editor's option values, the installed extensions. Empty before migration 0033.
pub async fn read_script_sources(conn: &mut sqlx::PgConnection) -> Result<ScriptSources> {
    read_script_sources_in(conn, "content", "edits").await
}

/// As [`read_script_sources`] with the schemas named (the tests of the data editor keep theirs apart). The names are the caller's own,
/// never a user's.
pub async fn read_script_sources_in(conn: &mut sqlx::PgConnection, content: &str, edits: &str) -> Result<ScriptSources> {
    use sqlx::AssertSqlSafe;
    let mut out = ScriptSources::default();
    let present: bool = sqlx::query_scalar(AssertSqlSafe(format!(
        "SELECT to_regclass('{content}.ext_scripts') IS NOT NULL AND to_regclass('{content}.ext_options') IS NOT NULL AND to_regclass('{content}.script_globals') IS NOT NULL"
    )))
    .fetch_one(&mut *conn)
    .await?;
    if !present {
        return Ok(out);
    }
    out.installed = sqlx::query_as(AssertSqlSafe(format!("SELECT name, enabled FROM {content}.extensions ORDER BY install_seq")))
        .fetch_all(&mut *conn)
        .await?;
    let scripts: Vec<(String, String, String)> = sqlx::query_as(AssertSqlSafe(format!(
        "SELECT s.extension, s.path, s.source FROM {content}.ext_scripts s JOIN {content}.extensions e ON e.name = s.extension \
         WHERE e.enabled ORDER BY e.install_seq, s.seq"
    )))
    .fetch_all(&mut *conn)
    .await?;
    for (extension, path, source) in scripts {
        match out.extensions.last_mut() {
            Some(e) if e.name == extension => e.scripts.push((path, source)),
            _ => out.extensions.push(crate::script::sources::ScriptExtension {
                name: extension,
                scripts: vec![(path, source)],
                ..Default::default()
            }),
        }
    }
    let options: Vec<(String, String, String)> = sqlx::query_as(AssertSqlSafe(format!(
        "SELECT o.extension, o.name, o.spec::text FROM {content}.ext_options o JOIN {content}.extensions e ON e.name = o.extension \
         WHERE e.enabled ORDER BY o.extension, o.name"
    )))
    .fetch_all(&mut *conn)
    .await?;
    for (extension, name, spec) in options {
        let json: serde_json::Value = serde_json::from_str(&spec).with_context(|| format!("content.ext_options {extension}.{name}"))?;
        let spec = crate::script::OptionSpec::parse(&json).with_context(|| format!("content.ext_options {extension}.{name}"))?;
        if let Some(e) = out.extensions.iter_mut().find(|e| e.name == extension) {
            e.options.push((name, spec));
        }
    }
    let globals: Vec<(String, String, String)> =
        sqlx::query_as(AssertSqlSafe(format!("SELECT extension, key, value::text FROM {content}.script_globals ORDER BY extension, key")))
            .fetch_all(&mut *conn)
            .await?;
    for (extension, key, value) in globals {
        let json: serde_json::Value = serde_json::from_str(&value).with_context(|| format!("content.script_globals {extension}.{key}"))?;
        if let Some(e) = out.extensions.iter_mut().find(|e| e.name == extension) {
            e.globals.push((key, json));
        }
    }
    let settings: bool = sqlx::query_scalar(AssertSqlSafe(format!("SELECT to_regclass('{content}.setting_values') IS NOT NULL")))
        .fetch_one(&mut *conn)
        .await?;
    if settings {
        let has_edits: bool =
            sqlx::query_scalar(AssertSqlSafe(format!("SELECT to_regclass('{edits}.fields') IS NOT NULL"))).fetch_one(&mut *conn).await?;
        let query = if has_edits {
            format!(
                "SELECT v.name, v.value, (SELECT max(f.change_set) FROM {edits}.fields f WHERE f.target = 'setting' AND f.key = v.name) \
                 FROM {content}.setting_values v WHERE v.name LIKE 'ext.%' ORDER BY v.name"
            )
        } else {
            format!("SELECT name, value, NULL::integer FROM {content}.setting_values WHERE name LIKE 'ext.%' ORDER BY name")
        };
        out.editor = sqlx::query_as(AssertSqlSafe(query)).fetch_all(&mut *conn).await?;
    }
    Ok(out)
}

/// The database layers of the rules (`docs/plans/ext-2-rules.md` section 4.3): the enabled extensions' (`content.ext_rules`, in
/// install order) and the data editor's (`content.rule_values`, with the change set that wrote each). Empty before migration 0032.
/// Returned as a [`RuleSources`] with the other layers left empty.
pub async fn read_rule_layers(conn: &mut sqlx::PgConnection) -> Result<RuleSources> {
    read_rule_layers_in(conn, "content", "edits").await
}

/// As [`read_rule_layers`] with the schemas named (the tests of the data editor keep theirs apart). The names are the caller's own,
/// never a user's.
pub async fn read_rule_layers_in(conn: &mut sqlx::PgConnection, content: &str, edits: &str) -> Result<RuleSources> {
    use sqlx::AssertSqlSafe;
    let mut layers = RuleSources::default();
    let present: bool = sqlx::query_scalar(AssertSqlSafe(format!(
        "SELECT to_regclass('{content}.ext_rules') IS NOT NULL AND to_regclass('{content}.rule_values') IS NOT NULL"
    )))
    .fetch_one(&mut *conn)
    .await?;
    if !present {
        return Ok(layers);
    }
    let json = |text: &str, place: String| -> Result<serde_json::Value> { serde_json::from_str(text).with_context(|| place) };
    let rows: Vec<(String, String, String)> = sqlx::query_as(AssertSqlSafe(format!(
        "SELECT r.extension, r.name, r.rule::text FROM {content}.ext_rules r JOIN {content}.extensions e ON e.name = r.extension \
         WHERE e.enabled ORDER BY e.install_seq, r.name"
    )))
    .fetch_all(&mut *conn)
    .await?;
    for (extension, name, rule) in rows {
        let value = json(&rule, format!("content.ext_rules {extension}/{name}"))?;
        layers.extensions.push((extension, name, value));
    }
    let has_edits: bool =
        sqlx::query_scalar(AssertSqlSafe(format!("SELECT to_regclass('{edits}.fields') IS NOT NULL"))).fetch_one(&mut *conn).await?;
    let query = if has_edits {
        format!(
            "SELECT v.name, v.rule::text, (SELECT max(f.change_set) FROM {edits}.fields f WHERE f.target = 'rule' AND f.key = v.name) \
             FROM {content}.rule_values v ORDER BY v.name"
        )
    } else {
        format!("SELECT name, rule::text, NULL::integer FROM {content}.rule_values ORDER BY name")
    };
    let rows: Vec<(String, String, Option<i32>)> = sqlx::query_as(AssertSqlSafe(query)).fetch_all(&mut *conn).await?;
    for (name, rule, change_set) in rows {
        let value = json(&rule, format!("content.rule_values {name}"))?;
        layers.editor.push((name, value, change_set));
    }
    Ok(layers)
}

/// The config file named by `GAME_CONFIG`, or `config/game.toml`; a missing default file is fine, a missing named one is an error.
pub fn read_config_file(named: Option<&str>) -> Result<Option<FileLayer>> {
    let path = named.unwrap_or(DEFAULT_CONFIG);
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(read_file(path, &text)?)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && named.is_none() => {
            tracing::info!(path, "no config file: the defaults and the extensions' defaults are used");
            Ok(None)
        }
        Err(error) => Err(error).with_context(|| format!("reading the config file {path} (GAME_CONFIG)")),
    }
}

/// A value as TOML: a flag as `true`/`false`, a number bare, the rest as a string.
fn toml_value(setting: &Setting, value: &str) -> String {
    match setting.kind {
        Kind::Flag => flag(value).to_string(),
        Kind::Number => value.trim().to_owned(),
        Kind::Text => format!("{:?}", value.trim()),
    }
}

/// `game-server config`: the resolved settings as TOML, each with a comment naming its layer and, when it differs, the default.
pub fn show(resolved: &[Resolved]) -> String {
    let mut out = String::new();
    for r in resolved {
        let setting = setting(r.name).expect("resolved settings are registered");
        let value = toml_value(setting, &r.value);
        let default = toml_value(setting, r.default);
        let comment =
            if r.layer != Layer::Default && value != default { format!("{} (default {default})", r.layer) } else { r.layer.to_string() };
        out.push_str(&format!("{:<44} # {comment}\n", format!("{} = {value}", r.name)));
    }
    out
}

/// `game-server config --example`: every setting commented out at its default, with its one-line documentation. Committed as
/// `config/game.toml.example`; a test checks it is current.
pub fn example() -> String {
    let mut out = String::from(
        "# The game server's settings (docs/server/settings.md). Every setting is listed at its default, commented out: remove the # and\n\
         # change the value to set it. Changes take effect at the next start (tools/game-server.sh restart). With nothing set the server\n\
         # behaves as the original. What is in force and where each value comes from: target/release/game-server config.\n\
         # This file is generated by `game-server config --example`; a list may be written as an array: market_maps = [35, 36, 42].\n\
         # the rules: game-server rules\n",
    );
    for setting in SETTINGS {
        out.push_str(&format!("\n# {}\n# {} = {}\n", setting.doc, setting.name, toml_value(setting, setting.default)));
    }
    out.push_str("\n# [rules]\n");
    out.push_str(crate::rules::EXAMPLE);
    out.push('\n');
    out.push_str(crate::script::sources::EXAMPLE);
    out
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The parsers
// ---------------------------------------------------------------------------------------------------------------------------------

/// The experience factor: a number of 0 or more (`/exp2xenable` refuses less than 0).
fn exp_rate(text: &str) -> Result<f64> {
    let rate: f64 = text.trim().parse()?;
    anyhow::ensure!(rate.is_finite() && rate >= 0.0, "{rate} is not 0 or more");
    Ok(rate)
}

/// The slot machine's payout in percent of the original's, 1 to 1000.
fn slot_payout_percent(text: &str) -> Result<u32> {
    let percent: u32 = text.trim().parse()?;
    anyhow::ensure!((1..=1000).contains(&percent), "{percent} is not 1 to 1000");
    Ok(percent)
}

/// Five chances in per mille, 0 to 1000 each.
fn card_win_chances(text: &str) -> Result<[u32; 5]> {
    let numbers: Vec<u32> = text.split(',').map(|n| n.trim().parse().with_context(|| format!("number {n:?}"))).collect::<Result<_>>()?;
    let chances: [u32; 5] = numbers.try_into().map_err(|n: Vec<u32>| anyhow::anyhow!("{} numbers, not 5", n.len()))?;
    anyhow::ensure!(chances.iter().all(|&c| c <= 1000), "a chance is above 1000");
    Ok(chances)
}

/// The kill rate, 1 to 100.
fn kill_rate(text: &str) -> Result<u32> {
    let rate: u32 = text.trim().parse()?;
    anyhow::ensure!((1..=100).contains(&rate), "{rate} is not 1 to 100");
    Ok(rate)
}

/// The staff level the console is open to, 1 to 5.
fn script_memory_mb(text: &str) -> Result<u32> {
    let mb: u32 = text.trim().parse()?;
    if !(1..=256).contains(&mb) {
        anyhow::bail!("{mb} is not 1 to 256 (megabytes)");
    }
    Ok(mb)
}

fn console_level(text: &str) -> Result<u8> {
    let level: u8 = text.trim().parse()?;
    anyhow::ensure!((1..=5).contains(&level), "{level} is not 1 to 5");
    Ok(level)
}

/// A comma-separated list of kill-boost skills, each `id[/level][:times]`: `id` any level, `times` 1 to 100 and 2 when left out;
/// empty for none.
fn kill_boosts(text: &str) -> Result<Vec<KillBoost>> {
    text.split(',')
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .map(|m| {
            let (skill, times) = match m.split_once(':') {
                Some((skill, times)) => (skill, times.trim().parse().with_context(|| format!("times in {m:?}"))?),
                None => (m, 2),
            };
            anyhow::ensure!((1..=100).contains(&times), "skill {m:?}: times {times} is not 1 to 100");
            Ok(KillBoost { skill: skill_match(skill.trim()).with_context(|| format!("skill {m:?}"))?, times })
        })
        .collect()
}

/// A skill, `id` (any level) or `id/level`.
fn skill_match(m: &str) -> Result<SkillMatch> {
    let (id, level) = match m.split_once('/') {
        Some((id, level)) => (id, Some(level.trim().parse().with_context(|| format!("skill level in {m:?}"))?)),
        None => (m, None),
    };
    let id = id.trim().parse().with_context(|| format!("skill id in {m:?}"))?;
    anyhow::ensure!(id != 0 && level != Some(0), "skill {m:?}: id and level start at 1");
    Ok(SkillMatch { id, level })
}

/// A comma-separated list of map ids; empty for none.
fn maps(text: &str) -> Result<Vec<u16>> {
    text.split(',').map(str::trim).filter(|m| !m.is_empty()).map(|m| m.parse().with_context(|| format!("map id {m:?}"))).collect()
}

/// `(weekday,hour,hours)` entries, e.g. `(1,14,1)` or `(1,14,1),(3,10,2)`; empty for none.
fn ranking_times(text: &str) -> Result<Option<Vec<[u16; 3]>>> {
    let digits: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    if digits.is_empty() {
        return Ok(None);
    }
    let mut times = Vec::new();
    for entry in digits.split(')').filter(|e| !e.is_empty() && *e != ",") {
        let numbers = entry.trim_start_matches(',').strip_prefix('(').with_context(|| format!("entry {entry:?}"))?;
        let numbers: Vec<u16> = numbers.split(',').map(|n| n.parse().with_context(|| format!("number {n:?}"))).collect::<Result<_>>()?;
        let [day, hour, hours] = numbers[..] else { anyhow::bail!("entry {entry:?} needs weekday, hour, hours") };
        anyhow::ensure!(day < 7 && hour < 24 && hours > 0, "entry {entry:?} out of range");
        times.push([day, hour, hours]);
    }
    Ok(Some(times))
}

fn flag(text: &str) -> bool {
    matches!(text.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn startup() -> Startup {
        Startup { bind: "0.0.0.0:30810".parse().unwrap(), database_url: "postgres://localhost/test".into(), server_id: 1, config: None }
    }

    fn vars(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect()
    }

    fn file(text: &str) -> FileLayer {
        read_file("config/game.toml", text).unwrap()
    }

    fn file_error(text: &str) -> String {
        format!("{:#}", read_file("config/game.toml", text).unwrap_err())
    }

    fn resolved(sources: &Sources) -> Result<(Config, Vec<Resolved>)> {
        resolve(&startup(), sources)
    }

    fn value<'a>(all: &'a [Resolved], name: &str) -> &'a Resolved {
        all.iter().find(|r| r.name == name).unwrap()
    }

    #[test]
    fn every_default_resolves_to_todays_config() {
        let (config, all) = resolved(&Sources::default()).unwrap();
        // The expected values are the defaults 0.2.0 read from the environment (the table of docs/server/settings.md).
        let expected = Config {
            bind: "0.0.0.0:30810".parse().unwrap(),
            database_url: "postgres://localhost/test".into(),
            server_id: 1,
            check_ip: true,
            heartbeat_interval: Duration::from_secs(5),
            handshake_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(600),
            delete_delay: Duration::from_secs(7200),
            logout_delay: Duration::from_secs(10),
            linger: Duration::from_secs(10),
            kick_notice: Duration::from_secs(3),
            town_delay: Duration::from_secs(10),
            handshake_wait: Duration::from_secs(15),
            trade_chat_min_level: 10,
            trade_chat_cooldown: Duration::from_secs(5),
            cross_faction_trade_maps: vec![],
            market_maps: vec![],
            guild_party_size: 7,
            kill_count: true,
            ranking_times: None,
            ranking_update_delay: Duration::from_secs(1800),
            smooth_chase: true,
            move_check: true,
            item_mall: true,
            game_log: true,
            game_chat_log: true,
            game_log_days: 0,
            max_connections: 5000,
            max_per_ip: 8,
            connects_per_minute: 30,
            write_timeout: Duration::from_secs(20),
            outbox_limit: 4_194_304,
            stats_every: Duration::ZERO,
            chat_per_sec: 3,
            chat_burst: 5,
            packets_per_sec: 100,
            db_pool: 24,
            db_acquire: Duration::from_secs(10),
            exp_rate: 1.0,
            kill_rate: 1,
            kill_boost_skills: vec![],
            mini_game: true,
            slot_payout_percent: 100,
            card_win_chances: [400, 200, 100, 50, 10],
            console_level: 1,
            scripts: true,
            script_memory_mb: 16,
            config_path: None,
        };
        assert_eq!(format!("{config:?}"), format!("{expected:?}"));
        assert!(all.iter().all(|r| r.layer == Layer::Default && r.value == r.default));
        assert_eq!(all.len(), SETTINGS.len());
    }

    #[test]
    fn the_registry_has_no_duplicates_and_every_default_passes_its_parser() {
        let mut names: Vec<_> = SETTINGS.iter().map(|s| s.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), SETTINGS.len());
        for setting in SETTINGS {
            assert!(check_setting(setting.name, setting.default).is_ok(), "{}", setting.name);
            assert!(!setting.doc.is_empty() && !setting.doc.contains('\n'), "{}", setting.name);
        }
    }

    #[test]
    fn each_layer_beats_the_one_below() {
        let sources = Sources {
            extensions: vec![
                ("kill-boost".into(), "kill_boost_skills".into(), "200/1:2".into()),
                ("kill-boost".into(), "guild_party_size".into(), "3".into()),
            ],
            file: Some(file("guild_party_size = 2\nmarket_maps = [35, 36]\n")),
            editor: vec![("market_maps".into(), "42".into(), Some(12))],
        };
        let (config, all) = resolved(&sources).unwrap();
        assert_eq!(value(&all, "kill_boost_skills").layer, Layer::Extension("kill-boost".into()));
        assert_eq!(config.kill_boost_skills.len(), 1);
        assert_eq!(
            (value(&all, "guild_party_size").layer.clone(), config.guild_party_size),
            (Layer::File, 2),
            "the file beats the extension"
        );
        assert_eq!(
            (value(&all, "market_maps").layer.clone(), config.market_maps.clone()),
            (Layer::Editor(Some(12)), vec![42]),
            "the editor beats the file"
        );
        assert_eq!(value(&all, "trade_chat_min_level").layer, Layer::Default);
    }

    #[test]
    fn a_disabled_extensions_default_does_not_count() {
        // The database layer reads enabled extensions only; resolve takes what it is given, so the proof is the query's WHERE e.enabled.
        // Here: with the extension's rows left out, the default stands.
        let (config, all) = resolved(&Sources::default()).unwrap();
        assert!(config.kill_boost_skills.is_empty());
        assert_eq!(value(&all, "kill_boost_skills").layer, Layer::Default);
    }

    #[test]
    fn an_array_and_a_string_give_the_same_value() {
        let a = file(
            "market_maps = [35, 36, 42]\ncard_win_chances = [420, 200, 130, 100, 80]\nexp_rate = 1.5\nmini_game = false\nranking_times = \"(1,14,1)\"\n",
        );
        let b = file(
            "market_maps = \"35,36,42\"\ncard_win_chances = \"420,200,130,100,80\"\nexp_rate = \"1.5\"\nmini_game = \"false\"\nranking_times = [\"(1,14,1)\"]\n",
        );
        let texts = |f: &FileLayer| f.values.iter().map(|(n, v, _)| (n.clone(), v.clone())).collect::<Vec<_>>();
        assert_eq!(texts(&a), texts(&b));
        let (config, _) = resolved(&Sources { file: Some(a), ..Sources::default() }).unwrap();
        assert_eq!(
            (config.market_maps.clone(), config.card_win_chances, config.exp_rate, config.mini_game),
            (vec![35, 36, 42], [420, 200, 130, 100, 80], 1.5, false)
        );
        assert_eq!(config.ranking_times, Some(vec![[1, 14, 1]]));
    }

    #[test]
    fn errors_name_the_line_and_the_layer() {
        let message = file_error("# comment\nguild_party_size = 2\n\ntrade_chat_minlevel = 10\n");
        assert_eq!(message, "config/game.toml line 4: unknown setting \"trade_chat_minlevel\"");
        let message = file_error("a = \n");
        assert!(message.starts_with("config/game.toml line 1:"), "{message}");
        let message = file_error("guild_party_size = 2\n\n[ext.Welcome]\nwelcome_text = \"x\"\n");
        assert!(message.contains("line 3") && message.contains("not an extension name"), "{message}");
        let message = file_error("# 1\n# 2\n\n\n\n\n\n\n[rules]\nx = 1\n");
        assert!(message.contains("line 10") && message.contains("rules.x: a rule is a table"), "{message}");
        let message = file_error("[market]\nx = 1\n");
        assert!(message.contains("line 1") && message.contains("unknown setting \"market\""), "{message}");
        let message = file_error("market_maps = { a = 1 }\n");
        assert!(message.contains("line 1") && message.contains("cannot be a setting"), "{message}");
        let message = file_error("market_maps = [[1], [2]]\n");
        assert!(message.contains("a list holds integers or strings"), "{message}");

        let bad = file("guild_party_size = 2\n\nmarket_maps = \"x\"\n");
        let message = format!("{:#}", resolved(&Sources { file: Some(bad), ..Sources::default() }).unwrap_err());
        assert!(message.starts_with("config/game.toml line 3: market_maps: map id \"x\""), "{message}");
        let extension =
            Sources { extensions: vec![("kill-boost".into(), "kill_boost_skills".into(), "200/1:101".into())], ..Sources::default() };
        let message = format!("{:#}", resolved(&extension).unwrap_err());
        assert!(message.starts_with("extension kill-boost: kill_boost_skills: "), "{message}");
        let editor = Sources { editor: vec![("slot_payout_percent".into(), "0".into(), Some(12))], ..Sources::default() };
        let message = format!("{:#}", resolved(&editor).unwrap_err());
        assert!(message.starts_with("data editor (change set 12): slot_payout_percent: "), "{message}");
        let unknown = Sources { editor: vec![("nothing".into(), "1".into(), None)], ..Sources::default() };
        assert!(format!("{:#}", resolved(&unknown).unwrap_err()).contains("data editor: \"nothing\" is not a game setting"));
    }

    #[test]
    fn a_game_variable_stops_the_start_with_the_line_to_paste() {
        let ok = Startup::from_vars(&vars(&[
            ("DATABASE_URL", "postgres://x/y"),
            ("GAME_BIND", "0.0.0.0:31810"),
            ("GAME_SERVER_ID", "3"),
            ("GAME_CONFIG", "/etc/g.toml"),
            ("RUST_LOG", "debug"),
        ]))
        .unwrap();
        assert_eq!((ok.bind.port(), ok.server_id, ok.config.as_deref()), (31810, 3, Some("/etc/g.toml")));
        let refused = format!(
            "{:#}",
            Startup::from_vars(&vars(&[
                ("DATABASE_URL", "x"),
                ("GAME_MARKET_MAPS", "35,36,42"),
                ("GAME_GUILD_PARTY_SIZE", "2"),
                ("GAME_MINI_GAME", "true")
            ]))
            .unwrap_err()
        );
        assert!(refused.contains("config/game.toml now, not from the environment"), "{refused}");
        assert!(refused.contains("guild_party_size = \"2\"") && refused.contains("# was GAME_GUILD_PARTY_SIZE"), "{refused}");
        assert!(refused.contains("market_maps = \"35,36,42\"") && refused.contains("# was GAME_MARKET_MAPS"), "{refused}");
        assert!(refused.contains("mini_game = \"true\""), "{refused}");
        let typo = format!("{:#}", Startup::from_vars(&vars(&[("DATABASE_URL", "x"), ("GAME_MARKET_MAP", "35")])).unwrap_err());
        assert!(typo.contains("GAME_MARKET_MAP is not a setting"), "{typo}");
        assert!(Startup::from_vars(&[]).is_err(), "DATABASE_URL is required");
        assert!(
            format!("{:#}", Startup::from_vars(&vars(&[("DATABASE_URL", "x"), ("GAME_BIND", "nowhere")])).unwrap_err())
                .contains("GAME_BIND")
        );
    }

    #[test]
    fn the_resolved_settings_print_with_their_layers() {
        let sources = Sources {
            extensions: vec![("kill-boost".into(), "kill_boost_skills".into(), "200/1:2,200/2:5,200/3:10".into())],
            file: Some(file("guild_party_size = 2\nmini_game = true\n")),
            editor: vec![],
        };
        let (_, all) = resolved(&sources).unwrap();
        let text = show(&all);
        let line = |name: &str| text.lines().find(|l| l.starts_with(&format!("{name} = "))).unwrap().to_owned();
        assert!(line("guild_party_size").ends_with("# config file (default 7)"), "{}", line("guild_party_size"));
        assert!(
            line("kill_boost_skills").starts_with("kill_boost_skills = \"200/1:2,200/2:5,200/3:10\"")
                && line("kill_boost_skills").ends_with("# extension kill-boost (default \"\")")
        );
        assert!(line("trade_chat_min_level").ends_with("# default") && line("trade_chat_min_level").contains("= 10"));
        assert!(line("mini_game").ends_with("# config file"), "set to its default: no default part: {}", line("mini_game"));
        // What `show` prints reads back as the same values.
        let again = read_file("x", &text).unwrap();
        assert_eq!(again.values.len(), SETTINGS.len());
        let (_, back) = resolved(&Sources { file: Some(again), ..Sources::default() }).unwrap();
        for (a, b) in all.iter().zip(&back) {
            assert_eq!((a.name, toml_value(setting(a.name).unwrap(), &a.value)), (b.name, toml_value(setting(b.name).unwrap(), &b.value)));
        }
    }

    #[test]
    fn the_example_is_the_committed_file_and_reads_as_nothing_set() {
        let text = example();
        assert!(read_file("example", &text).unwrap().values.is_empty(), "every setting is commented out");
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/game.toml.example");
        let committed = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{}: {e}; write it with: game-server config --example > config/game.toml.example", path.display()));
        assert_eq!(committed, text, "config/game.toml.example is out of date: game-server config --example > config/game.toml.example");
        // Uncommenting a setting line gives a valid file.
        let is_setting =
            |line: &str| line.strip_prefix("# ").and_then(|r| r.split_once(" = ")).is_some_and(|(name, _)| setting(name).is_some());
        let uncommented: String = text.lines().map(|l| if is_setting(l) { &l[2..] } else { l }).collect::<Vec<_>>().join("\n");
        let all = read_file("example", &uncommented).unwrap();
        assert_eq!(all.values.len(), SETTINGS.len());
        assert!(resolved(&Sources { file: Some(all), ..Sources::default() }).is_ok());
    }

    const RULES_FILE: &str = "# comment
guild_party_size = 2

[rules.market-no-pvp-gain]
type = \"pvp_gain\"
maps = [35, 36, 42]
exp = 0
kills = 0
deaths = false

[rules.\"pets/no-capture\"]
enabled = false

[rules.level-gap]
type = \"level_gap\"
gap = 20
maps = { not = [42] }
message = \"Too far apart.\"
";

    #[test]
    fn rules_in_the_file_are_read_with_their_lines() {
        let layer = file(RULES_FILE);
        assert_eq!(layer.values.len(), 1, "the setting is still a setting");
        let names: Vec<_> = layer.rules.iter().map(|r| (r.name.as_str(), r.line)).collect();
        assert_eq!(
            names,
            [("level-gap", 14), ("market-no-pvp-gain", 4), ("pets/no-capture", 11)],
            "the order of the table; each at its header's line"
        );
        let gap = layer.rules.iter().find(|r| r.name == "level-gap").unwrap();
        assert_eq!(gap.value, serde_json::json!({ "type": "level_gap", "gap": 20, "maps": { "not": [42] }, "message": "Too far apart." }));
        assert_eq!(gap.key_lines, [("gap".to_owned(), 16), ("maps".to_owned(), 17), ("message".to_owned(), 18), ("type".to_owned(), 15)]);
        let market = layer.rules.iter().find(|r| r.name == "market-no-pvp-gain").unwrap();
        assert_eq!(market.value["exp"], 0);
        assert_eq!((market.value["deaths"].as_bool(), market.value["maps"][1].as_u64()), (Some(false), Some(36)));
        let empty = file("[rules]\n");
        assert!(empty.rules.is_empty());
    }

    fn rules_error(text: &str) -> String {
        let layer = file(text);
        let (config, _) = resolved(&Sources { file: Some(layer.clone()), ..Sources::default() }).unwrap();
        let sources =
            RuleSources { file: Some(crate::rules::RuleFile { path: layer.path.clone(), rules: layer.rules }), ..RuleSources::default() };
        format!("{:#}", crate::rules::resolve(&config, &sources).unwrap_err())
    }

    #[test]
    fn rule_errors_name_the_line_and_the_key() {
        let text = "[rules.level-gap]\ntype = \"level_gap\"\n\n\ngap = 0\n";
        assert_eq!(rules_error(text), "config/game.toml line 5: rules.level-gap.gap: 0 is not 1 to 255");
        let text = "\n\n[rules.safe-evenings]\ntype = \"deny\"\npoint = \"attack\"\nhour = 3\n";
        assert_eq!(rules_error(text), "config/game.toml line 6: rules.safe-evenings: unknown key \"hour\"");
        assert_eq!(rules_error("\n\n\n[rules.x]\nmaps = [1]\n"), "config/game.toml line 4: rules.x: a rule needs a type");
        let message = file_error("[rules]\nx = 1\n");
        assert_eq!(message, "config/game.toml line 2: rules.x: a rule is a table, [rules.x]");
        let message = file_error("rules = 3\n");
        assert!(message.starts_with("config/game.toml line 1: rules:"), "{message}");
        let message = file_error("[rules.x]\nwhen = 1979-05-27T07:32:00Z\n");
        assert!(message.contains("line 1") && message.contains("a date cannot be in a rule"), "{message}");
        let message = file_error("[rules.x]\nfactor = nan\n");
        assert!(message.contains("line 1") && message.contains("is not a number a rule can hold"), "{message}");
        let message = file_error("guild_party_size = 2\n[ext.welcome]\nx = 1\n[ext.welcome.deeper]\ny = 2\n");
        assert!(
            message.contains("line 4") && message.contains("ext.welcome.deeper: an option is a value, not a table"),
            "nested tables are refused: {message}"
        );
        let message = file_error("ext = 3\n");
        assert!(message.starts_with("config/game.toml line 1: ext:") && message.contains("[ext.<name>]"), "{message}");
        let message = file_error("[ext.welcome]\nwhen = 1979-05-27T07:32:00Z\n");
        assert!(message.contains("line 2") && message.contains("ext.welcome.when: a date cannot be in a rule"), "{message}");
    }

    #[test]
    fn the_examples_rules_are_valid_when_uncommented() {
        let rules_part = &example()[example().find("# [rules]\n").unwrap()..];
        let uncommented: String = rules_part
            .lines()
            .skip(1)
            .filter(|l| {
                l.starts_with("# [rules.")
                    || l.starts_with("# type")
                    || l.starts_with("# maps")
                    || l.starts_with("# level =")
                    || l.starts_with("# exp")
                    || l.starts_with("# kills")
                    || l.starts_with("# deaths")
                    || l.starts_with("# gap")
                    || l.starts_with("# jumps")
                    || l.starts_with("# items")
                    || l.starts_with("# point")
                    || l.starts_with("# days")
                    || l.starts_with("# hours")
                    || l.starts_with("# message")
            })
            .map(|l| format!("{}\n", &l[2..]))
            .collect();
        let layer = read_file("example", &uncommented).unwrap();
        assert_eq!(layer.rules.len(), 7);
        let (config, _) = resolved(&Sources::default()).unwrap();
        let sources =
            RuleSources { file: Some(crate::rules::RuleFile { path: "example".into(), rules: layer.rules }), ..RuleSources::default() };
        assert_eq!(crate::rules::resolve(&config, &sources).unwrap().0.len(), 8, "the seven and the trade-chat level's");
    }

    #[test]
    fn check_file_gives_a_summary_or_the_first_error() {
        let dir = std::env::temp_dir().join(format!("afterglow-rules-check-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let good = dir.join("good.toml");
        std::fs::write(&good, RULES_FILE.replace("[rules.\"pets/no-capture\"]\nenabled = false\n\n", "")).unwrap();
        let path = good.to_str().unwrap();
        assert_eq!(crate::rules::check_file(path).unwrap(), format!("{path}: 2 rules, no errors"));
        std::fs::write(&good, "[rules.x]\ntype = \"exp\"\nfactor = 2\n").unwrap();
        assert_eq!(crate::rules::check_file(path).unwrap(), format!("{path}: 1 rule, no errors"));
        std::fs::write(&good, "guild_party_size = 2\n[rules.x]\ntype = \"exp\"\nfactor = 200\n").unwrap();
        assert_eq!(
            format!("{:#}", crate::rules::check_file(path).unwrap_err()),
            format!("{path} line 4: rules.x.factor: 200 is not 0 to 100")
        );
        std::fs::write(&good, "market_maps = \"x\"\n").unwrap();
        assert!(format!("{:#}", crate::rules::check_file(path).unwrap_err()).contains("market_maps"), "the settings are checked too");
        assert!(crate::rules::check_file(dir.join("nothing.toml").to_str().unwrap()).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn check_setting_takes_the_servers_own_parsers() {
        assert!(check_setting("kill_boost_skills", "200/1:2,200/2:5,200/3:10").is_ok());
        assert!(check_setting("kill_boost_skills", "200/1:101").is_err() && check_setting("kill_boost_skills", "x").is_err());
        assert!(check_setting("market_maps", "35,36,42").is_ok() && check_setting("market_maps", "35,x").is_err());
        assert!(check_setting("guild_party_size", "2").is_ok() && check_setting("guild_party_size", "two").is_err());
        assert!(check_setting("slot_payout_percent", "220").is_ok() && check_setting("slot_payout_percent", "0").is_err());
        assert!(check_setting("card_win_chances", "420,200,130,100,80").is_ok() && check_setting("card_win_chances", "1,2").is_err());
        assert!(check_setting("mini_game", "true").is_ok() && check_setting("exp_rate", "-1").is_err());
        assert!(check_setting("trade_chat_min_level", "10").is_ok() && check_setting("ranking_times", "(1,14,1)").is_ok());
        assert!(check_setting("trade_chat_min_level", "70000").is_err(), "a u16");
        for start_up in ["database_url", "bind", "server_id", "config", "kill_boost", ""] {
            let message = format!("{:#}", check_setting(start_up, "1").unwrap_err());
            assert!(message.contains("not a game setting"), "{start_up:?}: {message}");
        }
    }

    #[test]
    fn stats_every_is_off_by_default_and_reads_seconds() {
        let (config, all) = resolved(&Sources::default()).unwrap();
        assert_eq!(config.stats_every, Duration::ZERO);
        assert_eq!(value(&all, "stats_every").default, "0");
        let sources = Sources { file: Some(file("stats_every = 10\n")), ..Sources::default() };
        let (config, all) = resolved(&sources).unwrap();
        assert_eq!((config.stats_every, value(&all, "stats_every").layer.clone()), (Duration::from_secs(10), Layer::File));
        assert!(check_setting("stats_every", "5").is_ok() && check_setting("stats_every", "-1").is_err());
        assert!(check_setting("stats_every", "often").is_err() && check_setting("stats_every", "1.5").is_err());
    }

    #[test]
    fn ranking_times_parse() {
        assert_eq!(ranking_times("").unwrap(), None);
        assert_eq!(ranking_times("(1,14,1)").unwrap(), Some(vec![[1, 14, 1]]));
        assert_eq!(ranking_times(" (1, 14, 1) , (3,10,2) ").unwrap(), Some(vec![[1, 14, 1], [3, 10, 2]]));
        assert!(ranking_times("(1,14)").is_err() && ranking_times("(7,1,1)").is_err() && ranking_times("1,14,1").is_err());
    }

    #[test]
    fn the_gambling_settings_are_range_checked() {
        assert_eq!(slot_payout_percent("220").unwrap(), 220);
        assert!(slot_payout_percent("0").is_err() && slot_payout_percent("1001").is_err() && slot_payout_percent("x").is_err());
        assert_eq!(card_win_chances("420,200,130,100,80").unwrap(), [420, 200, 130, 100, 80]);
        assert_eq!(card_win_chances(" 400, 200 ,100,50,10 ").unwrap(), [400, 200, 100, 50, 10]);
        assert!(
            card_win_chances("1,2,3,4").is_err() && card_win_chances("1,2,3,4,5,6").is_err() && card_win_chances("1,2,3,4,1001").is_err()
        );
        assert!(card_win_chances("1,2,3,4,x").is_err() && card_win_chances("").is_err());
    }

    #[test]
    fn the_console_settings_are_range_checked() {
        assert_eq!(exp_rate("1.5").unwrap(), 1.5);
        assert!(exp_rate("0").is_ok() && exp_rate("-1").is_err() && exp_rate("nan").is_err() && exp_rate("x").is_err());
        assert!(kill_rate("1").is_ok() && kill_rate("100").is_ok() && kill_rate("0").is_err() && kill_rate("101").is_err());
        assert!(console_level("1").is_ok() && console_level("5").is_ok() && console_level("0").is_err() && console_level("6").is_err());
    }

    #[test]
    fn the_kill_boost_entries_are_id_level_and_times() {
        assert_eq!(kill_boosts("").unwrap(), []);
        let any = |id| SkillMatch { id, level: None };
        let at = |id, level| SkillMatch { id, level: Some(level) };
        let boost = |skill, times| KillBoost { skill, times };
        let default = kill_boosts("200/1:2,200/2:5,200/3:10").unwrap();
        assert_eq!(default, [boost(at(200, 1), 2), boost(at(200, 2), 5), boost(at(200, 3), 10)]);
        assert_eq!(kill_boosts(" 200 ").unwrap(), [boost(any(200), 2)], "times left out is 2, any level");
        assert_eq!(kill_boosts("200/2:5").unwrap(), [boost(at(200, 2), 5)]);
        assert_eq!(kill_boosts(" 310:3, 311 ").unwrap(), [boost(any(310), 3), boost(any(311), 2)]);
        assert_eq!(kill_boosts("200/1:1,200:100").unwrap(), [boost(at(200, 1), 1), boost(any(200), 100)]);
        for bad in ["300,x", "200/x", "200/", "0", "200/0", "200:0", "200:101", "200:x", "200:", "200/1:2:3"] {
            assert!(kill_boosts(bad).is_err(), "{bad:?}");
        }
        let error = format!("{:#}", kill_boosts("200/1:2,200/2:101").unwrap_err());
        assert!(error.contains("200/2:101"), "{error}");
    }
}
