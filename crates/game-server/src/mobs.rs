//! Monsters: their definitions, the maps' spawn areas, respawning and walking about (`docs/original/mobs.md`). Fighting and death
//! come later.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::skills::{self, SkillDef, SkillDefs};

/// Monster speeds after `CMob::Init` (attack 3, move 2), as `0x0304` reports them.
pub const ATTACK_SPEED: u8 = 3;
pub const MOVE_SPEED: u8 = 2;

/// The respawn time the original derives from a monster's `Attackspecial3` (`CGameData::LoadConstMob`, section 1).
pub fn respawn_time(code: u8) -> Duration {
    let millis: u64 = match code {
        0 => 5_000,
        1 => 30_000,
        2 => 60_000,
        3 => 180_000,
        4 => 300_000,
        5 => 600_000,
        6 => 900_000,
        7 => 1_800_000,
        8 => 3_600_000,
        9 => 14_400_000,
        10 => 43_200_000,
        11 => 86_400_000,
        12 => 259_200_000,
        13 => 7_200_000,
        14 => 0,
        15 => 604_800_000,
        // Above 15 the loader stores 0.
        _ => 5_000,
    };
    Duration::from_millis(millis)
}

/// What the server needs of a monster definition (`defs.mobs`, the original's `stMOBInfo`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MobDef {
    pub id: u16,
    pub level: u16,
    pub hp: u32,
    pub sp: u16,
    pub mp: u16,
    /// The states it cannot be put in (`pState`, by a skill's `byStateType` − 1) and skills that always miss it (`pSkill`;
    /// `docs/original/skills-use.md` section 11).
    pub resist_states: [bool; 15],
    pub resist_skills: [u16; 6],
    pub respawn: Duration,
    /// `byAI`: 4 never walks, 10 to 99 run a state machine, 100 and above a Lua script.
    pub ai: u8,
    /// `dwNormalTime`, added to the wait between idle steps.
    pub normal_time: Duration,
    /// `byNormalStep`, the idle step's length; 0 never walks.
    pub normal_step: u8,
    /// `byChaseStep`; 0 never walks either.
    pub chase_step: u8,
    /// Fighting (`docs/original/combat.md` section 4): experience, DEX, LUC, WIS, defence, magic defence, element, size, the
    /// defence bonus (`dwDefenseAddSH`), country (0 light, 1 fury, 2 both) and the respawn code (above 8 the respawn time varies).
    pub exp: u16,
    pub dex: u16,
    pub luc: u16,
    pub wis: u16,
    pub defense: u16,
    pub magic: u16,
    pub attrib: u8,
    pub size: u8,
    pub defense_add: u32,
    pub country: u8,
    pub respawn_code: u8,
    /// Chasing (`docs/original/mob-fight.md` section 4): the wait after a chase step (`dwChaseTime`) and how far an aggressive
    /// monster or a call for help reaches (`byChaseRange`).
    pub chase_time: Duration,
    pub chase_range: u8,
    /// The three attack entries (`stMobAttackInfo`, section 3).
    pub attacks: [MobAttack; 3],
    /// HP regained every 5 s while idle (`nRecoverValue`, the `Attack3` column; section 9).
    pub recover: u32,
    /// An obelisk guard (`bObelisk`, `Attackplus3` not 0): it never walks.
    pub obelisk: bool,
    /// Gold (`Money1`, `Money2`) and the drop entries 0 to 9 (grade, rate) of `defs.mob_items` (`docs/original/combat.md` section 7).
    pub money: u16,
    pub money_add: u16,
    pub drops: [(u16, u32); 10],
    /// The quest item it carries (`byQuestItemType`, `byQuestItemTypeID`; `docs/original/quest-drops.md` section 1.1).
    pub quest_item: Option<(u8, u8)>,
}

