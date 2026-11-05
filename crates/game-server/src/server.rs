//! Game connections: the `0xA301` handshake, the character screen (`docs/original/ps_game.md`) and entering the world
//! (`docs/original/enter-world.md`).

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU16, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::sync::{Relock, RelockRw};
use anyhow::{Result, bail};
use shaiya_crypto::{CtrStream, XOR_SEED, XorTable, game_session_keys};
use shaiya_proto::admin as ap;
use shaiya_proto::chat::{self, Text};
use shaiya_proto::combat as cp;
use shaiya_proto::friend as fr;
use shaiya_proto::game::{self as g, create_result, rename_result};
use shaiya_proto::gem;
use shaiya_proto::ground as gp;
use shaiya_proto::guild as gd;
use shaiya_proto::guild_war as gwp;
use shaiya_proto::item as it;
use shaiya_proto::party as pp;
use shaiya_proto::priest;
use shaiya_proto::quest as qp;
use shaiya_proto::shop::{self, BuyRequest, ShopResult};
use shaiya_proto::skill as sk;
use shaiya_proto::trade as tp;
use shaiya_proto::vehicle as vp;
use shaiya_proto::world as w;
use shaiya_proto::{FrameDecoder, frame};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Semaphore, watch};
use tokio::time::timeout;

use crate::admin::Heard;
use crate::backend::{
    Backend, Body, Claim, CreateOutcome, Faction, GuildCreated, ItemUnit, Kept, PlayCharacter, QuestEnd, RenameOutcome, SavePoint,
};
use crate::buffs::{self, BuffEffects, PlayerBuffs};
use crate::combat::{self, Progress};
use crate::config::Config;
use crate::creation::{self, CreationData};
use crate::drops::{DropTables, Loot};
use crate::enter::{self, ExpTable, WorldClock};
use crate::game_log::Revive;
use crate::guild;
use crate::inventory::{BANK, Buy, Inventory, ItemDefs, SlotChange, VISIBLE_SLOTS, Wearer, real_type};
use crate::lapis::OptionSkills;
use crate::limits::{Bucket, Every, FLOOD_GRACE, PacketFlood};
use crate::mobs::MobDefs;
use crate::move_check::{self, MoveCheck};
use crate::quests::{self, EndOutcome, NoReward, QuestDef, QuestDefs, Quests, Refused, Who};
use crate::skills::{self, Aim, Passives, SkillDef, SkillDefs};
use crate::stats::{self, StatusTable, Totals};
use crate::switches::{self, Switches};
use crate::trade::{self, Outcome, Parked, Settled};
use crate::world::{Gate, World};
mod admin;
pub mod console;
mod duel;
mod guild_bank;
mod guild_house;
mod items;
mod lapis;
mod mall;
mod market;
mod mini_game;
mod my_shop;
mod pvp;
mod record;
mod refused;
pub mod script;
mod tasks;
mod vehicles;

use tasks::Save;
pub use tasks::{
    MOB_UPDATE_INTERVAL, flush_saves, flush_writers, heartbeat, kill_count_changes, load_closed_quests, load_kill_counts, load_market,
    load_mob_id, load_obelisk_owners, load_open_questions, market_pass, mob_updater, obelisk_events, ranking_clock, rules_clock, save_mob_id,
    save_mob_ids, serve, shut_down, start_friends, start_guilds, start_saves,
};

use crate::zone::{
    Attack, AttackOutcome, Invitee, NORMAL_CHAT_RANGE, Occupant, SHOUT_RANGE, SkillCast, SkillTarget, Vitals, ZoneEvent, Zones,
};
use items::{ItemMove, cancel_item_move, used_quest_item};
use pvp::{death_counted, kill_counted, lose_to, reward_panel};

/// The original closed a connection after this many packets other than `0xA301` before the handshake.
const MAX_UNDEFINED_PACKETS: u32 = 10;
/// Max grow reported when the account has no faction yet (the original always reported 3).
const DEFAULT_MAX_GROW: u8 = 3;
/// Ultimate mode (`byGrow` 3).
const ULTIMATE: u8 = 3;

/// What the rules test of the player on this connection (`crate::rules::Who`); `country` is its faction.
fn rules_who(character: &PlayCharacter, country: u8) -> crate::rules::Who {
    crate::rules::Who { level: character.level, kills: character.kills[0], country }
}

/// The name the console's answers and the rules' refusals whisper from.
pub(crate) const SERVER_NAME: &[u8] = b"Server";

/// Tables loaded from the game definitions and the world at start-up.
pub struct GameData {
    pub creation: CreationData,
    pub exp: ExpTable,
    pub world: World,
    pub items: ItemDefs,
    pub status: StatusTable,
    /// The monsters and the drop tables, which the console's `reload` swaps while the server runs: read them through `live()`.
    pub live: std::sync::RwLock<Arc<Live>>,
    pub skills: SkillDefs,
    pub quests: QuestDefs,
    /// The factions' kill-count grades (`world.kill_status`, `docs/original/pvp-2.md` section 8).
    pub kill_grades: crate::kc::Grades,
    /// The item mall's products (`defs.product_list`, `docs/original/item-mall.md` section 1).
    pub products: crate::mall::Products,
}

/// What `reload` replaces (`docs/plans/done/data-editing.md` section 7): the monster definitions and the tables of the items each drop
/// grade holds (with the boxes, quest links and sequence groups that use the grades). Whoever uses them takes one snapshot with
/// `GameData::live` and keeps it for the whole operation, so a reload never changes a roll halfway.
#[derive(Debug, Default)]
pub struct Live {
    pub mobs: MobDefs,
    /// The items of each drop grade, from `items` (`crate::drops`).
    pub drops: DropTables,
}

impl Live {
    /// The cell `GameData::live` holds.
    pub fn shared(self) -> std::sync::RwLock<Arc<Live>> {
        std::sync::RwLock::new(Arc::new(self))
    }
}

impl GameData {
    /// The monsters and drop tables now in force.
    pub fn live(&self) -> Arc<Live> {
        self.live.read_guard().clone()
    }

    /// Puts `live` in force; snapshots taken before keep what they have.
    pub fn set_live(&self, live: Live) {
        *self.live.write_guard() = Arc::new(live);
    }
}

pub struct Context<S> {
    pub config: Config,
    pub store: S,
    pub data: GameData,
    /// The server-to-client XOR table, the same for every player (`docs/original/ps_game.md` section 5).
    pub xor: Arc<XorTable>,
    /// Start of the world clock sent in `0x0404`. The original kept that clock in its database; here it starts with the server.
    pub started: Instant,
    /// The players in each map's zone.
    pub zones: Zones,
    pub connections: Arc<Semaphore>,
    /// Connections and new connections per address (`Config::max_per_ip`, `connects_per_minute`).
    pub limits: Arc<crate::limits::IpLimits>,
    /// Authenticated connections, reported in the heartbeat.
    pub players: AtomicU16,
    /// The accounts with a connection on this server, a character staying after its connection ended included (`Online`).
    pub online: OnlineUsers,
    /// Set to true to close every connection the way a lost connection closes (`shut_down`).
    pub shutdown: watch::Sender<bool>,
    /// The factions' kill counts (`CWorld::m_KCInfo`); `main` puts the saved counts in.
    pub kill_counts: Mutex<crate::kc::KillCounts>,
    /// What a capture adds to the taker's kill count and takes off the loser's (`docs/original/obelisks.md` section 1).
    pub obelisk_worth: [u32; 2],
    /// New connections are accepted (`/server on`, `/server off`; deviation 12 of `docs/original/gm-commands.md`).
    pub accepting: AtomicBool,
    /// The quests a GM closed (deviation 16): nobody can start them.
    pub closed_quests: Mutex<HashSet<u16>>,
    /// The users' questions to the staff, open ones (`docs/original/gm-commands-2.md` section 3); `load_open_questions` fills it.
    pub questions: Mutex<crate::user_questions::QuestionQueue>,
    /// The auction house (`docs/original/auction.md`); `load_market` fills it before the listener starts.
    pub market: crate::market::House,
    /// The game log (`docs/original/game-log.md`); `main` starts its writer when `log` is on.
    pub game_log: crate::game_log::GameLog,
    /// What the console turns while the server runs (`docs/original/operator-console.md` sections 2 and 4).
    pub switches: Switches,
    /// The console's shutdown countdown, while one runs (`server/console.rs`): `quit_start` wakes the task that counts, `quit_cancel`
    /// its sleep, and `quit_now` is woken when the countdown has run out: `main` then shuts down as it does for Ctrl+C.
    pub quit: Mutex<Option<console::Quit>>,
    pub quit_start: tokio::sync::Notify,
    pub quit_cancel: tokio::sync::Notify,
    pub quit_now: tokio::sync::Notify,
    /// The queue of the obelisk and kill-count saves once `start_saves` runs; until then they are written where they are asked for.
    saves: Mutex<Option<tokio::sync::mpsc::UnboundedSender<Save>>>,
    /// Tests only: the character whose next ping panics inside a zone lock (`docs/server/audit.md` R5); 0 for none.
    #[doc(hidden)]
    pub test_panic: std::sync::atomic::AtomicU32,
    /// The countdown that ended was a `restart`: `main` exits with `console::RESTART_STATUS` after shutting down.
    pub restart: AtomicBool,
    /// The settings as they were resolved at start, by name and text, and the layer each of the three folded settings came from: what
    /// `reload` compares the config file with, and puts on the rules the settings make (`docs/plans/ext-2-rules.md` section 4.6).
    pub settings_in_force: Vec<(String, String)>,
    pub rule_setting_layers: Vec<(String, String)>,
    /// The names of the items, monsters and skills, for the scripts' `item_name` and `mob_name` (`main` loads them).
    pub names: crate::game_log::Names,
    /// The context itself, weakly, once `attach_scripts` ran: the world the scripts act on (`server/script.rs`).
    script_me: std::sync::OnceLock<std::sync::Weak<Context<S>>>,
}

impl<S> Context<S> {
    /// Starts recording to `log` (the zones write their own records to it too).
    pub fn set_game_log(&mut self, log: crate::game_log::GameLog) {
        self.zones.set_game_log(log.clone());
        self.game_log = log;
    }

    /// The names of the definitions the scripts' `item_name` and `mob_name` answer with.
    pub fn set_names(&mut self, names: crate::game_log::Names) {
        self.names = names;
    }

    /// Remembers the settings as resolved at start (see `settings_in_force`).
    pub fn set_settings(&mut self, resolved: &[crate::config::Resolved]) {
        self.settings_in_force = resolved.iter().map(|r| (r.name.to_owned(), r.value.clone())).collect();
        self.rule_setting_layers = resolved
            .iter()
            .filter(|r| matches!(r.name, "cross_faction_trade_maps" | "market_maps" | "trade_chat_min_level"))
            .map(|r| (r.name.to_owned(), r.layer.to_string()))
            .collect();
    }

    pub fn new(config: Config, store: S, data: GameData) -> Self {
        let connections = Arc::new(Semaphore::new(config.max_connections));
        let limits = crate::limits::IpLimits::new(config.max_per_ip, config.connects_per_minute);
        let xor = Arc::new(XorTable::new(&XOR_SEED));
        let zones = Zones::new(&data.world, &data.live().mobs);
        zones.load_item_skills(&data.skills);
        // The rules the settings make; `main` puts the layers' rules in force over them.
        zones.set_rules(crate::rules::resolve(&config, &crate::rules::RuleSources::default()).map(|r| r.0).unwrap_or_default());
        let mut kill_counts = crate::kc::KillCounts::new(data.kill_grades.clone(), [0; 2], Instant::now());
        kill_counts.disabled = !config.kill_count;
        let switches = Switches::new(config.exp_rate, config.kill_rate, config.item_mall, config.max_connections as u32);
        let obelisk_worth = crate::obelisk::worth(&data.kill_grades, data.world.obelisks().total);
        Self {
            config,
            store,
            data,
            xor,
            started: Instant::now(),
            zones,
            connections,
            limits,
            players: AtomicU16::new(0),
            online: OnlineUsers::default(),
            shutdown: watch::Sender::new(false),
            kill_counts: Mutex::new(kill_counts),
            obelisk_worth,
            accepting: AtomicBool::new(true),
            closed_quests: Mutex::default(),
            questions: Mutex::default(),
            market: crate::market::House::default(),
            game_log: crate::game_log::GameLog::off(),
            switches,
            quit: Mutex::new(None),
            quit_start: tokio::sync::Notify::new(),
            quit_cancel: tokio::sync::Notify::new(),
            quit_now: tokio::sync::Notify::new(),
            saves: Mutex::new(None),
            test_panic: std::sync::atomic::AtomicU32::new(0),
            restart: AtomicBool::new(false),
            settings_in_force: Vec::new(),
            rule_setting_layers: Vec::new(),
            names: Default::default(),
            script_me: std::sync::OnceLock::new(),
        }
    }
}

struct Connection {
    stream: TcpStream,
    decoder: FrameDecoder,
    recv: Option<CtrStream>,
    send: Option<CtrStream>,
    /// Set once the character is in the world: server-to-client packets are XORed instead of AES-encrypted, and the AES
    /// send counter stands still (the original's `m_bInitGame`).
    xor: Option<Arc<XorTable>>,
    /// The server is shutting down when this turns true; the connection then ends as if the client had closed it.
    shutdown: watch::Receiver<bool>,
    /// The connection has ended while the character stays in the world (`in_world`): nothing is sent any more.
    closed: bool,
    /// Turns true when a newer login of the account ends this connection (`OnlineUsers::kick_older`).
    kick: Option<watch::Receiver<Option<Kick>>>,
    /// Ended by a newer login: reads as closed from then on.
    kicked: bool,
    /// `Config::kick_notice`.
    kick_notice: Duration,
    /// After an operator's kick or ban notice: the socket stays open until then (`hold_notice`).
    hold_until: Option<Instant>,
    /// When the client last sent bytes; `next_packet` idles out `idle` after this, however many other things woke the loop in
    /// between (`docs/server/audit.md` R19).
    last_read: Instant,
    /// `Config::write_timeout`.
    write_timeout: Duration,
    /// The packets the client sends against `Config::packets_per_sec` (R4).
    flood: PacketFlood,
    peer: SocketAddr,
}

/// Why an account's connection is ended from outside (`OnlineUsers::kick`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kick {
    /// A newer login: `0x0004 0`, then the connection is closed.
    NewLogin,
    /// An operator's `/akick` or `/kick`: the opcode is sent first (0: closed with no packet, as the original does for an account
    /// that is not in the world).
    Operator { op: u16 },
    /// An account ban (`F70A` / `F805`, deviation 18 of `docs/original/gm-commands.md`): the ban notice with its minutes (0 = no end).
    Banned { op: u16, minutes: u16 },
}

/// What ended a wait for the client.
enum Wake {
    Stop,
    Kick(Kick),
    Read(usize),
}

/// What `next_packet` found.
enum Next {
    Packet(Vec<u8>),
    /// The client closed the connection, the server is shutting down or a newer login has ended it already.
    Closed,
    /// Ended from outside. The caller performs `kick_out` where nothing can cancel it (R19).
    Kick(Kick),
}

impl Connection {
    async fn send(&mut self, mut packet: Vec<u8>) -> Result<()> {
        if self.closed {
            return Ok(());
        }
        if let Some(table) = &self.xor {
            table.apply(&mut packet[2..]);
        } else if let Some(cipher) = &mut self.send {
            cipher.apply(&mut packet[2..]);
        }
        self.write(&packet).await
    }

    /// Writes with a deadline (`Config::write_timeout`; R1). Expiry is an error, handled as a lost connection. A kick or the
    /// shutdown while the write waits abandons it and closes the socket: a connection that is ending has nothing more to say, and
    /// what follows reads as closed.
    async fn write(&mut self, bytes: &[u8]) -> Result<()> {
        enum Outcome {
            Written(std::io::Result<()>),
            TimedOut,
            Abandoned { kicked: bool },
        }
        let outcome = tokio::select! {
            // The write first: a kick that was already waiting does not cut a write that goes through at once.
            biased;
            written = timeout(self.write_timeout, self.stream.write_all(bytes)) => match written {
                Ok(written) => Outcome::Written(written),
                Err(_) => Outcome::TimedOut,
            },
            _ = self.shutdown.wait_for(|stop| *stop) => Outcome::Abandoned { kicked: false },
            _ = kicked(&mut self.kick) => Outcome::Abandoned { kicked: true },
        };
        match outcome {
            Outcome::Written(written) => Ok(written?),
            Outcome::TimedOut => bail!("the client did not take a packet in {:?}", self.write_timeout),
            Outcome::Abandoned { kicked } => {
                tracing::debug!(kicked, "a write was abandoned: the connection is ending");
                self.closed = true;
                if kicked {
                    self.kicked = true;
                    self.kick = None;
                }
                let _ = timeout(CLOSE_WAIT, self.stream.shutdown()).await;
                Ok(())
            }
        }
    }

    /// Next decrypted packet; `Closed` when the client closed the connection, the server is shutting down or a newer login ended
    /// it. A kick comes back as `Next::Kick` and is not carried out here: the caller's `select!` may drop this future at any
    /// await, and a kick cut in the middle of its write would send half a packet.
    async fn next_packet(&mut self, idle: Duration) -> Result<Next> {
        if self.kicked {
            return Ok(Next::Closed);
        }
        let mut buffer = [0u8; 2048];
        loop {
            match self.decoder.next_frame() {
                Ok(Some(mut packet)) => {
                    if let Some(cipher) = &mut self.recv {
                        cipher.apply(&mut packet[2..]);
                    }
                    if self.flood.packet(Instant::now()) {
                        tracing::warn!(peer = %self.peer, "the client has sent more than its packets a second for {:?}: disconnected", FLOOD_GRACE);
                        bail!("too many packets");
                    }
                    return Ok(Next::Packet(packet));
                }
                Ok(None) => {}
                Err(error) => bail!("invalid packet: {error:?}"),
            }
            let wake = tokio::select! {
                // First, so a client that keeps sending cannot hold the connection open.
                biased;
                _ = self.shutdown.wait_for(|stop| *stop) => Wake::Stop,
                kick = kicked(&mut self.kick) => Wake::Kick(kick),
                read = tokio::time::timeout_at((self.last_read + idle).into(), self.stream.read(&mut buffer)) => Wake::Read(read??),
            };
            let read = match wake {
                Wake::Stop | Wake::Read(0) => return Ok(Next::Closed),
                Wake::Kick(kick) => return Ok(Next::Kick(kick)),
                Wake::Read(read) => read,
            };
            self.last_read = Instant::now();
            self.decoder.push(&buffer[..read]);
        }
    }

    /// `next_packet` for the handshake and the character screen, where the wait is not inside a `select!`: a kick is carried out
    /// here. `None` when the connection is over.
    async fn read_packet(&mut self, idle: Duration) -> Result<Option<Vec<u8>>> {
        match self.next_packet(idle).await? {
            Next::Packet(packet) => Ok(Some(packet)),
            Next::Closed => Ok(None),
            Next::Kick(kick) => {
                self.kick_out(kick).await;
                Ok(None)
            }
        }
    }
}

impl Connection {
    /// Ended by a newer login (`docs/original/client-extras.md` deviation 2): `0x0004` 0 under the current encryption ("disconnect
    /// due to using logged in ID.", and the client quits), then the sending side shut down. Nothing is sent or read afterwards.
    async fn kick_out(&mut self, kick: Kick) {
        let packet = match kick {
            Kick::NewLogin => Some(g::kicked(g::KICKED_LOGGED_IN)),
            Kick::Operator { op: 0 } => None,
            Kick::Operator { op } => Some(shaiya_proto::admin::bare(op)),
            Kick::Banned { op, minutes } => Some(shaiya_proto::admin::ban_notice(op, minutes)),
        };
        let notice = packet.is_some() && kick != Kick::NewLogin;
        // Not watched any more: the notice itself must not be cut by the kick it announces.
        self.kick = None;
        if let Some(packet) = packet
            && let Err(error) = self.send(packet).await
        {
            tracing::debug!(%error, "could not send the kick");
        }
        // An operator's notice is shown in the chat window, which the disconnect box would hide: the socket stays open a while
        // (`docs/original/small-items.md` deviation 1).
        if notice && !self.kick_notice.is_zero() {
            self.hold_until = Some(Instant::now() + self.kick_notice);
        } else {
            let _ = self.stream.shutdown().await;
        }
        self.closed = true;
        self.kicked = true;
    }

    /// After an operator's notice, the character out of the world: what the client still sends, dropped until the hold ends or the
    /// client closes, then the sending side shut down.
    async fn hold_notice(&mut self) {
        let Some(until) = self.hold_until.take() else { return };
        let mut buffer = [0u8; 2048];
        let _ = tokio::time::timeout_at(until.into(), async { while matches!(self.stream.read(&mut buffer).await, Ok(1..)) {} }).await;
        let _ = self.stream.shutdown().await;
    }

    /// After a kick, before the socket closes: what the client still sends, dropped until it closes or `QUIT_GRACE` ends, so the
    /// close does not turn into a reset that could lose the `0x0004`.
    async fn drain(&mut self) {
        let mut buffer = [0u8; 2048];
        let _ = timeout(QUIT_GRACE, async { while matches!(self.stream.read(&mut buffer).await, Ok(1..)) {} }).await;
    }
}

/// A line said and relayed: its chat log record and the copy for a GM listening to the speaker (`SendChatLogA`/`B` and
/// `SendListenChat`, side by side at each kind of line in `CUser::PacketChatE` and `PacketAdminChat`). `target` is a whisper's.
fn said<S>(ctx: &Context<S>, character: &PlayCharacter, state: &WorldState, heard: Heard, target: Option<&[u8]>, text: &[u8]) {
    let target_name = target.map(|t| String::from_utf8_lossy(t).trim_end_matches('\0').to_owned());
    record::chat(ctx, character, state, heard.chat_type(), target_name.as_deref(), text);
    ctx.zones.send_listener(character.id, || ap::listen_chat(character.name.as_bytes(), heard.kind(), target, text));
}

/// A line with a Message To Server armed: `0x1108` to every player in a zone, the sender included, and no `0x1101` to anyone
/// (`CWorld::PSendAll`, `docs/original/small-items.md` section 5.2); the item is disarmed.
fn all_server_chat<S: Backend>(ctx: &Context<S>, state: &mut WorldState, id: u32, name: &[u8], text: &[u8]) {
    state.chat_item_armed = false;
    ctx.zones.send_all(&chat::by_name(chat::OP_ALL_SERVER, name, text));
    tracing::info!(id, text = %String::from_utf8_lossy(text.strip_suffix(&[0]).unwrap_or(text)), "all-server chat");
}

/// Waits until a newer login ends the connection; forever without a kick to wait for (or once its sender is gone).
async fn kicked(kick: &mut Option<watch::Receiver<Option<Kick>>>) -> Kick {
    if let Some(kick) = kick
        && let Ok(reason) = kick.wait_for(Option::is_some).await
        && let Some(reason) = *reason
    {
        return reason;
    }
    std::future::pending().await
}

/// An authenticated player on the character screen.
struct Player {
    user_id: u32,
    identity: [u8; 16],
    /// The staff level now (1 to 10 staff, 11 a player); `F601` and `F602` change it for this connection.
    privilege: AtomicU8,
    faction: Option<Faction>,
    /// The list has been sent, so a character may be selected (`m_bEnableDBCharacterSelect`).
    can_select: bool,
}

/// Counts the player for the heartbeat while it lives.
struct PlayerCount<'a>(&'a AtomicU16);

impl<'a> PlayerCount<'a> {
    fn new(count: &'a AtomicU16) -> Self {
        count.fetch_add(1, Ordering::Relaxed);
        Self(count)
    }
}

impl Drop for PlayerCount<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

/// The accounts with a connection on this server (`Online`), each with the means to end it for a newer login.
#[derive(Default)]
pub struct OnlineUsers {
    users: Mutex<HashMap<u32, OnlineUser>>,
}

struct OnlineUser {
    /// `Claimed::created` of its session, once claimed.
    created: Option<i64>,
    kick: watch::Sender<Option<Kick>>,
    /// Its character's events while it is in the world, and what the item mall has for it there.
    world: Option<tokio::sync::mpsc::UnboundedSender<ZoneEvent>>,
    mall: MallInbox,
}

/// What the item mall has for an account in the world (`docs/original/item-mall.md` deviations 5 and 6): gifts that came, and
/// whether its points changed.
#[derive(Debug, Default)]
pub struct MallInbox {
    pub gifts: Vec<ArrivedGift>,
    pub points: bool,
}

/// A gift stored for the account: who sent it, the product, its storage rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArrivedGift {
    pub sender: String,
    pub code: String,
    pub rows: Vec<crate::backend::StoredPointItem>,
}

impl OnlineUsers {
    /// A login made at `created` (the login server's notification, `docs/original/client-extras.md` section 3.2): ends the
    /// account's connection here when its session is older. True when there was one to end.
    pub fn kick_older(&self, user_id: u32, created: i64) -> bool {
        match self.users.guard().get(&user_id) {
            Some(u) if u.created.is_some_and(|c| c < created) => {
                u.kick.send_replace(Some(Kick::NewLogin));
                true
            }
            _ => false,
        }
    }

    /// Ends the account's connection here for a reason (`/akick`, a ban). True when there was one.
    pub fn kick(&self, user_id: u32, kick: Kick) -> bool {
        match self.users.guard().get(&user_id) {
            Some(u) => {
                u.kick.send_replace(Some(kick));
                true
            }
            None => false,
        }
    }

    /// Ends every account's connection here (the console's `allout`); how many there were.
    pub fn kick_all(&self, kick: Kick) -> usize {
        let users = self.users.guard();
        for u in users.values() {
            u.kick.send_replace(Some(kick));
        }
        users.len()
    }

    /// Whether the account has a connection here.
    pub fn contains(&self, user_id: u32) -> bool {
        self.users.guard().contains_key(&user_id)
    }

    /// The account's character is in the world, with its events (from the burst until it leaves).
    fn enter_world(&self, user_id: u32, events: tokio::sync::mpsc::UnboundedSender<ZoneEvent>) {
        if let Some(u) = self.users.guard().get_mut(&user_id) {
            u.world = Some(events);
        }
    }

    /// The character left the world: the gifts it was not told of yet.
    fn leave_world(&self, user_id: u32) -> Vec<ArrivedGift> {
        match self.users.guard().get_mut(&user_id) {
            Some(u) => {
                u.world = None;
                std::mem::take(&mut u.mall).gifts
            }
            None => Vec::new(),
        }
    }

    /// A gift for the account's character in the world; given back when it is not in the world (a notice is due instead).
    pub fn deliver_gift(&self, user_id: u32, gift: ArrivedGift) -> Result<(), ArrivedGift> {
        let mut users = self.users.guard();
        let Some(u) = users.get_mut(&user_id) else { return Err(gift) };
        match &u.world {
            Some(events) if events.send(ZoneEvent::Mall).is_ok() => {
                u.mall.gifts.push(gift);
                Ok(())
            }
            _ => Err(gift),
        }
    }

    /// The account's points changed (the operator tool): its character in the world sends them again. False when not in the world.
    pub fn points_changed(&self, user_id: u32) -> bool {
        let mut users = self.users.guard();
        let Some(u) = users.get_mut(&user_id) else { return false };
        match &u.world {
            Some(events) if events.send(ZoneEvent::Mall).is_ok() => {
                u.mall.points = true;
                true
            }
            _ => false,
        }
    }

    /// What the item mall has for the account, taken.
    fn take_mall(&self, user_id: u32) -> MallInbox {
        self.users.guard().get_mut(&user_id).map(|u| std::mem::take(&mut u.mall)).unwrap_or_default()
    }
}

/// Marks an account as having a connection on this server while it lives: the original refuses a second `0xA301` for a user
/// still in its world (`CWorld::FindUserBill`, `docs/original/world.md` section 4.3).
struct Online<'a> {
    users: &'a OnlineUsers,
    user_id: u32,
}

