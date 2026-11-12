//! `bot swarm` (sections 3.1, 5.3 and 6 of `docs/plans/test-bots.md`): N bots, each a tokio task, logged in at `--ramp` a second, playing
//! the `pvp-battle` loop until the run's duration or Ctrl+C, then all logged out; the figures go into 10-second windows and
//! `report.json` (`crate::report`).
//!
//! What the bots share is lock-free: a bot hands its counters to the aggregator over an unbounded channel once a window (and when it
//! logs in or out), and the step echo's board is a row of atomics (`crate::echo`). No bot waits on another in its packet path.

use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use game_server::backend::{CharacterStore, ItemUnit};
use game_server::inventory::{Inventory, ItemDefs, real_type};
use game_server::pg::PgBackend;
use game_server::skills::{self, ATTACK, Aim, Skill, SkillDef, SkillDefs};
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinSet;
use tokio::time::sleep_until;

use crate::bot::Bot;
use crate::echo::{Echo, EchoBoard};
use crate::enter::connect_game;
use crate::link::LinkError;
use crate::login::login_and_select;
use crate::report::{Collector, Report, RunInfo};
use crate::scenario::pvp_battle::{Battle, Config, REACH_MARGIN, REACH_PLUS, SkillRule, run as play};
use crate::seed::{self, Defs, Refused};

/// The window the figures are kept in (section 6).
pub const WINDOW: Duration = Duration::from_secs(10);
/// How long a bot waits to be logged out before it closes anyway: the server's `logout_secs` (10 by default) and the retries of a bot
/// that is struck while it waits (`Bot::leave`).
pub const LOGOUT_WAIT: Duration = Duration::from_secs(30);
/// The most bots one swarm runs (as `bot seed`): a larger battle is several `--first` ranges.
pub const MAX_COUNT: u32 = seed::MAX_COUNT;
/// The longest account number: the account is the prefix and four digits.
const MAX_NUMBER: u32 = 9999;
/// The weapon slot and the shield slot of bag 0.
const WEAPON_SLOT: u8 = 5;
const SHIELD_SLOT: u8 = 6;

/// What `bot swarm` is asked for.
#[derive(Clone, Debug)]
pub struct Options {
    /// The login server, `host:30800`.
    pub login: String,
    pub count: u32,
    pub first: u32,
    pub prefix: String,
    /// Bots logged in a second.
    pub ramp: f64,
    pub scenario: String,
    /// The battle point as `map, x, z`; the midpoint of the factions' rebirth boxes when `None`.
    pub at: Option<(u16, f32, f32)>,
    pub radius: f32,
    pub duration: Duration,
    pub steps_per_sec: f32,
    pub skill_every: Duration,
    pub rebirth_after: Duration,
    /// The share of the bots that stand at the point and only read.
    pub idle_share: f32,
    pub report: PathBuf,
    /// The game server's number in the login server's list (as `bot seed --server`).
    pub server: u8,
    /// Every bot account's password (`SHAIYA_PASSWORD`).
    pub password: String,
    /// The window of the report (not a command-line flag; the tests shorten it).
    pub window: Duration,
    pub logout_wait: Duration,
    /// Replaces the port of the game server's address the login server returns (not a flag: the real client's port is fixed).
    pub game_port: Option<u16>,
}

impl Options {
    pub fn new(login: &str, count: u32, password: &str) -> Self {
        Self {
            login: login.into(),
            count,
            first: 1,
            prefix: "bot".into(),
            ramp: 5.0,
            scenario: "pvp-battle".into(),
            at: None,
            radius: 60.0,
            duration: Duration::from_secs(600),
            steps_per_sec: 2.0,
            skill_every: Duration::from_secs(6),
            rebirth_after: Duration::from_secs(5),
            idle_share: 0.0,
            report: PathBuf::from("report.json"),
            server: 1,
            password: password.into(),
            window: WINDOW,
            logout_wait: LOGOUT_WAIT,
            game_port: None,
        }
    }