/// One attack entry of a monster (`stMobAttackInfo`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MobAttack {
    /// A monster skill's id (`byType`, the `AttackType` column); 0 for a normal attack.
    pub skill: u16,
    /// That skill's level-100 definition (`pInfo`), kept by `MobDefs::attach_skills` (`docs/original/mob-skills.md` section 1).
    pub info: Option<SkillDef>,
    /// When the entry may be used (`byOK`, `AttackOk`): 0 never, 3 to 6 conditions, anything else always.
    pub ok: u8,
    /// Added to the wait after using it (`dwTime`).
    pub time: Duration,
    /// Reach (`byRange`; at most 60 for normal attacks).
    pub range: u8,
    /// Damage and the random extra (`wDamage`, `wDamageAdd`: `Attack`, `AttackPlus`).
    pub damage: u16,
    pub damage_add: u16,
}

/// `byAI` of monsters that stand still.
const AI_STATIONARY: u8 = 4;
/// `nCountry` from `Attackattrib3` (`CGameData::LoadConstMob`): 1 light, 2 fury, anything else both.
pub fn mob_country(attrib3: u8) -> u8 {
    match attrib3 {
        1 => 0,
        2 => 1,
        _ => 2,
    }
}

/// From this `byAI` on, a monster runs a state machine (`CAIFactory::SetFSM`, `docs/original/scripted-mobs.md`); from
/// `mob_ai::AI_LUA` on a Lua script (`docs/original/instances.md`).
pub const AI_SCRIPTED: u8 = 10;
/// Step length added per move speed 0 to 4 (`g_ConstMobMoveSpeed`).
pub const MOVE_SPEED_STEP: [i32; 5] = [-4, -2, 0, 2, 4];
/// The original's constant (`0x3c8efa35`).
const DEGREES_TO_RADIANS: f32 = 0.017_453_292;
/// An idle monster stays within this of its home (`CMob::UpdateIdle`).
pub const HOME_RANGE: f32 = 10.0;

/// A monster's quest item from the `QuestItemID` column (`CGameData::LoadConstMob`): 1 to 1000 are type 27, 1001 to 2000 type 28
/// (id + 0x18) and above that type 29 (id + 0x30); the type id is a byte, so it wraps as the original's.
pub fn quest_item_of(id: i32) -> Option<(u8, u8)> {
    match id {
        1..=1000 => Some((27, id as u8)),
        1001..=2000 => Some((28, (id + 0x18) as u8)),
        2001.. => Some((29, id.wrapping_add(0x30) as u8)),
        _ => None,
    }
}

impl MobDef {
    /// Whether it walks about when idle; the Lua monsters stand still.
    /// Not `AI` 4 and with both steps: the idle code's test, a Lua monster's too.
    fn steps(&self) -> bool {
        self.ai != AI_STATIONARY && self.normal_step != 0 && self.chase_step != 0
    }
}

/// When a new monster takes its first idle turn (`CMob::Init`): `rand() % 60000` ms from now.
pub fn first_move_delay(rand: &mut impl FnMut() -> u32) -> Duration {
    Duration::from_millis(u64::from(rand() % 60_000))
}

/// One idle turn of a monster (`CMob::UpdateIdle`, section 6): where it would step (not yet checked against its home and the map),
/// and how long until the next turn. Lua, dead and not idle monsters (fighting, going home, running their machine) have no turns:
/// `None`.
pub fn idle_turn(mob: &Mob, def: &MobDef, move_speed: u8, rand: &mut impl FnMut() -> u32) -> Option<(Option<[f32; 3]>, Duration)> {
    // A monster that cannot move (stopped, stunned, asleep) takes no turn (`CMob::UpdateIdle`).
    if def.ai >= crate::mob_ai::AI_LUA || mob.dead_until.is_some() || mob.state != MobState::Idle || move_speed == crate::buffs::CANNOT {
        return None;
    }
    idle_step(mob, def, move_speed, rand)
}