impl<'a> Online<'a> {
    /// The account's mark and the receiver of its kick. When the account still has a connection here, waits up to `wait` for it
    /// to go (`docs/original/client-extras.md` deviation 3); `None` when it is still there then.
    async fn claim(users: &'a OnlineUsers, user_id: u32, wait: Duration) -> Option<(Self, watch::Receiver<Option<Kick>>)> {
        let deadline = tokio::time::Instant::now() + wait;
        loop {
            // A waiter listens to its own account's entry, taken under the lock that shows it there: its kick channel closes when
            // the entry goes, so a connection that goes in between is not missed and no other account's waiter wakes
            // (`docs/server/audit.md` R9). A kick wakes it too, and it goes on waiting.
            let mut gone = {
                let mut online = users.users.guard();
                match online.entry(user_id) {
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        let (kick, kicked) = watch::channel(None);
                        entry.insert(OnlineUser { created: None, kick, world: None, mall: MallInbox::default() });
                        return Some((Self { users, user_id }, kicked));
                    }
                    std::collections::hash_map::Entry::Occupied(entry) => entry.get().kick.subscribe(),
                }
            };
            let left = async { while gone.changed().await.is_ok() {} };
            if tokio::time::timeout_at(deadline, left).await.is_err() {
                return None;
            }
        }
    }

    /// The claimed session's `created`, from which on a newer login's notification ends this connection.
    fn set_created(&self, created: i64) {
        if let Some(u) = self.users.users.guard().get_mut(&self.user_id) {
            u.created = Some(created);
        }
    }
}

impl Drop for Online<'_> {
    fn drop(&mut self) {
        self.users.users.guard().remove(&self.user_id);
    }
}

async fn handle<S: Backend>(stream: TcpStream, peer: SocketAddr, ctx: &Context<S>) -> Result<()> {
    let handshake_due = Instant::now() + ctx.config.handshake_timeout;
    stream.set_nodelay(true)?;
    let mut conn = Connection {
        stream,
        decoder: FrameDecoder::new(),
        recv: None,
        send: None,
        xor: None,
        shutdown: ctx.shutdown.subscribe(),
        closed: false,
        kick: None,
        kicked: false,
        kick_notice: ctx.config.kick_notice,
        hold_until: None,
        last_read: Instant::now(),
        write_timeout: ctx.config.write_timeout,
        flood: PacketFlood::new(ctx.config.packets_per_sec, Instant::now()),
        peer,
    };

    // The whole handshake within the timeout, counted from the accept: pings and the like do not extend it (R2). It includes the wait
    // for the account's older connection to go (`Config::handshake_wait`), which fits inside it.
    // A session the handshake claimed is released when the handshake then fails or runs out of time (R29): the slot says which.
    let claimed = Mutex::new(None);
    let handshake = tokio::time::timeout_at(handshake_due.into(), handshake(&mut conn, peer, ctx, &claimed));
    let outcome = handshake.await.map_err(|_| anyhow::anyhow!("the handshake took longer than {:?}", ctx.config.handshake_timeout));
    let (mut player, online) = match outcome {
        Ok(Ok(Some(entered))) => entered,
        other => {
            let session = claimed.guard().take();
            if let Some((user_id, identity)) = session
                && let Err(error) = ctx.store.release(user_id, identity).await
            {
                tracing::warn!(%error, user_id, "could not end the session of a failed handshake");
            }
            return other.and_then(|handshaken| handshaken).map(|_| ());
        }
    };
    let _counted = PlayerCount::new(&ctx.players);
    let result = async {
        loop {
            let Some((character, buffs, quests)) = character_screen(&mut conn, peer, ctx, &mut player).await? else { return Ok(()) };
            match in_world(&mut conn, peer, ctx, &player, character, buffs, quests).await? {
                // As after the original's database reload: the faction and the list again, under AES.
                Left::ToSelect => send_list(&mut conn, ctx, &mut player, true).await?,
                Left::Gone => return Ok(()),
            }
        }
    }
    .await;
    if let Err(error) = ctx.store.release(player.user_id, player.identity).await {
        tracing::warn!(%error, user_id = player.user_id, "could not end the session");
    }
    tracing::info!(%peer, user_id = player.user_id, "left");
    // The account is free for the newer login before the kicked client's socket is drained.
    drop(online);
    if conn.kicked {
        conn.hold_notice().await;
        conn.drain().await;
    }
    result
}

/// Waits for `0xA301` (plain text), checks the session and starts encryption. `None` when the client left or was refused;
/// a refused handshake gets no reply, as in the original. The account counts as connected while the `Online` lives.
async fn handshake<'c, S: Backend>(
    conn: &mut Connection,
    peer: SocketAddr,
    ctx: &'c Context<S>,
    claimed_session: &Mutex<Option<(u32, [u8; 16])>>,
) -> Result<Option<(Player, Online<'c>)>> {
    let mut undefined = 0;
    loop {
        let Some(packet) = conn.read_packet(ctx.config.handshake_timeout).await? else { return Ok(None) };
        match frame::opcode(&packet) {
            g::OP_PING => continue,
            g::OP_DISCONNECT => return Ok(None),
            g::OP_GAME_CONNECT => {}
            opcode => {
                undefined += 1;
                tracing::debug!(%peer, opcode = format_args!("{opcode:#06x}"), "packet before the handshake");
                if undefined > MAX_UNDEFINED_PACKETS {
                    bail!("too many packets before the handshake");
                }
                continue;
            }
        }
        let Some((user_id, identity)) = g::parse_game_connect(frame::payload(&packet)) else {
            bail!("malformed handshake");
        };
        // The session is looked at first, without taking it: a handshake that does not hold a session of this account neither takes
        // the account's slot nor waits for it (`docs/server/audit.md` R9).
        let claim = Claim { user_id, identity, server_id: ctx.config.server_id, ip: ctx.config.check_ip.then_some(peer.ip()) };
        if !ctx.store.claimable(&claim).await? {
            tracing::info!(%peer, user_id, "handshake refused: no matching session (wrong identity, server or address, expired, or already used)");
            return Ok(None);
        }
        // Before the session is claimed, so a refused handshake leaves it unused. An older connection a newer login has ended
        // is waited for while its character stays (deviation 3 of docs/original/client-extras.md).
        let Some((online, kick)) = Online::claim(&ctx.online, user_id, ctx.config.handshake_wait).await else {
            tracing::info!(%peer, user_id, "handshake refused: the account is still connected to this server");
            return Ok(None);
        };
        let Some(claimed) = ctx.store.claim(&claim).await? else {
            tracing::info!(%peer, user_id, "handshake refused: no matching session (wrong identity, server or address, expired, or already used)");
            return Ok(None);
        };
        *claimed_session.guard() = Some((user_id, identity));
        online.set_created(claimed.created);
        conn.kick = Some(kick);
        let keys = game_session_keys(&claimed.keys);
        conn.recv = Some(CtrStream::new(&keys.key, &keys.iv));
        conn.send = Some(CtrStream::new(&keys.key, &keys.iv));
        conn.send(g::game_connect_response(&XOR_SEED)).await?;
        // The account's premium and its end as a packed date (`docs/original/item-mall.md` deviation 4); none: 0, 0.
        let now = chrono::Utc::now().timestamp();
        let kind = claimed.premium.kind_at(now);
        let until = claimed.premium.until.filter(|_| kind != 0).map_or(0, crate::admin::packed_from_unix);
        conn.send(g::billing_info(kind, until)).await?;
        tracing::info!(%peer, user_id, "entered the character screen");

        let faction = ctx.store.faction(user_id, ctx.config.server_id).await?;
        let mut player = Player { user_id, identity, privilege: AtomicU8::new(claimed.privilege), faction, can_select: false };
        send_list(conn, ctx, &mut player, true).await?;
        return Ok(Some((player, online)));
    }
}

/// The character list as the original sends it: optionally the faction first, then every empty slot, then every character.
async fn send_list<S: Backend>(conn: &mut Connection, ctx: &Context<S>, player: &mut Player, with_country: bool) -> Result<()> {
    let characters = ctx.store.characters(player.user_id, ctx.config.server_id).await?;
    if characters.is_empty() {
        // Deleting the last character may have cleared the faction.
        player.faction = ctx.store.faction(player.user_id, ctx.config.server_id).await?;
    }
    if with_country {
        let (country, max_grow) = match player.faction {
            Some(f) => (f.country, f.max_grow),
            None => (g::COUNTRY_NONE, DEFAULT_MAX_GROW),
        };
        conn.send(g::char_country(country, max_grow)).await?;
    }
    for slot in 0..g::SLOTS {
        if !characters.iter().any(|c| c.slot == slot) {
            conn.send(g::char_list_empty(slot)).await?;
        }
    }
    for character in &characters {
        conn.send(g::char_list_entry(character)).await?;
    }
    player.can_select = true;
    Ok(())
}

/// The character screen until the player leaves (`None`) or enters the world with a character, its applied skills and its quests.
async fn character_screen<S: Backend>(
    conn: &mut Connection,
    peer: SocketAddr,
    ctx: &Context<S>,
    player: &mut Player,
) -> Result<Option<(PlayCharacter, PlayerBuffs, Quests)>> {
    let server_id = ctx.config.server_id;
    let user_id = player.user_id;
    loop {
        let Some(packet) = conn.read_packet(ctx.config.idle_timeout).await? else { return Ok(None) };
        let payload = frame::payload(&packet);
        let opcode = frame::opcode(&packet);
        match opcode {
            g::OP_PING => {}
            g::OP_DISCONNECT | g::OP_CHAR_LEAVE => return Ok(None),

            g::OP_CHAR_COUNTRY => {
                let Some(country) = g::parse_char_country(payload) else { bail!("invalid faction") };
                // The original accepted a change at any time; here only while the account has no characters on this server.
                if ctx.store.choose_faction(user_id, server_id, country).await? {
                    player.faction = ctx.store.faction(user_id, server_id).await?;
                    tracing::info!(%peer, user_id, country, "faction chosen");
                } else {
                    tracing::info!(%peer, user_id, country, "faction change refused: the account has characters");
                }
            }

            g::OP_CHAR_CREATE => {
                let Some(request) = g::parse_char_create(payload) else { bail!("malformed create request") };
                tracing::debug!(%peer, user_id, sent_len = request.sent_len, "create request");
                let result = match creation::new_character(user_id, server_id, player.faction, &request, &ctx.data.creation) {
                    Err(reason) => {
                        tracing::info!(%peer, user_id, %reason, "create refused");
                        create_result::FAILED
                    }
                    Ok(character) => match ctx.store.create(&character).await? {
                        CreateOutcome::Created(id) => {
                            tracing::info!(%peer, user_id, id, name = %character.name, "character created");
                            create_result::SUCCESS
                        }
                        other => {
                            tracing::info!(%peer, user_id, name = %character.name, ?other, "create refused");
                            create_result::FAILED
                        }
                    },
                };
                conn.send(g::char_create_response(result)).await?;
                if result == create_result::SUCCESS {
                    send_list(conn, ctx, player, false).await?;
                }
            }

            g::OP_CHAR_DELETE | g::OP_CHAR_RESTORE => {
                let Some(id) = g::parse_char_id(payload) else { bail!("malformed request") };
                if player.faction.is_none() {
                    tracing::debug!(%peer, user_id, "delete or restore without a faction");
                    continue;
                }
                let done = if opcode == g::OP_CHAR_DELETE {
                    ctx.store.request_delete(user_id, server_id, id).await?
                } else {
                    ctx.store.restore(user_id, server_id, id).await?
                };
                tracing::info!(%peer, user_id, id, done, delete = opcode == g::OP_CHAR_DELETE, "delete or restore");
                // A deleted character leaves its guild (`CGuildMgr::DeleteCharGuildUser`, `docs/original/guild.md` section 10).
                if done && opcode == g::OP_CHAR_DELETE {
                    ctx.zones.guild_deleted(id);
                }
                // The original answers only when it worked.
                if done {
                    conn.send(g::char_success(opcode, id)).await?;
                }
            }

            g::OP_CHAR_SELECT => {
                let Some(id) = g::parse_char_id(payload) else { bail!("malformed select") };
                // The original allows one select per list, and none without a faction.
                let Some(faction) = player.faction else { continue };
                if !std::mem::take(&mut player.can_select) {
                    continue;
                }
                let Some(mut character) = ctx.store.load(user_id, server_id, id).await? else {
                    tracing::info!(%peer, user_id, id, "select refused: no such live character");
                    continue;
                };
                // On a map with a CreateType the character starts at the map's bind position for its country
                // (`CUser::PacketUserDBChar`, `docs/original/instances.md` section 6): there is no zone to come back to.
                if ctx.data.world.create_type(character.map) != 0 {
                    use rand::Rng;
                    let mut rng = rand::rng();
                    let (map, position) =
                        ctx.data.world.bind_pos(character.map, faction.country, character.position, &mut || rng.random::<u32>() & 0x7fff);
                    tracing::info!(%peer, id, from = character.map, to = map, ?position, "start moved off a map with a CreateType");
                    (character.map, character.position) = (map, position);
                }
                let clock = WorldClock { world_seconds: ctx.started.elapsed().as_secs() as u32, packed_time: local_packed_time() };
                // The saved applied skills, their effects in the numbers of the burst (`CUser::InitApplySkill`).
                let now = Instant::now();
                let buffs = PlayerBuffs {
                    low_hp: low_hp_passive(ctx, &character),
                    // CUser::BillingApply, once at entry (`docs/original/item-mall.md` section 7).
                    premium: character.mall.premium.kind_at(chrono::Utc::now().timestamp()),
                    ..PlayerBuffs::load(&character.applied, &ctx.data.skills, character.offline_secs, character.map, now, &mut || {
                        ctx.zones.new_key()
                    })
                };
                let passives = with_buffs(skills::passives(&character.skills, &ctx.data.skills), &buffs.effects());
                let grade = ctx.kill_counts.guard().status(faction.country);
                let totals = totals(ctx, &character, &character.items, &passives, &grade);
                // The guild list, members and join requests go before 0x0105, which carries the guild's name.
                let guild = ctx.zones.guild_burst(character.id, faction.country);
                // The quests in progress (their time left running from now) and the finished ones (`docs/original/quest.md` section 10).
                let (quests, unknown) = Quests::load(&character.quests, &ctx.data.quests, now);
                if !unknown.is_empty() {
                    tracing::warn!(%peer, id, ?unknown, "quests the definitions lack were left out");
                }
                let mut lists = qp::active_list(&quests.list_entries(now));
                lists.extend(qp::finished_list(&quests.finished().collect::<Vec<_>>()));
                // Then the friend and block lists (`docs/original/friends.md` section 6).
                lists.extend(ctx.zones.friend_burst(character.id, &character.friends, &character.blocks));
                let applied = buffs.burst_entries(now);
                let kill_count = ctx.kill_counts.guard().status_packet_values(faction.country, Instant::now());
                for packet in
                    enter::burst(&character, faction.country, kill_count, &ctx.data.exp, &clock, &totals, &applied, &lists, &guild)
                {
                    conn.send(packet).await?;
                }
                // Everything after 0x010B is XORed; the client switches in its 0x010B handler.
                conn.xor = Some(ctx.xor.clone());
                tracing::info!(%peer, user_id, id, name = %character.name, items = character.items.len(), applied = buffs.list.len(), "entering the world");
                return Ok(Some((character, buffs, quests)));
            }

            // A rename allowed by a Name Change Stone (`docs/original/small-items.md` section 9.4, deviations 3 to 5): the creation
            // name rule, a live character of this account with a rename left that is not still in the world, a free name. One at a
            // time: the next request is read after the answer.
            g::OP_CHAR_RENAME => {
                let Some((id, name)) = g::parse_char_rename(payload) else { bail!("malformed rename") };
                let outcome = if !crate::creation::name_allowed(&name) {
                    None
                } else if ctx.zones.locate(id).is_some() {
                    tracing::info!(%peer, user_id, id, "rename refused: the character is still in the world");
                    None
                } else {
                    Some(ctx.store.rename(user_id, server_id, id, &String::from_utf8_lossy(&name)).await?)
                };
                match outcome {
                    Some(RenameOutcome::Renamed { old }) => {
                        let new = String::from_utf8_lossy(&name);
                        ctx.zones.guild_renamed(id, &name);
                        ctx.market.state.guard().renamed(id, &new);
                        // `0x0A05` (`CUser::PacketUserDBChar`): the account, the character and its old name only; the account's name is
                        // not known on the character screen (empty).
                        let header =
                            crate::game_log::Header { user_id, character_id: id, character_name: old.clone(), ..Default::default() };
                        ctx.game_log.action(crate::game_log::rename(header, &old, &new));
                        tracing::info!(%peer, user_id, id, %old, %new, "renamed");
                        conn.send(g::char_rename_response(rename_result::SUCCESS)).await?;
                    }
                    other => {
                        tracing::info!(%peer, user_id, id, name = %String::from_utf8_lossy(&name), ?other, "rename refused");
                        conn.send(g::char_rename_response(rename_result::FAILED)).await?;
                    }
                }
            }

            // Anything else in the character family closes the connection, as in the original (including 0x0119);
            // other families are ignored on this screen.
            opcode if opcode & 0xFF00 == 0x0100 => bail!("unexpected opcode {opcode:#06x} on the character screen"),
            opcode => tracing::debug!(%peer, opcode = format_args!("{opcode:#06x}"), "ignored on the character screen"),
        }
    }
}

/// How a player left the world.
enum Left {
    /// Logged out to the character screen; the connection stays open.
    ToSelect,
    /// Quit or lost the connection.
    Gone,
}

/// Longest horizontal step the original accepts in one `0x0501` (`SVector::Distance2`).
const MAX_STEP: f32 = 35.0;
/// Time between two shouts (`0x1107`) of one player.
const SHOUT_COOLDOWN: Duration = Duration::from_secs(30);
/// Time between two all-server lines sent as `0x1108` (`+0x5844`; the US client never sends it, `docs/original/small-items.md`
/// section 5.2).
const ALL_CHAT_COOLDOWN: Duration = Duration::from_secs(30);
/// The Priest's reading costs this, and needs more (`10 < gold`).
const PRIEST_PRICE: u32 = 10;
/// How long to wait for the client to close after `0x010D`.
const QUIT_GRACE: Duration = Duration::from_secs(10);
/// How long closing the sending side of a socket may take before it is left to the drop.
const CLOSE_WAIT: Duration = Duration::from_secs(1);

/// One gate for each log line a player can trigger.
struct LogGates {
    /// A line of chat dropped for the bucket: one a minute.
    chat_dropped: Every,
    refused_step: Every,
    unsafe_markup: Every,
    unhandled: Every,
}

impl LogGates {
    fn new() -> Self {
        let every = |secs| Every::new(Duration::from_secs(secs));
        Self { chat_dropped: every(60), refused_step: every(10), unsafe_markup: every(10), unhandled: every(10) }
    }
}

/// What the world keeps about the player while it is in it (`docs/original/world.md`).
struct WorldState {
    /// The character is staff (privilege 1 to 10; at entering, and after each `F601`), and its switches (`/char`, `/attack`, `/pvpon`):
    /// both kept through map changes, as the original keeps them on the `CUser` until the next login.
    staff: bool,
    /// The privilege byte, for the game log's staff records and the console's level check: at entering, and after each `F601`.
    staff_level: u8,
    gm: crate::admin::Flags,
    /// A move ban is on (`FA0F`): the move speed is 255.
    move_banned: bool,
    /// A `/xcall` move waiting for its time: when, the zone and the spot.
    called: Option<(Instant, u16, [f32; 3])>,
    /// The notices the staff member repeats (`F907` to `F90B`); they end with the connection.
    notices: crate::admin::Notices,
    /// Placed in the zone of `map`. Cleared by a move to another map until the client has loaded it and sent `0x0201`.
    entered: bool,
    /// The gambling house game the player sits at (`CUser+0x1760`, `docs/original/mini-game.md` section 1): out of the zone while it is open.
    mini_game: Option<crate::mini_game::MiniGame>,
    map: u16,
    position: [f32; 3],
    dir: u16,
    motion: u8,
    /// A logout request: `0x0107` or `0x010D`, and when it takes effect.
    logout: Option<(u16, Instant)>,
    /// How other players' actions reach this connection; a copy goes to the zone on each entry.
    outbox: crate::zone::Outbox,
    /// When the player may shout again.
    next_shout: Option<Instant>,
    /// When the player may use trade chat again.
    next_trade: Option<Instant>,
    /// The rules that whispered a refusal to the player lately: rule id and when, at most 8, the oldest dropped (`refused::due`).
    told: Vec<(u32, Instant)>,
    /// A Message To Server is armed: the next normal line goes to everyone (`m_bUseChatItem`; lost by leaving the world, kept over
    /// map changes).
    chat_item_armed: bool,
    /// When a client-sent `0x1108` may go out again.
    next_all_chat: Option<Instant>,
    /// A Name Change Stone was used since entering the world (`+0x58dc`).
    rename_stone_used: bool,
    /// The money and the bags, changed by shops and saved with every change.
    money: u32,
    /// The money at entering the world (the game log's `firstmoney`).
    first_money: u32,
    inventory: Inventory,
    /// Max HP/SP/MP, stats and speeds with the current equipment, passive and applied skills (`crate::stats`).
    totals: Totals,
    /// What the passive skills add (`crate::skills`), recomputed when a skill is learnt; and with the applied skills' bonuses.
    skill_passives: Passives,
    passives: Passives,
    /// What the applied skills add up to, as the zone last reported it (`crate::buffs`).
    effects: BuffEffects,
    /// The applied skills while the player is not in a zone; the zone keeps them while it is.
    buffs: PlayerBuffs,
    /// How the zone reaches this connection besides packets; a copy goes to the zone on each entry.
    events: tokio::sync::mpsc::UnboundedSender<ZoneEvent>,
    /// What the player attacks or aims a skill at (`m_Target`): `None` is status 0, otherwise status 2, or 3 while casting.
    target: Option<SkillTarget>,
    /// The monster attacked before a skill on oneself, attacked again after it (`m_TargetPrev`).
    prev_target: Option<SkillTarget>,
    /// What the faction's kill-count grade gives (`docs/original/pvp-2.md` section 10), as of entering or the last change.
    grade: [i32; crate::kc::TYPES],
    /// Attacking with normal blows, as `0x0503` starts (`m_nAttackMode`): they go on after a skill.
    attack_mode: bool,
    /// The skill to use at the next attack time, by number (`m_nAttackNext` 2, `m_nAttackSkill`); `None` for a normal blow.
    next_skill: Option<u8>,
    /// Casting that skill (status 3).
    casting: bool,
    /// When the next attack or skill is due, or a cast ends (`m_dwNextAttackTime`).
    next_attack: Instant,
    /// When each learnt skill's cooldown ends (`CSkill::m_dwResetTime`), by skill id.
    cooldowns: HashMap<u16, Instant>,
    /// When the client last reported a step (`m_dwPrevMoveTime`).
    last_move: Option<Instant>,
    /// The move speed check (`stUserMoveChk`, `docs/original/move-check.md`): started again wherever the server places the player.
    move_check: MoveCheck,
    /// HP, MP and SP while the player is not in a zone; the zone keeps them while it is (`docs/original/mob-fight.md`).
    health: [u16; 3],
    /// Dead, and when it comes back to life by itself (`CUser::UpdateRebirth`, 200 s).
    dead: bool,
    rebirth_due: Option<Instant>,
    /// The rebirth due is the party's: next to the leader (`0x0553` kind 3, `docs/original/party.md` section 11).
    rebirth_party: bool,
    /// Nothing is lost at rebirth (`m_nDropExp` −1: a duel's death, `docs/original/duel.md` section 6.1).
    keep_exp: bool,
    /// Otherwise the percent lost at rebirth, fixed at the death (`m_nDropExp`, `docs/original/item-flags.md` section 3).
    drop_exp: u8,
    /// Fallen (an ultimate-mode death without a rune, deviation 2 there): on its way to the character screen, never back to life
    /// (the original's `+0x1430` 2).
    fallen: bool,
    /// Running rather than walking (`m_bRun`, `0x0210`).
    run: bool,
    /// What a disguise copies and an illusion's monster, chosen with the skill's target (`docs/original/skills-special.md` section 5).
    look: Option<w::UserShape>,
    illusion_mob: u16,
    /// The connection has ended and the character stays in the world until then (`Config::linger`; `CWorld::LeaveUser`,
    /// `docs/original/world.md` section 4.3).
    linger_until: Option<Instant>,
    /// The speeds the view was last told of (`0x051C`).
    sent_speeds: (u8, u8),
    /// The quests in progress and finished (`crate::quests`).
    quests: Quests,
    /// When each item reuse group is ready (`docs/original/item-use.md` section 2), the quest item last used and until when it
    /// counts, a move by item under way, and when a summon waiting for this player runs out.
    reuse: crate::item_use::ReuseTimes,
    quest_item: Option<(u8, Instant)>,
    item_move: Option<ItemMove>,
    party_call_due: Option<Instant>,
    /// When the last skill request came (`0x0511`, `0x0517`): the skill-cut fix refuses moves by item for 3 s after it.
    last_skill_request: Option<Instant>,
    /// Which armour piece the next blow wears (`m_nQualityDec`), and the equipment slots worn since their quality was last saved
    /// (`docs/original/repair.md` sections 2 and 3).
    armour_wear: crate::repair::ArmourWear,
    unsaved_wear: std::collections::BTreeSet<u8>,
    /// Mounts (`docs/original/vehicles.md`): when the summon completes, when the next summon is allowed (`m_dwNextVehicleTime`), and
    /// the mount ridden into the next zone after a map change.
    summon_due: Option<Instant>,
    next_summon: Option<Instant>,
    mount: Option<crate::zone::Mount>,
    /// The auction search and its page (`SearchInfo`), dropped once the character is off map 42; the interest list, `None` while
    /// the character has none (`docs/original/auction.md` sections 2.6 and 2.7).
    market_search: Option<crate::market::Search>,
    interest: Option<Vec<u32>>,
    /// The account's points and storages as this connection knows them (`docs/original/item-mall.md`).
    mall: mall::MallState,
    /// The chat channels without a cooldown share one bucket per connection (`docs/server/audit.md` R4).
    chat_bucket: Bucket,
    /// Log lines a client can make as often as it likes, one per kind per interval (R4, R18).
    logs: LogGates,
    /// Which events of the extensions' scripts were raised for this character (`server/script.rs`).
    script: script::Track,
}

impl WorldState {
    /// The monster attacked before a skill on oneself: a buff's aggro goes to it.
    fn prev_mob(&self) -> Option<u32> {
        match self.prev_target {
            Some(SkillTarget::Mob(mob)) => Some(mob),
            _ => None,
        }
    }

    /// Attack and move speed with what debuffs hold (`docs/original/mob-skills.md` section 8; 255: cannot).
    fn speeds(&self) -> (u8, u8) {
        let (attack, movement) = self.effects.speeds(self.totals.speeds());
        (attack, if self.move_banned { buffs::CANNOT } else { movement })
    }
}

