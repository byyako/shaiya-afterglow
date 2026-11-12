//! The swarm's windows and `report.json` (sections 5.3 and 6 of `docs/plans/test-bots.md`): what every bot hands over at the end of a
//! window is added into that window's figures, the windows are told apart as ramp, steady and partial, and the summary is the steady
//! windows added together.
//!
//! The report's shape (`schema` 1):
//!
//! ```text
//! { "schema", "run": {args, bots, window_s, duration_s, build, server},
//!   "ramp": {bots_per_sec, first_login_s, last_login_s, logins_ok, logins_failed, login_ms, windows},
//!   "windows": [ {index, start_s, seconds, partial, ramp, bots, disconnects, login_ms, packets, per_bot, opcodes,
//!                 attack_rtt_ms, skill_rtt_ms, step_echo_ms, results, steps, refusals, protocol_errors, kills, deaths, rebirths} ],
//!   "summary": {steady, windows, seconds, ...the same figures over the steady windows},
//!   "logout": {bots, clean, took_ms}, "notes": [..] }
//! ```

use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use std::time::Duration;

use game_server::combat as server_combat;
use serde_json::{Value, json};

use crate::link::LinkError;
use crate::metrics::{Count, Histogram, Metrics};

pub const SCHEMA: u32 = 1;
/// A last window shorter than this is added to the one before it instead of standing alone.
const MIN_LAST_WINDOW: Duration = Duration::from_secs(1);

/// What one window holds: the sums of the bots' figures and the bots' states at its end.
#[derive(Clone, Debug, Default)]
pub struct WindowAcc {
    pub rx: BTreeMap<u16, Count>,
    pub tx: BTreeMap<u16, Count>,
    pub attack: Histogram,
    pub skill: Histogram,
    pub echo: Histogram,
    pub login: Histogram,
    pub results: BTreeMap<u8, u64>,
    pub steps: u64,
    pub refusals: u64,
    pub protocol_errors: u64,
    pub kills: u64,
    pub deaths: u64,
    pub rebirths: u64,
    /// Bots in the world at the end of the window, and of them the dead ones.
    pub in_world: u64,
    pub dead: u64,
    /// Bots logging in at the end of the window (sampled by the swarm's clock).
    pub connecting: u64,
    pub logins_ok: u64,
    pub logins_failed: u64,
    /// Connections that ended in the window, by why.
    pub closed: u64,
    pub timed_out: u64,
    pub errors: u64,
}

fn add_counts(into: &mut BTreeMap<u16, Count>, from: &BTreeMap<u16, Count>) {
    for (opcode, c) in from {
        let t = into.entry(*opcode).or_default();
        t.packets += c.packets;
        t.bytes += c.bytes;
    }
}

fn total(m: &BTreeMap<u16, Count>) -> Count {
    m.values().fold(Count::default(), |a, c| Count { packets: a.packets + c.packets, bytes: a.bytes + c.bytes })
}

impl WindowAcc {
    /// A bot's figures for the window; `alive` is how it stood at the end, `ended` why its connection ended in it, if it did.
    pub fn add_bot(&mut self, m: &Metrics, alive: bool, ended: Option<&LinkError>) {
        self.add_metrics(m);
        match ended {
            None => {
                self.in_world += 1;
                self.dead += u64::from(!alive);
            }
            Some(LinkError::Closed) => self.closed += 1,
            Some(LinkError::Timeout) => self.timed_out += 1,
            Some(LinkError::Error(_)) => self.errors += 1,
        }
    }

    /// A bot's counters, without its state: what it counted after the window's last report (the logout).
    pub fn add_metrics(&mut self, m: &Metrics) {
        add_counts(&mut self.rx, &m.received);
        add_counts(&mut self.tx, &m.sent);
        self.attack.add_samples(&m.attack_round_trip);
        self.skill.add_samples(&m.skill_round_trip);
        self.echo.add_samples(&m.step_echo);
        for (code, n) in &m.results {
            *self.results.entry(*code).or_default() += n;
        }
        self.steps += m.steps;
        self.refusals += m.refusals;
        self.protocol_errors += m.protocol_errors;
        self.kills += m.kills;
        self.deaths += m.deaths;
        self.rebirths += m.rebirths;
    }