/// The idle step itself, also a Lua monster's (`CMob::WhileIdle`): a random step near home, or none for one that does not walk, and
/// the wait before the next turn.
pub fn idle_step(mob: &Mob, def: &MobDef, move_speed: u8, rand: &mut impl FnMut() -> u32) -> Option<(Option<[f32; 3]>, Duration)> {
    if move_speed == crate::buffs::CANNOT {
        return None;
    }
    if !def.steps() {
        return Some((None, Duration::from_millis(10_000 + u64::from(rand() % 30_000))));
    }
    let angle = (rand() % 10_000) as f32;
    let extra = (rand() % 500 / 100) as i32;
    let length = (MOVE_SPEED_STEP[usize::from(move_speed.min(4))] + i32::from(def.normal_step) + extra) as f32;
    let (dx, dz) = turned(length, angle);
    let [x, y, z] = mob.position;
    let to = [x + dx, y, z + dz];
    let wait = Duration::from_millis(10_000 + u64::from(rand() % 30_000)) + def.normal_time;
    Some((Some(to), wait))
}

/// `(0, 0, length)` turned about y by `degrees` (`SVector::RotateY`: `x' = x cos a - z sin a`, `z' = z cos a + x sin a`): the x and
/// z offsets.
pub fn turned(length: f32, degrees: f32) -> (f32, f32) {
    let (sin, cos) = (degrees * DEGREES_TO_RADIANS).sin_cos();
    (-length * sin, length * cos)
}

/// A step length for a move speed (`g_ConstMobMoveSpeed`, the speed clamped to 0..4) and a step.
pub fn step_length(move_speed: i32, step: u8) -> i32 {
    MOVE_SPEED_STEP[move_speed.clamp(0, 4) as usize] + i32::from(step)
}

#[derive(Debug, Default)]
pub struct MobDefs(HashMap<u16, MobDef>);

impl MobDefs {
    pub fn new(defs: impl IntoIterator<Item = MobDef>) -> Self {
        Self(defs.into_iter().map(|d| (d.id, d)).collect())
    }