/// After the enter-world burst: wait for the client's `0x0201` (sent once it has loaded the map), place the player and stay in the
/// world until it logs out or the connection ends. The position is saved however it ends.
async fn in_world<S: Backend>(
    conn: &mut Connection,
    peer: SocketAddr,
    ctx: &Context<S>,
    player: &Player,
    mut character: PlayCharacter,
    buffs: PlayerBuffs,
    quests: Quests,
) -> Result<Left> {
    let (outbox, packets) = crate::outbox::channel(ctx.config.outbox_limit);
    let (events, zone_events) = tokio::sync::mpsc::unbounded_channel();
    let mut inboxes = Inboxes { packets, events: zone_events };
    let skill_passives = skills::passives(&character.skills, &ctx.data.skills);
    let effects = buffs.effects();
    let passives = with_buffs(skill_passives, &effects);
    let grade = ctx.kill_counts.guard().status(player.faction.map_or(0, |f| f.country));
    let totals = totals(ctx, &character, &character.items, &passives, &grade);
    let mut inventory = Inventory::new(&character.items).with_bank(&character.bank);
    if let Some(bags) = passives.bags {
        inventory.set_max_bag(bags);
    }
    let staff = player.privilege.load(Ordering::Relaxed) <= enter::STAFF_PRIVILEGE_MAX;
    let mut state = WorldState {
        staff,
        staff_level: player.privilege.load(Ordering::Relaxed),
        gm: crate::admin::Flags::on_entering(staff),
        move_banned: ctx.zones.bans_of(character.id).moves.is_some(),
        called: None,
        notices: crate::admin::Notices::default(),
        entered: false,
        mini_game: None,
        map: character.map,
        position: character.position,
        dir: character.dir as u16,
        motion: w::MOTION_STAND,
        logout: None,
        outbox,
        next_shout: None,
        next_trade: None,
        told: Vec::new(),
        chat_item_armed: false,
        next_all_chat: None,
        rename_stone_used: false,
        money: character.money,
        first_money: character.money,
        inventory,
        totals,
        skill_passives,
        passives,
        effects,
        buffs,
        events,
        target: None,
        prev_target: None,
        grade,
        attack_mode: false,
        next_skill: None,
        casting: false,
        next_attack: Instant::now(),
        // The saved cooldowns run on from now (`docs/original/skills.md` section 2).
        cooldowns: character
            .skills
            .iter()
            .filter(|s| s.delay > 0)
            .map(|s| (s.id, Instant::now() + Duration::from_secs(u64::from(s.delay))))
            .collect(),
        last_move: None,
        move_check: MoveCheck::new(Instant::now(), character.position),
        // Loaded and kept within the maxima (CUser::PacketUserDBChar 0x407).
        health: [0, 1, 2].map(|i| character.health[i].min(totals.max_health()[i])),
        dead: false,
        rebirth_due: None,
        rebirth_party: false,
        keep_exp: false,
        drop_exp: DEATH_EXP_PERCENT,
        fallen: false,
        run: true,
        look: None,
        illusion_mob: 0,
        linger_until: None,
        sent_speeds: (0, 0),
        quests,
        reuse: Default::default(),
        quest_item: None,
        item_move: None,
        party_call_due: None,
        last_skill_request: None,
        armour_wear: Default::default(),
        unsaved_wear: Default::default(),
        summon_due: None,
        next_summon: None,
        mount: None,
        market_search: None,
        interest: Some(character.interest.clone()).filter(|list| !list.is_empty()),
        mall: mall::MallState::new(&character.mall),
        chat_bucket: Bucket::new(ctx.config.chat_per_sec, ctx.config.chat_burst, Instant::now()),
        logs: LogGates::new(),
        script: script::Track { id: character.id, name: character.name.clone(), ..Default::default() },
    };
    sync_quests(ctx, character.id, &state.quests);
    // The guilds know the player from now on (`CGuild::ConnectUser`, `docs/original/guild.md` section 10).
    let penalty_left = character.guild_joined_secs.map(|secs| crate::guild::JOIN_PENALTY.saturating_sub(Duration::from_secs(secs)));
    ctx.zones.guild_connect(
        character.id,
        crate::guild::Player {
            outbox: state.outbox.clone(),
            events: state.events.clone(),
            name: character.name.clone().into_bytes(),
            country: player.faction.map_or(0, |f| f.country),
            job: character.job,
            level: character.level,
            grow: character.grow,
            penalty_until: penalty_left.filter(|left| !left.is_zero()).map(|left| Instant::now() + left),
        },
    );
    // So do the friends (`CUser::InitFriend`, `docs/original/friends.md` section 6).
    ctx.zones.friend_connect(
        character.id,
        crate::friends::Player::new(
            state.outbox.clone(),
            character.name.clone().into_bytes(),
            player.faction.map_or(0, |f| f.country),
            character.job,
            character.level,
            &character.friends,
            &character.blocks,
        ),
    );
    // `0x0306` and `0x030F` (`CUser::EnterWorld`).
    record::entered(ctx, &character, &state, peer.ip());
    // Gifts reach the account from now on; those that came while it was away are told (`docs/original/item-mall.md` deviation 6).
    let mut panicked = false;
    let result = match mall::enter(conn, ctx, player.user_id, &state).await {
        Ok(()) => guarded_world_loop(conn, peer, ctx, player, &mut character, &mut state, &mut inboxes, &mut panicked).await,
        Err(error) => Err(error),
    };
    // A connection that ended in the world with no logout pending: the character stays a while (10 s), with nothing sent, as in
    // the original. Not when it is dead (nothing left to happen to it) nor when the server shuts down.
    // An operator's kick or ban takes the character out at once: the notice hold keeps only the socket.
    // Not after a panic: the state may be half-changed, and the cleanup below is what a lost connection gets, minus the stay.
    let ended = matches!(result, Ok(Left::Gone) | Err(_)) && conn.hold_until.is_none() && !panicked;
    if ended && state.entered && !state.dead && state.logout.is_none() && !*ctx.shutdown.borrow() {
        // Closed at once when the server ended it (a fault); the character stays regardless.
        let _ = conn.stream.shutdown().await;
        conn.closed = true;
        state.linger_until = Some(Instant::now() + ctx.config.linger);
        tracing::info!(%peer, id = character.id, linger = ?ctx.config.linger, "connection ended in the world; the character stays");
        if let Err(error) = guarded_world_loop(conn, peer, ctx, player, &mut character, &mut state, &mut inboxes, &mut panicked).await {
            tracing::warn!(%peer, %error, id = character.id, "error while the character stayed");
        }
    }
    // Whatever lies on a gambling table goes back to the money before the character is saved (`CWorld::LeaveUser`, `GameExit`).
    mini_game::stand_for_leaving(ctx, &character, &mut state).await;
    mall::leave(ctx, player.user_id).await;
    // Normally done before the logout's last packet; this covers a lost connection.
    ctx.zones.cancel_actions(character.id);
    ctx.zones.party_leave_world(character.id);
    ctx.zones.forget_quests(character.id);
    ctx.zones.guild_leave_world(character.id);
    ctx.zones.war_left_world(state.map, character.id);
    ctx.zones.friend_leave_world(state.map, character.id);
    ctx.zones.forget_party_call(character.id);
    ctx.zones.forget_listens(character.id);
    // A staff member's held question goes back to the front of the queue (`CUser::LeaveWorld`).
    if state.staff {
        admin::leave_questions(ctx, character.id).await;
    }
    if let Some(left) = ctx.zones.leave(state.map, character.id)
        && let Err(error) = duel::left_zone(ctx, character.id, &mut state, left).await
    {
        tracing::warn!(%error, id = character.id, "could not put the duel stakes back");
    }
    // Out of the world for the scripts: `map_leave` and `logout` are queued, never run here (section 4.3).
    script::gone(ctx, &mut state);
    if state.dead
        && !state.fallen
        && let Err(error) = leave_dead(ctx, player, &mut character, &mut state).await
    {
        tracing::warn!(%error, id = character.id, "could not bring the character back to life on leaving");
    }
    // The records the zone and the guilds made for this character while it left (the party, the guild), before its last ones.
    // A party drop handed to this member meanwhile goes on the ground, not into the void (audit R14).
    while let Ok(event) = inboxes.events.try_recv() {
        match event {
            ZoneEvent::Record(mut row) => {
                row.header = record::header(&character, &state);
                ctx.game_log.action(*row);
            }
            event => {
                if let Some((loot, position, owner, party, dropped)) = left_on_the_ground(event, state.position, character.id) {
                    ctx.zones.drop_on_ground(state.map, loot, position, owner, party, Instant::now(), gp::KIND_PUT_BACK, dropped);
                }
            }
        }
    }
    // `0x0307` and `0x0310` (`CUser::LeaveWorld`).
    record::left(ctx, &character, &state);
    // Never on the way into an instance zone after this.
    ctx.zones.unbook(character.id);
    if let Err(error) =
        ctx.store.save_position(character.id, crate::world::map_of(state.map), state.position, state.dir as i16, state.health).await
    {
        tracing::warn!(%error, id = character.id, "could not save the position");
    }
    // The equipment's wear (`CUser::SendDBAgentCharGetInfo`, `docs/original/repair.md` section 3).
    let wear = take_unsaved_wear(&mut state);
    if !wear.is_empty() {
        match ctx.store.trade(character.id, 0, &wear).await {
            Ok(Some(_)) => {}
            Ok(None) => tracing::warn!(id = character.id, ?wear, "the database refused the equipment's wear"),
            Err(error) => tracing::warn!(%error, id = character.id, "could not save the equipment's wear"),
        }
    }
    // What is left of each cooldown, in whole seconds; it stands still until the character comes back.
    let now = Instant::now();
    for skill in &mut character.skills {
        let left = state.cooldowns.get(&skill.id).map_or(Duration::ZERO, |end| end.saturating_duration_since(now));
        skill.delay = left.as_secs().min(u64::from(u16::MAX)) as u16;
    }
    if let Err(error) = ctx.store.save_cooldowns(character.id, &character.skills).await {
        tracing::warn!(%error, id = character.id, "could not save the cooldowns");
    }
    if let Err(error) = ctx.store.save_applied(character.id, &state.buffs.saved(now)).await {
        tracing::warn!(%error, id = character.id, "could not save the applied skills");
    }
    // What is left of each time limit; it stands still until the character comes back (`docs/original/quest.md` section 10).
    if let Err(error) = ctx.store.save_quest_times(character.id, &state.quests.times_left(now)).await {
        tracing::warn!(%error, id = character.id, "could not save the quests' time left");
    }
    result
}

/// Where a party drop handed to a member who is leaving goes (audit R14): an item where it dropped, its owner and party as the zone
/// gave them; gold at the member's feet, its own. Other events are not drops.
fn left_on_the_ground(event: ZoneEvent, at: [f32; 3], id: u32) -> Option<(Loot, [f32; 3], u32, u32, crate::zone::Dropped)> {
    match event {
        ZoneEvent::Loot { loot, position, owner, party, dropped } => Some((loot, position, owner, party, dropped)),
        ZoneEvent::Gold { amount, from_mob } => {
            let dropped = if from_mob { crate::zone::Dropped::MOB } else { crate::zone::Dropped::death(0) };
            Some((Loot::Gold(amount), at, id, 0, dropped))
        }
        _ => None,
    }
}

/// `world_loop`, a panic in it turned into an error: the character is then cleaned up and saved as on a lost connection, and the
/// server goes on (`docs/server/audit.md` R5). `panicked` tells the caller.
#[allow(clippy::too_many_arguments)]
async fn guarded_world_loop<S: Backend>(
    conn: &mut Connection,
    peer: SocketAddr,
    ctx: &Context<S>,
    player: &Player,
    character: &mut PlayCharacter,
    state: &mut WorldState,
    inboxes: &mut Inboxes,
    panicked: &mut bool,
) -> Result<Left> {
    let id = character.id;
    match crate::supervise::catch(world_loop(conn, peer, ctx, player, character, state, inboxes)).await {
        Ok(result) => result,
        Err(panic) => {
            *panicked = true;
            tracing::error!(%peer, id, user_id = player.user_id, %panic, "a panic in the world loop; the connection ends and the character is saved");
            Err(anyhow::anyhow!("panic: {panic}"))
        }
    }
}

/// What other players' actions and the zone send this connection.
struct Inboxes {
    packets: crate::outbox::OutboxInbox,
    events: tokio::sync::mpsc::UnboundedReceiver<ZoneEvent>,
}

