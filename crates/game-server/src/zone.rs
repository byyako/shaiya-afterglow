//! The players in each map's zone and what they see of each other: cells, view and range sends as the original's
//! `CZone`/`CCell` do them (`docs/original/world.md` section 5). Players, monsters (`docs/original/mobs.md`) and NPCs
//! (`docs/original/maps-npcs.md` section 5); ground items come later.
//!
//! Locks: players (`maps`) may be held while taking monsters (`mobs`), never the other way round; NPCs (`npcs`) after both, before
//! fields and the ground; the ground last.
//!
//! Packets for other players go into their outbox and are written by their own connection; functions return the packets
//! meant for the acting player, so the caller sends them in order with its other replies.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use shaiya_proto::combat as cp;
use shaiya_proto::ground as gp;
use shaiya_proto::skill as sk;
use shaiya_proto::world::{self as w, Move, NpcEnter, UserEnter, UserShape};
use tokio::sync::mpsc::UnboundedSender;

use crate::buffs::{self, Applied, MobDebuff, PlayerBuffs};
use crate::combat::{self, Fighter};
use crate::drops::{self, DropTables, GOLD_TYPE, Loot};
use crate::fight::{self, Chase, Homeward};
use crate::inventory::{ItemDefs, real_type};
use crate::lapis::OptionSkills;
use crate::mobs::{self, Mob, MobDef, MobDefs, MobMap, MobState, Moving, Origin};
use crate::pacing;
use crate::skills::{self, KillBoost, SkillDef};
use crate::sync::{Relock, RelockRw};
use crate::world::{MapTerrain, World};

mod admin;
mod boss;
mod duel;
mod friends;
mod guild;
mod guild_war;
mod instance;
mod items;
mod lua;
mod mob_skills;
mod monsters;
mod move_tick;
mod my_shop;
mod npc;
mod obelisk;
mod party;
mod pvp;
mod ranking;
mod script;
mod scripted;
mod trade;
mod vehicle;

pub use admin::{AdminOrder, BanKind, Bans, CureKind, usable_def};
pub use script::{SPAWNS_ALIVE, SPAWNS_PER_CALL};
pub use items::Member;
use monsters::{
    fight_turn, find_target, go_home, mob_enter, mob_strike, mobs_in_view, paced_next, retarget, set_target, stop_moving, take_target,
    targetable, wake, walk_mob,
};
pub use party::Invitee;
pub use pvp::{Verdict, same_side};
pub use vehicle::{Mount, Riding};

/// Side of a cell (`CZone::Create`).
const CELL: f32 = 40.0;
/// A player keeps its cell until it is this far outside it (`CCell::IsInnerCellView`).
const CELL_MARGIN: f32 = 10.0;
/// Cells seen in each direction: a 5 x 5 block.
const VIEW_CELLS: i32 = 2;
/// Range sends (`CZone::PSendView`): moves and actions, the far ring every fourth move, speed.
const ACTION_RANGE: f32 = 80.0;
const FAR_MOVE_RANGE: f32 = 120.0;
const SPEED_RANGE: f32 = 60.0;
/// Chat ranges (`PSendViewChat`, `0x1107`'s `PSendViewRange`).
pub const NORMAL_CHAT_RANGE: f32 = 30.0;
pub const SHOUT_RANGE: f32 = 100.0;

pub use crate::outbox::Outbox;

/// What the zone tells a player's connection besides packets.
#[derive(Clone, Debug, PartialEq)]
pub enum ZoneEvent {
    /// A player of the same country asks for this player's equipment (`0x0307`): the list goes to `asker`
    /// (`docs/original/small-items.md` section 7).
    Inspect { asker: u32 },
    /// The player did the most damage to a monster that died (`CMob::Death`): its experience is the player's. `killer` made the
    /// last hit.
    Kill { mob_id: u16, killer: u32 },
    /// The player died (`CUser::Death`, `docs/original/mob-fight.md` section 8) by its killer of record (`docs/original/pvp.md`
    /// section 6). `loss`: the player who killed it, to whom it may lose an item and gold (section 6.2); `counted`: its death counts;
    /// `keep_exp`: nothing is lost at rebirth (`m_nDropExp` −1: a duel, `docs/original/duel.md` section 6.1); `drop_exp`: otherwise
    /// the percent lost, fixed at the death by its item flags (`docs/original/item-flags.md` section 3).
    Died { killer_kind: u8, killer: u32, loss: Option<u32>, counted: bool, keep_exp: bool, drop_exp: u8 },
    /// The player's faction changed grade (`CUser::UpdateKCStatus`, `docs/original/pvp-2.md` section 10): its numbers again.
    KillGrade,
    /// The player killed another player, or a member of its party near it did (`docs/original/pvp.md` section 6.1): the victim's
    /// experience at its own level (×1.2 alone; by the party's size and highest level otherwise) and a kill. `last_hit`: the
    /// player who struck the killing blow, 0 when a monster did.
    PlayerKill { victim: u32, victim_level: u16, last_hit: u32, party: Option<(u8, bool, u16)> },
    /// A monster's attack stood the sitting player up and cancels a pending logout (`CUser::StatusStand`).
    StoodUp,
    /// What the player's applied skills add up to changed (`docs/original/skills-buffs.md`): the numbers again.
    Buffs,
    /// A monster's hit landed (`CUser::StatusCastingStop`, `docs/original/mob-skills.md` section 9): a cast in progress breaks.
    Struck,
    /// A blow reached the player (`CUser::Damage` → `QualityByDefense`, `docs/original/repair.md` section 2): the next armour piece
    /// wears. Not sent while its shape is not 0 or with Eternal Endurance.
    Worn,
    /// A resurrection skill brings the dead player back to life where its caster stands (`CUser::LifeSkill`,
    /// `docs/original/skills-special.md` section 9).
    Resurrected { position: [f32; 3], caster: u32, skill: u16 },
    /// The player's party killed a monster near it (`CMob::Death`, `docs/original/party.md` section 8): its share of the experience,
    /// by the party's size and highest level. `killer` made the last hit.
    PartyKill { mob_id: u16, killer: u32, members: u8, union: bool, max_level: u16 },
    /// An item of its party's kill, its turn (section 9): into its bags, or back on the ground at `position` with that owner and
    /// party when it does not fit. `dropped` is how it came to be on the ground (the game log's drop type).
    Loot { loot: Loot, position: [f32; 3], owner: u32, party: u32, dropped: Dropped },
    /// Its share of its party's gold (section 9); `from_mob`: a monster's, which the item flags raise (`docs/original/item-flags.md`
    /// section 4), not a player kill's.
    Gold { amount: u32, from_mob: bool },
    /// Its trade partner pressed ready last and waits (`docs/original/trade.md` section 5): the exchange is this player's to run.
    Trade,
    /// Everyone asked agreed to the guild this player asked to create (`docs/original/guild.md` section 4): it pays and the
    /// creation is written.
    GuildAgreed,
    /// A party member's summon stone asks this player (`docs/original/item-use.md` section 9; `0x0223` came through the outbox): the
    /// summoner gets a refusal when there is no answer by `until`.
    PartyCalled { until: Instant },
    /// Both duellists are ready (`docs/original/duel.md` section 4): this player's stakes come out of its bags.
    DuelStakes,
    /// The player's duel or guild war is over (`CUser::PvPModeEnd`, `PvPGModeEnd`): it stops attacking.
    DuelEnded,
    /// The player won or lost a duel (section 6.1): a victory or a defeat is counted. `opponent` is the other duellist's id and name
    /// (the game log's `0x0E01` and `0x0E02`).
    DuelResult { won: bool, opponent: u32, name: [u8; 21] },
    /// The duel ended without a winner (`PvPChkLeaveBy*`, section 6): `reason` is the cancel reason the client is told. The game log
    /// writes `0x0E03` for both sides.
    DuelCancelled { reason: u8, opponent: u32, name: [u8; 21] },
    /// A record for the game log that the zone made without the character's numbers (the guilds, the parties): the connection puts its
    /// header on it (`record::header`) and writes it.
    Record(Box<crate::game_log::Row>),
    /// A purchase from the player's personal shop waits (`docs/original/personal-shop.md` section 7): this connection carries it out.
    Sale,
    /// A Lua monster calls the player to a spot (`LuaRecallUser`, `docs/original/instances.md` section 15): a move there, to `zone`.
    Recalled { zone: u16, position: [f32; 3] },
    /// A GM left orders for the player (`Zones::take_admin_orders`, `docs/plans/done/gm-commands.md`): this connection carries them out.
    Admin,
    /// A script left orders for the player (`Zones::take_script_orders`, `docs/plans/ext-5-lua.md` section 4.4).
    Script,
    /// The item mall has something for the account (`OnlineUsers::take_mall`, `docs/original/item-mall.md` deviations 5 and 6): a gift
    /// that came, or its points changed.
    Mall,
}

/// What a player takes with it out of the zone (`Zones::leave`).
#[derive(Debug)]
pub struct Left {
    /// HP, MP and SP.
    pub health: [u16; 3],
    pub buffs: PlayerBuffs,
    /// Duel stakes it had out of its bags, for them (`docs/original/duel.md`, deviation 3).
    pub stakes: Option<crate::duel::Held>,
}

/// A player's current and maximum HP, MP and SP, and what monster attacks need of it (`docs/original/mob-fight.md`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vitals {
    /// HP, MP and SP.
    pub health: [u16; 3],
    pub max: [u16; 3],
    /// The player's numbers against monsters (level, melee hit, evasion and defence, LUC) and its upper armour's element.
    pub guard: Fighter,
    pub guard_element: u8,
    /// Sitting recovery in percent of each maximum, from passive skills (`m_nSitRecHPP`, `MPP`, `SPP`).
    pub sit_percent: [i32; 3],
    /// REC: Fire Thorns adds REC × 10 aggro with each hit.
    pub rec: i32,
    /// Absorption of HP damage, and the states lapis block (`docs/original/lapis.md` section 2).
    pub absorption: i32,
    pub state_blocks: u16,
    /// The faction grade's recovery, HP, MP, SP: normal and sitting, and fighting (`docs/original/pvp-2.md` section 8).
    pub grade_recovery: [i32; 3],
    pub grade_fight_recovery: [i32; 3],
    /// The lapis skills its hits and the hits it takes set off (`docs/original/item-options.md`), and what the skills need of it.
    pub options: OptionSkills,
    pub cross_road: bool,
}

/// What the zone keeps of a player's equipment and numbers for monster attacks (`set_guard`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Guard {
    pub fighter: Fighter,
    pub element: u8,
    pub rec: i32,
    pub absorption: i32,
    pub state_blocks: u16,
    pub grade_recovery: [i32; 3],
    pub grade_fight_recovery: [i32; 3],
    pub options: OptionSkills,
    pub cross_road: bool,
}

/// One normal attack on a monster (`docs/original/combat.md` section 3).
pub struct Attack<'a> {
    pub attacker: u32,
    pub country: u8,
    pub fighter: &'a Fighter,
    pub kind: usize,
    /// `combat::range_slack` and the player's own range.
    pub slack: f32,
    pub range: i32,
    /// The attacker's REC: each hit adds REC × 10 aggro (`CDamageMob::Add`).
    pub rec: i32,
    /// A passive skill keeps ranged attacks within 3 at full damage (`m_nCrossRoad`).
    pub cross_road: bool,
}

#[derive(Debug, PartialEq)]
pub enum AttackOutcome {
    /// The monster is gone, dead or of the player's country: the attack stops without a word.
    Gone,
    OutOfRange,
    /// The packets for the attacker (`0x0606` on a kill, then `0x0503`), the kill, and whether the blow was a critical (it wears the
    /// weapon, `docs/original/repair.md` section 2).
    Struck {
        packets: Vec<Vec<u8>>,
        killed: bool,
        critical: bool,
    },
}

/// What a skill aims at when it goes off (`m_Target`): a monster, or a player (oneself included).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkillTarget {
    Mob(u32),
    Player(u32),
}

/// A skill going off (`docs/original/skills-use.md` section 7): who uses it, with what numbers.
pub struct SkillCast<'a> {
    pub caster: u32,
    pub country: u8,
    pub skill: &'a SkillDef,
    /// The caster's numbers; its `wis` adds to heals.
    pub fighter: &'a Fighter,
    /// The caster's REC: each hit adds REC × 10 aggro, and the skill's own.
    pub rec: i32,
    /// A passive skill keeps ranged hits within 3 at full damage (`m_nCrossRoad`).
    pub cross_road: bool,
    /// `m_nDecSP`, `m_nDecMP`.
    pub cost_reduction: [i32; 2],
    /// The monster the caster was attacking before a skill on itself (`m_TargetPrev`): a buff's aggro goes to it.
    pub fighting: Option<u32>,
    /// What a disguise copies (`m_pCloneUser`) and an illusion's monster (`m_wTransFormTargetMobID`), chosen with the target
    /// (`docs/original/skills-special.md` section 5).
    pub look: Option<&'a UserShape>,
    pub illusion_mob: u16,
    /// The map has a war type: nobody is an enemy there (`docs/original/pvp.md` section 1).
    pub war_map: bool,
}

#[derive(Debug, PartialEq)]
pub struct SkillOutcome {
    /// The caster's packets in order, the costs (`0x050C`) last.
    pub packets: Vec<Vec<u8>>,
    /// A monster died of it: the caster stops (section 8 step 4).
    pub killed: bool,
    /// The caster gave up its life (`wType` 23) and is dead.
    pub died: bool,
    /// Return (`wType` 125): the caster goes to the bind position (`docs/original/skills-special.md` section 9).
    pub recall: bool,
    /// Critical hits on monsters: each wears the weapon (`docs/original/repair.md` section 2).
    pub criticals: u8,
}

/// What a skill needs to know of a player it may aim at (`CUser::EnableAttackUser`, `IsTeamUsable`, `IsCloneTargetingUser`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Whereabouts {
    pub position: [f32; 3],
    pub country: u8,
    pub dead: bool,
    /// Its shape (`m_byShape`): hidden players cannot be aimed at.
    pub shape: u8,
    pub level: u16,
    pub grow: u8,
    /// Not on foot (summoning, riding, behind someone): Evolution cannot aim at it (`CUser::ChkTargetUser`).
    pub mounted: bool,
    /// Fighting a duel: no party help from or to it (`CUser::IsTeamUsable`, `docs/original/duel.md` section 5).
    pub fighting: bool,
    /// Its enemy guild in a guild war, 0 for none: party help only within a side (`docs/original/guild-war.md` section 5).
    pub war: u32,
}

/// A player's shape as others see it (`m_byShape` and the monster of an illusion; `docs/original/skills-special.md` section 2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Form {
    shape: u8,
    mob: u16,
}

/// A trap or fire wall on the ground (`CTrab`, `docs/original/skills-special.md` section 8).
#[derive(Clone, Copy, Debug)]
struct Field {
    id: u32,
    owner: u32,
    country: u8,
    /// The owner as the rules saw it when the field was made, so the test holds after it left (`docs/plans/ext-2-rules.md` point 1).
    owner_who: crate::rules::Who,
    skill: SkillDef,
    position: [f32; 3],
    cell: (i32, i32),
    ends: Instant,
}

/// Something on the ground (`docs/original/combat.md` section 7).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundItem {
    pub id: u32,
    pub loot: Loot,
    pub position: [f32; 3],
    cell: (i32, i32),
    /// Who may pick it up while it is fresh; 0 for anyone. The members of its party may too (`CItem::m_dwPartyID`).
    pub owner: u32,
    pub party: u32,
    /// When it disappears.
    pub expires: Instant,
    /// How it came to be on the ground (`CItem::m_byDropType`) and who dropped it: the game log's pick-up records are chosen by it
    /// (`docs/original/game-log.md` section 3.12).
    pub dropped: Dropped,
}

/// How an item came to be on the ground: the original's drop type (`CItem::m_byDropType`, `docs/original/game-log.md` section 3.12) and
/// the player it was dropped by or at the death of (`CItem +0x7c`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Dropped {
    pub kind: u8,
    pub by: u32,
}

impl Dropped {
    /// A monster's drop (`CMob::ItemDrop`).
    pub const MOB: Dropped = Dropped { kind: 1, by: 0 };

    /// What a player dropped (`CUser::ItemDrop`).
    pub fn player(by: u32) -> Self {
        Self { kind: 2, by }
    }

    /// What a player lost at a death (`ItemDropByUser`, `ItemDropByUserDeath`, `ItemDropPvP`).
    pub fn death(by: u32) -> Self {
        Self { kind: 3, by }
    }
}

impl GroundItem {
    /// A monster's drop: its gold is raised by the picker's item flags (`docs/original/item-flags.md` section 4).
    pub fn from_mob(&self) -> bool {
        self.dropped.kind == Dropped::MOB.kind
    }

    fn enter_packet(&self, kind: u8) -> Vec<u8> {
        let (item_type, type_id, count) = match self.loot {
            Loot::Item { item_type, type_id, count, .. } => (item_type, type_id, count),
            Loot::Gold(_) => (GOLD_TYPE, 0, 1),
        };
        gp::item_enter(self.id, kind, item_type, type_id, count, self.position)
    }
}

/// The items on one map's ground, by cell.
#[derive(Debug, Default)]
struct Ground {
    items: HashMap<u32, GroundItem>,
    cells: HashMap<(i32, i32), Vec<u32>>,
}

impl Ground {
    fn insert(&mut self, item: GroundItem) {
        self.cells.entry(item.cell).or_default().push(item.id);
        self.items.insert(item.id, item);
    }

    fn remove(&mut self, id: u32) -> Option<GroundItem> {
        let item = self.items.remove(&id)?;
        if let Some(ids) = self.cells.get_mut(&item.cell) {
            ids.retain(|&other| other != id);
            if ids.is_empty() {
                self.cells.remove(&item.cell);
            }
        }
        Some(item)
    }

    /// The items in the cells of `view` that are not in the view of `except` (all of them without one).
    fn in_view(&self, view: (i32, i32), except: Option<(i32, i32)>) -> Vec<GroundItem> {
        view_cells(view)
            .filter(|&cell| except.is_none_or(|other| !in_view(other, cell)))
            .filter_map(|cell| self.cells.get(&cell))
            .flatten()
            .filter_map(|id| self.items.get(id).copied())
            .collect()
    }
}

/// How long an item stays on the ground (`CZone::EnterItem`), and how long of that it is its owner's alone (`CZone::GetPickItem`).
const ITEM_LIFETIME: Duration = Duration::from_secs(60);
const OWNER_ONLY: Duration = Duration::from_secs(30);
/// How close a player must be to pick something up (`g_bChkItemPick`).
const PICK_RANGE: f32 = 10.0;

/// Range of attack packets (`CZone::PSendView` with 60).
const ATTACK_RANGE: f32 = 60.0;
/// A ranged attack this close is weaker (`CUser::AttackDamage`).
const CLOSE_RANGE: f32 = 3.0;

/// Recovery states (`m_nRecStatus`) and the amounts of `CUser::InitEquipment` (the same for HP, MP and SP; sitting adds 5 % of the
/// maximum); recovery comes every 5 s (`docs/original/mob-fight.md` section 9).
const REC_SITTING: u8 = 0;
const REC_NORMAL: u8 = 1;
const REC_FIGHTING: u8 = 2;
const REC_NORMAL_AMOUNT: i32 = 4;
const REC_SITTING_AMOUNT: i32 = 28;
const RECOVER_EVERY: Duration = Duration::from_secs(5);
/// Range of recovery, death-adjacent and monster attack packets (`CZone::PSendView` with 60).
const RECOVER_RANGE: f32 = 60.0;
/// Monsters' own attacks and calls look at the 3 x 3 cells around them (`CMob::FindTarget`, `FindHelp*`).
const NEAR_CELLS: i32 = 1;

fn clamp_i16(v: i32) -> i16 {
    v.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
}

/// A player as the others see it.
#[derive(Clone, Debug)]
pub struct Occupant {
    pub id: u32,
    pub staff: bool,
    /// The staff's visibility, protection from monsters and guards, and whether players may attack it (`+0x5774`, `+0x5775` and
    /// deviation 15 of `docs/original/gm-commands.md`); true for players, false for staff on entering.
    pub visible: bool,
    pub attackable: bool,
    pub pvp: bool,
    pub position: [f32; 3],
    pub dir: u16,
    pub guild_id: u32,
    pub attack_speed: u8,
    pub move_speed: u8,
    pub vitals: Vitals,
    /// Looks and motion, for other players' `0x0303`.
    pub shape: UserShape,
    /// The skills applied to the player and its low-HP passive (`docs/original/skills-buffs.md`); the zone keeps them while the
    /// player is in it.
    pub buffs: PlayerBuffs,
    pub outbox: Outbox,
    pub events: UnboundedSender<ZoneEvent>,
    /// Riding when it comes into the zone: a mount kept through a map change (`docs/original/vehicles.md` section 5).
    pub mount: Option<Mount>,
}

struct Placed {
    occupant: Occupant,
    cell: (i32, i32),
    moves: u32,
    dead: bool,
    /// Attacking a monster (the original's status 2), the recovery state and the next recovery (`docs/original/mob-fight.md`
    /// section 9).
    attacking: bool,
    rec_status: u8,
    next_recover: Option<Instant>,
    /// The shape others are told of, and what a disguise shows instead of the player's own looks.
    form: Form,
    disguise: Option<UserShape>,
    /// Mounts: the vehicle status, the pair and the invitation (`docs/original/vehicles.md`).
    ride: vehicle::Ride,
    /// Who hurt it since it was last at full HP (`CDamage`, `docs/original/pvp.md` section 5).
    harm: Vec<pvp::Harm>,
    /// Its side of a duel (`docs/original/duel.md`), the partner on this map.
    duel: Option<crate::duel::Side>,
    /// Its part in a guild war (`docs/original/guild-war.md`), on the war's map.
    war: Option<crate::guild_war::Part>,
    /// Its personal shop, set up or open, and the seller whose shop it looks at (`docs/original/personal-shop.md`), on this map.
    shop: Option<crate::my_shop::Shop>,
    visiting: Option<u32>,
    /// Its lapis skills are going off (`+0x57b4`, `docs/original/item-options.md` section 5): no more of its own meanwhile.
    options_running: bool,
    /// Its current NPC (`+0x1564`, `docs/original/guards.md` section 2) and until when a guard has taken it (`+0x5850`).
    npc: Option<u32>,
    guard_lock: Option<Instant>,
}