    /// The run's arguments as the report keeps them (the password is not one), with where the battle is.
    pub fn args_json(&self, map: u16, point: [f32; 2]) -> Value {
        json!({
            "login": self.login, "count": self.count, "first": self.first, "prefix": self.prefix, "ramp": self.ramp,
            "scenario": self.scenario, "at": {"map": map, "x": point[0], "z": point[1], "given": self.at.is_some()},
            "radius": self.radius, "duration_s": self.duration.as_secs_f64(), "steps_per_sec": self.steps_per_sec,
            "skill_every_s": self.skill_every.as_secs_f64(), "rebirth_after_s": self.rebirth_after.as_secs_f64(),
            "idle_share": self.idle_share, "report": self.report, "server": self.server,
        })
    }
}

/// One bot of the swarm: who it is and the `Config` of its battle loop, filled from the database.
#[derive(Clone, Debug)]
pub struct BotPlan {
    pub number: u32,
    pub username: String,
    pub character_id: u32,
    pub config: Config,
}

/// Whether the bot at `index` of `count` is one of the idle share: the idle ones are spread evenly through the numbers.
pub fn idle_for(index: u32, share: f32) -> bool {
    let share = f64::from(share.clamp(0.0, 1.0));
    ((f64::from(index) + 1.0) * share).floor() > (f64::from(index) * share).floor()
}

/// The attack skills a bot may use on an enemy player, in its own order: an attack on one named target (`skills::aim`), built, whose
/// weapon (or shield) the bot wears, with no skill needed before it, and not one that gives up the caster's life (`DESPERATE`: a bot
/// that fights on is the load to measure, and a death by its own skill is nobody's kill). The reach is the bot's own (`weapon_range`, which includes the
/// passives and accessories) plus the skill's, plus the server's 2.3, less the same margin the bot keeps for its blows. Each rule
/// carries the facts of the definition at the level the bot has the skill (`SkillDefs::get(id, level)`).
pub fn skill_rules(items: &ItemDefs, defs: &SkillDefs, worn: &[ItemUnit], known: &[Skill], weapon_range: i32) -> Vec<SkillRule> {
    let inventory = Inventory::new(worn);
    let weapon = inventory.get(0, WEAPON_SLOT).and_then(|i| items.get(i.item_type, i.type_id)).map(|d| real_type(d.item_type));
    let shield = inventory.get(0, SHIELD_SLOT).is_some();
    let equipped = |d: &SkillDef| {
        if d.need_shield {
            shield
        } else {
            weapon.is_some_and(|t| t >= 1 && d.need_weapons.get(usize::from(t) - 1).copied().unwrap_or(false))
        }
    };
    known
        .iter()
        .filter_map(|s| {
            let d = defs.get(s.id, s.level)?;
            (d.type_effect == ATTACK
                && skills::aim(d) == Aim::Named
                && !d.not_built()
                && d.kind != skills::DESPERATE
                && d.prev_skill == 0
                && equipped(d))
            .then(|| SkillRule {
                skill_id: s.id,
                level: s.level,
                attack_range: (weapon_range + i32::from(d.attack_range)) as f32 + REACH_PLUS - REACH_MARGIN,
                sp: d.need_sp,
                mp: d.need_mp,
                reset: Duration::from_secs(u64::from(d.reset_time)),
            })
        })
        .collect()
}

/// The plans, and where the battle is.
pub struct Setup {
    pub plans: Vec<BotPlan>,
    pub map: u16,
    pub point: [f32; 2],
}

/// The midpoint of the two factions' rebirth boxes on `map`: the mean of each faction's box centres, then the middle of the two.
pub async fn default_point(pool: &PgPool, map: u16) -> Result<[f32; 2]> {
    let mut centres = Vec::new();
    for country in 0..2 {
        let boxes = seed::load_boxes(pool, map, country).await?;
        if boxes.is_empty() {
            return Err(Refused(format!("map {map} has no rebirth box for faction {country}: give the battle point with --at")).into());
        }
        let n = boxes.len() as f32;
        centres
            .push([boxes.iter().map(|b| (b[0] + b[3]) / 2.0).sum::<f32>() / n, boxes.iter().map(|b| (b[2] + b[5]) / 2.0).sum::<f32>() / n]);
    }
    Ok([(centres[0][0] + centres[1][0]) / 2.0, (centres[0][1] + centres[1][1]) / 2.0])
}