async fn world_loop<S: Backend>(
    conn: &mut Connection,
    peer: SocketAddr,
    ctx: &Context<S>,
    player: &Player,
    character: &mut PlayCharacter,
    state: &mut WorldState,
    inboxes: &mut Inboxes,
) -> Result<Left> {
    let id = character.id;
    let idle = ctx.config.idle_timeout;
    let mut shutdown = conn.shutdown.clone();
    loop {
        let map = state.map;
        if crate::world::map_of(map) != crate::market::AUCTION_MAP {
            state.market_search = None;
        }
        // The next packet from the client, a packet from another player's actions, or a pending logout taking effect.
        let due = state.logout.map(|(_, due)| due);
        let attack_due = state.target.map(|_| state.next_attack);
        if inboxes.packets.overflowed() {
            bail!("the packets queued for the client passed {} bytes", ctx.config.outbox_limit);
        }
        let next = tokio::select! {
            packet = conn.next_packet(idle), if state.linger_until.is_none() => Some(packet?),
            () = linger_end(state.linger_until, &mut shutdown) => return Ok(Left::Gone),
            Some(packet) = inboxes.packets.recv() => {
                // More than the queue holds was dropped: this connection does not keep up (R1).
                if inboxes.packets.overflowed() {
                    bail!("the packets queued for the client passed {} bytes", ctx.config.outbox_limit);
                }
                conn.send(packet).await?;
                continue;
            }
            Some(event) = inboxes.events.recv() => {
                match event {
                    ZoneEvent::Kill { mob_id, killer } => {
                        if let Some(def) = ctx.data.live().mobs.get(mob_id) {
                            let gain = combat::kill_exp(def, character.level, ctx.switches.exp_rate());
                            award_exp(conn, ctx, character, state, gain, 0, killer == id, false, crate::rules::ExpSource::Mob).await?;
                        }
                        quest_kill(conn, ctx, character, state, mob_id).await?;
                    }
                    // Its share of a party kill, at its own level; a full party counts it twice (`docs/original/party.md` section 8).
                    // A union shares by its own table and never counts it twice (`docs/original/union.md` section 8).
                    ZoneEvent::PartyKill { mob_id, killer, members, union, max_level } => {
                        if let Some(def) = ctx.data.live().mobs.get(mob_id) {
                            let base = combat::mob_exp(def, character.level, ctx.switches.exp_rate());
                            let gain = crate::party::member_exp(base, usize::from(members), union, max_level, character.level);
                            let bonus = if crate::party::full_party(usize::from(members), union) { gain } else { 0 };
                            award_exp(conn, ctx, character, state, gain, bonus, killer == id, false, crate::rules::ExpSource::Party).await?;
                        }
                        quest_kill(conn, ctx, character, state, mob_id).await?;
                    }
                    ZoneEvent::Loot { loot, position, owner, party, dropped } => {
                        party_loot(conn, ctx, character, state, loot, position, owner, party, dropped).await?;
                    }
                    ZoneEvent::Gold { amount: share, from_mob } => {
                        // A monster's raised by the member's own item flags (`CMob::ItemDrop`, `docs/original/item-flags.md` section 4);
                        // capped at 2^32 - 1 (`CUser::ItemGetMoney`).
                        let amount = if from_mob { state.effects.with_gold_bonus(share) } else { share };
                        let amount = amount.min(u32::MAX - state.money);
                        if amount > 0 {
                            match ctx.store.trade(id, i64::from(amount), &[]).await {
                                Ok(Some(_)) => {
                                    state.money += amount;
                                    record::gold_picked_up(ctx, character, state, amount);
                                    conn.send(gp::picked_up_gold(0, 1, amount)).await?;
                                }
                                // A refused or failed save puts the member's share on the ground at its feet instead of losing it
                                // (audit R14); the picker's gold bonus is applied when it is picked up.
                                saved => {
                                    let dropped = if from_mob { crate::zone::Dropped::MOB } else { crate::zone::Dropped::death(0) };
                                    ctx.zones.drop_on_ground(state.map, Loot::Gold(share), state.position, id, 0, Instant::now(), gp::KIND_PUT_BACK, dropped);
                                    saved?;
                                    tracing::warn!(id, share, "the database refused party gold: it lies on the ground");
                                }
                            }
                        }
                    }
                    ZoneEvent::Died { killer_kind, killer, loss, counted, keep_exp, drop_exp } => {
                        died(ctx, state, id);
                        state.keep_exp = keep_exp;
                        state.drop_exp = drop_exp;
                        tracing::info!(%peer, id, killer_kind, killer, "died");
                        // `0x0301` (`CUser::Death`): a monster's kill, named by its definition; the skill used is not in the event (NULL).
                        if killer_kind == cp::KILLER_MOB
                            && let Some(mob) = ctx.zones.mob(state.map, killer)
                        {
                            let name = ctx.game_log.mob_name(mob.mob_id).to_owned();
                            ctx.game_log.action(crate::game_log::death_by_mob(record::header(character, state), u32::from(mob.mob_id), None, &name));
                        }
                        // A death a `pvp_gain` rule with `deaths = false` covers is not counted (`docs/plans/ext-2-rules.md` point 7).
                        if counted && ctx.zones.rules().pvp_deaths(state.map, &rules_who(character, player.faction.map_or(0, |f| f.country))) {
                            death_counted(conn, ctx, character, state).await?;
                        }
                        if let Some(killer) = loss {
                            lose_to(conn, ctx, player, character, state, killer).await?;
                        }
                    }
                    // The faction's grade changed: the numbers again (`CUser::UpdateKCStatus`, `docs/original/pvp-2.md` section 10).
                    ZoneEvent::KillGrade => {
                        state.grade = ctx.kill_counts.guard().status(player.faction.map_or(0, |f| f.country));
                        renumber(conn, ctx, character, state).await?;
                        // The current values capped at the maxima (`SendRecoverAdd(0, 0)`).
                        conn.send(cp::recover_set(ctx.zones.health(state.map, character.id).unwrap_or(state.health))).await?;
                    }
                    // A player kill, its own or its party's near it (`docs/original/pvp.md` section 6.1).
                    ZoneEvent::PlayerKill { victim, victim_level, last_hit, party } => {
                        let (gain, bonus) = match party {
                            None => (crate::pvp::solo_exp(victim_level, character.level), 0),
                            Some((members, union, max_level)) => {
                                let base = crate::pvp::victim_exp(victim_level, character.level);
                                let gain = crate::party::member_exp(base, usize::from(members), union, max_level, character.level);
                                (gain, if crate::party::full_party(usize::from(members), union) { gain } else { 0 })
                            }
                        };
                        award_exp(conn, ctx, character, state, gain, bonus, last_hit == id, false, crate::rules::ExpSource::Pvp).await?;
                        kill_counted(conn, ctx, character, state, player.faction.map_or(0, |f| f.country)).await?;
                        // `0x0302` (`CUser::KillPCTo`): the victim and the killer's counters after; the skill is not in the event (NULL).
                        let victim_name = ctx.zones.name_of(victim).unwrap_or_default();
                        ctx.game_log.action(crate::game_log::kill_player(record::header(character, state), victim, None, &victim_name, character.kills));
                        tracing::info!(%peer, id, victim, gain, "player killed");
                    }
                    // Duels (`docs/original/duel.md`): the stakes out of the bags, the end of a duel, a victory or a defeat.
                    ZoneEvent::DuelStakes => duel::take_stakes(ctx, character, state).await?,
                    ZoneEvent::Sale => my_shop::sales(ctx, character, state).await?,
                    // A Lua monster's recall (`docs/original/instances.md` section 15), unless a logout is pending.
                    ZoneEvent::Recalled { zone, position } if state.logout.is_none() => {
                        tracing::info!(%peer, id, to = zone, ?position, "recalled by a monster");
                        move_player(conn, ctx, state, id, zone, position, None).await?;
                    }
                    ZoneEvent::Recalled { .. } => {}
                    ZoneEvent::Mall => mall::inbox(conn, ctx, player.user_id, state).await?,
                    ZoneEvent::Admin => {
                        for order in ctx.zones.take_admin_orders(id) {
                            admin::carry_out(conn, ctx, player, character, state, order).await?;
                        }
                    }
                    ZoneEvent::Record(mut row) => {
                        row.header = record::header(character, state);
                        ctx.game_log.action(*row);
                    }
                    ZoneEvent::DuelEnded => stop_attacking(ctx, state, id),
                    ZoneEvent::DuelResult { won, opponent, name } => duel::result(ctx, character, state, won, (opponent, &name)).await?,
                    ZoneEvent::DuelCancelled { reason, opponent, name } => duel::cancelled(ctx, character, state, reason, (opponent, &name)),
                    // A monster's hit breaks a cast as a step does (`CUser::StatusCastingStop`, `docs/original/mob-skills.md`
                    // section 9).
                    ZoneEvent::Struck => {
                        // The zone stopped the summon or put the rider down already.
                        state.summon_due = None;
                        if state.casting {
                            cancel_cast(ctx, state, id);
                            tracing::debug!(%peer, id, "cast broken by a hit");
                        }
                        if let Some(packet) = cancel_item_move(ctx, state, id) {
                            conn.send(packet).await?;
                        }
                    }
                    // A blow reached the player: the next armour piece wears (`CUser::QualityByDefense`).
                    ZoneEvent::Worn => {
                        if let Some(worn) = state.armour_wear.wear(&state.inventory, &ctx.data.items) {
                            wore(conn, ctx, character, state, worn).await?;
                        }
                    }
                    ZoneEvent::StoodUp => {
                        state.motion = w::MOTION_STAND;
                        // Standing up ends a personal shop (`CUser::StatusStand`).
                        ctx.zones.shop_end(id);
                        // A fallen character's way out is never cancelled.
                        if !state.fallen && state.logout.take().is_some() {
                            tracing::info!(%peer, id, "logout cancelled: attacked");
                        }
                    }
                    // Its equipment, bag 0 slots 0 to 12 as worn now, to the player who asked (`TP_USER_ITEMS`).
                    ZoneEvent::Inspect { asker } => {
                        let mut items: Vec<ItemUnit> =
                            state.inventory.items().filter(|i| i.bag == 0 && usize::from(i.slot) < w::USER_ITEMS_MAX).cloned().collect();
                        items.sort_by_key(|i| i.slot);
                        ctx.zones.send_to_char(asker, &w::user_items(&items));
                    }
                    ZoneEvent::Buffs => {
                        if let Some(effects) = ctx.zones.buff_effects(state.map, id) {
                            buffs_changed(conn, ctx, character, state, effects).await?;
                        }
                    }
                    // Back to life where the caster stands, nothing lost (`CUser::LifeSkill`; the server position follows the
                    // client's: deviation, `docs/original/skills-special.md` section 12).
                    ZoneEvent::Resurrected { position, caster, skill } if state.dead => {
                        state.dead = false;
                        state.rebirth_due = None;
                        state.motion = w::MOTION_STAND;
                        state.health = state.totals.max_health();
                        state.position = position;
                        state.move_check.reset(Instant::now(), position);
                        for packet in ctx.zones.revive_here(state.map, id, position, character.exp, cp::REBIRTH_BIND, false) {
                            conn.send(packet).await?;
                        }
                        // `0x0305` otherwise (`CUser::LifeSkill`): revived by another player's skill (an item's has skill 0).
                        let caster_name = ctx.zones.name_of(caster).unwrap_or_default();
                        ctx.game_log.action(crate::game_log::revived_by_skill(record::header(character, state), caster, u32::from(skill), &caster_name));
                        tracing::info!(%peer, id, ?position, "resurrected");
                    }
                    ZoneEvent::Resurrected { .. } => {}
                    ZoneEvent::Trade => trade_exchange(ctx, character, state).await?,
                    ZoneEvent::GuildAgreed => found_guild(ctx, id, state).await,
                    ZoneEvent::PartyCalled { until } => state.party_call_due = Some(until),
                }
                continue;
            }
            () = sleep_until_some(attack_due) => {
                let country = player.faction.map_or(0, |f| f.country);
                attack_tick(conn, ctx, character, state, country).await?;
                continue;
            }
            () = sleep_until_some(state.called.map(|(due, ..)| due)) => {
                admin::called_move(conn, ctx, state, id).await?;
                continue;
            }
            () = sleep_until_some(state.notices.next_due()) => {
                admin::notices_due(conn, ctx, state, id).await?;
                continue;
            }
            () = sleep_until_some(admin::next_ban_end(ctx, id)) => {
                admin::bans_ended(conn, ctx, state, id).await?;
                continue;
            }
            () = sleep_until_some(state.summon_due) => {
                vehicles::summon_due(conn, ctx, character, state).await?;
                continue;
            }
            () = sleep_until_some(state.item_move.map(|m| m.due)) => {
                items::finish_item_move(conn, ctx, character, state, player.faction.map_or(0, |f| f.country)).await?;
                continue;
            }
            () = sleep_until_some(state.party_call_due) => {
                items::party_call_ran_out(ctx, state, id);
                continue;
            }
            () = sleep_until_some(state.rebirth_due) => {
                // The party's wait ending is the way the player chose; the 200 s timer is `timeover`.
                let way = if state.rebirth_party { Revive::SelParty } else { Revive::TimeOver };
                rebirth(conn, ctx, player, character, state, way).await?;
                continue;
            }
            () = sleep_until_some(state.quests.next_deadline()) => {
                // A time limit ran out: the quest fails (`CUser::UpdateQuest`, `docs/original/quest.md` section 9).
                for quest_id in state.quests.expired(Instant::now()) {
                    if let Some(def) = ctx.data.quests.get(quest_id) {
                        fail_quest(conn, ctx, character, state, def, 0).await?;
                        tracing::info!(%peer, id, quest_id, "quest failed: time ran out");
                    }
                }
                continue;
            }
            () = sleep_until_some(due) => None,
        };
        let Some(next) = next else {
            let (kind, _) = state.logout.take().expect("a logout was pending");
            // Out of the trade, the party and the zone first, so nothing from other players follows the last packet.
            ctx.zones.cancel_actions(id);
            ctx.zones.party_leave_world(id);
            ctx.zones.war_left_world(map, id);
            if let Some(left) = ctx.zones.leave(map, id) {
                for packet in duel::left_zone(ctx, id, state, left).await? {
                    conn.send(packet).await?;
                }
            }
            state.entered = false;
            return finish_logout(conn, peer, player, character, kind).await;
        };
        let packet = match next {
            Next::Packet(packet) => packet,
            Next::Closed => return Ok(Left::Gone),
            Next::Kick(kick) => {
                // Here, where no other arm can cancel it (R19).
                conn.kick_out(kick).await;
                return Ok(Left::Gone);
            }
        };
        let payload = frame::payload(&packet);
        let opcode = frame::opcode(&packet);
        // Every trade packet ends a personal shop first (`CUser::PacketExchange`, `docs/original/personal-shop.md` section 6).
        if opcode & 0xFF00 == 0x0A00 && state.entered {
            ctx.zones.shop_end(id);
        }
        // A game still open when the client enters the zone again: the player stands up first (`docs/original/mini-game.md` decision 3).
        if opcode == w::OP_USER_ENTER && !state.entered && state.mini_game.is_some() {
            mini_game::stand_for_entry(conn, ctx, character, state).await?;
        }
        match opcode {
            g::OP_PING => {
                // The tests' way to make a connection panic inside a zone lock (R5); 0 never matches.
                if ctx.test_panic.load(Ordering::Relaxed) == id {
                    ctx.zones.panic_holding_locks();
                }
            }
            g::OP_DISCONNECT => return Ok(Left::Gone),

            w::OP_USER_ENTER if !state.entered => {
                // A position off the map closes the connection (`CZone::EnterUser`).
                if !ctx.data.world.is_safe_pos(map, state.position) {
                    bail!("entering at {:?} on map {map}, which is off the map or not run by this server", state.position);
                }
                let guard = guard(ctx, character, state);
                let occupant = Occupant {
                    id,
                    staff: state.staff,
                    visible: state.gm.visible,
                    attackable: state.gm.attackable,
                    pvp: state.gm.pvp,
                    position: state.position,
                    dir: state.dir,
                    guild_id: 0,
                    attack_speed: state.speeds().0,
                    move_speed: state.speeds().1,
                    vitals: Vitals {
                        health: state.health,
                        max: state.totals.max_health(),
                        guard: guard.fighter,
                        guard_element: guard.element,
                        sit_percent: state.passives.bonuses.sit_recovery_percent,
                        rec: guard.rec,
                        absorption: guard.absorption,
                        state_blocks: guard.state_blocks,
                        grade_recovery: guard.grade_recovery,
                        grade_fight_recovery: guard.grade_fight_recovery,
                        options: guard.options,
                        cross_road: guard.cross_road,
                    },
                    shape: enter::user_shape(character, player.faction.map_or(0, |f| f.country), state.inventory.items()),
                    buffs: std::mem::take(&mut state.buffs),
                    outbox: state.outbox.clone(),
                    events: state.events.clone(),
                    // Riding on into a field, not into a dungeon (`CZone::EnterUser`).
                    mount: state.mount.take().filter(|_| !ctx.data.world.is_dungeon(map)),
                };
                for packet in ctx.zones.enter(map, occupant) {
                    conn.send(packet).await?;
                }
                // The burst carries only the maxima; the client gets the current values at once (docs/original/mob-fight.md
                // section 10).
                conn.send(cp::recover_set(state.health)).await?;
                // The burst carries the speeds without debuffs: a held player (a debuff loaded with the character) is told at once.
                state.sent_speeds = state.speeds();
                if state.sent_speeds != state.totals.speeds() {
                    let (attack, movement) = state.sent_speeds;
                    conn.send(ctx.zones.change_speeds(map, id, attack, movement)).await?;
                }
                state.entered = true;
                script::load_values(ctx, state).await;
                script::entered(ctx, state);
                // Entering a zone starts the move check again (`CUser::EnterZone`).
                state.move_check.reset(Instant::now(), state.position);
                tracing::info!(%peer, user_id = player.user_id, id, map, "in the world");
            }

            w::OP_MOVE | cp::OP_ATTACK_MOB | cp::OP_ATTACK_USER | sk::OP_USE_USER | sk::OP_USE_MOB if state.entered && state.dead => {
                tracing::debug!(%peer, id, opcode = format_args!("{opcode:#06x}"), "ignored: dead");
            }

            cp::OP_REBIRTH_REQUEST if state.entered => {
                // CUser::PacketPC 0x0553: 2 at the bind position; 3 the same without a party, with one next to its leader 30 s later
                // (`docs/original/party.md` section 11); 4 and 5 with a resurrection rune, at the bind position or where the player
                // lies (`docs/original/item-flags.md` section 7).
                match cp::parse_rebirth_request(payload) {
                    Some(kind @ (4 | 5)) if state.dead => {
                        state.rebirth_party = false;
                        rune_rebirth(conn, ctx, player, character, state, kind == 5).await?;
                    }
                    Some(3) if state.dead && ctx.zones.in_party(id) => {
                        state.rebirth_due = Some(Instant::now() + PARTY_REBIRTH_WAIT);
                        state.rebirth_party = true;
                    }
                    Some(2 | 3) if state.dead => {
                        state.rebirth_party = false;
                        rebirth(conn, ctx, player, character, state, Revive::SelBase).await?;
                    }
                    kind => tracing::debug!(%peer, id, ?kind, dead = state.dead, "rebirth request ignored"),
                }
            }

            pp::OP_INVITE | pp::OP_KICK | pp::OP_LEADER if state.entered => {
                let Some(target) = pp::parse_id(payload) else { continue };
                match opcode {
                    pp::OP_INVITE => ctx.zones.party_invite(map, id, Invitee::Id(target), false, Instant::now()),
                    pp::OP_KICK => ctx.zones.party_kick(id, target),
                    _ => ctx.zones.party_leader(id, target),
                }
            }
            pp::OP_INVITE_BY_NAME if state.entered => {
                let Some(name) = pp::parse_name(payload) else { continue };
                ctx.zones.party_invite(map, id, Invitee::Name(&name), false, Instant::now());
            }
            pp::OP_ANSWER | pp::OP_UNION_ANSWER if state.entered => {
                let Some(answer) = pp::parse_answer(payload) else { continue };
                ctx.zones.party_answer(map, id, answer == pp::ANSWER_YES, opcode == pp::OP_UNION_ANSWER, Instant::now());
            }
            // `0x0B0D` leaves a union or a party, `0x0B05` either too (`docs/original/union.md` section 5).
            pp::OP_LEAVE | pp::OP_UNION_LEAVE if state.entered => ctx.zones.party_leave(id),
            // Unions (`docs/original/union.md`).
            pp::OP_UNION_SET if state.entered => {
                let Some((union, auto_join, rule)) = pp::parse_union_set(payload) else { continue };
                ctx.zones.union_set(id, union, auto_join, rule);
            }
            pp::OP_UNION_RULE if state.entered => {
                let Some(rule) = pp::parse_id(payload) else { continue };
                ctx.zones.union_rule(id, rule);
            }
            pp::OP_UNION_AUTO_JOIN if state.entered => {
                let Some(&on) = payload.first() else { continue };
                ctx.zones.union_auto_join(id, on != 0);
            }
            pp::OP_UNION_JOIN if state.entered => {
                let Some(name) = pp::parse_name(payload) else { continue };
                ctx.zones.union_join(id, &name, Instant::now());
            }
            pp::OP_UNION_MOVE if state.entered => {
                let Some((from, to)) = pp::parse_union_move(payload) else { continue };
                ctx.zones.union_move(id, from, to);
            }
            pp::OP_UNION_INVITE | pp::OP_UNION_SUB_LEADER | pp::OP_UNION_LEADER | pp::OP_UNION_KICK if state.entered => {
                let Some(target) = pp::parse_id(payload) else { continue };
                match opcode {
                    pp::OP_UNION_INVITE => ctx.zones.party_invite(map, id, Invitee::Id(target), true, Instant::now()),
                    pp::OP_UNION_SUB_LEADER => ctx.zones.union_sub_leader(id, target),
                    pp::OP_UNION_LEADER => ctx.zones.union_leader(id, target),
                    _ => ctx.zones.union_kick(id, target),
                }
            }
            pp::OP_UNION_INVITE_BY_NAME if state.entered => {
                let Some(name) = pp::parse_name(payload) else { continue };
                ctx.zones.party_invite(map, id, Invitee::Name(&name), true, Instant::now());
            }
            pp::OP_UNION_BREAK if state.entered => ctx.zones.union_break(id),

            // No trading while dead (audit R3); the packet is ignored, not an error.
            tp::OP_REQUEST | tp::OP_ANSWER if state.entered && state.dead => {
                tracing::debug!(%peer, id, opcode = format_args!("{opcode:#06x}"), "ignored: dead");
            }
            tp::OP_REQUEST if state.entered => {
                let Some(target) = tp::parse_id(payload) else { continue };
                if let Some(refusal) = ctx.zones.trade_request(map, id, target, Instant::now()) {
                    tracing::debug!(id, map, point = "trade", rule = &*refusal.name, "refused by a rule");
                    refused::tell_refused(conn, state, &refusal).await?;
                }
            }
            tp::OP_ANSWER if state.entered => {
                let Some(answer) = tp::parse_byte(payload) else { continue };
                ctx.zones.trade_answer(id, answer == tp::ANSWER_YES);
            }
            tp::OP_READY if state.entered => {
                let Some(request) = tp::parse_byte(payload) else { continue };
                if ctx.zones.trade_ready(id, request).is_some() {
                    trade_exchange_last(ctx, character, state).await;
                }
            }
            tp::OP_ADD if state.entered => {
                let Some(offer) = tp::parse_offer(payload) else { continue };
                match tradeable(state, &ctx.data.items, offer) {
                    Some(item) => {
                        let item = item.clone();
                        if !ctx.zones.trade_offer(id, offer, &item) {
                            tracing::debug!(%peer, id, ?offer, "trade offer ignored");
                        }
                    }
                    None => {
                        tracing::debug!(%peer, id, ?offer, "trade offer refused: no such item, too many, a quest item or not tradeable")
                    }
                }
            }
            tp::OP_REMOVE if state.entered => {
                let Some(trade_slot) = tp::parse_byte(payload) else { continue };
                ctx.zones.trade_take_back(id, trade_slot);
            }
            tp::OP_MONEY if state.entered => {
                let Some(money) = tp::parse_money(payload) else { continue };
                if money <= state.money {
                    ctx.zones.trade_money(id, money);
                }
            }
            // `0x0A03` and `0x0A04` are the server's; anything else in the family closes the connection, as in the original.
            tp::OP_START | tp::OP_END if state.entered => {}

            // Guilds (`docs/original/guild.md`); the guild house, Etin return and NPC upgrades (`docs/original/guild-house.md`) and the bank
            // (bag 255 in `0x0204`, `docs/original/guild-bank.md`).
            gd::OP_CREATE if state.entered => {
                let Some((name, remark)) = gd::parse_create(payload) else { continue };
                ctx.zones.guild_create(map, id, &name, &remark, ctx.config.guild_party_size, Instant::now());
            }
            gd::OP_CREATE_AGREE if state.entered => {
                if let Some(yes) = gd::parse_yes(payload) {
                    ctx.zones.guild_create_answer(id, yes);
                }
            }
            // Under the bank's turn, so no bank move is half done when the guild is checked and closed (audit R23).
            gd::OP_DISBAND if state.entered => {
                let _turn = ctx.zones.guild_bank_turn().await;
                ctx.zones.guild_disband(map, id, Instant::now());
            }
            gd::OP_DISBAND_AGREE if state.entered => {
                if let Some(yes) = gd::parse_yes(payload) {
                    let _turn = ctx.zones.guild_bank_turn().await;
                    ctx.zones.guild_disband_answer(id, yes);
                }
            }
            gd::OP_INVITE if state.entered => {
                if let Some(target) = gd::parse_id(payload) {
                    ctx.zones.guild_invite(map, id, target, Instant::now());
                }
            }
            gd::OP_INVITE_ANSWER if state.entered => {
                if let Some(yes) = gd::parse_yes(payload) {
                    ctx.zones.guild_invite_answer(map, id, yes, Instant::now());
                }
            }
            gd::OP_JOIN_REQUEST if state.entered => {
                if let Some(guild) = gd::parse_id(payload) {
                    ctx.zones.guild_join_request(id, guild, Instant::now());
                }
            }
            gd::OP_JOINED if state.entered => {
                if let Some((accept, target)) = gd::parse_flag_and_id(payload) {
                    ctx.zones.guild_join_answer(id, accept == 1, target, Instant::now());
                }
            }
            gd::OP_LEAVE if state.entered => ctx.zones.guild_leave(id),
            gd::OP_KICK if state.entered => {
                if let Some(target) = gd::parse_id(payload) {
                    ctx.zones.guild_kick(id, target);
                }
            }
            gd::OP_MEMBER_STATE if state.entered => {
                if let Some((down, target)) = gd::parse_flag_and_id(payload) {
                    ctx.zones.guild_rank(id, down, target);
                }
            }
            gd::OP_REMARK if state.entered => {
                if let Some(remark) = gd::parse_remark(payload) {
                    ctx.zones.guild_remark(id, &remark);
                }
            }
            gd::OP_ETIN if state.entered => ctx.zones.guild_etin(id),
            gd::OP_NPC_UPGRADE if state.entered => {
                if let Some((key, level)) = gd::parse_npc_upgrade(payload) {
                    ctx.zones.guild_npc_upgrade(id, key, level, ctx.data.world.guild_house());
                }
            }
            gd::OP_HOUSE_BUY if state.entered => {
                if let Some(npc) = gd::parse_id(payload) {
                    guild_house::buy_house(conn, ctx, id, state, npc).await?;
                }
            }
            gd::OP_ETIN_RETURN if state.entered => {
                if let Some(npc) = gd::parse_id(payload) {
                    guild_house::return_etin(conn, ctx, character, state, npc).await?;
                }
            }
            opcode if opcode & 0xFF00 == 0x0A00 && state.entered => bail!("unexpected trade opcode {opcode:#06x}"),
            // Duels (`docs/original/duel.md`); anything else in the family closes the connection, as in the original.
            opcode if opcode & 0xFF00 == 0x2400 && state.entered => duel::packet(ctx, character, state, opcode, payload).await?,
            // Guild wars (`docs/original/guild-war.md`); anything else in the family is ignored, as in the original.
            gwp::OP_REQUEST if state.entered => {
                if let Some(target) = gwp::parse_id(payload) {
                    ctx.zones.war_request(state.map, id, target, Instant::now());
                }
            }
            gwp::OP_ANSWER if state.entered => {
                if let Some(answer) = gwp::parse_byte(payload) {
                    ctx.zones.war_answer(state.map, id, answer != gwp::ANSWER_REFUSE, Instant::now());
                }
            }
            opcode if opcode & 0xFF00 == 0x2300 && state.entered => my_shop::packet(conn, ctx, character, state, player.faction.map_or(0, |f| f.country), opcode, payload).await?,
            // The auction house (`docs/original/auction.md`).
            opcode if opcode & 0xFF00 == 0x1C00 && state.entered => market::packet(conn, ctx, character, state, opcode, payload).await?,
            // The item mall and the billing storage (`docs/original/item-mall.md`).
            opcode if (opcode & 0xFF00 == 0x2600 || opcode == shaiya_proto::point::OP_BILLING_GET) && state.entered => {
                mall::packet(conn, ctx, player, character, state, opcode, payload).await?
            }

            // Quests (`docs/original/quest.md`): every request first ends a trade (`CUser::CancleActionExc`).
            qp::OP_START | qp::OP_START_TOO if state.entered => {
                let Some((npc_id, quest_id)) = qp::parse_request(payload) else { continue };
                ctx.zones.cancel_actions(id);
                quest_start(conn, ctx, character, state, npc_id, quest_id).await?;
            }
            qp::OP_END if state.entered => {
                let Some((npc_id, quest_id)) = qp::parse_request(payload) else { continue };
                ctx.zones.cancel_actions(id);
                quest_end(conn, ctx, character, state, npc_id, quest_id).await?;
            }
            qp::OP_CHOOSE if state.entered => {
                let Some((quest_id, index)) = qp::parse_choice(payload) else { continue };
                ctx.zones.cancel_actions(id);
                let def = ctx.data.quests.get(quest_id).filter(|def| index < def.user_select && usize::from(index) < def.results.len());
                match def {
                    Some(def) if state.quests.find(quest_id).is_some_and(|a| a.choosing) => {
                        finish_quest(conn, ctx, character, state, def, usize::from(index), 0).await?;
                    }
                    _ => tracing::debug!(%peer, id, quest_id, index, "reward choice ignored"),
                }
            }
            qp::OP_GIVE_UP if state.entered => {
                let Some(quest_id) = qp::parse_quest_id(payload) else { continue };
                ctx.zones.cancel_actions(id);
                if let Some(def) = ctx.data.quests.get(quest_id).filter(|_| state.quests.find(quest_id).is_some()) {
                    fail_quest(conn, ctx, character, state, def, 0).await?;
                    tracing::info!(%peer, id, quest_id, "quest given up");
                }
            }
            // Anything else in the family closes the connection, as in the original.
            opcode if opcode & 0xFF00 == 0x0900 && state.entered => bail!("unexpected quest opcode {opcode:#06x}"),

            // Friends, the block list and the party search (`docs/original/friends.md`).
            fr::OP_REQUEST if state.entered => {
                let Some(name) = fr::parse_name(payload) else { continue };
                ctx.zones.friend_request(map, id, &name, Instant::now());
            }
            fr::OP_ANSWER if state.entered => {
                let Some(yes) = fr::parse_yes(payload) else { continue };
                ctx.zones.friend_answer(map, id, yes, Instant::now());
            }
            fr::OP_FRIEND_REMOVE if state.entered => {
                let Some(friend) = fr::parse_id(payload) else { continue };
                ctx.zones.friend_remove(id, friend);
            }
            fr::OP_FRIEND_MEMO if state.entered => {
                let Some((friend, memo)) = fr::parse_memo(payload) else { continue };
                ctx.zones.friend_memo(id, friend, &memo);
            }
            fr::OP_BLOCK_ADD if state.entered => {
                let Some(name) = fr::parse_name(payload) else { continue };
                ctx.zones.block(id, &name);
            }
            fr::OP_BLOCK_REMOVE if state.entered => {
                let Some(blocked) = fr::parse_id(payload) else { continue };
                ctx.zones.unblock(id, blocked);
            }
            fr::OP_BLOCK_MEMO if state.entered => {
                let Some((blocked, memo)) = fr::parse_memo(payload) else { continue };
                ctx.zones.block_memo(id, blocked, &memo);
            }
            fr::OP_PARTY_SEARCH if state.entered => ctx.zones.party_search(map, id),
            // Anything else in the family closes the connection, as in the original (`0x2204`, `0x2207`, `0x2208` included).
            opcode if opcode & 0xFF00 == 0x2200 && state.entered => bail!("unexpected friend opcode {opcode:#06x}"),

            w::OP_MOVE if state.entered => {
                let Some(step) = w::parse_move(payload) else {
                    tracing::debug!(%peer, len = payload.len(), "malformed move ignored");
                    continue;
                };
                stand_up(ctx, state, id);
                if state.casting {
                    cancel_cast(ctx, state, id);
                }
                let [x, _, z] = state.position;
                let distance = ((step.position[0] - x).powi(2) + (step.position[2] - z).powi(2)).sqrt();
                if distance > MAX_STEP {
                    // Put the client back where the server has it, as the original does.
                    if state.logs.refused_step.due(Instant::now()) {
                        tracing::debug!(%peer, id, distance, "move refused: step too long");
                    }
                    stop_attacking(ctx, state, id);
                    conn.send(ctx.zones.send_range(map, id, w::position(id, crate::world::map_of(map), state.position))).await?;
                    conn.send(w::move_refused(id)).await?;
                    continue;
                }
                // A position off the map closes the connection (`CZone::MoveUserByPacket`).
                if !ctx.data.world.is_safe_pos(map, step.position) {
                    bail!("moving to {:?}, off map {map}", step.position);
                }
                if ctx.config.move_check && !speed_checked(ctx, state, id, step.position) {
                    // Back to the window's start, the server's position included (deviation 1); the original's packets.
                    stop_attacking(ctx, state, id);
                    for packet in ctx.zones.move_back(map, id, state.position) {
                        conn.send(packet).await?;
                    }
                    conn.send(w::move_refused(id)).await?;
                    ctx.zones.trade_moved(map, id);
                    ctx.zones.duel_moved(map, id, Instant::now());
                    ctx.zones.war_moved(map, id);
                    continue;
                }
                state.position = step.position;
                state.dir = step.dir;
                state.last_move = Some(Instant::now());
                // A step stops a summon (`CUser::PacketPC` 0x0501, `docs/original/vehicles.md` section 1).
                vehicles::cancel_summon(ctx, state, id);
                for packet in ctx.zones.move_to(map, id, &step) {
                    conn.send(packet).await?;
                }
                ctx.zones.trade_moved(map, id);
                ctx.zones.duel_moved(map, id, Instant::now());
                ctx.zones.war_moved(map, id);
                // A step cancels a move by item, after it went out (`CUser::CancleMove`, `docs/original/item-use.md` section 9).
                if let Some(packet) = cancel_item_move(ctx, state, id) {
                    conn.send(packet).await?;
                }
            }

            // A click on an NPC makes it the player's current NPC; nothing is answered (`CUser::PacketNpc`,
            // `docs/original/guards.md` section 2).
            w::OP_NPC_USE if state.entered => {
                let Some(npc) = w::parse_npc_use(payload) else { continue };
                ctx.zones.use_npc(map, id, npc);
            }

            w::OP_MOTION if state.entered => {
                let Some(motion) = w::parse_motion(payload) else { continue };
                // Any motion request cancels a pending logout (`CUser::ChangeMotion`), but not a fallen character's way out.
                if !state.fallen && state.logout.take().is_some() {
                    tracing::info!(%peer, id, "logout cancelled");
                }
                // Standing and sitting are kept and go to the whole view; anything else (the client's emotes, 116 to 125) only to
                // the players within the action range (CUser::PacketPC 0x46a659).
                let packet = w::motion(id, motion);
                if motion == w::MOTION_STAND || motion == w::MOTION_SIT {
                    // Standing up from sitting ends a personal shop (`CUser::ChangeMotion`).
                    if motion == w::MOTION_STAND && state.motion == w::MOTION_SIT {
                        ctx.zones.shop_end(id);
                    }
                    state.motion = motion;
                    ctx.zones.set_motion(map, id, motion);
                    conn.send(ctx.zones.send_view(map, id, packet)).await?;
                } else {
                    conn.send(ctx.zones.send_range(map, id, packet)).await?;
                }
            }

            w::OP_LOGOUT_TO_SELECT | w::OP_LOGOUT_QUIT if state.entered => {
                if state.motion == w::MOTION_SIT && state.logout.is_none() {
                    state.logout = Some((opcode, Instant::now() + ctx.config.logout_delay));
                    tracing::info!(%peer, id, quit = opcode == w::OP_LOGOUT_QUIT, delay = ?ctx.config.logout_delay, "logout pending");
                } else {
                    tracing::debug!(%peer, id, motion = state.motion, "logout request ignored: not sitting or already pending");
                }
            }

            cp::OP_ATTACK_MOB if state.entered => {
                let Some(mob) = cp::parse_attack(payload) else { continue };
                // CUser::PacketPC 0x0503: not while casting, held (attack speed 255), sitting or not on foot, not again on the monster
                // already attacked.
                if state.casting
                    || ctx.zones.riding(map, id) != crate::zone::Riding::OnFoot
                    || state.speeds().0 == buffs::CANNOT
                    || state.motion == w::MOTION_SIT
                    || state.target == Some(SkillTarget::Mob(mob))
                {
                    continue;
                }
                let country = player.faction.map_or(0, |f| f.country);
                if !ctx.zones.attackable(map, mob, country, &ctx.data.live().mobs) {
                    tracing::debug!(%peer, id, mob, "attack refused: not attackable");
                    continue;
                }
                if state.target.is_none() {
                    conn.send(cp::attack_start()).await?;
                    ctx.zones.set_attacking(map, id, true);
                }
                state.target = Some(SkillTarget::Mob(mob));
                state.attack_mode = true;
                state.next_skill = None;
                // The first attack comes at once unless the last one's wait is still running.
                state.next_attack = state.next_attack.max(Instant::now());
                if let Some(packet) = cancel_item_move(ctx, state, id) {
                    conn.send(packet).await?;
                }
            }

            cp::OP_ATTACK_USER if state.entered => {
                let Some(target) = cp::parse_attack(payload) else { continue };
                // CUser::PacketPC 0x0502 (`docs/original/pvp.md` section 2): as 0x0503, on a player of the other side; the move by
                // item is cancelled whatever happens.
                if !(state.casting
                    || ctx.zones.riding(map, id) != crate::zone::Riding::OnFoot
                    || state.speeds().0 == buffs::CANNOT
                    || state.motion == w::MOTION_SIT
                    || state.target == Some(SkillTarget::Player(target)))
                {
                    match ctx.zones.attack_verdict(map, id, target, ctx.data.world.is_war_map(map)) {
                        crate::zone::Verdict::Yes => {
                            if state.target.is_none() {
                                conn.send(cp::attack_start()).await?;
                                ctx.zones.set_attacking(map, id, true);
                            }
                            state.target = Some(SkillTarget::Player(target));
                            state.attack_mode = true;
                            state.next_skill = None;
                            state.next_attack = state.next_attack.max(Instant::now());
                        }
                        crate::zone::Verdict::No => tracing::debug!(%peer, id, target, "attack refused: not an enemy here"),
                        crate::zone::Verdict::Ruled(refusal) => {
                            tracing::debug!(id, map, point = "attack", rule = &*refusal.name, "refused by a rule");
                            refused::tell_refused(conn, state, &refusal).await?;
                        }
                    }
                }
                if let Some(packet) = cancel_item_move(ctx, state, id) {
                    conn.send(packet).await?;
                }
            }

            cp::OP_STATUS_INFO | cp::OP_STATUS_KILL | cp::OP_STATUS_DEATH if state.entered => {
                reward_panel(conn, ctx, character, state, opcode).await?;
            }

            sk::OP_USE_USER | sk::OP_USE_MOB if state.entered => {
                let Some((number, target)) = sk::parse_use(payload) else { continue };
                // Not while summoning or riding (`docs/original/vehicles.md` section 2).
                if ctx.zones.riding(map, id) != crate::zone::Riding::OnFoot {
                    continue;
                }
                state.last_skill_request = Some(Instant::now());
                let country = player.faction.map_or(0, |f| f.country);
                request_skill(conn, ctx, character, state, country, opcode, number, target).await?;
                if let Some(packet) = cancel_item_move(ctx, state, id) {
                    conn.send(packet).await?;
                }
            }

            gp::OP_PICK_UP if state.entered => {
                let Some(item_id) = gp::parse_pick_up(payload) else { continue };
                ctx.zones.cancel_actions(id);
                for packet in pick_up(ctx, character, state, item_id).await? {
                    conn.send(packet).await?;
                }
            }

            gp::OP_ITEM_DROP if state.entered => {
                let Some((bag, slot, count)) = gp::parse_drop(payload) else { continue };
                for packet in drop_item(ctx, character, state, bag, slot, count).await? {
                    conn.send(packet).await?;
                }
            }

            w::OP_USER_HP if state.entered => {
                let Some(target) = g::parse_char_id(payload) else { continue };
                if let Some((max_hp, hp)) = ctx.zones.hp(map, target) {
                    conn.send(w::user_hp(target, max_hp, hp)).await?;
                }
            }

            w::OP_MOB_INFO | w::OP_MOB_HP if state.entered => {
                // CUser::PacketGetInfo: nothing when the monster is not on this map.
                let Some(target) = g::parse_char_id(payload) else { continue };
                let Some(mob) = ctx.zones.mob(map, target) else { continue };
                if opcode == w::OP_MOB_INFO {
                    conn.send(w::mob_move(mob.id, 1, mob.position[0], mob.position[2])).await?;
                    let (attack_speed, move_speed) = ctx.zones.mob_speeds(map, mob.id);
                    conn.send(w::mob_info(mob.id, mob.hp, attack_speed, move_speed)).await?;
                } else {
                    conn.send(w::mob_hp(mob.id, mob.hp)).await?;
                }
            }

            w::OP_USER_INFO | w::OP_USER_SHAPE if state.entered => {
                let Some(target) = g::parse_char_id(payload) else { continue };
                let replies = if opcode == w::OP_USER_INFO { ctx.zones.info(map, target) } else { ctx.zones.shape(map, id, target) };
                for packet in replies {
                    conn.send(packet).await?;
                }
            }

            // A chat ban drops the line without a word (`CUser::PacketChatE`).
            opcode
                if opcode & 0xFF00 == 0x1100
                    && state.entered
                    && ctx.zones.bans_of(id).chat.is_some_and(|until| !until.passed(Instant::now())) =>
            {
                tracing::debug!(%peer, id, opcode = format_args!("{opcode:#06x}"), "chat dropped: chat ban");
            }

            opcode if opcode & 0xFF00 == 0x1100 && state.entered => {
                let country = player.faction.map_or(0, |f| f.country);
                let name = character.name.as_bytes();
                // The text: too long closes the connection as in the original; too short is ignored.
                let (target, text) = if opcode == chat::OP_WHISPER {
                    let Some((target, text)) = chat::parse_whisper(payload) else { bail!("malformed whisper") };
                    (Some(target), text)
                } else {
                    (None, chat::parse_message(payload))
                };
                let text = match text {
                    Text::Ok(text) => text,
                    Text::Empty => continue,
                    Text::Invalid => bail!("chat text over 128 bytes or cut short"),
                };
                // The channels without a cooldown share a bucket; the rest of a flood is dropped without a word (R4).
                if !matches!(opcode, chat::OP_TRADE | chat::OP_SHOUT | chat::OP_ALL_SERVER) && !state.chat_bucket.take(Instant::now()) {
                    if state.logs.chat_dropped.due(Instant::now()) {
                        tracing::debug!(%peer, id, opcode = format_args!("{opcode:#06x}"), "chat dropped: too many lines");
                    }
                    continue;
                }
                // A colour marker running past the text crashes every client that shows it: not relayed, not even to the sender.
                if !chat::markup_is_safe(text) {
                    if state.logs.unsafe_markup.due(Instant::now()) {
                        tracing::debug!(%peer, id, opcode = format_args!("{opcode:#06x}"), text = %hex(text), "chat refused: unsafe markup");
                    }
                    continue;
                }
                // Without the terminating zero, which would make the log file binary to grep.
                let shown = String::from_utf8_lossy(text.strip_suffix(&[0]).unwrap_or(text));
                tracing::debug!(%peer, id, opcode = format_args!("{opcode:#06x}"), text = %shown, "chat");
                // A `!word` a script registered is not chat: it was counted in the bucket above and is queued for the script thread
                // (`docs/plans/ext-5-lua.md` section 7.2); no lock is held here.
                if opcode == chat::OP_NORMAL && script::command(ctx, character, state, text) {
                    continue;
                }
                match opcode {
                    chat::OP_NORMAL if state.chat_item_armed => {
                        said(ctx, character, state, Heard::Trade, None, text);
                        all_server_chat(ctx, state, id, name, text)
                    }
                    chat::OP_NORMAL => {
                        said(ctx, character, state, Heard::Normal, None, text);
                        conn.send(ctx.zones.send_near(map, id, chat::by_id(opcode, id, text), NORMAL_CHAT_RANGE)).await?;
                    }
                    chat::OP_WHISPER => {
                        let target = target.expect("parsed above");
                        // A player who blocked the sender is as good as not there (`docs/original/friends.md` deviation 5).
                        match ctx.zones.find_by_name(&target) {
                            Some((target_map, target_id, target_country, target_name))
                                if target_country == country && !ctx.zones.blocked(target_id, id) =>
                            {
                                ctx.zones.send_to(target_map, target_id, &chat::whisper(false, name, text));
                                conn.send(chat::whisper(true, &target_name, text)).await?;
                                said(ctx, character, state, Heard::Whisper, Some(&target_name), text);
                            }
                            _ => conn.send(chat::whisper_failed()).await?,
                        }
                    }
                    // The client's trade channel. Limited here as the US client limits it (configurable), since the original server did not.
                    chat::OP_TRADE => {
                        let config = &ctx.config;
                        let rules = ctx.zones.rules();
                        if let Some(refusal) = rules.denied(crate::rules::Point::TradeChat, map, &rules_who(character, country)) {
                            if refusal.name.starts_with("setting.") {
                                tracing::debug!(%peer, id, level = character.level, "trade chat refused: level too low");
                            } else {
                                tracing::debug!(id, map, point = "trade_chat", rule = &*refusal.name, "refused by a rule");
                            }
                            refused::tell_refused(conn, state, &refusal).await?;
                        } else if state.next_trade.is_some_and(|next| Instant::now() < next) {
                            tracing::debug!(%peer, id, "trade chat refused: too soon");
                        } else {
                            ctx.zones.send_country(country, None, &chat::by_name(opcode, name, text));
                            state.next_trade = Some(Instant::now() + config.trade_chat_cooldown);
                            said(ctx, character, state, Heard::All, None, text);
                        }
                    }
                    chat::OP_ZONE => {
                        ctx.zones.send_country(country, Some(map), &chat::by_name(opcode, name, text));
                        said(ctx, character, state, Heard::Zone, None, text);
                    }
                    chat::OP_SHOUT => {
                        if state.next_shout.is_none_or(|next| Instant::now() >= next) {
                            conn.send(ctx.zones.send_near(map, id, chat::by_id(opcode, id, text), SHOUT_RANGE)).await?;
                            state.next_shout = Some(Instant::now() + SHOUT_COOLDOWN);
                            said(ctx, character, state, Heard::Shout, None, text);
                        }
                    }
                    chat::OP_PARTY => {
                        if !ctx.zones.party_chat(id, &chat::by_id(opcode, id, text)) {
                            tracing::debug!(%peer, id, "party chat without a party");
                        } else {
                            said(ctx, character, state, Heard::Party, None, text);
                        }
                    }
                    chat::OP_GUILD => {
                        if !ctx.zones.guild_chat(id, &chat::by_name(opcode, name, text)) {
                            tracing::debug!(%peer, id, "guild chat without a guild");
                        } else {
                            said(ctx, character, state, Heard::Guild, None, text);
                        }
                    }
                    // The union's leader or sub-leader to the union, as the staff's line, which this client shows
                    // (`docs/original/union.md` section 9, deviation 7).
                    chat::OP_PARTY_LEADER => {
                        if !ctx.zones.union_chat(id, &chat::by_id(chat::OP_UNION_NOTICE, id, text)) {
                            tracing::debug!(%peer, id, "union chat without leading a union");
                        } else {
                            said(ctx, character, state, Heard::Party, None, text);
                        }
                    }
                    // Only with the item armed and past the cooldown, which this path alone sets.
                    chat::OP_ALL_SERVER if state.chat_item_armed && state.next_all_chat.is_none_or(|next| Instant::now() >= next) => {
                        said(ctx, character, state, Heard::Trade, None, text);
                        all_server_chat(ctx, state, id, name, text);
                        state.next_all_chat = Some(Instant::now() + ALL_CHAT_COOLDOWN);
                    }
                    chat::OP_ALL_SERVER => tracing::debug!(%peer, id, "all-server chat ignored: no item armed, or too soon"),
                    opcode => bail!("unexpected chat opcode {opcode:#06x}"),
                }
            }

            // `/request GM <text>` (`CUser::PacketUQ`): not stopped by a chat ban, as the original.
            ap::OP_USER_QUESTION if state.entered => admin::user_question(ctx, character, payload).await?,

            w::OP_ITEM_MOVE if state.entered => {
                let Some((from, to)) = w::parse_item_move(payload) else { continue };
                // Not while disguised (`CUser::PacketMaininterface`, `docs/original/skills-special.md` section 2).
                if state.effects.shape == sk::SHAPE_DISGUISE {
                    tracing::debug!(%peer, id, "item move ignored: disguised");
                    continue;
                }
                // Any move ends a trade first (`CUser::PacketMaininterface`, `docs/original/trade.md` section 4).
                if from != to {
                    ctx.zones.cancel_actions(id);
                }
                if from.0 == w::GUILD_BANK || to.0 == w::GUILD_BANK {
                    guild_bank::item_move(conn, ctx, map, character, state, from, to).await?;
                    continue;
                }
                if from.0 == BANK || to.0 == BANK {
                    match warehouse_move(ctx, map, character, state, from, to).await? {
                        Some(reply) => conn.send(reply).await?,
                        None => tracing::debug!(%peer, id, ?from, ?to, "warehouse move ignored"),
                    }
                    continue;
                }
                let wearer = Wearer {
                    level: character.level,
                    family: character.family,
                    job: character.job,
                    grow: character.grow,
                    sex: character.sex,
                };
                let Some(changes) = state.inventory.move_item(&ctx.data.items, &wearer, from, to) else {
                    tracing::debug!(%peer, id, ?from, ?to, "item move ignored");
                    continue;
                };
                if !changes.is_empty() {
                    // Worn equipment's quality is saved before it moves (`0x070C`, `docs/original/repair.md` section 3).
                    let mut saved = if from.0 == 0 || to.0 == 0 { take_unsaved_wear(state) } else { Vec::new() };
                    saved.extend(changes.iter().cloned());
                    if ctx.store.trade(id, 0, &saved).await?.is_none() {
                        bail!("the database refused an item move the server allowed: {saved:?}");
                    }
                    for change in &changes {
                        state.inventory.apply(change);
                    }
                    // The numbers follow the equipment (CUser::ItemEquipmentRem/Add).
                    if from.0 == 0 || to.0 == 0 {
                        renumber(conn, ctx, character, state).await?;
                    }
                    // A change of the mount slot ends a summon or a ride.
                    if vehicles::touches_mount([from, to]) {
                        vehicles::mount_slot_changed(conn, ctx, state, id).await?;
                    }
                    // Others see the visible equipment change (CUser::SendEquipment), before the mover's answer.
                    for (bag, slot) in [from, to] {
                        if bag == 0 && slot < VISIBLE_SLOTS {
                            conn.send(ctx.zones.change_equipment(map, id, slot, state.inventory.get(0, slot))).await?;
                        }
                    }
                }
                let (from_item, to_item) = (state.inventory.get(from.0, from.1), state.inventory.get(to.0, to.1));
                conn.send(w::item_move_result(from, from_item, to, to_item)).await?;
            }

            w::OP_GATEKEEPER if state.entered => {
                let Some((npc_id, destination)) = w::parse_gatekeeper(payload) else { continue };
                let country = player.faction.map_or(0, |f| f.country);
                let gate = match usable_gate(ctx, state, id, country, npc_id, destination) {
                    Ok(gate) => gate,
                    Err(Some(refused)) => {
                        conn.send(refused).await?;
                        continue;
                    }
                    Err(None) => {
                        tracing::debug!(%peer, id, npc_id, destination, "gatekeeper refused");
                        conn.send(w::gatekeeper_result(false, state.money)).await?;
                        continue;
                    }
                };
                if ctx.store.trade(id, -i64::from(gate.cost), &[]).await?.is_none() {
                    bail!("the database refused a gatekeeper payment the server allowed");
                }
                state.money -= gate.cost;
                conn.send(w::gatekeeper_result(true, state.money)).await?;
                tracing::info!(%peer, id, from = map, to = gate.map, position = ?gate.position, cost = gate.cost, "gatekeeper");
                move_player(conn, ctx, state, id, gate.map, gate.position, None).await?;
            }

            // `0x0227` is the yes to the ranking battle's question and goes the same way (`docs/original/guild-ranking.md` section 4).
            w::OP_PORTAL | w::OP_PORTAL_CONFIRM if state.entered => {
                let Some(index) = w::parse_portal(payload) else { continue };
                let country = player.faction.map_or(0, |f| f.country);
                match portal_use(ctx, state, id, country, character.level, index, opcode == w::OP_PORTAL_CONFIRM) {
                    PortalUse::Ignored => tracing::debug!(%peer, id, index, "portal use ignored"),
                    PortalUse::Refused(reason) => conn.send(w::portal_refused(index, reason)).await?,
                    PortalUse::Move(to_map, to) => {
                        tracing::info!(%peer, id, from = map, index, to = to_map, position = ?to, "portal");
                        move_player(conn, ctx, state, id, to_map, to, None).await?;
                    }
                }
            }

            shop::OP_BUY if state.entered => {
                let Some(request) = shop::parse_buy(payload) else { continue };
                ctx.zones.cancel_actions(id);
                match buy(ctx, map, character, state, &request).await? {
                    Some(reply) => conn.send(reply).await?,
                    None => tracing::debug!(%peer, id, ?request, "buy ignored"),
                }
            }

            shop::OP_SELL if state.entered => {
                let Some((bag, slot, count)) = shop::parse_sell(payload) else { continue };
                ctx.zones.cancel_actions(id);
                match sell(ctx, character, state, bag, slot, count).await? {
                    Some(reply) => conn.send(reply).await?,
                    None => tracing::debug!(%peer, id, bag, slot, count, "sell ignored"),
                }
            }

            sk::OP_CHAR_QUICK => match sk::parse_quick(payload) {
                // The same slots again are not written (R4).
                Some(slots) if slots == character.quick_slots => {}
                Some(slots) => {
                    ctx.store.save_quick_slots(id, &slots).await?;
                    character.quick_slots = slots;
                }
                None => tracing::debug!(%peer, id, payload = %hex(payload), "malformed quick slots ignored"),
            },

            sk::OP_STOP_ATTACK if state.entered => {
                // `CUser::StatusAttackStop`: attacking or casting stops (a cast costs nothing); no answer
                // (`docs/original/skills-special.md` section 10).
                if !state.dead && state.target.is_some() {
                    stop_attacking(ctx, state, id);
                }
            }

            sk::OP_RUN if state.entered => {
                // Hidden players walk (section 10).
                if state.effects.shape == sk::SHAPE_HIDDEN {
                    conn.send(sk::run_mode(false)).await?;
                } else {
                    state.run = !state.run;
                    conn.send(sk::run_mode(state.run)).await?;
                }
            }

            sk::OP_LEARN if state.entered => {
                let Some((skill_id, level)) = sk::parse_learn(payload) else { continue };
                // Not while dead (nor while a skill reset waits for its save in the original; here the reset is saved before anything
                // else is handled).
                if !state.dead {
                    learn_skill(conn, ctx, character, state, skill_id, level).await?;
                }
            }

            w::OP_STATUS_UP if state.entered => {
                let Some(points) = w::parse_status_up(payload) else { continue };
                // Not while dead (nor while a stat reset waits for its save in the original; here the reset is saved before anything
                // else is handled).
                if !state.dead {
                    spend_stat_points(conn, ctx, character, state, points).await?;
                }
            }

            vp::OP_USE if state.entered => vehicles::use_vehicle(conn, ctx, character, state).await?,

            vp::OP_ASK if state.entered => {
                let Some(target) = vp::parse_ask(payload) else { continue };
                if let Some(refused) = ctx.zones.ask_to_ride(map, id, target, Instant::now()) {
                    conn.send(refused).await?;
                }
            }

            vp::OP_ANSWER if state.entered => {
                let Some(answer) = vp::parse_answer(payload) else { continue };
                ctx.zones.answer_ride(map, id, answer, Instant::now());
            }

            vp::OP_BEHIND if state.entered => ctx.zones.get_off_behind(map, id),

            it::OP_SAVE_POINT if state.entered => {
                let Some(request) = it::parse_save_point(payload) else { continue };
                items::save_point(conn, ctx, character, state, request).await?;
            }

            it::OP_BODY_CHANGE if state.entered => {
                let Some(request) = it::parse_body_change(payload) else { continue };
                items::change_body(conn, ctx, character, state, request).await?;
            }

            it::OP_REPAIR if state.entered => {
                let Some((bag, slot)) = it::parse_repair(payload) else { continue };
                items::repair_items(conn, ctx, character, state, bag, slot).await?;
            }

            // Another player's equipment: answered by that player's connection, or not at all (`docs/original/small-items.md` section 7).
            w::OP_USER_ITEMS if state.entered => {
                let Some(target) = g::parse_char_id(payload) else { continue };
                if !ctx.zones.inspect(map, id, target) {
                    tracing::debug!(%peer, id, target, "equipment not shown: not in the zone, staff or of the other country");
                }
            }

            // The VG, for the lapis link window's price; always answered (`docs/original/small-items.md` section 8).
            it::OP_VG if state.entered => conn.send(it::vg(character.vg)).await?,

            // The Priest's reading: 10 gold for the five hidden numbers; more than 10 needed (`CUser::PacketPriest`,
            // `docs/original/small-items.md` section 12). Any other `0x0Fxx` closes the connection.
            priest::OP_PRIEST_START | priest::OP_PRIEST_NEXT if state.entered => {
                let paid = state.money > PRIEST_PRICE;
                if paid {
                    if ctx.store.trade(id, -i64::from(PRIEST_PRICE), &[]).await?.is_none() {
                        bail!("the database refused the Priest's price");
                    }
                    state.money -= PRIEST_PRICE;
                    tracing::info!(%peer, id, opcode = format_args!("{opcode:#06x}"), money = state.money, "priest reading");
                }
                let answer = if opcode == priest::OP_PRIEST_NEXT {
                    priest::priest_next(paid)
                } else {
                    let c = &*character;
                    priest::priest_start(paid.then_some(priest::Reading { hg: c.honor, vg: c.vg, cg: c.cg, og: c.og, ig: c.ig }))
                };
                conn.send(answer).await?;
            }
            opcode if opcode & 0xFF00 == 0x0F00 && state.entered => bail!("unknown Priest opcode {opcode:#06x}"),

            // `/town`: to the map's bind position after standing still a while, never answered; not while dead or while another move
            // is pending (`CUser::PacketPC`, `docs/original/small-items.md` section 6).
            it::OP_TOWN if state.entered => {
                if state.dead || state.called.is_some() || state.item_move.is_some() {
                    tracing::debug!(%peer, id, "town refused: dead or a move pending");
                    continue;
                }
                state.item_move = Some(ItemMove::town(Instant::now() + ctx.config.town_delay));
            }

            // Lapis and enchanting (`docs/original/lapis.md`).
            gem::OP_LINK if state.entered => {
                let Some(places) = gem::parse_link(payload) else { continue };
                lapis::link(conn, ctx, character, state, places).await?;
            }

            gem::OP_EXTRACT if state.entered => {
                let Some(request) = gem::parse_extract(payload) else { continue };
                lapis::extract(conn, ctx, character, state, request).await?;
            }

            gem::OP_BADGE if state.entered => {
                let Some(request) = gem::parse_badge(payload) else { continue };
                lapis::badge(conn, ctx, character, state, request).await?;
            }

            gem::OP_BADGE_REMOVE if state.entered => {
                let Some(place) = gem::parse_place(payload) else { continue };
                lapis::remove_badge(conn, ctx, character, state, place).await?;
            }

            gem::OP_ENCHANT if state.entered => {
                let Some(request) = gem::parse_enchant(payload) else { continue };
                lapis::enchant(conn, ctx, character, state, request).await?;
            }

            gem::OP_COMPOSE if state.entered => {
                let Some(request) = gem::parse_compose(payload) else { continue };
                lapis::compose(conn, ctx, character, state, request).await?;
            }

            gem::OP_REMAKE if state.entered => {
                let Some(places) = gem::parse_remake(payload) else { continue };
                lapis::remake(conn, ctx, character, state, places).await?;
            }

            gem::OP_CONSIDER if state.entered => {
                let Some(place) = gem::parse_place(payload) else { continue };
                lapis::consider(conn, ctx, character, state, place).await?;
            }

            it::OP_USE | it::OP_USE_ON | it::OP_USE_SKILL if state.entered => {
                let Some(request) = it::parse_use(opcode, payload) else { continue };
                // An item's skill (`0x0559`) is refused off foot (`CUser::PacketPC`, `docs/original/vehicles.md` section 2).
                if opcode == it::OP_USE_SKILL && !state.dead && ctx.zones.riding(map, id) != crate::zone::Riding::OnFoot {
                    conn.send(it::use_failed(id)).await?;
                    continue;
                }
                items::use_item(conn, ctx, player, character, state, request).await?;
            }

            // The free stat reset is refused, always (`docs/original/item-use.md` section 8).
            it::OP_RESET_STATS if state.entered => conn.send(it::reset_refused(it::OP_RESET_STATS)).await?,

            // The free skill reset, once per character.
            it::OP_RESET_SKILLS if state.entered => {
                if character.free_skill_reset_used {
                    conn.send(it::reset_refused(it::OP_RESET_SKILLS)).await?;
                } else {
                    items::reset_skills(conn, ctx, character, state, true).await?;
                }
            }

            it::OP_PARTY_CALL_ANSWER if state.entered => {
                let Some(answer) = it::parse_party_call_answer(payload) else { continue };
                items::answer_party_call(conn, ctx, character, state, answer).await?;
            }

            // The gambling house (`docs/original/mini-game.md`).
            opcode if opcode & 0xFF00 == 0x3100 => mini_game::packet(conn, ctx, character, state, opcode, payload).await?,

            // The operator commands (`docs/original/gm-commands.md`).
            opcode if opcode >= 0xF000 && state.entered => admin::packet(conn, ctx, player, character, state, opcode, payload).await?,

            // Anything else in the character family closes the connection in the original; elsewhere, log for later analysis.
            opcode if state.entered && opcode & 0xFF00 == 0x0100 => bail!("unexpected opcode {opcode:#06x} in the world"),
            opcode => {
                if state.logs.unhandled.due(Instant::now()) {
                    tracing::debug!(
                        %peer,
                        opcode = format_args!("{opcode:#06x}"),
                        entered = state.entered,
                        payload = %hex(payload),
                        "not handled in the world yet"
                    );
                }
            }
        }
    }
}