    /// A definition that can spawn: id, HP and level not 0 (`CMap::LoadMap`).
    pub fn get(&self, id: u16) -> Option<&MobDef> {
        self.0.get(&id).filter(|d| d.id != 0 && d.hp != 0 && d.level != 0)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Sets a drop entry (`CGameData::LoadConstMobItem`: entries 0 to 9).
    pub fn set_drop(&mut self, mob_id: u16, entry: usize, grade: u16, rate: u32) {
        if let Some(def) = self.0.get_mut(&mob_id)
            && let Some(slot) = def.drops.get_mut(entry)
        {
            *slot = (grade, rate);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// `CGameData::LoadConstMob` (`docs/original/mob-skills.md` section 1): an entry naming a skill keeps its level-100 definition
    /// when that is an attack, a debuff, or a heal of `wType` 41 or 42; any other entry naming a skill is cleared (never used).
    /// Returns how many entries kept a skill.
    pub fn attach_skills(&mut self, skills: &SkillDefs) -> usize {
        let mut kept = 0;
        for def in self.0.values_mut() {
            for entry in def.attacks.iter_mut().filter(|e| e.skill != 0) {
                let info = skills.get(entry.skill, skills::MOB_LEVEL).filter(|s| match s.type_effect {
                    skills::ATTACK | skills::DEBUFF => true,
                    skills::HEAL => matches!(s.kind, skills::HEAL_HEALTH | skills::DISPEL),
                    _ => false,
                });
                match info {
                    Some(info) => {
                        entry.info = Some(*info);
                        kept += 1;
                    }
                    None => *entry = MobAttack::default(),
                }
            }
        }
        kept
    }
}

/// A spawn area of a map file (`stMapMob`): the box's first corner, the spread on x and z, and its (monster, count) entries.
#[derive(Clone, Debug, PartialEq)]
pub struct SpawnArea {
    pub corner: [f32; 3],
    pub delta_x: u32,
    pub delta_z: u32,
    pub entries: Vec<(u16, u32)>,
}

impl SpawnArea {
    /// From the file's box `x1, y1, z1, x2, y2, z2` (`CMap::LoadMap`): the spreads are `trunc(x2 - x1)` and `trunc(z2 - z1)`, at least 1.
    pub fn new(b: [f32; 6], entries: Vec<(u16, u32)>) -> Self {
        let spread = |from: f32, to: f32| ((to - from) as i32).max(1) as u32;
        Self { corner: [b[0], b[1], b[2]], delta_x: spread(b[0], b[3]), delta_z: spread(b[2], b[5]), entries }
    }

    /// `x1 + r1 % delta_x`, `y1`, `z1 + r2 % delta_z` (`CZone::UpdateMobRespawn`).
    pub fn spot(&self, r1: u32, r2: u32) -> [f32; 3] {
        let [x, y, z] = self.corner;
        [x + (r1 % self.delta_x) as f32, y, z + (r2 % self.delta_z) as f32]
    }
}

/// What a monster is doing (`m_nStatus`, `docs/original/mob-fight.md` section 1); dead monsters have `Mob::dead_until`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MobState {
    Idle,
    /// Fighting the player with this id.
    Fighting(u32),
    /// Going home.
    Home,
    /// Running its state machine (state 100, `docs/original/scripted-mobs.md` section 3), with its target (`m_Target`) if any.
    Scripted(Option<u32>),
    /// A Lua monster (`docs/original/instances.md` section 10), with its target (`+0x12c`) if any; its status is the top of its
    /// stack (`MobMap::lua`).
    Lua(Option<u32>),
}

/// Where a monster comes from: a spawn area's entry (`Mob::entry`), an obelisk (its index in the map's obelisks) or an obelisk's
/// guard group (`docs/original/obelisks.md` section 3; `generation` tells the guards of one rising of the obelisk from the next),
/// a boss group's boss, follower or chest (its index in the zone's chains, section 11.3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Origin {
    #[default]
    Area,
    Obelisk(usize),
    Guard {
        obelisk: usize,
        group: usize,
        generation: u32,
    },
    Boss(usize),
    Follower {
        chain: usize,
        set: usize,
        generation: u32,
    },
    Chest(usize),
}

/// What a monster is in the middle of between the passes (`docs/original/chase-smoothing.md` deviation 2): the move tick takes
/// the next step of it and nothing else.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Moving {
    #[default]
    None,
    /// Running after `target` with an entry's `range`; full steps wait `wait` (`dwChaseTime`, 400 ms less for `ChaseExp`).
    Chase { target: u32, range: u8, wait: Duration },
    /// Running home (`ReturnHome`, the Lua `WhileReturnHome`).
    Home,
    /// A Lua monster walking to `spot` (`WhileMove`).
    Walk { spot: [f32; 3] },
}

/// A monster in a zone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mob {
    pub id: u32,
    pub mob_id: u16,
    pub position: [f32; 3],
    pub hp: u32,
    /// Its definition's HP, which `0x0608` tells the client (`docs/original/client-extras.md` deviation 1).
    pub max_hp: u32,
    pub sp: u16,
    pub mp: u16,
    /// Its spawn entry (index into the map's respawns), counted down when it goes.
    pub entry: usize,
    /// Where it entered the zone (`m_vPosRespawn`); idle steps stay within `HOME_RANGE` of it.
    pub home: [f32; 3],
    /// Its next idle turn (`m_dwNextMoveTime`).
    pub next_move: Instant,
    /// The zone cell it is filed under.
    pub cell: (i32, i32),
    /// Dead: when its body goes (`m_dwDeathTime`).
    pub dead_until: Option<Instant>,
    pub state: MobState,
    /// The attack pass's next turn (`m_dwNextAttackTime`), each attack entry's ready time (`m_pAttackTime`, `None` is 0) and
    /// the next chase step (`m_dwNextChaseTime`).
    pub next_attack: Instant,
    pub entry_ready: [Option<Instant>; 3],
    pub next_chase: Instant,
    /// Next change of target by aggro (`m_dwNextTargetingTime`) and next look for a target (`m_dwNextNewTargetingTime`).
    pub next_retarget: Option<Instant>,
    pub next_new_target: Option<Instant>,
    /// Next recovery (`m_dwNextRecoverTime`, `None` is 0).
    pub next_recover: Option<Instant>,
    /// Cleared when it gives up a chase at a wall (`m_bFairCombat`): a kill then gives nothing.
    pub fair: bool,
    pub origin: Origin,
    /// It has a state machine (`m_bUsePAIS`): where an ordinary monster would become idle, it starts its machine over.
    pub scripted: bool,
    /// The attack entry last used (`m_nCurrAttack`), which a script's skill names to the client.
    pub entry_used: u8,
    /// It runs a Lua script (`AI` 100 and up with a script, `docs/original/instances.md` section 9): the ordinary passes leave it to
    /// the Lua pass.
    pub lua: bool,
    /// `LuaSetUnBeatable` (`+0x298`): every player blow and skill misses, damage over time does nothing.
    pub unbeatable: bool,
    /// Until when a guard has taken it (`+0x2b0`, `docs/original/guards.md` section 5).
    pub guard_lock: Option<Instant>,
    /// When the client gets the monster to its last spot, and what it is in the middle of (`docs/original/chase-smoothing.md`).
    pub client_eta: Option<Instant>,
    pub moving: Moving,
}