    /// The figures added into this window's (the states at the end are the caller's business).
    pub fn add_figures(&mut self, other: &Self) {
        add_counts(&mut self.rx, &other.rx);
        add_counts(&mut self.tx, &other.tx);
        self.attack.merge(&other.attack);
        self.skill.merge(&other.skill);
        self.echo.merge(&other.echo);
        self.login.merge(&other.login);
        for (code, n) in &other.results {
            *self.results.entry(*code).or_default() += n;
        }
        self.steps += other.steps;
        self.refusals += other.refusals;
        self.protocol_errors += other.protocol_errors;
        self.kills += other.kills;
        self.deaths += other.deaths;
        self.rebirths += other.rebirths;
        self.logins_ok += other.logins_ok;
        self.logins_failed += other.logins_failed;
        self.closed += other.closed;
        self.timed_out += other.timed_out;
        self.errors += other.errors;
    }

    fn disconnects(&self) -> u64 {
        self.closed + self.timed_out + self.errors
    }
}

/// Everything the swarm collected, before it is told apart into windows.
#[derive(Debug, Default)]
pub struct Collector {
    pub windows: BTreeMap<u32, WindowAcc>,
    /// When (since the start) the first and the last bot got into the world.
    pub first_login: Option<Duration>,
    pub last_login: Option<Duration>,
    pub logout_bots: u64,
    pub logout_clean: u64,
    pub logout_took: Histogram,
    /// Things worth saying in the report: the first failed logins' reasons, tasks that panicked.
    pub notes: Vec<String>,
}

/// The first failed logins' reasons kept as notes.
const MAX_NOTES: usize = 10;

impl Collector {
    pub fn window(&mut self, index: u32) -> &mut WindowAcc {
        self.windows.entry(index).or_default()
    }

    pub fn note(&mut self, text: String) {
        if self.notes.len() < MAX_NOTES {
            self.notes.push(text);
        }
    }

    pub fn login(&mut self, window: u32, at: Duration, took: Duration) {
        let w = self.window(window);
        w.logins_ok += 1;
        w.login.record(took);
        self.first_login = Some(self.first_login.map_or(at, |f| f.min(at)));
        self.last_login = Some(self.last_login.map_or(at, |l| l.max(at)));
    }

    pub fn login_failed(&mut self, window: u32, reason: String) {
        self.window(window).logins_failed += 1;
        self.note(format!("login failed: {reason}"));
    }

    pub fn logout(&mut self, clean: bool, took: Duration) {
        self.logout_bots += 1;
        self.logout_clean += u64::from(clean);
        self.logout_took.record(took);
    }
}

/// What the report says of the run itself.
#[derive(Clone, Debug)]
pub struct RunInfo {
    /// The run's arguments, as the command line gave them.
    pub args: Value,
    pub bots: usize,
    pub window: Duration,
    pub ramp_per_sec: f64,
    /// How long the run was, from the first login to the order to stop.
    pub ended: Duration,
    pub scenario: String,
    /// The login server's list and the game server the bots were sent to, when they logged in through a login server.
    pub server: Option<Value>,
}

/// One window of the report.
#[derive(Clone, Debug)]
pub struct Window {
    pub index: u32,
    pub start: Duration,
    pub seconds: f64,
    pub partial: bool,
    /// Before the last login: the ramp's own figures, kept apart from the steady state.
    pub ramp: bool,
    /// Disconnected bots so far.
    pub disconnected_total: u64,
    pub acc: WindowAcc,
}

impl Window {
    fn steady(&self) -> bool {
        !self.ramp && !self.partial
    }
}

/// The steady windows added together.
#[derive(Clone, Debug)]
pub struct Summary {
    /// The windows' indices; empty when the run had no steady state.
    pub windows: Vec<u32>,
    pub seconds: f64,
    pub from: Duration,
    pub in_world: [f64; 3],
    pub dead_avg: f64,
    pub connecting_avg: f64,
    pub acc: WindowAcc,
}

#[derive(Clone, Debug)]
pub struct Report {
    pub info: RunInfo,
    pub windows: Vec<Window>,
    pub summary: Summary,
    pub first_login: Option<Duration>,
    pub last_login: Option<Duration>,
    pub ramp_acc: WindowAcc,
    pub logout: (u64, u64, Histogram),
    pub notes: Vec<String>,
}