/// Reads every bot's character, items and skills from the database (as the game server loads them) and makes its plan. The bots are
/// `<prefix>NNNN` for the numbers `first` to `first + count - 1`; one that does not exist refuses the run (`bot seed` first).
pub async fn prepare(pool: &PgPool, defs: &Defs, options: &Options) -> Result<Setup> {
    let numbers: Vec<u32> = (options.first..options.first + options.count).collect();
    let names: Vec<String> = numbers.iter().map(|&n| seed::bot_names(&options.prefix, n).0).collect();
    let rows: Vec<(String, i32, i32)> = sqlx::query_as(
        "SELECT a.username, a.id, c.id FROM accounts a JOIN characters c ON c.user_id = a.id
          WHERE a.username = ANY($1) AND c.server_id = $2 AND c.deleted_at IS NULL AND c.delete_requested_at IS NULL
          ORDER BY a.username, c.slot",
    )
    .bind(&names)
    .bind(i16::from(options.server))
    .fetch_all(pool)
    .await
    .context("looking up the bot accounts")?;
    // The first character of each account: the one the bot plays.
    let mut first_of: HashMap<String, (u32, u32)> = HashMap::new();
    for (name, user, character) in rows {
        first_of.entry(name).or_insert((u32::try_from(user)?, u32::try_from(character)?));
    }
    let missing: Vec<&String> = names.iter().filter(|n| !first_of.contains_key(*n)).collect();
    if !missing.is_empty() {
        return Err(Refused(format!(
            "{} bot account(s) have no character on server {} (first: {}): run bot seed with the same prefix and server first",
            missing.len(),
            options.server,
            missing[0]
        ))
        .into());
    }
    let store = PgBackend::new(pool.clone(), Duration::from_secs(7200));
    let mut loaded = Vec::new();
    for (&number, name) in numbers.iter().zip(&names) {
        let (user, character) = first_of[name];
        let play = store
            .load(user, options.server, character)
            .await
            .with_context(|| format!("loading {name}"))?
            .with_context(|| format!("{name}: the game server's store has no character {character}"))?;
        loaded.push((number, name.clone(), play));
    }
    let map = loaded[0].2.map;
    if let Some(&(at_map, ..)) = options.at.as_ref()
        && let Some((number, name, play)) = loaded.iter().find(|(_, _, p)| p.map != at_map)
    {
        return Err(Refused(format!(
            "--at is on map {at_map} but {name} (bot {number}) stands on map {}: seed with --map {at_map}",
            play.map
        ))
        .into());
    }
    let point = match options.at {
        Some((_, x, z)) => [x, z],
        None => default_point(pool, map).await?,
    };
    let plans = loaded
        .into_iter()
        .enumerate()
        .map(|(i, (number, username, play))| {
            let weapon_range = defs.attack_range(&play.items, &play.skills);
            BotPlan {
                number,
                username,
                character_id: play.id,
                config: Config {
                    point,
                    radius: options.radius,
                    steps_per_sec: options.steps_per_sec,
                    skill_every: options.skill_every,
                    rebirth_after: options.rebirth_after,
                    idle: idle_for(i as u32, options.idle_share),
                    weapon_range,
                    skills: skill_rules(&defs.items, &defs.skills, &play.items, &play.skills, weapon_range),
                    ..Config::default()
                },
            }
        })
        .collect();
    Ok(Setup { plans, map, point })
}

/// How a bot gets into the world: the plan in, a bot out.
pub type Enter = Arc<dyn Fn(BotPlan) -> Pin<Box<dyn Future<Output = Result<Bot>> + Send>> + Send + Sync>;