/// Equipment wear (`docs/original/repair.md` section 2): the new quality in memory, saved later; a changed wear level works the numbers
/// out again.
async fn wore<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    worn: crate::repair::Worn,
) -> Result<()> {
    state.inventory.apply(&worn.change());
    state.unsaved_wear.insert(worn.slot);
    if worn.level_changed {
        renumber(conn, ctx, character, state).await?;
    }
    Ok(())
}

/// A critical blow on a monster wears the weapon (`CUser::QualityByAttack`), unless the player's shape is not 0 or it has Eternal
/// Endurance.
async fn wear_weapon<S: Backend>(conn: &mut Connection, ctx: &Context<S>, character: &PlayCharacter, state: &mut WorldState) -> Result<()> {
    if state.effects.shape != 0 || state.effects.no_wear {
        return Ok(());
    }
    match crate::repair::wear_weapon(&state.inventory, &ctx.data.items) {
        Some(worn) => wore(conn, ctx, character, state, worn).await,
        None => Ok(()),
    }
}

/// The qualities worn since they were last saved, as changes to save with the next write (section 3); forgotten here.
fn take_unsaved_wear(state: &mut WorldState) -> Vec<SlotChange> {
    std::mem::take(&mut state.unsaved_wear)
        .into_iter()
        .filter_map(|slot| state.inventory.get(0, slot).map(|item| SlotChange::SetQuality { bag: 0, slot, quality: item.quality }))
        .collect()
}

/// After a change of equipment or passive skills: the numbers again, then what changed to the player and those around (speeds, then
/// the maxima), and the zone's copy of the player's numbers.
async fn renumber<S: Backend>(conn: &mut Connection, ctx: &Context<S>, character: &PlayCharacter, state: &mut WorldState) -> Result<()> {
    let (id, map) = (character.id, state.map);
    let before = std::mem::replace(&mut state.totals, totals(ctx, character, state.inventory.items(), &state.passives, &state.grade));
    let after = state.totals;
    if state.speeds() != state.sent_speeds {
        state.sent_speeds = state.speeds();
        let (attack, movement) = state.sent_speeds;
        conn.send(ctx.zones.change_speeds(map, id, attack, movement)).await?;
    }
    let ([hp, mp, sp], [old_hp, old_mp, old_sp]) = (after.max_health(), before.max_health());
    for (kind, new, old) in [(w::MAX_HP, hp, old_hp), (w::MAX_SP, sp, old_sp), (w::MAX_MP, mp, old_mp)] {
        if new != old {
            conn.send(ctx.zones.change_max(map, id, kind, new)).await?;
        }
    }
    ctx.zones.set_guard(map, id, guard(ctx, character, state));
    ctx.zones.set_sit_percent(map, id, state.passives.bonuses.sit_recovery_percent);
    Ok(())
}

/// `0x0209`: learning a skill or its next level (`docs/original/skills.md` section 3). A passive skill's numbers go out before the
/// answer, as in the original.
async fn learn_skill<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &mut PlayCharacter,
    state: &mut WorldState,
    skill_id: u16,
    level: u8,
) -> Result<()> {
    let id = character.id;
    let learner = skills::Learner {
        level: character.level,
        skill_points: character.skill_point,
        family: character.family,
        job: character.job,
        grow: character.grow,
    };
    let (skill, def) = match skills::learn(&learner, &character.skills, skill_id, level, &ctx.data.skills) {
        Ok(learnt) => learnt,
        Err(result) => {
            tracing::debug!(id, skill_id, level, result, "skill not learnt");
            conn.send(sk::learn_result(result, 0, skill_id, level)).await?;
            return Ok(());
        }
    };
    let points = character.skill_point - u16::from(def.points);
    let passive = def.type_effect == 0;
    ctx.store.learn(id, points, skill).await?;
    character.skill_point = points;
    // A new level is ready at once.
    state.cooldowns.remove(&skill.id);
    match character.skills.iter_mut().find(|s| s.number == skill.number) {
        Some(known) => *known = skill,
        None => character.skills.push(skill),
    }
    if passive {
        state.skill_passives = skills::passives(&character.skills, &ctx.data.skills);
        state.passives = with_buffs(state.skill_passives, &state.effects);
        if let Some(bags) = state.passives.bags {
            state.inventory.set_max_bag(bags);
        }
        ctx.zones.set_low_hp(state.map, id, low_hp_passive(ctx, character));
        renumber(conn, ctx, character, state).await?;
    }
    // `0x0309` (`CUser::LearnSkill`): the skill, its level and the skill points left.
    ctx.game_log.action(crate::game_log::skill_learned(
        record::header(character, state),
        u32::from(skill_id),
        level,
        points,
        ctx.game_log.skill_name(skill_id),
    ));
    tracing::info!(id, skill_id, level, number = skill.number, points, passive, "skill learnt");
    conn.send(sk::learn_result(skills::LEARNT, skill.number, skill_id, level)).await?;
    Ok(())
}

/// `0x0208`: spending stat points (`docs/original/stats.md` section 5). The maxima that changed go out before the answer, as in the
/// original.
async fn spend_stat_points<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &mut PlayCharacter,
    state: &mut WorldState,
    points: [u16; 6],
) -> Result<()> {
    let id = character.id;
    let (stat_point, stats) = match stats::spend(character.stat_point, character.stats, points) {
        stats::Spend::Nothing => return Ok(()),
        stats::Spend::Refused => {
            tracing::debug!(id, ?points, have = character.stat_point, "stat points refused");
            conn.send(w::status_up([0; 6])).await?;
            return Ok(());
        }
        stats::Spend::Spent { stat_point, stats } => (stat_point, stats),
    };
    let progress = Progress { stat_point, stats, ..progress(character) };
    let health = ctx.zones.health(state.map, id).unwrap_or(state.health);
    ctx.store.save_progress(id, &progress, health).await?;
    character.stat_point = stat_point;
    character.stats = stats;
    // `0x030A` (`CUser::PacketMaininterface`): one record per stat raised, in the original's order (str, dex, int, wis, rec, luc;
    // the server's arrays run str, dex, rec, int, wis, luc): the new value and the points put in.
    use crate::game_log::Stat;
    for (index, stat) in [(0, Stat::Str), (1, Stat::Dex), (3, Stat::Int), (4, Stat::Wis), (2, Stat::Rec), (5, Stat::Luc)] {
        if points[index] > 0 {
            ctx.game_log.action(crate::game_log::stat_raised(record::header(character, state), stat, stats[index], points[index]));
        }
    }
    renumber(conn, ctx, character, state).await?;
    tracing::info!(id, ?points, ?stats, stat_point, "stat points spent");
    conn.send(w::status_up(points)).await?;
    Ok(())
}

/// A character's numbers with the given items and passive skills (`crate::stats`).
/// `grade`: what the faction's kill-count grade gives (`docs/original/pvp-2.md` section 10).
fn totals<'a, S>(
    ctx: &Context<S>,
    c: &PlayCharacter,
    items: impl IntoIterator<Item = &'a crate::backend::ItemUnit>,
    passives: &Passives,
    grade: &[i32; crate::kc::TYPES],
) -> Totals {
    let base = stats::Base { job: c.job, level: c.level, grow: c.grow, family: c.family, stats: c.stats, grade: *grade };
    stats::compute(&ctx.data.status, &base, items, &ctx.data.items, passives)
}

/// The passives' numbers with the applied skills' bonuses added.
fn with_buffs(passives: Passives, effects: &BuffEffects) -> Passives {
    Passives { bonuses: passives.bonuses.plus(&effects.bonuses), ..passives }
}

/// The low-HP passive the character has learnt, at its level (`m_pHPAbility`, `docs/original/skills-buffs.md` section 11).
fn low_hp_passive<S>(ctx: &Context<S>, character: &PlayCharacter) -> Option<SkillDef> {
    character
        .skills
        .iter()
        .filter_map(|s| ctx.data.skills.get(s.id, s.level))
        .find(|d| d.type_effect == skills::PASSIVE && d.kind == crate::buffs::LOW_HP)
        .copied()
}

/// The equipment's attack numbers with the passives, and the elements an applied skill replaces (`wType` 94, 95). While transformed,
/// the item numbers are the monster's, or none (`CUser::InitEquipment(false)`, `docs/original/skills-special.md` section 5.4).
fn gear<S>(ctx: &Context<S>, state: &WorldState) -> combat::Gear {
    let mut g = combat::gear(state.inventory.items(), &ctx.data.items, &state.passives);
    // The faction grade's critical, melee hit, magic defence and evasion, and its defence percentages (`docs/original/pvp-2.md`
    // section 8, deviation 1).
    let k = &state.grade;
    g.critical += k[16];
    g.adds[0].hit += k[17];
    g.adds[2].defence += k[20];
    for (add, kind) in g.adds.iter_mut().zip([21, 22, 23]) {
        add.evasion += k[kind];
    }
    g.defence_percent = [k[18], k[19]];
    // A monster's polymorph (101) roots the player but keeps its equipment's numbers (`docs/original/mob-skills.md` section 8).
    if !matches!(state.effects.shape, 0 | sk::SHAPE_HIDDEN) && !state.effects.rooted {
        let live = ctx.data.live();
        let mob = live.mobs.get(state.effects.shape_mob);
        g.attack = mob.map_or(0, |m| i32::from(m.attacks[0].damage));
        g.attack_var = mob.map_or(0, |m| i32::from(m.attacks[0].damage_add));
        g.defence = mob.map_or(0, |m| i32::from(m.defense));
        g.magic_defence = mob.map_or(0, |m| i32::from(m.magic));
    }
    if let Some(element) = state.effects.attack_element {
        g.element = element;
    }
    if let Some(element) = state.effects.defence_element {
        g.armour_element = element;
    }
    // `wType` 68 from a monster: no defence element while it lasts.
    if state.effects.no_element {
        g.armour_element = 0;
    }
    g
}

/// The applied skills changed (`ZoneEvent::Buffs`): their effects into the passives and the numbers again.
async fn buffs_changed<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    effects: BuffEffects,
) -> Result<()> {
    if effects == state.effects {
        return Ok(());
    }
    // Hiding stops attacking and casting (`docs/original/skills-special.md` section 3).
    let hidden = effects.shape == sk::SHAPE_HIDDEN && state.effects.shape != sk::SHAPE_HIDDEN;
    state.effects = effects;
    state.passives = with_buffs(state.skill_passives, &effects);
    if hidden {
        stop_attacking(ctx, state, character.id);
    }
    renumber(conn, ctx, character, state).await
}

