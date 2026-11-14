//! The server's background tasks and loaders: the listener, the shutdown, the guild, friend, ranking, heartbeat, market, obelisk
//! and monster tasks, the loads at start, and the save writers.

use super::*;

pub async fn serve<S: Backend>(listener: TcpListener, ctx: Arc<Context<S>>) -> Result<()> {
    let mut failures = 0u32;
    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(accepted) => {
                failures = 0;
                accepted
            }
            Err(error) if listener_is_gone(&error) => return Err(error.into()),
            // Running out of file descriptors or memory, or a connection that reset before it was taken: the server goes on, and
            // waits a little so a persistent error is not a busy loop (`docs/server/audit.md` R6).
            Err(error) => {
                failures = failures.saturating_add(1);
                let wait = accept_backoff(failures);
                tracing::warn!(%error, ?wait, "cannot accept a connection");
                tokio::time::sleep(wait).await;
                continue;
            }
        };
        if !ctx.accepting.load(Ordering::Relaxed) {
            tracing::debug!(%peer, "new connection refused: /server off");
            continue;
        }
        // The console's `maxusers` (`/setmaxuser`): refused while that many connections or more are open.
        let open = ctx.config.max_connections.saturating_sub(ctx.connections.available_permits());
        if open >= ctx.switches.max_users() as usize {
            tracing::warn!(%peer, open, "connection limit reached (maxusers), dropping");
            continue;
        }
        let Ok(permit) = ctx.connections.clone().try_acquire_owned() else {
            tracing::warn!(%peer, "connection limit reached, dropping");
            continue;
        };
        // One address cannot hold the slots (R2); the stream is dropped when it is over its share.
        let guard = match ctx.limits.try_connect(peer.ip()) {
            Ok(guard) => guard,
            Err(reason) => {
                tracing::debug!(%peer, ?reason, "connection refused");
                continue;
            }
        };
        let ctx = ctx.clone();
        tokio::spawn(async move {
            let _permit = permit;
            let _guard = guard;
            match handle(stream, peer, &ctx).await {
                Ok(()) => tracing::debug!(%peer, "closed"),
                Err(error) => tracing::info!(%peer, %error, "dropped"),
            }
        });
    }
}

/// How long to wait after the `failures`th accept error in a row: 100 ms, doubling to 1 s.
pub(super) fn accept_backoff(failures: u32) -> Duration {
    Duration::from_millis(100 << failures.saturating_sub(1).min(4)).min(Duration::from_secs(1))
}

/// An accept error that means the listening socket itself is gone (closed, or not listening), which no wait mends.
pub(super) fn listener_is_gone(error: &std::io::Error) -> bool {
    // EBADF, EINVAL and ENOTSOCK.
    matches!(error.raw_os_error(), Some(9 | 22 | 88))
}

/// Closes every connection as if the client had gone (players in the world leave their zone and are saved, sessions end), then
/// waits up to `grace` for them to finish. Stop accepting first: a connection that starts afterwards is closed at its first read.
/// False when some were still running at the end.
pub async fn shut_down<S>(ctx: &Context<S>, grace: Duration) -> bool {
    ctx.shutdown.send_replace(true);
    let all = ctx.config.max_connections as u32;
    timeout(grace, ctx.connections.acquire_many(all)).await.is_ok_and(|permits| permits.is_ok())
}

/// Writes the player count and liveness every `heartbeat_interval`; the login server lists this server as open
/// only while these arrive.
/// Loads this server's guilds and starts their writer (`docs/original/guild.md` section 15). Returns how many there are.
pub async fn start_guilds<S: Backend>(ctx: &Arc<Context<S>>) -> Result<usize> {
    let mut stored = ctx.store.load_guilds(ctx.config.server_id).await?;
    let (writer, queue) = tokio::sync::mpsc::unbounded_channel();
    // The count fix-ups are written before the writer starts and the server listens, so a member's first bank move cannot race them
    // (`docs/server/audit.md` R29).
    for (guild, slot, count) in crate::guild_bank::check_loaded(&mut stored, &ctx.data.items) {
        let write = crate::backend::GuildWrite::BankCount { guild, slot, count };
        match crate::supervise::retried(&crate::supervise::WRITE_RETRIES, || ctx.store.write_guild(&write)).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => tracing::error!(%error, ?write, "could not write a guild bank count fix; given up"),
            Err(panic) => tracing::error!(%panic, ?write, "writing a guild bank count fix panicked"),
        }
    }
    tracing::info!(items = stored.iter().map(|g| g.bank.len()).sum::<usize>(), "guild bank items loaded");
    let mut guilds = guild::Guilds::load(stored, writer);
    guilds.set_log(ctx.game_log.clone());
    ctx.zones.load_guilds(guilds);
    let writing = ctx.clone();
    tokio::spawn(async move { guild::run_writer(&writing.store, queue).await });
    Ok(ctx.zones.guild_count())
}