/// The real road: the login server, then the game server at the address it returns (the port replaced when `game_port` says so). The
/// first login leaves the login server's list and the game server's address in `note`, for the report.
pub fn login_entry(options: &Options, note: Arc<OnceLock<Value>>) -> Enter {
    let (login, password, server, game_port) = (options.login.clone(), options.password.clone(), options.server, options.game_port);
    Arc::new(move |plan: BotPlan| {
        let (login, password, note) = (login.clone(), password.clone(), note.clone());
        Box::pin(async move {
            let selected = login_and_select(&login, &plan.username, &password, Some(server)).await?;
            let mut address = selected.game_address.clone();
            if let Some(port) = game_port
                && let Some((host, _)) = address.rsplit_once(':')
            {
                address = format!("{host}:{port}");
            }
            let _ = note.set(json!({
                "login_server": login,
                "game_server_id": selected.server_id,
                "game_address": address,
                "servers": selected.servers.iter().map(|s| json!({"id": s.id, "name": s.name, "status": s.status})).collect::<Vec<_>>(),
            }));
            let (link, screen) = connect_game(&address, selected.user_id, &selected.identity, &selected.keys).await?;
            Bot::enter_from(link, &screen).await
        })
    })
}

enum Msg {
    /// A bot's window: its counters, whether it was alive at the end, why its connection ended if it did.
    Window {
        index: u32,
        metrics: Box<crate::metrics::Metrics>,
        alive: bool,
        ended: Option<LinkError>,
    },
    /// What a bot counted while logging out, which belongs to the run's last window.
    Tail {
        index: u32,
        metrics: Box<crate::metrics::Metrics>,
    },
    Login {
        index: u32,
        at: Duration,
        took: Duration,
    },
    LoginFailed {
        index: u32,
        reason: String,
    },
    Logout {
        clean: bool,
        took: Duration,
    },
    Connecting {
        index: u32,
        bots: u64,
    },
    Panicked(String),
}

struct Shared {
    start: Instant,
    window: Duration,
    ramp: f64,
    logout_wait: Duration,
    connecting: AtomicU64,
    /// How long the run was, in microseconds, set before the bots are told to stop.
    end_us: AtomicU64,
    tx: mpsc::UnboundedSender<Msg>,
    board: Arc<EchoBoard>,
    end: watch::Receiver<bool>,
}

impl Shared {
    fn window_of(&self, elapsed: Duration) -> u32 {
        (elapsed.as_secs_f64() / self.window.as_secs_f64()).floor() as u32
    }

    fn send(&self, msg: Msg) {
        // The aggregator outlives every bot; a closed channel only means the run is being torn down.
        let _ = self.tx.send(msg);
    }
}

async fn stopped(end: &mut watch::Receiver<bool>) {
    let _ = end.wait_for(|stop| *stop).await;
}

async fn bot_task(shared: Arc<Shared>, index: usize, plan: BotPlan, enter: Enter) {
    let mut end = shared.end.clone();
    // The ramp: bot `i` logs in at `i / ramp` seconds.
    let due = shared.start + Duration::from_secs_f64(index as f64 / shared.ramp);
    tokio::select! {
        () = sleep_until(due.into()) => {}
        () = stopped(&mut end) => return,
    }
    shared.connecting.fetch_add(1, Ordering::Relaxed);
    let began = Instant::now();
    let entered = enter(plan.clone()).await;
    shared.connecting.fetch_sub(1, Ordering::Relaxed);
    let at = shared.start.elapsed();
    let mut bot = match entered {
        Ok(bot) => bot,
        Err(error) => {
            shared.send(Msg::LoginFailed { index: shared.window_of(at), reason: format!("{}: {error:#}", plan.username) });
            return;
        }
    };
    shared.send(Msg::Login { index: shared.window_of(at), at, took: began.elapsed() });
    bot.echo = Some(Echo { board: shared.board.clone(), slot: index });
    let mut battle = Battle::new(plan.config);

    loop {
        if *end.borrow() {
            break;
        }
        // Play to the end of the window the clock is in, or to the order to stop.
        let current = shared.window_of(shared.start.elapsed());
        let boundary = shared.start + shared.window * (current + 1);
        let stop = async {
            tokio::select! {
                () = sleep_until(boundary.into()) => {}
                () = stopped(&mut end) => {}
            }
        };
        let result = play(&mut bot, &mut battle, stop).await;
        let finished = *end.borrow();
        let now = shared.start.elapsed();
        let metrics = Box::new(bot.metrics.take_window());
        match result {
            Ok(()) => {
                let index = current;
                shared.send(Msg::Window { index, metrics, alive: bot.world.alive, ended: None });
            }
            Err(error) => {
                shared.send(Msg::Window { index: shared.window_of(now), metrics, alive: bot.world.alive, ended: Some(error) });
                // The connection is gone: nothing to log out.
                return;
            }
        }
        if finished {
            break;
        }
    }
    let began = Instant::now();
    let clean = bot.leave(shared.logout_wait).await;
    shared.send(Msg::Tail {
        index: shared.window_of(Duration::from_micros(shared.end_us.load(Ordering::Acquire))),
        metrics: Box::new(bot.metrics.take_window()),
    });
    shared.send(Msg::Logout { clean, took: began.elapsed() });
}

