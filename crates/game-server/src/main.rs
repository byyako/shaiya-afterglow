use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context as _, Result};
use game_server::config::Config;
use game_server::creation::{CreationData, place_base_items};
use game_server::game_log::{GameLog, Header};
use game_server::pg::{self, PgBackend};
use game_server::server::console;
use game_server::server::{
    Context, GameData, flush_saves, flush_writers, heartbeat, load_closed_quests, load_kill_counts, load_market, load_mob_id,
    load_obelisk_owners, load_open_questions, market_pass, mob_updater, ranking_clock, rules_clock, save_mob_id, save_mob_ids, serve,
    shut_down, start_friends, start_guilds, start_saves,
};
use game_server::supervise::supervise;
use sqlx::PgPool;
use sqlx::postgres::{PgListener, PgPoolOptions};
use tokio::net::TcpListener;

/// A background task as `supervise` starts it.
type Task = fn(Arc<Context<PgBackend>>) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    // `game-server config [--example]` prints the settings and exits (`docs/plans/ext-1-foundation.md` section 7.3).
    let mut args = std::env::args().skip(1);
    if let Some(word) = args.next() {
        match (word.as_str(), args.next().as_deref()) {
            ("config", Some("--example")) => print!("{}", game_server::config::example()),
            ("config", None) => {
                let (_, resolved, _, scripts) = Config::inspect().await?;
                print!("{}", game_server::config::show(&resolved));
                let options = game_server::script::sources::resolve_options(&scripts)?;
                print!("{}", options.show());
                for warning in &options.warnings {
                    eprintln!("warning: {warning}");
                }
            }
            // `game-server rules [--check [file]]` (`docs/plans/ext-2-rules.md` section 4.7).
            ("rules", None) => {
                let (config, _, sources, _) = Config::inspect().await?;
                print!("{}", game_server::rules::show(&game_server::rules::resolve(&config, &sources)?.1));
            }
            ("rules", Some("--check")) => {
                let file = args
                    .next()
                    .or_else(|| std::env::var("GAME_CONFIG").ok().filter(|c| !c.is_empty()))
                    .unwrap_or_else(|| game_server::config::DEFAULT_CONFIG.to_owned());
                match game_server::rules::check_file(&file) {
                    Ok(line) => println!("{line}"),
                    Err(error) => {
                        eprintln!("{error:#}");
                        std::process::exit(1);
                    }
                }
            }
            _ => anyhow::bail!("usage: game-server [config [--example] | rules [--check [file]]]"),
        }
        return Ok(());
    }

    // The settings, from the start-up environment, the config file and the database; the migrations ran on the one connection that
    // read the database layers (`Config::load`), then the pool, whose size is a setting.
    let (config, settings, rule_sources, script_sources) = Config::load().await?;
    for setting in settings.iter().filter(|s| s.value != s.default && s.layer != game_server::config::Layer::Default) {
        tracing::info!(setting = setting.name, value = %setting.value, layer = %setting.layer, "setting not at its default");
    }
    let (rules, _) = game_server::rules::resolve(&config, &rule_sources)?;
    tracing::info!(rules = rules.len(), "rules in force");
    // The options of the extensions' scripts (an error in a layer stops the start); the scripts load once the context exists.
    game_server::script::sources::resolve_options(&script_sources)?;
    let pool =
        PgPoolOptions::new().max_connections(config.db_pool).acquire_timeout(config.db_acquire).connect(&config.database_url).await?;
    let creation = CreationData {
        class_base: pg::load_class_base(&pool).await.context("loading the class table")?,
        start: CreationData::original_start_positions(),
        base_items: place_base_items(pg::load_base_items(&pool).await.context("loading the starting items")?),
    };
    let exp = pg::load_exp(&pool).await.context("loading the experience table")?;
    let items = pg::load_item_defs(&pool).await.context("loading the item definitions")?;
    let status = pg::load_status_table(&pool).await.context("loading the status table")?;
    let mut skills = pg::load_skill_defs(&pool).await.context("loading the skill definitions")?;
    // The extensions' item effects and skill abilities (`docs/plans/ext-3-effects.md` section 3.4), then the `kill_boost_skills`
    // shorthand as abilities too (section 5.4).
    let effects = pg::load_item_effects(&pool, &items).await.context("loading the item effects of the extensions")?;
    let abilities = pg::load_skill_abilities(&pool, &skills).await.context("loading the skill abilities of the extensions")?;
    let ability_levels = skills.attach_abilities(abilities).map_err(|(id, level)| anyhow::anyhow!("skill {id}/{level} is not loaded"))?;
    for (id, level) in skills.attach_kill_boosts(&config.kill_boost_skills) {
        tracing::warn!(skill = id, level, "kill_boost_skills: this skill level has four abilities already and takes no more");
    }
    let by_type: Vec<String> = effects.counts().iter().map(|(ty, n)| format!("{ty} {n}")).collect();
    tracing::info!(effects = effects.len(), types = %by_type.join(", "), ability_levels, "item effects loaded");
    let (live, mob_skills) = pg::load_live(&pool, &items, &skills).await?;
    let mobs = &live.mobs;
    let mut world = pg::load_world(&pool, config.mini_game).await.context("loading the maps")?;
    if let Some(times) = &config.ranking_times {
        tracing::info!(?times, "the ranking battle's open times are set by ranking_times");
        world.set_ranking_times(times.clone());
    }
    let obelisks =
        pg::load_obelisks(&pool).await.context("loading the obelisks (re-run tools/world/import.py if world.obelisks is missing)")?;
    world.set_obelisks(&obelisks, mobs);
    let bosses = pg::load_boss_groups(&pool)
        .await
        .context("loading the boss groups (re-run tools/world/import.py if world.boss_groups is missing)")?;
    world.set_boss_groups(&bosses, mobs);
    let groups = world.boss_groups();
    tracing::info!(
        groups = groups.maps.values().map(Vec::len).sum::<usize>(),
        portals = groups.maps.values().flatten().filter(|g| g.portal.is_some()).count(),
        maps = ?{
            let mut maps: Vec<_> = groups.maps.iter().map(|(&map, g)| (map, g.len())).collect();
            maps.sort_unstable();
            maps
        },
        "boss groups loaded"
    );
    let (mob_ai, failed) =
        pg::load_mob_ai(&pool).await.context("loading the monster AIs (re-run tools/world/import.py if world.mob_ai is missing)")?;
    for (ai, error) in &failed {
        tracing::warn!(ai, error = %error, "monster AI not loaded; its monsters stay ordinary");
    }
    tracing::info!(machines = mob_ai.machines.len(), lines = mob_ai.lines.len(), "monster AIs loaded");
    world.set_mob_ai(mob_ai);
    let lua = pg::load_mob_lua(&pool)
        .await
        .context("loading the Lua monster scripts (re-run tools/world/import.py if world.mob_lua is missing)")?;
    tracing::info!(scripts = lua.0.len(), "Lua monster scripts loaded");
    world.set_mob_lua(lua);
    let quests =
        pg::load_quest_defs(&pool).await.context("loading the quests (re-run tools/world/import.py if world.quests is missing)")?;
    let npcs: usize = world.npcs().values().map(Vec::len).sum();
    tracing::info!(maps = world.map_count(), npcs, skills = skills.len(), mob_skills, quests = quests.len(), "world loaded");

    let listener = TcpListener::bind(config.bind).await?;
    tracing::info!(address = %config.bind, server_id = config.server_id, check_ip = config.check_ip, "game server listening");

    let kill_rows = pg::load_kill_status(&pool)
        .await
        .context("loading the kill-count grades (re-run tools/world/import.py if world.kill_status is missing)")?;
    let kill_grades = game_server::kc::Grades::new(&kill_rows);
    let item_names = pg::load_item_names(&pool).await.context("loading the item names")?;
    let products = pg::load_products(&pool).await.context("loading the item mall's products")?;
    for problem in products.problems(&items) {
        tracing::warn!(%problem, "item mall product");
    }
    tracing::info!(products = products.len(), item_mall = config.item_mall, "item mall loaded");
    let backend = PgBackend::new(pool.clone(), config.delete_delay);
    let data = GameData { creation, exp, world, items, status, live: live.shared(), skills, effects, quests, kill_grades, products };
    // The names of the definitions: the game log writes them into its records, the scripts' `item_name` and `mob_name` read them.
    let names = pg::load_game_log_names(&pool).await.context("loading the names of the game log")?;
    let game_log = if config.game_log {
        tracing::info!(chat = config.game_chat_log, days = config.game_log_days, "game log on");
        Some(GameLog::spawn(pool.clone(), &config, names.clone()))
    } else {
        tracing::info!("game log off (log = false)");
        None
    };
    let mut ctx = Context::new(config, backend, data);
    ctx.set_settings(&settings);
    ctx.set_names(names);
    if let Some(game_log) = game_log {
        ctx.set_game_log(game_log);
    }
    ctx.zones.set_rules(rules);
    let ctx = Arc::new(ctx);
    // The extensions' scripts (`docs/plans/ext-5-lua.md` section 9.1): an extension whose scripts do not load runs without them.
    if ctx.config.scripts {
        let set = game_server::server::script::load_at_start(&ctx, &script_sources)?;
        ctx.start_scripts(set);
    } else {
        tracing::info!("scripts off (scripts = false)");
    }
    load_kill_counts(&ctx).await.context("loading the factions' kill counts")?;
    let mob_id = load_mob_id(&ctx).await.context("loading the monsters' id counter")?;
    tracing::info!(mob_id, "monster id counter loaded");
    let closed = load_closed_quests(&ctx).await.context("loading the closed quests")?;
    tracing::info!(closed, "quests closed by a GM");
    let questions = load_open_questions(&ctx).await.context("loading the open user questions")?;
    tracing::info!(questions, "user questions waiting");
    load_obelisk_owners(&ctx).await.context("loading the obelisks' owners")?;
    let (auctions, item_rows, gold_rows) = load_market(&ctx, item_names).await.context("loading the auction house")?;
    tracing::info!(auctions, item_rows, gold_rows, "auction house loaded");
    let guilds = start_guilds(&ctx).await.context("loading the guilds")?;
    tracing::info!(guilds, "guilds loaded");
    start_friends(&ctx);
    start_saves(&ctx);
    // Each starts again after a panic (`docs/server/audit.md` R5).
    let spawn = |name, make: Task| {
        let ctx = ctx.clone();
        supervise(name, move || make(ctx.clone()))
    };
    spawn("heartbeat", |ctx| Box::pin(heartbeat(ctx)));
    spawn("console countdown", |ctx| Box::pin(console::countdown(ctx)));
    spawn("monster updater", |ctx| Box::pin(mob_updater(ctx)));
    spawn("monster id saver", |ctx| Box::pin(save_mob_ids(ctx)));
    spawn("ranking clock", |ctx| Box::pin(ranking_clock(ctx)));
    spawn("rules clock", |ctx| Box::pin(rules_clock(ctx)));
    spawn("script values writer", |ctx| Box::pin(game_server::server::script::script_writer(ctx)));
    spawn("market pass", |ctx| Box::pin(market_pass(ctx)));
    let notified = ctx.clone();
    supervise("account notifications", move || account_notifications(pool.clone(), notified.clone()));

    // Whatever ends `serve` goes through the shutdown path: everyone is saved and the log flushed (`docs/server/audit.md` R6).
    let mut served = Ok(());
    tokio::select! {
        result = serve(listener, ctx.clone()) => {
            if let Err(error) = &result {
                tracing::error!(%error, "the listener failed; shutting down");
            }
            served = result;
        }
        () = stop_requested() => {}
        () = ctx.quit_now.notified() => tracing::info!("the console's shutdown countdown is over"),
    }
    // No longer accepting; everyone in the world is saved as on a lost connection.
    let players = ctx.players.load(std::sync::atomic::Ordering::Relaxed);
    tracing::info!(players, "shutting down: closing the connections");
    if shut_down(&ctx, SHUTDOWN_GRACE).await {
        tracing::info!("all connections closed");
    } else {
        tracing::warn!("some connections did not close in time; their last changes may be lost");
    }
    flush_saves(&ctx, SHUTDOWN_GRACE).await;
    // The values the scripts set in the last two seconds.
    if !game_server::server::script::flush_script_values(&ctx).await {
        tracing::warn!("the scripts' stored values were not all written");
    }
    if !flush_writers(&ctx, SHUTDOWN_GRACE).await {
        tracing::warn!("the guild or friend changes were not all written in time");
    }
    save_mob_id(&ctx).await;
    ctx.game_log.flush(SHUTDOWN_GRACE).await;
    // A `restart` ends with the status that makes the supervisor start the server again (`docs/plans/done/data-editing.md` section 7).
    if ctx.restart.load(std::sync::atomic::Ordering::Relaxed) {
        tracing::info!(status = game_server::console::RESTART_STATUS, "exiting for a restart");
        std::process::exit(game_server::console::RESTART_STATUS);
    }
    served
}