/// NPC types of merchants and gatekeepers.
const MERCHANT: u8 = 1;
const GATEKEEPER: u8 = 2;
/// How close a player must be to a gatekeeper (`SVector::Distance2`).
const GATEKEEPER_RANGE: f32 = 30.0;

/// The move speed check on a step that passed the step check (`CUser::PacketPC` 0x46a36d, `docs/original/move-check.md`): false
/// when refused, with `state.position` then the window's start. Riding is checked at 8.0, a passenger not at all.
fn speed_checked<S>(ctx: &Context<S>, state: &mut WorldState, id: u32, position: [f32; 3]) -> bool {
    let speed = match ctx.zones.riding(state.map, id) {
        crate::zone::Riding::Behind => return true,
        crate::zone::Riding::Riding(_) => move_check::RIDING,
        _ => move_check::foot_speed(state.totals.speeds().1),
    };
    match state.move_check.update(Instant::now(), position, speed) {
        move_check::Checked::Open => true,
        move_check::Checked::Passed(w) => {
            tracing::debug!(id, walked = w.walked, seconds = w.seconds, average = w.walked / w.seconds, fastest = w.fastest, "move window");
            true
        }
        move_check::Checked::Refused { window: w, to } => {
            tracing::info!(
                id,
                walked = w.walked,
                allowed = w.allowed,
                seconds = w.seconds,
                speed = w.fastest,
                ?to,
                "move refused: too fast"
            );
            state.position = to;
            false
        }
    }
}

/// Carries out a move (`CUser::UpdateResetPosition`, type 1; `docs/original/gatekeeper.md` section 3). On the same map the
/// view is swapped and everyone in view gets `0x020B`; to another map the player leaves its zone, gets `0x020B` with the new
/// map, and is placed there when the client has loaded it and sends `0x0201`. `own`: an operator's summon (`F904`, `F905`, `F90C`),
/// which the player is told with the command's own packet, after the view's on the same map and instead of `0x020B` on another.
async fn move_player<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    state: &mut WorldState,
    id: u32,
    to_map: u16,
    position: [f32; 3],
    own: Option<u16>,
) -> Result<()> {
    let map = state.map;
    state.position = position;
    // A move by the server starts the move check again (`CUser::MoveByGame`; another map's zone entry does it too).
    state.move_check.reset(Instant::now(), position);
    stop_attacking(ctx, state, id);
    // A move carried out ends a personal shop (`CUser::UpdateResetPosition`).
    ctx.zones.shop_end(id);
    if to_map == map {
        for packet in ctx.zones.warp(map, id, position) {
            conn.send(packet).await?;
        }
        if let Some(op) = own {
            conn.send(shaiya_proto::admin::summoned(op, id, crate::world::map_of(to_map), position)).await?;
        }
    } else {
        leave_zone(conn, ctx, state, id).await?;
        state.map = to_map;
        // `to_map` may be an instance zone: the client gets its map.
        conn.send(match own {
            Some(op) => shaiya_proto::admin::summoned(op, id, crate::world::map_of(to_map), position),
            None => w::position(id, crate::world::map_of(to_map), position),
        })
        .await?;
    }
    Ok(())
}

/// Out of the zone, to wait for `0x0201` (a move to another map, `docs/original/gatekeeper.md` section 3; a seat at a gambling table,
/// `docs/original/mini-game.md` section 3): the trade and the summon end, a rider keeps its mount for the next zone, the others see
/// the player go. `state.map` is the zone left.
async fn leave_zone<S: Backend>(conn: &mut Connection, ctx: &Context<S>, state: &mut WorldState, id: u32) -> Result<()> {
    let map = state.map;
    ctx.zones.cancel_actions(id);
    // A summon ends with the zone; a rider rides on into the next one (`docs/original/vehicles.md` section 4).
    vehicles::cancel_summon(ctx, state, id);
    state.mount = match ctx.zones.riding(map, id) {
        crate::zone::Riding::Riding(mount) => Some(mount),
        _ => None,
    };
    if let Some(left) = ctx.zones.leave(map, id) {
        for packet in duel::left_zone(ctx, id, state, left).await? {
            conn.send(packet).await?;
        }
    }
    state.entered = false;
    script::left_zone(ctx, state);
    Ok(())
}

/// How close a player must be to a portal (`SVector::Distance2`).
const PORTAL_RANGE: f32 = 40.0;
/// `0x020A` refusals (`docs/original/portals.md`).
const PORTAL_NEEDS_GUILD: u8 = 0;

enum PortalUse {
    /// The original does not answer.
    Ignored,
    Refused(u8),
    /// To a zone (an instance zone's key for a party instance map) and a position.
    Move(u16, [f32; 3]),
}

/// What a `0x020A` does (`docs/original/portals.md` section 2; party instance maps `docs/original/instances.md` sections 3 and 5).
/// Guild maps refuse as the original does for a player without a guild.
fn portal_use<S>(ctx: &Context<S>, state: &WorldState, id: u32, country: u8, level: u16, index: u8, confirmed: bool) -> PortalUse {
    let world = &ctx.data.world;
    let Some(portal) = world.portal(state.map, index) else { return PortalUse::Ignored };
    if crate::zone::distance(state.position, portal.position) >= PORTAL_RANGE {
        return PortalUse::Ignored;
    }
    // On a map with a CreateType an instance portal's country is the zone's (`CZone::GetInsZonePortalCountry`); on a static map the
    // map's, which a boss group may have changed (`docs/original/obelisks.md` section 11.3).
    let portal_country = if world.create_type(state.map) != 0 && portal.country > crate::instance::PORTAL_ID_FLOOR {
        ctx.zones.instance_portal_country(state.map, portal.country)
    } else if world.create_type(state.map) == 0 {
        ctx.zones.portal_country(state.map, index, portal.country)
    } else {
        portal.country
    };
    let allowed = match portal_country {
        0 => true,
        1 => country == 0,
        2 => country == 1,
        _ => false,
    };
    let level = u32::from(level);
    let in_levels = (portal.level_min == 0 && portal.level_max == 0) || (portal.level_min..=portal.level_max).contains(&level);
    if !allowed || !in_levels {
        return PortalUse::Ignored;
    }
    let Ok(to_map) = u16::try_from(portal.to_map) else { return PortalUse::Ignored };
    // Unlike the original, which closes the connection when the player arrives, a destination off its map is refused here.
    if !world.is_safe_pos(to_map, portal.to) {
        return PortalUse::Ignored;
    }
    match world.create_type(to_map) {
        crate::world::create_type::PARTY => {
            use chrono::{Datelike, Timelike};
            let rules = world.instance_rules(to_map);
            let now = chrono::Local::now();
            let open = rules.is_open(now.weekday().num_days_from_sunday() as u16, now.hour() as u16);
            match ctx.zones.enter_instance(to_map, id, state.map, &rules, open, world, &ctx.data.live().mobs) {
                Err(reason) => PortalUse::Refused(reason),
                // A pending logout keeps the player in place, after the checks (`m_nLogoutType`).
                Ok(_) if state.logout.is_some() => {
                    ctx.zones.unbook(id);
                    PortalUse::Ignored
                }
                Ok(zone) => PortalUse::Move(zone, portal.to),
            }
        }
        crate::world::create_type::GUILD_HOUSE => {
            match ctx.zones.enter_guild_house(to_map, id, state.map, world, &ctx.data.live().mobs) {
                Err(reason) => PortalUse::Refused(reason),
                // A pending logout keeps the player in place, after the checks (`m_nLogoutType`).
                Ok(_) if state.logout.is_some() => {
                    ctx.zones.unbook(id);
                    PortalUse::Ignored
                }
                Ok(zone) => PortalUse::Move(zone, portal.to),
            }
        }
        crate::world::create_type::GUILD_RANKING => {
            match ctx.zones.enter_ranking(to_map, id, state.map, confirmed, world, &ctx.data.live().mobs) {
                Err(reason) => PortalUse::Refused(reason),
                // A pending logout keeps the player in place, after the checks (`m_nLogoutType`).
                Ok(_) if state.logout.is_some() => {
                    ctx.zones.unbook(id);
                    PortalUse::Ignored
                }
                Ok(zone) => PortalUse::Move(zone, portal.to),
            }
        }
        crate::world::create_type::GUILD => PortalUse::Refused(PORTAL_NEEDS_GUILD),
        // A pending logout keeps the player in place (`m_nLogoutType`), as in the original.
        _ if state.logout.is_some() => PortalUse::Ignored,
        _ => PortalUse::Move(to_map, portal.to),
    }
}

/// The destination a `0x020C` asks for, if the original's checks pass (`docs/original/gatekeeper.md` section 2); in a guild house
/// the house's rate and refusal (`0x0D28`, `docs/original/guild-house.md` section 9). Unlike the original, a pending logout refuses
/// it rather than taking the money without moving.
fn usable_gate<S>(
    ctx: &Context<S>,
    state: &WorldState,
    id: u32,
    country: u8,
    npc_id: u32,
    destination: u8,
) -> Result<Gate, Option<Vec<u8>>> {
    if state.logout.is_some() {
        return Err(None);
    }
    let npc = ctx.zones.npc(state.map, npc_id).filter(|npc| npc.kind == GATEKEEPER).ok_or(None)?;
    if crate::zone::distance(state.position, npc.position) >= GATEKEEPER_RANGE {
        return Err(None);
    }
    let keeper = ctx.data.world.gatekeeper(npc.type_id).ok_or(None)?;
    if keeper.country != 2 && keeper.country != country {
        return Err(None);
    }
    let mut gate = keeper.gates.get(usize::from(destination)).copied().flatten().ok_or(None)?;
    if let Some(rate) = guild_house::gate_rate(ctx, state.map, id, npc.type_id) {
        gate.cost = crate::guild_house::scaled(gate.cost, rate.map_err(Some)?);
    }
    if gate.cost <= state.money && ctx.data.world.is_safe_pos(gate.map, gate.position) { Ok(gate) } else { Err(None) }
}

/// `0x0702` (`docs/original/shop.md` section 2). `None` when the original would not answer.
async fn buy<S: Backend>(
    ctx: &Context<S>,
    map: u16,
    character: &PlayCharacter,
    state: &mut WorldState,
    request: &BuyRequest,
) -> Result<Option<Vec<u8>>> {
    let character_id = character.id;
    let Some(npc) = ctx.zones.npc_in_view(map, character_id, request.npc_id) else { return Ok(None) };
    if npc.kind != MERCHANT {
        return Ok(None);
    }
    let goods = ctx.data.world.shop(npc.type_id).unwrap_or_default();
    let Some(&(item_type, type_id)) = goods.get(usize::from(request.index)) else { return Ok(None) };
    let Some(def) = ctx.data.items.get(item_type, type_id) else { return Ok(None) };
    if request.count == 0 || request.count > def.max_count {
        return Ok(None);
    }
    // In a guild house the house's rate, or its refusal and nothing else (`docs/original/guild-house.md` section 9).
    let house_def;
    let def = match guild_house::shop_rate(ctx, map, character_id, npc.type_id) {
        None => def,
        Some(Err(refused)) => return Ok(Some(refused)),
        Some(Ok(rate)) => {
            house_def = crate::inventory::ItemDef { buy: crate::guild_house::scaled(def.buy, rate), ..*def };
            &house_def
        }
    };
    let reply = match state.inventory.buy(state.money, def, request.count) {
        Buy::NoMoney => ShopResult { result: shop::result::NO_MONEY, item_type, type_id, count: request.count, ..Default::default() },
        Buy::NoRoom => ShopResult { result: shop::result::NO_ROOM, item_type, type_id, ..Default::default() },
        Buy::Done { price, change, bag, slot, count } => {
            let Some(rows) = ctx.store.trade(character_id, -i64::from(price), std::slice::from_ref(&change)).await? else {
                bail!("the database refused a purchase the server allowed: {change:?}");
            };
            state.inventory.apply(&change);
            state.money -= price;
            // `0x0403` (`CUser::PacketShop`): the item bought, named by the row the purchase made or topped up; `count` is how many
            // were bought, the shop's NPC the one asked.
            if let Some(item) = record::item_in(ctx, state, (bag, slot), rows[0].row) {
                ctx.game_log.action(crate::game_log::shop_buy(
                    record::header(character, state),
                    &item,
                    request.npc_id,
                    request.count,
                    price,
                    state.money,
                ));
            }
            tracing::info!(character_id, item_type, type_id, count = request.count, price, money = state.money, "bought");
            ShopResult { result: shop::result::BOUGHT_OR_SOLD, bag, slot, item_type, type_id, count, money: state.money }
        }
    };
    Ok(Some(shop::shop_result(shop::OP_BUY, &reply)))
}

/// `0x0703` (`docs/original/shop.md` section 3). The packet names no NPC; like the original, selling works anywhere.
async fn sell<S: Backend>(
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    bag: u8,
    slot: u8,
    count: u8,
) -> Result<Option<Vec<u8>>> {
    let character_id = character.id;
    let Some(sale) = state.inventory.sell(state.money, &ctx.data.items, bag, slot, count) else { return Ok(None) };
    let Some(rows) = ctx.store.trade(character_id, i64::from(sale.gain), std::slice::from_ref(&sale.change)).await? else {
        bail!("the database refused a sale the server allowed: {:?}", sale.change);
    };
    // `0x0404`: the item as it left the slot (the count sold); the packet names no NPC (NULL).
    let sold = state.inventory.get(bag, slot).cloned();
    state.inventory.apply(&sale.change);
    state.money += sale.gain;
    if let Some(unit) = sold {
        let item = ctx.game_log.item(&ItemUnit { count, ..unit }, rows[0].row);
        ctx.game_log.action(crate::game_log::shop_sell(
            record::header(character, state),
            &item,
            None,
            count,
            sale.gain,
            state.money,
            bag,
            slot,
        ));
    }
    tracing::info!(character_id, item_type = sale.item_type, type_id = sale.type_id, count, gain = sale.gain, money = state.money, "sold");
    let reply = ShopResult {
        result: shop::result::BOUGHT_OR_SOLD,
        bag,
        slot,
        item_type: sale.item_type,
        type_id: sale.type_id,
        count: sale.left,
        money: state.money,
    };
    Ok(Some(shop::shop_result(shop::OP_SELL, &reply)))
}

/// NPC type of the warehouse keepers (`docs/original/maps-npcs.md`).
const WAREHOUSE_KEEPER: u8 = 6;

/// A move into, out of or inside the warehouse (`docs/original/warehouse.md` sections 3 to 6): its answer, or `None` when nothing
/// happens (no answer, as the original). It needs a warehouse keeper in view or the Warehouse Recall buff (deviation 1).
async fn warehouse_move<S: Backend>(
    ctx: &Context<S>,
    map: u16,
    character: &PlayCharacter,
    state: &mut WorldState,
    from: (u8, u8),
    to: (u8, u8),
) -> Result<Option<Vec<u8>>> {
    let character_id = character.id;
    if !state.effects.bank_anywhere && !ctx.zones.npc_kind_in_view(map, character_id, WAREHOUSE_KEEPER) {
        return Ok(None);
    }
    let Some(done) = state.inventory.bank_move(&ctx.data.items, state.money, state.effects.bank_second_half, from, to) else {
        return Ok(None);
    };
    let Some(rows) = ctx.store.trade(character_id, -i64::from(done.fee), &done.changes).await? else {
        bail!("the database refused a warehouse move the server allowed: {:?}", done.changes);
    };
    // `0x040B` into the warehouse, `0x040C` out of it with the fee (`CUser::ItemBagToBank`, `ItemBankToBag`); a move inside the
    // warehouse is not logged. The item is named by the row that moved, with all of its count.
    let moved = state.inventory.at(from).cloned();
    for change in &done.changes {
        state.inventory.apply(change);
    }
    state.money -= done.fee;
    if let (Some(unit), [SlotChange::Move { .. }], [moved_row]) = (moved, &done.changes[..], &rows[..]) {
        let item = ctx.game_log.item(&unit, moved_row.row);
        let header = record::header(character, state);
        if to.0 == BANK && from.0 != BANK {
            ctx.game_log.action(crate::game_log::bag_to_bank(header, &item, unit.count, from.0, from.1, to.1));
        } else if from.0 == BANK && to.0 != BANK {
            ctx.game_log.action(crate::game_log::bank_to_bag(header, &item, unit.count, from.1, to.0, to.1, done.fee));
        }
    }
    tracing::info!(character_id, ?from, ?to, fee = done.fee, money = state.money, "warehouse move");
    let (from_item, to_item) = (state.inventory.at(from), state.inventory.at(to));
    Ok(Some(if from.0 == BANK && to.0 != BANK {
        w::bank_to_bag_result(from, from_item, to, to_item, state.money)
    } else {
        w::item_move_result(from, from_item, to, to_item)
    }))
}

/// The attack time has come (`CUser::UpdateAttack`): a cast ends, a skill is used, or a normal blow falls (`docs/original/combat.md`
/// sections 2 and 3).
async fn attack_tick<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &mut PlayCharacter,
    state: &mut WorldState,
    country: u8,
) -> Result<()> {
    if state.casting {
        return cast_done(conn, ctx, character, state, country).await;
    }
    // Stunned, asleep or rooted: the attack stops (`docs/original/combat.md` section 2).
    if state.speeds().0 == buffs::CANNOT {
        stop_attacking(ctx, state, character.id);
        conn.send(cp::attack_refused(combat::RESULT_CANNOT_ATTACK, character.id)).await?;
        return Ok(());
    }
    // Attacking ends hiding and transformations but Evolution (`CUser::RemApplySkillToMOB`, `docs/original/skills-special.md`
    // section 6); the numbers follow before the blow.
    if ctx.zones.end_shape_by_attack(state.map, character.id)
        && let Some(effects) = ctx.zones.buff_effects(state.map, character.id)
    {
        buffs_changed(conn, ctx, character, state, effects).await?;
    }
    if let Some(number) = state.next_skill {
        return skill_tick(conn, ctx, character, state, country, number).await;
    }
    let (id, map, now) = (character.id, state.map, Instant::now());
    // Normal blows fall on a monster or on another player (`docs/original/pvp.md` section 3).
    let target = match state.target {
        Some(SkillTarget::Player(target)) if target == id => None,
        other => other,
    };
    let Some(target) = target else {
        stop_attacking(ctx, state, id);
        return Ok(());
    };
    let (attack_speed, move_speed) = state.totals.speeds();
    let gear = gear(ctx, state);
    let Some(kind) = gear.kind else {
        stop_attacking(ctx, state, id);
        conn.send(cp::attack_refused(combat::RESULT_NO_WEAPON, id)).await?;
        return Ok(());
    };
    let fighter = combat::player(character.level, state.totals.stats, &gear);
    let attack = Attack {
        attacker: id,
        country,
        fighter: &fighter,
        kind,
        slack: combat::range_slack(move_speed, state.last_move.map(|at| now - at)),
        range: gear.range,
        rec: state.totals.stats[2],
        cross_road: gear.cross_road,
    };
    let outcome = {
        use rand::Rng;
        let mut rng = rand::rng();
        // Like the C library's rand(): 0 to 32767.
        let mut rand = || rng.random::<u32>() & 0x7fff;
        let live = ctx.data.live();
        let loot = (&live.drops, &ctx.data.items);
        match target {
            SkillTarget::Mob(mob) => ctx.zones.attack_mob(map, mob, &attack, &live.mobs, loot, now, &mut rand),
            SkillTarget::Player(target) => {
                let war = ctx.data.world.is_war_map(map);
                ctx.zones.attack_player(map, target, &attack, war, &live.mobs, loot, now, &mut rand)
            }
        }
    };
    match outcome {
        AttackOutcome::Gone => stop_attacking(ctx, state, id),
        AttackOutcome::OutOfRange => {
            state.next_attack = now + combat::attack_interval(attack_speed);
            stop_attacking(ctx, state, id);
            conn.send(cp::attack_refused(combat::RESULT_OUT_OF_RANGE, id)).await?;
        }
        AttackOutcome::Struck { packets, killed, critical } => {
            state.next_attack = now + combat::attack_interval(attack_speed);
            if critical {
                wear_weapon(conn, ctx, character, state).await?;
            }
            if killed {
                stop_attacking(ctx, state, id);
            }
            for packet in packets {
                conn.send(packet).await?;
            }
        }
    }
    Ok(())
}

/// The faction grade's types read here: the experience loss spared at death (`docs/original/pvp-2.md` section 8).
const EXP_LOSS_KEPT: usize = 25;

/// The map where `0x0511` does nothing (when it has a war type).
const NO_PLAYER_SKILLS_MAP: u16 = 42;

/// Range of skill casting packets (`CZone::PSendView` with 60).
const CAST_RANGE: f32 = 60.0;
/// A cast takes `byReadyTime` quarters of a second.
const READY_TIME_UNIT: Duration = Duration::from_millis(250);

/// `0x0511`/`0x0517`: a skill for the next attack time (`docs/original/skills-use.md` sections 2 and 3). Nothing happens yet but
/// the choice of target; an unsuitable target gets result 11 and changes nothing.
#[allow(clippy::too_many_arguments)]
async fn request_skill<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    country: u8,
    opcode: u16,
    number: u8,
    named: u32,
) -> Result<()> {
    let (id, map) = (character.id, state.map);
    // Number 255 with a target cancels one's own buff with that key (`docs/original/skills-buffs.md` section 7).
    if opcode == sk::OP_USE_USER && number == sk::CANCEL_NUMBER && named != 0 {
        for packet in ctx.zones.cancel_buff(map, id, named, Instant::now()) {
            conn.send(packet).await?;
        }
        return Ok(());
    }
    // Not while casting, held (attack speed 255) or sitting, nor transformed (hidden is fine: `docs/original/skills-special.md`
    // section 2).
    if state.casting
        || state.speeds().0 == buffs::CANNOT
        || state.motion == w::MOTION_SIT
        || !matches!(state.effects.shape, 0 | sk::SHAPE_HIDDEN)
    {
        return Ok(());
    }
    let Some((_, def)) = learnt(ctx, character, number) else { return Ok(()) };
    if !skills::usable(def, character.level, character.job, character.grow) {
        return Ok(());
    }
    // `0x0511` is ignored on map 42 while it has a war type (`CUser::ChkTargetUser`, `docs/original/pvp-2.md` section 1).
    if opcode == sk::OP_USE_USER && map == NO_PLAYER_SKILLS_MAP && ctx.data.world.is_war_map(map) {
        tracing::debug!(id, skill = def.id, "player skill ignored on map 42");
        return Ok(());
    }
    let harms = matches!(def.type_effect, skills::ATTACK | skills::DEBUFF);
    let (mut look, mut illusion_mob) = (None, 0);
    let target = match skills::aim(def) {
        // An illusion copies the monster aimed at, a disguise the player (`CUser::ChkTargetMob`, `ChkTargetUser`, section 5).
        Aim::Oneself if def.kind == skills::TRANSFORMATION => match (def.change_type, opcode) {
            (buffs::SHAPE_ILLUSION, sk::OP_USE_MOB) => {
                ctx.zones.illusion_mob(map, named, character.level, &ctx.data.live().mobs).map(|mob| {
                    illusion_mob = mob;
                    SkillTarget::Player(id)
                })
            }
            (buffs::SHAPE_DISGUISE, sk::OP_USE_USER) => ctx.zones.copy_looks(map, id, named, country, character.level).map(|copy| {
                look = Some(copy);
                SkillTarget::Player(id)
            }),
            (buffs::SHAPE_ILLUSION | buffs::SHAPE_DISGUISE, _) => None,
            _ => Some(SkillTarget::Player(id)),
        },
        Aim::Oneself => Some(SkillTarget::Player(id)),
        Aim::Named if opcode == sk::OP_USE_MOB => {
            (harms && def.kind != skills::TRANSFORMATION && ctx.zones.attackable(map, named, country, &ctx.data.live().mobs))
                .then_some(SkillTarget::Mob(named))
        }
        // An enemy attacked or debuffed (`docs/original/pvp-2.md` section 1: alive, not hidden, not of the caster's side).
        Aim::Named if harms => match ctx.zones.attack_verdict(map, id, named, ctx.data.world.is_war_map(map)) {
            crate::zone::Verdict::Yes => Some(SkillTarget::Player(named)),
            crate::zone::Verdict::No => None,
            crate::zone::Verdict::Ruled(refusal) => {
                tracing::debug!(id, map, point = "attack", rule = &*refusal.name, "refused by a rule");
                refused::tell_refused(conn, state, &refusal).await?;
                None
            }
        },
        // A player who is not hidden (`EnableAttackUser`): a resurrection reaches one's own side, the dead too (ultimate mode needs
        // level 2 or 3); scouting the other side; anything else a living player of the same country (`IsTeamUsable`).
        Aim::Named => {
            let fits = ctx.zones.whereabouts(map, named).is_some_and(|w| {
                w.shape != sk::SHAPE_HIDDEN
                    && match (def.type_effect, def.kind) {
                        (skills::ONCE, skills::RESURRECTION) => w.country == country && (w.grow != ULTIMATE || def.level >= 2),
                        (skills::ONCE, _) => w.country != country && !w.dead,
                        // Evolution not on someone mounted (`CUser::ChkTargetUser`, `docs/original/vehicles.md` section 2).
                        // Nobody helps or is helped by a duellist but itself (`CUser::IsTeamUsable`, `docs/original/duel.md` section 5);
                        // help stays within one side of a guild war (`docs/original/guild-war.md` section 5).
                        _ => {
                            let me = ctx.zones.whereabouts(map, id);
                            let duelling = named != id && (w.fighting || me.is_some_and(|me| me.fighting));
                            let side = named == id || crate::guild_war::may_help(w.war, me.map_or(0, |me| me.war));
                            w.country == country && !w.dead && !(def.id == buffs::EVOLUTION && w.mounted) && !duelling && side
                        }
                    }
            });
            fits.then_some(SkillTarget::Player(named))
        }
        Aim::Wrong => None,
    };
    let Some(target) = target else {
        tracing::debug!(id, skill = def.id, named, "skill refused: wrong target");
        conn.send(sk::skill_use(opcode, skills::RESULT_WRONG_TARGET, id, 0, def.id, def.level, [0; 3])).await?;
        return Ok(());
    };
    if state.target.is_none() {
        conn.send(cp::attack_start()).await?;
        ctx.zones.set_attacking(map, id, true);
    }
    // A skill on oneself keeps the monster or player attacked to go back to (`m_TargetPrev`).
    if target == SkillTarget::Player(id)
        && let Some(previous) = state.target.filter(|t| *t != SkillTarget::Player(id))
    {
        state.prev_target = Some(previous);
    }
    state.target = Some(target);
    state.next_skill = Some(number);
    state.next_attack = state.next_attack.max(Instant::now());
    (state.look, state.illusion_mob) = (look, illusion_mob);
    Ok(())
}

/// The learnt skill at a number and its definition.
fn learnt<'a, S>(ctx: &'a Context<S>, character: &PlayCharacter, number: u8) -> Option<(skills::Skill, &'a SkillDef)> {
    let skill = character.skills.iter().find(|s| s.number == number).copied()?;
    Some((skill, ctx.data.skills.get(skill.id, skill.level)?))
}

/// A resurrection aims at the dead (`CUser::EnableAttackUser` with `wType` 121).
fn resurrects(def: &SkillDef) -> bool {
    def.type_effect == skills::ONCE && def.kind == skills::RESURRECTION
}

/// Where the skill's target is when it can still be aimed at (`CUser::UpdateAttack`): a monster still attackable (with its size),
/// a player in the zone, not hidden, and alive (`dead_ok`: a resurrection's target may be dead).
fn target_spot<S>(ctx: &Context<S>, state: &WorldState, country: u8, target: SkillTarget, dead_ok: bool) -> Option<([f32; 3], u8)> {
    match target {
        SkillTarget::Mob(m) => {
            if !ctx.zones.attackable(state.map, m, country, &ctx.data.live().mobs) {
                return None;
            }
            let mob = ctx.zones.mob(state.map, m)?;
            Some((mob.position, ctx.data.live().mobs.get(mob.mob_id).map_or(0, |d| d.size)))
        }
        SkillTarget::Player(p) => {
            ctx.zones.whereabouts(state.map, p).filter(|w| (dead_ok || !w.dead) && w.shape != sk::SHAPE_HIDDEN).map(|w| (w.position, 0))
        }
    }
}