/// Starts the friends with their writer (`docs/original/friends.md` section 11).
pub fn start_friends<S: Backend>(ctx: &Arc<Context<S>>) {
    let (writer, queue) = tokio::sync::mpsc::unbounded_channel();
    ctx.zones.load_friends(crate::friends::Friends::new(writer));
    let writing = ctx.clone();
    tokio::spawn(async move { crate::friends::run_writer(&writing.store, queue).await });
}

/// Keeps the rules' hour of the week current (`docs/plans/ext-2-rules.md` section 8, point 4): the clock is read every 30 seconds,
/// never on the hot path.
pub async fn rules_clock<S: Backend>(_ctx: Arc<Context<S>>) {
    loop {
        crate::rules::refresh_hour_of_week();
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}

/// The guild ranking battle's clock (`docs/original/guild-ranking.md` section 3): at every new minute of the local clock, the
/// notice of that minute to everyone, the close (everyone left in the battle map is called out) and, after the delay, the ranking
/// update. An update left due by an earlier run (deviation 4) runs at the first minute.
pub async fn ranking_clock<S: Backend>(ctx: Arc<Context<S>>) {
    use chrono::{Datelike, Timelike};
    use rand::{Rng, SeedableRng};
    let Some(map) = ctx.zones.ranking_map() else { return };
    let times = ctx.data.world.ranking_times();
    let delay = i64::try_from(ctx.config.ranking_update_delay.as_secs()).unwrap_or(i64::MAX);
    match ctx.store.load_ranking_due().await {
        Ok(due) => {
            if let Some(due) = due {
                tracing::info!(due, "a ranking update was left due");
            }
            ctx.zones.set_ranking_due(due);
        }
        Err(error) => tracing::error!(%error, "cannot read when the ranking update is due"),
    }
    let mut rng = rand::rngs::StdRng::from_os_rng();
    loop {
        let now = chrono::Local::now();
        let moment = crate::ranking::Moment {
            weekday: now.weekday().num_days_from_sunday() as u16,
            hour: now.hour() as u16,
            minute: now.minute() as u16,
            date: now.date_naive().num_days_from_ce() as u32,
        };
        let open = crate::ranking::open_tag(&times, moment);
        let was_open = ctx.zones.ranking_open().is_some();
        let step = ctx.zones.ranking_step(open, now.timestamp(), delay);
        if open.is_some() && !was_open {
            tracing::info!(tag = open, "ranking battle open");
        }
        if let Some(kind) = crate::ranking::notice(&times, moment) {
            ctx.zones.send_all(&shaiya_proto::guild::ranking_notice(map, kind));
            tracing::info!(map, kind, "ranking notice");
        }
        match step {
            crate::ranking::Step::Nothing => {
                // Anyone still inside a closed battle map (a logout was pending, or a login there) is called out each minute.
                if open.is_none() {
                    let moved = ctx.zones.ranking_close(&ctx.data.world, &mut || rng.random::<u32>() & 0x7fff);
                    if moved > 0 {
                        tracing::info!(moved, "ranking battle map emptied");
                    }
                }
            }
            crate::ranking::Step::Closed { due } => {
                let moved = ctx.zones.ranking_close(&ctx.data.world, &mut || rng.random::<u32>() & 0x7fff);
                tracing::info!(moved, due, "ranking battle closed");
                ctx.zones.write_ranking_due(Some(due));
            }
            crate::ranking::Step::Update => {
                let placed = ctx.zones.ranking_update();
                for p in &placed {
                    tracing::info!(guild = p.guild, country = p.country, place = p.place, score = p.score, "ranking update: placed");
                }
                tracing::info!(placed = placed.len(), "ranking update");
                ctx.zones.write_ranking_due(None);
            }
        }
        // To the next minute's start, a little after it.
        let into_minute = u64::from(now.second()) * 1000 + u64::from(now.timestamp_subsec_millis().min(999));
        tokio::time::sleep(Duration::from_millis(60_000 - into_minute + 100)).await;
    }
}

/// How often the monsters' object id counter is saved (deviation 2 of `docs/original/quest-drops.md`; the original: every hour).
const MOB_ID_SAVE_EVERY: Duration = Duration::from_secs(60);

/// Continues the monsters' object id counter from its saved value (`CObjectMgr::InitMobID`); returns it.
pub async fn load_mob_id<S: Backend>(ctx: &Context<S>) -> Result<u32> {
    let id = ctx.store.load_mob_id(ctx.config.server_id).await?;
    ctx.zones.set_last_mob_id(id);
    Ok(id)
}

/// Saves the counter now (`CObjectMgr::SaveMobID`), logging a failure.
pub async fn save_mob_id<S: Backend>(ctx: &Context<S>) {
    if let Err(error) = ctx.store.save_mob_id(ctx.config.server_id, ctx.zones.last_mob_id()).await {
        tracing::warn!(%error, "cannot save the monsters' id counter");
    }
}

/// Saves the counter every `MOB_ID_SAVE_EVERY`.
pub async fn save_mob_ids<S: Backend>(ctx: Arc<Context<S>>) {
    let mut tick = tokio::time::interval_at(tokio::time::Instant::now() + MOB_ID_SAVE_EVERY, MOB_ID_SAVE_EVERY);
    loop {
        tick.tick().await;
        save_mob_id(&ctx).await;
    }
}

pub async fn heartbeat<S: Backend>(ctx: Arc<Context<S>>) {
    let mut tick = tokio::time::interval(ctx.config.heartbeat_interval);
    loop {
        tick.tick().await;
        ctx.limits.sweep();
        let players = ctx.players.load(Ordering::Relaxed);
        let online = ctx.online.user_ids();
        if let Err(error) = ctx.store.heartbeat(ctx.config.server_id, players, ctx.accepting.load(Ordering::Relaxed), &online).await {
            tracing::warn!(%error, "heartbeat failed");
        }
    }
}

/// How often monsters fight (`CZone::UpdateMobAttack`, every 500 ms); every second turn they also walk and respawn
/// (`CZone::UpdateMobIdle`) and everyone recovers (`CZone::UpdateApplySkill`).
pub const MOB_UPDATE_INTERVAL: Duration = Duration::from_millis(500);
/// Move ticks in one monster pass with smoothing on.
const FINE_TICKS: u32 = 5;
/// Every sixth turn (3 s) the NPCs walk (`CZone::UpdateNpcIdle`, `docs/original/guards.md` section 4).
const NPC_IDLE_TURNS: u32 = 6;

/// Walks, removes the bodies of, spawns and respawns the monsters of every map (`docs/original/mobs.md` sections 3 and 6,
/// `docs/original/combat.md` section 6); runs the fields every second and the applied skills' effects every three.
/// The saved kill counts into the world (`CWorld::InitKillCount` on ps_dbAgent's `0x0205`).
/// The quests a GM closed (deviation 16 of `docs/original/gm-commands.md`), from the database.
pub async fn load_closed_quests<S: Backend>(ctx: &Context<S>) -> Result<usize> {
    let closed = ctx.store.closed_quests(ctx.config.server_id).await?;
    let mut set = ctx.closed_quests.guard();
    set.extend(closed);
    Ok(set.len())
}

/// The questions still open in the database (deviation 1 of `docs/original/gm-commands-2.md`): all waiting, oldest first.
pub async fn load_open_questions<S: Backend>(ctx: &Context<S>) -> Result<usize> {
    let open = ctx.store.open_questions(ctx.config.server_id).await?;
    let questions: Vec<_> = open
        .into_iter()
        .map(|q| crate::user_questions::Question {
            id: q.id,
            character_id: q.character_id,
            name: q.name.into_bytes(),
            text: q.text,
            time: admin::packed_from_unix(q.asked_unix),
        })
        .collect();
    let count = questions.len();
    *ctx.questions.guard() = crate::user_questions::QuestionQueue::load(questions);
    Ok(count)
}

pub async fn load_kill_counts<S: Backend>(ctx: &Context<S>) -> Result<()> {
    let counts = ctx.store.load_kill_counts(ctx.config.server_id).await?;
    let mut k = ctx.kill_counts.guard();
    let disabled = k.disabled;
    *k = crate::kc::KillCounts::new(ctx.data.kill_grades.clone(), counts, Instant::now());
    k.disabled = disabled;
    tracing::info!(light = counts[0], fury = counts[1], counting = !disabled, "kill counts loaded");
    Ok(())
}

/// The auction house as stored (`docs/original/auction.md` section 15), with the item names its search reads. Returns the counts
/// of auctions, item rows and gold rows.
pub async fn load_market<S: Backend>(ctx: &Context<S>, names: HashMap<(u8, u8), Vec<u8>>) -> Result<(usize, usize, usize)> {
    let stored = ctx.store.load_market(ctx.config.server_id).await?;
    let market = crate::market::Market::new(ctx.config.server_id, names, stored);
    let counts = market.counts();
    *ctx.market.state.guard() = market;
    Ok(counts)
}

/// The auction house's end pass every `crate::market::PASS_EVERY` (section 10): auctions past their end, rows past their deadline.
pub async fn market_pass<S: Backend>(ctx: Arc<Context<S>>) {
    let every = crate::market::PASS_EVERY;
    let mut tick = tokio::time::interval_at(tokio::time::Instant::now() + every, every);
    loop {
        tick.tick().await;
        market::end_pass(&ctx).await;
    }
}

/// Who holds the obelisks, as saved (`CWorld::SetKeepObelisk` on ps_dbAgent's `0x0206`), before they rise.
pub async fn load_obelisk_owners<S: Backend>(ctx: &Context<S>) -> Result<()> {
    let owners = ctx.store.load_obelisk_owners(ctx.config.server_id).await?;
    let set = ctx.zones.set_obelisk_owners(&owners);
    let o = ctx.data.world.obelisks();
    tracing::info!(obelisks = o.maps.values().map(Vec::len).sum::<usize>(), saved = set, worth = ?ctx.obelisk_worth, "obelisks loaded");
    Ok(())
}

/// A save the monster tick asks for, written in order by `start_saves`' task so the tick never waits on the database
/// (`docs/server/audit.md` R11).
pub(super) enum Save {
    ObeliskOwner(crate::obelisk::Owner),
    KillCounts([u32; 2]),
    /// Answered once every save before it is written.
    Flush(tokio::sync::oneshot::Sender<()>),
}

async fn write_save<S: Backend>(store: &S, server_id: u8, save: Save) {
    match save {
        Save::ObeliskOwner(owner) => {
            if let Err(error) = store.save_obelisk_owner(server_id, owner).await {
                tracing::warn!(%error, ?owner, "saving an obelisk's owner failed");
            }
        }
        Save::KillCounts(counts) => {
            if let Err(error) = store.save_kill_counts(server_id, counts).await {
                tracing::warn!(%error, "saving the kill counts failed");
            }
        }
        Save::Flush(done) => {
            let _ = done.send(());
        }
    }
}

/// Starts the task that writes the obelisk owners and kill counts the monster tick hands over, in the order they were asked for.
pub fn start_saves<S: Backend>(ctx: &Arc<Context<S>>) {
    let (sender, mut queue) = tokio::sync::mpsc::unbounded_channel();
    *ctx.saves.guard() = Some(sender);
    let writing = ctx.clone();
    tokio::spawn(async move {
        while let Some(save) = queue.recv().await {
            let monitored = !matches!(save, Save::Flush(_));
            // A panic in one write costs that write, not the queue.
            if let Err(panic) = crate::supervise::catch(write_save(&writing.store, writing.config.server_id, save)).await {
                tracing::error!(%panic, "a save panicked");
            }
            if monitored {
                crate::monitor::counters().save_done();
            }
        }
    });
}

/// Hands a save to the queue, or writes it at once when no queue runs.
async fn save<S: Backend>(ctx: &Context<S>, save: Save) {
    let queue = ctx.saves.guard().clone();
    match queue {
        Some(queue) => {
            let monitored = !matches!(save, Save::Flush(_));
            if queue.send(save).is_ok() && monitored {
                crate::monitor::counters().save_queued();
            }
        }
        None => write_save(&ctx.store, ctx.config.server_id, save).await,
    }
}

/// Waits up to `grace` until the saves asked for so far are written; for the shutdown. False when they were not.
pub async fn flush_saves<S: Backend>(ctx: &Context<S>, grace: Duration) -> bool {
    let (done, written) = tokio::sync::oneshot::channel();
    let queue = ctx.saves.guard().clone();
    match queue {
        Some(queue) if queue.send(Save::Flush(done)).is_ok() => timeout(grace, written).await.is_ok(),
        _ => true,
    }
}

/// Waits up to `grace` until the guild and friend changes queued so far are written; for the shutdown (audit R16). False when they were
/// not.
pub async fn flush_writers<S: Backend>(ctx: &Context<S>, grace: Duration) -> bool {
    let waiting = ctx.zones.flush_writers();
    timeout(grace, wait_all(waiting)).await.is_ok()
}

async fn wait_all(waiting: Vec<tokio::sync::oneshot::Receiver<()>>) {
    for written in waiting {
        let _ = written.await;
    }
}

/// What the obelisks asked for (`docs/original/obelisks.md` section 4): a fall changes the kill counts and goes to the world as
/// `0x2103`; owners are saved.
pub async fn obelisk_events<S: Backend>(ctx: &Context<S>) {
    for event in ctx.zones.take_obelisk_events() {
        match event {
            crate::obelisk::Event::Fell { mob_id, from, to } => {
                let changes = crate::obelisk::capture(&mut ctx.kill_counts.guard(), from, to, ctx.obelisk_worth, Instant::now());
                use crate::obelisk::packet_country;
                ctx.zones.send_all(&shaiya_proto::obelisk::destroyed(mob_id, packet_country(from), packet_country(to)));
                tracing::info!(mob_id, from, to, "obelisk fell");
                kill_count_changes(ctx, changes).await;
            }
            crate::obelisk::Event::Owner(owner) => save(ctx, Save::ObeliskOwner(owner)).await,
        }
    }
}

/// What a kill-count change asks for (`docs/original/pvp-2.md` section 9): a faction whose grade was set again has its players'
/// numbers worked out again (`ZoneEvent::KillGrade`), then `0x020F` goes to that faction's players; the counts are saved. A faction
/// at its top grade resets the obelisks (`docs/original/obelisks.md` section 5).
pub async fn kill_count_changes<S: Backend>(ctx: &Context<S>, changes: crate::kc::Changes) {
    if !changes.any() {
        return;
    }
    if changes.top {
        ctx.zones.reset_obelisks(Instant::now(), &ctx.data.world);
        tracing::info!("obelisks reset");
        // `0x0A01` (`CWorld::AddKillCount`): a country's count reached its top grade; no user columns.
        if let Some(country) = changes.top_by {
            ctx.game_log.action(crate::game_log::keep_top_grade(country));
        }
    }
    let (shown, counts) = {
        let k = ctx.kill_counts.guard();
        ([k.shown(0), k.shown(1)], k.counts())
    };
    for country in 0..2u8 {
        let f = usize::from(country);
        if changes.applied[f] {
            ctx.zones.notify_country(country, &ZoneEvent::KillGrade);
        }
        if changes.applied[f] || changes.sent[f] {
            ctx.zones.send_country(country, None, &w::faction_kill_count(country, shown[f]));
        }
    }
    if changes.save {
        save(ctx, Save::KillCounts(counts)).await;
    }
    tracing::debug!(?changes, ?counts, "kill counts");
}

pub async fn mob_updater<S: Backend>(ctx: Arc<Context<S>>) {
    use rand::{Rng, SeedableRng};
    let mut rng = rand::rngs::StdRng::from_os_rng();
    // With smoothing the updater wakes every 100 ms: the passes on every fifth tick as before, the move tick on the others
    // (`docs/original/chase-smoothing.md` deviation 2).
    let smooth = ctx.config.smooth_chase;
    ctx.zones.set_smooth_chase(smooth);
    let period = if smooth { crate::pacing::TICK } else { MOB_UPDATE_INTERVAL };
    crate::monitor::counters().set_mob_period(period);
    let mut tick = tokio::time::interval(period);
    let mut total = 0;
    let (mut steps, mut ticks) = (0, 0);
    let mut half = false;
    let mut turn = 0;
    let mut fine = 0u32;
    loop {
        tick.tick().await;
        // Timed to the end of the pass, whichever way it ends (`monitor::PassTimer`).
        let _timed = crate::monitor::PassTimer::start();
        let now = Instant::now();
        // One snapshot per pass: a `reload` takes effect with the next.
        let live = ctx.data.live();
        if smooth {
            fine = (fine + 1) % FINE_TICKS;
            if fine != 1 {
                ctx.zones.move_steps(now, &ctx.data.world, &live.mobs);
                continue;
            }
        }
        // Like the C library's rand(): 0 to 32767.
        let mut rand = || rng.random::<u32>() & 0x7fff;
        let loot = (&live.drops, &ctx.data.items);
        ctx.zones.mob_fight(now, &ctx.data.world, &live.mobs, loot, &mut rand);
        // The guards after the monsters; every 3 s all the NPCs (`docs/original/guards.md` section 4).
        ctx.zones.npc_attack(now, &ctx.data.world, &live.mobs, loot, &mut rand);
        turn = (turn + 1) % NPC_IDLE_TURNS;
        if turn == 0 {
            ctx.zones.npc_idle(now, &ctx.data.world, &live.mobs, loot, &mut rand);
        }
        // The pets follow their owners (`docs/plans/ext-4-pets.md` section 5.3); nothing happens without one out.
        ctx.zones.pets(now, &|item_type, type_id, quests| {
            live.drops.quest_start(item_type, type_id) || live.drops.quest_ends(item_type, type_id).iter().any(|q| quests.contains(q))
        });
        // What changed of each party member, for the others (`docs/original/party.md` section 13).
        ctx.zones.party_status();
        obelisk_events(&ctx).await;
        half = !half;
        if half {
            continue;
        }
        steps += ctx.zones.idle(now, &ctx.data.world, &live.mobs, &mut rand);
        ctx.zones.scripts(now, &ctx.data.world, &live.mobs, loot, &mut rand);
        // The Lua monsters' second (`docs/original/instances.md` section 10).
        ctx.zones.lua(now, &ctx.data.world, &live.mobs, loot, &mut rand);
        ctx.zones.fields(now, &ctx.data.world, &live.mobs, loot, &mut rand);
        ctx.zones.recover(now, &live.mobs, loot, &mut rand);
        // The monsters scripts spawned go when their time is over, quietly (the bodies pass takes them away).
        ctx.zones.expire_ext_spawns(now);
        ctx.zones.remove_bodies(now, &live.mobs, &mut rand);
        ctx.zones.expire_items(now);
        ctx.zones.expire_invites(now);
        ctx.zones.expire_trades(now);
        ctx.zones.duel_timers(now);
        ctx.zones.war_timers(now);
        ctx.zones.expire_guilds(now);
        // Empty party instance zones go after their map's ExpireTime (`docs/original/instances.md` section 6).
        ctx.zones.expire_instances(now, &ctx.data.world);
        ticks += 1;
        // The kill counts' ten-minute sends and the end of a top grade (`CWorld::UpdateKillCount`).
        let changes = ctx.kill_counts.guard().tick(now);
        kill_count_changes(&ctx, changes).await;
        // Heals and damage over time, Fire Thorns and Detect every three seconds (`docs/original/skills-special.md` section 7).
        if ticks % 3 == 0 {
            ctx.zones.keep_effects(now, &ctx.data.world, &live.mobs, loot, &mut rand);
        }
        if ticks == 60 {
            tracing::debug!(steps, "monster steps in the last minute");
            (steps, ticks) = (0, 0);
        }
        let spawned = ctx.zones.respawn(now, &ctx.data.world, &live.mobs, &mut rand);
        total += spawned;
        if spawned > 0 {
            tracing::debug!(spawned, total, "monsters spawned");
        }
    }
}

/// The figures of the `stats` line over the time since the last one (this task's or the console's).
pub fn stats_figures<S>(ctx: &Context<S>) -> crate::monitor::Figures {
    let gauges = crate::monitor::Gauges {
        players: u32::from(ctx.players.load(Ordering::Relaxed)),
        connections: ctx.config.max_connections.saturating_sub(ctx.connections.available_permits()),
        game_log_dropped: ctx.game_log.dropped(),
        rss_mb: crate::monitor::rss_mb(),
        workers: tokio::runtime::Handle::try_current().map_or(0, |h| h.metrics().num_workers()),
    };
    ctx.sampler.guard().sample(Instant::now(), crate::monitor::counters(), crate::monitor::locks(), gauges)
}

/// Logs the `stats` line every `stats_every` (`docs/plans/test-bots.md` section 7); `main` starts it when the setting is above 0.
pub async fn stats_line<S: Backend>(ctx: Arc<Context<S>>) {
    let every = ctx.config.stats_every;
    if every.is_zero() {
        return;
    }
    let mut tick = tokio::time::interval_at(tokio::time::Instant::now() + every, every);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tick.tick().await;
        tracing::info!(target: "stats", "{}", stats_figures(&ctx));
    }
}