async fn collect(mut rx: mpsc::UnboundedReceiver<Msg>) -> Collector {
    let mut c = Collector::default();
    while let Some(msg) = rx.recv().await {
        match msg {
            Msg::Window { index, metrics, alive, ended } => c.window(index).add_bot(&metrics, alive, ended.as_ref()),
            Msg::Tail { index, metrics } => c.window(index).add_metrics(&metrics),
            Msg::Login { index, at, took } => c.login(index, at, took),
            Msg::LoginFailed { index, reason } => c.login_failed(index, reason),
            Msg::Logout { clean, took } => c.logout(clean, took),
            Msg::Connecting { index, bots } => c.window(index).connecting = bots,
            Msg::Panicked(what) => c.note(what),
        }
    }
    c
}

/// Runs the swarm until `options.duration` or until `stop` completes, logs every bot out and returns the report. `args` is what the
/// report says the run was asked for (`Options::args_json`); `server` is filled by the entry with what it learnt of the servers.
pub async fn run(
    plans: Vec<BotPlan>,
    enter: Enter,
    options: &Options,
    args: Value,
    server: &OnceLock<Value>,
    stop: impl Future<Output = ()>,
) -> Report {
    let start = Instant::now();
    let ids: Vec<u32> = plans.iter().map(|p| p.character_id).collect();
    let bots = plans.len();
    let (tx, rx) = mpsc::unbounded_channel();
    let (end_tx, end) = watch::channel(false);
    let shared = Arc::new(Shared {
        start,
        window: options.window,
        ramp: options.ramp.max(0.001),
        logout_wait: options.logout_wait,
        connecting: AtomicU64::new(0),
        end_us: AtomicU64::new(0),
        tx,
        board: EchoBoard::new(&ids, start),
        end,
    });
    let collector = tokio::spawn(collect(rx));
    let mut tasks = JoinSet::new();
    for (index, plan) in plans.into_iter().enumerate() {
        tasks.spawn(bot_task(shared.clone(), index, plan, enter.clone()));
    }

    // The clock: the windows' boundaries, then the end.
    tokio::pin!(stop);
    let deadline = start + options.duration;
    let mut boundary = 1u32;
    loop {
        let next = start + options.window * boundary;
        tokio::select! {
            () = sleep_until(next.into()), if next < deadline => {
                shared.send(Msg::Connecting { index: boundary - 1, bots: shared.connecting.load(Ordering::Relaxed) });
                boundary += 1;
            }
            () = sleep_until(deadline.into()) => break,
            () = &mut stop => break,
        }
    }
    let ended = start.elapsed();
    shared.end_us.store(ended.as_micros() as u64, Ordering::Release);
    let _ = end_tx.send(true);
    while let Some(joined) = tasks.join_next().await {
        if let Err(error) = joined {
            shared.send(Msg::Panicked(format!("a bot's task ended badly: {error}")));
        }
    }
    drop(shared);
    let collected = collector.await.expect("the aggregator does not panic");
    let info = RunInfo {
        args,
        bots,
        window: options.window,
        ramp_per_sec: options.ramp,
        ended,
        scenario: options.scenario.clone(),
        server: server.get().cloned(),
    };
    Report::build(info, collected)
}

/// The command line of `bot swarm`.
pub mod cli {
    use std::path::PathBuf;
    use std::time::Duration;

    use anyhow::{Result, bail};

