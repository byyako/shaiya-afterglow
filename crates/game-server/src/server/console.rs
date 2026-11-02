//! The operator console on the server's side (`docs/original/operator-console.md` sections 4 to 6): the commands of `crate::console`
//! carried out, whether typed in the game by level-1 staff (`server/admin.rs`) or sent by the `console` tool through `NOTIFY`
//! (`main.rs`). Each returns the lines to answer with.

use super::*;
use crate::console::{self, Command, KickBy};
use crate::game_log::Header;
use crate::sync::Relock;

/// A typed command line carried out, or the answer to a wrong one (`console::parse`); `by` names who sent it (for the logs), `header`
/// and `level` are the sender's for the game log (the tool's: `Header::none()`, 0). The answer is one line per element.
pub async fn line<S: Backend>(ctx: &Context<S>, text: &str, by: &str, header: Header, level: u8) -> Vec<String> {
    match console::parse(text) {
        Ok(command) => run(ctx, command, by, header, level, text).await,
        Err(answer) => answer.split('\n').map(str::to_owned).collect(),
    }
}

async fn run<S: Backend>(ctx: &Context<S>, command: Command, by: &str, header: Header, level: u8, line: &str) -> Vec<String> {
    let line = line.trim().trim_start_matches('.');
    tracing::info!(by, command = line, "console command");
    let (word, args) = line.split_once(char::is_whitespace).map_or((line, ""), |(w, a)| (w, a.trim()));
    let word = format!(".{}", word.to_ascii_lowercase());
    ctx.game_log.action(crate::game_log::staff_command(header, level, &word, None, Some(args).filter(|a| !a.is_empty())));
    carry_out(ctx, command).await
}

async fn carry_out<S: Backend>(ctx: &Context<S>, command: Command) -> Vec<String> {
    let s = &ctx.switches;
    match command {
        Command::Help(None) => console::help(),
        Command::Help(Some(word)) => console::help_for(&word).unwrap_or_else(|| vec![format!("unknown command {word}; try help")]),
        Command::Info => vec![info(ctx)],
        Command::Notice(text) => match ap::world_notice(&text) {
            Some(packet) => {
                ctx.zones.send_world(&packet);
                vec!["notice sent".to_owned()]
            }
            None => vec![format!("the notice must be 1 to {} bytes", ap::MAX_WORLD_NOTICE)],
        },
        Command::Quit(minutes) => vec![quit(ctx, console::quit_duration(minutes), false)],
        Command::Restart(minutes) => vec![quit(ctx, console::quit_duration(minutes), true)],
        Command::Reload => vec![reload(ctx).await],
        Command::QuitCancel => vec![quit_cancel(ctx).await],
        Command::AllOut => vec![format!("{} connections closed", ctx.online.kick_all(Kick::Operator { op: 0 }))],
        Command::Close => {
            set_accepting(ctx, false).await;
            vec!["closed: new logins are refused".to_owned()]
        }
        Command::Open => {
            set_accepting(ctx, true).await;
            vec!["open: logins are accepted".to_owned()]
        }
        Command::MaxUsers(n) => {
            s.set_max_users(n);
            let top = ctx.config.max_connections;
            let note = if n as usize > top { format!(" (at most {top} take effect: GAME_MAX_CONNECTIONS)") } else { String::new() };
            vec![format!("maxusers {n}{note}")]
        }
        Command::Kick(by) => vec![kick(ctx, by).await],
        Command::Exp(rate) => {
            s.set_exp_rate(rate);
            vec![format!("exp x{rate:.3}")]
        }
        Command::ExpOff => {
            s.set_exp_rate(1.0);
            vec![format!("exp x{:.3}", 1.0)]
        }
        Command::KillRate(n) => {
            s.set_kill_rate(n);
            vec![format!("killrate {n}")]
        }
        Command::Shop(shop) => {
            s.set_shop(shop);
            vec![format!("shop {}", shop_word(shop))]
        }
        Command::KillCount(on) => {
            ctx.kill_counts.guard().disabled = !on;
            vec![format!("killcount {}", on_off(on))]
        }
        Command::Zones => {
            let (party, guild) = ctx.zones.instance_counts();
            vec![format!("instance zones: party {party}, guild house {guild}, total {}", party + guild)]
        }
        Command::Map(map) => {
            let zones = ctx.zones.map_report(map);
            if zones.is_empty() {
                return vec![format!("map {map}: no zones")];
            }
            zones
                .into_iter()
                .map(|(zone, players, monsters)| format!("map {map} zone {zone}: {players} players, {monsters} monsters"))
                .collect()
        }
    }
}

fn on_off(on: bool) -> &'static str {
    if on { "on" } else { "off" }
}

fn shop_word(shop: u8) -> &'static str {
    match shop {
        switches::SHOP_ON => "on",
        switches::SHOP_OFF => "off",
        _ => "silent",
    }
}

/// `info` (section 5): the numbers of `/si`, `/uc` and `/ticktime`, and the switches, on one line.
fn info<S>(ctx: &Context<S>) -> String {
    let s = &ctx.switches;
    let open = ctx.config.max_connections.saturating_sub(ctx.connections.available_permits());
    let up = ctx.started.elapsed().as_secs();
    format!(
        "players {} ({open}/{}), accepting {}, up {}d {}h {}m, exp x{:.3}, killrate {}, shop {}, killcount {}",
        ctx.players.load(Ordering::Relaxed),
        s.max_users(),
        if ctx.accepting.load(Ordering::Relaxed) { "yes" } else { "no" },
        up / 86400,
        up / 3600 % 24,
        up / 60 % 60,
        s.exp_rate(),
        s.kill_rate(),
        shop_word(s.shop()),
        on_off(!ctx.kill_counts.guard().disabled),
    )
}