/// A skill's turn (`CUser::UseSkillMob`/`UseSkillUser`, `docs/original/skills-use.md` sections 4 to 6): the checks, then a cast or
/// the effect.
async fn skill_tick<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &mut PlayCharacter,
    state: &mut WorldState,
    country: u8,
    number: u8,
) -> Result<()> {
    let (id, map, now) = (character.id, state.map, Instant::now());
    let Some((_, def)) = learnt(ctx, character, number) else {
        stop_attacking(ctx, state, id);
        conn.send(cp::attack_refused(combat::RESULT_CANNOT_ATTACK, id)).await?;
        return Ok(());
    };
    let Some(target) = state.target else { return Ok(()) };
    let Some((spot, size)) = target_spot(ctx, state, country, target, resurrects(def)) else {
        stop_attacking(ctx, state, id);
        return Ok(());
    };
    let gear = gear(ctx, state);
    let in_range = target == SkillTarget::Player(id) || {
        let slack = combat::range_slack(state.totals.speeds().1, state.last_move.map(|at| now - at));
        combat::in_range(crate::zone::distance(state.position, spot), slack, size, gear.range + i32::from(def.attack_range))
    };
    let health = ctx.zones.health(map, id).unwrap_or(state.health);
    let b = &state.passives.bonuses;
    let weapon =
        state.inventory.get(0, 5).and_then(|item| ctx.data.items.get(item.item_type, item.type_id)).map(|d| real_type(d.item_type));
    let caster = skills::Caster {
        sp: health[2],
        mp: health[1],
        cost_reduction: b.cost_reduction,
        weapon,
        shield: state.inventory.get(0, 6).is_some(),
        disabled: b.disabled,
        cool_time: b.cool_time,
    };
    if let Err(result) = skills::check_use(def, &caster, &character.skills, |skill| state.cooldowns.get(&skill).copied(), in_range, now) {
        tracing::debug!(id, skill = def.id, result, "skill refused");
        // 0x0517 for a monster target, except result 8 and the monster path's result 10.
        let opcode = match (target, result) {
            (SkillTarget::Mob(_), r) if r != skills::RESULT_PREVIOUS && r != skills::RESULT_CANNOT_ATTACK => sk::OP_USE_MOB,
            _ => sk::OP_USE_USER,
        };
        conn.send(sk::skill_use(opcode, result, id, 0, def.id, def.level, [0; 3])).await?;
        // Back to normal blows in attack mode, else a stop; the next attack time stays.
        if state.attack_mode {
            back_to_blows(ctx, state, id);
        } else {
            stop_attacking(ctx, state, id);
        }
        return Ok(());
    }
    if def.ready_time != 0 {
        state.casting = true;
        let packet = match target {
            SkillTarget::Mob(m) => sk::casting(sk::OP_CAST_MOB, id, m, def.id, def.level),
            SkillTarget::Player(p) => sk::casting(sk::OP_CAST_USER, id, p, def.id, def.level),
        };
        conn.send(ctx.zones.send_near(map, id, packet, CAST_RANGE)).await?;
        state.next_attack = now + READY_TIME_UNIT * u32::from(def.ready_time);
        return Ok(());
    }
    go_off(conn, ctx, character, state, country, def, target).await
}

/// A cast's end (`CUser::UpdateAttack` in status 3, section 4): the effect without the checks again.
async fn cast_done<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &mut PlayCharacter,
    state: &mut WorldState,
    country: u8,
) -> Result<()> {
    let id = character.id;
    state.casting = false;
    let skill = state.next_skill.and_then(|number| learnt(ctx, character, number));
    let (Some((_, def)), Some(target)) = (skill, state.target) else {
        stop_attacking(ctx, state, id);
        return Ok(());
    };
    if target_spot(ctx, state, country, target, resurrects(def)).is_none() {
        stop_attacking(ctx, state, id);
        return Ok(());
    }
    go_off(conn, ctx, character, state, country, def, target).await
}

/// The skill goes off (section 7): the cooldown, the effect and the costs (the zone's), then `CUser::StatusNextAttack`.
async fn go_off<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &mut PlayCharacter,
    state: &mut WorldState,
    country: u8,
    def: &SkillDef,
    target: SkillTarget,
) -> Result<()> {
    let (id, map, now) = (character.id, state.map, Instant::now());
    let gear = gear(ctx, state);
    let fighter = combat::player(character.level, state.totals.stats, &gear);
    let b = &state.passives.bonuses;
    let cast = SkillCast {
        caster: id,
        country,
        skill: def,
        fighter: &fighter,
        rec: state.totals.stats[2],
        cross_road: gear.cross_road,
        cost_reduction: b.cost_reduction,
        fighting: state.prev_mob(),
        look: state.look.as_ref(),
        illusion_mob: state.illusion_mob,
        war_map: ctx.data.world.is_war_map(map),
    };
    let cool_time = b.cool_time;
    let outcome = {
        use rand::Rng;
        let mut rng = rand::rng();
        let live = ctx.data.live();
        let loot = (&live.drops, &ctx.data.items);
        // Like the C library's rand(): 0 to 32767.
        ctx.zones.use_skill(map, target, &cast, &live.mobs, loot, now, &mut || rng.random::<u32>() & 0x7fff)
    };
    let Some(outcome) = outcome else {
        stop_attacking(ctx, state, id);
        return Ok(());
    };
    state.cooldowns.insert(def.id, skills::cooldown_end(def, cool_time, now));
    // Charge's dash is the client's: its reach comes off the distance walked, hit or miss (`CUser::AttackMob`, `AttackUser`).
    if def.id == move_check::CHARGE {
        state.move_check.shorten(f32::from(def.attack_range));
    }
    tracing::debug!(id, skill = def.id, level = def.level, ?target, killed = outcome.killed, died = outcome.died, "skill used");
    for packet in outcome.packets {
        conn.send(packet).await?;
    }
    for _ in 0..outcome.criticals {
        wear_weapon(conn, ctx, character, state).await?;
    }
    if outcome.died {
        died(ctx, state, id);
        return Ok(());
    }
    if !outcome.killed && state.attack_mode {
        back_to_blows(ctx, state, id);
    } else {
        stop_attacking(ctx, state, id);
    }
    state.next_attack = now + combat::attack_interval(state.totals.speeds().0);
    // What the skill applied counts at once: the numbers, and hiding stops attacking after all (`CUser::AddApplySkillBuff`
    // 88 runs `StatusAttackStop` before `StatusNextAttack`, `docs/original/skills-special.md` section 3).
    if let Some(effects) = ctx.zones.buff_effects(map, id) {
        buffs_changed(conn, ctx, character, state, effects).await?;
    }
    // Return: to the bind position of this map, unless a logout is pending (`docs/original/skills-special.md` section 9).
    if outcome.recall && state.logout.is_none() {
        let (to_map, position) = {
            use rand::Rng;
            let mut rng = rand::rng();
            ctx.data.world.bind_pos(map, country, state.position, &mut || rng.random::<u32>() & 0x7fff)
        };
        tracing::info!(id, from = map, to = to_map, ?position, "return");
        move_player(conn, ctx, state, id, to_map, position, None).await?;
    }
    Ok(())
}

/// Normal blows next (`CUser::StatusNextAttack` in attack mode): after a skill on oneself, on the monster attacked before, if any.
fn back_to_blows<S>(ctx: &Context<S>, state: &mut WorldState, id: u32) {
    state.next_skill = None;
    match state.target {
        Some(SkillTarget::Player(p)) if p == id => match state.prev_target.take() {
            Some(previous) => state.target = Some(previous),
            None => stop_attacking(ctx, state, id),
        },
        // An enemy player or a monster: normal blows go on against it (`docs/original/pvp-2.md` section 1).
        _ => {}
    }
}

/// Moving stops a cast (`CUser::StatusCastingStopByMove`, section 5): nothing is sent or spent; a skill on oneself goes back to the
/// monster attacked before, then normal blows go on at once in attack mode, otherwise the player stops.
fn cancel_cast<S>(ctx: &Context<S>, state: &mut WorldState, id: u32) {
    state.casting = false;
    if state.attack_mode {
        back_to_blows(ctx, state, id);
        state.next_attack = Instant::now();
    } else {
        stop_attacking(ctx, state, id);
    }
}

/// The player died (`CUser::Death`): it stops and waits to come back to life.
/// The item a `0x0A06` names, when it may be offered (`docs/original/trade.md` section 2.3): an inventory bag in use, a count from 1
/// to what is there, not a quest item (real type 27) nor item type 99, not bound (`reqog`; deviation 6).
fn tradeable<'a>(state: &'a WorldState, items: &ItemDefs, offer: tp::Offer) -> Option<&'a ItemUnit> {
    if offer.count == 0 || offer.bag == 0 || offer.bag > state.inventory.max_bag() {
        return None;
    }
    let item = state.inventory.get(offer.bag, offer.slot)?;
    let bound = items.get(item.item_type, item.type_id).is_none_or(|def| def.bound);
    (real_type(item.item_type) != 27 && item.item_type != 99 && !bound && item.count >= offer.count).then_some(item)
}

/// How long the player who pressed ready last waits for its partner's connection to take the exchange.
const TRADE_WAIT: Duration = Duration::from_secs(30);

/// Both are ready and this player pressed last (`CUser::Exchange`, `docs/original/trade.md` section 5): its side, checked against its
/// bags, is parked; the partner's connection runs the exchange and hands back this side's share. Nothing else happens on this
/// connection meanwhile, so its bags stay as checked.
async fn trade_exchange_last<S: Backend>(ctx: &Context<S>, character: &PlayCharacter, state: &mut WorldState) {
    let id = character.id;
    let Some((offers, money)) = ctx.zones.trade_offers(id) else { return };
    // An offer that is not in the bags as offered cancels the trade for both; nothing moves (audit R3).
    let Some(side) = trade::prepare(id, state.money, &offers, money, &state.inventory) else {
        tracing::info!(id, "trade cancelled: an offered item is gone or smaller");
        ctx.zones.trade_end(id);
        return;
    };
    let (reply, mut outcome) = tokio::sync::oneshot::channel();
    if ctx.zones.trade_park(id, Parked { side, reply }).is_err() {
        return;
    }
    let outcome = match timeout(TRADE_WAIT, &mut outcome).await {
        Ok(outcome) => outcome.ok(),
        Err(_) if ctx.zones.trade_unpark(id) => {
            tracing::warn!(id, "trade cancelled: the partner's connection did not take the exchange");
            None
        }
        // The partner's connection has it: its answer comes.
        Err(_) => outcome.await.ok(),
    };
    match outcome {
        Some(Outcome::Done(settled)) => settle(ctx, character, state, settled),
        Some(Outcome::Failed(code)) => {
            let _ = state.outbox.send(tp::end(code));
        }
        // The trade ended meanwhile; whoever ended it told both.
        None => {}
    }
}

/// The partner pressed ready last and waits (`ZoneEvent::Trade`): this connection checks its own side, makes the exchange, writes it
/// in one transaction, takes its share and hands the partner its own.
async fn trade_exchange<S: Backend>(ctx: &Context<S>, character: &PlayCharacter, state: &mut WorldState) -> Result<()> {
    let id = character.id;
    let Some((parked, offers, money)) = ctx.zones.trade_take_parked(id) else { return Ok(()) };
    let partner = parked.side.id;
    let Some(mine) = trade::prepare(id, state.money, &offers, money, &state.inventory) else {
        tracing::info!(id, partner, "trade cancelled: an offered item is gone or smaller");
        let _ = state.outbox.send(tp::end(tp::END_CANCELLED));
        let _ = parked.reply.send(Outcome::Failed(tp::END_CANCELLED_BY_PARTNER));
        return Ok(());
    };
    let (last, me, handovers) = match trade::plan(&parked.side, &mine) {
        Ok(plan) => plan,
        Err((last_code, my_code)) => {
            tracing::info!(id, partner, last_code, my_code, "trade refused");
            let _ = state.outbox.send(tp::end(my_code));
            let _ = parked.reply.send(Outcome::Failed(last_code));
            return Ok(());
        }
    };
    let saved = ctx.store.exchange([(partner, last.money), (id, me.money)], &handovers).await;
    let Ok(Some(rows)) = saved else {
        let _ = state.outbox.send(tp::end(tp::END_CANCELLED));
        let _ = parked.reply.send(Outcome::Failed(tp::END_CANCELLED));
        saved?;
        bail!("the database refused a trade the server allowed: {handovers:?}");
    };
    tracing::info!(id, partner, items = handovers.len(), money_gained = me.money, "traded");
    let (mut last, mut me) = (last, me);
    for settled in [&mut last, &mut me] {
        for traded in &mut settled.traded {
            // The giver's record names its own row (the rest of a stack, or the whole row that moved); the receiver's the row it got.
            let handed = rows[traded.index];
            traded.row = if traded.gave { handed.giver.or(handed.receiver) } else { handed.receiver };
        }
    }
    // The parked partner is answered first, so a panic in `settle` (its `expect` cannot fire: the plan checked the money) never
    // leaves it waiting (audit N6).
    let _ = parked.reply.send(Outcome::Done(last));
    settle(ctx, character, state, me);
    Ok(())
}

/// One side's share of a saved exchange: its bags and money, and its packets after those already queued for it.
fn settle<S>(ctx: &Context<S>, character: &PlayCharacter, state: &mut WorldState, settled: Settled) {
    record::traded(ctx, character, state, &settled);
    for change in &settled.given {
        state.inventory.apply(change);
    }
    for item in settled.received {
        state.inventory.receive(item);
    }
    state.money = u32::try_from(i64::from(state.money) + settled.money).expect("the database kept the money in range");
    for packet in settled.packets {
        let _ = state.outbox.send(packet);
    }
}

fn died<S>(ctx: &Context<S>, state: &mut WorldState, id: u32) {
    stop_attacking(ctx, state, id);
    ctx.zones.cancel_actions(id);
    state.dead = true;
    state.rebirth_due = Some(Instant::now() + REBIRTH_WAIT);
    state.rebirth_party = false;
}

/// Experience for the player (`CUser::AddExpFromUser`, `CUser::LevelUp`; `docs/original/combat.md` section 5). `own_kill`: the
/// player also made the last hit. Unless `plain` (a quest's reward), the item flags' factor first (`docs/original/item-flags.md`
/// section 2), the full party's bonus being the gain so raised; then the rules' factor for the `source` of the experience
/// (`docs/plans/ext-2-rules.md` point 5).
#[allow(clippy::too_many_arguments)]
async fn award_exp<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &mut PlayCharacter,
    state: &mut WorldState,
    gain: u32,
    bonus: u32,
    own_kill: bool,
    plain: bool,
    source: crate::rules::ExpSource,
) -> Result<()> {
    let (gain, bonus) = if plain {
        (gain, bonus)
    } else {
        let raised = (f64::from(gain) * state.effects.exp_factor()) as u32;
        (raised, if bonus > 0 { raised } else { 0 })
    };
    // A race's two families belong to one faction (creation checks it), so the faction is the family over two.
    let changed = ctx.zones.rules().exp_factor(state.map, &rules_who(character, character.family / 2), source);
    let (before_rules, bonus_before) = (gain, bonus);
    let (gain, bonus) = ruled_exp(gain, bonus, changed.value);
    if changed.value != 1.0 {
        tracing::debug!(id = character.id, map = state.map, point = "exp", rules = ?changed.rules, before = before_rules + bonus_before, after = gain + bonus, "changed by rules");
    }
    let before = Progress {
        level: character.level,
        exp: character.exp,
        stat_point: character.stat_point,
        skill_point: character.skill_point,
        stats: character.stats,
    };
    // A full party's bonus counts on a level-up gain too (deviation, `docs/original/party.md` section 8).
    let total = gain.saturating_add(bonus);
    let Some((after, levels)) = combat::add_exp(before, character.job, character.grow, total, &ctx.data.exp) else { return Ok(()) };
    let (id, map) = (character.id, state.map);
    // At the highest level the total is capped, so less than the gain may count.
    let gained = after.exp - before.exp;
    character.level = after.level;
    character.exp = after.exp;
    character.stat_point = after.stat_point;
    character.skill_point = after.skill_point;
    character.stats = after.stats;
    // `0x0308` (`CUser::LevelUp`): one record per level gained, each at its own level.
    for step in 1..=levels {
        let mut header = record::header(character, state);
        header.level = before.level + step;
        ctx.game_log.action(crate::game_log::level_up(header));
    }
    if gained > 0 {
        let bonus = bonus.min(gained);
        conn.send(cp::exp_gain(gained - bonus, bonus)).await?;
    }
    if levels > 0 {
        announce_level(conn, ctx, character, state, !own_kill).await?;
    }
    let health = ctx.zones.health(map, id).unwrap_or(state.health);
    ctx.store.save_progress(id, &after, health).await?;
    Ok(())
}

/// The gain and the bonus after the rules' factor (`docs/plans/ext-2-rules.md` point 5): each times the product, truncated; a product
/// of exactly 1 changes nothing.
fn ruled_exp(gain: u32, bonus: u32, factor: f64) -> (u32, u32) {
    if factor == 1.0 {
        return (gain, bonus);
    }
    ((f64::from(gain) * factor) as u32, (f64::from(bonus) * factor) as u32)
}

/// What a level change announces (`CUser::LevelUp`): the numbers again, the level-up packet and the maxima to everyone within 60, a
/// full refill, the guard numbers and the party's, guild's and friends' member level. `by_other`: the form for a level not made by the
/// player's own kill (`0x051E`).
async fn announce_level<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    by_other: bool,
) -> Result<()> {
    let (id, map) = (character.id, state.map);
    state.totals = totals(ctx, character, state.inventory.items(), &state.passives, &state.grade);
    let max = state.totals.max_health();
    let bounds = ctx.data.exp.bounds(character.grow, character.level);
    let level_up = cp::level_up(by_other, id, character.level, character.stat_point, character.skill_point, bounds);
    conn.send(ctx.zones.send_near(map, id, level_up, LEVEL_UP_RANGE)).await?;
    conn.send(ctx.zones.send_near(map, id, cp::max_all(id, max), LEVEL_UP_RANGE)).await?;
    // Filled up (CUser::SendRecoverAdd with the maxima).
    if let Some(packet) = ctx.zones.fill(map, id, max) {
        conn.send(packet).await?;
    }
    ctx.zones.set_guard(map, id, guard(ctx, character, state));
    ctx.zones.party_level(id, character.level);
    ctx.zones.guild_level(id, character.level);
    ctx.zones.friend_level(id, character.level);
    tracing::info!(id, level = character.level, "level up");
    Ok(())
}

/// What section 4 of `docs/original/quest.md` needs of the character.
fn quest_who(character: &PlayCharacter) -> Who {
    Who { level: character.level, family: character.family, job: character.job, grow: character.grow }
}

/// The NPC a quest request names, if the player sees it (deviation 1 of `docs/original/quest.md` section 12).
fn quest_npc<S>(ctx: &Context<S>, state: &WorldState, id: u32, npc_id: u32) -> Option<quests::Npc> {
    ctx.zones.npc_in_view(state.map, id, npc_id).map(|n| (n.object_id, n.kind, n.type_id))
}

/// `0x0902`: taking a quest (`docs/original/quest.md` section 5). The item given (type 2) goes first (`0x0206`), then `0x0902`, then
/// each start item (`0x0205`).
async fn quest_start<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    npc_id: u32,
    quest_id: u16,
) -> Result<()> {
    let id = character.id;
    let Some(def) = ctx.data.quests.get(quest_id) else { return Ok(()) };
    let closed = ctx.closed_quests.guard().contains(&quest_id);
    if closed {
        tracing::debug!(id, quest_id, "quest start refused: closed by a GM");
        return Ok(());
    }
    if !state.quests.may_take(def, quest_who(character)) {
        tracing::debug!(id, quest_id, "quest start refused: not allowed, in progress or finished");
        return Ok(());
    }
    let npc = quest_npc(ctx, state, id, npc_id);
    let plan = match quests::plan_start(def, npc, npc_id, used_quest_item(state), &state.inventory, &ctx.data.items) {
        Ok(plan) => plan,
        Err(Refused::NoRoom) => {
            tracing::debug!(id, quest_id, "quest start refused: the start items do not fit");
            return conn.send(gp::pick_up_failed(gp::PICK_NO_ROOM)).await;
        }
        Err(reason) => {
            tracing::debug!(id, quest_id, npc_id, ?reason, "quest start refused");
            return Ok(());
        }
    };
    let seconds = (def.time_limit > 0).then(|| def.time_limit.saturating_mul(60));
    let Some(rows) = ctx.store.start_quest(id, quest_id, seconds, &plan.items.changes).await? else {
        bail!("the database refused a quest start the server allowed: {:?}", plan.items.changes);
    };
    for change in &plan.items.changes {
        state.inventory.apply(change);
    }
    record::created(ctx, character, state, &plan.items.changes, &rows);
    state.quests.take(def, Instant::now());
    sync_quests(ctx, id, &state.quests);
    // `0x0501` (`CUser::QuestStart`): the quest and the NPC asked; the NPC's name has no source here (NULL).
    ctx.game_log.action(crate::game_log::quest_start(record::header(character, state), quest_id, plan.npc_id, &def.name));
    send_item_changes(conn, state, &plan.items.changes, false).await?;
    conn.send(qp::started(plan.npc_id, quest_id)).await?;
    send_item_changes(conn, state, &plan.items.changes, true).await?;
    tracing::info!(id, quest_id, name = %def.name, "quest taken");
    Ok(())
}

/// The packets for a quest's item changes: what is left of the slots it took from (`0x0206`), or the items it put in (`0x0205`).
async fn send_item_changes(conn: &mut Connection, state: &WorldState, changes: &[SlotChange], inserted: bool) -> Result<()> {
    for change in changes {
        match change {
            SlotChange::Insert(n) if inserted => {
                if let Some(item) = state.inventory.get(n.bag, n.slot) {
                    conn.send(gp::picked_up(item)).await?;
                }
            }
            SlotChange::SetCount { bag, slot, .. } | SlotChange::Remove { bag, slot } if !inserted => {
                conn.send(gp::item_left(*bag, *slot, state.inventory.get(*bag, *slot))).await?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// `0x0903`: handing a quest in (`docs/original/quest.md` section 7).
async fn quest_end<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &mut PlayCharacter,
    state: &mut WorldState,
    npc_id: u32,
    quest_id: u16,
) -> Result<()> {
    let id = character.id;
    let (Some(def), Some(active)) = (ctx.data.quests.get(quest_id), state.quests.find(quest_id).copied()) else { return Ok(()) };
    let npc = quest_npc(ctx, state, id, npc_id);
    match quests::end_request(def, &active, npc, &state.inventory) {
        EndOutcome::Ignored => tracing::debug!(id, quest_id, npc_id, "quest end refused: not done or not at its NPC"),
        EndOutcome::Choose => {
            state.quests.set_choosing(quest_id);
            conn.send(qp::choose(quest_id)).await?;
        }
        EndOutcome::Finish(result) => finish_quest(conn, ctx, character, state, def, result, quests::end_npc_id(def, npc_id)).await?,
    }
    Ok(())
}

/// A quest ends well with `result` (`CUser::QuestSuccessResult`, section 7.4): the end items taken (`0x0206`), the experience
/// (`0x0207` or a level-up), then `0x0903` with the money and the reward item, all saved together but the experience.
async fn finish_quest<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &mut PlayCharacter,
    state: &mut WorldState,
    def: &QuestDef,
    result: usize,
    npc_id: u32,
) -> Result<()> {
    let id = character.id;
    ctx.zones.cancel_actions(id);
    let reward = match quests::plan_reward(def, result, &state.inventory, &ctx.data.items) {
        Ok(reward) => reward,
        Err(NoReward::Room) => {
            tracing::debug!(id, quest_id = def.id, "quest end refused: the reward item does not fit");
            return conn.send(gp::pick_up_failed(gp::PICK_NO_ROOM)).await;
        }
        Err(NoReward::Items) => {
            tracing::debug!(id, quest_id = def.id, "quest end refused: the end items are gone");
            return Ok(());
        }
    };
    // Capped at 2^32 - 1, and 0x0903 tells what was added (deviation 7).
    let money = def.results[result].money.min(u32::MAX - state.money);
    let end =
        QuestEnd { quest_id: def.id, success: true, keep: !def.repeatable, money: i64::from(money), changes: reward.items.changes.clone() };
    let Some(rows) = ctx.store.end_quest(id, &end).await? else {
        bail!("the database refused a quest end the server allowed: {end:?}");
    };
    // The items the hand-in took go with the quest's end; the reward item is new.
    for change in &end.changes {
        state.inventory.apply(change);
    }
    record::created(ctx, character, state, &end.changes, &rows);
    state.money += money;
    state.quests.remove(def, true);
    sync_quests(ctx, id, &state.quests);
    // `0x0502` (`CUser::QuestSuccessResult`): done, the money and the reward item.
    let reward_item = reward.item.map(|(_, (item_type, type_id))| {
        (u32::from(item_type) * 1000 + u32::from(type_id), ctx.game_log.item_name(item_type, type_id).to_owned())
    });
    ctx.game_log.action(crate::game_log::quest_end(
        record::header(character, state),
        def.id,
        true,
        money,
        reward_item.as_ref().map(|(item_id, name)| (*item_id, name.as_str(), 1)),
        &def.name,
    ));
    send_item_changes(conn, state, &end.changes, false).await?;
    // The killer id 0 is not the player's: a level-up shows at once (`0x051E`).
    let exp = def.exp(result);
    award_exp(conn, ctx, character, state, exp, 0, false, true, crate::rules::ExpSource::Quest).await?;
    let ((bag, slot), (item_type, type_id)) = reward.item.unwrap_or_default();
    let ended = qp::Ended { npc_id, quest_id: def.id, success: true, result: result as u8, exp, money, bag, slot, item_type, type_id };
    conn.send(qp::ended(&ended)).await?;
    tracing::info!(id, quest_id = def.id, name = %def.name, result, exp, money, item = ?reward.item, "quest done");
    Ok(())
}

/// Tells the zones which quests the character has in progress, for the quest items monsters drop (`ctx.zones.has_quest`); after
/// every change of the list.
fn sync_quests<S: Backend>(ctx: &Context<S>, id: u32, quests: &Quests) {
    ctx.zones.set_quests(id, quests.active().iter().map(|a| a.id).collect());
}

/// A quest fails: given up or out of time (`QuestEnd` with success 0, section 7.3).
async fn fail_quest<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    def: &QuestDef,
    npc_id: u32,
) -> Result<()> {
    let id = character.id;
    let end = QuestEnd { quest_id: def.id, success: false, keep: !def.repeatable, money: 0, changes: Vec::new() };
    if ctx.store.end_quest(id, &end).await?.is_none() {
        bail!("the database refused a quest failure: quest {}", def.id);
    }
    state.quests.remove(def, false);
    sync_quests(ctx, id, &state.quests);
    // `0x0502` with success 0 (`CUser::QuestEnd`).
    ctx.game_log.action(crate::game_log::quest_end(record::header(character, state), def.id, false, 0, None, &def.name));
    conn.send(qp::ended(&qp::Ended { npc_id, quest_id: def.id, ..Default::default() })).await
}

/// A monster this player gets the kill of, alone or with its party (`CUser::QuestUpdateKillMOB`, section 6): the counters that name it
/// move (`0x0905`), and a quest without an end NPC whose kills are complete ends.
async fn quest_kill<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &mut PlayCharacter,
    state: &mut WorldState,
    mob_id: u16,
) -> Result<()> {
    let id = character.id;
    let (moves, complete) = state.quests.mob_killed(&ctx.data.quests, mob_id);
    for &(quest_id, index, count) in &moves {
        conn.send(qp::update(quest_id, index, count)).await?;
    }
    let mut saved: Vec<u16> = Vec::new();
    for &(quest_id, ..) in &moves {
        if !saved.contains(&quest_id) {
            saved.push(quest_id);
            if let Some(active) = state.quests.find(quest_id) {
                ctx.store.quest_counts(id, quest_id, active.counts).await?;
            }
        }
    }
    for quest_id in complete {
        let (Some(def), Some(active)) = (ctx.data.quests.get(quest_id), state.quests.find(quest_id).copied()) else { continue };
        match quests::outcome(def, active.counts, &state.inventory) {
            EndOutcome::Choose => {
                state.quests.set_choosing(quest_id);
                conn.send(qp::choose(quest_id)).await?;
            }
            EndOutcome::Finish(result) => finish_quest(conn, ctx, character, state, def, result, 0).await?,
            EndOutcome::Ignored => {}
        }
    }
    Ok(())
}