impl Mob {
    /// A monster entering the zone (`CMob::Init`): idle, its first idle turn at `next_move`, looking for targets from 5 s on.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: u32,
        mob_id: u16,
        position: [f32; 3],
        hp: u32,
        entry: usize,
        cell: (i32, i32),
        now: Instant,
        next_move: Instant,
    ) -> Self {
        Self {
            id,
            mob_id,
            position,
            hp,
            max_hp: hp,
            sp: 0,
            mp: 0,
            entry,
            home: position,
            next_move,
            cell,
            dead_until: None,
            state: MobState::Idle,
            next_attack: now,
            entry_ready: [None; 3],
            next_chase: now,
            next_retarget: None,
            next_new_target: Some(now + Duration::from_secs(5)),
            next_recover: None,
            fair: true,
            origin: Origin::Area,
            scripted: false,
            entry_used: 0,
            lua: false,
            unbeatable: false,
            guard_lock: None,
            client_eta: None,
            moving: Moving::None,
        }
    }
}

/// Who an entry of a monster's damage record is for (`stDamageMob` kind 1 or 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dealer {
    Player(u32),
    /// A party's damage together (`CMob::DamageParty`, `docs/original/party.md` section 7).
    Party(u32),
}

/// One entry of a monster's damage record (`stDamageMob`): damage and aggro summed, and for a player its party when it last hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    pub dealer: Dealer,
    pub damage: u32,
    pub aggro: u32,
    pub party: u32,
}

impl Hit {
    pub fn player(&self) -> Option<u32> {
        match self.dealer {
            Dealer::Player(id) => Some(id),
            Dealer::Party(_) => None,
        }
    }
}

/// The entry with the highest `key`, the first on a tie.
fn most(hits: impl Iterator<Item = Hit>, key: impl Fn(&Hit) -> u32) -> Option<Hit> {
    let mut top: Option<Hit> = None;
    for hit in hits {
        if top.is_none_or(|best| key(&best) < key(&hit)) {
            top = Some(hit);
        }
    }
    top
}

/// One area entry's respawn state (`stMobRespawn`).
#[derive(Clone, Debug)]
pub struct Respawn {
    pub area: usize,
    pub mob_id: u16,
    pub count: u32,
    pub current: u32,
    /// When it may spawn again; `None` is "now" (the original's 0).
    pub due: Option<Instant>,
}