/// New connections accepted or refused, and the login server told at once (`/server on`, `/server off`; deviation 12 of
/// `docs/original/gm-commands.md`).
pub(super) async fn set_accepting<S: Backend>(ctx: &Context<S>, on: bool) {
    ctx.accepting.store(on, Ordering::Relaxed);
    // The login server lists the server as closed at once, not at the next heartbeat.
    if let Err(error) = ctx.store.heartbeat(ctx.config.server_id, ctx.players.load(Ordering::Relaxed), on).await {
        tracing::warn!(%error, "heartbeat failed");
    }
}

/// The notice to everyone (`CWorld::SendNotice`).
fn world_notice<S>(ctx: &Context<S>, text: &str) {
    if let Some(packet) = ap::world_notice(text) {
        ctx.zones.send_world(&packet);
    }
}

/// A shutdown counting down (`Context::quit`): when it ends, and whether logins were open before it began.
pub struct Quit {
    ends: Instant,
    was_accepting: bool,
    /// A `restart`: when it ends `Context::restart` is set and the process exits with `console::RESTART_STATUS`.
    restart: bool,
}

/// `quit` and `restart` (section 1.1): logins stop and `countdown` (the task `main` runs) takes the notices to the end.
fn quit<S>(ctx: &Context<S>, total: Duration, restart: bool) -> String {
    let mut running = ctx.quit.guard();
    if running.is_some() {
        return "a shutdown is already counting down".to_owned();
    }
    let was_accepting = ctx.accepting.swap(false, Ordering::Relaxed);
    *running = Some(Quit { ends: Instant::now() + total, was_accepting, restart });
    ctx.quit_start.notify_one();
    format!("{} in {} seconds", if restart { "restarting" } else { "shutting down" }, total.as_secs())
}

/// The task that carries a `quit` to its end, one at a time (section 1.1): the first notice, then "in N seconds" with the pause
/// `next_notice` gives, and at the end `main` is woken to shut down as it does for Ctrl+C.
pub async fn countdown<S: Backend>(ctx: Arc<Context<S>>) {
    loop {
        ctx.quit_start.notified().await;
        let Some((ends, restart)) = ctx.quit.guard().as_ref().map(|q| (q.ends, q.restart)) else { continue };
        world_notice(&ctx, console::QUIT_SHORTLY);
        let ended = loop {
            // A cancel, or another countdown after it, ends this one.
            if ctx.quit.guard().as_ref().is_none_or(|q| q.ends != ends) {
                break false;
            }
            let left = ends.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break true;
            }
            let seconds = left.as_secs() + u64::from(left.subsec_nanos() > 0);
            tracing::info!(players = ctx.players.load(Ordering::Relaxed), seconds, "shutdown countdown");
            world_notice(&ctx, &console::quit_notice(seconds));
            tokio::select! {
                () = tokio::time::sleep(console::next_notice(left).min(left)) => {}
                () = ctx.quit_cancel.notified() => {}
            }
        };
        if ended {
            tracing::info!(restart, "shutdown countdown over: shutting down");
            ctx.restart.store(restart, Ordering::Relaxed);
            ctx.quit_now.notify_one();
        }
    }
}

/// `reload` (`docs/plans/done/data-editing.md` section 7): the monsters and drop tables are read again and put in force together; on any
/// error nothing is swapped. Living monsters keep what they were spawned with (the section 2 table of the plan says per field).
async fn reload<S: Backend>(ctx: &Context<S>) -> String {
    let started = Instant::now();
    match ctx.store.load_live(&ctx.data.skills).await {
        Ok(live) => {
            let (monsters, grades, box_sets) = (live.mobs.len(), live.drops.grade_count(), live.drops.box_count());
            ctx.data.set_live(live);
            let ms = started.elapsed().as_millis();
            tracing::info!(monsters, grades, box_sets, ms, "monsters and drops reloaded");
            format!("reloaded: {monsters} monsters, {grades} drop grades, {box_sets} box sets ({ms} ms)")
        }
        Err(error) => {
            tracing::warn!(error = format!("{error:#}"), "reload failed; nothing changed");
            format!("reload failed, nothing changed: {error:#}")
        }
    }
}

/// `quit cancel` (an addition): the countdown stops, logins are accepted again if they were before.
async fn quit_cancel<S: Backend>(ctx: &Context<S>) -> String {
    let counting = ctx.quit.guard().take();
    let Some(counting) = counting else { return "no shutdown is counting down".to_owned() };
    ctx.quit_cancel.notify_one();
    if counting.was_accepting {
        set_accepting(ctx, true).await;
    }
    world_notice(ctx, console::QUIT_CANCELLED);
    "the shutdown was cancelled".to_owned()
}

/// `kick` (`/kickun`, `/kickuid`, `/kickcn`, `/kickcid`): the account's connection ends as for `/akick` (saved, no packet).
async fn kick<S: Backend>(ctx: &Context<S>, by: KickBy) -> String {
    let not_found = "not found".to_owned();
    let result = match by {
        KickBy::User(name) => ctx.store.account_id(&name).await,
        KickBy::UserId(n) => Ok(Some((n, 0))),
        KickBy::Char(name) => ctx.store.character_account(&name).await,
        KickBy::CharId(id) => match ctx.zones.name_of(id) {
            Some(name) => ctx.store.character_account(&name).await,
            None => Ok(None),
        },
    };
    match result {
        Ok(Some((user, _))) if ctx.online.kick(user, Kick::Operator { op: 0 }) => format!("kicked user {user}"),
        Ok(_) => not_found,
        Err(error) => {
            tracing::warn!(%error, "console kick failed");
            "the database could not be read".to_owned()
        }
    }
}