    use super::{MAX_COUNT, MAX_NUMBER, Options};
    use crate::seed::{Refused, check_prefix};

    pub const SWARM_USAGE: &str = "usage: bot swarm --login <host:30800> --count <n> [--first 1] [--prefix bot] [--server 1] [--ramp 5] [--scenario pvp-battle]\n       \
                                   [--at <map,x,z>] [--radius 60] [--duration 10m] [--steps-per-sec 2] [--skill-every 6s]\n       \
                                   [--rebirth-after 5s] [--idle-share 0] [--report report.json]";

    fn value<'a>(args: &'a [String], i: usize, flag: &str) -> Result<&'a str> {
        args.get(i + 1).map(String::as_str).ok_or_else(|| anyhow::anyhow!("{flag} needs a value"))
    }

    /// `10m`, `90s`, `2h`, `500ms`, `1.5s`; a bare number is seconds.
    pub fn parse_duration(text: &str) -> Result<Duration> {
        let (number, unit) = match text.find(|c: char| c.is_ascii_alphabetic()) {
            Some(at) => text.split_at(at),
            None => (text, "s"),
        };
        let number: f64 = number.parse().map_err(|_| anyhow::anyhow!("{text:?} is not a duration (10m, 90s, 2h, 500ms)"))?;
        let seconds = match unit {
            "ms" => number / 1000.0,
            "s" => number,
            "m" => number * 60.0,
            "h" => number * 3600.0,
            _ => bail!("{text:?} is not a duration (10m, 90s, 2h, 500ms)"),
        };
        if !seconds.is_finite() || seconds < 0.0 {
            bail!("{text:?} is not a duration (10m, 90s, 2h, 500ms)");
        }
        Ok(Duration::from_secs_f64(seconds))
    }

    /// `map,x,z`.
    pub fn parse_at(text: &str) -> Result<(u16, f32, f32)> {
        match text.split(',').collect::<Vec<_>>()[..] {
            [map, x, z] => Ok((map.trim().parse()?, x.trim().parse()?, z.trim().parse()?)),
            _ => bail!("--at is map,x,z (for example 18,636,394)"),
        }
    }

    /// `args` are those after `swarm`; the password is `SHAIYA_PASSWORD`'s.
    pub fn parse_swarm(args: &[String], password: &str) -> Result<Options> {
        let mut options = Options::new("", 0, password);
        let mut i = 0;
        while i < args.len() {
            let flag = args[i].as_str();
            let v = || value(args, i, flag);
            match flag {
                "--login" => options.login = v()?.to_owned(),
                "--count" => options.count = v()?.parse()?,
                "--first" => options.first = v()?.parse()?,
                "--prefix" => options.prefix = v()?.to_owned(),
                "--server" => options.server = v()?.parse()?,
                "--ramp" => options.ramp = v()?.parse()?,
                "--scenario" => options.scenario = v()?.to_owned(),
                "--at" => options.at = Some(parse_at(v()?)?),
                "--radius" => options.radius = v()?.parse()?,
                "--duration" => options.duration = parse_duration(v()?)?,
                "--steps-per-sec" => options.steps_per_sec = v()?.parse()?,
                "--skill-every" => options.skill_every = parse_duration(v()?)?,
                "--rebirth-after" => options.rebirth_after = parse_duration(v()?)?,
                "--idle-share" => options.idle_share = v()?.parse()?,
                "--report" => options.report = PathBuf::from(v()?),
                _ => bail!("{SWARM_USAGE}"),
            }
            i += 2;
        }
        let refuse = |why: String| -> Result<Options> { Err(Refused(why).into()) };
        if options.login.is_empty() {
            return refuse("--login <host:30800> is required".into());
        }
        if options.count == 0 || options.count > MAX_COUNT {
            return refuse(format!("--count is 1 to {MAX_COUNT}: a larger battle is several --first ranges"));
        }
        if options.first == 0 || options.first + options.count - 1 > MAX_NUMBER {
            return refuse(format!("--first {} and --count {} go past bot number {MAX_NUMBER} or below 1", options.first, options.count));
        }
        check_prefix(&options.prefix)?;
        if options.scenario != "pvp-battle" {
            return refuse(format!("--scenario {:?}: only pvp-battle exists", options.scenario));
        }
        if !(options.ramp > 0.0 && options.ramp.is_finite()) {
            return refuse("--ramp is bots a second, above 0".into());
        }
        if !(options.steps_per_sec > 0.0 && options.steps_per_sec.is_finite()) || !(options.radius >= 0.0 && options.radius.is_finite()) {
            return refuse("--steps-per-sec is above 0 and --radius 0 or more".into());
        }
        if !(0.0..=1.0).contains(&options.idle_share) {
            return refuse("--idle-share is 0 to 1".into());
        }
        if options.duration.is_zero() {
            return refuse("--duration is above 0".into());
        }
        Ok(options)
    }
}