/// The monsters of one map and its respawn entries.
#[derive(Debug, Default)]
pub struct MobMap {
    pub mobs: HashMap<u32, Mob>,
    pub cells: HashMap<(i32, i32), Vec<u32>>,
    pub respawns: Vec<Respawn>,
    /// Damage and aggro on each monster, by player in the order they first hit (`CDamageMob`).
    pub damage: HashMap<u32, Vec<Hit>>,
    /// Each fighting monster's chase path (`ChasePathList`), newest last.
    pub trails: HashMap<u32, Vec<(i32, i32)>>,
    /// Each monster's walking path (`CMob+0x7c`, `docs/original/pathing.md` section 2), the current cell first.
    pub paths: HashMap<u32, crate::fight::Path>,
    /// The debuffs on each monster (`m_applySkill`, `docs/original/skills-buffs.md` section 12).
    pub debuffs: HashMap<u32, Vec<crate::buffs::MobDebuff>>,
    /// The map's obelisks (`docs/original/obelisks.md`).
    pub obelisks: Vec<crate::obelisk::Obelisk>,
    /// The zone's boss chains (`docs/original/obelisks.md` section 11.3).
    pub bosses: Vec<crate::boss::Chain>,
    /// The scripted monsters' machine states (`docs/original/scripted-mobs.md`).
    pub scripts: HashMap<u32, crate::mob_ai::Script>,
    /// The Lua monsters' scripts and stacks (`docs/original/instances.md` sections 9 and 10), made at their first Lua pass.
    pub lua: HashMap<u32, crate::mob_lua::LuaMob>,
    /// Events for Lua monsters from outside the Lua pass (a player's hit, a death), in order: monster, event, argument.
    pub lua_events: Vec<(u32, u8, u32)>,
}

impl MobMap {
    /// The respawn entries of a map's areas; entries without a definition that can spawn are left out, as they never spawn.
    pub fn new(areas: &[SpawnArea], defs: &MobDefs) -> Self {
        let respawns = areas
            .iter()
            .enumerate()
            .flat_map(|(area, a)| a.entries.iter().map(move |&(mob_id, count)| (area, mob_id, count)))
            .filter(|&(_, mob_id, count)| count > 0 && defs.get(mob_id).is_some())
            .map(|(area, mob_id, count)| Respawn { area, mob_id, count, current: 0, due: None })
            .collect();
        Self { respawns, ..Default::default() }
    }

    pub fn insert(&mut self, mob: Mob) {
        if mob.scripted {
            self.scripts.insert(mob.id, crate::mob_ai::Script::new());
        }
        self.cells.entry(mob.cell).or_default().push(mob.id);
        self.mobs.insert(mob.id, mob);
    }

    /// Records damage and aggro of a player in `party` (0 for none) (`CDamageMob::Add`).
    pub fn add_damage(&mut self, mob: u32, player: u32, damage: u32, aggro: u32, party: u32) {
        let list = self.damage.entry(mob).or_default();
        match list.iter_mut().find(|h| h.dealer == Dealer::Player(player)) {
            Some(h) => {
                h.damage = h.damage.saturating_add(damage);
                h.aggro = h.aggro.saturating_add(aggro);
                h.party = party;
            }
            None => list.push(Hit { dealer: Dealer::Player(player), damage, aggro, party }),
        }
    }

    /// Records a party's damage together (`CMob::DamageParty`), before its member's own.
    pub fn add_party_damage(&mut self, mob: u32, party: u32, damage: u32) {
        let list = self.damage.entry(mob).or_default();
        match list.iter_mut().find(|h| h.dealer == Dealer::Party(party)) {
            Some(h) => h.damage = h.damage.saturating_add(damage),
            None => list.push(Hit { dealer: Dealer::Party(party), damage, aggro: 0, party }),
        }
    }

    /// The killer of record, the record cleared (`CDamageMob::GetMaxDamage`): the entry with the most damage, the first on a tie;
    /// for a party, its member with the most damage of its own (`docs/original/party.md` section 7).
    pub fn take_top_damage(&mut self, mob: u32) -> Option<u32> {
        let list = self.damage.remove(&mob)?;
        let top = most(list.iter().copied(), |h| h.damage)?;
        match top.dealer {
            Dealer::Player(id) => Some(id),
            Dealer::Party(party) => {
                most(list.iter().copied().filter(|h| h.player().is_some() && h.party == party), |h| h.damage).and_then(|h| h.player())
            }
        }
    }