impl Placed {
    /// `TP_USER_ENTER` of this player: `byNew` 1 when it just entered the zone, 0 when it is already there; 2 for staff.
    fn enter_packet(&self, new: bool) -> Vec<u8> {
        let o = &self.occupant;
        let kind = if o.staff { 2 } else { u8::from(new) };
        // A passenger's driver in the last field (`docs/original/vehicles.md` section 4).
        let vehicle = if self.ride.status == Riding::Behind { self.ride.partner } else { 0 };
        w::user_enter(&UserEnter { char_id: o.id, kind, dir: o.dir, position: o.position, guild_id: o.guild_id, vehicle })
    }

    fn speed_packet(&self) -> Vec<u8> {
        w::speed(self.occupant.id, self.occupant.attack_speed, self.occupant.move_speed)
    }

    /// The shape for players it comes into view of, after its `0x0201`, when it has one (`CCell::SendCellInfoEnter`); an illusion
    /// with its monster (deviation: the original sends a bare `0x051D` 102 there).
    fn form_packet(&self) -> Option<Vec<u8>> {
        (self.form.shape != 0 || !self.occupant.visible || matches!(self.ride.status, Riding::Riding(_))).then(|| self.shape_packet())
    }

    /// Logs an invisible staff member's shape 13 going to a viewer, so the server log can be laid beside a client's packet log
    /// (`docs/original/small-items.md` section 3).
    fn log_hidden(&self, viewer: u32, path: &'static str) {
        if !self.occupant.visible {
            tracing::debug!(staff = self.occupant.id, viewer, path, "staff hidden from viewer");
        }
    }

    /// `0x0303`: its looks, or the copied ones while disguised (`CUser::SendUserShape`).
    fn looks_packet(&self) -> Vec<u8> {
        match &self.disguise {
            Some(copy) if self.form.shape == sk::SHAPE_DISGUISE => w::user_shape(copy),
            _ => w::user_shape(&self.occupant.shape),
        }
    }

    fn send(&self, packet: &[u8]) {
        // A closed outbox means that connection is ending; it leaves the zone itself.
        let _ = self.occupant.outbox.send(packet.to_vec());
    }
}

/// The shape of an invisible staff member (`CUser::GetShape` while `+0x5774` is 0).
const INVISIBLE_SHAPE: u8 = 13;

/// `CUser::SendShape`: `0x0524` with the monster for an illusion, `0x051D` otherwise.
fn form_packet(id: u32, form: Form) -> Vec<u8> {
    if form.shape == sk::SHAPE_ILLUSION { sk::shape_mob(id, form.mob) } else { sk::shape(id, form.shape) }
}

/// After a player's applied skills changed (`docs/original/skills-special.md` sections 3, 5 and 6): a new shape goes to its view,
/// the player included (`CUser::SendShape`); hiding makes its client walk and its end run again (`0x0210`); a disguise's start and
/// end send the looks (`CUser::SendUserShape`). At death only the looks go: the clients clear the rest themselves.
fn reform(players: &mut HashMap<u32, Placed>, id: u32, death: bool) {
    let Some(p) = players.get_mut(&id) else { return };
    let effects = p.occupant.buffs.effects();
    let (old, new) = (p.form, Form { shape: effects.shape, mob: effects.shape_mob });
    if old == new {
        return;
    }
    p.form = new;
    let disguise = (old.shape == sk::SHAPE_DISGUISE) != (new.shape == sk::SHAPE_DISGUISE);
    if new.shape != sk::SHAPE_DISGUISE {
        p.disguise = None;
    }
    let p = &players[&id];
    let viewers = || players.values().filter(|o| in_view(o.cell, p.cell));
    if !death {
        let packet = if p.occupant.visible { form_packet(id, new) } else { sk::shape(id, INVISIBLE_SHAPE) };
        viewers().for_each(|o| o.send(&packet));
        if new.shape == sk::SHAPE_HIDDEN {
            p.send(&sk::run_mode(false));
        } else if old.shape == sk::SHAPE_HIDDEN {
            p.send(&sk::run_mode(true));
        }
    }
    if disguise {
        let looks = p.looks_packet();
        viewers().for_each(|o| o.send(&looks));
    }
}

/// `CUser::RemApplySkillHide` (section 6): a hidden player is found; its hiding ends (`0x050E`) and so does the shape.
fn reveal(players: &mut HashMap<u32, Placed>, id: u32) {
    let Some(p) = players.get_mut(&id) else { return };
    if p.form.shape == 0 {
        return;
    }
    let Some(a) = p.occupant.buffs.reveal() else { return };
    p.send(&sk::applied_end(a.key));
    let _ = p.occupant.events.send(ZoneEvent::Buffs);
    reform(players, id, false);
}

/// `CUser::ChkFindHide` (section 4): the living players of another country in the 3 x 3 cells around the finder and within `range`
/// of it are revealed.
fn reveal_around(players: &mut HashMap<u32, Placed>, finder: u32, range: f32) {
    let Some(f) = players.get(&finder) else { return };
    let (cell, at, country) = (f.cell, f.occupant.position, f.occupant.shape.country);
    let found: Vec<u32> = players
        .values()
        .filter(|p| p.occupant.id != finder && !p.dead && p.form.shape == sk::SHAPE_HIDDEN && p.occupant.shape.country != country)
        .filter(|p| near_cells(p.cell, cell) && distance(p.occupant.position, at) <= range)
        .map(|p| p.occupant.id)
        .collect();
    for id in found {
        reveal(players, id);
    }
}

/// Within the 3 x 3 cells around a cell.
fn near_cells(a: (i32, i32), b: (i32, i32)) -> bool {
    (a.0 - b.0).abs() <= NEAR_CELLS && (a.1 - b.1).abs() <= NEAR_CELLS
}

/// The `0x0501` with motion `0xC4` that the original sends before telling about a player.
fn position_sync(o: &Occupant) -> Vec<u8> {
    w::user_move(o.id, &Move { motion: w::MOTION_SYNC, dir: o.dir, position: o.position })
}

fn cell_of(position: [f32; 3]) -> (i32, i32) {
    ((position[0] / CELL) as i32, (position[2] / CELL) as i32)
}

/// Whether a position is still inside the cell's widened bounds.
fn keeps_cell(cell: (i32, i32), position: [f32; 3]) -> bool {
    let inside = |index: i32, v: f32| {
        let low = index as f32 * CELL - CELL_MARGIN;
        (low..=low + CELL + 2.0 * CELL_MARGIN).contains(&v)
    };
    inside(cell.0, position[0]) && inside(cell.1, position[2])
}

fn in_view(a: (i32, i32), b: (i32, i32)) -> bool {
    (a.0 - b.0).abs() <= VIEW_CELLS && (a.1 - b.1).abs() <= VIEW_CELLS
}

/// Horizontal distance (x and z, `SVector::Distance2`).
pub fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

#[derive(Default)]
pub struct Zones {
    maps: Mutex<HashMap<u16, HashMap<u32, Placed>>>,
    mobs: Mutex<HashMap<u16, MobMap>>,
    /// The monsters' object id counter (the original's object manager `+0x08`).
    last_mob_id: AtomicU32,
    /// Each zone's NPCs (`docs/original/guards.md`). Taken after `maps` and `mobs`, before the ground.
    npcs: Mutex<HashMap<u16, npc::NpcMap>>,
    /// The NPCs' object id counter: party and guild zones get theirs from it.
    last_npc_id: AtomicU32,
    /// Items on the ground by map. Taken after `maps` and `mobs`.
    ground: Mutex<HashMap<u16, Ground>>,
    last_item_id: AtomicU32,
    /// Keys of applied skills (`CObjectMgr::CreateSkill`).
    last_skill_key: AtomicU32,
    /// Traps and fire walls by map, and their ids (`CObjectMgr::CreateTrab`). Taken after `maps` and `mobs`.
    fields: Mutex<HashMap<u16, Vec<Field>>>,
    last_field_id: AtomicU32,
    /// The parties and pending invitations (`docs/original/party.md`). Taken after `maps` and `mobs`, held briefly.
    parties: Mutex<crate::party::Parties>,
    /// The trades (`docs/original/trade.md`). Taken after `maps`, held briefly.
    trades: Mutex<crate::trade::Trades>,
    /// The guilds and every player in the world as they need it (`docs/original/guild.md`). Taken after `maps` and `parties`, held
    /// briefly.
    guilds: Mutex<crate::guild::Guilds>,
    /// Guild bank moves run one at a time, from the plan to the apply across the database write (`docs/original/guild-bank.md`
    /// section 14). Never taken while holding one of the locks above.
    guild_bank_turn: tokio::sync::Mutex<()>,
    /// Friends, block lists and the party search, and every player in the world as they need it (`docs/original/friends.md`). Taken
    /// after `maps` and `parties`, held briefly; never with `guilds`.
    friends: Mutex<crate::friends::Friends>,
    /// Summons waiting for an answer, by member: the summoner and until when (`docs/original/item-use.md` section 9). Taken after
    /// `maps`, held briefly.
    party_calls: Mutex<HashMap<u32, (u32, Instant)>>,
    /// Purchases waiting for each seller's connection, in order (`docs/original/personal-shop.md` section 7). Taken after `maps`,
    /// held briefly.
    sales: Mutex<HashMap<u32, std::collections::VecDeque<crate::my_shop::Purchase>>>,
    /// The guild wars by guild (`docs/original/guild-war.md`). Taken after `maps` and `guilds`, held briefly.
    wars: Mutex<crate::guild_war::Wars>,
    /// The level-100 skills, which lapis set off (`docs/original/item-options.md`).
    item_skills: OnceLock<HashMap<u16, SkillDef>>,
    /// The game log, for the records the zone makes itself (the obelisks, the monsters; `docs/original/game-log.md` section 3.6).
    game_log: OnceLock<crate::game_log::GameLog>,
    /// The `AI` numbers with a state machine (`docs/original/scripted-mobs.md`).
    scripted_ais: std::collections::HashSet<u8>,
    /// What the obelisks ask of the server (`zone/obelisk.rs`). Taken after `maps` and `mobs`, held briefly.
    obelisk_events: Mutex<Vec<crate::obelisk::Event>>,
    /// Static maps' portal countries changed by a boss group (`zone/boss.rs`), by map and place; missing: the map file's. Taken after
    /// `maps` and `mobs`, held briefly.
    portal_countries: Mutex<HashMap<(u16, u8), u32>>,
    /// Instance portal countries to set (`CZone::UpdateInsZonePortalCountry`: zone, id, country), written from inside the `mobs` lock
    /// and applied to `instances` by whoever takes it next. Taken alone, held briefly.
    instance_portal_writes: Mutex<Vec<(u16, u32, u32)>>,
    /// The party instance zones (`docs/original/instances.md`): their parties, who is on the way, their portal countries. Every zone
    /// structure here is keyed by zone (`crate::world::map_of` gives its map). Taken after `maps` and `parties`, before `mobs`.
    instances: Mutex<crate::instance::Instances>,
    /// The guild ranking battle (`docs/original/guild-ranking.md`): its map and its clock's state. Taken alone, held briefly.
    ranking_map: Option<u16>,
    ranking: Mutex<crate::ranking::Battle>,
    /// What GMs asked of other players' connections, by character (`admin::AdminOrder`). Taken after `maps`, held briefly.
    admin_orders: Mutex<HashMap<u32, Vec<AdminOrder>>>,
    /// What scripts asked of players' connections, by character (`script::event::Queued`). Taken after `maps`, held briefly.
    script_orders: Mutex<HashMap<u32, Vec<crate::script::event::Queued>>>,
    /// The chat and move bans by character (deviation 9 of `docs/original/gm-commands.md`). Taken alone, held briefly.
    bans: Mutex<HashMap<u32, Bans>>,
    /// The GMs listening to players' chat (`admin::Listens`). Taken alone, held briefly.
    listens: Mutex<admin::Listens>,
    /// The quests in progress by online character, for the quest items a monster drops (`docs/original/quest-drops.md` section 6).
    /// A leaf lock: taken last, nothing else's lock taken inside it.
    quests: Mutex<HashMap<u32, Vec<u16>>>,
    /// Running steps paced to the client, with the move tick between the passes (`docs/original/chase-smoothing.md`); off in the
    /// tests unless one turns it on.
    smooth: AtomicBool,
    /// The rules in force (`docs/plans/ext-2-rules.md` section 5): swapped whole by `reload`. A leaf lock, held for the clone of the
    /// `Arc` only (section 8).
    rules: std::sync::RwLock<std::sync::Arc<crate::rules::RuleSet>>,
    /// The extensions' scripts in force and the script thread's queue (`docs/plans/ext-5-lua.md` section 4.1): `zone/script.rs`.
    scripts: std::sync::Arc<crate::script::runner::ScriptCell>,
    script_jobs: OnceLock<crate::script::runner::Jobs>,
    /// The staff level each online staff member entered with, for `p:staff()`. A leaf lock.
    script_staff: Mutex<HashMap<u32, u8>>,
    /// The monsters extensions spawned (`game.spawn`), by object id. A leaf lock, taken inside the zone's.
    ext_spawns: Mutex<HashMap<u32, script::ExtSpawn>>,
}

/// A packet for the players within `range` of a spot.
fn send_around(players: Option<&HashMap<u32, Placed>>, at: [f32; 3], range: f32, packet: &[u8]) {
    for p in players.into_iter().flat_map(|p| p.values()).filter(|p| distance(p.occupant.position, at) <= range) {
        p.send(packet);
    }
}

/// Alive again with HP, MP and SP full, standing; returns what was gained.
fn revive(p: &mut Placed, half: bool) -> [i16; 3] {
    let v = &mut p.occupant.vitals;
    // A resurrection item brings it back at half (`CUser::KillSoulByItem`, `docs/original/item-flags.md` section 7).
    let back = if half { v.max.map(|m| m / 2) } else { v.max };
    let gained = [0, 1, 2].map(|i| clamp_i16(i32::from(back[i]) - i32::from(v.health[i])));
    v.health = back;
    p.dead = false;
    p.attacking = false;
    p.rec_status = p.rec_status.max(REC_NORMAL);
    p.occupant.shape.motion = w::MOTION_STAND;
    gained
}

/// What the monster passes need besides the zone's maps: the definitions, the loot tables (a reflected blow can kill), the time and
/// the random numbers.
struct Pass<'a, R> {
    zones: &'a Zones,
    map: u16,
    defs: &'a MobDefs,
    loot: (&'a DropTables, &'a ItemDefs),
    now: Instant,
    rand: &'a mut R,
}

/// One hit on a monster: who, the HP, SP and MP damage (HP in 32 bits: a death sentence takes all of it), and the aggro it adds
/// (`CDamageMob::Add`).
struct Hit {
    attacker: u32,
    damage: [u32; 3],
    aggro: u32,
    /// A blow or a skill hit (`CUser::AttackMob`): it also counts for the attacker's party (`CMob::DamageParty`,
    /// `docs/original/party.md` section 7).
    direct: bool,
}

/// The end of a heal over time (`wType` 81, `docs/original/skills-buffs.md` section 5): the player healed for the time it ran.
/// Returns `0x050F` for the players within 60 of it.
fn heal_over_time(p: &mut Placed, a: &Applied, now: Instant) -> Option<Vec<u8>> {
    if a.def.type_effect != skills::BUFF || a.def.kind != buffs::HEAL_OVER_TIME || p.dead {
        return None;
    }
    let healed = heal_player(p, buffs::time_heal(&a.def, now.saturating_duration_since(a.started)));
    Some(sk::keep(sk::OP_KEEP_USER, p.occupant.id, a.def.id, a.def.level, healed))
}

/// What a monster debuff's end deals (`CMob::RemApplySkill`, section 12): damage over time for the time it ran, or all the HP
/// for a death sentence; nothing for the others.
fn debuff_end_damage(d: &MobDebuff, mob: &Mob, def: &MobDef, now: Instant) -> Option<[u32; 3]> {
    match d.def.kind {
        buffs::DAMAGE_OVER_TIME => {
            let current = [mob.hp, u32::from(mob.sp), u32::from(mob.mp)];
            let max = [def.hp, u32::from(def.sp), u32::from(def.mp)];
            Some(buffs::time_damage(&d.def, current, max, now.saturating_duration_since(d.started)).map(u32::from))
        }
        buffs::DEATH_SENTENCE => Some([mob.hp, 0, 0]),
        _ => None,
    }
}

/// The values a packet can carry.
fn shown(damage: [u32; 3]) -> [u16; 3] {
    damage.map(|v| v.min(u32::from(u16::MAX)) as u16)
}

/// `heal_vitals` for a player in the zone: HP back at its maximum from below clears the damage record (`CUser::Heal`,
/// `SendRecoverAdd`; `docs/original/pvp.md` section 5).
fn heal_player(p: &mut Placed, amounts: [u16; 3]) -> [u16; 3] {
    let healed = heal_vitals(&mut p.occupant.vitals, amounts);
    full_again(p, healed[0] > 0);
    healed
}

/// Clears the damage record when HP has just come back to its maximum.
fn full_again(p: &mut Placed, rose: bool) {
    if rose && p.occupant.vitals.health[0] == p.occupant.vitals.max[0] {
        p.harm.clear();
    }
}

/// HP, SP and MP raised by `amounts` (HP, SP, MP), not above the maxima (`CUser::Heal`); returns what fitted.
fn heal_vitals(v: &mut Vitals, amounts: [u16; 3]) -> [u16; 3] {
    // Vitals are HP, MP, SP.
    let mut healed = [0; 3];
    for (i, slot) in [0, 2, 1].into_iter().enumerate() {
        let room = v.max[slot].saturating_sub(v.health[slot]);
        healed[i] = amounts[i].min(room);
        v.health[slot] += healed[i];
    }
    healed
}

/// A skill's effect being carried out in a zone, with both locks held (`Zones::use_skill`).
/// Whose lapis go off: the weapon's on the target of the owner's hit (`CUser::AttackItemOption`), or the armour's on who hit the
/// owner (`CUser::BeAttackedItemOption`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Worn {
    Weapon,
    Armour,
}

/// What an owner's lapis skills came to: its packets in order (none when they went to its queue), a kill (it stops attacking), critical
/// hits (each wears the weapon).
#[derive(Debug, Default)]
struct Optioned {
    out: Vec<Vec<u8>>,
    killed: bool,
    criticals: u8,
}

/// `CUser::AttackItemOption` and `BeAttackedItemOption` (`docs/original/item-options.md` sections 2 to 5): each of the owner's lapis
/// skills (`Worn`) goes off on `target` as a skill used by hand on one target, the owner its caster, without cooldown or costs: an
/// attack through `SkillAttackSelect`, a debuff as one hit. Nothing while the owner's lapis already go off (its guard), or when the
/// owner or the target is dead. `queue`: the owner's packets go to its connection's queue as they come (it is not the player whose
/// connection runs this).
#[allow(clippy::too_many_arguments)]
fn item_options<R: FnMut() -> u32>(
    pass: &mut Pass<'_, R>,
    mobs: &mut MobMap,
    players: &mut HashMap<u32, Placed>,
    owner: u32,
    target: SkillTarget,
    worn: Worn,
    war_map: bool,
    queue: bool,
) -> Optioned {
    let mut done = Optioned::default();
    let Some(table) = pass.zones.item_skills.get() else { return done };
    let Some(me) = players.get(&owner).filter(|p| !p.dead && !p.options_running) else { return done };
    let v = me.occupant.vitals;
    let ids = match worn {
        Worn::Weapon => &v.options.weapon[..],
        Worn::Armour => &v.options.armour[..],
    };
    let target_alive = match target {
        SkillTarget::Mob(id) => mobs.mobs.get(&id).is_some_and(|m| m.dead_until.is_none() && m.hp > 0),
        SkillTarget::Player(id) => players.get(&id).is_some_and(|p| !p.dead),
    };
    if !target_alive || ids.iter().all(|&id| id == 0) {
        return done;
    }
    let (at, cell, country) = (me.occupant.position, me.cell, me.occupant.shape.country);
    let at_war = me.war.is_some_and(|w| w.ready);
    let duel_flag = at_war
        || matches!(target, SkillTarget::Player(t) if duel::fighting(me, t) && players.get(&t).is_some_and(|p| duel::fighting(p, owner)));
    players.get_mut(&owner).expect("found above").options_running = true;
    let rules = pass.zones.rules();
    for skill in ids.iter().filter_map(|id| table.get(id)) {
        let cast = SkillCast {
            caster: owner,
            country,
            skill,
            fighter: &v.guard,
            rec: v.rec,
            cross_road: v.cross_road,
            cost_reduction: [0; 2],
            fighting: None,
            look: None,
            illusion_mob: 0,
            war_map,
        };
        let mut fx = Effect {
            zones: pass.zones,
            rules: rules.clone(),
            map: pass.map,
            players: &mut *players,
            mobs: &mut *mobs,
            cast: &cast,
            at,
            cell,
            defs: pass.defs,
            loot: pass.loot,
            now: pass.now,
            rand: &mut *pass.rand,
            out: Vec::new(),
            killed: false,
            died: false,
            recall: false,
            criticals: 0,
            queue,
            duel_flag,
            options: true,
        };
        // A target that died of an earlier lapis is left alone (deviation 2): the hits look for that themselves.
        match (skill.type_effect, target) {
            (skills::ATTACK, SkillTarget::Mob(id)) => fx.select_mob(id),
            (skills::DEBUFF, SkillTarget::Mob(id)) => fx.hit(id, true),
            (skills::ATTACK, SkillTarget::Player(id)) => fx.select_player(id),
            (skills::DEBUFF, SkillTarget::Player(id)) => fx.hit_player(id, true),
            _ => {}
        }
        if skill.type_effect == skills::ATTACK && skill.kind == skills::DESPERATE {
            fx.give_up_life();
        }
        done.out.append(&mut fx.out);
        done.killed |= fx.killed;
        done.criticals = done.criticals.saturating_add(fx.criticals);
    }
    if let Some(me) = players.get_mut(&owner) {
        me.options_running = false;
    }
    done
}

struct Effect<'a, 'c, R> {
    zones: &'a Zones,
    /// The rules in force when the cast started (`docs/plans/ext-2-rules.md` section 8: one snapshot per cast).
    rules: std::sync::Arc<crate::rules::RuleSet>,
    map: u16,
    players: &'a mut HashMap<u32, Placed>,
    mobs: &'a mut MobMap,
    cast: &'a SkillCast<'c>,
    /// The caster's position and cell.
    at: [f32; 3],
    cell: (i32, i32),
    defs: &'a MobDefs,
    loot: (&'a DropTables, &'a ItemDefs),
    now: Instant,
    rand: &'a mut R,
    out: Vec<Vec<u8>>,
    killed: bool,
    died: bool,
    recall: bool,
    /// Critical hits on monsters: each wears the weapon.
    criticals: u8,
    /// The caster is not the player whose connection runs this (its lapis struck back): its own packets go to its queue as they come,
    /// behind those that reached it meanwhile.
    queue: bool,
    /// `CUser::Death`'s flag for the desperate counterattack: the caster fights a duel with the player it aimed at
    /// (`docs/original/duel.md` section 6.1), or is in a guild war (`docs/original/guild-war.md` section 5; deviation 3: from the
    /// countdown on).
    duel_flag: bool,
    /// The lapis skills follow a hit that landed; not for an item's skill on a target (`docs/original/small-items.md` section 11).
    options: bool,
}

