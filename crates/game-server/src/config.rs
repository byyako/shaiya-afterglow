use std::net::SocketAddr;
use std::time::Duration;

use anyhow::{Context, Result};

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
}

impl Config {
    /// Reads `GAME_*` environment variables; what is not set has the original's behaviour (`docs/server/deviations.md` section 1).
    pub fn from_env() -> Result<Self> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// As [`Config::from_env`], with the variables from `lookup`.
    fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self> {
        let var = |name: &str, default: &str| lookup(name).unwrap_or_else(|| default.to_owned());
        let secs = |name: &str, default: &str| -> Result<Duration> {
            Ok(Duration::from_secs(var(name, default).parse().with_context(|| name.to_owned())?))
        };
        Ok(Self {
            bind: var("GAME_BIND", "0.0.0.0:30810").parse().context("GAME_BIND")?,
            database_url: lookup("DATABASE_URL").context("DATABASE_URL is required")?,
            server_id: var("GAME_SERVER_ID", "1").parse().context("GAME_SERVER_ID")?,
            check_ip: flag(&var("GAME_CHECK_IP", "true")),
            heartbeat_interval: secs("GAME_HEARTBEAT_SECS", "5")?,
            handshake_timeout: secs("GAME_HANDSHAKE_SECS", "30")?,
            idle_timeout: secs("GAME_IDLE_SECS", "600")?,
            delete_delay: secs("GAME_DELETE_DELAY_SECS", "7200")?,
            logout_delay: secs("GAME_LOGOUT_SECS", "10")?,
            linger: secs("GAME_LINGER_SECS", "10")?,
            kick_notice: secs("GAME_KICK_NOTICE_SECS", "3")?,
            town_delay: secs("GAME_TOWN_SECS", "10")?,
            handshake_wait: secs("GAME_HANDSHAKE_WAIT_SECS", "15")?,
            trade_chat_min_level: var("GAME_TRADE_CHAT_MIN_LEVEL", "10").parse().context("GAME_TRADE_CHAT_MIN_LEVEL")?,
            trade_chat_cooldown: secs("GAME_TRADE_CHAT_COOLDOWN_SECS", "5")?,
            cross_faction_trade_maps: maps(&var("GAME_CROSS_FACTION_TRADE_MAPS", "")).context("GAME_CROSS_FACTION_TRADE_MAPS")?,
            market_maps: maps(&var("GAME_MARKET_MAPS", "")).context("GAME_MARKET_MAPS")?,
            guild_party_size: var("GAME_GUILD_PARTY_SIZE", "7").parse().context("GAME_GUILD_PARTY_SIZE")?,
            kill_count: flag(&var("GAME_KILL_COUNT", "true")),
            ranking_times: ranking_times(&var("GAME_RANKING_TIMES", "")).context("GAME_RANKING_TIMES")?,
            ranking_update_delay: secs("GAME_RANKING_UPDATE_DELAY_SECS", "1800")?,
            smooth_chase: flag(&var("GAME_SMOOTH_CHASE", "true")),
            move_check: flag(&var("GAME_MOVE_CHECK", "true")),
            item_mall: flag(&var("GAME_ITEM_MALL", "true")),
            game_log: flag(&var("GAME_LOG", "true")),
            game_chat_log: flag(&var("GAME_CHAT_LOG", "true")),
            game_log_days: var("GAME_LOG_DAYS", "0").parse().context("GAME_LOG_DAYS")?,
            max_connections: var("GAME_MAX_CONNECTIONS", "5000").parse().context("GAME_MAX_CONNECTIONS")?,
            max_per_ip: var("GAME_MAX_PER_IP", "8").parse().context("GAME_MAX_PER_IP")?,
            connects_per_minute: var("GAME_CONNECTS_PER_MINUTE", "30").parse().context("GAME_CONNECTS_PER_MINUTE")?,
            write_timeout: secs("GAME_WRITE_TIMEOUT_SECS", "20")?,
            outbox_limit: var("GAME_OUTBOX_LIMIT", "4194304").parse().context("GAME_OUTBOX_LIMIT")?,
            chat_per_sec: var("GAME_CHAT_PER_SEC", "3").parse().context("GAME_CHAT_PER_SEC")?,
            chat_burst: var("GAME_CHAT_BURST", "5").parse().context("GAME_CHAT_BURST")?,
            packets_per_sec: var("GAME_PACKETS_PER_SEC", "100").parse().context("GAME_PACKETS_PER_SEC")?,
            db_pool: var("GAME_DB_POOL", "24").parse().context("GAME_DB_POOL")?,
            db_acquire: secs("GAME_DB_ACQUIRE_SECS", "10")?,
            exp_rate: exp_rate(&var("GAME_EXP_RATE", "1")).context("GAME_EXP_RATE")?,
            kill_rate: kill_rate(&var("GAME_KILL_RATE", "1")).context("GAME_KILL_RATE")?,
            kill_boost_skills: kill_boosts(&var("GAME_KILL_BOOST_SKILLS", "")).context("GAME_KILL_BOOST_SKILLS")?,
            mini_game: flag(&var("GAME_MINI_GAME", "true")),
            slot_payout_percent: slot_payout_percent(&var("GAME_SLOT_PAYOUT_PERCENT", "100")).context("GAME_SLOT_PAYOUT_PERCENT")?,
            card_win_chances: card_win_chances(&var("GAME_CARD_WIN_CHANCES", "400,200,100,50,10")).context("GAME_CARD_WIN_CHANCES")?,
            console_level: console_level(&var("GAME_CONSOLE_LEVEL", "1")).context("GAME_CONSOLE_LEVEL")?,
        })
    }
}

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

    #[test]
    fn the_defaults_are_the_original() {
        let config = Config::from_lookup(|name| (name == "DATABASE_URL").then(|| "postgres://localhost/test".to_owned())).unwrap();
        assert!(config.mini_game, "the gambling house is on by default, with the original's odds below");
        assert_eq!(config.slot_payout_percent, 100);
        assert_eq!(config.card_win_chances, [400, 200, 100, 50, 10]);
        assert!(config.cross_faction_trade_maps.is_empty());
        assert!(config.market_maps.is_empty(), "empty: a shop on every map");
        assert!(config.kill_boost_skills.is_empty());
        assert!(Config::from_lookup(|_| None).is_err(), "DATABASE_URL is required");
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