    /// The player with the most aggro, the first on a tie (`CDamageMob::GetMaxAggro`).
    pub fn top_aggro(&self, mob: u32) -> Option<u32> {
        most(self.damage.get(&mob)?.iter().copied().filter(|h| h.player().is_some()), |h| h.aggro).and_then(|h| h.player())
    }

    /// Takes a monster out of its cell and the map.
    /// What a monster's debuffs add up to (nothing without any).
    pub fn effects(&self, id: u32) -> crate::buffs::MobEffects {
        self.debuffs.get(&id).map(|list| crate::buffs::mob_effects(list)).unwrap_or_default()
    }

    /// Attack and move speed with the debuffs (`CMob::GetAttackSpeed`, `GetMoveSpeed`; 255: cannot).
    pub fn speeds(&self, id: u32) -> (u8, u8) {
        self.effects(id).speeds()
    }

    pub fn remove(&mut self, id: u32) {
        self.debuffs.remove(&id);
        let Some(mob) = self.mobs.remove(&id) else { return };
        if let Some(ids) = self.cells.get_mut(&mob.cell) {
            ids.retain(|&other| other != id);
            if ids.is_empty() {
                self.cells.remove(&mob.cell);
            }
        }
        self.damage.remove(&id);
        self.trails.remove(&id);
        self.paths.remove(&id);
        self.scripts.remove(&id);
        self.lua.remove(&id);
        self.lua_events.retain(|&(mob, _, _)| mob != id);
    }

    /// The script flags of a monster with a machine (`docs/original/scripted-mobs.md` section 7).
    pub fn script_flags(&self, id: u32) -> Option<crate::mob_ai::Flags> {
        self.scripts.get(&id).map(|s| s.flags)
    }