#[cfg(test)]
mod tests {
    use game_server::inventory::ItemDef;

    use super::*;

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn the_command_line_is_read_with_the_specs_defaults() {
        let o = cli::parse_swarm(&strings(&["--login", "10.0.0.2:30800", "--count", "300"]), "pw").unwrap();
        assert_eq!((o.count, o.first, o.prefix.as_str(), o.ramp, o.scenario.as_str()), (300, 1, "bot", 5.0, "pvp-battle"));
        assert_eq!((o.radius, o.duration, o.steps_per_sec), (60.0, Duration::from_secs(600), 2.0));
        assert_eq!((o.skill_every, o.rebirth_after, o.idle_share), (Duration::from_secs(6), Duration::from_secs(5), 0.0));
        assert_eq!((o.report.to_str(), o.at, o.server), (Some("report.json"), None, 1));
        let o = cli::parse_swarm(
            &strings(&[
                "--login",
                "h:1",
                "--count",
                "150",
                "--first",
                "151",
                "--prefix",
                "tb",
                "--ramp",
                "2.5",
                "--at",
                "18,636,394",
                "--radius",
                "30",
                "--duration",
                "90s",
                "--steps-per-sec",
                "4",
                "--skill-every",
                "2500ms",
                "--rebirth-after",
                "1s",
                "--idle-share",
                "0.3",
                "--report",
                "runs/R1.json",
                "--server",
                "2",
            ]),
            "pw",
        )
        .unwrap();
        assert_eq!((o.count, o.first, o.prefix.as_str(), o.ramp, o.at), (150, 151, "tb", 2.5, Some((18, 636.0, 394.0))));
        assert_eq!(
            (o.radius, o.duration, o.steps_per_sec, o.skill_every),
            (30.0, Duration::from_secs(90), 4.0, Duration::from_millis(2500))
        );
        assert_eq!((o.rebirth_after, o.idle_share, o.report.to_str(), o.server), (Duration::from_secs(1), 0.3, Some("runs/R1.json"), 2));
    }

    #[test]
    fn a_bad_command_line_is_refused_or_explained() {
        let refused = |args: &[&str]| cli::parse_swarm(&strings(args), "pw").unwrap_err().downcast_ref::<Refused>().is_some();
        assert!(refused(&["--count", "5"]), "no login server");
        assert!(refused(&["--login", "h:1"]), "no count");
        assert!(refused(&["--login", "h:1", "--count", "1001"]));
        assert!(refused(&["--login", "h:1", "--count", "10", "--first", "9995"]), "past bot 9999");
        assert!(refused(&["--login", "h:1", "--count", "10", "--scenario", "soak"]));
        assert!(refused(&["--login", "h:1", "--count", "10", "--ramp", "0"]));
        assert!(refused(&["--login", "h:1", "--count", "10", "--idle-share", "1.5"]));
        assert!(refused(&["--login", "h:1", "--count", "10", "--prefix", "waytoolongprefix"]));
        assert!(cli::parse_swarm(&strings(&["--login", "h:1", "--count", "10", "--bogus", "1"]), "pw").is_err());
        assert!(cli::parse_swarm(&strings(&["--login", "h:1", "--count", "10", "--duration"]), "pw").is_err());
        assert!(cli::parse_at("18,1").is_err() && cli::parse_duration("ten").is_err() && cli::parse_duration("5d").is_err());
        assert_eq!(cli::parse_duration("2h").unwrap(), Duration::from_secs(7200));
        assert_eq!(cli::parse_duration("45").unwrap(), Duration::from_secs(45));
        assert_eq!(cli::parse_duration("1.5s").unwrap(), Duration::from_millis(1500));
    }