impl<R: FnMut() -> u32> Effect<'_, '_, R> {
    /// The caster's own copy of a packet: in `out`, or straight to its connection's queue with `queue`.
    fn mine(&mut self, packet: Vec<u8>) {
        if !self.queue {
            self.out.push(packet);
        } else if let Some(p) = self.players.get(&self.cast.caster) {
            p.send(&packet);
        }
    }

    /// A packet for the players within 60 of the caster (`CZone::PSendView`), the caster's in `out`.
    fn around_caster(&mut self, packet: Vec<u8>) {
        let (me, at) = (self.cast.caster, self.at);
        for p in self.players.values().filter(|p| p.occupant.id != me && distance(p.occupant.position, at) <= ATTACK_RANGE) {
            p.send(&packet);
        }
        self.mine(packet);
    }

    /// A packet for the players in the caster's view (`CZone::SendView`), the caster's in `out`.
    fn in_view(&mut self, packet: Vec<u8>) {
        let (me, cell) = (self.cast.caster, self.cell);
        for p in self.players.values().filter(|p| p.occupant.id != me && in_view(p.cell, cell)) {
            p.send(&packet);
        }
        self.mine(packet);
    }

    /// A packet for the players within 60 of a spot (`CZone::PSendView`), the caster's in `out`.
    fn around(&mut self, at: [f32; 3], packet: Vec<u8>) {
        let me = self.cast.caster;
        let mut mine = false;
        for p in self.players.values().filter(|p| distance(p.occupant.position, at) <= ATTACK_RANGE) {
            if p.occupant.id == me {
                mine = true;
            } else {
                p.send(&packet);
            }
        }
        if mine {
            self.mine(packet);
        }
    }

    /// A packet for one player, the caster's in `out`.
    fn to(&mut self, id: u32, packet: Vec<u8>) {
        if id == self.cast.caster {
            self.mine(packet);
        } else if let Some(p) = self.players.get(&id) {
            p.send(&packet);
        }
    }

    /// Tells a player's connection that its applied skills changed.
    fn buffs_changed(&self, id: u32) {
        if let Some(p) = self.players.get(&id) {
            let _ = p.occupant.events.send(ZoneEvent::Buffs);
        }
    }

    /// Applied skills taken off a player (section 6): `0x050E` each, a heal over time paid.
    fn end_applied(&mut self, id: u32, ended: Vec<Applied>) {
        for a in ended {
            self.to(id, sk::applied_end(a.key));
            let now = self.now;
            let Some(p) = self.players.get_mut(&id) else { continue };
            let at = p.occupant.position;
            if let Some(packet) = heal_over_time(p, &a, now) {
                self.around(at, packet);
            }
        }
    }

    /// A buff on a player (`CUser::BuffUser`, `docs/original/skills-buffs.md` section 4): `0x0511` (`0x0513` through a party type)
    /// within 60 of the caster, a heal at once for `wType` 81, the skill applied, and a buff's aggro.
    fn buff(&mut self, target: u32, single: bool) {
        let (s, caster) = (self.cast.skill, self.cast.caster);
        let mut values = [0; 3];
        if s.kind == buffs::HEAL_OVER_TIME {
            let mut heal = s.heal;
            if heal[0] != 0 {
                heal[0] = heal[0].saturating_add((self.cast.fighter.wis.max(0) * 2).min(i32::from(u16::MAX)) as u16);
            }
            if let Some(p) = self.players.get_mut(&target) {
                values = heal_player(p, heal);
            }
        }
        let opcode = if single { sk::OP_USE_USER } else { sk::OP_USE_PARTY };
        self.around_caster(sk::skill_use(opcode, combat::RESULT_HIT, caster, target, s.id, s.level, values));
        self.apply_to(target);
        // The monster the target fights turns on the caster (`CMob::Aggro`). Only the caster's own fight is known here, and the
        // original's status check, which keeps this from ever happening, is left out (deviation, section 4 step 3).
        if target == caster
            && s.aggro != 0
            && s.kind != buffs::HEAL_OVER_TIME
            && let Some(mob) = self.cast.fighting
            && self.mob_alive(mob)
        {
            self.mobs.add_damage(mob, caster, 0, u32::from(s.aggro), self.zones.party_id(caster));
            self.retarget(mob);
        }
    }

    /// Whom a party heal or buff reaches (`docs/original/party.md` section 10): the members on this map that the caster may help
    /// (itself, or alive and of its country) within the skill's `byApplyRange`, the caster only when `with_caster`; without a party
    /// the caster alone (`with_caster`, when alive) or nobody.
    /// In a union, `whole_union` (the heal with the caster, and every buff: deviation 6) reaches every member, the heal without the
    /// caster only the caster's group (`docs/original/union.md` section 9).
    fn party_members(&self, with_caster: bool, whole_union: bool) -> Vec<u32> {
        let (me, at, range) = (self.cast.caster, self.at, f32::from(self.cast.skill.apply_range));
        let alive = |id: u32| self.players.get(&id).is_some_and(|p| !p.dead);
        let group = if whole_union { None } else { self.zones.union_group(me) };
        let members = group.or_else(|| self.zones.parties.guard().party_of(me).map(|p| p.members.clone()));
        let Some(members) = members else { return if with_caster && alive(me) { vec![me] } else { Vec::new() } };
        members
            .into_iter()
            .filter(|&m| with_caster || m != me)
            .filter(|&m| {
                // `CUser::IsTeamUsable`: nobody helps or is helped by a duellist but itself (`docs/original/duel.md` section 5), and
                // help stays within one side of a guild war (`docs/original/guild-war.md` section 5).
                let duelling = self.players.get(&me).is_some_and(duel::in_fight);
                let war = self.players.get(&me).and_then(|p| p.war).map_or(0, |w| w.enemy);
                self.players.get(&m).is_some_and(|p| {
                    let side = crate::guild_war::may_help(war, p.war.map_or(0, |w| w.enemy));
                    !p.dead
                        && (m == me || (p.occupant.shape.country == self.cast.country && !duelling && !duel::in_fight(p) && side))
                        && distance(p.occupant.position, at) <= range
                })
            })
            .collect()
    }

    /// Re-targets a fighting monster by aggro.
    fn retarget(&mut self, mob: u32) {
        let Some(m) = self.mobs.mobs.get(&mob) else { return };
        let Some(def) = self.defs.get(m.mob_id) else { return };
        if matches!(m.state, MobState::Fighting(_)) {
            retarget(self.mobs, self.players, mob, def, self.now);
        }
    }

    /// The skill applied to a player's list (`CUser::AddApplySkill`, section 3): what it pushed off ends, `0x050D`, a new
    /// taunt-around buff taunts, a transformation takes its monster or copied looks, a new Detect looks for the hidden, and the
    /// shape follows (`docs/original/skills-special.md` sections 3 to 5).
    fn apply_to(&mut self, target: u32) {
        let s = self.cast.skill;
        if buffs::refused_on_map(self.map, s) {
            return;
        }
        let (zones, now, caster) = (self.zones, self.now, self.cast.caster);
        let Some(p) = self.players.get_mut(&target) else { return };
        // Not hiding or a transformation on a player with a personal shop (`CUser::BuffUser`, `docs/original/personal-shop.md`
        // section 6).
        if p.shop.is_some() && matches!(s.kind, buffs::HIDE | buffs::TRANSFORMATION) {
            return;
        }
        let applying = p.occupant.buffs.apply(s, caster, now, &mut || zones.new_key());
        if let Some(key) = applying.set {
            p.occupant.buffs.mark_attacker(key, true);
        }
        // A new disguise over an old one keeps the shape: only the looks change.
        let mut new_looks = false;
        if let (true, Some(key)) = (s.kind == skills::TRANSFORMATION, applying.set) {
            if s.change_type == buffs::SHAPE_ILLUSION {
                p.occupant.buffs.set_mob(key, self.cast.illusion_mob);
            }
            if s.change_type == buffs::SHAPE_DISGUISE && applying.fresh {
                new_looks = p.form.shape == sk::SHAPE_DISGUISE;
                p.disguise = self.cast.look.map(|look| UserShape { id: target, ..look.clone() });
            }
        }
        self.end_applied(target, applying.ended);
        if let Some(key) = applying.set {
            self.to(target, sk::applied(key, s.id, s.level));
        }
        if applying.fresh && s.kind == buffs::TAUNT_AROUND {
            self.taunt_around();
        }
        reform(self.players, target, false);
        if new_looks && let Some(p) = self.players.get(&target) {
            let (looks, cell) = (p.looks_packet(), p.cell);
            self.players.values().filter(|o| in_view(o.cell, cell)).for_each(|o| o.send(&looks));
        }
        if applying.fresh && s.kind == buffs::DETECT {
            reveal_around(self.players, target, f32::from(s.apply_range));
        }
        self.buffs_changed(target);
    }

    /// A field (`CUser::SkillField`, `docs/original/skills-special.md` section 8): `0x0511` within 60, and a trap or fire wall at the
    /// caster's feet until its time is up.
    fn field(&mut self) {
        let (s, caster) = (self.cast.skill, self.cast.caster);
        if !matches!(s.kind, skills::TRAP | skills::FIRE_WALL) {
            return;
        }
        self.around_caster(sk::skill_use(sk::OP_USE_USER, combat::RESULT_HIT, caster, 0, s.id, s.level, [0; 3]));
        let field = Field {
            id: self.zones.last_field_id.fetch_add(1, Ordering::Relaxed) + 1,
            owner: caster,
            country: self.cast.country,
            owner_who: self.players.get(&caster).map(pvp::who).unwrap_or(crate::rules::Who { level: 1, kills: 0, country: self.cast.country }),
            skill: *s,
            position: self.at,
            cell: self.cell,
            ends: self.now + Duration::from_secs(u64::from(s.keep_time)),
        };
        self.zones.fields.guard().entry(self.map).or_default().push(field);
    }

    /// A one-off skill (`CUser::SkillOnce`, `docs/original/skills-special.md` section 9): resurrection, scouting, return. The hit
    /// roll by `bySuccessType` 1 (every row has it); a failure is result 3.
    fn once(&mut self, target: u32) {
        let (s, caster) = (self.cast.skill, self.cast.caster);
        let success = s.success_type != 1 || (self.rand)() % 100 < u32::from(s.success_value);
        let result = if success { combat::RESULT_HIT } else { combat::RESULT_MISS };
        match s.kind {
            skills::RESURRECTION => {
                let Some(p) = self.players.get(&target) else { return };
                if !p.dead {
                    self.mine(sk::skill_use(sk::OP_USE_USER, skills::RESULT_WRONG_TARGET, caster, 0, s.id, s.level, [0; 3]));
                    return;
                }
                let events = p.occupant.events.clone();
                self.around_caster(sk::skill_use(sk::OP_USE_USER, result, caster, target, s.id, s.level, [0; 3]));
                if success {
                    let _ = events.send(ZoneEvent::Resurrected { position: self.at, caster, skill: s.id });
                }
            }
            skills::SCOUT => {
                let Some(p) = self.players.get(&target) else { return };
                let (country, element, level, grow) =
                    (p.occupant.shape.country, p.occupant.vitals.guard_element, p.occupant.vitals.guard.level, p.occupant.shape.grow);
                let result = if success && country == self.cast.country { skills::RESULT_WRONG_TARGET } else { result };
                self.around_caster(sk::skill_use(sk::OP_USE_USER, result, caster, target, s.id, s.level, [0; 3]));
                if result == combat::RESULT_HIT {
                    self.mine(sk::scouting(i32::from(element), level, grow));
                }
            }
            skills::RETURN => {
                self.around_caster(sk::skill_use(sk::OP_USE_USER, combat::RESULT_HIT, caster, 0, s.id, s.level, [0; 3]));
                self.recall = true;
            }
            _ => {}
        }
    }

    /// `CUser::TauntMob` (section 5.2): the living monsters in the 3 x 3 cells around the caster and within `byApplyRange` take it as
    /// their target and get `wAggro + REC × 10` aggro.
    fn taunt_around(&mut self) {
        let (s, caster) = (self.cast.skill, self.cast.caster);
        let reach = f32::from(s.apply_range);
        let (cell, at) = (self.cell, self.at);
        let ids: Vec<u32> = (-NEAR_CELLS..=NEAR_CELLS)
            .flat_map(|dx| (-NEAR_CELLS..=NEAR_CELLS).map(move |dz| (cell.0 + dx, cell.1 + dz)))
            .filter_map(|c| self.mobs.cells.get(&c))
            .flatten()
            .filter_map(|id| self.mobs.mobs.get(id))
            .filter(|m| m.dead_until.is_none() && distance(m.position, at) <= reach)
            .map(|m| m.id)
            .collect();
        for id in ids {
            set_target(self.mobs, self.players, id, caster, true, self.defs, self.now, &mut *self.rand);
            let aggro = i32::from(s.aggro) + self.cast.rec * 10;
            if aggro > 0 && self.mob_alive(id) {
                self.mobs.add_damage(id, caster, 0, aggro as u32, self.zones.party_id(caster));
                self.retarget(id);
            }
        }
    }

    /// A sleeping monster hit by a skill wakes up (`CMob::RemApplySkillSleep`): its sleep debuffs go without their end.
    fn wake(&mut self, id: u32) {
        wake(self.players, self.mobs, id);
    }

    /// `0x0609` within 60 of a monster whose speeds changed.
    fn speeds_changed(&mut self, id: u32, before: (u8, u8)) {
        let after = self.mobs.speeds(id);
        if after != before
            && let Some(m) = self.mobs.mobs.get(&id)
        {
            let at = m.position;
            self.around(at, cp::mob_speed(id, after.0, after.1));
        }
    }

    /// A debuff that did not kill stays on the monster (`CMob::AddApplySkill`, section 12): what it pushed off ends with its end
    /// effect, a taunt turns the monster on the caster.
    fn debuff(&mut self, id: u32) {
        let (s, caster, now) = (self.cast.skill, self.cast.caster, self.now);
        let before = self.mobs.speeds(id);
        let applying = buffs::apply_mob(self.mobs.debuffs.entry(id).or_default(), s, caster, now);
        // `0x0A04`: a debuff added to the monster, with the player and the skill.
        if let Some(mob) = self.mobs.mobs.get(&id) {
            let who = self.players.get(&caster).map(|p| String::from_utf8_lossy(&p.occupant.shape.name).trim_end_matches('\0').to_owned());
            let log = self.zones.game_log();
            self.zones.mob_record(self.map, mob, 0, crate::game_log::MobAction::DebuffAdd, who.as_deref(), Some(log.skill_name(s.id)));
        }
        for d in applying.ended {
            self.end_debuff(id, &d);
        }
        if applying.taunt
            && let Some(m) = self.mobs.mobs.get_mut(&id)
            && m.dead_until.is_none()
        {
            // A monster running its machine keeps it (deviation 4 of `docs/original/scripted-mobs.md`).
            m.state = match m.state {
                MobState::Scripted(_) => MobState::Scripted(Some(caster)),
                _ => MobState::Fighting(caster),
            };
            m.next_move = now;
        }
        self.speeds_changed(id, before);
    }

    /// A debuff's end on a monster during the caster's skill: its damage from the player who applied it, `0x0607` within 60.
    fn end_debuff(&mut self, id: u32, d: &MobDebuff) {
        let Some(mob) = self.mobs.mobs.get(&id).copied() else { return };
        let Some(def) = self.defs.get(mob.mob_id) else { return };
        if mob.dead_until.is_some() || mob.hp == 0 {
            return;
        }
        let Some(damage) = debuff_end_damage(d, &mob, def, self.now) else { return };
        let hit = Hit { attacker: d.attacker, damage, aggro: 0, direct: false };
        let killed =
            self.zones.damage_mob(self.map, self.players, self.mobs, id, &hit, self.defs, self.loot, self.now, self.rand, &mut self.out);
        self.killed |= killed && d.attacker == self.cast.caster;
        self.around(mob.position, sk::keep(sk::OP_KEEP_MOB, id, d.def.id, d.def.level, shown(damage)));
    }

    fn mob_alive(&self, id: u32) -> bool {
        self.mobs.mobs.get(&id).is_some_and(|m| m.dead_until.is_none() && m.hp > 0)
    }

    /// `CUser::SkillAttackSelect` on a monster (section 8): a multi-hit announced (`0x0517` result 4) and its hits, any other kind
    /// one hit.
    fn select_mob(&mut self, id: u32) {
        let (s, caster) = (self.cast.skill, self.cast.caster);
        if s.kind == skills::MULTI_HIT {
            self.around_caster(sk::skill_use(sk::OP_USE_MOB, skills::RESULT_AREA, caster, id, s.id, s.level, [0; 3]));
            self.hits(id);
        } else {
            self.hit(id, true);
        }
    }

    /// `CUser::SkillAttackSelect` on a player (`docs/original/pvp-2.md` section 2), as on a monster with `0x0511`.
    fn select_player(&mut self, id: u32) {
        let (s, caster) = (self.cast.skill, self.cast.caster);
        if s.kind == skills::MULTI_HIT {
            self.around_caster(sk::skill_use(sk::OP_USE_USER, skills::RESULT_AREA, caster, id, s.id, s.level, [0; 3]));
            self.hits_player(id);
        } else {
            self.hit_player(id, true);
        }
    }

    /// The lapis skills after a hit that landed (`docs/original/item-options.md` section 5): the caster's weapon on the target, then,
    /// on a player, that player's armour back at the caster. The caster's packets join its own; the target's go to its queue.
    fn options(&mut self, target: SkillTarget) {
        if !self.options {
            return;
        }
        let (caster, war_map) = (self.cast.caster, self.cast.war_map);
        let mut pass = Pass { zones: self.zones, map: self.map, defs: self.defs, loot: self.loot, now: self.now, rand: &mut *self.rand };
        let mine = item_options(&mut pass, self.mobs, self.players, caster, target, Worn::Weapon, war_map, self.queue);
        self.out.extend(mine.out);
        self.killed |= mine.killed;
        self.criticals = self.criticals.saturating_add(mine.criticals);
        if let SkillTarget::Player(id) = target {
            item_options(&mut pass, self.mobs, self.players, id, SkillTarget::Player(caster), Worn::Armour, war_map, true);
        }
        self.died |= !self.player_alive(caster);
    }

    /// `byMultiAttack` area hits on one monster, stopping when it or the caster dies (`wType` 22).
    fn hits(&mut self, id: u32) {
        for _ in 0..self.cast.skill.multi_attack {
            if !self.mob_alive(id) || self.died {
                break;
            }
            self.hit(id, false);
        }
    }

    /// One hit on a monster (`CUser::AttackMob(CMob*, stSkillInfo*, single, option)`, section 8): `0x0517` on its target
    /// (`single`), `0x0519` in an area; the drain only on its target.
    fn hit(&mut self, id: u32, single: bool) {
        let (s, caster) = (self.cast.skill, self.cast.caster);
        let Some(mob) = self.mobs.mobs.get(&id).copied() else { return };
        let Some(def) = self.defs.get(mob.mob_id) else { return };
        if mob.dead_until.is_some() || mob.hp == 0 {
            return;
        }
        let drain = single && s.type_effect == skills::ATTACK && s.kind == skills::DRAIN;
        let opcode = if single { sk::OP_USE_MOB } else { sk::OP_AREA_MOB };
        let effects = self.mobs.effects(id);
        let fighter = scripted::scripted_fighter(combat::monster_with(def, &effects), def, self.mobs.script_flags(id), effects.no_element);
        // A monster going home or unbeatable cannot be hit (`CUser::AttackSuccess`).
        let hits = !lua::shielded(self.mobs, &mob)
            && scripted::takes_attack_kind(self.mobs, id, s.attack_kind)
            && combat::skill_hits(self.cast.fighter, &fighter, def, s, self.rand);
        if !hits {
            let packet = if drain {
                sk::drain(combat::RESULT_MISS, caster, id, s.id, s.level, [0; 3], [0; 3])
            } else {
                sk::skill_use(opcode, combat::RESULT_MISS, caster, id, s.id, s.level, [0; 3])
            };
            self.around_caster(packet);
            set_target(self.mobs, self.players, id, caster, true, self.defs, self.now, &mut *self.rand);
            scripted::mark_attacked(self.mobs, id, true);
            return;
        }
        self.wake(id);
        let Some(me) = self.players.get(&caster) else { return };
        let v = me.occupant.vitals;
        let close = distance(self.at, mob.position) <= CLOSE_RANGE && !self.cast.cross_road;
        let blow = combat::skill_damage(
            self.cast.fighter,
            &fighter,
            def,
            [mob.hp, u32::from(mob.sp), u32::from(mob.mp)],
            [v.health[0], v.health[2], v.health[1]],
            s,
            close,
            self.rand,
        );
        // A critical wears the weapon before the damage (`CUser::QualityByAttack`).
        if blow.result == combat::RESULT_CRITICAL {
            self.criticals = self.criticals.saturating_add(1);
        }
        let aggro = self.cast.rec.max(0) as u32 * 10 + u32::from(s.aggro);
        let mut killed = self.damage(id, blow.damage, aggro);
        if drain {
            let healed = self.players.get_mut(&caster).map_or([0; 3], |me| heal_player(me, blow.damage));
            self.around_caster(sk::drain(blow.result, caster, id, s.id, s.level, blow.damage, healed));
        } else {
            self.around_caster(sk::skill_use(opcode, blow.result, caster, id, s.id, s.level, blow.damage));
            if !killed && s.add_damage != [0; 3] {
                killed = self.damage(id, s.add_damage, aggro);
                self.in_view(sk::add_damage(caster, id, s.id, s.level, s.add_damage));
            }
        }
        if !killed {
            set_target(self.mobs, self.players, id, caster, true, self.defs, self.now, &mut *self.rand);
            scripted::mark_attacked(self.mobs, id, false);
            if s.type_effect == skills::DEBUFF {
                self.debuff(id);
            }
        }
        self.options(SkillTarget::Mob(id));
    }

    /// Whether the caster may harm the player `id` (`IsTeam` false, alive, not hidden; `docs/original/pvp-2.md` sections 1 and 4).
    fn enemy(&self, id: u32) -> bool {
        let (Some(me), Some(t)) = (self.players.get(&self.cast.caster), self.players.get(&id)) else { return false };
        id != self.cast.caster && pvp::attackable(me, t, self.cast.war_map, &self.rules, self.map).is_yes()
    }