/// Everyone agreed to the guild this player asked to create (`docs/original/guild.md` section 4): it pays and the guild is written;
/// the zone tells everyone. The money is the connection's own, so this runs here.
async fn found_guild<S: Backend>(ctx: &Context<S>, id: u32, state: &mut WorldState) {
    if state.money < guild::CREATE_COST {
        return ctx.zones.guild_create_failed(id, gd::CREATE_NO_GOLD);
    }
    let Some(outcome) = ctx.zones.guild_create_write(id, ctx.config.server_id) else { return };
    match outcome.await {
        Ok(Ok(GuildCreated::Created(guild))) => {
            state.money -= guild::CREATE_COST;
            ctx.zones.guild_create_done(id, guild, state.money, Instant::now());
        }
        Ok(Ok(GuildCreated::NameTaken)) => ctx.zones.guild_create_failed(id, gd::CREATE_NAME),
        Ok(Ok(GuildCreated::NoMoney)) => ctx.zones.guild_create_failed(id, gd::CREATE_NO_GOLD),
        Ok(Err(error)) => {
            tracing::error!(%error, id, "could not create a guild");
            ctx.zones.guild_create_failed(id, gd::CREATE_FAILED);
        }
        Err(_) => ctx.zones.guild_create_failed(id, gd::CREATE_FAILED),
    }
}

/// `0x0205` (`CUser::ItemGet`, `docs/original/combat.md` section 8): the item off the ground into the money or the bags, saved;
/// what does not fit goes back on the ground. Returns the packets for the player in order.
async fn pick_up<S: Backend>(ctx: &Context<S>, character: &PlayCharacter, state: &mut WorldState, item_id: u32) -> Result<Vec<Vec<u8>>> {
    let (id, map, now) = (character.id, state.map, Instant::now());
    let (item, leave) = match ctx.zones.pick_up(map, id, item_id, now, |item_type, type_id| {
        let live = ctx.data.live();
        live.drops.quest_start(item_type, type_id)
            || live.drops.quest_ends(item_type, type_id).iter().any(|&q| state.quests.find(q).is_some())
    }) {
        Ok(taken) => taken,
        Err(gp::PICK_GONE) => return Ok(Vec::new()),
        Err(code) => return Ok(vec![gp::pick_up_failed(code)]),
    };
    let mut out = vec![leave];
    match item.loot {
        Loot::Gold(amount) => {
            // A monster's gold raised by the item flags (`CUser::ItemGet`, `docs/original/item-flags.md` section 4).
            let amount = if item.from_mob() { state.effects.with_gold_bonus(amount) } else { amount };
            let Some(money) = state.money.checked_add(amount) else {
                out.extend(ctx.zones.put_back(map, id, item, item.loot, now));
                out.push(gp::pick_up_failed(gp::PICK_TOO_MUCH_GOLD));
                return Ok(out);
            };
            let saved = ctx.store.trade(id, i64::from(amount), &[]).await;
            if !matches!(saved, Ok(Some(_))) {
                // Not lost: back on the ground before the connection ends.
                ctx.zones.put_back(map, id, item, item.loot, now);
                saved?;
                bail!("the database refused picked-up gold the server allowed");
            }
            state.money = money;
            record::gold_picked_up(ctx, character, state, amount);
            out.push(gp::picked_up_gold(0, 1, amount));
        }
        Loot::Item { item_type, type_id, count, quality, kept } => {
            let Some(def) = ctx.data.items.get(item_type, type_id) else {
                out.extend(ctx.zones.put_back(map, id, item, item.loot, now));
                out.push(gp::pick_up_failed(gp::PICK_NO_ROOM));
                return Ok(out);
            };
            let left = match into_bags(ctx, id, state, def, count, quality, kept, Some((character, item.dropped)), &mut out).await {
                Ok(left) => left,
                Err(e) => {
                    ctx.zones.put_back(map, id, item, item.loot, now);
                    return Err(e);
                }
            };
            if left < count {
                ctx.zones.party_loot_notice(map, id, item_type, type_id);
            }
            if left > 0 {
                let rest = Loot::Item { item_type, type_id, count: left, quality, kept };
                out.extend(ctx.zones.put_back(map, id, item, rest, now));
                out.push(gp::pick_up_failed(gp::PICK_NO_ROOM));
            }
        }
    }
    tracing::debug!(id, item_id, loot = ?item.loot, "picked up");
    Ok(out)
}

/// `0x0206` (`CUser::ItemDrop`, `docs/original/item-drop.md`): `count` of a bag slot onto the ground at the player's feet, saved first;
/// kind 1, the player's alone for 30 s. Returns the packets for the player in order: `0x0401`, then `0x0206` with what is left.
async fn drop_item<S: Backend>(
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    bag: u8,
    slot: u8,
    count: u8,
) -> Result<Vec<Vec<u8>>> {
    let id = character.id;
    if state.dead || bag == 0 || count == 0 || !state.inventory.is_bag((bag, slot)) {
        return Ok(Vec::new());
    }
    // The trade ends first (`CUser::CancleActionExc`).
    ctx.zones.cancel_actions(id);
    let Some(drop) = state.inventory.drop_from(&ctx.data.items, bag, slot, count) else { return Ok(Vec::new()) };
    let Some((kept, row)) = ctx.store.drop_item(id, bag, slot, drop.had, drop.count).await? else {
        bail!("the database refused a drop the server allowed: {drop:?}");
    };
    // `0x0402`: the item as it left the slot, named by the row the drop touched.
    if let Some(unit) = state.inventory.get(bag, slot) {
        let item = ctx.game_log.item(&ItemUnit { count: drop.count, ..unit.clone() }, Some(row));
        ctx.game_log.action(crate::game_log::item_drop(record::header(character, state), &item, bag, slot, drop.count));
    }
    state.inventory.apply(&drop.change);
    let loot = Loot::Item { item_type: drop.item_type, type_id: drop.type_id, count: drop.count, quality: drop.quality, kept: Some(kept) };
    let mut out: Vec<Vec<u8>> = ctx.zones.drop_by_player(state.map, id, loot, state.position, Instant::now()).into_iter().collect();
    out.push(gp::item_left(bag, slot, state.inventory.get(bag, slot)));
    tracing::debug!(id, bag, slot, ?loot, "dropped");
    Ok(out)
}

/// Puts an item into the bags as a pick-up does (`CUser::ItemGet`): stacks of it first, then the first empty slot; saved, with a
/// `0x0205` for each slot changed added to `out`. Returns how many did not fit.
#[allow(clippy::too_many_arguments)]
async fn into_bags<S: Backend>(
    ctx: &Context<S>,
    id: u32,
    state: &mut WorldState,
    def: &crate::inventory::ItemDef,
    count: u8,
    quality: u16,
    kept: Option<Kept>,
    log: Option<(&PlayCharacter, crate::zone::Dropped)>,
    out: &mut Vec<Vec<u8>>,
) -> Result<u8> {
    let pick = state.inventory.pick_up(def, count, quality, kept);
    if !pick.changes.is_empty() {
        let Some(rows) = ctx.store.trade(id, 0, &pick.changes).await? else {
            bail!("the database refused a pick-up the server allowed: {:?}", pick.changes);
        };
        picked_up(ctx, state, def, &pick.changes, &rows, log, out);
    }
    Ok(pick.left)
}

/// The changes of a saved pick-up applied to the bags, with the game log's records and a `0x0205` for each slot changed added to `out`.
fn picked_up<S>(
    ctx: &Context<S>,
    state: &mut WorldState,
    def: &crate::inventory::ItemDef,
    changes: &[SlotChange],
    rows: &[crate::backend::Changed],
    log: Option<(&PlayCharacter, crate::zone::Dropped)>,
    out: &mut Vec<Vec<u8>>,
) {
    for (change, row) in changes.iter().zip(rows) {
        // How many this change added to its slot.
        let added = match *change {
            crate::inventory::SlotChange::SetCount { bag, slot, count } => {
                count.saturating_sub(state.inventory.get(bag, slot).map_or(0, |i| i.count))
            }
            crate::inventory::SlotChange::Insert(ref new) => new.count,
            _ => 0,
        };
        state.inventory.apply(change);
        let (bag, slot) = match *change {
            crate::inventory::SlotChange::SetCount { bag, slot, .. } => (bag, slot),
            crate::inventory::SlotChange::Insert(ref new) => (new.bag, new.slot),
            _ => continue,
        };
        let Some(now_there) = state.inventory.get(bag, slot) else { continue };
        if let Some((character, dropped)) = log {
            let item = ctx.game_log.item(now_there, row.row);
            record::picked_up(ctx, character, state, dropped, &item, added);
        }
        // Stackable items: the short form without quality, gems or craft name.
        out.push(if def.max_count > 1 {
            gp::picked_up(&ItemUnit { quality: 0, gems: [0; 6], craft_name: Vec::new(), ..now_there.clone() })
        } else {
            gp::picked_up(now_there)
        });
    }
}

/// An item of a party kill in the player's turn (`CUser::ItemGet(CItem*)`, `docs/original/party.md` section 9): into the bags, the
/// members near told (`0x0B08`) when something went in; what does not fit goes on the ground where it dropped (kind 1).
#[allow(clippy::too_many_arguments)]
async fn party_loot<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    loot: Loot,
    position: [f32; 3],
    owner: u32,
    party: u32,
    dropped: crate::zone::Dropped,
) -> Result<()> {
    let id = character.id;
    let Loot::Item { item_type, type_id, count, quality, kept } = loot else { return Ok(()) };
    let Some(def) = ctx.data.items.get(item_type, type_id) else { return Ok(()) };
    let mut out = Vec::new();
    let left = match into_bags(ctx, id, state, def, count, quality, kept, Some((character, dropped)), &mut out).await {
        Ok(left) => left,
        Err(error) => {
            // The write failed: the whole drop goes on the ground where it dropped, then the connection ends as before (audit R14;
            // as for a pick-up, N1 in the audit).
            ctx.zones.drop_on_ground(state.map, loot, position, owner, party, Instant::now(), gp::KIND_PUT_BACK, dropped);
            return Err(error);
        }
    };
    // What did not fit goes on the ground before any packet is sent, so a write error cannot lose it.
    if left > 0 {
        let rest = Loot::Item { item_type, type_id, count: left, quality, kept };
        ctx.zones.drop_on_ground(state.map, rest, position, owner, party, Instant::now(), gp::KIND_PUT_BACK, dropped);
    }
    for packet in out {
        conn.send(packet).await?;
    }
    if left < count {
        ctx.zones.party_loot_notice(state.map, id, item_type, type_id);
    }
    tracing::debug!(id, ?loot, left, "party loot");
    Ok(())
}

/// A resurrection item: real type 26 with special 37 (`CUser::FindItemRebirth`, `docs/original/item-flags.md` section 7).
const RUNE_REAL_TYPE: u8 = 26;
const RUNE_SPECIAL: u8 = 37;
/// How long a dead player waits before coming back to life by itself (`CUser::Death`: 200 s).
const REBIRTH_WAIT: Duration = Duration::from_secs(200);
/// How long a dead party member waits to come back next to its leader (`0x0553` kind 3).
const PARTY_REBIRTH_WAIT: Duration = Duration::from_secs(30);
/// Experience lost on coming back to life, in percent (`m_nDropExp`).
const DEATH_EXP_PERCENT: u8 = 5;
/// Up to this level nothing is lost (`CUser::ExpDrop`).
const DEATH_EXP_FREE_LEVEL: u16 = 10;

/// The player's numbers against monsters: its melee numbers and its upper armour's element.
fn guard<S>(ctx: &Context<S>, character: &PlayCharacter, state: &WorldState) -> crate::zone::Guard {
    let gear = gear(ctx, state);
    crate::zone::Guard {
        fighter: combat::player(character.level, state.totals.stats, &gear),
        element: gear.armour_element,
        rec: state.totals.stats[2],
        absorption: gear.absorption,
        state_blocks: gear.state_blocks,
        grade_recovery: state.totals.grade_recovery,
        grade_fight_recovery: state.totals.grade_fight_recovery,
        options: OptionSkills::new(state.inventory.items(), &ctx.data.items, |id| ctx.data.skills.get(id, skills::MOB_LEVEL).is_some()),
        cross_road: gear.cross_road,
    }
}

/// Clears the attack target, the skill and the cast, and the zone's attacking state with it (`CUser::StatusAttackStop`).
fn stop_attacking<S>(ctx: &Context<S>, state: &mut WorldState, id: u32) {
    state.prev_target = None;
    state.attack_mode = false;
    state.next_skill = None;
    state.casting = false;
    if state.target.take().is_some() {
        ctx.zones.set_attacking(state.map, id, false);
    }
}

/// Experience lost when coming back to life (`CUser::ExpDrop`): a share of the level's span, never below the level's start.
/// `kept`: the faction grade's type 25, the percentage of the loss spared (`CUser::ExpDrop`, `docs/original/pvp-2.md` section 8).
/// `percent`: the death's (`m_nDropExp`, `docs/original/item-flags.md` section 3); 0 loses nothing.
fn death_exp_loss(exp: u32, level: u16, grow: u8, table: &ExpTable, kept: i32, percent: u8) -> u32 {
    if level <= DEATH_EXP_FREE_LEVEL || percent == 0 {
        return 0;
    }
    let (start, next) = (table.total(grow, level - 1), table.total(grow, level));
    let span = i64::from(next) - i64::from(start);
    let mut loss = (f64::from(f32::from(percent)) / 100.0 * span as f64) as i64;
    if kept != 0 {
        loss -= (f64::from(kept as f32) / 100.0 * f64::from(loss as i32 as f32)) as i64;
    }
    let have = i64::from(exp) - i64::from(start);
    if have <= 0 { 0 } else { loss.clamp(0, have) as u32 }
}

/// The experience percent this rebirth loses (`CUser::ExpDrop`): none after a safe death (`m_nDropExp` −1) or with the no-loss flag
/// still on, else the death's (`docs/original/item-flags.md` section 3).
fn rebirth_loss<S>(ctx: &Context<S>, character: &PlayCharacter, state: &mut WorldState) -> u32 {
    let kept = std::mem::take(&mut state.keep_exp);
    let percent = if kept || state.effects.no_exp_loss { 0 } else { state.drop_exp };
    state.drop_exp = DEATH_EXP_PERCENT;
    death_exp_loss(character.exp, character.level, character.grow, &ctx.data.exp, state.grade[EXP_LOSS_KEPT], percent)
}

/// The bind position of the player's map for its country (`CMap::GetBindPos`).
fn bind_position<S>(ctx: &Context<S>, player: &Player, state: &WorldState) -> (u16, [f32; 3]) {
    use rand::Rng;
    let mut rng = rand::rng();
    let country = player.faction.map_or(0, |f| f.country);
    ctx.data.world.bind_pos(state.map, country, state.position, &mut || rng.random::<u32>() & 0x7fff)
}

/// A resurrection item in the bags (`CUser::FindItemRebirth`, `docs/original/item-flags.md` section 7): bags 1 to the last, slots in
/// order, the first of real type 26 and special 37 whose VG is not 0 (`vg`: the bind position's, kind 4) or is 0 (kind 5's), else
/// the first of either.
fn find_rune<S>(ctx: &Context<S>, state: &WorldState, vg: bool) -> Option<(u8, u8)> {
    let runes: Vec<(u8, u8, bool)> = (1..=state.inventory.max_bag())
        .flat_map(|bag| (0..24).map(move |slot| (bag, slot)))
        .filter_map(|(bag, slot)| {
            let item = state.inventory.get(bag, slot)?;
            let def = ctx.data.items.get(item.item_type, item.type_id)?;
            (real_type(item.item_type) == RUNE_REAL_TYPE && def.special == RUNE_SPECIAL).then_some((bag, slot, def.vg != 0))
        })
        .collect();
    runes.iter().find(|r| r.2 == vg).or(runes.first()).map(|&(bag, slot, _)| (bag, slot))
}

/// Where a dead player comes back (`CUser::Rebirth`'s position and kind).
#[derive(Clone, Copy, PartialEq)]
enum Comeback {
    /// The map's bind position (`0x0551` kind 4).
    Bind,
    /// Next to its party's leader (kind 5).
    Party([f32; 3]),
    /// Where it lies (kind 1, a resurrection item's).
    Here,
}

/// Back to life (`CUser::Rebirth`): the experience lost (already off), the place, HP, MP and SP full or at half; another map is
/// reached as with a gatekeeper.
#[allow(clippy::too_many_arguments)]
async fn come_back<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    player: &Player,
    character: &mut PlayCharacter,
    state: &mut WorldState,
    place: Comeback,
    half: bool,
    way: Revive,
) -> Result<(u16, [f32; 3])> {
    let (id, map) = (character.id, state.map);
    let (to_map, position, kind) = match place {
        Comeback::Bind => {
            let (to_map, position) = bind_position(ctx, player, state);
            (to_map, position, cp::REBIRTH_BIND)
        }
        Comeback::Party(at) => (map, at, cp::REBIRTH_PARTY),
        Comeback::Here => (map, state.position, cp::REBIRTH_HERE),
    };
    state.dead = false;
    state.rebirth_due = None;
    state.motion = w::MOTION_STAND;
    let max = state.totals.max_health();
    state.health = if half { max.map(|m| m / 2) } else { max };
    if to_map == map {
        state.position = position;
        state.move_check.reset(Instant::now(), position);
        for packet in ctx.zones.revive_here(map, id, position, character.exp, kind, half) {
            conn.send(packet).await?;
        }
    } else {
        for packet in ctx.zones.revive_away(map, id, position, character.exp, half) {
            conn.send(packet).await?;
        }
        move_player(conn, ctx, state, id, to_map, position, None).await?;
    }
    ctx.store.save_progress(id, &progress(character), state.health).await?;
    // `0x0304` and `0x0305` (`CUser::Rebirth` and the way back): the revival, then the way the player came back.
    ctx.game_log.action(crate::game_log::rebirth(record::header(character, state)));
    ctx.game_log.action(crate::game_log::revived(record::header(character, state), way));
    Ok((to_map, position))
}

/// Back to life at the bind position (`CUser::KillSoul`, `CUser::Rebirth`; `docs/original/mob-fight.md` section 8): the
/// experience loss, the bind position of the map, HP, MP and SP full; another map is reached as with a gatekeeper. In ultimate mode
/// (`docs/original/item-flags.md` section 7), unless the death was a safe one: a Continuous Res Rune, else a resurrection item (used
/// up), brings the player back at half with nothing lost, and without either it falls (deviation 2).
async fn rebirth<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    player: &Player,
    character: &mut PlayCharacter,
    state: &mut WorldState,
    way: Revive,
) -> Result<()> {
    if !state.dead || state.fallen {
        return Ok(());
    }
    let id = character.id;
    if character.grow == ULTIMATE && !state.keep_exp {
        state.rebirth_party = false;
        if state.effects.rebirth {
            return revived_by_item(conn, ctx, player, character, state, None, Comeback::Bind, way).await;
        }
        if let Some(rune) = find_rune(ctx, state, true) {
            return revived_by_item(conn, ctx, player, character, state, Some(rune), Comeback::Bind, way).await;
        }
        return fall(conn, ctx, character, state).await;
    }
    let loss = rebirth_loss(ctx, character, state);
    character.exp -= loss;
    // With the party: next to its leader when it is alive on this map (`CUser::KillSoulParty`, never in ultimate mode), else as
    // without.
    let party = std::mem::take(&mut state.rebirth_party) && character.grow != ULTIMATE;
    let at_leader = if party { ctx.zones.party_leader_spot(state.map, id) } else { None };
    let place = at_leader.map_or(Comeback::Bind, Comeback::Party);
    // The party's way is `sel_party` only when the player went to its leader.
    let way = if matches!(place, Comeback::Party(_)) {
        Revive::SelParty
    } else if way == Revive::SelParty {
        Revive::SelBase
    } else {
        way
    };
    let (to_map, position) = come_back(conn, ctx, player, character, state, place, false, way).await?;
    tracing::info!(id, map = to_map, ?position, loss, "back to life");
    Ok(())
}

/// `0x0553` kinds 4 and 5 (`CUser::KillSoulByItem`, `KillSoulByItemNoMove`; `docs/original/item-flags.md` section 7): after a safe
/// death, or without a resurrection item, as kind 2; else one used up and back at half with nothing lost, at the bind position
/// (kind 4) or where the player lies (`here`, kind 5).
async fn rune_rebirth<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    player: &Player,
    character: &mut PlayCharacter,
    state: &mut WorldState,
    here: bool,
) -> Result<()> {
    if !state.dead || state.fallen {
        return Ok(());
    }
    let rune = if state.keep_exp { None } else { find_rune(ctx, state, !here) };
    let Some(rune) = rune else { return rebirth(conn, ctx, player, character, state, Revive::SelBase).await };
    let place = if here { Comeback::Here } else { Comeback::Bind };
    let way = if here { Revive::SelItemBig } else { Revive::SelItemSmall };
    revived_by_item(conn, ctx, player, character, state, Some(rune), place, way).await
}

/// Back at half with nothing lost, a resurrection item used up first (`0x050A`) when there is one (`docs/original/item-flags.md`
/// section 7).
#[allow(clippy::too_many_arguments)]
async fn revived_by_item<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    player: &Player,
    character: &mut PlayCharacter,
    state: &mut WorldState,
    rune: Option<(u8, u8)>,
    place: Comeback,
    way: Revive,
) -> Result<()> {
    let id = character.id;
    if let Some((bag, slot)) = rune {
        ctx.zones.cancel_actions(id);
        items::use_up(conn, ctx, character, state, bag, slot).await?;
    }
    state.keep_exp = false;
    state.drop_exp = DEATH_EXP_PERCENT;
    let (to_map, position) = come_back(conn, ctx, player, character, state, place, true, way).await?;
    tracing::info!(id, map = to_map, ?position, ?rune, continuous = rune.is_none(), "back to life by a resurrection item");
    Ok(())
}

/// An ultimate-mode death without a rune (`CUser::KillSoul`, `docs/original/item-flags.md` section 7 and deviation 2): the experience
/// drop, the character marked fallen (out of the list, kept for a restore) and out of its guild as a deletion, then back to the
/// character screen at once (`CWorld::LogoutUser`: `0x0107`).
async fn fall<S: Backend>(conn: &mut Connection, ctx: &Context<S>, character: &mut PlayCharacter, state: &mut WorldState) -> Result<()> {
    let _ = conn;
    let id = character.id;
    let loss = rebirth_loss(ctx, character, state);
    character.exp -= loss;
    ctx.store.save_progress(id, &progress(character), state.health).await?;
    if ctx.store.fall(id).await? {
        ctx.zones.guild_deleted(id);
    }
    state.fallen = true;
    state.rebirth_due = None;
    state.logout = Some((w::OP_LOGOUT_TO_SELECT, Instant::now()));
    tracing::info!(id, loss, "fallen: an ultimate-mode death without a rune");
    Ok(())
}

/// Leaving the world dead: back to life at the bind position with the experience drop and full HP, SP and MP, only in what is
/// saved; nothing is sent (`CUser::KillSoulForDisconnect`, `docs/original/world.md` section 4.3). In ultimate mode as `rebirth`: a
/// Continuous Res Rune or a resurrection item (used up) at half with nothing lost, else the fall. Called once the player is out of
/// its zone.
async fn leave_dead<S: Backend>(ctx: &Context<S>, player: &Player, character: &mut PlayCharacter, state: &mut WorldState) -> Result<()> {
    let id = character.id;
    let mut half = false;
    if character.grow == ULTIMATE && !state.keep_exp {
        half = true;
        if !state.effects.rebirth {
            match find_rune(ctx, state, true) {
                Some((bag, slot)) => {
                    items::use_up_unsent(ctx, character, state, bag, slot).await?;
                }
                None => {
                    let loss = rebirth_loss(ctx, character, state);
                    character.exp -= loss;
                    ctx.store.save_progress(id, &progress(character), state.health).await?;
                    if ctx.store.fall(id).await? {
                        ctx.zones.guild_deleted(id);
                    }
                    tracing::info!(id, loss, "left dead: fallen, an ultimate-mode death without a rune");
                    return Ok(());
                }
            }
        }
    }
    let loss = if half {
        state.keep_exp = false;
        0
    } else {
        rebirth_loss(ctx, character, state)
    };
    character.exp -= loss;
    let (map, position) = bind_position(ctx, player, state);
    state.dead = false;
    state.rebirth_due = None;
    let max = state.totals.max_health();
    state.health = if half { max.map(|m| m / 2) } else { max };
    state.map = map;
    state.position = position;
    ctx.store.save_progress(id, &progress(character), state.health).await?;
    // `0x0304` and `0x0305` with the way `logout` (`CUser::KillSoulForDisconnect`).
    ctx.game_log.action(crate::game_log::rebirth(record::header(character, state)));
    ctx.game_log.action(crate::game_log::revived(record::header(character, state), Revive::Logout));
    tracing::info!(id = character.id, map, ?position, loss, half, "left dead: back to life at the bind position");
    Ok(())
}

/// What `save_progress` keeps of the character.
fn progress(character: &PlayCharacter) -> Progress {
    Progress {
        level: character.level,
        exp: character.exp,
        stat_point: character.stat_point,
        skill_point: character.skill_point,
        stats: character.stats,
    }
}

/// Range of the level-up packets (`CZone::PSendView` with 60).
const LEVEL_UP_RANGE: f32 = 60.0;

/// `CUser::StatusStand`: a sitting player stands, its pending logout and personal shop end.
fn stand_up<S>(ctx: &Context<S>, state: &mut WorldState, id: u32) {
    if state.motion == w::MOTION_SIT {
        state.motion = w::MOTION_STAND;
        state.logout = None;
        ctx.zones.shop_end(id);
    }
}

/// The logout's end (`CWorld::LogoutUser`): `0x0107` back to the character screen, or `0x010D` and the client quits.
async fn finish_logout(conn: &mut Connection, peer: SocketAddr, player: &Player, character: &PlayCharacter, kind: u16) -> Result<Left> {
    // Still XORed; both sides return to AES right after (the client in its 0x0107 handler).
    conn.send(w::logout_done(kind)).await?;
    if kind == w::OP_LOGOUT_TO_SELECT {
        conn.xor = None;
        tracing::info!(%peer, user_id = player.user_id, id = character.id, "back to the character screen");
        return Ok(Left::ToSelect);
    }
    tracing::info!(%peer, user_id = player.user_id, id = character.id, "quit");
    let _ = timeout(QUIT_GRACE, async { while let Ok(Next::Packet(_)) = conn.next_packet(QUIT_GRACE).await {} }).await;
    Ok(Left::Gone)
}

/// Waits until the character's stay after a lost connection ends, or the server shuts down; forever when it is not staying.
async fn linger_end(until: Option<Instant>, shutdown: &mut watch::Receiver<bool>) {
    let Some(until) = until else { return std::future::pending().await };
    tokio::select! {
        () = tokio::time::sleep_until(until.into()) => {}
        _ = shutdown.wait_for(|stop| *stop) => {}
    }
}

/// Waits until the instant, or forever without one.
async fn sleep_until_some(due: Option<Instant>) {
    match due {
        Some(due) => tokio::time::sleep_until(due.into()).await,
        None => std::future::pending().await,
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut text: String = bytes.iter().take(HEX_MAX).map(|b| format!("{b:02x}")).collect();
    if bytes.len() > HEX_MAX {
        text.push('…');
    }
    text
}

/// A hex dump in the log is cut to this many bytes (`docs/server/audit.md` R18).
const HEX_MAX: usize = 64;

/// The local time as the original packs it for `0x0114`.
fn local_packed_time() -> u32 {
    use chrono::{Datelike, Timelike};
    let now = chrono::Local::now();
    w::pack_time(now.year() as u16, now.month() as u8, now.day() as u8, now.hour() as u8, now.minute() as u8, now.second() as u8)
}

#[cfg(test)]
mod tests;