    /// Files a monster under another cell.
    pub fn change_cell(&mut self, id: u32, to: (i32, i32)) {
        let Some(mob) = self.mobs.get_mut(&id) else { return };
        if let Some(ids) = self.cells.get_mut(&mob.cell) {
            ids.retain(|&other| other != id);
            if ids.is_empty() {
                self.cells.remove(&mob.cell);
            }
        }
        mob.cell = to;
        self.cells.entry(to).or_default().push(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn respawn_codes_follow_the_original_table() {
        assert_eq!(respawn_time(0), Duration::from_secs(5));
        assert_eq!(respawn_time(3), Duration::from_secs(180));
        assert_eq!(respawn_time(13), Duration::from_secs(7200));
        assert_eq!(respawn_time(14), Duration::ZERO);
        assert_eq!(respawn_time(200), Duration::from_secs(5), "above 15 counts as 0");
    }

    #[test]
    fn a_monsters_quest_item_comes_from_the_column() {
        assert_eq!(quest_item_of(0), None);
        assert_eq!(quest_item_of(-5), None);
        assert_eq!(quest_item_of(1), Some((27, 1)));
        assert_eq!(quest_item_of(255), Some((27, 255)));
        assert_eq!(quest_item_of(256), Some((27, 0)), "the byte wraps as the original's");
        assert_eq!(quest_item_of(1001), Some((28, 1)));
        assert_eq!(quest_item_of(1145), Some((28, 145)));
        assert_eq!(quest_item_of(2001), Some((29, 1)));
    }

    #[test]
    fn areas_spread_from_their_first_corner() {
        let area = SpawnArea::new([100.5, 10.0, 200.0, 110.9, 12.0, 199.0], vec![(1, 3)]);
        assert_eq!((area.delta_x, area.delta_z), (10, 1), "trunc(10.4), at least 1");
        assert_eq!(area.spot(23, 5), [103.5, 10.0, 200.0]);
    }

    #[test]
    fn only_entries_that_can_spawn_are_kept() {
        let defs = MobDefs::new([
            MobDef { id: 1, level: 1, hp: 100, ..Default::default() },
            MobDef { id: 2, level: 1, hp: 0, ..Default::default() },
        ]);
        let areas = [SpawnArea::new([0.0; 6], vec![(1, 3), (2, 1), (9, 1), (1, 0)])];
        let map = MobMap::new(&areas, &defs);
        assert_eq!(map.respawns.iter().map(|r| (r.mob_id, r.count)).collect::<Vec<_>>(), [(1, 3)]);
    }

    #[test]
    fn a_party_kills_through_its_top_member() {
        let mut map = MobMap::default();
        // A solo player 3 does 50; party 9's members 1 and 2 do 30 each: the party's 60 wins, and 1 is the first of its best.
        map.add_party_damage(5, 9, 30);
        map.add_damage(5, 1, 30, 300, 9);
        map.add_damage(5, 3, 50, 900, 0);
        map.add_party_damage(5, 9, 30);
        map.add_damage(5, 2, 30, 0, 9);
        assert_eq!(map.top_aggro(5), Some(3), "aggro is the players' alone");
        assert_eq!(map.take_top_damage(5), Some(1));
        // Alone against a stronger solo player, the party loses.
        map.add_party_damage(6, 9, 10);
        map.add_damage(6, 1, 10, 0, 9);
        map.add_damage(6, 3, 11, 0, 0);
        assert_eq!(map.take_top_damage(6), Some(3));
        // A tie goes to whoever hit first; the party's entry comes before its member's.
        map.add_damage(7, 3, 10, 0, 0);
        map.add_party_damage(7, 9, 10);
        map.add_damage(7, 2, 10, 0, 9);
        assert_eq!(map.take_top_damage(7), Some(3));
    }

    fn walker(ai: u8, normal_step: u8) -> MobDef {
        MobDef { id: 1, level: 1, hp: 10, ai, normal_step, chase_step: 8, normal_time: Duration::from_secs(4), ..Default::default() }
    }

    fn mob_at(position: [f32; 3]) -> Mob {
        Mob::new(1, 1, position, 10, 0, (0, 0), Instant::now(), Instant::now())
    }

    #[test]
    fn idle_steps_follow_the_original_formula() {
        let mob = mob_at([100.0, 5.0, 200.0]);
        // 90 degrees, 4 extra units: 8 + 4 = 12 towards -x.
        let mut rolls = [90, 499, 1234].into_iter();
        let (to, wait) = idle_turn(&mob, &walker(1, 8), MOVE_SPEED, &mut || rolls.next().unwrap()).unwrap();
        let to = to.unwrap();
        assert!((to[0] - 88.0).abs() < 1e-3 && to[1] == 5.0 && (to[2] - 200.0).abs() < 1e-3, "{to:?}");
        assert_eq!(wait, Duration::from_millis(10_000 + 4_000 + 1_234));
        // 10000 is 0 degrees, towards +z; 100 rolls 1 extra unit.
        let mut rolls = [10_000, 100, 0].into_iter();
        let (to, _) = idle_turn(&mob, &walker(1, 8), MOVE_SPEED, &mut || rolls.next().unwrap()).unwrap();
        assert_eq!(to, Some([100.0, 5.0, 209.0]));
    }

    #[test]
    fn some_monsters_do_not_walk() {
        let mob = mob_at([100.0, 5.0, 200.0]);
        let mut roll = || 2_000;
        assert_eq!(
            idle_turn(&mob, &walker(4, 8), MOVE_SPEED, &mut roll),
            Some((None, Duration::from_millis(12_000))),
            "AI 4, no normal time"
        );
        assert_eq!(idle_turn(&mob, &walker(1, 0), MOVE_SPEED, &mut roll), Some((None, Duration::from_millis(12_000))), "no step");
        assert_eq!(idle_turn(&mob, &walker(100, 8), MOVE_SPEED, &mut roll), None, "Lua");
        let running = Mob { state: MobState::Scripted(None), ..mob };
        assert_eq!(idle_turn(&running, &walker(10, 8), MOVE_SPEED, &mut roll), None, "running its machine");
        assert_eq!(idle_turn(&mob, &walker(1, 8), crate::buffs::CANNOT, &mut roll), None, "stopped, stunned or asleep");
        assert_eq!(first_move_delay(&mut || 32_767), Duration::from_millis(32_767));
    }
}