    /// `CUser::Damage(victim, 1, by, damage)` during the skill (the record, a death by the killer of record): a death's `0x0504`
    /// reaches the caster among its packets, in order. A caster that dies of it is `died`.
    fn hurt(&mut self, by: u32, victim: u32, damage: [u16; 3]) -> ([u16; 3], bool) {
        let caster = self.cast.caster;
        let sees = match (self.players.get(&caster), self.players.get(&victim)) {
            (Some(c), Some(v)) => in_view(c.cell, v.cell),
            _ => false,
        };
        let mut pass = Pass { zones: self.zones, map: self.map, defs: self.defs, loot: self.loot, now: self.now, rand: &mut *self.rand };
        let (shown, death) =
            mob_skills::hurt_player_skip(&mut pass, self.mobs, self.players, (cp::KILLER_PLAYER, by), victim, damage, Some(caster));
        let died = death.is_some();
        if let Some(death) = death.filter(|_| sees) {
            self.mine(death);
        }
        if died && victim == caster {
            self.died = true;
        }
        (shown, died)
    }

    fn player_alive(&self, id: u32) -> bool {
        self.players.get(&id).is_some_and(|p| !p.dead)
    }

    /// `byMultiAttack` area hits on one player, stopping when it or the caster dies (`wType` 22).
    fn hits_player(&mut self, id: u32) {
        for _ in 0..self.cast.skill.multi_attack {
            if !self.player_alive(id) || !self.player_alive(self.cast.caster) {
                break;
            }
            self.hit_player(id, false);
        }
    }

    /// One skill hit on a player (`CUser::AttackUser(CUser*, stSkillInfo*, single, option)`, `docs/original/pvp-2.md` section 3):
    /// `0x0511` on its target (`0x0512` a drain), `0x0513` in an area; the additional damage `0x0514`, a reflection `0x0515`, then
    /// a debuff stays on the target unless it died.
    fn hit_player(&mut self, id: u32, single: bool) {
        let (s, caster) = (self.cast.skill, self.cast.caster);
        if !self.player_alive(id) || !self.player_alive(caster) {
            return;
        }
        let drain = single && s.type_effect == skills::ATTACK && s.kind == skills::DRAIN;
        let opcode = if single { sk::OP_USE_USER } else { sk::OP_USE_PARTY };
        let p = self.players.get_mut(&id).expect("alive");
        if p.occupant.shape.motion == w::MOTION_SIT {
            p.occupant.shape.motion = w::MOTION_STAND;
            let _ = p.occupant.events.send(ZoneEvent::StoodUp);
        }
        let block = p.occupant.buffs.block(s.attack_kind, &mut *self.rand);
        if let Some(used_up) = block.used_up {
            p.send(&sk::applied_end(used_up.key));
            let _ = p.occupant.events.send(ZoneEvent::Buffs);
        }
        // A cure item's protection against state 7 or 8 makes it miss, used up; then a lapis that blocks the state.
        let protected = !block.blocked && p.occupant.buffs.protected(s.state_type);
        let protected = protected || (!block.blocked && s.state_type < 16 && p.occupant.vitals.state_blocks & (1 << s.state_type) != 0);
        let guard = p.occupant.vitals.guard;
        if block.blocked || protected || !combat::pvp_skill_hits(self.cast.fighter, &guard, s, &mut *self.rand) {
            let packet = if drain {
                sk::drain_user(combat::RESULT_MISS, caster, id, s.id, s.level, [0; 3], [0; 3])
            } else {
                sk::skill_use(opcode, combat::RESULT_MISS, caster, id, s.id, s.level, [0; 3])
            };
            self.around_caster(packet);
            return;
        }
        mob_skills::struck(self.players, id);
        duel::hit(self.zones, self.map, self.players, id, Some(caster), self.now);
        let t = &self.players[&id];
        let kind = usize::from(s.attack_kind).min(combat::MAGIC);
        let reflected = t.occupant.buffs.reflects(kind, guard.luc, guard.wis, &mut *self.rand).is_some();
        let (v, at) = (t.occupant.vitals, t.occupant.position);
        let mine = self.players.get(&caster).map_or([0; 3], |me| me.occupant.vitals.health);
        let numbers = combat::PvpSkillHit {
            caster: self.cast.fighter,
            target: &guard,
            defence_element: v.guard_element,
            caster_health: [mine[0], mine[2], mine[1]],
            target_health: [v.health[0], v.health[2], v.health[1]],
            target_max: [v.max[0], v.max[2], v.max[1]],
            within_three: distance(self.at, at) <= CLOSE_RANGE,
            cross_road: self.cast.cross_road,
        };
        let blow = combat::pvp_skill_damage(&numbers, s, &mut *self.rand);
        // A critical wears the weapon (`CUser::QualityByAttack`).
        if blow.result == combat::RESULT_CRITICAL {
            self.criticals = self.criticals.saturating_add(1);
        }
        let (shown, mut killed) = if reflected { ([0; 3], false) } else { self.hurt(caster, id, blow.damage) };
        if drain {
            // The caster heals by the damage, also after a kill.
            let healed = if reflected { [0; 3] } else { self.players.get_mut(&caster).map_or([0; 3], |me| heal_player(me, blow.damage)) };
            self.around_caster(sk::drain_user(blow.result, caster, id, s.id, s.level, shown, healed));
        } else {
            self.around_caster(sk::skill_use(opcode, blow.result, caster, id, s.id, s.level, shown));
            // The additional damage, also after a reflection.
            if !killed && s.add_damage != [0; 3] {
                let (added, died) = self.hurt(caster, id, s.add_damage);
                killed = died;
                self.in_view(sk::add_damage_user(caster, id, s.id, s.level, added));
            }
        }
        // A kill stops the caster's attack (`CUser::StatusAttackStop`).
        self.killed |= killed;
        if killed {
            return;
        }
        if reflected {
            let (back, _) = self.hurt(id, caster, blow.damage);
            self.in_view(cp::mirror_user(id, caster, back));
        }
        if s.type_effect == skills::DEBUFF && self.player_alive(id) {
            self.apply_to(id);
        }
        self.options(SkillTarget::Player(id));
    }

    /// The enemy players in an area (`docs/original/pvp-2.md` section 4): in the 5 x 5 cells around `centre_cell`, alive, not of the
    /// caster's side, within `byApplyRange` of `centre`; in id order.
    fn players_around(&self, centre: [f32; 3], centre_cell: (i32, i32)) -> Vec<u32> {
        let reach = f32::from(self.cast.skill.apply_range);
        let mut ids: Vec<u32> = self
            .players
            .values()
            .filter(|p| in_view(p.cell, centre_cell) && distance(p.occupant.position, centre) <= reach)
            .map(|p| p.occupant.id)
            .filter(|&id| self.enemy(id))
            .collect();
        ids.sort_unstable();
        ids
    }

    /// `CMob::Damage` from the caster; a kill stops it.
    fn damage(&mut self, id: u32, damage: [u16; 3], aggro: u32) -> bool {
        let hit = Hit { attacker: self.cast.caster, damage: damage.map(u32::from), aggro, direct: true };
        let killed =
            self.zones.damage_mob(self.map, self.players, self.mobs, id, &hit, self.defs, self.loot, self.now, self.rand, &mut self.out);
        self.killed |= killed;
        killed
    }

    /// An area (section 9, `docs/original/pvp-2.md` section 4): the animation to the players within 60, then the enemy players and
    /// every living monster of another country within `byApplyRange` of `centre` in the 5 x 5 cells around `centre_cell`.
    fn area(&mut self, centre: [f32; 3], centre_cell: (i32, i32), announce: Vec<u8>) {
        self.around_caster(announce);
        let s = self.cast.skill;
        for id in self.players_around(centre, centre_cell) {
            match (s.type_effect, s.kind) {
                (skills::ATTACK, skills::MULTI_HIT) => self.hits_player(id),
                (skills::ATTACK, skills::DRAIN) => {}
                _ => self.hit_player(id, false),
            }
        }
        let reach = f32::from(s.apply_range);
        let ids: Vec<u32> = view_cells(centre_cell)
            .filter_map(|cell| self.mobs.cells.get(&cell))
            .flatten()
            .filter_map(|id| self.mobs.mobs.get(id))
            .filter(|m| m.dead_until.is_none() && m.hp > 0 && distance(m.position, centre) <= reach)
            .filter(|m| self.defs.get(m.mob_id).is_some_and(|d| d.country != self.cast.country))
            .map(|m| m.id)
            .collect();
        for id in ids {
            match (s.type_effect, s.kind) {
                (skills::ATTACK, skills::MULTI_HIT) => self.hits(id),
                (skills::ATTACK, skills::DRAIN) => {}
                _ => self.hit(id, false),
            }
        }
    }

    /// A heal on a player (`CUser::HealUser`, section 10): `wType` 41 heals, 43 gives the caster's maxima.
    fn heal(&mut self, target: u32, single: bool) {
        let (s, caster) = (self.cast.skill, self.cast.caster);
        let opcode = if single { sk::OP_USE_USER } else { sk::OP_USE_PARTY };
        let Some(max) = self.players.get(&caster).map(|me| me.occupant.vitals.max) else { return };
        let amounts = match s.kind {
            skills::HEAL_HEALTH => {
                let mut heal = s.heal;
                if heal[0] != 0 {
                    heal[0] = heal[0].saturating_add((self.cast.fighter.wis.max(0) * 2).min(i32::from(u16::MAX)) as u16);
                }
                heal
            }
            // Level 1 SP, level 2 SP and MP, level 3 HP, each the caster's maximum (HP, SP, MP order; Vitals are HP, MP, SP).
            skills::SACRIFICE => match s.level {
                1 => [0, max[2], 0],
                2 => [0, max[2], max[1]],
                _ => [max[0], 0, 0],
            },
            // `CUser::RemStateSkill` by the dispel's level (`docs/original/skills-buffs.md` section 9).
            skills::DISPEL => {
                let Some(p) = self.players.get_mut(&target) else { return };
                let gone = p.occupant.buffs.dispel(s.level);
                self.around_caster(sk::skill_use(opcode, combat::RESULT_HIT, caster, target, s.id, s.level, [0; 3]));
                if !gone.is_empty() {
                    for a in gone {
                        self.to(target, sk::applied_end(a.key));
                    }
                    reform(self.players, target, false);
                    self.buffs_changed(target);
                }
                return;
            }
            _ => return,
        };
        let Some(p) = self.players.get_mut(&target) else { return };
        let healed = heal_player(p, amounts);
        let packet = sk::skill_use(opcode, combat::RESULT_HIT, caster, target, s.id, s.level, healed);
        if s.kind == skills::SACRIFICE { self.in_view(packet) } else { self.around_caster(packet) }
    }

    /// The desperate counterattack's price (`wType` 23): the caster loses all its HP (`CUser::Damage(own HP)`, its own entry in the
    /// damage record) and dies (`CUser::Death(1, own id)`): the killer of record decides who killed it (`docs/original/pvp.md`
    /// section 6). The caster's `0x0504` is in `out`.
    fn give_up_life(&mut self) {
        let caster = self.cast.caster;
        let Some(me) = self.players.get_mut(&caster) else { return };
        if me.dead {
            return;
        }
        pvp::record(&mut me.harm, cp::KILLER_PLAYER, caster, me.occupant.vitals.health[0]);
        let before = me.occupant.buffs.effects();
        me.occupant.vitals.health = [0; 3];
        me.dead = true;
        me.attacking = false;
        self.died = true;
        let (by, flag) = ((cp::KILLER_PLAYER, caster), self.duel_flag);
        let death = pvp::player_died(self.zones, self.map, self.mobs, self.players, caster, by, &before, Some(caster), flag, self.now);
        self.mine(death);
    }

    /// The costs (`CUser::SendUseSPMP`, section 7 step 3): SP and MP lowered, `0x050C` to the caster.
    fn pay(&mut self) {
        let s = self.cast.skill;
        if s.need_sp == 0 && s.need_mp == 0 {
            return;
        }
        let Some(me) = self.players.get_mut(&self.cast.caster) else { return };
        let v = &mut me.occupant.vitals;
        let (sp, mp) = skills::spend(s, v.health[2], v.health[1], self.cast.cost_reduction);
        v.health[2] -= sp;
        v.health[1] -= mp;
        self.mine(sk::costs(sp, mp));
    }
}

/// The 5 x 5 cells around a cell.
/// When a full spawn entry may spawn again after one of its monsters went (`CZone::UpdateMobDeath`): at once without a respawn
/// time; after it, varied by up to an hour either way from respawn code 9 on.
fn respawn_due(def: &MobDef, now: Instant, rand: &mut impl FnMut() -> u32) -> Instant {
    if def.respawn.is_zero() {
        now
    } else if def.respawn_code >= 9 {
        let minutes = i64::from(rand() % 120) - 60;
        let due = now + def.respawn;
        if minutes >= 0 { due + Duration::from_secs(minutes as u64 * 60) } else { due - Duration::from_secs(minutes.unsigned_abs() * 60) }
    } else {
        now + def.respawn
    }
}

fn view_cells(view: (i32, i32)) -> impl Iterator<Item = (i32, i32)> {
    (-VIEW_CELLS..=VIEW_CELLS).flat_map(move |dx| (-VIEW_CELLS..=VIEW_CELLS).map(move |dz| (view.0 + dx, view.1 + dz)))
}

impl Zones {
    /// Zones with the world's NPCs placed in their cells and the monster spawn areas ready (nothing spawned yet).
    pub fn new(world: &World, mob_defs: &MobDefs) -> Self {
        let (npcs, last_npc_id) = npc::build(world);
        // Maps with a CreateType have no static zone (`CWorld::InitZones`): their zones are made for a party or guild.
        // Every static map with spawn areas or boss groups (`docs/original/obelisks.md` section 11.1).
        let mut mobs = world
            .spawn_areas()
            .keys()
            .chain(world.boss_groups().maps.keys())
            .copied()
            .filter(|&map| world.create_type(map) == 0)
            .map(|map| (map, Self::new_mob_map(map, world, mob_defs)))
            .collect();
        Self::place_obelisks(&mut mobs, world);
        Self {
            maps: Mutex::default(),
            mobs: Mutex::new(mobs),
            last_mob_id: AtomicU32::new(0),
            npcs: Mutex::new(npcs),
            last_npc_id: AtomicU32::new(last_npc_id),
            ground: Mutex::default(),
            last_item_id: AtomicU32::new(0),
            last_skill_key: AtomicU32::new(0),
            fields: Mutex::default(),
            last_field_id: AtomicU32::new(0),
            parties: Mutex::default(),
            trades: Mutex::default(),
            guilds: Mutex::default(),
            guild_bank_turn: tokio::sync::Mutex::new(()),
            friends: Mutex::default(),
            party_calls: Mutex::default(),
            sales: Mutex::default(),
            wars: Mutex::default(),
            item_skills: OnceLock::new(),
            game_log: OnceLock::new(),
            scripted_ais: world.mob_ai().machines.keys().copied().collect(),
            obelisk_events: Mutex::default(),
            portal_countries: Mutex::default(),
            instance_portal_writes: Mutex::default(),
            instances: Mutex::default(),
            ranking_map: world.ranking_map(),
            ranking: Mutex::default(),
            admin_orders: Mutex::default(),
            script_orders: Mutex::default(),
            bans: Mutex::default(),
            listens: Mutex::default(),
            quests: Mutex::default(),
            smooth: AtomicBool::new(false),
            rules: std::sync::RwLock::default(),
            scripts: std::sync::Arc::default(),
            script_jobs: OnceLock::new(),
            script_staff: Mutex::default(),
            ext_spawns: Mutex::default(),
        }
    }

    /// The rules in force now: one snapshot per request, blow, cast or field pass (`docs/plans/ext-2-rules.md` section 8).
    pub fn rules(&self) -> std::sync::Arc<crate::rules::RuleSet> {
        self.rules.read_guard().clone()
    }

    /// Puts `rules` in force (the start and `reload`); the players' next requests ask them.
    pub fn set_rules(&self, rules: crate::rules::RuleSet) {
        *self.rules.write_guard() = std::sync::Arc::new(rules);
    }

    /// Turns the pacing of running steps and the move tick on or off (`smooth_chase`).
    pub fn set_smooth_chase(&self, on: bool) {
        self.smooth.store(on, Ordering::Relaxed);
    }

    pub fn smooth_chase(&self) -> bool {
        self.smooth.load(Ordering::Relaxed)
    }

    /// The monsters' object id counter (`CObjectMgr::m_dwMobID`): the last id given out.
    pub fn last_mob_id(&self) -> u32 {
        self.last_mob_id.load(Ordering::Relaxed)
    }

    /// Continues the counter from a saved value (`CObjectMgr::InitMobID`).
    pub fn set_last_mob_id(&self, id: u32) {
        self.last_mob_id.store(id, Ordering::Relaxed);
    }

    /// Gives the zones the game log (once).
    pub fn set_game_log(&self, log: crate::game_log::GameLog) {
        let _ = self.game_log.set(log);
    }

    /// The game log the zones write to; off until `set_game_log`.
    pub fn game_log(&self) -> crate::game_log::GameLog {
        self.game_log.get().cloned().unwrap_or_default()
    }

    /// The skills lapis may set off: the level-100 rows. Only the first call counts.
    pub fn load_item_skills(&self, defs: &skills::SkillDefs) {
        let _ = self.item_skills.set(defs.level_100().map(|d| (d.id, *d)).collect());
    }

    /// The respawn pass (`CZone::UpdateMobRespawn`, `docs/original/mobs.md` section 3): every entry below its count whose time
    /// has come spawns one monster at a random spot of its area (`rand` like the C library's), if that spot is on the map and not
    /// blocked; then the obelisks (`docs/original/obelisks.md` section 3). The players in view get each new monster's `0x0601`.
    /// Returns how many spawned.
    pub fn respawn(&self, now: Instant, world: &World, defs: &MobDefs, rand: &mut impl FnMut() -> u32) -> usize {
        let mut spawned = Vec::new();
        let mut left = Vec::new();
        {
            let mut all = self.mobs.guard();
            for (&map, mobs) in all.iter_mut() {
                let areas = world.spawn_areas_of(map);
                for entry in 0..mobs.respawns.len() {
                    let r = &mobs.respawns[entry];
                    if r.current >= r.count || r.due.is_some_and(|due| due > now) {
                        continue;
                    }
                    let Some(def) = defs.get(r.mob_id) else { continue };
                    let Some(area) = areas.get(r.area) else { continue };
                    let position = area.spot(rand(), rand());
                    let (x, z) = (position[0].round_ties_even() as i32, position[2].round_ties_even() as i32);
                    if !world.is_safe_pos(map, position) || world.is_blocked(map, x, z) {
                        continue;
                    }
                    let mob = self.new_mob(def, position, entry, mobs::Origin::Area, now, rand);
                    let r = &mut mobs.respawns[entry];
                    r.current += 1;
                    r.due = Some(now + def.respawn);
                    mobs.insert(mob);
                    spawned.push((map, mob, mob.cell));
                }
                if !mobs.obelisks.is_empty() {
                    self.obelisk_respawn(map, mobs, now, world, defs, rand, &mut spawned);
                }
                if !mobs.bosses.is_empty() {
                    self.boss_respawn(map, mobs, now, world, defs, rand, &mut spawned, &mut left);
                }
            }
        }
        let maps = self.maps.guard();
        for (map, mob) in &left {
            obelisk::leave_view(maps.get(map), mob);
        }
        for (map, mob, cell) in &spawned {
            let Some(players) = maps.get(map) else { continue };
            let packets = mob_enter(mob, true);
            for p in players.values().filter(|p| in_view(p.cell, *cell)) {
                packets.iter().for_each(|packet| p.send(packet));
            }
        }
        spawned.len()
    }

    /// The idle pass (`CZone::UpdateMobIdle`, `docs/original/mobs.md` section 6, `docs/original/mob-fight.md` section 2): an idle
    /// aggressive monster looks for a target; once its turn has come, a monster away from home goes home, others may take a
    /// step, if it stays near home, the line to it is free and it is on the map. A monster changing cells leaves and enters the
    /// views around (`0x0602`/`0x0601`, still at the old spot), then everyone in the view of its cell gets `0x0603`. Returns how
    /// many moved.
    pub fn idle(&self, now: Instant, world: &World, defs: &MobDefs, rand: &mut impl FnMut() -> u32) -> usize {
        let maps = self.maps.guard();
        let mut all = self.mobs.guard();
        let mut moved = 0;
        for (&map, mobs) in all.iter_mut() {
            let players = maps.get(&map);
            let mut ids: Vec<u32> =
                mobs.mobs.values().filter(|m| m.state == MobState::Idle && m.dead_until.is_none()).map(|m| m.id).collect();
            // The original walks its list in spawn order; the order only decides who uses the random numbers first.
            ids.sort_unstable();
            for id in ids {
                let Some(mob) = mobs.mobs.get(&id).copied() else { continue };
                let Some(def) = defs.get(mob.mob_id) else { continue };
                // Called to help by an earlier monster of this pass.
                if mob.state != MobState::Idle {
                    continue;
                }
                // Aggressive monsters look for a target (CMob::Targeting).
                if mob.next_new_target.is_none_or(|t| t <= now) {
                    mobs.mobs.get_mut(&id).expect("listed").next_new_target = None;
                    if (def.ai == 1 || def.ai == 3)
                        && let Some(players) = players
                        && let Some(target) = find_target(&mob, def, players)
                    {
                        set_target(mobs, players, id, target, true, defs, now, rand);
                        continue;
                    }
                }
                if def.obelisk || mob.next_move > now {
                    continue;
                }
                let from_home = distance(mob.position, mob.home);
                let trail = mobs.trails.get(&id).is_some_and(|t| !t.is_empty());
                if (trail && from_home > 0.5) || from_home > mobs::HOME_RANGE {
                    mobs.mobs.get_mut(&id).expect("listed").state = MobState::Home;
                    continue;
                }
                let Some((step, wait)) = mobs::idle_turn(&mob, def, mobs.speeds(id).1, rand) else { continue };
                mobs.mobs.get_mut(&id).expect("listed").next_move = now + wait;
                let Some(to) = step else { continue };
                let line = |p: [f32; 3]| (p[0] as i32, p[2] as i32);
                if distance(to, mob.home) >= mobs::HOME_RANGE
                    || !world.is_line_free(map, line(mob.position), line(to))
                    || !world.is_safe_pos(map, to)
                {
                    continue;
                }
                walk_mob(mobs, players, id, to, 0);
                moved += 1;
            }
        }
        moved
    }