/// Carries out the console tool's commands (`docs/original/operator-console.md` section 6 step 6), ends an account's older connection here when it logs in again (the login server's notification, the original session
/// server's `0x0802`, `docs/original/client-extras.md` section 3.2), and tells an account's character in the world that its points
/// changed (the operator tool's notification, `docs/original/item-mall.md` deviation 5). Notifications sent while the listener
/// reconnects are lost. The listener has a connection of its own, not one of the pool's (`docs/server/audit.md` R11).
async fn account_notifications(pool: PgPool, ctx: Arc<Context<PgBackend>>) {
    loop {
        let mut listener = match PgListener::connect(&ctx.config.database_url).await {
            Ok(listener) => listener,
            Err(error) => {
                tracing::warn!(%error, "cannot listen for logins; retrying");
                tokio::time::sleep(LISTEN_RETRY).await;
                continue;
            }
        };
        if let Err(error) = listener.listen_all([pg::ACCOUNT_LOGIN_CHANNEL, pg::ACCOUNT_POINTS_CHANNEL, pg::CONSOLE_CHANNEL]).await {
            tracing::warn!(%error, "cannot listen for logins; retrying");
            tokio::time::sleep(LISTEN_RETRY).await;
            continue;
        }
        tracing::info!("listening for logins, point changes and console commands");
        loop {
            match listener.recv().await {
                Ok(notification) if notification.channel() == pg::ACCOUNT_POINTS_CHANNEL => match notification.payload().trim().parse() {
                    Ok(user_id) => {
                        if ctx.online.points_changed(user_id) {
                            tracing::info!(user_id, "the account's points changed: its character is told");
                        }
                    }
                    Err(_) => tracing::warn!(payload = notification.payload(), "unreadable points notification"),
                },
                Ok(notification) if notification.channel() == pg::CONSOLE_CHANNEL => {
                    // Another server's command is ignored; ours is carried out and answered on the reply channel.
                    let payload = notification.payload().to_owned();
                    match pg::parse_console_request(&payload) {
                        Some((server, request, line)) if server == ctx.config.server_id => {
                            let (ctx, pool) = (ctx.clone(), pool.clone());
                            let (request, line) = (request.to_owned(), line.to_owned());
                            tokio::spawn(async move {
                                let lines = console::line(&ctx, &line, "console tool", Header::none(), 0).await;
                                let reply = pg::console_reply(&request, &lines);
                                if let Err(error) =
                                    sqlx::query("SELECT pg_notify($1, $2)").bind(pg::CONSOLE_REPLY_CHANNEL).bind(reply).execute(&pool).await
                                {
                                    tracing::warn!(%error, "cannot answer a console command");
                                }
                            });
                        }
                        Some(_) => {}
                        None => tracing::warn!(payload, "unreadable console command"),
                    }
                }
                Ok(notification) => match pg::parse_account_login(notification.payload()) {
                    Some((user_id, created)) => {
                        if ctx.online.kick_older(user_id, created) {
                            tracing::info!(user_id, "the account logged in again: its connection here ends");
                        }
                    }
                    None => tracing::warn!(payload = notification.payload(), "unreadable login notification"),
                },
                // The next recv() reconnects.
                Err(error) => {
                    tracing::warn!(%error, "login notifications interrupted");
                    tokio::time::sleep(LISTEN_RETRY).await;
                }
            }
        }
    }
}

/// How long to wait before listening for logins again after a failure.
const LISTEN_RETRY: Duration = Duration::from_secs(5);

/// How long the connections get to save and close at shutdown.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(10);

/// Ctrl+C, or SIGTERM (`kill`, `tools/game-server.sh stop`, service managers).
async fn stop_requested() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut terminate) => tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = terminate.recv() => {}
            },
            Err(error) => {
                tracing::warn!(%error, "cannot listen for SIGTERM; only Ctrl+C shuts down cleanly");
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }
    #[cfg(not(unix))]
    let _ = tokio::signal::ctrl_c().await;
}
