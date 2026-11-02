//! Guilds (`docs/original/guild.md`): the guilds, their members and join requests, invitations, the creation and disband agreements,
//! and every player in the world as the guilds need it (its outbox, country, level, join penalty). The guild's own packets go from
//! here to the online players' outboxes; the zone checks what needs the maps and the parties (`crate::zone::guild`) and applies the
//! looks. Every change to the database goes, in order, to the writer task (`run_writer`).

use std::collections::HashMap;
use std::time::{Duration, Instant};

use shaiya_proto::chat;
use shaiya_proto::guild::{self as gp, GuildUnit, MemberUnit, RequestUnit};
use shaiya_proto::world as w;
use tokio::sync::{mpsc, oneshot};

use crate::backend::{GuildCreated, GuildStore, GuildWrite, NewGuild, RankingRow, StoredGuild};
use crate::game_log::{self, GameLog, Header, Leave, Row};
use crate::guild_bank::{self, GuildBank, Kind as BankKind, Planned, Refusal};
use crate::guild_house::{self as gh, House, HouseRules};
use crate::inventory::{Inventory, ItemDefs};
use crate::ranking::{self, Tag};
use crate::zone::{Outbox, ZoneEvent};

/// What creating a guild costs the requester (`CGuildMgr::CreateGuild`).
pub const CREATE_COST: u32 = 1000;
/// The requester's lowest level (`CUser::PacketGuild` case `0x0D21`).
pub const CREATE_MIN_LEVEL: u16 = 11;
/// How long an invitation (`m_dwTimeOverGuild`) and, here, an agreement wait for their answers.
pub const ANSWER_TIME: Duration = Duration::from_secs(30);
/// How long an agreed creation waits for the founder's connection to take it up before it lapses (audit R27).
pub const AGREED_TIME: Duration = Duration::from_secs(30);
/// How long an agreed creation waits for the database once it is written; the outcome ends it long before.
const CREATING_TIME: Duration = Duration::from_secs(300);
/// After joining a guild, no other may be created or asked to join for this long (`usp_Read_Char_GuildJoinDate_R`).
pub const JOIN_PENALTY: Duration = Duration::from_secs(72 * 3600);
pub const MASTER: u8 = 1;
pub const OFFICER: u8 = 2;
pub const NEW_MEMBER: u8 = 9;
/// The rank a join request is stored with.
pub const REQUEST: u8 = 0;
/// Members below rank 3 (the master included) at most (the slots at `CGuild` +0x4D8).
pub const MAX_OFFICERS: usize = 7;
/// A guild this small is disbanded without asking (`CUser::PacketGuild` case `0x0D03`).
pub const DISBAND_AT_ONCE: usize = 2;
/// The officers besides the master that a disband needs in its party.
pub const DISBAND_OFFICERS: usize = 2;
pub const UNRANKED: u8 = 31;
/// The game mode that may neither create nor ask to join (`m_byGrow` 0, easy).
pub const EASY: u8 = 0;

/// Rank 1 or 2: invites, kicks, sees and answers the join requests.
pub fn is_officer(rank: u8) -> bool {
    (MASTER..=OFFICER).contains(&rank)
}

/// 2 to 24 printable ASCII bytes without `'` or `<`, not starting or ending with a space (deviation 7).
pub fn valid_name(name: &[u8]) -> bool {
    (2..=24).contains(&name.len())
        && name.iter().all(|b| (0x20..0x7F).contains(b) && !b"'<".contains(b))
        && name.first() != Some(&b' ')
        && name.last() != Some(&b' ')
}