    /// Whether a player of `country` may attack the monster (`CUser::EnableAttackMob`): on the map, alive and not of that country.
    pub fn attackable(&self, map: u16, mob_id: u32, country: u8, defs: &MobDefs) -> bool {
        let all = self.mobs.guard();
        let Some(mob) = all.get(&map).and_then(|m| m.mobs.get(&mob_id)) else { return false };
        mob.dead_until.is_none() && defs.get(mob.mob_id).is_some_and(|def| def.country != country)
    }

    /// One normal attack (`CUser::AttackMob`, `docs/original/combat.md` section 3): range, hit or miss, damage. The players
    /// within 60 of the attacker get `0x0503`; on a kill the monster's view gets `0x0606` first and the player with the most
    /// damage is told (`ZoneEvent::Kill`).
    #[allow(clippy::too_many_arguments)]
    pub fn attack_mob(
        &self,
        map: u16,
        mob_id: u32,
        attack: &Attack,
        defs: &MobDefs,
        loot: (&DropTables, &ItemDefs),
        now: Instant,
        rand: &mut impl FnMut() -> u32,
    ) -> AttackOutcome {
        let mut maps = self.maps.guard();
        let mut all = self.mobs.guard();
        let (Some(players), Some(mobs)) = (maps.get_mut(&map), all.get_mut(&map)) else { return AttackOutcome::Gone };
        let Some(at) = players.get(&attack.attacker).map(|me| me.occupant.position) else { return AttackOutcome::Gone };
        let Some(mob) = mobs.mobs.get(&mob_id).copied() else { return AttackOutcome::Gone };
        let Some(def) = defs.get(mob.mob_id) else { return AttackOutcome::Gone };
        if mob.dead_until.is_some() || def.country == attack.country {
            return AttackOutcome::Gone;
        }
        let gap = distance(at, mob.position);
        if !combat::in_range(gap, attack.slack, def.size, attack.range) {
            return AttackOutcome::OutOfRange;
        }
        // A monster going home cannot be hit (`CUser::AttackSuccess`), nor one whose script turned this attack kind off
        // (`docs/original/scripted-mobs.md` deviation 5).
        let shielded = lua::shielded(mobs, &mob) || !scripted::takes_attack_kind(mobs, mob_id, attack.kind as u8);
        let blow = if shielded {
            combat::Blow { result: combat::RESULT_MISS, damage: 0 }
        } else {
            let no_element = mobs.effects(mob_id).no_element;
            let fighter = scripted::scripted_fighter(combat::monster(def), def, mobs.script_flags(mob_id), no_element);
            combat::strike(attack.fighter, &fighter, attack.kind, gap <= CLOSE_RANGE && !attack.cross_road, rand)
        };
        let mut packets = Vec::new();
        let mut killed = false;
        if blow.result != combat::RESULT_MISS {
            let aggro = attack.rec.max(0) as u32 * 10;
            let hit = Hit { attacker: attack.attacker, damage: [u32::from(blow.damage), 0, 0], aggro, direct: true };
            killed = self.damage_mob(map, players, mobs, mob_id, &hit, defs, loot, now, rand, &mut packets);
        }
        let packet = cp::attack_mob(blow.result, attack.attacker, mob_id, blow.damage);
        for p in players.values().filter(|p| p.occupant.id != attack.attacker && distance(p.occupant.position, at) <= ATTACK_RANGE) {
            p.send(&packet);
        }
        packets.push(packet);
        let mut critical = blow.result == combat::RESULT_CRITICAL;
        if blow.result != combat::RESULT_MISS && !killed {
            set_target(mobs, players, mob_id, attack.attacker, true, defs, now, rand);
            scripted::mark_attacked(mobs, mob_id, false);
            // The weapon's lapis skills (`docs/original/item-options.md` section 5).
            let mut pass = Pass { zones: self, map, defs, loot, now, rand: &mut *rand };
            let mine = item_options(&mut pass, mobs, players, attack.attacker, SkillTarget::Mob(mob_id), Worn::Weapon, false, false);
            packets.extend(mine.out);
            killed |= mine.killed;
            critical |= mine.criticals > 0;
        }
        AttackOutcome::Struck { packets, killed, critical }
    }

    /// `CMob::Damage` (`docs/original/combat.md` section 3 step 5): HP, SP and MP lowered, the damage and aggro recorded, a target by
    /// aggro; on a kill `CMob::Death` (section 6): `0x0606` to the monster's view, the experience to the player with the most damage
    /// (`ZoneEvent::Kill`), the drops. Nothing for a monster already dead. The packets for the attacker go to `mine`; returns whether
    /// it died.
    #[allow(clippy::too_many_arguments)]
    fn damage_mob(
        &self,
        map: u16,
        players: &HashMap<u32, Placed>,
        mobs: &mut MobMap,
        mob_id: u32,
        hit: &Hit,
        defs: &MobDefs,
        loot: (&DropTables, &ItemDefs),
        now: Instant,
        rand: &mut impl FnMut() -> u32,
        mine: &mut Vec<Vec<u8>>,
    ) -> bool {
        let Some(mob) = mobs.mobs.get_mut(&mob_id) else { return false };
        let Some(def) = defs.get(mob.mob_id) else { return false };
        if mob.dead_until.is_some() || mob.hp == 0 {
            return false;
        }
        // Hits counted for a script (`CMob::Damage`, `docs/original/scripted-mobs.md` section 7).
        if let Some(s) = mobs.scripts.get_mut(&mob_id)
            && s.flags.counting
        {
            s.flags.count = s.flags.count.wrapping_add(1);
        }
        let Some(mob) = mobs.mobs.get_mut(&mob_id) else { return false };
        let [hp, sp, mp] = hit.damage;
        mob.hp = mob.hp.saturating_sub(hp);
        mob.sp = mob.sp.saturating_sub(sp.min(u32::from(u16::MAX)) as u16);
        mob.mp = mob.mp.saturating_sub(mp.min(u32::from(u16::MAX)) as u16);
        let killed = mob.hp == 0;
        if killed {
            let body = if def.respawn.is_zero() { 5 } else { 10 };
            mob.dead_until = Some(now + Duration::from_secs(body));
        }
        // `0x0A04`: the blow taken, and the death.
        let attacker =
            players.get(&hit.attacker).map(|p| String::from_utf8_lossy(&p.occupant.shape.name).trim_end_matches('\0').to_owned());
        self.mob_record(map, mob, hp, crate::game_log::MobAction::Damage, attacker.as_deref(), None);
        if killed {
            self.mob_record(map, mob, hp, crate::game_log::MobAction::Death, attacker.as_deref(), None);
        }
        let (cell, kind, fair) = (mob.cell, mob.mob_id, mob.fair);
        let party = self.party_id(hit.attacker);
        // A blow or a skill hit counts for the party too, before the player's own (`CMob::DamageParty`).
        if hit.direct && party != 0 {
            mobs.add_party_damage(mob_id, party, hp);
        }
        mobs.add_damage(mob_id, hit.attacker, hp, hit.aggro, party);
        // Its debuffs go without their end (`CMob::ClearApplySkill`).
        if killed {
            mobs.debuffs.remove(&mob_id);
            // A Lua monster's death is event 3 (`docs/original/instances.md` section 11).
            lua::lua_killed(mobs, mob_id);
        }
        if !killed && matches!(mobs.mobs.get(&mob_id).map(|m| m.state), Some(MobState::Fighting(_))) {
            retarget(mobs, players, mob_id, def, now);
        }
        // A Lua monster counts the attack and may change target by aggro (`CMob::ReTargetByAggroEx`).
        if !killed && mobs.mobs.get(&mob_id).is_some_and(|m| m.lua) {
            lua::lua_damaged(mobs, players, mob_id, def, now);
        }
        // A monster that gave up a chase at a wall gives nothing (`m_bFairCombat`): only `0x0606`.
        // A monster that killed itself and did itself the most damage gives nothing to anyone (`docs/original/mob-skills.md` section 4).
        let mut killer_country = None;
        if killed && let Some(top) = mobs.take_top_damage(mob_id) {
            killer_country = players.get(&top).map(|p| p.occupant.shape.country);
            let fair = fair && top != mob_skills::SELF_DESTRUCT;
            let death = if top == mob_skills::SELF_DESTRUCT {
                cp::mob_death(mob_id, cp::KILLER_MOB, mob_id)
            } else {
                cp::mob_death(mob_id, cp::KILLER_PLAYER, top)
            };
            for p in players.values().filter(|p| in_view(p.cell, cell)) {
                if p.occupant.id == hit.attacker {
                    mine.push(death.clone());
                } else {
                    p.send(&death);
                }
            }
            // A party shares the kill among the members near the killer (`docs/original/party.md` sections 8 and 9).
            let share = if fair { self.kill_share(players, top) } else { None };
            // The scripts' `kill` (and a boss's `boss_kill`): queued, never run here (`docs/plans/ext-5-lua.md` section 4.3).
            if fair && players.contains_key(&top) {
                let near: Vec<u32> = share.as_ref().map_or_else(|| vec![top], |s| s.near.clone());
                self.raise_kill(players, top, mobs, mob_id, def.level, map, &near);
            }
            let (spawn_exp, spawn_drops) = self.spawn_gives(mob_id);
            if let Some(p) = players.get(&top).filter(|_| fair && spawn_exp) {
                match &share {
                    None => {
                        let _ = p.occupant.events.send(ZoneEvent::Kill { mob_id: kind, killer: hit.attacker });
                    }
                    Some(share) => {
                        let event = ZoneEvent::PartyKill {
                            mob_id: kind,
                            killer: hit.attacker,
                            members: share.size,
                            union: share.union,
                            max_level: share.max_level,
                        };
                        for m in share.near.iter().filter_map(|m| players.get(m)) {
                            let _ = m.occupant.events.send(event.clone());
                        }
                    }
                }
            }
            // The drops (CMob::ItemDrop), its owner the player with the most damage if still here; a union led for loot makes the
            // leader the owner when it is on the map (`docs/original/union.md` section 7).
            let killer_owner = if players.contains_key(&top) { top } else { 0 };
            let leader_owner =
                share.as_ref().filter(|s| s.rule == crate::party::RULE_LEADER && players.contains_key(&s.leader)).map(|s| s.leader);
            let at = mobs.mobs.get(&mob_id).map_or([0.0; 3], |m| m.position);
            let mut ground = self.ground.guard();
            let ground = ground.entry(map).or_default();
            let item_defs = loot.1;
            let mut loot = if fair && spawn_drops {
                drops::roll(
                    def,
                    mob_id,
                    loot.0,
                    loot.1,
                    &mut |wanted| killer_owner != 0 && self.has_quest(players, killer_owner, wanted),
                    rand,
                )
            } else {
                Vec::new()
            };
            // In the ranking battle a monster with no gold of its own gives battle points instead of gold (section 7).
            if self.is_ranking_zone(map) && def.money == 0 && def.money_add > 0 {
                loot.retain(|l| !matches!(l, Loot::Gold(_)));
                if fair
                    && killer_owner != 0
                    && let Some((guild, total)) = self.battle_points(players, killer_owner, def.money_add)
                {
                    tracing::debug!(guild, mob = kind, points = def.money_add, total, "ranking battle points");
                }
            }
            let log = self.game_log();
            let owner_name =
                players.get(&killer_owner).map(|p| String::from_utf8_lossy(&p.occupant.shape.name).trim_end_matches('\0').to_owned());
            for l in loot {
                // `0x0A04` for the monster's item and gold drops, and `0x0A06` for a sequence item (made with make type 'V'); its row
                // is made when it is picked up, so its unique id and making time are NULL here and `0x0401` names them then.
                if let Some(mob) = mobs.mobs.get(&mob_id) {
                    match l {
                        Loot::Item { .. } => {
                            self.mob_record(map, mob, 0, crate::game_log::MobAction::ItemDrop, owner_name.as_deref(), None)
                        }
                        Loot::Gold(amount) => {
                            self.mob_record(map, mob, amount, crate::game_log::MobAction::MoneyDrop, owner_name.as_deref(), None)
                        }
                    }
                }
                if let Loot::Item { item_type, type_id, kept: Some(kept), .. } = l
                    && kept.make_type == b'V'
                {
                    let item = crate::game_log::ItemPart {
                        item_id: u32::from(item_type) * 1000 + u32::from(type_id),
                        name: log.item_name(item_type, type_id).to_owned(),
                        ..Default::default()
                    };
                    let header = crate::game_log::Header::character_only(killer_owner, owner_name.as_deref().unwrap_or_default());
                    log.action(crate::game_log::sequence_drop(header, &item, u32::from(kind), log.mob_name(kind), None));
                }
                // CMob::GetDropPosRand: up to half a unit either way on x and z; y is 0.
                let mut spread = || (rand() % 1000) as f32 / 1000.0 - 0.5;
                let (dx, dz) = (spread(), spread());
                let position = [at[0] + dx, 0.0, at[2] + dz];
                let mut party = 0;
                // Items with a `ReqDex` of 0 stay the killer's (`wDex`, +0x38).
                let leader_item = matches!(l, Loot::Item { item_type, type_id, .. }
                    if item_defs.get(item_type, type_id).is_some_and(|d| d.req_dex != 0));
                let mut owner = if leader_item { leader_owner.unwrap_or(killer_owner) } else { killer_owner };
                // A quest item (real type 27) skips the loot rules: on the ground, the killer's, with the killer's party id.
                if let Loot::Item { item_type, .. } = l
                    && real_type(item_type) == 27
                {
                    party = share.as_ref().map_or(0, |s| s.party);
                    if let Some(packet) = self.place(
                        ground,
                        Some(players),
                        l,
                        position,
                        killer_owner,
                        party,
                        now,
                        gp::KIND_DROPPED,
                        hit.attacker,
                        Dropped::MOB,
                    ) {
                        mine.push(packet);
                    }
                    continue;
                }
                if let Some(share) = &share {
                    party = share.party;
                    match l {
                        // No gold on the ground: each member near gets its share.
                        Loot::Gold(amount) => {
                            let each = crate::party::gold_share(amount, usize::from(share.size));
                            for m in share.near.iter().filter_map(|m| players.get(m)) {
                                // A member whose connection is gone gets its share on the ground at its feet (audit R14).
                                if m.occupant.events.send(ZoneEvent::Gold { amount: each, from_mob: true }).is_err()
                                    && let Some(packet) = self.place(
                                        ground,
                                        Some(players),
                                        Loot::Gold(each),
                                        m.occupant.position,
                                        m.occupant.id,
                                        0,
                                        now,
                                        gp::KIND_DROPPED,
                                        hit.attacker,
                                        Dropped::MOB,
                                    )
                                {
                                    mine.push(packet);
                                }
                            }
                            continue;
                        }
                        // By the loot rule (always 1, in turn, in a normal party): straight into a near member's bags, or on the
                        // ground (`docs/original/union.md` section 7).
                        Loot::Item { .. } => {
                            let mut parties = self.parties.guard();
                            let near = |m: u32| share.near.contains(&m);
                            let receiver = match share.rule {
                                crate::party::RULE_IN_TURN => parties.next_in_turn(share.party, near),
                                crate::party::RULE_RANDOM => parties.random_member(share.party, rand()).filter(|&m| near(m)),
                                crate::party::RULE_LEADER => Some(share.leader).filter(|&m| leader_item && near(m)),
                                crate::party::RULE_IN_TURN_GROUND => {
                                    // On the ground, that member's alone.
                                    if let Some(m) = parties.next_in_turn(share.party, near) {
                                        owner = m;
                                        party = 0;
                                    }
                                    None
                                }
                                _ => None,
                            };
                            drop(parties);
                            if let Some(r) = receiver.and_then(|r| players.get(&r)) {
                                // A receiver whose connection is gone leaves it on the ground below (audit R14).
                                if r.occupant
                                    .events
                                    .send(ZoneEvent::Loot { loot: l, position, owner, party, dropped: Dropped::MOB })
                                    .is_ok()
                                {
                                    continue;
                                }
                            }
                        }
                    }
                }
                if let Some(packet) =
                    self.place(ground, Some(players), l, position, owner, party, now, gp::KIND_DROPPED, hit.attacker, Dropped::MOB)
                {
                    mine.push(packet);
                }
            }
        }
        // An obelisk falls, a guard leaves its group (`docs/original/obelisks.md` section 4); a boss group moves on (section 11.3).
        if killed && let Some(mob) = mobs.mobs.get(&mob_id).copied() {
            self.obelisk_death(map, players, mobs, &mob, killer_country);
            self.boss_death(map, Some(players), mobs, &mob, false);
        }
        killed
    }

    /// A skill going off (`CUser::AttackMob`/`CUser::AttackUser` with a skill, `docs/original/skills-use.md` sections 7 to 10): what it
    /// does by kind and target type, then its costs. The cooldown is the caller's. `None` when the caster or the target is no longer
    /// there (a monster dead or of the caster's country, a player dead).
    #[allow(clippy::too_many_arguments)]
    pub fn use_skill(
        &self,
        map: u16,
        target: SkillTarget,
        cast: &SkillCast,
        defs: &MobDefs,
        loot: (&DropTables, &ItemDefs),
        now: Instant,
        rand: &mut impl FnMut() -> u32,
    ) -> Option<SkillOutcome> {
        let mut maps = self.maps.guard();
        let mut all = self.mobs.guard();
        let mut no_mobs = MobMap::default();
        let players = maps.get_mut(&map)?;
        let mobs = all.get_mut(&map).unwrap_or(&mut no_mobs);
        let me = players.get(&cast.caster)?;
        let (at, cell) = (me.occupant.position, me.cell);
        let at_war = me.war.is_some_and(|w| w.ready);
        let mut fx = Effect {
            zones: self,
            rules: self.rules(),
            map,
            players,
            mobs,
            cast,
            at,
            cell,
            defs,
            loot,
            now,
            rand,
            out: Vec::new(),
            killed: false,
            died: false,
            recall: false,
            criticals: 0,
            queue: false,
            duel_flag: at_war,
            options: true,
        };
        let s = cast.skill;
        match target {
            SkillTarget::Mob(id) => {
                let mob = fx.mobs.mobs.get(&id).copied()?;
                if mob.dead_until.is_some() || defs.get(mob.mob_id).is_none_or(|d| d.country == cast.country) {
                    return None;
                }
                match (s.type_effect, s.target_type) {
                    (skills::ATTACK, skills::TARGET_ONE) => fx.select_mob(id),
                    (skills::DEBUFF, skills::TARGET_ONE) => fx.hit(id, true),
                    (skills::ATTACK | skills::DEBUFF, skills::TARGET_AROUND_TARGET) => {
                        let announce = sk::skill_use(sk::OP_USE_MOB, skills::RESULT_AREA, cast.caster, id, s.id, s.level, [0; 3]);
                        fx.area(mob.position, mob.cell, announce);
                    }
                    _ => {}
                }
            }
            SkillTarget::Player(id) => {
                let mutual = |a: u32, b: u32| fx.players.get(&a).is_some_and(|p| duel::fighting(p, b));
                fx.duel_flag = at_war || (mutual(cast.caster, id) && mutual(id, cast.caster));
                // Only a resurrection aims at the dead (`CUser::EnableAttackUser`).
                let resurrection = s.type_effect == skills::ONCE && s.kind == skills::RESURRECTION;
                if fx.players.get(&id).is_none_or(|p| p.dead && !resurrection) {
                    return None;
                }
                // Harm to another player needs an enemy (`CUser::ChkTargetUser` chose one; it may have hidden since).
                let harms = matches!(s.type_effect, skills::ATTACK | skills::DEBUFF);
                if harms && id != cast.caster && !fx.enemy(id) {
                    return None;
                }
                match (s.type_effect, s.target_type) {
                    // Another player: an enemy attacked or debuffed (`docs/original/pvp-2.md` sections 2 to 4).
                    (skills::ATTACK, skills::TARGET_ONE) if id != cast.caster => fx.select_player(id),
                    (skills::DEBUFF, skills::TARGET_ONE) if id != cast.caster => fx.hit_player(id, true),
                    (skills::ATTACK | skills::DEBUFF, skills::TARGET_AROUND_TARGET) if id != cast.caster => {
                        let (centre, centre_cell) = (fx.players[&id].occupant.position, fx.players[&id].cell);
                        let announce = sk::skill_use(sk::OP_USE_USER, skills::RESULT_AREA, cast.caster, id, s.id, s.level, [0; 3]);
                        fx.area(centre, centre_cell, announce);
                    }
                    (skills::ATTACK | skills::DEBUFF, skills::TARGET_AROUND_ONESELF) => {
                        let announce = sk::skill_use(sk::OP_USE_USER, skills::RESULT_AREA, cast.caster, 0, s.id, s.level, [0; 3]);
                        fx.area(at, cell, announce);
                    }
                    (skills::HEAL, skills::TARGET_ONESELF | skills::TARGET_ONE) => fx.heal(id, true),
                    (skills::BUFF, skills::TARGET_ONESELF | skills::TARGET_ONE) => fx.buff(id, true),
                    // The party near the caster, the caster included or not; without a party the caster itself or nobody, through
                    // the party packet (`CUser::SkillHealParty`, `SkillBuffParty`, `SkillHealNParty`, `SkillBuffNParty`;
                    // `docs/original/party.md` section 10).
                    (skills::HEAL | skills::BUFF, skills::TARGET_PARTY | skills::TARGET_PARTY_AROUND) => {
                        fx.around_caster(sk::skill_use(sk::OP_USE_USER, skills::RESULT_AREA, cast.caster, 0, s.id, s.level, [0; 3]));
                        let with_caster = s.target_type == skills::TARGET_PARTY;
                        for member in fx.party_members(with_caster, with_caster || s.type_effect == skills::BUFF) {
                            if s.type_effect == skills::HEAL {
                                fx.heal(member, false);
                            } else {
                                fx.buff(member, false);
                            }
                        }
                    }
                    (skills::FIELD, _) => fx.field(),
                    (skills::ONCE, _) => fx.once(id),
                    _ => {}
                }
            }
        }
        if s.type_effect == skills::ATTACK && s.kind == skills::DESPERATE {
            fx.give_up_life();
        }
        fx.pay();
        Some(SkillOutcome { packets: fx.out, killed: fx.killed, died: fx.died, recall: fx.recall, criticals: fx.criticals })
    }