fn rounded(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

fn ms(d: Option<Duration>) -> Value {
    d.map_or(Value::Null, |d| json!(rounded(d.as_secs_f64() * 1000.0)))
}

fn stats(h: &Histogram) -> Value {
    json!({"n": h.len(), "p50": ms(h.percentile(50.0)), "p95": ms(h.percentile(95.0)), "p99": ms(h.percentile(99.0)), "max": ms(h.max())})
}

fn count_json(c: Count) -> Value {
    json!({"packets": c.packets, "bytes": c.bytes})
}

fn opcodes_json(rx: &BTreeMap<u16, Count>, tx: &BTreeMap<u16, Count>) -> Value {
    let mut all: BTreeMap<u16, Value> = BTreeMap::new();
    for opcode in rx.keys().chain(tx.keys()) {
        let (r, t) = (rx.get(opcode).copied().unwrap_or_default(), tx.get(opcode).copied().unwrap_or_default());
        all.insert(*opcode, json!({"rx_packets": r.packets, "rx_bytes": r.bytes, "tx_packets": t.packets, "tx_bytes": t.bytes}));
    }
    Value::Object(all.into_iter().map(|(opcode, v)| (format!("{opcode:#06x}"), v)).collect())
}

fn results_json(results: &BTreeMap<u8, u64>) -> Value {
    Value::Object(results.iter().map(|(code, n)| (code.to_string(), json!(n))).collect())
}

/// Packets and bytes a second for one bot, from a window's totals and the bots in the world at its end.
fn per_bot(acc: &WindowAcc, bots: f64, seconds: f64) -> [f64; 4] {
    let each = |x: u64| if bots > 0.0 && seconds > 0.0 { x as f64 / bots / seconds } else { 0.0 };
    let (rx, tx) = (total(&acc.rx), total(&acc.tx));
    [each(rx.packets), each(rx.bytes), each(tx.packets), each(tx.bytes)]
}

/// The figures the windows and the summary share.
fn figures_json(acc: &WindowAcc, bots: f64, seconds: f64) -> Value {
    let [rx_pps, rx_bps, tx_pps, tx_bps] = per_bot(acc, bots, seconds);
    json!({
        "packets": {"rx": count_json(total(&acc.rx)), "tx": count_json(total(&acc.tx))},
        "per_bot": {
            "rx_packets_per_sec": rounded(rx_pps), "rx_bytes_per_sec": rounded(rx_bps),
            "tx_packets_per_sec": rounded(tx_pps), "tx_bytes_per_sec": rounded(tx_bps),
        },
        "opcodes": opcodes_json(&acc.rx, &acc.tx),
        "attack_rtt_ms": stats(&acc.attack),
        "skill_rtt_ms": stats(&acc.skill),
        "step_echo_ms": stats(&acc.echo),
        "results": results_json(&acc.results),
        "steps": acc.steps,
        "refusals": acc.refusals,
        "protocol_errors": acc.protocol_errors,
        "kills": acc.kills,
        "deaths": acc.deaths,
        "rebirths": acc.rebirths,
    })
}

fn merge_into(figures: &mut Value, extra: Value) {
    if let (Value::Object(base), Value::Object(more)) = (figures, extra) {
        base.extend(more);
    }
}

impl Report {
    /// Tells the collected windows apart and adds the steady ones up.
    pub fn build(info: RunInfo, mut collected: Collector) -> Self {
        let window_secs = info.window.as_secs_f64();
        let last_index = (info.ended.as_secs_f64() / window_secs).floor() as u32;
        for index in 0..=last_index {
            collected.window(index);
        }
        let mut windows: Vec<Window> = Vec::new();
        for (index, acc) in std::mem::take(&mut collected.windows) {
            let start = info.window * index;
            let seconds = (info.ended.as_secs_f64() - start.as_secs_f64()).min(window_secs).max(0.0);
            let last = index >= last_index;
            if last && Duration::from_secs_f64(seconds) < MIN_LAST_WINDOW && !windows.is_empty() {
                // The run ended on a boundary: what the last few milliseconds held belongs to the window before.
                let before = windows.last_mut().expect("not empty");
                before.acc.add_figures(&acc);
                // The bots that reported in the last moments (those whose boundary slice had already ended when the order came) are the
                // ones in the world at the end.
                if acc.in_world >= before.acc.in_world {
                    (before.acc.in_world, before.acc.dead) = (acc.in_world, acc.dead);
                }
                before.seconds += seconds;
                continue;
            }
            windows.push(Window {
                index,
                start,
                seconds,
                partial: last && seconds < window_secs - 0.5,
                ramp: collected.last_login.is_none_or(|login| login > start),
                disconnected_total: 0,
                acc,
            });
        }
        let mut gone = 0;
        for w in &mut windows {
            gone += w.acc.disconnects();
            w.disconnected_total = gone;
        }

        let steady: Vec<&Window> = windows.iter().filter(|w| w.steady()).collect();
        let mut acc = WindowAcc::default();
        for w in &steady {
            acc.add_figures(&w.acc);
        }
        let n = steady.len().max(1) as f64;
        let world: Vec<f64> = steady.iter().map(|w| w.acc.in_world as f64).collect();
        let summary = Summary {
            windows: steady.iter().map(|w| w.index).collect(),
            seconds: steady.iter().map(|w| w.seconds).sum(),
            from: steady.first().map_or(Duration::ZERO, |w| w.start),
            in_world: [
                world.iter().copied().reduce(f64::min).unwrap_or(0.0),
                world.iter().sum::<f64>() / n,
                world.iter().copied().reduce(f64::max).unwrap_or(0.0),
            ],
            dead_avg: steady.iter().map(|w| w.acc.dead as f64).sum::<f64>() / n,
            connecting_avg: steady.iter().map(|w| w.acc.connecting as f64).sum::<f64>() / n,
            acc,
        };
        let mut ramp_acc = WindowAcc::default();
        for w in windows.iter().filter(|w| w.ramp) {
            ramp_acc.add_figures(&w.acc);
        }
        Self {
            info,
            windows,
            summary,
            first_login: collected.first_login,
            last_login: collected.last_login,
            ramp_acc,
            logout: (collected.logout_bots, collected.logout_clean, collected.logout_took),
            notes: collected.notes,
        }
    }

    pub fn json(&self) -> Value {
        let windows: Vec<Value> = self
            .windows
            .iter()
            .map(|w| {
                let mut v = json!({
                    "index": w.index,
                    "start_s": rounded(w.start.as_secs_f64()),
                    "seconds": rounded(w.seconds),
                    "partial": w.partial,
                    "ramp": w.ramp,
                    "bots": {
                        "connecting": w.acc.connecting, "in_world": w.acc.in_world, "dead": w.acc.dead,
                        "disconnected": w.disconnected_total, "logins_ok": w.acc.logins_ok, "logins_failed": w.acc.logins_failed,
                    },
                    "disconnects": {"closed": w.acc.closed, "timeout": w.acc.timed_out, "error": w.acc.errors},
                    "login_ms": stats(&w.acc.login),
                });
                merge_into(&mut v, figures_json(&w.acc, w.acc.in_world as f64, w.seconds));
                v
            })
            .collect();
        let s = &self.summary;
        let mut summary = json!({
            "steady": !s.windows.is_empty(),
            "windows": s.windows,
            "from_s": rounded(s.from.as_secs_f64()),
            "seconds": rounded(s.seconds),
            "bots_in_world": {"min": rounded(s.in_world[0]), "avg": rounded(s.in_world[1]), "max": rounded(s.in_world[2])},
            "dead_avg": rounded(s.dead_avg),
            "disconnects": {"closed": s.acc.closed, "timeout": s.acc.timed_out, "error": s.acc.errors},
        });
        merge_into(&mut summary, figures_json(&s.acc, s.in_world[1], s.seconds));
        json!({
            "schema": SCHEMA,
            "run": {
                "args": self.info.args,
                "scenario": self.info.scenario,
                "bots": self.info.bots,
                "window_s": rounded(self.info.window.as_secs_f64()),
                "duration_s": rounded(self.info.ended.as_secs_f64()),
                "build": {"bot": env!("CARGO_PKG_VERSION"), "client_version": format!("{:#010x}", crate::login::CLIENT_VERSION)},
                "server": self.info.server,
            },
            "ramp": {
                "bots_per_sec": self.info.ramp_per_sec,
                "first_login_s": self.first_login.map(|d| rounded(d.as_secs_f64())),
                "last_login_s": self.last_login.map(|d| rounded(d.as_secs_f64())),
                "logins_ok": self.windows.iter().map(|w| w.acc.logins_ok).sum::<u64>(),
                "logins_failed": self.windows.iter().map(|w| w.acc.logins_failed).sum::<u64>(),
                "login_ms": stats(&self.windows.iter().fold(Histogram::default(), |mut h, w| { h.merge(&w.acc.login); h })),
                "windows": self.windows.iter().filter(|w| w.ramp).map(|w| w.index).collect::<Vec<_>>(),
            },
            "windows": windows,
            "summary": summary,
            "logout": {"bots": self.logout.0, "clean": self.logout.1, "took_ms": stats(&self.logout.2)},
            "notes": self.notes,
        })
    }
}

fn ms_text(d: Option<Duration>) -> String {
    d.map_or_else(|| "-".to_owned(), |d| format!("{:.1}", d.as_secs_f64() * 1000.0))
}

fn line_of(label: &str, h: &Histogram) -> String {
    format!(
        "{label}: p50 {} p95 {} p99 {} max {} ms (n {})",
        ms_text(h.percentile(50.0)),
        ms_text(h.percentile(95.0)),
        ms_text(h.percentile(99.0)),
        ms_text(h.max()),
        h.len()
    )
}

fn result_name(code: u8) -> String {
    match code {
        server_combat::RESULT_HIT => "hit".to_owned(),
        server_combat::RESULT_CRITICAL => "critical".to_owned(),
        server_combat::RESULT_MISS => "miss".to_owned(),
        server_combat::RESULT_OUT_OF_RANGE => "out of reach".to_owned(),
        server_combat::RESULT_NO_WEAPON => "no weapon".to_owned(),
        server_combat::RESULT_CANNOT_ATTACK => "cannot attack".to_owned(),
        other => format!("result {other}"),
    }
}

/// The printed summary: one screen, the steady-state windows only.
impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (s, w) = (&self.summary, self.info.window.as_secs_f64());
        writeln!(f, "bot swarm: {} bots, {}, {:.1} s run", self.info.bots, self.info.scenario, self.info.ended.as_secs_f64())?;
        let (ok, failed) =
            (self.windows.iter().map(|x| x.acc.logins_ok).sum::<u64>(), self.windows.iter().map(|x| x.acc.logins_failed).sum::<u64>());
        let mut logins = Histogram::default();
        self.windows.iter().for_each(|x| logins.merge(&x.acc.login));
        writeln!(
            f,
            "ramp: {} a second, {ok} logged in, {failed} failed, last login at {} s; login p50 {} p95 {} max {} ms",
            self.info.ramp_per_sec,
            self.last_login.map_or_else(|| "-".to_owned(), |d| format!("{:.1}", d.as_secs_f64())),
            ms_text(logins.percentile(50.0)),
            ms_text(logins.percentile(95.0)),
            ms_text(logins.max())
        )?;
        if s.windows.is_empty() {
            writeln!(f, "no steady-state window: the run ended before the ramp and one whole window of {w} s were over")?;
        } else {
            writeln!(
                f,
                "steady state: {} windows of {w} s from {:.1} s ({:.0} s in all)",
                s.windows.len(),
                s.from.as_secs_f64(),
                s.seconds
            )?;
            writeln!(
                f,
                "bots in world: {:.0} min, {:.1} avg, {:.0} max; dead {:.1} avg; connecting {:.1} avg; disconnected {} (closed {}, timeout {}, error {})",
                s.in_world[0],
                s.in_world[1],
                s.in_world[2],
                s.dead_avg,
                s.connecting_avg,
                s.acc.closed + s.acc.timed_out + s.acc.errors,
                s.acc.closed,
                s.acc.timed_out,
                s.acc.errors
            )?;
            writeln!(f, "{}", line_of("attack round trip", &s.acc.attack))?;
            writeln!(f, "{}", line_of("skill round trip", &s.acc.skill))?;
            writeln!(f, "{}", line_of("step echo", &s.acc.echo))?;
            let [rx_pps, rx_bps, tx_pps, tx_bps] = per_bot(&s.acc, s.in_world[1], s.seconds);
            writeln!(
                f,
                "per bot: received {rx_pps:.1} packets/s ({:.1} KB/s), sent {tx_pps:.1} packets/s ({:.1} KB/s)",
                rx_bps / 1024.0,
                tx_bps / 1024.0
            )?;
            writeln!(
                f,
                "steps {}, refusals {}, protocol errors {}; kills {}, deaths {}, rebirths {}",
                s.acc.steps, s.acc.refusals, s.acc.protocol_errors, s.acc.kills, s.acc.deaths, s.acc.rebirths
            )?;
            let mut results = String::new();
            for (code, n) in &s.acc.results {
                let _ = write!(results, "{}{} {n}", if results.is_empty() { "" } else { ", " }, result_name(*code));
            }
            writeln!(f, "attack results: {}", if results.is_empty() { "none" } else { &results })?;
        }
        write!(f, "logout: {} of {} clean, slowest {} ms", self.logout.1, self.logout.0, ms_text(self.logout.2.max()))?;
        for note in &self.notes {
            write!(f, "\nnote: {note}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One bot's window: opcode 0x0501 and 0x0502 traffic, some round trips, a few kills.
    fn metrics(attack_ms: &[u64], echo_ms: &[u64], kills: u64, deaths: u64) -> Metrics {
        let mut m = Metrics::default();
        for &ms in attack_ms {
            m.attack_round_trip.record(Duration::from_millis(ms));
        }
        for &ms in echo_ms {
            m.step_echo.record(Duration::from_millis(ms));
        }
        m.received.insert(0x0501, Count { packets: 100, bytes: 1_900 });
        m.received.insert(0x0502, Count { packets: 20, bytes: 400 });
        m.sent.insert(0x0501, Count { packets: 20, bytes: 400 });
        m.results.insert(0, 5);
        m.results.insert(3, 1);
        m.results.insert(5, 2);
        (m.steps, m.refusals, m.protocol_errors, m.kills, m.deaths, m.rebirths) = (20, 0, 0, kills, deaths, deaths);
        m
    }

    /// Three bots, 10 s windows, a run of 35 s: window 0 holds the ramp (the last login at 4 s), windows 1 and 2 are steady, window 3 is
    /// the 5 s that were left.
    fn fixed() -> Report {
        let mut c = Collector::default();
        for (i, at) in [1.0, 2.5, 4.0].into_iter().enumerate() {
            c.login(0, Duration::from_secs_f64(at), Duration::from_millis(100 * (i as u64 + 1)));
        }
        c.login_failed(0, "bot0009: login refused with status 3".into());
        for index in 0..=3 {
            c.window(index).connecting = u64::from(index == 0);
            for bot in 0..3 {
                let (kills, deaths) = (u64::from(bot == 0 && index > 0), u64::from(bot == 1 && index > 0));
                c.window(index).add_bot(&metrics(&[10, 20, 30], &[5, 15], kills, deaths), bot != 1 || index != 2, None);
            }
        }
        c.logout(true, Duration::from_millis(40));
        c.logout(true, Duration::from_millis(80));
        c.logout(false, Duration::from_millis(5000));
        let info = RunInfo {
            args: json!({"count": 3}),
            bots: 3,
            window: Duration::from_secs(10),
            ramp_per_sec: 1.0,
            ended: Duration::from_secs(35),
            scenario: "pvp-battle".into(),
            server: Some(json!({"game_address": "10.0.0.5:30810"})),
        };
        Report::build(info, c)
    }

    #[test]
    fn the_report_has_the_documented_shape_and_the_numbers_add_up() {
        let r = fixed();
        let v = r.json();
        assert_eq!(v["schema"], 1);
        assert_eq!(
            (v["run"]["bots"].clone(), v["run"]["window_s"].clone(), v["run"]["duration_s"].clone()),
            (json!(3), json!(10.0), json!(35.0))
        );
        assert_eq!(v["run"]["server"]["game_address"], "10.0.0.5:30810");
        assert_eq!(v["run"]["args"]["count"], 3);
        assert_eq!(v["run"]["build"]["client_version"], "0x10042600");
        // The ramp: three logins and one failure, the last at 4 s, in window 0 only.
        assert_eq!(v["ramp"]["logins_ok"], 3);
        assert_eq!(v["ramp"]["logins_failed"], 1);
        assert_eq!((v["ramp"]["first_login_s"].clone(), v["ramp"]["last_login_s"].clone()), (json!(1.0), json!(4.0)));
        assert_eq!(v["ramp"]["windows"], json!([0]));
        // Four windows: ramp, steady, steady, partial.
        let w = v["windows"].as_array().unwrap();
        assert_eq!(w.len(), 4);
        let flags: Vec<(bool, bool)> = w.iter().map(|x| (x["ramp"].as_bool().unwrap(), x["partial"].as_bool().unwrap())).collect();
        assert_eq!(flags, [(true, false), (false, false), (false, false), (false, true)]);
        assert_eq!(w[3]["seconds"], 5.0);
        assert_eq!(w[1]["start_s"], 10.0);
        // A window's bots and what they did.
        assert_eq!(w[0]["bots"], json!({"connecting": 1, "in_world": 3, "dead": 0, "disconnected": 0, "logins_ok": 3, "logins_failed": 1}));
        assert_eq!(w[2]["bots"]["dead"], 1);
        assert_eq!(w[0]["login_ms"]["n"], 3);
        assert_eq!(w[1]["packets"], json!({"rx": {"packets": 360, "bytes": 6900}, "tx": {"packets": 60, "bytes": 1200}}));
        assert_eq!(w[1]["opcodes"]["0x0501"], json!({"rx_packets": 300, "rx_bytes": 5700, "tx_packets": 60, "tx_bytes": 1200}));
        assert_eq!(w[1]["opcodes"]["0x0502"]["tx_packets"], 0);
        assert_eq!(w[1]["results"], json!({"0": 15, "3": 3, "5": 6}));
        assert_eq!(
            (w[1]["steps"].clone(), w[1]["kills"].clone(), w[1]["deaths"].clone(), w[1]["rebirths"].clone()),
            (json!(60), json!(1), json!(1), json!(1))
        );
        // 360 packets in 10 s by three bots: 12 a second each.
        assert_eq!(w[1]["per_bot"]["rx_packets_per_sec"], 12.0);
        assert_eq!(w[1]["attack_rtt_ms"]["n"], 9);
        assert_eq!(w[1]["attack_rtt_ms"]["max"], 30.0);
        assert_eq!(w[1]["step_echo_ms"]["n"], 6);
        // The summary is the two steady windows: 120 s of bot time, not the ramp's or the partial window's.
        let s = &v["summary"];
        assert_eq!(
            (s["steady"].clone(), s["windows"].clone(), s["from_s"].clone(), s["seconds"].clone()),
            (json!(true), json!([1, 2]), json!(10.0), json!(20.0))
        );
        assert_eq!(s["bots_in_world"], json!({"min": 3.0, "avg": 3.0, "max": 3.0}));
        assert_eq!(s["dead_avg"], 0.5);
        assert_eq!(s["packets"]["rx"]["packets"], 720);
        assert_eq!((s["steps"].clone(), s["kills"].clone(), s["deaths"].clone()), (json!(120), json!(2), json!(2)));
        assert_eq!(s["attack_rtt_ms"]["n"], 18);
        assert_eq!(s["results"], json!({"0": 30, "3": 6, "5": 12}));
        assert_eq!(s["disconnects"], json!({"closed": 0, "timeout": 0, "error": 0}));
        // Logout.
        assert_eq!(
            (v["logout"]["bots"].clone(), v["logout"]["clean"].clone(), v["logout"]["took_ms"]["max"].clone()),
            (json!(3), json!(2), json!(5000.0))
        );
        assert_eq!(v["notes"], json!(["login failed: bot0009: login refused with status 3"]));
        // The windows add up to what the bots counted: 4 windows of three bots.
        assert_eq!(w.iter().map(|x| x["steps"].as_u64().unwrap()).sum::<u64>(), 240);
        assert_eq!(
            w.iter().map(|x| x["kills"].as_u64().unwrap()).sum::<u64>(),
            w.iter().map(|x| x["deaths"].as_u64().unwrap()).sum::<u64>()
        );
    }

    #[test]
    fn a_run_ending_on_a_boundary_folds_its_last_moments_into_the_window_before() {
        let mut c = Collector::default();
        c.login(0, Duration::from_secs(1), Duration::from_millis(10));
        for (index, steps) in [(0, 20), (1, 20), (2, 1)] {
            let mut m = Metrics::default();
            m.steps = steps;
            c.window(index).add_bot(&m, true, None);
        }
        let info = RunInfo {
            args: json!({}),
            bots: 1,
            window: Duration::from_secs(10),
            ramp_per_sec: 1.0,
            ended: Duration::from_millis(20_003),
            scenario: "pvp-battle".into(),
            server: None,
        };
        let v = Report::build(info, c).json();
        let w = v["windows"].as_array().unwrap();
        assert_eq!(w.len(), 2);
        assert_eq!((w[1]["steps"].clone(), w[1]["partial"].clone()), (json!(21), json!(false)));
        assert_eq!(v["run"]["server"], Value::Null);
    }

    #[test]
    fn a_run_without_a_steady_window_says_so_and_still_prints() {
        let mut c = Collector::default();
        c.login(0, Duration::from_secs(8), Duration::from_millis(10));
        c.window(0).add_bot(&Metrics::default(), true, None);
        let info = RunInfo {
            args: json!({}),
            bots: 1,
            window: Duration::from_secs(10),
            ramp_per_sec: 1.0,
            ended: Duration::from_secs(9),
            scenario: "pvp-battle".into(),
            server: None,
        };
        let r = Report::build(info, c);
        assert!(r.summary.windows.is_empty());
        assert_eq!(r.json()["summary"]["steady"], false);
        assert!(r.to_string().contains("no steady-state window"));
    }

    #[test]
    fn a_disconnect_is_counted_in_its_window_and_the_bot_stays_gone() {
        let mut c = Collector::default();
        c.login(0, Duration::from_secs(1), Duration::from_millis(10));
        c.window(0).add_bot(&Metrics::default(), true, None);
        c.window(1).add_bot(&Metrics::default(), true, Some(&LinkError::Closed));
        c.window(2).add_bot(&Metrics::default(), true, Some(&LinkError::Timeout));
        c.window(3).add_bot(&Metrics::default(), true, Some(&LinkError::Error("write: broken pipe".into())));
        let info = RunInfo {
            args: json!({}),
            bots: 1,
            window: Duration::from_secs(10),
            ramp_per_sec: 1.0,
            ended: Duration::from_secs(40),
            scenario: "pvp-battle".into(),
            server: None,
        };
        let v = Report::build(info, c).json();
        let gone: Vec<u64> = v["windows"].as_array().unwrap().iter().map(|w| w["bots"]["disconnected"].as_u64().unwrap()).collect();
        assert_eq!(gone, [0, 1, 2, 3], "the last window of no length folds into the one before");
        assert_eq!(v["summary"]["disconnects"], json!({"closed": 1, "timeout": 1, "error": 1}));
        assert_eq!(v["windows"][1]["bots"]["in_world"], 0);
    }

    #[test]
    fn the_printed_summary_is_one_screen_of_the_steady_windows() {
        // Percentiles come from the histogram's buckets (within 4.4 %): 20 ms reads as 20.3.
        let text = fixed().to_string();
        let expected = "\
bot swarm: 3 bots, pvp-battle, 35.0 s run
ramp: 1 a second, 3 logged in, 1 failed, last login at 4.0 s; login p50 202.1 p95 300.0 max 300.0 ms
steady state: 2 windows of 10 s from 10.0 s (20 s in all)
bots in world: 3 min, 3.0 avg, 3 max; dead 0.5 avg; connecting 0.0 avg; disconnected 0 (closed 0, timeout 0, error 0)
attack round trip: p50 20.3 p95 30.0 p99 30.0 max 30.0 ms (n 18)
skill round trip: p50 - p95 - p99 - max - ms (n 0)
step echo: p50 5.1 p95 15.0 p99 15.0 max 15.0 ms (n 12)
per bot: received 12.0 packets/s (0.2 KB/s), sent 2.0 packets/s (0.0 KB/s)
steps 120, refusals 0, protocol errors 0; kills 2, deaths 2, rebirths 2
attack results: hit 30, miss 6, out of reach 12
logout: 2 of 3 clean, slowest 5000.0 ms
note: login failed: bot0009: login refused with status 3";
        assert_eq!(text, expected, "\n{text}");
        assert!(text.lines().count() <= 24);
    }
}