/// Printable ASCII without `'`, its colour markers safe (deviation 7).
pub fn valid_remark(remark: &[u8]) -> bool {
    remark.len() < gp::REMARK_LEN && remark.iter().all(|b| (0x20..0x7F).contains(b) && *b != b'\'') && chat::markup_is_safe(remark)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Member {
    pub id: u32,
    pub name: Vec<u8>,
    pub level: u16,
    pub job: u8,
    pub rank: u8,
}

impl Member {
    fn unit(&self) -> MemberUnit {
        MemberUnit { id: self.id, rank: self.rank, level: self.level, job: self.job, name: self.name.clone() }
    }

    fn request_unit(&self) -> RequestUnit {
        RequestUnit { id: self.id, level: self.level, job: self.job, name: self.name.clone() }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Guild {
    pub id: u32,
    pub name: Vec<u8>,
    pub country: u8,
    pub remark: Vec<u8>,
    /// Longest in the guild first.
    pub members: Vec<Member>,
    pub requests: Vec<Member>,
    /// Its house, ranking and Etin (`docs/original/guild-house.md`).
    pub house: House,
    /// Its bank (`docs/original/guild-bank.md`).
    pub bank: GuildBank,
}

impl Guild {
    pub fn member(&self, id: u32) -> Option<&Member> {
        self.members.iter().find(|m| m.id == id)
    }

    fn member_mut(&mut self, id: u32) -> Option<&mut Member> {
        self.members.iter_mut().find(|m| m.id == id)
    }

    /// The members below rank 3.
    pub fn officers(&self) -> usize {
        self.members.iter().filter(|m| is_officer(m.rank)).count()
    }

    fn master_name(&self) -> &[u8] {
        self.members.iter().find(|m| m.rank == MASTER).map_or(&[][..], |m| &m.name)
    }

    fn unit(&self) -> GuildUnit {
        GuildUnit {
            id: self.id,
            name: self.name.clone(),
            master: self.master_name().to_vec(),
            remark: self.remark.clone(),
            ranking: self.house.ranking,
            points: self.house.points,
        }
    }
}

/// A player in the world.
#[derive(Clone, Debug)]
pub struct Player {
    pub outbox: Outbox,
    pub events: mpsc::UnboundedSender<ZoneEvent>,
    pub name: Vec<u8>,
    pub country: u8,
    pub job: u8,
    pub level: u16,
    pub grow: u8,
    /// When its join penalty ends.
    pub penalty_until: Option<Instant>,
}

impl Player {
    fn send(&self, packet: &[u8]) {
        // A closed outbox means that connection is ending; it leaves by itself.
        let _ = self.outbox.send(packet.to_vec());
    }

    fn penalised(&self, now: Instant) -> bool {
        self.penalty_until.is_some_and(|until| now < until)
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Kind {
    Create { name: Vec<u8>, remark: Vec<u8> },
    Disband { guild: u32 },
}

/// A creation or disband agreement (`CGuildCreate`).
#[derive(Clone, Debug)]
struct Agreement {
    kind: Kind,
    requester: u32,
    /// Everyone asked, the requester first.
    members: Vec<u32>,
    /// Those who have not said yes yet.
    waiting: Vec<u32>,
    due: Instant,
    /// Everyone said yes; the creation is being written.
    agreed: bool,
}

struct Invite {
    inviter: u32,
    due: Instant,
}

/// What the zone does after a guild change: the looks of these players (character, guild id, guild name) change.
#[derive(Debug, Default, PartialEq)]
pub struct Aftermath {
    pub looks: Vec<(u32, u32, Vec<u8>)>,
}

impl Aftermath {
    fn look(&mut self, id: u32, guild: Option<&Guild>) {
        self.looks.push((id, guild.map_or(0, |g| g.id), guild.map_or_else(Vec::new, |g| g.name.clone())));
    }
}

/// The guild packets of the enter-world burst (`docs/original/guild.md` section 10) and the guild name of `0x0105`.
#[derive(Debug, Default, PartialEq)]
pub struct Burst {
    pub packets: Vec<Vec<u8>>,
    pub name: Vec<u8>,
}

/// `0x0D29`: the bank's items, by slot; none for an empty bank.
fn bank_packets(guild: &Guild) -> Vec<Vec<u8>> {
    gp::bank_list(&guild.bank.items().cloned().collect::<Vec<_>>())
}

/// A queued database write; a creation waits for its outcome.
pub enum Queued {
    Create(NewGuild, oneshot::Sender<anyhow::Result<GuildCreated>>),
    Write(GuildWrite),
    /// Answered once everything queued before it is written (the shutdown waits for it).
    Flush(oneshot::Sender<()>),
}

/// Writes the guild changes in the order they were made, as long as the server runs. A failed write is tried again after 0.2 s, 1 s
/// and 5 s, then logged with the write at error level (`docs/server/audit.md` R16).
pub async fn run_writer<S: GuildStore>(store: &S, queue: mpsc::UnboundedReceiver<Queued>) {
    run_writer_with(store, queue, &crate::supervise::WRITE_RETRIES).await;
}

async fn run_writer_with<S: GuildStore>(store: &S, mut queue: mpsc::UnboundedReceiver<Queued>, retries: &[std::time::Duration]) {
    while let Some(item) = queue.recv().await {
        match item {
            // A panic in one item costs that item, not the queue (`docs/server/audit.md` R5).
            Queued::Create(guild, reply) => match crate::supervise::catch(store.create_guild(&guild)).await {
                Ok(result) => {
                    let _ = reply.send(result);
                }
                Err(panic) => tracing::error!(%panic, "creating a guild panicked"),
            },
            Queued::Write(write) => match crate::supervise::retried(retries, || store.write_guild(&write)).await {
                Ok(Ok(())) => {}
                Ok(Err(error)) => tracing::error!(%error, ?write, "could not write a guild change; given up"),
                Err(panic) => tracing::error!(%panic, ?write, "writing a guild change panicked"),
            },
            Queued::Flush(done) => {
                let _ = done.send(());
            }
        }
    }
}

#[derive(Default)]
pub struct Guilds {
    guilds: HashMap<u32, Guild>,
    /// Each member's and requester's guild.
    of: HashMap<u32, u32>,
    online: HashMap<u32, Player>,
    invites: HashMap<u32, Invite>,
    agreements: Vec<Agreement>,
    writer: Option<mpsc::UnboundedSender<Queued>>,
    /// The ranking battle's best guild so far for each country: `(points, guild)` (`docs/original/guild-ranking.md` section 7).
    battle_best: [(u32, u32); 2],
    /// The battle each character entered last (deviation 3: per character, in memory only).
    battle_entered: HashMap<u32, Tag>,
    /// The game log (`docs/original/game-log.md` section 3.7).
    log: GameLog,
}

/// The country's place in the ranking tables: Light 0, anything else Fury.
fn side(country: u8) -> usize {
    usize::from(country != 0)
}

/// What a guild's kill points came to (`CGuild::SendRankingZone`): its points, its country's best and that best guild.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BattleScore {
    pub points: u32,
    pub best_points: u32,
    pub best_guild: u32,
}

impl BattleScore {
    pub fn packet(&self) -> Vec<u8> {
        gp::battle_points(self.points, self.best_points, self.best_guild)
    }
}

/// A guild placed by the ranking update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placed {
    pub guild: u32,
    pub score: u32,
    pub place: u8,
    pub country: u8,
}

fn bytes(text: &str) -> Vec<u8> {
    text.as_bytes().to_vec()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

impl Guilds {
    /// The stored guilds (section 15); those without a member are disbanded now (`RemoveGuildZeroUser`).
    pub fn load(stored: Vec<StoredGuild>, writer: mpsc::UnboundedSender<Queued>) -> Self {
        let mut guilds = Self { writer: Some(writer), ..Self::default() };
        for g in stored {
            let (members, requests): (Vec<_>, Vec<_>) = g
                .members
                .into_iter()
                .map(|m| Member { id: m.id, name: bytes(&m.name), level: m.level, job: m.job, rank: m.rank })
                .partition(|m| m.rank != REQUEST);
            if members.is_empty() {
                tracing::info!(guild = g.id, name = %g.name, "disbanding a guild with no member");
                guilds.write(GuildWrite::Disband { guild: g.id });
                continue;
            }
            for m in members.iter().chain(&requests) {
                guilds.of.insert(m.id, g.id);
            }
            guilds.guilds.insert(
                g.id,
                Guild {
                    id: g.id,
                    name: bytes(&g.name),
                    country: g.country,
                    remark: bytes(&g.remark),
                    members,
                    requests,
                    house: g.house,
                    bank: GuildBank::new(&g.bank),
                },
            );
        }
        guilds
    }

    /// The game log the guild records go to.
    pub fn set_log(&mut self, log: GameLog) {
        self.log = log;
    }

    /// A guild record for the member `who` (section 3.7): the connection of a member in the world completes it with its header
    /// (`ZoneEvent::Record`); one who is not gets the character's id, name and level only.
    fn record(&self, who: u32, build: impl FnOnce(Header) -> Row) {
        if let Some(p) = self.online.get(&who) {
            let _ = p.events.send(ZoneEvent::Record(Box::new(build(Header::none()))));
            return;
        }
        let member =
            self.of.get(&who).and_then(|g| self.guilds.get(g)).and_then(|g| g.members.iter().chain(&g.requests).find(|m| m.id == who));
        let (name, level) = member.map_or((String::new(), 0), |m| (text(&m.name).trim_end_matches('\0').to_owned(), m.level));
        self.log.action(build(Header { level, ..Header::character_only(who, &name) }));
    }

    /// A guild's name as text.
    fn name_of(&self, guild: u32) -> String {
        self.guilds.get(&guild).map(|g| text(&g.name).trim_end_matches('\0').to_owned()).unwrap_or_default()
    }

    pub fn len(&self) -> usize {
        self.guilds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.guilds.is_empty()
    }

    fn write(&self, write: GuildWrite) {
        if let Some(writer) = &self.writer {
            let _ = writer.send(Queued::Write(write));
        }
    }

    /// Asks the writer to answer once what is queued now is written; `None` without a writer.
    pub fn flush(&self) -> Option<oneshot::Receiver<()>> {
        let (done, written) = oneshot::channel();
        self.writer.as_ref()?.send(Queued::Flush(done)).ok()?;
        Some(written)
    }

    /// A character's new name in its guild's member list or join requests (`docs/original/small-items.md` deviation 5).
    pub fn renamed(&mut self, id: u32, name: &[u8]) {
        let Some(guild) = self.of.get(&id).and_then(|g| self.guilds.get_mut(g)) else { return };
        for m in guild.members.iter_mut().chain(guild.requests.iter_mut()).filter(|m| m.id == id) {
            m.name = name.to_vec();
        }
    }

    /// A guild by its id.
    pub fn by_id(&self, id: u32) -> Option<&Guild> {
        self.guilds.get(&id)
    }

    /// The guild the player is a member of (not one it only asked to join).
    pub fn guild_of(&self, id: u32) -> Option<&Guild> {
        let guild = self.guilds.get(self.of.get(&id)?)?;
        guild.member(id).is_some().then_some(guild)
    }

    pub fn rank_of(&self, id: u32) -> Option<u8> {
        self.guild_of(id).and_then(|g| g.member(id)).map(|m| m.rank)
    }

    /// The guild it asked to join.
    fn requested(&self, id: u32) -> Option<u32> {
        let guild = self.of.get(&id)?;
        self.guilds[guild].requests.iter().any(|r| r.id == id).then_some(*guild)
    }

    fn in_agreement(&self, id: u32) -> bool {
        self.agreements.iter().any(|a| a.members.contains(&id))
    }

    /// The guild id and name in the player's looks.
    pub fn looks(&self, id: u32) -> (u32, Vec<u8>) {
        self.guild_of(id).map_or((0, Vec::new()), |g| (g.id, g.name.clone()))
    }

    fn send_to(&self, id: u32, packet: &[u8]) {
        if let Some(p) = self.online.get(&id) {
            p.send(packet);
        }
    }

    /// `CGuild::Send`: to every online member.
    fn send_guild(&self, guild: u32, packet: &[u8]) {
        if let Some(g) = self.guilds.get(&guild) {
            for m in &g.members {
                self.send_to(m.id, packet);
            }
        }
    }

    /// `CGuild::SendExcludeMe`: to every online member but `me`.
    fn send_guild_except(&self, guild: u32, me: u32, packet: &[u8]) {
        if let Some(g) = self.guilds.get(&guild) {
            for m in g.members.iter().filter(|m| m.id != me) {
                self.send_to(m.id, packet);
            }
        }
    }

    /// `CGuild::SendAdmin`: to the online officers.
    fn send_officers(&self, guild: u32, packet: &[u8]) {
        if let Some(g) = self.guilds.get(&guild) {
            for m in g.members.iter().filter(|m| is_officer(m.rank)) {
                self.send_to(m.id, packet);
            }
        }
    }

    /// `CWorld::SendAllCountry`.
    fn send_country(&self, country: u8, packet: &[u8]) {
        for p in self.online.values().filter(|p| p.country == country) {
            p.send(packet);
        }
    }

    fn send_agreement(&self, a: &Agreement, packet: &[u8]) {
        for &m in &a.members {
            self.send_to(m, packet);
        }
    }

    /// The member list (section 9), `me` counted online.
    fn member_list(&self, guild: &Guild, me: u32) -> Vec<Vec<u8>> {
        let (online, offline): (Vec<_>, Vec<_>) = guild.members.iter().partition(|m| m.id == me || self.online.contains_key(&m.id));
        gp::member_list(&online.iter().map(|m| m.unit()).collect::<Vec<_>>(), &offline.iter().map(|m| m.unit()).collect::<Vec<_>>())
    }

    /// Section 10: the guild list of its country, then (a member) the member list and (an officer) the join requests.
    pub fn burst(&self, me: u32, country: u8) -> Burst {
        let mut list: Vec<&Guild> = self.guilds.values().filter(|g| g.country == country).collect();
        list.sort_by_key(|g| g.id);
        let mut packets = gp::guild_list(&list.iter().map(|g| g.unit()).collect::<Vec<_>>());
        let Some(guild) = self.guild_of(me) else { return Burst { packets, name: Vec::new() } };
        packets.extend(self.member_list(guild, me));
        if self.rank_of(me).is_some_and(is_officer) {
            packets.extend(gp::request_list(&guild.requests.iter().map(Member::request_unit).collect::<Vec<_>>()));
        }
        packets.extend(bank_packets(guild));
        packets.extend(gp::npc_levels(&guild.house.used_slots()));
        Burst { packets, name: guild.name.clone() }
    }

    /// Entering the world (`CGuild::ConnectUser`): a member's online state to the others.
    pub fn connect(&mut self, me: u32, player: Player) {
        self.online.insert(me, player);
        if let Some(guild) = self.guild_of(me).map(|g| g.id) {
            let packet = gp::member_state(gp::STATE_ONLINE, me);
            for m in self.guilds[&guild].members.iter().filter(|m| m.id != me) {
                self.send_to(m.id, &packet);
            }
        }
    }

    /// Leaving the world (`CGuild::DisconnectUser`): offline to the others; its invitation forgotten; an agreement it is in ends
    /// (deviation 2).
    pub fn disconnect(&mut self, me: u32) {
        if self.online.remove(&me).is_none() {
            return;
        }
        if let Some(guild) = self.guild_of(me).map(|g| g.id) {
            self.send_guild(guild, &gp::member_state(gp::STATE_OFFLINE, me));
        }
        self.invites.remove(&me);
        // An agreement that was agreed to ends when its founder leaves, too (audit R27).
        let (ended, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.agreements)
            .into_iter()
            .partition(|a| if a.agreed { a.requester == me } else { a.members.contains(&me) });
        self.agreements = kept;
        for a in ended {
            let packet = match &a.kind {
                Kind::Create { name, .. } => gp::create_failed(gp::CREATE_MEMBERS, name),
                Kind::Disband { .. } => gp::disband_result(gp::DISBAND_MEMBERS),
            };
            self.send_agreement(&a, &packet);
        }
    }

    /// `CGuild::ChangeUserLevel`: the new level to the online members.
    pub fn level_up(&mut self, me: u32, level: u16) {
        if let Some(p) = self.online.get_mut(&me) {
            p.level = level;
        }
        let Some(guild) = self.guild_of(me).map(|g| g.id) else { return };
        if let Some(m) = self.guilds.get_mut(&guild).and_then(|g| g.member_mut(me)) {
            m.level = level;
        }
        // States below 100 are levels; the client takes the low byte.
        self.send_guild(guild, &gp::member_state(level.min(99) as u8, me));
    }

    /// Guild chat (`CGuild::SendAllGuildChat`): to every online member, the speaker included. False without a guild.
    pub fn chat(&self, me: u32, packet: &[u8]) -> bool {
        let Some(guild) = self.guild_of(me).map(|g| g.id) else { return false };
        self.send_guild(guild, packet);
        true
    }

    /// `0x0D23`: the guild's Etin.
    pub fn etin(&self, me: u32) {
        if let Some(g) = self.guild_of(me) {
            self.send_to(me, &gp::etin(g.house.etin));
        }
    }

    // Guild houses (`docs/original/guild-house.md`).

    /// The player's guild: its id, country and house.
    pub fn house_of(&self, me: u32) -> Option<(u32, u8, House)> {
        self.guild_of(me).map(|g| (g.id, g.country, g.house.clone()))
    }

    /// A guild's house.
    pub fn house(&self, guild: u32) -> Option<&House> {
        self.guilds.get(&guild).map(|g| &g.house)
    }

    /// `0x0D25` (section 4): rank 1 or 2 only. Done, every online member gets the answer and (deviation 3) the new Etin; refused,
    /// the requester gets the answer.
    pub fn upgrade(&mut self, me: u32, key: u16, level: u8, rules: &HouseRules) {
        if !self.rank_of(me).is_some_and(is_officer) {
            return;
        }
        let Some(guild) = self.guild_of(me).map(|g| g.id) else { return };
        let house = &mut self.guilds.get_mut(&guild).expect("a member's guild").house;
        match house.upgrade(rules, key, level) {
            Err(result) => self.send_to(me, &gp::npc_upgrade(result, key, level, 0)),
            Ok(done) => {
                self.send_guild(guild, &gp::npc_upgrade(gh::upgrade::DONE, key, level, done.slot));
                self.send_guild(guild, &gp::etin(done.etin));
                self.write(GuildWrite::Upgrade { guild, slot: done.slot, key, level, etin: done.etin, keep_etin: done.keep_etin });
                // `0x0B09` (`CGuild::NpcUpgrade`): the NPC's type word, its new level, the guild's Etin and the keep Etin after.
                let name = self.name_of(guild);
                self.record(me, |h| game_log::guild_npc_upgrade(h, guild, &name, key, level, done.etin, done.keep_etin));
                tracing::info!(
                    guild,
                    by = me,
                    key,
                    level,
                    slot = done.slot,
                    etin = done.etin,
                    keep_etin = done.keep_etin,
                    "guild NPC upgraded"
                );
            }
        }
    }

    /// `0x0D34`'s checks (section 5): the guild, or the refusal (not the master 3, bought already 1, too little money 2).
    pub fn house_purchase(&self, me: u32, money: u32, rules: &HouseRules) -> Result<u32, u8> {
        let g = self.guild_of(me).filter(|_| self.rank_of(me) == Some(MASTER)).ok_or(gh::buy::NOT_MASTER)?;
        if g.house.bought {
            Err(gh::buy::ALREADY)
        } else if money < rules.buy_money {
            Err(gh::buy::NO_MONEY)
        } else {
            Ok(g.id)
        }
    }

    /// The purchase is committed (`GuildStore::buy_guild_house`, with the money): the house is bought, its keep Etin `HouseKeepEtin`;
    /// the house state stays as it was. Nothing is queued: the transaction wrote it (audit R16).
    pub fn house_bought(&mut self, guild: u32, rules: &HouseRules) {
        let Some(g) = self.guilds.get_mut(&guild) else { return };
        g.house.bought = true;
        g.house.keep_etin = rules.keep_etin;
        tracing::info!(guild, "guild house bought");
    }

    /// Etin returned by `me` to `guild` (section 3), committed by `GuildStore::guild_return_etin`: the guild's Etin grows by `sum` and
    /// its return count by 1, and every online member gets the new Etin. Nothing for 0 or a guild that is gone. Nothing is queued: the
    /// transaction wrote it (audit R16).
    pub fn return_etin(&mut self, guild: u32, me: u32, sum: u32) {
        if sum == 0 {
            return;
        }
        let Some(house) = self.guilds.get_mut(&guild).map(|g| &mut g.house) else { return };
        house.etin = house.etin.saturating_add(sum);
        house.etin_returns += 1;
        let (etin, returns) = (house.etin, house.etin_returns);
        self.send_guild(guild, &gp::etin(etin));
        // `0x0B0A` (`CUser::PacketGuild`): the guild's Etin and what was returned.
        let name = self.name_of(guild);
        self.record(me, |h| game_log::guild_etin(h, guild, &name, etin, sum));
        tracing::info!(guild, by = me, sum, etin, returns, "Etin returned");
    }

    // The bank (`docs/original/guild-bank.md`).

    /// A move with bag 255 on one side, planned by the guild's rules: the guild's id and the plan, `None` without a guild.
    #[allow(clippy::too_many_arguments)]
    pub fn bank_plan(
        &self,
        me: u32,
        inventory: &Inventory,
        defs: &ItemDefs,
        rules: &HouseRules,
        money: u32,
        from: (u8, u8),
        to: (u8, u8),
    ) -> Option<(u32, Result<Planned, Refusal>)> {
        let g = self.guild_of(me)?;
        let rank = g.member(me)?.rank;
        let allowed = guild_bank::allowed_slots(&g.house, rules);
        Some((g.id, guild_bank::plan(&g.bank, inventory, defs, g.house.ranking, allowed, rank, money, from, to)))
    }

    /// A planned move made, after the database has it: the changes applied, the other online members told (`0x0D32` and `0x0D31` for
    /// a move in or out, `0x0204` to everyone for a move inside the bank) and the mover's answer (`None` for a move inside the bank,
    /// whose answer is the guild's).
    #[allow(clippy::too_many_arguments)]
    pub fn bank_apply(
        &mut self,
        guild: u32,
        me: u32,
        inventory: &mut Inventory,
        planned: &Planned,
        from: (u8, u8),
        to: (u8, u8),
        money_after: u32,
    ) -> Option<Vec<u8>> {
        let Some(g) = self.guilds.get_mut(&guild) else {
            // Only a deposit into an empty bank can meet a disband meanwhile: the database holds the item with the disbanded guild.
            tracing::warn!(guild, character = me, "guild bank move for a guild disbanded meanwhile");
            inventory.take(from);
            return Some(w::item_move_result(from, inventory.get(from.0, from.1), to, None));
        };
        for change in &planned.changes {
            guild_bank::apply(&mut g.bank, inventory, change);
        }
        let bank_item = |slot: u8| g.bank.get(slot);
        let bag_item = |place: (u8, u8)| inventory.get(place.0, place.1);
        match planned.kind {
            BankKind::Deposit => {
                let (now_bag, now_bank) = (bag_item(from).cloned(), bank_item(to.1).cloned());
                let answer = w::item_move_result(from, now_bag.as_ref(), to, now_bank.as_ref());
                if let Some(item) = &now_bag {
                    self.send_guild_except(guild, me, &gp::bank_item_out(to.1, item, me));
                }
                if let Some(item) = &now_bank {
                    self.send_guild_except(guild, me, &gp::bank_item_in(to.1, item, me));
                }
                Some(answer)
            }
            BankKind::Withdraw => {
                let (now_bank, now_bag) = (bank_item(from.1).cloned(), bag_item(to).cloned());
                let answer = w::bank_to_bag_result(from, now_bank.as_ref(), to, now_bag.as_ref(), money_after);
                if let Some(item) = &now_bag {
                    self.send_guild_except(guild, me, &gp::bank_item_out(from.1, item, me));
                }
                if let Some(item) = &now_bank {
                    self.send_guild_except(guild, me, &gp::bank_item_in(from.1, item, me));
                }
                Some(answer)
            }
            BankKind::Rearrange => {
                let packet = w::item_move_result(from, bank_item(from.1), to, bank_item(to.1));
                self.send_guild(guild, &packet);
                None
            }
        }
    }

    // The ranking battle (`docs/original/guild-ranking.md`).

    /// Whether the character went into this battle already.
    pub fn entered(&self, me: u32, tag: Tag) -> bool {
        self.battle_entered.get(&me) == Some(&tag)
    }

    /// The character arrives in the battle map while it is open: its tag is the battle's, and its guild's entry count grows by one
    /// (section 5). False, with nothing counted, when it was counted for this battle already.
    pub fn count_entry(&mut self, me: u32, tag: Tag) -> bool {
        if self.entered(me, tag) {
            return false;
        }
        self.battle_entered.insert(me, tag);
        if let Some(guild) = self.guild_of(me).map(|g| g.id) {
            let house = &mut self.guilds.get_mut(&guild).expect("a member's guild").house;
            house.battle_entries += 1;
            tracing::info!(guild, character = me, entries = house.battle_entries, "ranking battle entry");
        }
        true
    }

    /// Battle points from a monster kill (section 7): the guild's points grow by `points`, and the country's best moves when the
    /// guild is now strictly above it. None for an unknown guild.
    pub fn add_battle_points(&mut self, guild: u32, points: u32) -> Option<BattleScore> {
        let g = self.guilds.get_mut(&guild)?;
        g.house.battle_points = g.house.battle_points.saturating_add(points);
        let total = g.house.battle_points;
        let best = &mut self.battle_best[side(g.country)];
        if total > best.0 {
            *best = (total, guild);
        }
        Some(BattleScore { points: total, best_points: best.0, best_guild: best.1 })
    }

    /// `0x0B0B` (`CGuild::SendRankingZone`): the points a member's kill got the guild, the battle's total so far and the guild's total.
    pub fn record_battle(&self, who: u32, guild: u32, got: u32, battle_total: u32) {
        let name = self.name_of(guild);
        let total = self.guilds.get(&guild).map_or(0, |g| g.house.points);
        self.record(who, |h| game_log::guild_battle_points(h, guild, &name, got, battle_total, total));
    }

    /// The ranking update (section 8): every guild's score, then each country's places. Points, ranking, `use_house` and Etin are
    /// set, the three battle counters zeroed, the best pairs reset, the database written, the guilds that paid their keep told their
    /// Etin (`0x0D23`) and each country's online players the ranking (`0x0D37`). The placed guilds, country by country.
    pub fn apply_ranking(&mut self) -> Vec<Placed> {
        let mut ids: Vec<u32> = self.guilds.keys().copied().collect();
        ids.sort_unstable();
        let mut scored: [Vec<(u32, u32)>; 2] = Default::default();
        for id in &ids {
            let g = self.guilds.get_mut(id).expect("a listed guild");
            let h = &mut g.house;
            let score = ranking::score(h.battle_points, h.etin_returns, h.battle_entries);
            (h.battle_points, h.battle_entries, h.etin_returns, h.points) = (0, 0, 0, score);
            if score == 0 {
                (h.ranking, h.use_house) = (UNRANKED, gh::HOUSE_NOT_OPEN);
            } else {
                scored[side(g.country)].push((*id, score));
            }
        }
        let mut placed = Vec::new();
        let mut paid = Vec::new();
        for (country, list) in scored.iter().enumerate() {
            let mut listed = Vec::new();
            for (guild, score, place) in ranking::places(list) {
                let h = &mut self.guilds.get_mut(&guild).expect("a placed guild").house;
                if usize::from(place) > ranking::PLACES {
                    (h.ranking, h.use_house) = (UNRANKED, gh::HOUSE_NOT_OPEN);
                    continue;
                }
                h.ranking = place;
                if h.bought {
                    if h.etin < h.keep_etin {
                        h.use_house = gh::HOUSE_CLOSED;
                    } else {
                        h.etin -= h.keep_etin;
                        h.use_house = gh::HOUSE_OPEN;
                        paid.push((guild, h.etin));
                    }
                }
                listed.push((guild, score, place));
                placed.push(Placed { guild, score, place, country: country as u8 });
            }
            // The client sets everyone else of its list to 31 itself.
            self.send_side(country, &gp::ranking(&listed));
        }
        for (guild, etin) in paid {
            self.send_guild(guild, &gp::etin(etin));
        }
        self.battle_best = [(0, 0); 2];
        let rows = ids
            .iter()
            .map(|id| {
                let h = &self.guilds[id].house;
                RankingRow { guild: *id, points: h.points, etin: h.etin, ranking: h.ranking, use_house: h.use_house }
            })
            .collect();
        self.write(GuildWrite::Ranking { rows });
        placed
    }

    /// When the ranking update is due (unix seconds), or nothing waits (deviation 4).
    pub fn write_ranking_due(&self, due: Option<i64>) {
        self.write(GuildWrite::RankingDue(due));
    }

    /// The players of one side (Light 0, Fury 1) online.
    fn send_side(&self, side_of: usize, packet: &[u8]) {
        for p in self.online.values().filter(|p| side(p.country) == side_of) {
            p.send(packet);
        }
    }

    /// Joins `guild` as a new member of `rank` (`JoinUser`, `JoinOK`, `CreateAddUser`): any request elsewhere withdrawn first; an
    /// online player's penalty starts.
    fn add_member(&mut self, guild: u32, id: u32, member: Member, now: Instant) {
        if let Some(other) = self.requested(id) {
            self.withdraw(other, id);
        }
        if let Some(p) = self.online.get_mut(&id) {
            p.penalty_until = Some(now + JOIN_PENALTY);
        }
        self.of.insert(id, guild);
        if let Some(g) = self.guilds.get_mut(&guild) {
            g.members.push(member);
        }
    }

    /// A pending request leaves its guild (`JoinRequestCancle`): the officers are told.
    fn withdraw(&mut self, guild: u32, id: u32) {
        let Some(g) = self.guilds.get_mut(&guild) else { return };
        g.requests.retain(|r| r.id != id);
        self.of.remove(&id);
        self.write(GuildWrite::Remove { character: id });
        self.send_officers(guild, &gp::request_removed(id));
    }

    fn member_from(&self, id: u32, rank: u8) -> Option<Member> {
        let p = self.online.get(&id)?;
        Some(Member { id, name: p.name.clone(), level: p.level, job: p.job, rank })
    }

    // Creating (section 4).

    /// `0x0D21`: `party` is the requester's party in its order, each with whether it is on the requester's map; `size` the party
    /// size needed (`GAME_GUILD_PARTY_SIZE`; 1 or less: the requester alone).
    pub fn create_request(&mut self, me: u32, party: &[(u32, bool)], size: usize, name: &[u8], remark: &[u8], now: Instant) {
        let Some(requester) = self.online.get(&me) else { return };
        let fail = |code: u8| requester.send(&gp::create_result(code, 0, 0, name, remark));
        if requester.level < CREATE_MIN_LEVEL {
            return fail(gp::CREATE_LEVEL);
        }
        if requester.grow == EASY {
            return fail(gp::CREATE_MODE);
        }
        if self.guild_of(me).is_some() {
            return fail(gp::CREATE_IN_GUILD);
        }
        if self.in_agreement(me) {
            return fail(gp::CREATE_MEMBERS);
        }
        let members: Vec<(u32, bool)> = if size <= 1 { vec![(me, true)] } else { party.to_vec() };
        if members.len() != size.max(1) || !members.iter().any(|&(m, _)| m == me) {
            return fail(gp::CREATE_MEMBERS);
        }
        for &(m, here) in &members {
            let Some(p) = self.online.get(&m) else { return fail(gp::CREATE_MEMBERS) };
            if self.guild_of(m).is_some() {
                return fail(gp::CREATE_IN_GUILD);
            }
            if p.grow == EASY {
                return fail(gp::CREATE_MODE);
            }
            if self.in_agreement(m) || !here {
                return fail(gp::CREATE_MEMBERS);
            }
            if p.penalised(now) {
                return fail(gp::CREATE_PENALTY);
            }
        }
        let taken = self.guilds.values().any(|g| g.name.eq_ignore_ascii_case(name))
            || self.agreements.iter().any(|a| matches!(&a.kind, Kind::Create { name: n, .. } if n.eq_ignore_ascii_case(name)));
        if !valid_name(name) || taken {
            return fail(gp::CREATE_NAME);
        }
        if !valid_remark(remark) {
            return fail(gp::CREATE_FAILED);
        }
        let mut asked: Vec<u32> = vec![me];
        asked.extend(members.iter().map(|&(m, _)| m).filter(|&m| m != me));
        let a = Agreement {
            kind: Kind::Create { name: name.to_vec(), remark: remark.to_vec() },
            requester: me,
            waiting: asked.clone(),
            members: asked,
            due: now + ANSWER_TIME,
            agreed: false,
        };
        // Everyone, the requester included, is asked (`SendGuildCreateAgree` excludes nobody).
        self.send_agreement(&a, &gp::create_agreement(me, name, remark));
        tracing::info!(requester = me, name = %text(name), members = a.members.len(), "guild creation asked");
        self.agreements.push(a);
    }

    /// `0x0D22`: no ends it for everyone; the last yes hands it to the requester's connection (`ZoneEvent::GuildAgreed`), which pays.
    pub fn create_answer(&mut self, me: u32, yes: bool) {
        let Some(i) = self.agreements.iter().position(|a| !a.agreed && matches!(a.kind, Kind::Create { .. }) && a.waiting.contains(&me))
        else {
            return;
        };
        if !yes {
            let a = self.agreements.remove(i);
            let Kind::Create { name, .. } = &a.kind else { unreachable!() };
            self.send_agreement(&a, &gp::create_failed(gp::CREATE_REJECTED, name));
            tracing::info!(member = me, requester = a.requester, "guild creation refused");
            return;
        }
        let a = &mut self.agreements[i];
        a.waiting.retain(|&m| m != me);
        if a.waiting.is_empty() {
            a.agreed = true;
            // The founder's connection takes it up at once; if it never does, the agreement lapses (audit R27).
            a.due = Instant::now() + AGREED_TIME;
            if let Some(p) = self.online.get(&a.requester) {
                let _ = p.events.send(ZoneEvent::GuildAgreed);
            }
        }
    }

    /// The requester's connection after `ZoneEvent::GuildAgreed`, with enough money: the creation goes to the writer; its outcome
    /// comes back on the receiver. `None` when there is no agreed creation of this requester (it ended meanwhile).
    pub fn create_write(&mut self, me: u32, server_id: u8) -> Option<oneshot::Receiver<anyhow::Result<GuildCreated>>> {
        let i = self.agreements.iter().position(|a| a.agreed && a.requester == me)?;
        let a = &self.agreements[i];
        let Kind::Create { name, remark } = &a.kind else { return None };
        let requester = self.online.get(&me)?;
        // Anyone who joined a guild meanwhile is left out.
        let members =
            a.members.iter().filter(|&&m| self.guild_of(m).is_none()).map(|&m| (m, if m == me { MASTER } else { NEW_MEMBER })).collect();
        let guild = NewGuild {
            server_id,
            name: text(name),
            country: requester.country,
            remark: text(remark),
            members,
            payer: me,
            cost: CREATE_COST,
        };
        let (reply, outcome) = oneshot::channel();
        let _ = self.writer.as_ref()?.send(Queued::Create(guild, reply));
        // The creation is on its way and its outcome ends the agreement: it no longer lapses by the clock.
        self.agreements[i].due = Instant::now() + CREATING_TIME;
        Some(outcome)
    }

    /// The creation failed at the requester (`0x0D01` with `code` to everyone asked): not enough money, or what the database said.
    pub fn create_failed(&mut self, me: u32, code: u8) {
        if let Some(i) = self.agreements.iter().position(|a| a.agreed && a.requester == me) {
            let a = self.agreements.remove(i);
            let Kind::Create { name, .. } = &a.kind else { unreachable!() };
            self.send_agreement(&a, &gp::create_failed(code, name));
        }
    }

    /// The creation is stored as guild `id` (`CreateGuild`'s sends): `money` is the requester's money after paying.
    pub fn create_done(&mut self, me: u32, id: u32, money: u32, now: Instant) -> Aftermath {
        let mut after = Aftermath::default();
        let Some(i) = self.agreements.iter().position(|a| a.agreed && a.requester == me) else { return after };
        let a = self.agreements.remove(i);
        let Kind::Create { name, remark } = a.kind else { unreachable!() };
        let Some(country) = self.online.get(&me).map(|p| p.country) else { return after };
        self.guilds.insert(
            id,
            Guild {
                id,
                name: name.clone(),
                country,
                remark: remark.clone(),
                members: Vec::new(),
                requests: Vec::new(),
                house: House::default(),
                bank: GuildBank::default(),
            },
        );
        let joining: Vec<u32> = a.members.iter().copied().filter(|&m| self.guild_of(m).is_none()).collect();
        for &m in &joining {
            let rank = if m == me { MASTER } else { NEW_MEMBER };
            // Someone who left the world meanwhile is a member all the same, with what the world knew of it.
            let member = self.member_from(m, rank).unwrap_or(Member { id: m, name: Vec::new(), level: 0, job: 0, rank });
            self.add_member(id, m, member, now);
        }
        // `0x0B01` (`CGuildMgr::CreateGuild`) for the creator, `0x0B04` (`CreateAddUser`) for every member joined.
        let guild_name = text(&name);
        self.record(me, |h| game_log::guild_create(h, id, &guild_name));
        for &m in &joining {
            self.record(m, |h| game_log::guild_join(h, id, &guild_name));
        }
        for &m in &joining {
            let rank = if m == me { MASTER } else { NEW_MEMBER };
            self.send_to(m, &gp::create_result(gp::CREATED, id, rank, &name, &remark));
            if m == me {
                self.send_to(m, &w::set_money(money));
            }
            for packet in self.member_list(&self.guilds[&id], m) {
                self.send_to(m, &packet);
            }
            after.look(m, self.guilds.get(&id));
        }
        self.send_country(country, &gp::guild_added(&self.guilds[&id].unit()));
        tracing::info!(guild = id, name = %text(&name), master = me, members = joining.len(), "guild created");
        after
    }

    // Inviting (section 5).

    /// `0x0D05`: `here` whether the target is on the inviter's map.
    pub fn invite(&mut self, me: u32, target: u32, here: bool, now: Instant) {
        let refuse = || self.send_to(me, &gp::invite_answer(false, target));
        let Some(inviter) = self.online.get(&me) else { return };
        let Some(t) = self.online.get(&target) else { return refuse() };
        let free = !self.of.contains_key(&target) && !self.in_agreement(target) && !self.invites.contains_key(&target);
        if !self.rank_of(me).is_some_and(is_officer) || !here || target == me || t.country != inviter.country || !free {
            return refuse();
        }
        self.invites.insert(target, Invite { inviter: me, due: now + ANSWER_TIME });
        t.send(&gp::invitation(me));
    }

    /// `0x0D06`: `inviter_here` whether the inviter is on the answerer's map (else the answer is lost, as in the original).
    pub fn invite_answer(&mut self, me: u32, yes: bool, inviter_here: impl Fn(u32) -> bool, now: Instant) -> Aftermath {
        let mut after = Aftermath::default();
        let Some(invite) = self.invites.remove(&me) else { return after };
        let inviter = invite.inviter;
        if !inviter_here(inviter) {
            return after;
        }
        if !yes {
            self.send_to(inviter, &gp::invite_answer(false, inviter));
            return after;
        }
        let Some(guild) = self.guild_of(inviter).filter(|g| g.member(inviter).is_some_and(|m| is_officer(m.rank))).map(|g| g.id) else {
            return after;
        };
        if self.of.contains_key(&me) || self.in_agreement(me) {
            return after;
        }
        let Some(member) = self.member_from(me, NEW_MEMBER) else { return after };
        self.join(guild, member, now, &mut after);
        self.write(GuildWrite::Add { guild, character: me, rank: NEW_MEMBER });
        let name = self.name_of(guild);
        self.record(me, |h| game_log::guild_join(h, guild, &name));
        self.send_to(inviter, &gp::invite_answer(true, inviter));
        tracing::info!(guild, inviter, member = me, "joined a guild by invitation");
        after
    }

    /// `JoinUser`: `0x0D08` to the player, `0x0D14` to the guild, the member list to the player.
    fn join(&mut self, guild: u32, member: Member, now: Instant, after: &mut Aftermath) {
        let id = member.id;
        let unit = member.unit();
        self.add_member(guild, id, member, now);
        let g = &self.guilds[&guild];
        self.send_to(id, &gp::joined(true, guild, NEW_MEMBER, &g.name));
        self.send_guild(guild, &gp::member_added(gp::ADDED_ONLINE, &unit));
        for packet in self.member_list(g, id) {
            self.send_to(id, &packet);
        }
        for packet in bank_packets(g) {
            self.send_to(id, &packet);
        }
        if let Some(packet) = gp::npc_levels(&g.house.used_slots()) {
            self.send_to(id, &packet);
        }
        after.look(id, Some(g));
    }

    // Join requests (section 6).

    /// `0x0D07`.
    pub fn join_request(&mut self, me: u32, guild: u32, now: Instant) {
        let Some(p) = self.online.get(&me) else { return };
        if p.penalised(now) {
            return p.send(&gp::request_result(gp::REQUEST_PENALTY));
        }
        let busy = p.grow == EASY || self.guild_of(me).is_some() || self.in_agreement(me) || self.invites.contains_key(&me);
        if busy || !self.guilds.get(&guild).is_some_and(|g| g.country == p.country) {
            return p.send(&gp::request_result(gp::REQUEST_REFUSED));
        }
        if let Some(other) = self.requested(me) {
            self.withdraw(other, me);
        }
        let Some(request) = self.member_from(me, REQUEST) else { return };
        let unit = request.request_unit();
        self.of.insert(me, guild);
        self.guilds.get_mut(&guild).expect("checked").requests.push(request);
        self.write(GuildWrite::Add { guild, character: me, rank: REQUEST });
        let name = self.name_of(guild);
        self.record(me, |h| game_log::guild_request(h, guild, &name));
        self.send_officers(guild, &gp::request_added(&unit));
        self.send_to(me, &gp::request_result(gp::REQUEST_SENT));
    }

    /// `0x0D08` from an officer: accept (`JoinOK`) or refuse (`JoinNO`) a request.
    pub fn join_answer(&mut self, me: u32, accept: bool, target: u32, now: Instant) -> Aftermath {
        let mut after = Aftermath::default();
        let Some(guild) = self.guild_of(me).filter(|g| g.member(me).is_some_and(|m| is_officer(m.rank))).map(|g| g.id) else {
            return after;
        };
        if self.requested(target) != Some(guild) || (accept && self.in_agreement(target)) {
            return after;
        }
        let g = self.guilds.get_mut(&guild).expect("checked");
        let i = g.requests.iter().position(|r| r.id == target).expect("checked");
        let mut member = g.requests.remove(i);
        if !accept {
            self.of.remove(&target);
            self.write(GuildWrite::Remove { character: target });
            self.send_to(target, &gp::joined(false, guild, NEW_MEMBER, &self.guilds[&guild].name));
            self.send_officers(guild, &gp::request_removed(target));
            return after;
        }
        member.rank = NEW_MEMBER;
        if let Some(p) = self.online.get(&target) {
            member.level = p.level;
        }
        let unit = member.unit();
        self.add_member(guild, target, member, now);
        self.write(GuildWrite::Accept { character: target });
        let name = self.name_of(guild);
        self.record(target, |h| game_log::guild_join(h, guild, &name));
        self.send_officers(guild, &gp::request_removed(target));
        let online = self.online.contains_key(&target);
        self.send_guild(guild, &gp::member_added(if online { gp::ADDED_ONLINE } else { gp::ADDED_OFFLINE }, &unit));
        if online {
            let g = &self.guilds[&guild];
            self.send_to(target, &gp::joined(true, guild, NEW_MEMBER, &g.name));
            for packet in self.member_list(g, target) {
                self.send_to(target, &packet);
            }
            for packet in bank_packets(g) {
                self.send_to(target, &packet);
            }
            if let Some(packet) = gp::npc_levels(&g.house.used_slots()) {
                self.send_to(target, &packet);
            }
            after.look(target, Some(g));
        }
        tracing::info!(guild, officer = me, member = target, "join request accepted");
        after
    }

    // Leaving, kicking, ranks, the remark (sections 7, 8, 13a).

    /// Out of the guild; a guild left with no member is disbanded.
    fn remove_member(&mut self, guild: u32, id: u32, how: Leave, after: &mut Aftermath) {
        // `0x0B05` to `0x0B07` (`JoinLeave`, `JoinExpel`, `DeleteChar`), under the member who goes.
        let name = self.name_of(guild);
        self.record(id, |h| game_log::guild_leave(h, guild, &name, how));
        let Some(g) = self.guilds.get_mut(&guild) else { return };
        g.members.retain(|m| m.id != id);
        self.of.remove(&id);
        self.write(GuildWrite::Remove { character: id });
        if self.online.contains_key(&id) {
            after.look(id, None);
        }
        if self.guilds[&guild].members.is_empty() {
            self.fire(guild, id, after);
        }
    }

    /// `0x0D09`: the master only as the last member.
    pub fn leave(&mut self, me: u32) -> Aftermath {
        let mut after = Aftermath::default();
        let Some(g) = self.guild_of(me) else { return after };
        if self.in_agreement(me) {
            return after;
        }
        let guild = g.id;
        if g.member(me).is_some_and(|m| m.rank == MASTER) && g.members.len() > 1 {
            self.send_to(me, &gp::leave_result(false));
            return after;
        }
        self.send_to(me, &gp::leave_result(true));
        self.remove_member(guild, me, Leave::Left, &mut after);
        self.send_guild(guild, &gp::member_state(gp::STATE_LEFT, me));
        tracing::info!(guild, member = me, "left the guild");
        after
    }

    /// `0x0D0A` from an officer: a member of a higher rank number, online or not.
    pub fn kick(&mut self, me: u32, target: u32) -> Aftermath {
        let mut after = Aftermath::default();
        let Some(g) = self.guild_of(me) else { return after };
        let guild = g.id;
        let mine = g.member(me).map_or(0, |m| m.rank);
        if !is_officer(mine) || self.in_agreement(me) {
            return after;
        }
        if !g.member(target).is_some_and(|t| t.rank > mine) {
            self.send_to(me, &gp::kick_result(false, target));
            return after;
        }
        let packet = gp::kick_result(true, target);
        self.send_to(target, &packet);
        self.send_to(me, &packet);
        self.remove_member(guild, target, Leave::Expelled, &mut after);
        self.send_guild(guild, &gp::member_state(gp::STATE_KICKED, target));
        tracing::info!(guild, officer = me, member = target, "kicked from the guild");
        after
    }

    /// `0x0D0C` (section 7): `down` 0 promotes, 1 demotes.
    pub fn change_rank(&mut self, me: u32, down: u8, target: u32) {
        let step: i16 = match down {
            0 => -1,
            1 => 1,
            _ => return,
        };
        let Some(g) = self.guild_of(me) else { return };
        let (guild, officers) = (g.id, g.officers());
        let mine = g.member(me).map_or(0, |m| m.rank);
        let Some(theirs) = g.member(target).map(|m| m.rank) else { return };
        let new = i16::from(theirs) + step;
        let allowed = is_officer(mine)
            && target != me
            && theirs > mine
            && (2..=9).contains(&new)
            && (new > 2 || officers < MAX_OFFICERS)
            && !(mine == OFFICER && new == 2);
        if !allowed {
            return;
        }
        let new = new as u8;
        if let Some(m) = self.guilds.get_mut(&guild).and_then(|g| g.member_mut(target)) {
            m.rank = new;
        }
        self.write(GuildWrite::Rank { character: target, rank: new });
        self.send_guild(guild, &gp::member_state(gp::STATE_RANK + new, target));
    }

    // Operator commands (`docs/original/gm-commands.md` section 8.6 with deviation 10).

    /// A live guild by name, ignoring case.
    pub fn guild_id_by_name(&self, name: &[u8]) -> Option<u32> {
        self.guilds.values().find(|g| g.name.eq_ignore_ascii_case(name)).map(|g| g.id)
    }

    /// `FA13`: the member's rank becomes `grade` (2 to 9; one master per guild, so 1 is refused, and so is changing the master's own
    /// rank). Not the rank it has already; into 1 or 2 needs a free officer place. `0x0D0C` goes to the guild as for a rank change.
    pub fn admin_grade(&mut self, guild_name: &[u8], member_name: &[u8], grade: u8) -> bool {
        let Some(guild) = self.guild_id_by_name(guild_name) else { return false };
        let g = &self.guilds[&guild];
        let Some((target, current)) = g.members.iter().find(|m| m.name.eq_ignore_ascii_case(member_name)).map(|m| (m.id, m.rank)) else {
            return false;
        };
        if !(2..=9).contains(&grade)
            || current == grade
            || current == MASTER
            || (is_officer(grade) && !is_officer(current) && g.officers() >= MAX_OFFICERS)
        {
            return false;
        }
        if let Some(m) = self.guilds.get_mut(&guild).and_then(|g| g.member_mut(target)) {
            m.rank = grade;
        }
        self.write(GuildWrite::Rank { character: target, rank: grade });
        self.send_guild(guild, &gp::member_state(gp::STATE_RANK + grade, target));
        true
    }

    /// `FA14`: the guild's ranking (1 to 31) with its house in use, saved (deviation 10).
    pub fn admin_ranking(&mut self, guild_name: &[u8], ranking: u8) -> bool {
        let Some(guild) = self.guild_id_by_name(guild_name) else { return false };
        if !(1..=31).contains(&ranking) {
            return false;
        }
        let house = &mut self.guilds.get_mut(&guild).expect("found above").house;
        (house.ranking, house.use_house) = (ranking, crate::guild_house::HOUSE_OPEN);
        self.write(GuildWrite::Standing { guild, ranking, use_house: crate::guild_house::HOUSE_OPEN });
        true
    }

    /// `FA15`: the guild's points, saved.
    pub fn admin_points(&mut self, guild_name: &[u8], points: u32) -> bool {
        let Some(guild) = self.guild_id_by_name(guild_name) else { return false };
        self.guilds.get_mut(&guild).expect("found above").house.points = points;
        self.write(GuildWrite::Points { guild, points });
        true
    }

    /// `0x0D20` from the master; otherwise the current remark back (deviation 6).
    pub fn set_remark(&mut self, me: u32, remark: &[u8]) {
        let Some(g) = self.guild_of(me) else { return };
        let guild = g.id;
        if g.member(me).is_none_or(|m| m.rank != MASTER) || !valid_remark(remark) {
            return self.send_to(me, &gp::remark(false, &g.remark));
        }
        let g = self.guilds.get_mut(&guild).expect("checked");
        let changed = g.remark != remark;
        g.remark = remark.to_vec();
        // The guild is told either way; a remark that changes nothing is not written (`docs/server/audit.md` R4).
        if changed {
            self.write(GuildWrite::Remark { guild, remark: text(remark) });
        }
        self.send_guild(guild, &gp::remark(true, remark));
    }

    // Disbanding (section 11).

    /// `0x0D03` from the master: `officers_here` the officers of its guild in its party on its map.
    pub fn disband_request(&mut self, me: u32, officers_here: &[u32], now: Instant) -> Aftermath {
        let mut after = Aftermath::default();
        let Some(g) = self.guild_of(me) else { return after };
        let guild = g.id;
        if g.member(me).is_none_or(|m| m.rank != MASTER) || self.in_agreement(me) {
            return after;
        }
        // Deviation 4 (`docs/original/guild-bank.md` section 14): a bank that holds items keeps the guild.
        if !g.bank.is_empty() {
            self.send_to(me, &gp::disband_result(gp::DISBAND_FAILED));
            tracing::info!(guild, master = me, "disband refused: the guild bank holds items");
            return after;
        }
        if g.members.len() <= DISBAND_AT_ONCE {
            self.fire(guild, me, &mut after);
            return after;
        }
        let mut asked = vec![me];
        asked.extend(officers_here.iter().copied().filter(|&o| {
            o != me && self.guild_of(o).is_some_and(|og| og.id == guild) && self.rank_of(o).is_some_and(is_officer) && !self.in_agreement(o)
        }));
        if asked.len() < 1 + DISBAND_OFFICERS {
            self.send_to(me, &gp::disband_result(gp::DISBAND_MEMBERS));
            return after;
        }
        let a = Agreement {
            kind: Kind::Disband { guild },
            requester: me,
            waiting: asked.clone(),
            members: asked,
            due: now + ANSWER_TIME,
            agreed: false,
        };
        self.send_agreement(&a, &gp::disband_agreement(me));
        self.agreements.push(a);
        after
    }

    /// `0x0D04`: no ends it; when everyone asked said yes, the guild is disbanded (deviation 3).
    pub fn disband_answer(&mut self, me: u32, yes: bool) -> Aftermath {
        let mut after = Aftermath::default();
        let Some(i) = self.agreements.iter().position(|a| matches!(a.kind, Kind::Disband { .. }) && a.waiting.contains(&me)) else {
            return after;
        };
        if !yes {
            let a = self.agreements.remove(i);
            self.send_agreement(&a, &gp::disband_result(gp::DISBAND_REJECTED));
            return after;
        }
        let a = &mut self.agreements[i];
        a.waiting.retain(|&m| m != me);
        if a.waiting.is_empty() {
            let a = self.agreements.remove(i);
            let Kind::Disband { guild } = a.kind else { unreachable!() };
            // A deposit made during the vote would be stranded in a dead guild: the request's check again (audit R23).
            if self.guilds.get(&guild).is_some_and(|g| !g.bank.is_empty()) {
                self.send_agreement(&a, &gp::disband_result(gp::DISBAND_FAILED));
                tracing::info!(guild, master = a.requester, "disband refused: the guild bank holds items");
                return after;
            }
            self.fire(guild, a.requester, &mut after);
        }
        after
    }

    /// `CGuild::Fire`: every member and request out, `0x0D0C 101` to the online members, `0x0D11` to the country.
    fn fire(&mut self, guild: u32, by: u32, after: &mut Aftermath) {
        // `0x0B02` (`CGuild::Fire`), under the member who disbanded it (the last one to leave, or the master).
        let guild_name = self.name_of(guild);
        self.record(by, |h| game_log::guild_disband(h, guild, &guild_name));
        self.send_guild(guild, &gp::member_state(gp::STATE_DISBANDED, 0));
        let Some(g) = self.guilds.remove(&guild) else { return };
        for m in g.members.iter().chain(&g.requests) {
            self.of.remove(&m.id);
            if self.online.contains_key(&m.id) && m.rank != REQUEST {
                after.look(m.id, None);
            }
        }
        self.agreements.retain(|a| a.kind != Kind::Disband { guild });
        for item in g.bank.items() {
            tracing::info!(
                guild,
                slot = item.slot,
                item_type = item.item_type,
                type_id = item.type_id,
                count = item.count,
                "disbanded guild's bank item kept in the database"
            );
        }
        self.write(GuildWrite::Disband { guild });
        self.send_country(g.country, &gp::guild_removed(guild));
        tracing::info!(guild, name = %text(&g.name), "guild disbanded");
    }

    // Deletion and the clock (section 10).

    /// A character deleted (at the character screen): its request withdrawn, or out of its guild; a master's guild gets the member
    /// with the lowest rank number, the longest in the guild among equals (deviation 5).
    pub fn deleted(&mut self, id: u32) -> Aftermath {
        let mut after = Aftermath::default();
        if let Some(guild) = self.requested(id) {
            self.withdraw(guild, id);
            return after;
        }
        let Some(g) = self.guild_of(id) else { return after };
        let (guild, was_master) = (g.id, g.member(id).is_some_and(|m| m.rank == MASTER));
        self.remove_member(guild, id, Leave::Deleted, &mut after);
        let Some(g) = self.guilds.get_mut(&guild) else { return after };
        let packet = gp::member_state(gp::STATE_LEFT, id);
        let heir = if was_master { g.members.iter_mut().min_by_key(|m| m.rank) } else { None };
        let heir = heir.map(|m| {
            m.rank = MASTER;
            m.id
        });
        self.send_guild(guild, &packet);
        if let Some(heir) = heir {
            self.write(GuildWrite::Rank { character: heir, rank: MASTER });
            self.send_guild(guild, &gp::member_state(gp::STATE_RANK + MASTER, heir));
            tracing::info!(guild, master = heir, "guild master handed on after a deletion");
        }
        after
    }

    /// The one-second pass: invitations lapse without a word, agreements with their failure (deviation 2).
    pub fn expire(&mut self, now: Instant) {
        self.invites.retain(|_, i| now < i.due);
        let (lapsed, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.agreements).into_iter().partition(|a| now >= a.due);
        self.agreements = kept;
        for a in lapsed {
            let packet = match &a.kind {
                Kind::Create { name, .. } => gp::create_failed(gp::CREATE_FAILED, name),
                Kind::Disband { .. } => gp::disband_result(gp::DISBAND_FAILED),
            };
            self.send_agreement(&a, &packet);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::StoredMember;
    use crate::sync::Relock;
    use shaiya_proto::frame::{opcode, payload};

    struct Client {
        packets: crate::outbox::OutboxInbox,
        events: mpsc::UnboundedReceiver<ZoneEvent>,
    }

    impl Client {
        fn opcodes(&mut self) -> Vec<u16> {
            self.drain().iter().map(|p| opcode(p)).collect()
        }

        fn drain(&mut self) -> Vec<Vec<u8>> {
            std::iter::from_fn(|| self.packets.try_recv().ok()).collect()
        }
    }

    /// A store whose writes fail the first `fails` times, then land in `written`.
    struct Flaky {
        fails: std::sync::atomic::AtomicUsize,
        tries: std::sync::atomic::AtomicUsize,
        written: std::sync::Mutex<Vec<GuildWrite>>,
    }

    impl crate::backend::GuildStore for Flaky {
        async fn load_guilds(&self, _: u8) -> anyhow::Result<Vec<crate::backend::StoredGuild>> {
            Ok(Vec::new())
        }
        async fn create_guild(&self, _: &NewGuild) -> anyhow::Result<GuildCreated> {
            anyhow::bail!("not used")
        }
        async fn write_guild(&self, write: &GuildWrite) -> anyhow::Result<()> {
            use std::sync::atomic::Ordering::SeqCst;
            self.tries.fetch_add(1, SeqCst);
            if self.fails.load(SeqCst) > 0 {
                self.fails.fetch_sub(1, SeqCst);
                anyhow::bail!("database down");
            }
            self.written.lock().unwrap().push(write.clone());
            Ok(())
        }
        async fn load_ranking_due(&self) -> anyhow::Result<Option<i64>> {
            Ok(None)
        }
        async fn buy_guild_house(&self, _: u32, _: u32, _: u32, _: u32) -> anyhow::Result<bool> {
            Ok(false)
        }
    }

    #[tokio::test]
    async fn a_failed_guild_write_is_tried_again_then_given_up_and_a_flush_waits_for_the_queue() {
        use std::sync::atomic::Ordering::SeqCst;
        let store = Flaky { fails: 2.into(), tries: 0.into(), written: Default::default() };
        let (writer, queue) = mpsc::unbounded_channel();
        let first = GuildWrite::Remark { guild: 1, remark: "one".into() };
        let second = GuildWrite::Remark { guild: 1, remark: "two".into() };
        writer.send(Queued::Write(first.clone())).unwrap();
        let (done, written) = oneshot::channel();
        writer.send(Queued::Flush(done)).unwrap();
        drop(writer);
        run_writer_with(&store, queue, &[std::time::Duration::from_millis(1); 3]).await;
        assert!(written.await.is_ok(), "the flush was answered");
        assert_eq!(*store.written.lock().unwrap(), std::slice::from_ref(&first), "the third try landed");
        assert_eq!(store.tries.load(SeqCst), 3);
        // Down for good: four tries, the write is dropped, the next one goes on in order.
        store.fails.store(4, SeqCst);
        store.tries.store(0, SeqCst);
        let (writer, queue) = mpsc::unbounded_channel();
        writer.send(Queued::Write(second.clone())).unwrap();
        drop(writer);
        run_writer_with(&store, queue, &[std::time::Duration::from_millis(1); 3]).await;
        assert_eq!(store.tries.load(SeqCst), 4);
        assert_eq!(*store.written.lock().unwrap(), std::slice::from_ref(&first), "given up");
    }

    fn world() -> (Guilds, mpsc::UnboundedReceiver<Queued>) {
        let (writer, writes) = mpsc::unbounded_channel();
        (Guilds { writer: Some(writer), ..Guilds::default() }, writes)
    }

    fn enter(g: &mut Guilds, id: u32, level: u16) -> Client {
        let (outbox, packets) = crate::outbox::channel(usize::MAX);
        let (events, events_rx) = mpsc::unbounded_channel();
        let name = format!("P{id}").into_bytes();
        g.connect(id, Player { outbox, events, name, country: 0, job: 1, level, grow: 1, penalty_until: None });
        Client { packets, events: events_rx }
    }

    fn written(writes: &mut mpsc::UnboundedReceiver<Queued>) -> Vec<GuildWrite> {
        std::iter::from_fn(|| writes.try_recv().ok())
            .filter_map(|q| match q {
                Queued::Write(w) => Some(w),
                Queued::Create(..) | Queued::Flush(_) => None,
            })
            .collect()
    }

    /// A guild 1 "Knights" with master 1 and members 2 and 3 (rank 9), and its players online.
    fn knights() -> (Guilds, mpsc::UnboundedReceiver<Queued>, Vec<Client>) {
        let (writer, writes) = mpsc::unbounded_channel();
        let member = |id: u32, rank: u8| StoredMember { id, name: format!("P{id}"), level: 20, job: 1, rank };
        let stored = StoredGuild {
            id: 1,
            name: "Knights".into(),
            country: 0,
            remark: String::new(),
            members: vec![member(1, MASTER), member(2, NEW_MEMBER), member(3, NEW_MEMBER)],
            house: Default::default(),
            bank: Vec::new(),
        };
        let mut g = Guilds::load(vec![stored], writer);
        let clients = (1..=3).map(|id| enter(&mut g, id, 20)).collect();
        (g, writes, clients)
    }

    #[test]
    fn a_renamed_member_shows_its_new_name() {
        // docs/original/small-items.md deviation 5.
        let (mut g, _, _) = knights();
        g.renamed(1, b"Lord");
        g.renamed(3, b"Third");
        g.renamed(9, b"Nobody");
        let guild = g.guild_of(1).unwrap();
        assert_eq!(guild.members.iter().map(|m| m.name.as_slice()).collect::<Vec<_>>(), [&b"Lord"[..], b"P2", b"Third"]);
        assert_eq!(guild.master_name(), b"Lord");
    }

    #[test]
    fn house_purchase_upgrades_and_etin() {
        let rules = HouseRules::default();
        let (mut g, mut writes, mut c) = knights();
        // Not the master, then too little money, then bought.
        assert_eq!(g.house_purchase(2, 99_999_999, &rules), Err(gh::buy::NOT_MASTER));
        assert_eq!(g.house_purchase(1, 49_999_999, &rules), Err(gh::buy::NO_MONEY));
        assert_eq!(g.house_purchase(1, 50_000_000, &rules), Ok(1));
        g.house_bought(1, &rules);
        assert_eq!(g.house_purchase(1, 50_000_000, &rules), Err(gh::buy::ALREADY));
        assert!(written(&mut writes).is_empty(), "the purchase's transaction wrote it, nothing is queued");
        let house = g.house(1).unwrap();
        assert!(house.bought && !house.enabled(), "bought, not open until a ranking");

        // An Etin return: the whole guild sees the new Etin.
        for client in &mut c {
            client.drain();
        }
        g.return_etin(1, 3, 0);
        g.return_etin(9, 3, 100);
        assert_eq!(g.house(1).unwrap().etin, 0, "nothing for no Etin or a guild that is gone");
        g.return_etin(1, 3, 4000);
        assert!(written(&mut writes).is_empty(), "the return's transaction wrote it, nothing is queued");
        assert_eq!((g.house(1).unwrap().etin, g.house(1).unwrap().etin_returns), (4000, 1));
        for client in &mut c {
            assert_eq!(client.drain(), [gp::etin(4000)]);
        }

        // Upgrades: rank 9 is ignored; refused while not open; done once open, to everyone.
        g.upgrade(3, gh::KEY_GATEKEEPER, 1, &rules);
        assert!(c[2].drain().is_empty());
        g.upgrade(1, gh::KEY_GATEKEEPER, 1, &rules);
        assert_eq!(c[0].drain(), [gp::npc_upgrade(gh::upgrade::FAILED, gh::KEY_GATEKEEPER, 1, 0)]);
        let house = &mut g.guilds.get_mut(&1).unwrap().house;
        (house.ranking, house.use_house) = (5, gh::HOUSE_OPEN);
        g.upgrade(1, gh::KEY_GATEKEEPER, 1, &rules);
        for client in &mut c {
            assert_eq!(client.drain(), [gp::npc_upgrade(gh::upgrade::DONE, gh::KEY_GATEKEEPER, 1, 0), gp::etin(3000)]);
        }
        assert_eq!(written(&mut writes), [GuildWrite::Upgrade { guild: 1, slot: 0, key: 2, level: 1, etin: 3000, keep_etin: 2200 }]);

        // The levels come with the burst and to a new member.
        let burst = g.burst(2, 0);
        assert_eq!(burst.packets.last(), gp::npc_levels(&[(2, 1, 0)]).as_ref());
        g.etin(2);
        assert_eq!(c[1].drain(), [gp::etin(3000)]);
    }

    #[test]
    fn the_ranking_battle_counts_points_and_entries_then_ranks() {
        let rules = HouseRules::default();
        let (mut g, mut writes, mut c) = knights();
        let tag: Tag = 7;
        // One entry per character and battle; the guild's count grows once each.
        assert!(!g.entered(2, tag) && g.count_entry(2, tag) && g.entered(2, tag));
        assert!(!g.count_entry(2, tag) && g.count_entry(3, tag));
        assert!(!g.entered(2, tag + 1), "another battle");
        assert_eq!(g.house(1).unwrap().battle_entries, 2);
        // Points: the country's best follows the first guild above it, only when strictly above.
        assert_eq!(g.add_battle_points(1, 40), Some(BattleScore { points: 40, best_points: 40, best_guild: 1 }));
        assert_eq!(g.add_battle_points(1, 60), Some(BattleScore { points: 100, best_points: 100, best_guild: 1 }));
        assert_eq!(g.add_battle_points(9, 1), None);
        // Bought house, Etin above the keep Etin, one return.
        g.house_bought(1, &rules);
        g.return_etin(1, 1, 3000);
        for client in &mut c {
            client.drain();
        }
        written(&mut writes);
        let placed = g.apply_ranking();
        // 100 points + min(1 return + 2 entries x 25, 50) = 150.
        assert_eq!(placed, [Placed { guild: 1, score: 150, place: 1, country: 0 }]);
        let house = g.house(1).unwrap();
        assert_eq!((house.ranking, house.points, house.use_house, house.etin), (1, 150, gh::HOUSE_OPEN, 1000), "keep Etin of 2000 taken");
        assert_eq!((house.battle_points, house.battle_entries, house.etin_returns), (0, 0, 0));
        assert!(house.enabled());
        for client in &mut c {
            assert_eq!(client.drain(), [gp::ranking(&[(1, 150, 1)]), gp::etin(1000)]);
        }
        assert_eq!(
            written(&mut writes),
            [GuildWrite::Ranking { rows: vec![RankingRow { guild: 1, points: 150, etin: 1000, ranking: 1, use_house: gh::HOUSE_OPEN }] }]
        );
        assert_eq!(g.guilds[&1].unit().ranking, 1);
        assert_eq!(g.guilds[&1].unit().points, 150);
        // The best pair was reset, so a first point counts again.
        assert_eq!(g.add_battle_points(1, 1).unwrap().best_points, 1);

        // A second update without points: unranked and closed, the Etin stays.
        g.add_battle_points(1, 0);
        g.guilds.get_mut(&1).unwrap().house.battle_points = 0;
        assert!(g.apply_ranking().is_empty());
        let house = g.house(1).unwrap();
        assert_eq!((house.ranking, house.points, house.use_house, house.etin), (31, 0, gh::HOUSE_NOT_OPEN, 1000));
        assert!(!house.enabled());
    }

    #[test]
    fn a_ranked_guild_short_of_the_keep_etin_has_its_house_closed() {
        let rules = HouseRules::default();
        let (mut g, _writes, _c) = knights();
        g.house_bought(1, &rules);
        g.return_etin(1, 1, 1999);
        g.add_battle_points(1, 10);
        assert_eq!(g.apply_ranking().len(), 1);
        let house = g.house(1).unwrap();
        assert_eq!((house.ranking, house.use_house, house.etin), (1, gh::HOUSE_CLOSED, 1999));
    }

    #[test]
    fn names_and_remarks_are_checked() {
        assert!(valid_name(b"Knights of Old"));
        for bad in [&b"K"[..], b" Knights", b"Knights ", b"O'Neil", b"<<001c01x", b"\x01ab", b"abcdefghijklmnopqrstuvwxy"] {
            assert!(!valid_name(bad), "{bad:?}");
        }
        assert!(valid_remark(b""));
        assert!(valid_remark(b"We meet at <<001c01x"));
        assert!(!valid_remark(b"It's"));
        assert!(!valid_remark(b"<<999c01"));
    }

    #[test]
    fn creation_asks_everyone_and_the_requester_pays() {
        let (mut g, mut writes) = world();
        let mut a = enter(&mut g, 1, 11);
        let mut b = enter(&mut g, 2, 5);
        let now = Instant::now();
        let party = [(1, true), (2, true)];
        g.create_request(1, &party, 2, b"Knights", b"Hello", now);
        assert_eq!(a.opcodes(), [gp::OP_CREATE_AGREE], "the requester is asked too");
        assert_eq!(b.opcodes(), [gp::OP_CREATE_AGREE]);
        g.create_answer(2, true);
        assert!(a.events.try_recv().is_err());
        g.create_answer(1, true);
        assert_eq!(a.events.try_recv().ok(), Some(ZoneEvent::GuildAgreed));
        let mut outcome = g.create_write(1, 1).expect("agreed");
        let Some(Queued::Create(new, reply)) = writes.try_recv().ok() else { panic!("no creation queued") };
        assert_eq!(new.members, [(1, MASTER), (2, NEW_MEMBER)]);
        assert_eq!((new.payer, new.cost, new.name.as_str()), (1, CREATE_COST, "Knights"));
        reply.send(Ok(GuildCreated::Created(7))).unwrap();
        assert_eq!(outcome.try_recv().unwrap().unwrap(), GuildCreated::Created(7));
        let after = g.create_done(1, 7, 4000, now);
        assert_eq!(after.looks, [(1, 7, b"Knights".to_vec()), (2, 7, b"Knights".to_vec())]);
        let mine = a.drain();
        assert_eq!(
            mine.iter().map(|p| opcode(p)).collect::<Vec<_>>(),
            [gp::OP_CREATE, w::OP_SET_MONEY, gp::OP_MEMBERS_ONLINE, gp::OP_GUILD_ADD]
        );
        assert_eq!(payload(&mine[0])[5], MASTER);
        assert_eq!(payload(&mine[2])[0], 2, "both online");
        let theirs = b.drain();
        assert_eq!(payload(&theirs[0])[5], NEW_MEMBER);
        assert_eq!(g.looks(2), (7, b"Knights".to_vec()));
        assert!(g.online[&2].penalised(now), "the penalty starts with a join");
    }

    #[test]
    fn an_agreed_creation_nobody_took_up_lapses_and_ends_with_its_founder() {
        let agreed = || {
            let (mut g, writes) = world();
            let (a, b) = (enter(&mut g, 1, 11), enter(&mut g, 2, 5));
            let now = Instant::now();
            g.create_request(1, &[(1, true), (2, true)], 2, b"Knights", b"Hello", now);
            g.create_answer(2, true);
            g.create_answer(1, true);
            let (mut a, mut b) = (a, b);
            drop((a.drain(), b.drain()));
            (g, writes, a, b)
        };
        // Audit R27: the founder's connection never takes it up; 30 s after the last yes it lapses for everyone.
        let (mut g, _writes, mut a, mut b) = agreed();
        assert!(g.in_agreement(2));
        g.expire(Instant::now() + AGREED_TIME - Duration::from_secs(1));
        assert!(g.in_agreement(1) && g.in_agreement(2), "not yet");
        g.expire(Instant::now() + AGREED_TIME + Duration::from_secs(1));
        assert!(!g.in_agreement(1) && !g.in_agreement(2), "the guild actions are free again");
        assert_eq!(a.drain(), [gp::create_failed(gp::CREATE_FAILED, b"Knights")]);
        assert_eq!(b.drain(), [gp::create_failed(gp::CREATE_FAILED, b"Knights")]);
        // Another member leaving the world does not end it; the founder leaving does, at once.
        let (mut g, _writes, _a, _b) = agreed();
        g.disconnect(2);
        assert!(g.in_agreement(1), "an agreed member leaving is not the founder");
        let (mut g, _writes, mut a, mut b) = agreed();
        g.disconnect(1);
        assert!(!g.in_agreement(1) && !g.in_agreement(2));
        assert!(a.drain().is_empty(), "the founder is gone");
        assert!(b.drain().contains(&gp::create_failed(gp::CREATE_MEMBERS, b"Knights")));
        // A creation on its way to the database does not lapse by the clock; its outcome ends it.
        let (mut g, mut writes, _a, _b) = agreed();
        let _outcome = g.create_write(1, 1).expect("agreed");
        assert!(matches!(writes.try_recv(), Ok(Queued::Create(..))));
        g.expire(Instant::now() + AGREED_TIME + Duration::from_secs(60));
        assert!(g.in_agreement(1));
    }

    #[test]
    fn creation_checks_in_the_original_order() {
        let (mut g, _writes) = world();
        let mut a = enter(&mut g, 1, 10);
        let _b = enter(&mut g, 2, 30);
        let now = Instant::now();
        let code = |c: &mut Client| payload(&c.drain()[0])[0];
        g.create_request(1, &[(1, true), (2, true)], 2, b"Knights", b"", now);
        assert_eq!(code(&mut a), gp::CREATE_LEVEL);
        g.online.get_mut(&1).unwrap().level = 11;
        g.create_request(1, &[(1, true)], 2, b"Knights", b"", now);
        assert_eq!(code(&mut a), gp::CREATE_MEMBERS, "the party size");
        g.create_request(1, &[(1, true), (2, false)], 2, b"Knights", b"", now);
        assert_eq!(code(&mut a), gp::CREATE_MEMBERS, "a member elsewhere");
        g.online.get_mut(&2).unwrap().penalty_until = Some(now + Duration::from_secs(5));
        g.create_request(1, &[(1, true), (2, true)], 2, b"Knights", b"", now);
        assert_eq!(code(&mut a), gp::CREATE_PENALTY);
        g.online.get_mut(&2).unwrap().grow = EASY;
        g.create_request(1, &[(1, true), (2, true)], 2, b"Knights", b"", now);
        assert_eq!(code(&mut a), gp::CREATE_MODE);
        g.create_request(1, &[], 1, b"K'", b"", now);
        assert_eq!(code(&mut a), gp::CREATE_NAME);
        g.create_request(1, &[], 1, b"Knights", b"", now);
        assert_eq!(a.opcodes(), [gp::OP_CREATE_AGREE], "alone with a party size of 1");
        g.create_request(1, &[], 1, b"Other", b"", now);
        assert_eq!(code(&mut a), gp::CREATE_MEMBERS, "already in an agreement");
    }

    #[test]
    fn a_refusal_a_departure_or_the_clock_ends_an_agreement() {
        let (mut g, _writes) = world();
        let mut a = enter(&mut g, 1, 11);
        let mut b = enter(&mut g, 2, 11);
        let now = Instant::now();
        let party = [(1, true), (2, true)];
        g.create_request(1, &party, 2, b"Knights", b"", now);
        g.create_answer(2, false);
        a.drain();
        assert_eq!(payload(&b.drain()[1])[0], gp::CREATE_REJECTED);
        g.create_request(1, &party, 2, b"Knights", b"", now);
        g.disconnect(2);
        assert_eq!(payload(&a.drain()[1])[0], gp::CREATE_MEMBERS);
        let _b = enter(&mut g, 2, 11);
        g.create_request(1, &party, 2, b"Knights", b"", now);
        g.expire(now + ANSWER_TIME);
        assert_eq!(payload(&a.drain()[1])[0], gp::CREATE_FAILED);
        assert!(g.agreements.is_empty());
    }

    #[test]
    fn invitation_joins_with_rank_nine() {
        let (mut g, mut writes, mut c) = knights();
        let mut d = enter(&mut g, 4, 15);
        let now = Instant::now();
        c.iter_mut().for_each(|c| drop(c.drain()));
        g.invite(2, 4, true, now);
        assert_eq!(c[1].opcodes(), [gp::OP_INVITE_ANSWER], "a rank 9 member may not invite");
        g.invite(1, 4, false, now);
        assert_eq!(c[0].opcodes(), [gp::OP_INVITE_ANSWER], "not on the inviter's map");
        g.invite(1, 4, true, now);
        assert_eq!(d.opcodes(), [gp::OP_INVITE]);
        let after = g.invite_answer(4, true, |_| true, now);
        assert_eq!(after.looks, [(4, 1, b"Knights".to_vec())]);
        assert_eq!(d.opcodes(), [gp::OP_JOINED, gp::OP_MEMBER_ADD, gp::OP_MEMBERS_ONLINE]);
        assert_eq!(c[0].opcodes(), [gp::OP_MEMBER_ADD, gp::OP_INVITE_ANSWER]);
        assert_eq!(c[2].opcodes(), [gp::OP_MEMBER_ADD]);
        assert_eq!(written(&mut writes), [GuildWrite::Add { guild: 1, character: 4, rank: NEW_MEMBER }]);
        assert_eq!(g.rank_of(4), Some(NEW_MEMBER));
    }

    #[test]
    fn join_requests_go_to_the_officers() {
        let (mut g, mut writes, mut c) = knights();
        let mut d = enter(&mut g, 4, 15);
        let now = Instant::now();
        c.iter_mut().for_each(|c| drop(c.drain()));
        g.join_request(4, 1, now);
        assert_eq!(d.opcodes(), [gp::OP_JOIN_REQUEST]);
        assert_eq!(c[0].opcodes(), [gp::OP_REQUEST_ADD], "the master");
        assert!(c[1].opcodes().is_empty(), "not a rank 9 member");
        assert_eq!(g.join_answer(2, true, 4, now), Aftermath::default(), "rank 9 cannot answer");
        let after = g.join_answer(1, true, 4, now);
        assert_eq!(after.looks.len(), 1);
        assert_eq!(c[0].opcodes(), [gp::OP_REQUEST_REMOVE, gp::OP_MEMBER_ADD]);
        assert_eq!(d.opcodes(), [gp::OP_MEMBER_ADD, gp::OP_JOINED, gp::OP_MEMBERS_ONLINE]);
        assert_eq!(written(&mut writes), [GuildWrite::Add { guild: 1, character: 4, rank: REQUEST }, GuildWrite::Accept { character: 4 }]);
        // A penalised player is told so.
        let mut e = enter(&mut g, 5, 15);
        g.online.get_mut(&5).unwrap().penalty_until = Some(now + JOIN_PENALTY);
        g.join_request(5, 1, now);
        assert_eq!(payload(&e.drain()[0])[0], gp::REQUEST_PENALTY);
    }

    #[test]
    fn a_remark_that_changes_nothing_is_told_to_the_guild_but_not_written() {
        let (mut g, mut writes, mut c) = knights();
        g.set_remark(1, b"We meet at noon");
        assert_eq!(written(&mut writes), [GuildWrite::Remark { guild: 1, remark: "We meet at noon".into() }]);
        c.iter_mut().for_each(|c| drop(c.drain()));
        g.set_remark(1, b"We meet at noon");
        assert!(written(&mut writes).is_empty(), "not written again");
        assert!(c.iter_mut().all(|c| !c.opcodes().is_empty()), "the guild is told");
    }

    #[test]
    fn operator_commands_change_grades_ranking_and_points() {
        let (mut g, mut writes, mut c) = knights();
        c.iter_mut().for_each(|c| drop(c.drain()));
        assert_eq!(g.guild_id_by_name(b"KNIGHTS"), Some(1));
        assert_eq!(g.guild_id_by_name(b"Nobody"), None);
        assert!(!g.admin_grade(b"Knights", b"P2", 1), "one master per guild");
        assert!(!g.admin_grade(b"Knights", b"P2", 9), "the rank it has");
        assert!(!g.admin_grade(b"Knights", b"P2", 10) && !g.admin_grade(b"Knights", b"P2", 0));
        assert!(!g.admin_grade(b"Knights", b"P1", 5), "the master keeps its rank");
        assert!(!g.admin_grade(b"Knights", b"Nobody", 5) && !g.admin_grade(b"Nobody", b"P2", 5));
        assert!(written(&mut writes).is_empty() && c.iter_mut().all(|c| c.opcodes().is_empty()));

        assert!(g.admin_grade(b"Knights", b"p2", 3));
        assert_eq!(g.rank_of(2), Some(3));
        assert_eq!(written(&mut writes), [GuildWrite::Rank { character: 2, rank: 3 }]);
        for client in &mut c {
            let packets = client.drain();
            assert_eq!(packets, [gp::member_state(gp::STATE_RANK + 3, 2)]);
        }
        // A demotion to 9 sends 200 + 9.
        assert!(g.admin_grade(b"Knights", b"P2", 9));
        assert_eq!(payload(&c[0].drain()[0])[..1], [209]);

        // Officers: the master and six more fill the 7 places; the next promotion into 2 is refused, one into 5 is not.
        for id in 4..=9 {
            g.guilds.get_mut(&1).unwrap().members.push(Member {
                id,
                name: format!("P{id}").into_bytes(),
                level: 20,
                job: 1,
                rank: OFFICER,
            });
        }
        assert_eq!(g.guilds[&1].officers(), 7);
        assert!(!g.admin_grade(b"Knights", b"P2", 2), "seven officers already");
        assert!(g.admin_grade(b"Knights", b"P4", 5), "an officer may go down");
        assert!(g.admin_grade(b"Knights", b"P2", 2), "a place is free again");
        written(&mut writes);

        // Ranking and points.
        assert!(!g.admin_ranking(b"Knights", 0) && !g.admin_ranking(b"Knights", 32) && !g.admin_ranking(b"Nobody", 5));
        assert!(g.admin_ranking(b"Knights", 31));
        let house = g.house(1).unwrap();
        assert_eq!((house.ranking, house.use_house), (31, 1));
        assert!(g.admin_points(b"Knights", 100) && !g.admin_points(b"Nobody", 100));
        assert_eq!(g.house(1).unwrap().points, 100);
        assert_eq!(
            written(&mut writes),
            [GuildWrite::Standing { guild: 1, ranking: 31, use_house: 1 }, GuildWrite::Points { guild: 1, points: 100 }]
        );
    }

    #[test]
    fn leaving_kicking_and_ranks() {
        let (mut g, mut writes, mut c) = knights();
        c.iter_mut().for_each(|c| drop(c.drain()));
        g.leave(1);
        assert_eq!(payload(&c[0].drain()[0])[0], 0, "the master stays while others remain");
        g.change_rank(1, 0, 2);
        assert_eq!(g.rank_of(2), Some(8));
        g.change_rank(2, 0, 3);
        assert_eq!(g.rank_of(3), Some(NEW_MEMBER), "rank 8 is not an officer");
        for _ in 0..6 {
            g.change_rank(1, 0, 2);
        }
        assert_eq!(g.rank_of(2), Some(OFFICER), "never rank 1");
        g.change_rank(2, 0, 3);
        assert_eq!(g.rank_of(3), Some(8), "rank 2 promotes up to 3");
        c.iter_mut().for_each(|c| drop(c.drain()));
        g.kick(2, 1);
        assert_eq!(payload(&c[1].drain()[0])[0], 0, "not the master");
        let after = g.kick(2, 3);
        assert_eq!(after.looks, [(3, 0, Vec::new())]);
        assert_eq!(c[2].opcodes(), [gp::OP_KICK]);
        assert_eq!(c[0].opcodes(), [gp::OP_MEMBER_STATE]);
        let after = g.leave(2);
        assert_eq!(after.looks, [(2, 0, Vec::new())]);
        written(&mut writes);
        let after = g.leave(1);
        assert_eq!(after.looks, [(1, 0, Vec::new())]);
        assert_eq!(written(&mut writes), [GuildWrite::Remove { character: 1 }, GuildWrite::Disband { guild: 1 }]);
        assert!(g.is_empty(), "the last one out closes it");
    }

    #[test]
    fn guild_events_are_recorded_under_the_member_who_goes() {
        use crate::game_log::Entry;
        let (mut g, _writes, mut c) = knights();
        let (log, entries) = GameLog::memory(false);
        g.set_log(log);
        c.iter_mut().for_each(|c| drop(c.drain()));
        // A member in the world: its connection gets the record to complete with its header (an expulsion is 179 with V2 1).
        g.kick(1, 3);
        let records: Vec<_> = std::iter::from_fn(|| c[2].events.try_recv().ok())
            .filter_map(|e| if let ZoneEvent::Record(r) = e { Some(r) } else { None })
            .collect();
        assert_eq!(records.len(), 1);
        let row = &records[0];
        assert_eq!((row.action, row.values[..2].to_vec(), row.texts[0].as_deref()), (179, vec![Some(1), Some(1)], Some("Knights")));
        // One who is out of the world (a deleted character) is written at once with its id, name and level only (V2 2).
        g.disconnect(2);
        g.deleted(2);
        let entries = entries.guard();
        let [Entry::Action(row)] = &entries[..] else { panic!("one record: {entries:?}") };
        assert_eq!(
            (row.action, row.values[1], row.header.character_id, row.header.character_name.as_str(), row.header.level),
            (179, Some(2), 2, "P2", 20)
        );
    }

    #[test]
    fn disbanding_needs_every_officer() {
        let (mut g, mut writes, mut c) = knights();
        let _d = enter(&mut g, 4, 20);
        g.guilds.get_mut(&1).unwrap().members.push(Member { id: 4, name: b"P4".to_vec(), level: 20, job: 1, rank: OFFICER });
        g.of.insert(4, 1);
        for _ in 0..7 {
            g.change_rank(1, 0, 2);
        }
        c.iter_mut().for_each(|c| drop(c.drain()));
        let now = Instant::now();
        g.disband_request(1, &[2], now);
        assert_eq!(payload(&c[0].drain()[0])[0], gp::DISBAND_MEMBERS, "two officers needed");
        g.disband_request(1, &[2, 4, 3], now);
        assert_eq!(c[0].opcodes(), [gp::OP_DISBAND_AGREE]);
        assert!(c[2].opcodes().is_empty(), "rank 9 is not asked");
        g.disband_answer(1, true);
        g.disband_answer(2, true);
        assert!(!g.is_empty());
        written(&mut writes);
        let after = g.disband_answer(4, true);
        assert!(g.is_empty());
        assert_eq!(after.looks.len(), 4);
        assert_eq!(c[2].opcodes(), [gp::OP_MEMBER_STATE, gp::OP_GUILD_REMOVED]);
        assert_eq!(written(&mut writes), [GuildWrite::Disband { guild: 1 }]);
    }

    #[test]
    fn a_small_guild_disbands_at_once_and_a_deleted_master_hands_on() {
        let (mut g, mut writes, mut c) = knights();
        g.disconnect(1);
        c.iter_mut().for_each(|c| drop(c.drain()));
        g.deleted(1);
        assert_eq!(g.rank_of(2), Some(MASTER), "the longest in the guild among equals");
        assert_eq!(c[1].opcodes(), [gp::OP_MEMBER_STATE, gp::OP_MEMBER_STATE]);
        assert_eq!(written(&mut writes), [GuildWrite::Remove { character: 1 }, GuildWrite::Rank { character: 2, rank: MASTER }]);
        let after = g.disband_request(2, &[], Instant::now());
        assert!(g.is_empty(), "two members: no agreement needed");
        assert_eq!(after.looks.len(), 2);
    }

    #[test]
    fn the_burst_lists_guilds_members_and_requests() {
        let (mut g, _writes, _c) = knights();
        g.disconnect(3);
        let _d = enter(&mut g, 4, 15);
        g.join_request(4, 1, Instant::now());
        let burst = g.burst(1, 0);
        assert_eq!(burst.name, b"Knights");
        let ops: Vec<u16> = burst.packets.iter().map(|p| opcode(p)).collect();
        assert_eq!(
            ops,
            [gp::OP_LIST_START, gp::OP_GUILDS, gp::OP_LIST_END, gp::OP_MEMBERS_ONLINE, gp::OP_MEMBERS_OFFLINE, gp::OP_REQUESTS]
        );
        assert_eq!(payload(&burst.packets[3])[0], 2);
        let member = g.burst(2, 0);
        assert!(member.packets.iter().all(|p| opcode(p) != gp::OP_REQUESTS), "requests go to officers only");
        let other = g.burst(9, 1);
        assert_eq!(other.packets.len(), 2, "no guild of that country");
        assert!(other.name.is_empty());
    }

    // The bank (`docs/original/guild-bank.md`).

    fn stack(bag: u8, slot: u8, item_type: u8, count: u8) -> crate::inventory::ItemUnit {
        crate::inventory::ItemUnit { bag, slot, item_type, type_id: 1, count, ..Default::default() }
    }

    fn bank_defs() -> ItemDefs {
        let def = |item_type, max_count, sell| crate::inventory::ItemDef { item_type, type_id: 1, max_count, sell, ..Default::default() };
        ItemDefs::new([def(25, 10, 100), def(5, 1, 100)])
    }

    /// `knights()` ranked 5 with player 2 a rank-3 member, and the given items in its bank.
    fn bank_knights(items: &[crate::inventory::ItemUnit]) -> (Guilds, Vec<Client>) {
        let (mut g, _writes, c) = knights();
        let guild = g.guilds.get_mut(&1).unwrap();
        guild.house.ranking = 5;
        guild.bank = GuildBank::new(items);
        guild.member_mut(2).unwrap().rank = 3;
        (g, c)
    }

    #[test]
    fn the_burst_and_a_join_bring_the_bank_before_the_npc_levels() {
        let (mut g, mut c) = bank_knights(&[stack(255, 9, 25, 3), stack(255, 2, 5, 1)]);
        g.guilds.get_mut(&1).unwrap().house.slots[0] = (2, 1);
        let ops = |packets: &[Vec<u8>]| packets.iter().map(|p| opcode(p)).collect::<Vec<_>>();
        let burst = g.burst(1, 0);
        assert_eq!(ops(&burst.packets)[burst.packets.len() - 2..], [gp::OP_BANK_LIST, gp::OP_NPC_LEVELS]);
        let list = payload(&burst.packets[burst.packets.len() - 2]);
        assert_eq!((list[0], list[1], list[34]), (2, 2, 9), "two items, by slot");
        g.guilds.get_mut(&1).unwrap().bank = GuildBank::default();
        assert!(ops(&g.burst(1, 0).packets).iter().all(|&o| o != gp::OP_BANK_LIST), "nothing for an empty bank");

        g.guilds.get_mut(&1).unwrap().bank = GuildBank::new(&[stack(255, 0, 25, 1)]);
        let mut newcomer = enter(&mut g, 4, 20);
        c.iter_mut().for_each(|c| drop(c.drain()));
        let member = Member { id: 4, name: b"P4".to_vec(), level: 20, job: 1, rank: NEW_MEMBER };
        g.join(1, member, Instant::now(), &mut Aftermath::default());
        let got = newcomer.opcodes();
        let (bank, levels) =
            (got.iter().position(|&o| o == gp::OP_BANK_LIST).unwrap(), got.iter().position(|&o| o == gp::OP_NPC_LEVELS).unwrap());
        assert!(bank < levels);
    }

    #[test]
    fn a_deposit_tells_the_others_and_a_rearrange_tells_everyone() {
        let (mut g, mut c) = bank_knights(&[stack(255, 9, 5, 1)]);
        let (defs, rules) = (bank_defs(), HouseRules::default());
        let mut inv = Inventory::new(&[stack(1, 0, 25, 4), stack(1, 1, 25, 6)]);
        c.iter_mut().for_each(|c| drop(c.drain()));

        let (guild, planned) = g.bank_plan(1, &inv, &defs, &rules, 0, (1, 0), (255, 3)).unwrap();
        let planned = planned.unwrap();
        let answer = g.bank_apply(guild, 1, &mut inv, &planned, (1, 0), (255, 3), 0).unwrap();
        assert_eq!(answer.len() - 2, 70);
        assert!(inv.get(1, 0).is_none());
        assert_eq!(g.guild_of(1).unwrap().bank.get(3).map(|i| (i.bag, i.slot, i.count)), Some((255, 3, 4)));
        assert!(c[0].drain().is_empty(), "not to the mover");
        for other in &mut c[1..] {
            let got = other.drain();
            assert_eq!(got.iter().map(|p| opcode(p)).collect::<Vec<_>>(), [gp::OP_BANK_IN]);
            assert_eq!(payload(&got[0])[0], 3);
            assert_eq!(payload(&got[0])[33..], 1u32.to_le_bytes());
        }

        // A swap onto a taken bank slot: the others get the item that came out, then the one that went in.
        let (guild, planned) = g.bank_plan(1, &inv, &defs, &rules, 0, (1, 1), (255, 9)).unwrap();
        let answer = g.bank_apply(guild, 1, &mut inv, &planned.unwrap(), (1, 1), (255, 9), 0).unwrap();
        assert_eq!(answer.len() - 2, 70);
        assert_eq!(inv.get(1, 1).map(|i| i.item_type), Some(5));
        assert_eq!(c[2].opcodes(), [gp::OP_BANK_OUT, gp::OP_BANK_IN]);
        c.iter_mut().for_each(|c| drop(c.drain()));

        // Out: the others get the item that left; the fee comes with the answer's money.
        let (guild, planned) = g.bank_plan(1, &inv, &defs, &rules, 500, (255, 3), (1, 7)).unwrap();
        let planned = planned.unwrap();
        assert_eq!(planned.fee, 20);
        let answer = g.bank_apply(guild, 1, &mut inv, &planned, (255, 3), (1, 7), 480).unwrap();
        assert_eq!(answer.len() - 2, 74);
        assert_eq!(answer[answer.len() - 4..], 480u32.to_le_bytes());
        assert_eq!(c[1].opcodes(), [gp::OP_BANK_OUT]);
        c.iter_mut().for_each(|c| drop(c.drain()));

        // Inside the bank: a rank-3 member is refused; the master's move goes as 0x0204 to everyone, the mover included.
        let (_, refused) = g.bank_plan(2, &inv, &defs, &rules, 0, (255, 9), (255, 10)).unwrap();
        assert_eq!(refused, Err(Some((4, 2))));
        let (guild, planned) = g.bank_plan(1, &inv, &defs, &rules, 0, (255, 9), (255, 10)).unwrap();
        assert_eq!(g.bank_apply(guild, 1, &mut inv, &planned.unwrap(), (255, 9), (255, 10), 0), None);
        for client in &mut c {
            assert_eq!(client.opcodes(), [w::OP_ITEM_MOVE]);
        }
        assert!(g.bank_plan(9, &inv, &defs, &rules, 0, (255, 10), (255, 11)).is_none(), "no guild, no plan");
    }

    #[test]
    fn a_deposit_for_a_guild_disbanded_meanwhile_still_leaves_the_bag() {
        let (mut g, _c) = bank_knights(&[]);
        let (defs, rules) = (bank_defs(), HouseRules::default());
        let mut inv = Inventory::new(&[stack(1, 0, 25, 4)]);
        let (guild, planned) = g.bank_plan(1, &inv, &defs, &rules, 0, (1, 0), (255, 3)).unwrap();
        g.fire(guild, 1, &mut Aftermath::default());
        let answer = g.bank_apply(guild, 1, &mut inv, &planned.unwrap(), (1, 0), (255, 3), 0).unwrap();
        assert!(inv.get(1, 0).is_none(), "the database has it");
        assert_eq!(answer.len() - 2, 70);
    }

    #[test]
    fn a_deposit_made_during_the_disband_vote_keeps_the_guild() {
        let (mut g, mut c) = bank_knights(&[]);
        for rank in [2, 3] {
            g.guilds.get_mut(&1).unwrap().member_mut(rank).unwrap().rank = OFFICER;
        }
        let guild = g.guilds.get_mut(&1).unwrap();
        guild.members.push(Member { id: 4, name: b"P4".to_vec(), level: 20, job: 1, rank: OFFICER });
        g.of.insert(4, 1);
        let _d = enter(&mut g, 4, 20);
        c.iter_mut().for_each(|c| drop(c.drain()));
        g.disband_request(1, &[2, 3, 4], Instant::now());
        assert_eq!(c[0].opcodes(), [gp::OP_DISBAND_AGREE]);
        for who in [1, 2] {
            g.disband_answer(who, true);
        }
        // The deposit lands before the last yes.
        g.guilds.get_mut(&1).unwrap().bank = GuildBank::new(&[stack(255, 0, 25, 1)]);
        c.iter_mut().for_each(|c| drop(c.drain()));
        g.disband_answer(3, true);
        g.disband_answer(4, true);
        assert!(!g.is_empty(), "the guild stays with its bank");
        assert_eq!(c[0].drain(), [gp::disband_result(gp::DISBAND_FAILED)]);
        assert!(!g.in_agreement(1), "and the agreement is over");
    }

    #[test]
    fn a_guild_whose_bank_holds_items_is_not_disbanded() {
        let (mut g, mut c) = bank_knights(&[stack(255, 0, 25, 1)]);
        g.guilds.get_mut(&1).unwrap().members.truncate(2);
        c.iter_mut().for_each(|c| drop(c.drain()));
        g.disband_request(1, &[], Instant::now());
        assert!(!g.is_empty(), "two members would be disbanded at once");
        assert_eq!(c[0].drain(), [gp::disband_result(gp::DISBAND_FAILED)]);
        assert!(c[1].drain().is_empty());
        g.guilds.get_mut(&1).unwrap().bank = GuildBank::default();
        g.disband_request(1, &[], Instant::now());
        assert!(g.is_empty());
    }
}