    /// An item's skill on another player or a monster (`0x0559` kinds 1 and 2: `CUser::AttackUser` / `AttackMob` with `single` and
    /// `option` false, `docs/original/small-items.md` section 11): one hit as an area hit (`0x0513` / `0x0519`), no lapis skills,
    /// nothing paid. The caller checked that the target may be attacked; `None` when the caster or the target is gone, or the player
    /// is no enemy any more.
    #[allow(clippy::too_many_arguments)]
    pub fn item_skill_hit(
        &self,
        map: u16,
        target: SkillTarget,
        cast: &SkillCast,
        defs: &MobDefs,
        loot: (&DropTables, &ItemDefs),
        now: Instant,
        rand: &mut impl FnMut() -> u32,
    ) -> Option<SkillOutcome> {
        let mut maps = self.maps.guard();
        let mut all = self.mobs.guard();
        let mut no_mobs = MobMap::default();
        let players = maps.get_mut(&map)?;
        let mobs = all.get_mut(&map).unwrap_or(&mut no_mobs);
        let me = players.get(&cast.caster)?;
        let (at, cell) = (me.occupant.position, me.cell);
        let at_war = me.war.is_some_and(|w| w.ready);
        let mut fx = Effect {
            zones: self,
            rules: self.rules(),
            map,
            players,
            mobs,
            cast,
            at,
            cell,
            defs,
            loot,
            now,
            rand,
            out: Vec::new(),
            killed: false,
            died: false,
            recall: false,
            criticals: 0,
            queue: false,
            duel_flag: at_war,
            options: false,
        };
        match target {
            SkillTarget::Mob(id) => {
                let mob = fx.mobs.mobs.get(&id).copied()?;
                if mob.dead_until.is_some() || defs.get(mob.mob_id).is_none_or(|d| d.country == cast.country) {
                    return None;
                }
                fx.hit(id, false);
            }
            SkillTarget::Player(id) => {
                let mutual = |a: u32, b: u32| fx.players.get(&a).is_some_and(|p| duel::fighting(p, b));
                fx.duel_flag = at_war || (mutual(cast.caster, id) && mutual(id, cast.caster));
                if !fx.enemy(id) {
                    return None;
                }
                fx.hit_player(id, false);
            }
        }
        Some(SkillOutcome { packets: fx.out, killed: fx.killed, died: fx.died, recall: fx.recall, criticals: fx.criticals })
    }

    /// Puts something on the ground (`CZone::EnterItem`): a new object id, 60 s to live, `0x0401` of `kind` to the view. Returns the
    /// packet for `me` when it is in that view.
    #[allow(clippy::too_many_arguments)]
    fn place(
        &self,
        ground: &mut Ground,
        players: Option<&HashMap<u32, Placed>>,
        loot: Loot,
        position: [f32; 3],
        owner: u32,
        party: u32,
        now: Instant,
        kind: u8,
        me: u32,
        dropped: Dropped,
    ) -> Option<Vec<u8>> {
        let id = self.last_item_id.fetch_add(1, Ordering::Relaxed) + 1;
        let item = GroundItem { id, loot, position, cell: cell_of(position), owner, party, expires: now + ITEM_LIFETIME, dropped };
        ground.insert(item);
        self.show(players, &item, kind, me)
    }

    /// Puts loot on the ground for everyone to see (`0x0401` of `kind`), as a drop does.
    #[allow(clippy::too_many_arguments)]
    pub fn drop_on_ground(
        &self,
        map: u16,
        loot: Loot,
        position: [f32; 3],
        owner: u32,
        party: u32,
        now: Instant,
        kind: u8,
        dropped: Dropped,
    ) {
        let maps = self.maps.guard();
        let mut ground = self.ground.guard();
        self.place(ground.entry(map).or_default(), maps.get(&map), loot, position, owner, party, now, kind, 0, dropped);
    }

    /// Puts what a player dropped on the ground where it stands (`CUser::ItemDrop`, `docs/original/item-drop.md` section 3): kind 1,
    /// the player's alone for its first 30 s, no party. The view gets `0x0401`; the player's own is returned.
    pub fn drop_by_player(&self, map: u16, player: u32, loot: Loot, position: [f32; 3], now: Instant) -> Option<Vec<u8>> {
        let maps = self.maps.guard();
        let mut ground = self.ground.guard();
        self.place(
            ground.entry(map).or_default(),
            maps.get(&map),
            loot,
            position,
            player,
            0,
            now,
            gp::KIND_PLAYER_DROP,
            player,
            Dropped::player(player),
        )
    }

    /// `0x0401` of an item to everyone in its view; the packet for `me` is returned instead.
    fn show(&self, players: Option<&HashMap<u32, Placed>>, item: &GroundItem, kind: u8, me: u32) -> Option<Vec<u8>> {
        let packet = item.enter_packet(kind);
        let mut mine = None;
        for p in players.into_iter().flat_map(HashMap::values).filter(|p| in_view(p.cell, item.cell)) {
            if p.occupant.id == me {
                mine = Some(packet.clone());
            } else {
                p.send(&packet);
            }
        }
        mine
    }

    /// Takes an item off the ground for a player (`CZone::GetPickItem`): `Err` with the `0x0205` code when it is gone (3), someone
    /// else's for its first 30 s (0), more than 10 away (4) or a quest item `quest_ok` refuses for its type and type id (2, the item
    /// stays). The view gets `0x0402`; the player's own is returned.
    pub fn pick_up(
        &self,
        map: u16,
        player: u32,
        item_id: u32,
        now: Instant,
        quest_ok: impl Fn(u8, u8) -> bool,
    ) -> Result<(GroundItem, Vec<u8>), u8> {
        let maps = self.maps.guard();
        let mut ground = self.ground.guard();
        let players = maps.get(&map);
        let me = players.and_then(|p| p.get(&player)).ok_or(gp::PICK_GONE)?;
        let ground = ground.get_mut(&map).ok_or(gp::PICK_GONE)?;
        let item = *ground.items.get(&item_id).ok_or(gp::PICK_GONE)?;
        let shared = item.party != 0 && self.party_id(player) == item.party;
        if item.owner != 0 && item.owner != player && !shared && now + OWNER_ONLY <= item.expires {
            return Err(gp::PICK_NOT_YOURS);
        }
        if distance(me.occupant.position, item.position) > PICK_RANGE {
            return Err(gp::PICK_TOO_FAR);
        }
        if let Loot::Item { item_type, type_id, .. } = item.loot
            && real_type(item_type) == 27
            && !quest_ok(item_type, type_id)
        {
            return Err(gp::PICK_NO_QUEST);
        }
        ground.remove(item_id);
        let packet = gp::item_leave(item_id);
        for p in players.into_iter().flat_map(HashMap::values).filter(|p| p.occupant.id != player && in_view(p.cell, item.cell)) {
            p.send(&packet);
        }
        Ok((item, packet))
    }

    /// An item that could not be taken goes back on the ground for another 60 s (`CZone::EnterItem`, kind 1); returns the player's
    /// `0x0401`.
    pub fn put_back(&self, map: u16, player: u32, item: GroundItem, loot: Loot, now: Instant) -> Option<Vec<u8>> {
        let maps = self.maps.guard();
        let mut ground = self.ground.guard();
        let item = GroundItem { loot, expires: now + ITEM_LIFETIME, ..item };
        ground.entry(map).or_default().insert(item);
        self.show(maps.get(&map), &item, gp::KIND_PUT_BACK, player)
    }

    /// Everything on one map's ground. For the tests that count value (`tests/fuzz_dispatch.rs`).
    #[doc(hidden)]
    pub fn ground_items(&self, map: u16) -> Vec<GroundItem> {
        self.ground.guard().get(&map).map(|g| g.items.values().copied().collect()).unwrap_or_default()
    }

    /// Removes the items whose time is up (`CZone::UpdateItem`): `0x0402` to their view.
    pub fn expire_items(&self, now: Instant) -> usize {
        let maps = self.maps.guard();
        let mut all = self.ground.guard();
        let mut removed = 0;
        for (map, ground) in all.iter_mut() {
            let gone: Vec<u32> = ground.items.values().filter(|i| i.expires < now).map(|i| i.id).collect();
            for id in gone {
                let Some(item) = ground.remove(id) else { continue };
                let packet = gp::item_leave(id);
                for p in maps.get(map).into_iter().flat_map(HashMap::values).filter(|p| in_view(p.cell, item.cell)) {
                    p.send(&packet);
                }
                removed += 1;
            }
        }
        removed
    }

    /// Removes the bodies whose time has come (`CZone::UpdateMobDeath`): `0x0602` to their view, and their spawn entries: one
    /// fewer, and an entry that was full may spawn again after the respawn time (`docs/original/combat.md` section 6); an
    /// obelisk's and its guards' as `docs/original/obelisks.md` section 3 says.
    pub fn remove_bodies(&self, now: Instant, defs: &MobDefs, rand: &mut impl FnMut() -> u32) -> usize {
        let maps = self.maps.guard();
        let mut all = self.mobs.guard();
        let mut removed = 0;
        for (&map, mobs) in all.iter_mut() {
            let gone: Vec<Mob> = mobs.mobs.values().filter(|m| m.dead_until.is_some_and(|t| t <= now)).copied().collect();
            for mob in gone {
                let packet = w::mob_leave(mob.id);
                for p in maps.get(&map).into_iter().flat_map(|p| p.values()).filter(|p| in_view(p.cell, mob.cell)) {
                    p.send(&packet);
                }
                mobs.remove(mob.id);
                self.forget_ext_spawn(mob.id);
                removed += 1;
                if Self::obelisk_body(mobs, &mob, defs, now, rand) || Self::boss_body(map, mobs, &mob, defs, now, rand) {
                    continue;
                }
                if let Some(r) = mobs.respawns.get_mut(mob.entry) {
                    if r.current >= r.count
                        && let Some(def) = defs.get(r.mob_id)
                    {
                        r.due = Some(respawn_due(def, now, rand));
                    }
                    r.current = r.current.saturating_sub(1);
                }
            }
        }
        removed
    }