    #[test]
    fn the_idle_share_is_spread_evenly() {
        let idle = |share, count| (0..count).filter(|&i| idle_for(i, share)).count();
        assert_eq!((idle(0.0, 300), idle(1.0, 300), idle(0.3, 300), idle(0.5, 7)), (0, 300, 90, 3));
        // Never two in a row at a share of one half.
        assert!((0..100).all(|i| !(idle_for(i, 0.5) && idle_for(i + 1, 0.5))));
    }

    fn weapon(real_type_id: u8) -> ItemUnit {
        ItemUnit { bag: 0, slot: WEAPON_SLOT, item_type: real_type_id, type_id: 1, quality: 1200, count: 1, ..Default::default() }
    }

    #[test]
    fn skill_rules_keep_the_attacks_on_one_target_the_bot_can_use() {
        let items = ItemDefs::new([ItemDef {
            item_id: 5001,
            item_type: 5,
            type_id: 1,
            level: 1,
            country: 6,
            jobs: [true; 6],
            ..Default::default()
        }]);
        let mut sword = [false; 15];
        sword[4] = true;
        let attack = |id, level| SkillDef {
            id,
            level,
            country: 6,
            jobs: [true; 6],
            type_show: 1,
            type_effect: ATTACK,
            target_type: skills::TARGET_ONE,
            need_weapons: sword,
            need_sp: 10,
            need_mp: 2,
            reset_time: 4,
            attack_range: 3,
            ..Default::default()
        };
        let mut bow = [false; 15];
        bow[8] = true;
        let defs = SkillDefs::new([
            attack(1, 1),
            attack(1, 2),
            SkillDef { type_effect: skills::BUFF, ..attack(2, 1) },
            SkillDef { target_type: 2, ..attack(3, 1) },
            SkillDef { need_weapons: bow, ..attack(4, 1) },
            SkillDef { need_shield: true, ..attack(5, 1) },
            SkillDef { prev_skill: 1, ..attack(6, 1) },
            SkillDef { target_type: skills::TARGET_AROUND_TARGET, ..attack(7, 1) },
            SkillDef { type_effect: 0, ..attack(8, 1) },
            SkillDef { kind: skills::DESPERATE, ..attack(10, 1) },
        ]);
        let known: Vec<Skill> = [(1, 2), (2, 1), (3, 1), (4, 1), (5, 1), (6, 1), (7, 1), (8, 1), (9, 1), (10, 1)]
            .iter()
            .enumerate()
            .map(|(number, &(id, level))| Skill { id, level, number: number as u8, delay: 0 })
            .collect();
        // With a sword: skill 1 (at the level the bot has it, 2) and the area attack around the target (7); not the buff, the one on
        // oneself, the bow's, the shield's, the follow-up (6), the passive (8), an undefined skill (9) or the one that gives up the caster's life (10).
        let rules = skill_rules(&items, &defs, &[weapon(5)], &known, 1);
        assert_eq!(rules.iter().map(|r| (r.skill_id, r.level)).collect::<Vec<_>>(), [(1, 2), (7, 1)]);
        // Reach: the bot's 1, the skill's 3, the server's 2.3, less the 0.5 margin; costs and the rest period as defined.
        assert_eq!((rules[0].attack_range, rules[0].sp, rules[0].mp, rules[0].reset), (5.8, 10, 2, Duration::from_secs(4)));
        // With a shield instead of a sword only the shield's skill is usable, with nothing worn none.
        let shield = ItemUnit { slot: SHIELD_SLOT, ..weapon(5) };
        assert_eq!(skill_rules(&items, &defs, &[shield], &known, 0).iter().map(|r| r.skill_id).collect::<Vec<_>>(), [5]);
        assert!(skill_rules(&items, &defs, &[], &known, 0).is_empty());
    }
}