    /// A new key for an applied skill.
    pub fn new_key(&self) -> u32 {
        self.last_skill_key.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// A monster on the map (`CZone::FindMob`).
    pub fn mob(&self, map: u16, id: u32) -> Option<Mob> {
        self.mobs.guard().get(&map)?.mobs.get(&id).copied()
    }

    pub fn mob_count(&self, map: u16) -> usize {
        self.mobs.guard().get(&map).map_or(0, |m| m.mobs.len())
    }

    /// Whether a player may sit at a gambling table (`docs/original/mini-game.md` decision 3): alive, in no trade, with no personal
    /// shop open, in no duel and no guild war. The caller asks for the NPC separately.
    pub fn may_gamble(&self, map: u16, id: u32) -> bool {
        let maps = self.maps.guard();
        let Some(p) = maps.get(&map).and_then(|players| players.get(&id)) else { return false };
        !p.dead && p.shop.is_none() && p.duel.is_none() && p.war.is_none() && !self.trades.guard().busy(id)
    }

    /// Where a player in the zone stands, and what decides whether a skill may aim at it.
    pub fn whereabouts(&self, map: u16, id: u32) -> Option<Whereabouts> {
        let maps = self.maps.guard();
        let p = maps.get(&map)?.get(&id)?;
        let o = &p.occupant;
        Some(Whereabouts {
            position: o.position,
            country: o.shape.country,
            dead: p.dead,
            shape: p.form.shape,
            level: o.vitals.guard.level,
            grow: o.shape.grow,
            mounted: p.ride.status != Riding::OnFoot,
            fighting: duel::in_fight(p),
            war: p.war.map_or(0, |w| w.enemy),
        })
    }

    /// A player's max HP and HP (`0x0302`).
    pub fn hp(&self, map: u16, target: u32) -> Option<(u16, u16)> {
        let maps = self.maps.guard();
        let p = maps.get(&map)?.get(&target)?;
        Some((p.occupant.vitals.max[0], p.occupant.vitals.health[0]))
    }

    /// An NPC in the player's view. The original only looks on the same map (`CZone::FindNpc`); the view keeps a modified
    /// client from dealing with NPCs anywhere on the map.
    pub fn npc_in_view(&self, map: u16, player: u32, npc_id: u32) -> Option<NpcEnter> {
        let npc = self.npc(map, npc_id)?;
        let maps = self.maps.guard();
        let me = maps.get(&map)?.get(&player)?;
        in_view(me.cell, cell_of(npc.position)).then_some(npc)
    }

    /// Whether an NPC of `kind` (`npc_type`) is in the player's view.
    pub fn npc_kind_in_view(&self, map: u16, player: u32, kind: u8) -> bool {
        let Some(cell) = self.maps.guard().get(&map).and_then(|players| players.get(&player)).map(|me| me.cell) else {
            return false;
        };
        self.npcs_in_view(map, cell, None).iter().any(|npc| npc.kind == kind)
    }

    /// The NPCs of the zone in the cells of `view` that are not in the view of `except` (all of them without one).
    fn npcs_in_view(&self, map: u16, view: (i32, i32), except: Option<(i32, i32)>) -> Vec<NpcEnter> {
        npc::in_sight(self.npcs.guard().get(&map), view, except)
    }

    /// Places a player (`CZone::EnterUser`). The others in view get its `0x0201` and `0x051C` (and its shape, if it has one); the
    /// returned packets for the player are its own `0x0201` and `0x051C`, then those of everyone in view (with their shapes), the
    /// NPCs in view, the weather, and its own shape (`CUser::SendShape`; a hidden player's client is made to walk).
    /// (The original goes cell by cell, players then NPCs in each; here all players come first. Not yet seen to matter.)
    pub fn enter(&self, map: u16, mut occupant: Occupant) -> Vec<Vec<u8>> {
        let mut maps = self.maps.guard();
        // Arrived: no longer on the way into an instance zone.
        self.instances.guard().unbook(occupant.id);
        self.ranking_arrival(map, occupant.id);
        let players = maps.entry(map).or_default();
        let effects = occupant.buffs.effects();
        let occupant_mount = occupant.mount.take();
        // A party outlives a map change: its type shows in the looks; the members here learn where it is.
        occupant.shape.party_type = self.party_type(occupant.id);
        // So does a guild: its id in `0x0201`, its name in `0x0303`.
        (occupant.guild_id, occupant.shape.guild_name) = self.guild_looks(occupant.id);
        self.tell_position(players, map, occupant.id, occupant.position);
        let placed = Placed {
            cell: cell_of(occupant.position),
            // Only the dead have no HP: a body moved to another map stays dead (a summon answered while dead,
            // `docs/original/item-use.md` section 9).
            dead: occupant.vitals.health[0] == 0,
            occupant,
            moves: 0,
            attacking: false,
            rec_status: REC_NORMAL,
            next_recover: None,
            form: Form { shape: effects.shape, mob: effects.shape_mob },
            disguise: None,
            ride: vehicle::Ride { status: occupant_mount.map_or(Riding::OnFoot, Riding::Riding), ..Default::default() },
            harm: Vec::new(),
            duel: None,
            war: None,
            shop: None,
            visiting: None,
            options_running: false,
            npc: None,
            guard_lock: None,
        };
        let mine = placed.enter_packet(true);
        let my_speed = placed.speed_packet();
        let my_form = placed.form_packet();
        let mut out = vec![mine.clone(), my_speed.clone()];
        for other in players.values().filter(|p| in_view(p.cell, placed.cell)) {
            other.send(&mine);
            if distance(other.occupant.position, placed.occupant.position) <= SPEED_RANGE {
                other.send(&my_speed);
            }
            if let Some(form) = &my_form {
                other.send(form);
                placed.log_hidden(other.occupant.id, "enter");
            }
            out.push(other.enter_packet(false));
            out.push(other.speed_packet());
            out.extend(other.form_packet());
            other.log_hidden(placed.occupant.id, "enter");
        }
        let mobs = mobs_in_view(self.mobs.guard().get(&map), placed.cell, None);
        out.extend(mobs.iter().flat_map(|mob| mob_enter(mob, false)));
        out.extend(self.npcs_in_view(map, placed.cell, None).iter().map(w::npc_enter));
        let items = self.ground.guard().get(&map).map(|g| g.in_view(placed.cell, None)).unwrap_or_default();
        out.extend(items.iter().map(|item| item.enter_packet(gp::KIND_IN_VIEW)));
        out.push(w::weather(0, 0));
        out.extend(Self::obelisk_list(self.mobs.guard().get(&map)));
        if let Some(form) = my_form {
            out.push(form);
            if placed.form.shape == sk::SHAPE_HIDDEN {
                out.push(sk::run_mode(false));
            }
        }
        players.insert(placed.occupant.id, placed);
        out
    }

    /// Removes a player; everyone in view gets `0x0202` (`CZone::LeaveUser`). Returns its HP, MP and SP, its applied skills, and the
    /// duel stakes it had out of its bags (`docs/original/duel.md` section 6.7, deviation 3: back into them).
    pub fn leave(&self, map: u16, id: u32) -> Option<Left> {
        let mut maps = self.maps.guard();
        let players = maps.get_mut(&map)?;
        let now = Instant::now();
        // A duel ends, and the player's guild war (`CUser::PvPChkLeaveByZoneLeave`).
        let stakes = self.end_duel(map, players, id, duel::How::Left, now);
        self.war_left(players, id, shaiya_proto::guild_war::OUT_OF_AREA);
        // A pair on one mount splits, invitations end (`docs/original/vehicles.md` section 4).
        vehicle::split(players, id, true, now);
        // The shop ends and the visit with it (`CUser::LeaveZone`; deviation 4: out of the seller's list).
        self.shop_left(players, id);
        let mut gone = players.remove(&id)?;
        // Its current NPC is let go (`CUser::LeaveZone`).
        npc::let_go(self.npcs.guard().get_mut(&map), &mut gone);
        // Leaving the map refuses a friend request waiting for this player (`CUser::LeaveZone`, `docs/original/friends.md` section 4).
        self.friends().left_map(id, |asker| players.contains_key(&asker));
        let packet = w::user_leave(id);
        for other in players.values().filter(|p| in_view(p.cell, gone.cell)) {
            other.send(&packet);
        }
        Some(Left { health: gone.occupant.vitals.health, buffs: gone.occupant.buffs, stakes })
    }

    /// An accepted step (`CZone::MoveUserByPacket`, `CZone::SendMoveUser`). Changes the cell when the player is past the
    /// margin and swaps what each side sees (players both ways, then the NPCs that left and came into the mover's view),
    /// then sends `0x0501` to the players within range, the mover included.
    pub fn move_to(&self, map: u16, id: u32, step: &Move) -> Vec<Vec<u8>> {
        let mut maps = self.maps.guard();
        let Some(players) = maps.get_mut(&map) else { return Vec::new() };
        let Some(mut me) = players.remove(&id) else { return Vec::new() };
        me.occupant.position = step.position;
        me.occupant.dir = step.dir;
        // Moving stands a sitting player up (`CUser::StatusStand`).
        me.occupant.shape.motion = w::MOTION_STAND;
        me.moves += 1;
        let mut out = Vec::new();
        self.swap_view(map, players, &mut me, &mut out);
        // More than 10 from its current NPC, the player lets go of it (`CUser::PacketPC`).
        npc::let_go_if_far(self.npcs.guard().get_mut(&map), &mut me);

        let packet = w::user_move(id, step);
        let far = me.moves % 4 == 0;
        let range = if far { FAR_MOVE_RANGE } else { ACTION_RANGE };
        for other in players.values().filter(|p| distance(p.occupant.position, step.position) <= range) {
            other.send(&packet);
        }
        out.push(packet);
        // The party on this map learns of every fourth step (`CParty::SendMove` after `PSendViewMove`).
        if far {
            self.tell_position(players, map, id, step.position);
        }
        players.insert(id, me);
        out
    }

    /// A teleport on the same map (`CZone::MoveUserLocal`, `docs/original/gatekeeper.md` section 3): the view is swapped as
    /// for a step but without `0x0501`, then everyone in view, the player included, gets `0x020B` with the new position.
    pub fn warp(&self, map: u16, id: u32, position: [f32; 3]) -> Vec<Vec<u8>> {
        self.relocate(map, id, position, true)
    }

    /// A step refused by the move speed check: the player goes back to `position` as for a teleport, but a pair on one mount
    /// stays together (`docs/original/move-check.md`, deviation 1).
    pub fn move_back(&self, map: u16, id: u32, position: [f32; 3]) -> Vec<Vec<u8>> {
        self.relocate(map, id, position, false)
    }

    fn relocate(&self, map: u16, id: u32, position: [f32; 3], split: bool) -> Vec<Vec<u8>> {
        let mut maps = self.maps.guard();
        let Some(players) = maps.get_mut(&map) else { return Vec::new() };
        // A teleport splits a pair on one mount (`CZone::MoveUser`).
        if split {
            vehicle::split(players, id, false, Instant::now());
        }
        let Some(mut me) = players.remove(&id) else { return Vec::new() };
        me.occupant.position = position;
        let mut out = Vec::new();
        self.swap_view(map, players, &mut me, &mut out);
        let packet = w::position(id, crate::world::map_of(map), position);
        for other in players.values().filter(|p| in_view(p.cell, me.cell)) {
            other.send(&packet);
        }
        out.push(packet);
        self.tell_position(players, map, id, position);
        players.insert(id, me);
        out
    }

    /// After a position change: when the player is past its cell's margin, the cell changes and each side gets what came
    /// into and left its view (`CZone::SendMoveUser`): players both ways, then the NPCs for the player. `players` are the
    /// others on the map; the player's packets are added to `out`.
    fn swap_view(&self, map: u16, players: &HashMap<u32, Placed>, me: &mut Placed, out: &mut Vec<Vec<u8>>) {
        let old_cell = me.cell;
        if keeps_cell(old_cell, me.occupant.position) {
            return;
        }
        let new_cell = cell_of(me.occupant.position);
        me.cell = new_cell;
        let id = me.occupant.id;
        for other in players.values() {
            let (before, after) = (in_view(old_cell, other.cell), in_view(new_cell, other.cell));
            if before && !after {
                other.send(&w::user_leave(id));
                out.push(w::user_leave(other.occupant.id));
            } else if !before && after {
                other.send(&me.enter_packet(false));
                other.send(&me.speed_packet());
                if let Some(form) = me.form_packet() {
                    other.send(&form);
                    me.log_hidden(other.occupant.id, "view");
                }
                out.push(other.enter_packet(false));
                out.push(other.speed_packet());
                out.extend(other.form_packet());
                other.log_hidden(id, "view");
            }
        }
        // What left the view, then what came into it: monsters before NPCs (CCell::SendCellInfoLeave/Enter).
        let (left, came) = {
            let mobs = self.mobs.guard();
            (mobs_in_view(mobs.get(&map), old_cell, Some(new_cell)), mobs_in_view(mobs.get(&map), new_cell, Some(old_cell)))
        };
        let (items_left, items_came) = {
            let ground = self.ground.guard();
            let g = ground.get(&map);
            (
                g.map(|g| g.in_view(old_cell, Some(new_cell))).unwrap_or_default(),
                g.map(|g| g.in_view(new_cell, Some(old_cell))).unwrap_or_default(),
            )
        };
        out.extend(left.iter().map(|mob| w::mob_leave(mob.id)));
        out.extend(self.npcs_in_view(map, old_cell, Some(new_cell)).iter().map(|npc| w::npc_leave(npc.object_id)));
        out.extend(items_left.iter().map(|item| gp::item_leave(item.id)));
        out.extend(came.iter().flat_map(|mob| mob_enter(mob, false)));
        out.extend(self.npcs_in_view(map, new_cell, Some(old_cell)).iter().map(w::npc_enter));
        out.extend(items_came.iter().map(|item| item.enter_packet(gp::KIND_IN_VIEW)));
    }

    /// A packet for everyone in the player's view (`CZone::SendView`); returns it for the player itself.
    pub fn send_view(&self, map: u16, id: u32, packet: Vec<u8>) -> Vec<u8> {
        let maps = self.maps.guard();
        if let Some(players) = maps.get(&map)
            && let Some(me) = players.get(&id)
        {
            for other in players.values().filter(|p| p.occupant.id != id && in_view(p.cell, me.cell)) {
                other.send(&packet);
            }
        }
        packet
    }

    /// A packet for the players within the action range of the player (`PSendViewAction`); returns it for the player itself.
    pub fn send_range(&self, map: u16, id: u32, packet: Vec<u8>) -> Vec<u8> {
        self.send_near(map, id, packet, ACTION_RANGE)
    }

    /// A packet for the players within `range` of the player (`CZone::PSendView`); returns it for the player itself.
    pub fn send_near(&self, map: u16, id: u32, packet: Vec<u8>, range: f32) -> Vec<u8> {
        let maps = self.maps.guard();
        if let Some(players) = maps.get(&map)
            && let Some(me) = players.get(&id)
        {
            for other in players.values().filter(|p| p.occupant.id != id && distance(p.occupant.position, me.occupant.position) <= range) {
                other.send(&packet);
            }
        }
        packet
    }

    /// A player anywhere on this server by name, ignoring case: (map, id, country, name as stored).
    pub fn find_by_name(&self, name: &[u8]) -> Option<(u16, u32, u8, Vec<u8>)> {
        let maps = self.maps.guard();
        maps.iter().find_map(|(&map, players)| {
            players.values().find(|p| p.occupant.shape.name.eq_ignore_ascii_case(name)).map(|p| {
                let o = &p.occupant;
                (map, o.id, o.shape.country, o.shape.name.clone())
            })
        })
    }

    /// `CMob::SendLogBothMob` (`docs/original/game-log.md` section 3.6, `0x0A04`): a monster's action, for a Lua-AI monster or one of a
    /// boss group only. `damage` is the HP damage or healing; the player's name and the skill's are NULL where there are none.
    pub(super) fn mob_record(
        &self,
        map: u16,
        mob: &Mob,
        damage: u32,
        what: crate::game_log::MobAction,
        player: Option<&str>,
        skill: Option<&str>,
    ) {
        let logged = mob.lua || matches!(mob.origin, Origin::Boss(_) | Origin::Follower { .. } | Origin::Chest(_));
        if !logged {
            return;
        }
        let log = self.game_log();
        let header = crate::game_log::Header::map_only(map, mob.position);
        log.action(crate::game_log::both_mob(header, mob.hp, damage, u32::from(mob.mob_id), log.mob_name(mob.mob_id), what, player, skill));
    }

    /// The name of a player in the world by its id, as text (the game log's records name the dead player of a death drop).
    pub fn name_of(&self, id: u32) -> Option<String> {
        let maps = self.maps.guard();
        maps.values()
            .find_map(|players| players.get(&id))
            .map(|p| String::from_utf8_lossy(&p.occupant.shape.name).trim_end_matches('\0').to_owned())
    }

    /// What a counted player kill is multiplied by for this player (the kill boost, `docs/original/operator-console.md` section 4 and
    /// `docs/plans/done/client-patches.md` decision 1): the highest `times` of the entries matching an applied skill, 1 when none does.
    pub fn kill_boost(&self, map: u16, id: u32, boosts: &[KillBoost]) -> u32 {
        let maps = self.maps.guard();
        let Some(player) = maps.get(&map).and_then(|players| players.get(&id)) else { return 1 };
        let applied = &player.occupant.buffs.list;
        boosts.iter().filter(|b| applied.iter().any(|a| b.skill.matches(&a.def))).map(|b| b.times).max().unwrap_or(1)
    }

    /// A packet for one player.
    pub fn send_to(&self, map: u16, id: u32, packet: &[u8]) {
        if let Some(p) = self.maps.guard().get(&map).and_then(|players| players.get(&id)) {
            p.send(packet);
        }
    }

    /// A packet for every player of a faction, on one map or on all (`CZone::SendAllCountry`, `CWorld::PSendAllCountry`).
    /// Tells every player of a faction's connection, in every map.
    pub fn notify_country(&self, country: u8, event: &ZoneEvent) {
        let maps = self.maps.guard();
        for p in maps.values().flat_map(|players| players.values()).filter(|p| p.occupant.shape.country == country) {
            let _ = p.occupant.events.send(event.clone());
        }
    }

    pub fn send_country(&self, country: u8, map: Option<u16>, packet: &[u8]) {
        let maps = self.maps.guard();
        for (_, players) in maps.iter().filter(|(m, _)| map.is_none_or(|map| **m == map)) {
            for p in players.values().filter(|p| p.occupant.shape.country == country) {
                p.send(packet);
            }
        }
    }

    /// A visible equipment slot (0 to 7) changed (`CUser::SendEquipment`): the looks other players get with `0x0303` follow, and
    /// everyone in view gets `0x0507`; returns it for the player itself.
    pub fn change_equipment(&self, map: u16, id: u32, slot: u8, item: Option<&w::ItemUnit>) -> Vec<u8> {
        if let Some(p) = self.maps.guard().get_mut(&map).and_then(|players| players.get_mut(&id))
            && let Some(looks) = p.occupant.shape.equipment.get_mut(usize::from(slot))
        {
            *looks = item.map_or([0; 3], |i| [i.item_type, i.type_id, w::enchant_level(&i.craft_name)]);
            if slot == 7 {
                p.occupant.shape.cloak = item.map(|i| i.gems);
            }
        }
        self.send_view(map, id, w::equipment(id, slot, item))
    }

    /// New looks from a scroll (`docs/original/item-windows.md` section 2): kept for `0x0303`, `0x0226` to the view; returns it for
    /// the player itself.
    pub fn change_body(&self, map: u16, id: u32, body: crate::backend::Body) -> Vec<u8> {
        if let Some(p) = self.maps.guard().get_mut(&map).and_then(|players| players.get_mut(&id)) {
            let s = &mut p.occupant.shape;
            (s.hair, s.face, s.size, s.sex) = (body.hair, body.face, body.size, body.sex);
        }
        self.send_view(map, id, shaiya_proto::item::body_changed(id, [body.hair, body.face, body.size, body.sex]))
    }

    /// New speeds after an equipment change: other players' `0x0301` follows, and the players within 60 get `0x051C`
    /// (`CUser::ItemEquipmentAdd`); returns it for the player itself.
    pub fn change_speeds(&self, map: u16, id: u32, attack_speed: u8, move_speed: u8) -> Vec<u8> {
        if let Some(p) = self.maps.guard().get_mut(&map).and_then(|players| players.get_mut(&id)) {
            p.occupant.attack_speed = attack_speed;
            p.occupant.move_speed = move_speed;
        }
        self.send_near(map, id, w::speed(id, attack_speed, move_speed), SPEED_RANGE)
    }

    /// A new maximum (`kind` 0 HP, 1 SP, 2 MP) to the players within 60 (`CUser::SendMaxHP` and the others); returns it for the
    /// player itself. The current value is kept within it.
    pub fn change_max(&self, map: u16, id: u32, kind: u8, max: u16) -> Vec<u8> {
        if let Some(p) = self.maps.guard().get_mut(&map).and_then(|players| players.get_mut(&id)) {
            // Vitals are HP, MP, SP; the kinds HP, SP, MP.
            let i = match kind {
                w::MAX_HP => 0,
                w::MAX_MP => 1,
                _ => 2,
            };
            p.occupant.vitals.max[i] = max;
            p.occupant.vitals.health[i] = p.occupant.vitals.health[i].min(max);
        }
        self.send_near(map, id, w::set_max(id, kind, max), SPEED_RANGE)
    }

    /// The player's numbers against monsters and its REC after a change of level, equipment or applied skills.
    pub fn set_guard(&self, map: u16, id: u32, guard: Guard) {
        if let Some(p) = self.maps.guard().get_mut(&map).and_then(|players| players.get_mut(&id)) {
            let v = &mut p.occupant.vitals;
            (v.guard, v.guard_element, v.rec) = (guard.fighter, guard.element, guard.rec);
            (v.absorption, v.state_blocks) = (guard.absorption, guard.state_blocks);
            (v.grade_recovery, v.grade_fight_recovery) = (guard.grade_recovery, guard.grade_fight_recovery);
            (v.options, v.cross_road) = (guard.options, guard.cross_road);
        }
    }

    /// Attacking (`CUser::RemApplySkillToMOB` at the attack tick, `docs/original/skills-special.md` section 6): a hiding or a
    /// transformation (but Evolution) ends, `0x050E`, and the shape goes. Returns whether one ended.
    pub fn end_shape_by_attack(&self, map: u16, id: u32) -> bool {
        let mut maps = self.maps.guard();
        let Some(players) = maps.get_mut(&map) else { return false };
        let Some(p) = players.get_mut(&id) else { return false };
        if p.form.shape == 0 {
            return false;
        }
        let Some(a) = p.occupant.buffs.end_shape_for_attack() else { return false };
        p.send(&sk::applied_end(a.key));
        reform(players, id, false);
        true
    }

    /// The sitting recovery percentages after a passive skill changed them.
    pub fn set_sit_percent(&self, map: u16, id: u32, percent: [i32; 3]) {
        if let Some(p) = self.maps.guard().get_mut(&map).and_then(|players| players.get_mut(&id)) {
            p.occupant.vitals.sit_percent = percent;
        }
    }

    /// HP, MP and SP of a player in the zone.
    pub fn health(&self, map: u16, id: u32) -> Option<[u16; 3]> {
        Some(self.maps.guard().get(&map)?.get(&id)?.occupant.vitals.health)
    }

    /// New maxima after a level-up, and HP, MP and SP filled to them (`CUser::LevelUp`): `0x0505` with what was gained to the
    /// players within 60 when something changed. Returns that packet for the player itself.
    pub fn fill(&self, map: u16, id: u32, max: [u16; 3]) -> Option<Vec<u8>> {
        let gained = {
            let mut maps = self.maps.guard();
            let p = maps.get_mut(&map)?.get_mut(&id)?;
            let v = &mut p.occupant.vitals;
            let gained = [0, 1, 2].map(|i| clamp_i16(i32::from(max[i]) - i32::from(v.health[i])));
            v.max = max;
            v.health = max;
            full_again(p, gained[0] > 0);
            gained
        };
        (gained != [0; 3]).then(|| self.send_near(map, id, cp::recover(id, gained), RECOVER_RANGE))
    }

    /// Starting or stopping attacks: the recovery state follows at once (fighting, or normal; `CUser::SendAttackStart`,
    /// `CUser::StatusAttackStop`).
    pub fn set_attacking(&self, map: u16, id: u32, attacking: bool) {
        if let Some(p) = self.maps.guard().get_mut(&map).and_then(|players| players.get_mut(&id)) {
            p.attacking = attacking;
            p.rec_status = if attacking { REC_FIGHTING } else { REC_NORMAL };
        }
    }

    /// Records sitting or standing, which other players' `0x0303` reports.
    pub fn set_motion(&self, map: u16, id: u32, motion: u8) {
        if let Some(p) = self.maps.guard().get_mut(&map).and_then(|players| players.get_mut(&id)) {
            p.occupant.shape.motion = motion;
        }
    }

    /// `0x0301` (`CUser::PacketGetInfo`): the target's position, then its HP and speeds. Nothing if it is not in the zone.
    pub fn info(&self, map: u16, target: u32) -> Vec<Vec<u8>> {
        let maps = self.maps.guard();
        let Some(p) = maps.get(&map).and_then(|players| players.get(&target)) else { return Vec::new() };
        let o = &p.occupant;
        vec![position_sync(o), w::user_info(o.id, o.vitals.max[0], o.vitals.health[0], o.attack_speed, o.move_speed)]
    }

    /// `0x0303` (`CZone::SendUserShape`): the target's position, then its looks (the copied ones while disguised), then its shop's
    /// sign when open and the asker is of its country or on map 42 (`docs/original/personal-shop.md` section 3.7); `0x0202` if it
    /// is not in the zone.
    /// `0x0307` (`CUser::PacketGetInfo`): a player of the asker's zone and country who is not staff is asked for its equipment; true
    /// when it was. No distance or state check, as the original.
    pub fn inspect(&self, map: u16, asker: u32, target: u32) -> bool {
        let maps = self.maps.guard();
        let Some(players) = maps.get(&map) else { return false };
        let (Some(a), Some(t)) = (players.get(&asker), players.get(&target)) else { return false };
        !t.occupant.staff
            && t.occupant.shape.country == a.occupant.shape.country
            && t.occupant.events.send(ZoneEvent::Inspect { asker }).is_ok()
    }

    pub fn shape(&self, map: u16, asker: u32, target: u32) -> Vec<Vec<u8>> {
        let maps = self.maps.guard();
        let Some(players) = maps.get(&map) else { return vec![w::user_leave(target)] };
        let Some(p) = players.get(&target) else { return vec![w::user_leave(target)] };
        let mut out = vec![position_sync(&p.occupant), p.looks_packet()];
        let country = players.get(&asker).map(|a| a.occupant.shape.country);
        if let Some(sign) = my_shop::sign(p, map, country) {
            out.push(sign);
        }
        out
    }

    /// The monster an illusion copies (`CUser::ChkTargetMob`, `docs/original/skills-special.md` section 5): in the zone, alive, not
    /// scripted or an obelisk, of a level not above `level`.
    pub fn illusion_mob(&self, map: u16, mob_id: u32, level: u16, defs: &MobDefs) -> Option<u16> {
        let all = self.mobs.guard();
        let mob = all.get(&map)?.mobs.get(&mob_id)?;
        let def = defs.get(mob.mob_id)?;
        (mob.dead_until.is_none() && def.ai < mobs::AI_SCRIPTED && !def.obelisk && def.level <= level).then_some(def.id)
    }

    /// The looks a disguise copies (`CUser::IsCloneTargetingUser`, `docs/original/skills-special.md` section 5): another player on
    /// the map of the other country, alive, of a level not above `level`, not hidden, disguised or an illusion; under the
    /// disguised player's own id.
    pub fn copy_looks(&self, map: u16, me: u32, target: u32, country: u8, level: u16) -> Option<UserShape> {
        let maps = self.maps.guard();
        let p = maps.get(&map)?.get(&target)?;
        let o = &p.occupant;
        // Never the duel partner (`docs/original/duel.md` section 5) or a guild war's enemy (`docs/original/guild-war.md` section 5).
        let mine = maps.get(&map)?.get(&me);
        let partner = mine.is_some_and(|m| duel::fighting(m, target));
        let foe = mine.is_some_and(|m| crate::guild_war::foes((m.occupant.guild_id, m.war.as_ref()), (o.guild_id, p.war.as_ref())));
        let fits = target != me
            && !partner
            && !foe
            && !p.dead
            && o.vitals.guard.level <= level
            && p.form.shape < sk::SHAPE_DISGUISE
            && p.form.shape != sk::SHAPE_HIDDEN
            && o.shape.country != country;
        fits.then(|| UserShape { id: me, ..o.shape.clone() })
    }

    /// The attack pass (`CZone::UpdateMobAttack`, every 500 ms; `docs/original/mob-fight.md` sections 3 to 6): fighting
    /// monsters chase and strike, those going home walk; a stunned or sleeping monster does nothing (`CMob::UpdateAttack`).
    pub fn mob_fight(&self, now: Instant, world: &World, defs: &MobDefs, loot: (&DropTables, &ItemDefs), rand: &mut impl FnMut() -> u32) {
        let mut maps = self.maps.guard();
        let mut all = self.mobs.guard();
        let mut nobody = HashMap::new();
        for (&map, mobs) in all.iter_mut() {
            let mut pass = Pass { zones: self, map, defs, loot, now, rand: &mut *rand };
            let players = maps.get_mut(&map).unwrap_or(&mut nobody);
            let mut ids: Vec<u32> = mobs
                .mobs
                .values()
                .filter(|m| m.state != MobState::Idle && m.dead_until.is_none() && m.next_attack <= now)
                .map(|m| m.id)
                .collect();
            ids.sort_unstable();
            let terrain = MapTerrain { world, map };
            for id in ids {
                let Some(mob) = mobs.mobs.get(&id).copied() else { continue };
                let Some(def) = defs.get(mob.mob_id) else { continue };
                if def.ai >= crate::mob_ai::AI_LUA || mobs.speeds(id).0 == buffs::CANNOT {
                    continue;
                }
                match mob.state {
                    MobState::Idle | MobState::Scripted(_) | MobState::Lua(_) => {}
                    MobState::Home => go_home(mobs, players, id, def, now, &terrain, world, self.smooth_chase()),
                    MobState::Fighting(target) => fight_turn(&mut pass, mobs, players, id, target, def, &terrain, world),
                }
            }
        }
    }

    /// The one-second pass (`CZone::UpdateApplySkill`): recovery (`docs/original/mob-fight.md` section 9: players every 5 s by
    /// their recovery state, idle monsters that recover), then the applied skills whose time is up end on players and monsters
    /// (`docs/original/skills-buffs.md` sections 6 and 12).
    pub fn recover(&self, now: Instant, defs: &MobDefs, loot: (&DropTables, &ItemDefs), rand: &mut impl FnMut() -> u32) {
        let mut maps = self.maps.guard();
        let mut all = self.mobs.guard();
        for (&map, players) in maps.iter_mut() {
            let mut changes = Vec::new();
            let mut heals = Vec::new();
            let mut reshaped = Vec::new();
            let mut payments = Vec::new();
            for p in players.values_mut() {
                let ended = p.occupant.buffs.expire(now);
                if !ended.is_empty() {
                    for a in &ended {
                        p.send(&sk::applied_end(a.key));
                        if let Some(packet) = heal_over_time(p, a, now) {
                            heals.push((p.occupant.position, packet));
                        }
                        if !p.dead
                            && let Some(damage) = mob_skills::end_damage(a, p.occupant.vitals.health, p.occupant.vitals.max, now)
                        {
                            payments.push((p.occupant.id, *a, damage));
                        }
                    }
                    let _ = p.occupant.events.send(ZoneEvent::Buffs);
                    reshaped.push(p.occupant.id);
                }
                if p.dead || p.next_recover.is_some_and(|t| t > now) {
                    continue;
                }
                let v = &mut p.occupant.vitals;
                // The faction grade adds to each (`docs/original/pvp-2.md` section 8).
                let amount = |max: u16, percent: i32, grade: i32, fighting: i32| {
                    let max = f64::from(max);
                    match p.rec_status {
                        REC_SITTING => {
                            (f64::from(percent) * max * f64::from(0.01f32)
                                + max * f64::from(0.05f32)
                                + f64::from(REC_SITTING_AMOUNT + grade)) as i32
                        }
                        REC_NORMAL => REC_NORMAL_AMOUNT + grade,
                        _ => fighting,
                    }
                };
                let before = v.health;
                for i in 0..3 {
                    let max = v.max[i];
                    let gained = amount(max, v.sit_percent[i], v.grade_recovery[i], v.grade_fight_recovery[i]);
                    v.health[i] = (i32::from(v.health[i]) + gained).clamp(0, i32::from(max)) as u16;
                }
                let rose = v.health[0] > before[0];
                full_again(p, rose);
                let v = &p.occupant.vitals;
                let sitting = p.occupant.shape.motion == w::MOTION_SIT && !p.attacking;
                if v.health != before {
                    let gained = [0, 1, 2].map(|i| clamp_i16(i32::from(v.health[i]) - i32::from(before[i])));
                    let full = (0..3).any(|i| v.health[i] == v.max[i]);
                    changes.push((p.occupant.id, p.occupant.position, gained, (full || sitting).then_some(v.health)));
                }
                p.next_recover = Some(now + RECOVER_EVERY);
                if p.attacking {
                    p.rec_status = REC_FIGHTING;
                } else if p.occupant.shape.motion == w::MOTION_SIT {
                    p.rec_status = REC_SITTING;
                } else if p.occupant.shape.motion == w::MOTION_STAND {
                    p.rec_status = REC_NORMAL;
                }
            }
            for (id, at, gained, set) in changes {
                send_around(Some(players), at, RECOVER_RANGE, &cp::recover(id, gained));
                if let (Some(health), Some(p)) = (set, players.get(&id)) {
                    p.send(&cp::recover_set(health));
                }
            }
            for (at, packet) in heals {
                send_around(Some(players), at, RECOVER_RANGE, &packet);
            }
            for id in reshaped {
                reform(players, id, false);
            }
            // A monster's damage over time pays the rest, the death sentence kills (`docs/original/mob-skills.md` section 8).
            let mut no_mobs = MobMap::default();
            let mobs = all.get_mut(&map).unwrap_or(&mut no_mobs);
            for (id, a, damage) in payments {
                if players.get(&id).is_none_or(|p| p.dead) {
                    continue;
                }
                let mut pass = Pass { zones: self, map, defs, loot, now, rand: &mut *rand };
                mob_skills::keep_damage(&mut pass, mobs, players, id, &a.def, a.attacker_of(), damage);
            }
        }
        let nobody = HashMap::new();
        for (&map, mobs) in all.iter_mut() {
            let players = maps.get(&map).unwrap_or(&nobody);
            self.end_debuffs(map, players, mobs, defs, loot, now, rand);
        }
        for (map, mobs) in all.iter_mut() {
            let players = maps.get(map);
            // Lua monsters recover in state 0 (`CMob::GetStatus`, `docs/original/instances.md` section 10).
            let lua_busy: std::collections::HashSet<u32> =
                mobs.lua.iter().filter(|(_, l)| l.stack.top() != crate::mob_lua::IDLE).map(|(&id, _)| id).collect();
            for mob in mobs.mobs.values_mut() {
                let Some(def) = defs.get(mob.mob_id) else { continue };
                if def.recover == 0 || mob.next_recover.is_some_and(|t| t > now) || mob.hp >= def.hp || mob.dead_until.is_some() {
                    continue;
                }
                // A monster with a machine recovers only while running it; in battle only when its script allows it, by its
                // ratio and at its pace (`docs/original/scripted-mobs.md` section 7).
                let (amount, next) = if mob.scripted {
                    let flags = mobs.scripts.get(&mob.id).map(|s| s.flags).unwrap_or_default();
                    match mob.state {
                        MobState::Scripted(_) if flags.battle && flags.recover => {
                            ((def.recover as f32 * flags.recover_ratio) as u32, Duration::from_millis(u64::from(flags.recover_time)))
                        }
                        MobState::Scripted(_) if !flags.battle => (def.recover, RECOVER_EVERY),
                        _ => continue,
                    }
                } else if mob.state == MobState::Idle || (mob.lua && !lua_busy.contains(&mob.id)) {
                    (def.recover, RECOVER_EVERY)
                } else {
                    continue;
                };
                let gained = amount.min(def.hp - mob.hp);
                mob.hp += gained;
                mob.next_recover = Some(now + next);
                send_around(players, mob.position, RECOVER_RANGE, &cp::mob_recover(mob.id, gained as i32));
            }
        }
    }

    /// The three-second pass (`CZone::UpdateApplyEffectTick`, `docs/original/skills-special.md` section 7). Each living player's
    /// applied skills in order: a heal over time pays for the time since its last payment (`0x050F` within 60), Fire Thorns hurts the
    /// monsters around (players: not built), Detect finds the hidden. Then each monster's damage over time strikes for
    /// the time since its last blow (`0x0607` within 60) and wakes it.
    pub fn keep_effects(
        &self,
        now: Instant,
        world: &World,
        defs: &MobDefs,
        loot: (&DropTables, &ItemDefs),
        rand: &mut impl FnMut() -> u32,
    ) {
        let mut maps = self.maps.guard();
        let mut all = self.mobs.guard();
        for (&map, players) in maps.iter_mut() {
            let mut no_mobs = MobMap::default();
            let mobs = all.get_mut(&map).unwrap_or(&mut no_mobs);
            let mut ids: Vec<u32> = players.values().filter(|p| !p.dead).map(|p| p.occupant.id).collect();
            ids.sort_unstable();
            for id in ids {
                let Some(p) = players.get_mut(&id) else { continue };
                let mut due = Vec::new();
                for a in p.occupant.buffs.list.iter_mut() {
                    match (a.def.type_effect, a.def.kind) {
                        (skills::BUFF, buffs::HEAL_OVER_TIME | buffs::FIRE_THORNS) | (skills::DEBUFF, buffs::DAMAGE_OVER_TIME) => {
                            due.push((a.def, now.saturating_duration_since(a.started), a.attacker_of()));
                            a.started = now;
                        }
                        (skills::BUFF, buffs::DETECT) => due.push((a.def, Duration::ZERO, a.attacker_of())),
                        _ => {}
                    }
                }
                let mut hurt = false;
                for (def, elapsed, attacker) in due {
                    if players.get(&id).is_none_or(|p| p.dead) {
                        break;
                    }
                    match def.kind {
                        // A monster's damage over time (`docs/original/mob-skills.md` section 8).
                        buffs::DAMAGE_OVER_TIME => {
                            let p = &players[&id];
                            let damage = mob_skills::over_time(&def, p.occupant.vitals.health, p.occupant.vitals.max, elapsed);
                            let mut pass = Pass { zones: self, map, defs, loot, now, rand: &mut *rand };
                            hurt |= mob_skills::keep_damage(&mut pass, mobs, players, id, &def, attacker, damage);
                        }
                        buffs::HEAL_OVER_TIME => {
                            let Some(p) = players.get_mut(&id) else { break };
                            if p.dead {
                                break;
                            }
                            let healed = heal_player(p, buffs::time_heal(&def, elapsed));
                            let at = p.occupant.position;
                            send_around(Some(players), at, RECOVER_RANGE, &sk::keep(sk::OP_KEEP_USER, id, def.id, def.level, healed));
                        }
                        buffs::FIRE_THORNS => self.thorns(map, world.is_war_map(map), players, mobs, id, &def, defs, loot, now, rand),
                        _ => reveal_around(players, id, f32::from(def.apply_range)),
                    }
                }
                // Hurt and alive: hiding and transformations end, and the player wakes.
                if hurt && players.get(&id).is_some_and(|p| !p.dead) {
                    mob_skills::shape_and_sleep_end(players, id);
                }
            }
        }
        let nobody = HashMap::new();
        for (&map, mobs) in all.iter_mut() {
            let players = maps.get(&map).unwrap_or(&nobody);
            self.damage_over_time(map, players, mobs, defs, loot, now, rand);
        }
    }

    /// Fire Thorns' three-second hit (`CUser::ChkProtect`, `docs/original/thorns-fields-pvp.md` section 1): first the enemy players
    /// (`pvp::enemies`, attackable and visible, hidden ones revealed) in the 3 x 3 cells around the caster and within `byApplyRange`
    /// take `wDamage` as it is, as a monster's hit does (`0x0513` within 60 of the caster, after the death's `0x0504`); then every
    /// living monster of another country there takes `wDamage` as it is (no roll, no defence) with `REC × 10 + wAggro` aggro, turns
    /// on the player, and the players within 60 get `0x0519`. A kill does not stop the caster's attack (deviation 3).
    #[allow(clippy::too_many_arguments)]
    fn thorns(
        &self,
        map: u16,
        war: bool,
        players: &mut HashMap<u32, Placed>,
        mobs: &mut MobMap,
        caster: u32,
        s: &SkillDef,
        defs: &MobDefs,
        loot: (&DropTables, &ItemDefs),
        now: Instant,
        rand: &mut impl FnMut() -> u32,
    ) {
        let Some(p) = players.get(&caster) else { return };
        let (at, cell, country, rec) = (p.occupant.position, p.cell, p.occupant.shape.country, p.occupant.vitals.rec);
        let reach = f32::from(s.apply_range);
        // One snapshot of the rules for the pass (`docs/plans/ext-2-rules.md` section 8); a victim a rule protects is skipped.
        let rules = self.rules();
        let mut victims: Vec<u32> = players
            .values()
            .filter(|t| pvp::area_victim(t) && pvp::strikes(p, t, war, &rules, map).is_yes())
            .filter(|t| near_cells(t.cell, cell) && distance(t.occupant.position, at) <= reach)
            .map(|t| t.occupant.id)
            .collect();
        victims.sort_unstable();
        for id in victims {
            if players.get(&caster).is_none_or(|c| c.dead) {
                return;
            }
            // Still alive at its turn (a death earlier in the pass cannot reach it, but keep the guard).
            if players.get(&id).is_none_or(|t| t.dead) {
                continue;
            }
            reveal(players, id);
            let mut pass = Pass { zones: self, map, defs, loot, now, rand: &mut *rand };
            let (shown, _) = mob_skills::hurt_player_skip(&mut pass, mobs, players, (cp::KILLER_PLAYER, caster), id, s.damage, None);
            send_around(
                Some(players),
                at,
                ATTACK_RANGE,
                &sk::skill_use(sk::OP_USE_PARTY, combat::RESULT_HIT, caster, id, s.id, s.level, shown),
            );
        }
        let Some(p) = players.get(&caster) else { return };
        let players = &*players;
        let mut ids: Vec<u32> = mobs
            .mobs
            .values()
            // An unbeatable monster is passed over (`CUser::ChkProtect`, `docs/original/instances.md` section 15).
            .filter(|m| {
                near_cells(m.cell, cell) && m.dead_until.is_none() && m.hp > 0 && !m.unbeatable && distance(m.position, at) <= reach
            })
            .filter(|m| defs.get(m.mob_id).is_some_and(|d| d.country != country))
            .map(|m| m.id)
            .collect();
        ids.sort_unstable();
        for id in ids {
            let hit = Hit {
                attacker: caster,
                damage: s.damage.map(u32::from),
                aggro: rec.max(0) as u32 * 10 + u32::from(s.aggro),
                direct: false,
            };
            let mut mine = Vec::new();
            let killed = self.damage_mob(map, players, mobs, id, &hit, defs, loot, now, rand, &mut mine);
            if !killed {
                set_target(mobs, players, id, caster, true, defs, now, rand);
            }
            for packet in mine {
                p.send(&packet);
            }
            send_around(
                Some(players),
                at,
                ATTACK_RANGE,
                &sk::skill_use(sk::OP_AREA_MOB, combat::RESULT_HIT, caster, id, s.id, s.level, s.damage),
            );
        }
    }

    /// Damage over time on monsters every three seconds (`CMob::ChkKeepSkill`): each `wType` 61 debuff strikes for the time since its
    /// last blow, from the player who applied it, `0x0607` within 60, until the monster dies; a monster struck and alive wakes.
    #[allow(clippy::too_many_arguments)]
    fn damage_over_time(
        &self,
        map: u16,
        players: &HashMap<u32, Placed>,
        mobs: &mut MobMap,
        defs: &MobDefs,
        loot: (&DropTables, &ItemDefs),
        now: Instant,
        rand: &mut impl FnMut() -> u32,
    ) {
        let mut ids: Vec<u32> =
            mobs.debuffs.iter().filter(|(_, list)| list.iter().any(|d| d.def.kind == buffs::DAMAGE_OVER_TIME)).map(|(&id, _)| id).collect();
        ids.sort_unstable();
        for id in ids {
            let mut struck = false;
            for i in 0.. {
                let Some(d) = mobs.debuffs.get_mut(&id).and_then(|list| list.get_mut(i)) else { break };
                if d.def.kind != buffs::DAMAGE_OVER_TIME {
                    continue;
                }
                let elapsed = now.saturating_duration_since(d.started);
                d.started = now;
                let d = *d;
                let Some(mob) = mobs.mobs.get(&id).copied() else { break };
                let Some(def) = defs.get(mob.mob_id) else { break };
                if mob.dead_until.is_some() || mob.hp == 0 {
                    break;
                }
                let current = [mob.hp, u32::from(mob.sp), u32::from(mob.mp)];
                let max = [def.hp, u32::from(def.sp), u32::from(def.mp)];
                let damage = buffs::time_damage(&d.def, current, max, elapsed).map(u32::from);
                let mut mine = Vec::new();
                let hit = Hit { attacker: d.attacker, damage, aggro: 0, direct: false };
                let killed = self.damage_mob(map, players, mobs, id, &hit, defs, loot, now, rand, &mut mine);
                if let Some(p) = players.get(&d.attacker) {
                    for packet in mine {
                        p.send(&packet);
                    }
                }
                send_around(
                    Some(players),
                    mob.position,
                    ATTACK_RANGE,
                    &sk::keep(sk::OP_KEEP_MOB, id, d.def.id, d.def.level, shown(damage)),
                );
                struck = true;
                if killed {
                    break;
                }
            }
            if struck && mobs.mobs.get(&id).is_some_and(|m| m.dead_until.is_none()) {
                wake(players, mobs, id);
            }
        }
    }

    /// The fields' pass (`CZone::UpdateTrab`, every second; `docs/original/skills-special.md` section 8): each field hits the living
    /// monsters of another country (and neutral ones) in the 3 x 3 cells around it and within `byApplyRange` (a trap only those its
    /// state does not bounce off, and only its first victim); the monster turns on the field's owner when it is in the zone;
    /// `0x0519` within 60 of the field. Before its monsters, the field hits the players of another country on a map with no war type
    /// (`docs/original/thorns-fields-pvp.md` section 2: attackable and visible, hidden ones revealed, `TrabDamage` of their current
    /// numbers, `0x0513` naming the owner within 60 of the field; the owner need not be in the zone), and a trap is used up by its
    /// first victim, player or monster. Fields whose time is up go.
    pub fn fields(&self, now: Instant, world: &World, defs: &MobDefs, loot: (&DropTables, &ItemDefs), rand: &mut impl FnMut() -> u32) {
        let mut maps = self.maps.guard();
        let mut all = self.mobs.guard();
        let mut fields = self.fields.guard();
        let mut nobody = HashMap::new();
        let rules = self.rules();
        for (&map, list) in fields.iter_mut() {
            let war = world.is_war_map(map);
            let players = maps.get_mut(&map).unwrap_or(&mut nobody);
            let mut no_mobs = MobMap::default();
            let mobs = all.get_mut(&map).unwrap_or(&mut no_mobs);
            let mut used = Vec::new();
            {
                for f in list.iter() {
                    let s = &f.skill;
                    // The players first (deviation 2).
                    let mut victims: Vec<u32> = if war {
                        Vec::new()
                    } else {
                        players
                            .values()
                            .filter(|t| pvp::area_victim(t) && t.occupant.shape.country != f.country)
                            .filter(|t| rules.attack(map, &f.owner_who, &pvp::who(t)).is_none())
                            .filter(|t| near_cells(t.cell, f.cell) && distance(t.occupant.position, f.position) <= f32::from(s.apply_range))
                            .map(|t| t.occupant.id)
                            .collect()
                    };
                    victims.sort_unstable();
                    let mut spent = false;
                    for id in victims {
                        let Some(t) = players.get(&id).filter(|t| !t.dead) else { continue };
                        let v = &t.occupant.vitals;
                        let (h, m) = (v.health, v.max);
                        let damage =
                            buffs::field_damage(s, [h[0].into(), h[2].into(), h[1].into()], [m[0].into(), m[2].into(), m[1].into()]);
                        reveal(players, id);
                        let damage = damage.map(|d| d as u16);
                        let mut pass = Pass { zones: self, map, defs, loot, now, rand: &mut *rand };
                        let (shown, _) =
                            mob_skills::hurt_player_skip(&mut pass, mobs, players, (cp::KILLER_PLAYER, f.owner), id, damage, None);
                        let packet = sk::skill_use(sk::OP_USE_PARTY, combat::RESULT_HIT, f.owner, id, s.id, s.level, shown);
                        send_around(Some(players), f.position, ATTACK_RANGE, &packet);
                        if s.kind == skills::TRAP {
                            used.push(f.id);
                            spent = true;
                            break;
                        }
                    }
                    if spent {
                        continue;
                    }
                    let players = &*players;
                    let bounces = |d: &MobDef| {
                        s.kind == skills::TRAP
                            && (s.state_type == 0 || d.resist_states.get(usize::from(s.state_type) - 1).copied().unwrap_or(true))
                    };
                    let mut ids: Vec<u32> = mobs
                        .mobs
                        .values()
                        .filter(|m| near_cells(m.cell, f.cell) && m.dead_until.is_none() && m.hp > 0)
                        .filter(|m| distance(m.position, f.position) <= f32::from(s.apply_range))
                        .filter(|m| defs.get(m.mob_id).is_some_and(|d| d.country != f.country && !bounces(d)))
                        .map(|m| m.id)
                        .collect();
                    ids.sort_unstable();
                    for id in ids {
                        let Some(mob) = mobs.mobs.get(&id).copied() else { continue };
                        let Some(def) = defs.get(mob.mob_id) else { continue };
                        let current = [mob.hp, u32::from(mob.sp), u32::from(mob.mp)];
                        let damage = buffs::field_damage(s, current, [def.hp, u32::from(def.sp), u32::from(def.mp)]);
                        let hit = Hit { attacker: f.owner, damage, aggro: u32::from(s.aggro), direct: false };
                        let mut mine = Vec::new();
                        let killed = self.damage_mob(map, players, mobs, id, &hit, defs, loot, now, rand, &mut mine);
                        if let Some(owner) = players.get(&f.owner) {
                            for packet in mine {
                                owner.send(&packet);
                            }
                            if !killed {
                                set_target(mobs, players, id, f.owner, true, defs, now, rand);
                            }
                        }
                        let packet = sk::skill_use(sk::OP_AREA_MOB, combat::RESULT_HIT, f.owner, id, s.id, s.level, shown(damage));
                        send_around(Some(players), f.position, ATTACK_RANGE, &packet);
                        if s.kind == skills::TRAP {
                            used.push(f.id);
                            break;
                        }
                    }
                }
            }
            list.retain(|f| !used.contains(&f.id) && f.ends > now);
        }
        fields.retain(|_, list| !list.is_empty());
    }

    /// The monster debuffs whose time is up end (`CMob::ChkEndTimeSkill`): damage over time and death sentences strike from the
    /// player who applied them (`0x0607` within 60), changed speeds go out as `0x0609`.
    #[allow(clippy::too_many_arguments)]
    fn end_debuffs(
        &self,
        map: u16,
        players: &HashMap<u32, Placed>,
        mobs: &mut MobMap,
        defs: &MobDefs,
        loot: (&DropTables, &ItemDefs),
        now: Instant,
        rand: &mut impl FnMut() -> u32,
    ) {
        let due: Vec<u32> = mobs.debuffs.iter().filter(|(_, list)| list.iter().any(|d| d.ends <= now)).map(|(&id, _)| id).collect();
        for id in due {
            let before = mobs.speeds(id);
            let Some(list) = mobs.debuffs.get_mut(&id) else { continue };
            let (ended, kept): (Vec<MobDebuff>, Vec<MobDebuff>) = list.drain(..).partition(|d| d.ends <= now);
            *list = kept;
            for d in ended {
                let Some(mob) = mobs.mobs.get(&id).copied() else { break };
                let Some(def) = defs.get(mob.mob_id) else { break };
                if mob.dead_until.is_some() || mob.hp == 0 {
                    break;
                }
                let Some(damage) = debuff_end_damage(&d, &mob, def, now) else { continue };
                let mut mine = Vec::new();
                let hit = Hit { attacker: d.attacker, damage, aggro: 0, direct: false };
                self.damage_mob(map, players, mobs, id, &hit, defs, loot, now, rand, &mut mine);
                if let Some(p) = players.get(&d.attacker) {
                    for packet in mine {
                        p.send(&packet);
                    }
                }
                send_around(
                    Some(players),
                    mob.position,
                    ATTACK_RANGE,
                    &sk::keep(sk::OP_KEEP_MOB, id, d.def.id, d.def.level, shown(damage)),
                );
            }
            let after = mobs.speeds(id);
            if after != before
                && let Some(m) = mobs.mobs.get(&id)
            {
                send_around(Some(players), m.position, SPEED_RANGE, &cp::mob_speed(id, after.0, after.1));
            }
        }
        mobs.debuffs.retain(|_, list| !list.is_empty());
    }

    /// A player's own buff cancelled (`0x0511` with number 255, `docs/original/skills-buffs.md` section 7): `0x050E`, and a heal
    /// over time paid for the time it ran. Returns the player's packets; nothing for an unknown key or what is not a buff.
    pub fn cancel_buff(&self, map: u16, id: u32, key: u32, now: Instant) -> Vec<Vec<u8>> {
        let mut maps = self.maps.guard();
        let Some(players) = maps.get_mut(&map) else { return Vec::new() };
        let Some(p) = players.get_mut(&id) else { return Vec::new() };
        if p.dead {
            return Vec::new();
        }
        let Some(a) = p.occupant.buffs.cancel(key) else { return Vec::new() };
        let _ = p.occupant.events.send(ZoneEvent::Buffs);
        let mut out = vec![sk::applied_end(a.key)];
        let at = p.occupant.position;
        if let Some(packet) = heal_over_time(p, &a, now) {
            for other in players.values().filter(|o| o.occupant.id != id && distance(o.occupant.position, at) <= RECOVER_RANGE) {
                other.send(&packet);
            }
            out.push(packet);
        }
        reform(players, id, false);
        out
    }

    /// What a player's applied skills add up to, while it is in the zone.
    pub fn buff_effects(&self, map: u16, id: u32) -> Option<buffs::BuffEffects> {
        Some(self.maps.guard().get(&map)?.get(&id)?.occupant.buffs.effects())
    }

    /// The player's low-HP passive after learning one (`m_pHPAbility`).
    pub fn set_low_hp(&self, map: u16, id: u32, def: Option<SkillDef>) {
        if let Some(p) = self.maps.guard().get_mut(&map).and_then(|players| players.get_mut(&id)) {
            p.occupant.buffs.low_hp = def;
        }
    }

    /// A monster's attack and move speeds with its debuffs (`0x0304`).
    pub fn mob_speeds(&self, map: u16, id: u32) -> (u8, u8) {
        self.mobs.guard().get(&map).map_or((mobs::ATTACK_SPEED, mobs::MOVE_SPEED), |m| m.speeds(id))
    }

    /// Back to life on the same map (`CUser::Rebirth`, `docs/original/mob-fight.md` section 8): HP, MP and SP full (at half with
    /// `half`), the player moved (the view swapped, no `0x020B`), `0x0551` to its view, then `0x0505` within 60. Returns the player's
    /// packets.
    #[allow(clippy::too_many_arguments)]
    pub fn revive_here(&self, map: u16, id: u32, position: [f32; 3], exp: u32, kind: u8, half: bool) -> Vec<Vec<u8>> {
        let mut maps = self.maps.guard();
        let Some(players) = maps.get_mut(&map) else { return Vec::new() };
        let Some(mut me) = players.remove(&id) else { return Vec::new() };
        let gained = revive(&mut me, half);
        me.occupant.position = position;
        let mut out = Vec::new();
        self.swap_view(map, players, &mut me, &mut out);
        let rebirth = cp::rebirth(id, kind, exp, position);
        for other in players.values().filter(|p| in_view(p.cell, me.cell)) {
            other.send(&rebirth);
        }
        out.push(rebirth);
        let recover = cp::recover(id, gained);
        send_around(Some(players), position, RECOVER_RANGE, &recover);
        out.push(recover);
        players.insert(id, me);
        out
    }

    /// Back to life before going to another map: HP, MP and SP full (at half with `half`), `0x0551` (with the new position) for the
    /// player alone, then `0x0505` within 60. The caller then moves the player. Returns the player's packets.
    pub fn revive_away(&self, map: u16, id: u32, position: [f32; 3], exp: u32, half: bool) -> Vec<Vec<u8>> {
        let mut maps = self.maps.guard();
        let Some(players) = maps.get_mut(&map) else { return Vec::new() };
        let Some(me) = players.get_mut(&id) else { return Vec::new() };
        let gained = revive(me, half);
        let at = me.occupant.position;
        let recover = cp::recover(id, gained);
        let players = &*players;
        for other in players.values().filter(|p| p.occupant.id != id && distance(p.occupant.position, at) <= RECOVER_RANGE) {
            other.send(&recover);
        }
        vec![cp::rebirth(id, cp::REBIRTH_BIND, exp, position), recover]
    }

    pub fn count(&self, map: u16) -> usize {
        self.maps.guard().get(&map).map_or(0, HashMap::len)
    }
}

#[cfg(test)]
mod tests;
