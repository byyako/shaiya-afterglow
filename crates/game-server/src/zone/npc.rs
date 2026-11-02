//! NPCs that walk and guards that fight (`docs/original/guards.md`): each map's NPCs with their positions and cells, the walk
//! (`CNpc::UpdateMove`), the guards' turns (`CNpc::UpdateGuard`), and the players' current NPC (`0x0E04`). `crate::guard` has the
//! pure steps.

use crate::sync::Relock;
use std::collections::{BTreeMap, HashMap};
use std::time::Instant;

use shaiya_proto::combat as cp;
use shaiya_proto::skill as sk;
use shaiya_proto::world::{self as w, NpcEnter};

use super::{
    ATTACK_RANGE, Pass, Placed, ZoneEvent, cell_of, distance, duel, in_view, keeps_cell, lua, mob_skills, pvp, send_around, view_cells,
};
use crate::drops::DropTables;
use crate::fight::Path;
use crate::guard::{self, Back, Chase, GUARD};
use crate::inventory::ItemDefs;
use crate::mobs::MobMap;
use crate::world::{MapTerrain, NpcDef, World};

/// A guard's state (`m_nStatus`; 2 is never set).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(super) enum Status {
    #[default]
    Idle,
    Fighting,
    Back,
}

/// A guard's target (`m_Target`; type 0 is `None`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Target {
    Player(u32),
    Mob(u32),
}

/// What an NPC is in the middle of between the passes (`docs/original/chase-smoothing.md` deviations 2 and 5): the move tick takes
/// the next step of it and nothing else.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(super) enum Moving {
    #[default]
    None,
    Chase,
    Back,
    Walk,
}

/// One NPC (`CNpc`).
#[derive(Clone, Debug)]
pub(super) struct Npc {
    pub enter: NpcEnter,
    pub cell: (i32, i32),
    pub def: NpcDef,
    /// Its positions (`stMapNpc::pPos`, at least the first); two or more make a walk.
    pub points: Vec<[f32; 3]>,
    /// The position walked to (`m_nPosNext`) and the way along them (`m_bDir`).
    pub next_point: usize,
    pub forwards: bool,
    /// `m_dwNextMoveTime`, `m_dwNextChaseTime` and `m_dwDisableTime` (`None` is 0).
    pub next_move: Option<Instant>,
    pub next_chase: Option<Instant>,
    pub disabled_until: Option<Instant>,
    /// A guard on a blocked square at creation (`m_dwDisableTime` `0xFFFFFFFF`): never updated.
    pub stuck: bool,
    /// Players who have it as their current NPC (`m_nNumUsing`).
    pub users: u32,
    pub status: Status,
    pub target: Option<Target>,
    /// The path it walks and the cells it ran through (`m_AStar`, `m_ChasePath`).
    pub path: Path,
    pub trail: Vec<(i32, i32)>,
    /// When the client gets it to its last spot, and what it is in the middle of (`docs/original/chase-smoothing.md`).
    pub client_eta: Option<Instant>,
    pub moving: Moving,
}

impl Npc {
    fn is_guard(&self) -> bool {
        self.enter.kind == GUARD
    }

    /// The position it walks to, which is also where a guard goes back to.
    fn point(&self) -> [f32; 3] {
        self.points.get(self.next_point).copied().unwrap_or(self.enter.position)
    }
}

/// The NPCs of one zone, by object id (the original's list order) and by cell. A party or guild zone has its own (`CZone::MapInit`).
#[derive(Default)]
pub(super) struct NpcMap {
    pub npcs: BTreeMap<u32, Npc>,
    pub cells: HashMap<(i32, i32), Vec<u32>>,
}

impl NpcMap {
    fn change_cell(&mut self, id: u32, from: (i32, i32), to: (i32, i32)) {
        if let Some(list) = self.cells.get_mut(&from) {
            list.retain(|&n| n != id);
        }
        self.cells.entry(to).or_default().push(id);
    }
}

/// A zone's NPCs as `CMap::InitNPC` places them from its map's list, each with the object id `id` gives it.
pub(super) fn place(world: &World, zone: u16, list: &[NpcEnter], mut id: impl FnMut(&NpcEnter) -> u32) -> NpcMap {
    let map = crate::world::map_of(zone);
    let mut m = NpcMap::default();
    for n in list {
        let cell = cell_of(n.position);
        let points = world.npc_walk(n.object_id).map_or_else(|| vec![n.position], <[_]>::to_vec);
        let [x, _, z] = n.position;
        let object_id = id(n);
        let npc = Npc {
            enter: NpcEnter { object_id, ..*n },
            cell,
            def: world.npc_def(n.kind, n.type_id),
            points,
            next_point: 0,
            forwards: true,
            next_move: None,
            next_chase: None,
            disabled_until: None,
            // "npc guard move block": only guards are checked.
            stuck: n.kind == GUARD && world.is_blocked(map, x as i32, z as i32),
            users: 0,
            status: Status::Idle,
            target: None,
            path: Path::new(),
            trail: Vec::new(),
            client_eta: None,
            moving: Moving::None,
        };
        m.npcs.insert(object_id, npc);
        m.cells.entry(cell).or_default().push(object_id);
    }
    m
}

/// The static zones' NPCs (maps without a `CreateType`), with the world's object ids, and the highest id given.
pub(super) fn build(world: &World) -> (HashMap<u16, NpcMap>, u32) {
    let last = world.npcs().values().flatten().map(|n| n.object_id).max().unwrap_or(0);
    let maps = world
        .npcs()
        .iter()
        .filter(|&(&map, _)| world.create_type(map) == 0)
        .map(|(&map, list)| (map, place(world, map, list, |n| n.object_id)))
        .collect();
    (maps, last)
}

/// A new party or guild zone's NPCs: its map's, with new object ids from the counter (`CZone::MapInit`).
pub(super) fn for_zone(world: &World, zone: u16, last_id: &std::sync::atomic::AtomicU32) -> NpcMap {
    let list = world.npcs().get(&crate::world::map_of(zone)).map_or(&[][..], Vec::as_slice);
    place(world, zone, list, |_| last_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1)
}

/// The NPCs in the cells of `view` that are not in the view of `except` (all of them without one).
pub(super) fn in_sight(npcs: Option<&NpcMap>, view: (i32, i32), except: Option<(i32, i32)>) -> Vec<NpcEnter> {
    let Some(npcs) = npcs else { return Vec::new() };
    view_cells(view)
        .filter(|&cell| except.is_none_or(|other| !in_view(other, cell)))
        .filter_map(|cell| npcs.cells.get(&cell))
        .flatten()
        .filter_map(|id| npcs.npcs.get(id).map(|n| n.enter))
        .collect()
}

/// A player lets go of its current NPC (`CUser::LeaveZone`, or more than 10 away after a move): the NPC's user count goes down.
pub(super) fn let_go(npcs: Option<&mut NpcMap>, p: &mut Placed) {
    let Some(id) = p.npc.take() else { return };
    if let Some(n) = npcs.and_then(|m| m.npcs.get_mut(&id)) {
        n.users = n.users.saturating_sub(1);
    }
}

/// After a move: more than 10 from its current NPC, the player lets go of it.
pub(super) fn let_go_if_far(npcs: Option<&mut NpcMap>, p: &mut Placed) {
    let Some(id) = p.npc else { return };
    let Some(npcs) = npcs else { return };
    let far = npcs.npcs.get(&id).is_none_or(|n| distance(p.occupant.position, n.enter.position) > guard::USE_RANGE);
    if far {
        let_go(Some(npcs), p);
    }
}

/// `CZone::MoveNpc`: nothing off the map; past its cell's margin the NPC changes cells (by its new spot, deviation 1) and leaves and
/// enters the views around (`0x0E02`/`0x0E01`, still at the old spot); then everyone in the view of its cell gets `0x0E03`.
fn move_npc(world: &World, map: u16, npcs: &mut NpcMap, players: &HashMap<u32, Placed>, id: u32, to: [f32; 3], motion: u8) -> bool {
    if !world.is_safe_pos(map, to) {
        return false;
    }
    let Some(n) = npcs.npcs.get(&id) else { return false };
    let (old, old_cell) = (n.enter, n.cell);
    let cell = if keeps_cell(old_cell, to) { old_cell } else { cell_of(to) };
    if cell != old_cell {
        npcs.change_cell(id, old_cell, cell);
        for p in players.values() {
            match (in_view(p.cell, old_cell), in_view(p.cell, cell)) {
                (true, false) => p.send(&w::npc_leave(id)),
                (false, true) => p.send(&w::npc_enter(&old)),
                _ => {}
            }
        }
    }
    let n = npcs.npcs.get_mut(&id).expect("listed");
    if motion != 0 {
        // Guards chasing or going back, for checking their steps in a live test.
        tracing::debug!(map, id, from = ?old.position, to = ?to, step = distance(old.position, to), "guard run step");
    }
    n.enter.position = to;
    n.cell = cell;
    let packet = w::npc_move(id, motion, to);
    for p in players.values().filter(|p| in_view(p.cell, cell)) {
        p.send(&packet);
    }
    true
}

/// The next step time after a step from `from` to `to` that was made (`None`: none): the original's `next`, or with smoothing the
/// client's arrival there less a tick (`docs/original/chase-smoothing.md` deviation 5: the client walks an NPC at `nMoveDist /
/// nMoveSpeed`, four times that running). Keeps the arrival time and what the move tick may go on with.
#[allow(clippy::too_many_arguments)]
fn paced_next(
    n: &mut Npc,
    from: [f32; 3],
    to: Option<[f32; 3]>,
    run: bool,
    next: Instant,
    now: Instant,
    smooth: bool,
    moving: Moving,
) -> Instant {
    n.moving = Moving::None;
    let Some(to) = to.filter(|_| smooth) else { return next };
    let Some(v) = crate::pacing::npc_speed(n.def.move_dist, n.def.move_speed, run) else { return next };
    let (eta, paced) = crate::pacing::paced(now, n.client_eta, distance(from, to), v);
    n.client_eta = Some(eta);
    n.moving = moving;
    paced
}

/// A walk's turn (`CNpc::UpdateMove`, section 3): nothing while someone uses the NPC, before its next move, or without a walk.
fn walk_turn<R: FnMut() -> u32>(pass: &mut Pass<'_, R>, world: &World, npcs: &mut NpcMap, players: &HashMap<u32, Placed>, id: u32) {
    let Some(n) = npcs.npcs.get(&id) else { return };
    if n.users > 0 || n.next_move.is_some_and(|t| pass.now < t) || n.points.len() < 2 {
        return;
    }
    let from = n.enter.position;
    let Some(turn) = guard::walk(from, n.point(), &n.def, pass.now, &mut *pass.rand) else { return };
    let made = move_npc(world, pass.map, npcs, players, id, turn.to, 0);
    let n = npcs.npcs.get_mut(&id).expect("listed");
    if turn.arrival && made {
        (n.next_point, n.forwards) = guard::next_point(n.next_point, n.forwards, n.points.len());
    }
    let to = (made && !turn.arrival).then_some(turn.to);
    n.next_move = Some(paced_next(n, from, to, false, turn.next, pass.now, pass.zones.smooth_chase(), Moving::Walk));
}

/// What a guard sees when it looks around (`CNpc::FindTarget`, section 5): the first player, then monster, of each of the 3 x 3
/// cells around it (rows of z, then x) that is of another country, alive, within 20 of it and 30 of its position, and not taken
/// in the last 5 s. Players as aggressive monsters take them (no shape), in id order within a cell.
fn find_target(
    world: &World,
    map: u16,
    n: &Npc,
    players: &HashMap<u32, Placed>,
    mobs: &MobMap,
    pass_defs: &crate::mobs::MobDefs,
    now: Instant,
) -> Option<Target> {
    let at = n.enter.position;
    if world.is_blocked(map, at[0] as i32, at[2] as i32) {
        return None;
    }
    let point = n.point();
    let near = |position: [f32; 3]| distance(position, at) <= guard::SIGHT && distance(position, point) <= guard::LEASH;
    let free = |lock: Option<Instant>| lock.is_none_or(|t| now >= t);
    let country = n.def.country;
    for dz in -1..=1 {
        for dx in -1..=1 {
            let cell = (n.cell.0 + dx, n.cell.1 + dz);
            let mut here: Vec<&Placed> = players.values().filter(|p| p.cell == cell).collect();
            here.sort_unstable_by_key(|p| p.occupant.id);
            let player = here.into_iter().find(|p| {
                p.occupant.shape.country != country
                    && !p.dead
                    && p.occupant.visible
                    && p.occupant.attackable
                    && p.form.shape == 0
                    && near(p.occupant.position)
                    && free(p.guard_lock)
            });
            if let Some(p) = player {
                return Some(Target::Player(p.occupant.id));
            }
            let mob = mobs.cells.get(&cell).into_iter().flatten().filter_map(|id| mobs.mobs.get(id)).find(|m| {
                let other = pass_defs.get(m.mob_id).is_some_and(|d| country == 2 || d.country != country);
                other && m.dead_until.is_none() && near(m.position) && free(m.guard_lock)
            });
            if let Some(m) = mob {
                return Some(Target::Mob(m.id));
            }
        }
    }
    None
}

/// `FindTarget` with its `SetTarget`: a guard already fighting keeps its target; otherwise it fights what it found, which stays
/// taken for 5 s. Whether anything was found.
fn look<R: FnMut() -> u32>(pass: &Pass<'_, R>, world: &World, n: &mut Npc, players: &mut HashMap<u32, Placed>, mobs: &mut MobMap) -> bool {
    let Some(found) = find_target(world, pass.map, n, players, mobs, pass.defs, pass.now) else { return false };
    if n.status == Status::Fighting {
        return true;
    }
    n.status = Status::Fighting;
    n.target = Some(found);
    let until = Some(pass.now + guard::TAKEN);
    match found {
        Target::Player(id) => players.get_mut(&id).iter_mut().for_each(|p| p.guard_lock = until),
        Target::Mob(id) => mobs.mobs.get_mut(&id).iter_mut().for_each(|m| m.guard_lock = until),
    }
    true
}

/// After a blow that ended the target, or one on a dead target: look again; nobody: back to its position.
fn look_or_back<R: FnMut() -> u32>(pass: &Pass<'_, R>, world: &World, n: &mut Npc, players: &mut HashMap<u32, Placed>, mobs: &mut MobMap) {
    if !look(pass, world, n, players, mobs) {
        n.status = Status::Back;
    }
}

fn drop_target(n: &mut Npc) {
    n.status = Status::Idle;
    n.target = None;
}

/// A guard's turn (`CNpc::UpdateGuard`, section 4).
fn guard_turn<R: FnMut() -> u32>(
    pass: &mut Pass<'_, R>,
    world: &World,
    npcs: &mut NpcMap,
    mobs: &mut MobMap,
    players: &mut HashMap<u32, Placed>,
    id: u32,
) {
    let Some(n) = npcs.npcs.get_mut(&id) else { return };
    match n.status {
        Status::Idle => {
            if look(pass, world, n, players, mobs) {
                attack_turn(pass, world, npcs, mobs, players, id);
            } else {
                walk_turn(pass, world, npcs, players, id);
            }
        }
        Status::Fighting => attack_turn(pass, world, npcs, mobs, players, id),
        // Deviation 7: on the way back it looks too (the original only walks), and a target found ends the walk home.
        Status::Back => {
            if look(pass, world, n, players, mobs) {
                n.path = Path::new();
                n.trail = Vec::new();
                attack_turn(pass, world, npcs, mobs, players, id);
            } else {
                back_turn(pass, world, npcs, players, id);
            }
        }
    }
}

/// `CNpc::UpdateAttackAI` (section 6): the target looked up on the map, chased into range, struck.
fn attack_turn<R: FnMut() -> u32>(
    pass: &mut Pass<'_, R>,
    world: &World,
    npcs: &mut NpcMap,
    mobs: &mut MobMap,
    players: &mut HashMap<u32, Placed>,
    id: u32,
) {
    let Some(n) = npcs.npcs.get_mut(&id) else { return };
    let at = match n.target {
        None => return,
        Some(Target::Player(pid)) => match players.get(&pid) {
            None => return drop_target(n),
            // Deviation 6: a dead player is dropped and the guard looks again at once.
            Some(p) if p.dead => {
                drop_target(n);
                return look_or_back(pass, world, n, players, mobs);
            }
            Some(p) => p.occupant.position,
        },
        Some(Target::Mob(mid)) => match mobs.mobs.get(&mid) {
            None => return drop_target(n),
            Some(m) => m.position,
        },
    };
    let terrain = MapTerrain { world, map: pass.map };
    let (mut path, mut trail) = (std::mem::take(&mut n.path), std::mem::take(&mut n.trail));
    let next_chase = n.next_chase.unwrap_or(pass.now);
    let c = guard::chase(n.enter.position, at, n.point(), &n.def, next_chase, pass.now, &terrain, &mut path, &mut trail);
    (n.path, n.trail) = (path, trail);
    if !matches!(c, Chase::Wait | Chase::Step { .. }) {
        n.moving = Moving::None;
    }
    match c {
        Chase::Wait => {}
        Chase::InRange => match n.target {
            Some(Target::Player(pid)) => strike_player(pass, world, npcs, mobs, players, id, pid),
            Some(Target::Mob(mid)) => strike_mob(pass, world, npcs, mobs, players, id, mid),
            None => {}
        },
        Chase::Blocked | Chase::Stop => drop_target(n),
        Chase::GiveUp { stuck } => {
            n.status = Status::Back;
            n.target = None;
            if stuck {
                n.disabled_until = Some(pass.now + guard::STUCK);
            }
        }
        Chase::Step { to, then, next } => {
            let from = n.enter.position;
            let made = to.filter(|&to| move_npc(world, pass.map, npcs, players, id, to, 1));
            let n = npcs.npcs.get_mut(&id).expect("listed");
            if made.is_some()
                && let Some(then) = then
            {
                // Put on the cell without telling anyone (the original's `m_vPos = position + v`).
                n.enter.position = then;
            }
            n.next_chase = Some(paced_next(n, from, made, true, next, pass.now, pass.zones.smooth_chase(), Moving::Chase));
        }
    }
}

/// A turn of going back to its position (`CNpc::ReturnHome`, section 9).
fn back_turn<R: FnMut() -> u32>(pass: &mut Pass<'_, R>, world: &World, npcs: &mut NpcMap, players: &HashMap<u32, Placed>, id: u32) {
    let Some(n) = npcs.npcs.get_mut(&id) else { return };
    let terrain = MapTerrain { world, map: pass.map };
    let (mut path, mut trail) = (std::mem::take(&mut n.path), std::mem::take(&mut n.trail));
    let next_chase = n.next_chase.unwrap_or(pass.now);
    let b = guard::back(n.enter.position, n.point(), &n.def, next_chase, pass.now, &terrain, &mut path, &mut trail);
    (n.path, n.trail) = (path, trail);
    if !matches!(b, Back::Wait | Back::Step { .. }) {
        n.moving = Moving::None;
    }
    match b {
        Back::Wait => {}
        Back::Arrived => n.status = Status::Idle,
        Back::Idle { stuck } => {
            n.status = Status::Idle;
            if stuck {
                n.disabled_until = Some(pass.now + guard::STUCK);
            }
        }
        Back::Step { to, next } => {
            let from = n.enter.position;
            let made = to.filter(|&to| move_npc(world, pass.map, npcs, players, id, to, 1));
            let n = npcs.npcs.get_mut(&id).expect("listed");
            n.next_chase = Some(paced_next(n, from, made, true, next, pass.now, pass.zones.smooth_chase(), Moving::Back));
        }
    }
}

/// The move tick for one zone's NPCs (`docs/original/chase-smoothing.md` deviations 2 and 5): a guard chasing or going back, or an
/// NPC walking, whose next step has come takes that step and nothing else (no look around, no blow, no arrival); anything else is
/// left to its next pass.
pub(super) fn move_tick(world: &World, map: u16, npcs: &mut NpcMap, mobs: &MobMap, players: &HashMap<u32, Placed>, now: Instant) {
    let due: Vec<u32> = npcs
        .npcs
        .values()
        .filter(|n| n.moving != Moving::None && !n.stuck && n.disabled_until.is_none_or(|t| now >= t))
        .map(|n| n.enter.object_id)
        .collect();
    let terrain = MapTerrain { world, map };
    for id in due {
        let Some(n) = npcs.npcs.get_mut(&id) else { continue };
        let from = n.enter.position;
        match n.moving {
            Moving::None => {}
            Moving::Chase => {
                let at = match n.target {
                    Some(Target::Player(pid)) => players.get(&pid).filter(|p| !p.dead).map(|p| p.occupant.position),
                    Some(Target::Mob(mid)) => mobs.mobs.get(&mid).filter(|m| m.dead_until.is_none()).map(|m| m.position),
                    None => None,
                };
                let Some(at) = at.filter(|_| n.status == Status::Fighting) else {
                    n.moving = Moving::None;
                    continue;
                };
                let next_chase = n.next_chase.unwrap_or(now);
                if now < next_chase {
                    continue;
                }
                let (mut path, mut trail) = (n.path.clone(), n.trail.clone());
                let Chase::Step { to: Some(to), then, next } =
                    guard::chase(from, at, n.point(), &n.def, next_chase, now, &terrain, &mut path, &mut trail)
                else {
                    n.moving = Moving::None;
                    continue;
                };
                if !move_npc(world, map, npcs, players, id, to, 1) {
                    npcs.npcs.get_mut(&id).expect("listed").moving = Moving::None;
                    continue;
                }
                let n = npcs.npcs.get_mut(&id).expect("listed");
                (n.path, n.trail) = (path, trail);
                if let Some(then) = then {
                    n.enter.position = then;
                }
                n.next_chase = Some(paced_next(n, from, Some(to), true, next, now, true, Moving::Chase));
            }
            Moving::Back => {
                let next_chase = n.next_chase.unwrap_or(now);
                if n.status != Status::Back {
                    n.moving = Moving::None;
                    continue;
                }
                if now < next_chase {
                    continue;
                }
                let (mut path, mut trail) = (n.path.clone(), n.trail.clone());
                let Back::Step { to: Some(to), next } =
                    guard::back(from, n.point(), &n.def, next_chase, now, &terrain, &mut path, &mut trail)
                else {
                    n.moving = Moving::None;
                    continue;
                };
                if !move_npc(world, map, npcs, players, id, to, 1) {
                    npcs.npcs.get_mut(&id).expect("listed").moving = Moving::None;
                    continue;
                }
                let n = npcs.npcs.get_mut(&id).expect("listed");
                (n.path, n.trail) = (path, trail);
                n.next_chase = Some(paced_next(n, from, Some(to), true, next, now, true, Moving::Back));
            }
            Moving::Walk => {
                if n.users > 0 || n.points.len() < 2 || n.status != Status::Idle {
                    n.moving = Moving::None;
                    continue;
                }
                if n.next_move.is_some_and(|t| now < t) {
                    continue;
                }
                // An arrival draws the wait at the next position: left to the pass.
                let mut drawn = false;
                let turn = guard::walk(from, n.point(), &n.def, now, &mut || {
                    drawn = true;
                    0
                });
                let Some(turn) = turn.filter(|t| !t.arrival && !drawn) else {
                    n.moving = Moving::None;
                    continue;
                };
                if !move_npc(world, map, npcs, players, id, turn.to, 0) {
                    npcs.npcs.get_mut(&id).expect("listed").moving = Moving::None;
                    continue;
                }
                let n = npcs.npcs.get_mut(&id).expect("listed");
                n.next_move = Some(paced_next(n, from, Some(turn.to), false, turn.next, now, true, Moving::Walk));
            }
        }
    }
}

/// When the guard's next turn is due: after its blow, `wait` (`m_dwDisableTime`, only when it has passed).
fn rest(n: &mut Npc, now: Instant, wait: std::time::Duration) {
    if n.disabled_until.is_none_or(|t| t <= now) {
        n.disabled_until = Some(now + wait);
    }
}

/// `CNpc::Attack` on a player (section 6): the player's whole HP (less 1 with absorption at half HP or more, `0x0525` with the
/// absorption), the death by a guard (`zone::pvp::player_died`, killer kind 3, no killer of record), then the blow within 60
/// (`0x0E05`) and on success a new look.
fn strike_player<R: FnMut() -> u32>(
    pass: &mut Pass<'_, R>,
    world: &World,
    npcs: &mut NpcMap,
    mobs: &mut MobMap,
    players: &mut HashMap<u32, Placed>,
    id: u32,
    pid: u32,
) {
    let (now, zones, map) = (pass.now, pass.zones, pass.map);
    let at = npcs.npcs[&id].enter.position;
    let mut success = true;
    if players.get(&pid).is_some_and(|p| !p.dead) {
        // `CUser::StatusCastingStop` and `CUser::PvPChkLeaveByAttack`.
        mob_skills::struck(players, pid);
        duel::hit(zones, map, players, pid, None, now);
        let p = players.get_mut(&pid).expect("listed");
        let hp = p.occupant.vitals.health[0];
        let mut died = false;
        if hp > 0 {
            let v = &p.occupant.vitals;
            let mut damage = hp;
            if v.absorption != 0 && i32::from(damage) >= i32::from(v.max[0]) / 2 {
                damage -= 1;
                p.send(&cp::absorbed(v.absorption as u16));
            }
            let effects = p.occupant.buffs.effects();
            if effects.shape == 0 && !effects.no_wear {
                let _ = p.occupant.events.send(ZoneEvent::Worn);
            }
            pvp::record(&mut p.harm, cp::KILLER_NPC, id, damage);
            let v = &mut p.occupant.vitals;
            if v.health[0] <= damage {
                died = true;
                v.health = [0; 3];
                p.dead = true;
                p.attacking = false;
                pvp::player_died(zones, map, mobs, players, pid, (cp::KILLER_NPC, id), &effects, None, false, now);
                drop_target(npcs.npcs.get_mut(&id).expect("listed"));
            } else {
                v.health[0] -= damage;
                let (left, max) = (v.health[0], v.max[0]);
                if let Some(a) = p.occupant.buffs.low_hp_check(left, max, now, &mut || zones.new_key()) {
                    p.send(&sk::applied(a.key, a.def.id, a.def.level));
                    let _ = p.occupant.events.send(ZoneEvent::Buffs);
                }
            }
        }
        success = died;
        send_around(Some(players), at, ATTACK_RANGE, &w::npc_attack(true, id, pid, hp));
    }
    let n = npcs.npcs.get_mut(&id).expect("listed");
    if success {
        look_or_back(pass, world, n, players, mobs);
    }
    rest(n, now, if success { guard::AFTER_KILL } else { guard::AFTER_BLOW });
}

/// `CNpc::Attack` on a monster (section 6): its whole HP; the kill gives nothing (fair combat cleared): `0x0606` with killer kind 3 to
/// its view, its obelisk's rules; the blow within 60 (`0x0E06`); then a new look.
fn strike_mob<R: FnMut() -> u32>(
    pass: &mut Pass<'_, R>,
    world: &World,
    npcs: &mut NpcMap,
    mobs: &mut MobMap,
    players: &mut HashMap<u32, Placed>,
    id: u32,
    mid: u32,
) {
    let now = pass.now;
    let at = npcs.npcs[&id].enter.position;
    if let Some(mob) = mobs.mobs.get(&mid).copied().filter(|m| m.dead_until.is_none() && m.hp > 0) {
        guard_kill(pass, mobs, players, mid, id);
        drop_target(npcs.npcs.get_mut(&id).expect("listed"));
        send_around(Some(players), at, ATTACK_RANGE, &w::npc_attack(false, id, mid, mob.hp.min(u32::from(u16::MAX)) as u16));
    }
    let n = npcs.npcs.get_mut(&id).expect("listed");
    look_or_back(pass, world, n, players, mobs);
    rest(n, now, guard::AFTER_BLOW);
}

/// `CMob::Damage(3, guard, HP)` and `CMob::Death(3, guard)` with fair combat cleared: the body, the script's hit count, the record and
/// debuffs gone, a Lua monster's death, `0x0606` to its view, and what an obelisk monster's death does.
fn guard_kill<R: FnMut() -> u32>(pass: &mut Pass<'_, R>, mobs: &mut MobMap, players: &HashMap<u32, Placed>, mid: u32, guard: u32) {
    let Some(def) = mobs.mobs.get(&mid).and_then(|m| pass.defs.get(m.mob_id)) else { return };
    if let Some(s) = mobs.scripts.get_mut(&mid)
        && s.flags.counting
    {
        s.flags.count = s.flags.count.wrapping_add(1);
    }
    let m = mobs.mobs.get_mut(&mid).expect("listed");
    m.fair = false;
    m.hp = 0;
    let body = if def.respawn.is_zero() { 5 } else { 10 };
    m.dead_until = Some(pass.now + std::time::Duration::from_secs(body));
    let mob = *m;
    mobs.take_top_damage(mid);
    mobs.debuffs.remove(&mid);
    lua::lua_killed(mobs, mid);
    let death = cp::mob_death(mid, cp::KILLER_NPC, guard);
    for p in players.values().filter(|p| in_view(p.cell, mob.cell)) {
        p.send(&death);
    }
    pass.zones.obelisk_death(pass.map, players, mobs, &mob, None);
    pass.zones.boss_death(pass.map, Some(players), mobs, &mob, false);
}

impl super::Zones {
    /// An NPC in the player's zone (`CZone::FindNpc`), where it is now.
    pub fn npc(&self, zone: u16, npc_id: u32) -> Option<NpcEnter> {
        self.npcs.guard().get(&zone)?.npcs.get(&npc_id).map(|n| n.enter)
    }

    /// `0x0E04` (`CUser::PacketNpc`, section 2): an NPC on the player's map becomes its current NPC; the old one's user count goes
    /// down, the new one's up.
    pub fn use_npc(&self, map: u16, player: u32, npc_id: u32) {
        let mut maps = self.maps.guard();
        let Some(p) = maps.get_mut(&map).and_then(|players| players.get_mut(&player)) else { return };
        let mut npcs = self.npcs.guard();
        let Some(m) = npcs.get_mut(&map) else { return };
        if !m.npcs.contains_key(&npc_id) || p.npc == Some(npc_id) {
            return;
        }
        let_go(Some(m), p);
        m.npcs.get_mut(&npc_id).expect("listed").users += 1;
        p.npc = Some(npc_id);
    }

    /// The guards' pass (`CZone::UpdateNpcAttack`, every 500 ms after the monsters' attack pass).
    pub fn npc_attack(
        &self,
        now: Instant,
        world: &World,
        defs: &crate::mobs::MobDefs,
        loot: (&DropTables, &ItemDefs),
        rand: &mut impl FnMut() -> u32,
    ) {
        self.npc_pass(now, world, defs, loot, rand, false);
    }

    /// The NPCs' pass (`CZone::UpdateNpcIdle`, every 3 s): guards take their turn, the others walk.
    pub fn npc_idle(
        &self,
        now: Instant,
        world: &World,
        defs: &crate::mobs::MobDefs,
        loot: (&DropTables, &ItemDefs),
        rand: &mut impl FnMut() -> u32,
    ) {
        self.npc_pass(now, world, defs, loot, rand, true);
    }

    fn npc_pass(
        &self,
        now: Instant,
        world: &World,
        defs: &crate::mobs::MobDefs,
        loot: (&DropTables, &ItemDefs),
        rand: &mut impl FnMut() -> u32,
        idle: bool,
    ) {
        let mut maps = self.maps.guard();
        let mut all = self.mobs.guard();
        let mut npcs = self.npcs.guard();
        let mut nobody = HashMap::new();
        for (&map, m) in npcs.iter_mut() {
            let mut no_mobs = MobMap::default();
            let players = maps.get_mut(&map).unwrap_or(&mut nobody);
            let mobs = all.get_mut(&map).unwrap_or(&mut no_mobs);
            let mut pass = Pass { zones: self, map, defs, loot, now, rand: &mut *rand };
            let due: Vec<(u32, bool)> = m
                .npcs
                .values()
                .filter(|n| !n.stuck && n.disabled_until.is_none_or(|t| now >= t) && (idle || n.is_guard()))
                .map(|n| (n.enter.object_id, n.is_guard()))
                .collect();
            for (id, is_guard) in due {
                if is_guard {
                    guard_turn(&mut pass, world, m, mobs, players, id);
                } else {
                    walk_turn(&mut pass, world, m, players, id);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::time::Duration;

    use shaiya_proto::frame::{opcode, payload};
    use tokio::sync::mpsc::unbounded_channel;

    use super::super::tests::{drain, fighting_ground, no_loot, player};
    use super::super::{Occupant, Zones};
    use super::*;
    use crate::mobs::MobDefs;
    use crate::world::MapNpcRow;

    const WALKER: NpcDef = NpcDef { move_dist: 16, move_speed: 8000, country: 2 };
    const LIGHT_GUARD: NpcDef = NpcDef { move_dist: 16, move_speed: 8000, country: 0 };

    /// Map 1 with one NPC of `kind` (type id 1) at the first of `walk`.
    fn ground(kind: u8, def: NpcDef, walk: &[[f32; 3]]) -> World {
        let [x, y, z] = walk[0];
        let row = MapNpcRow { map: 1, kind: i16::from(kind), type_id: 1, first: Some([x, y, z, 0.0]), walk: walk.to_vec() };
        let mut world = World::build([(1, 2048)], &HashSet::from([(kind, 1)]), [row]);
        world.set_npc_defs([((kind, 1), def)].into());
        world
    }

    fn moves(out: &[(u16, u32)]) -> usize {
        out.iter().filter(|&&(op, id)| op == 0x0E03 && id == 1).count()
    }

    #[test]
    fn npcs_walk_back_and_forth_unless_used() {
        let world = ground(9, WALKER, &[[100.0, 0.0, 100.0], [140.0, 0.0, 100.0]]);
        let defs = MobDefs::default();
        let zones = Zones::new(&world, &defs);
        let (a, mut a_in) = player(7, 100.0, 120.0);
        zones.enter(1, a);
        drain(&mut a_in);
        let now = Instant::now();
        // The first turn puts it on position 0 (where it stands), then it waits 0 + 8 + 10 s.
        zones.npc_idle(now, &world, &defs, no_loot(), &mut || 0);
        assert_eq!(drain(&mut a_in), [(0x0E03, 1)]);
        zones.npc_idle(now + Duration::from_secs(17), &world, &defs, no_loot(), &mut || 0);
        assert!(drain(&mut a_in).is_empty(), "still waiting");
        // The guards' pass leaves it alone.
        zones.npc_attack(now + Duration::from_secs(18), &world, &defs, no_loot(), &mut || 0);
        assert!(drain(&mut a_in).is_empty());
        let t = now + Duration::from_secs(18);
        zones.npc_idle(t, &world, &defs, no_loot(), &mut || 0);
        let out = std::iter::from_fn(|| a_in.try_recv().ok()).collect::<Vec<_>>();
        assert_eq!(
            (opcode(&out[0]), payload(&out[0])[4], &payload(&out[0])[5..9]),
            (0x0E03, 0, &116.0f32.to_le_bytes()[..]),
            "16 on, walking"
        );
        assert_eq!(zones.npc(1, 1).map(|n| n.position), Some([116.0, 0.0, 100.0]));

        // A click holds it; moving 20 away lets it go.
        zones.use_npc(1, 7, 1);
        zones.npc_idle(t + Duration::from_secs(8), &world, &defs, no_loot(), &mut || 0);
        assert_eq!(moves(&drain(&mut a_in)), 0, "used: it stands");
        zones.move_to(1, 7, &super::super::tests::step(100.0, 135.0));
        zones.npc_idle(t + Duration::from_secs(8), &world, &defs, no_loot(), &mut || 0);
        assert_eq!(moves(&drain(&mut a_in)), 1, "let go: on again");
        // 132 → within 16 of 140: onto it, then back towards 100.
        zones.npc_idle(t + Duration::from_secs(16), &world, &defs, no_loot(), &mut || 0);
        assert_eq!(zones.npc(1, 1).map(|n| n.position), Some([140.0, 0.0, 100.0]));
        zones.npc_idle(t + Duration::from_secs(40), &world, &defs, no_loot(), &mut || 0);
        assert_eq!(zones.npc(1, 1).map(|n| n.position), Some([124.0, 0.0, 100.0]));
    }

    #[test]
    fn a_guard_runs_up_to_an_enemy_and_kills_it() {
        let world = ground(GUARD, LIGHT_GUARD, &[[100.0, 0.0, 100.0]]);
        let defs = MobDefs::default();
        let zones = Zones::new(&world, &defs);
        let (mut friend, mut friend_in) = player(6, 95.0, 100.0);
        friend.shape.country = 0;
        zones.enter(1, friend);
        let (mut enemy, mut enemy_in) = player(7, 110.0, 100.0);
        enemy.shape.country = 1;
        let (events, mut enemy_events) = unbounded_channel();
        zones.enter(1, Occupant { events, ..enemy });
        drain(&mut friend_in);
        drain(&mut enemy_in);
        let now = Instant::now();
        // 10 away: 0.3 short of the enemy's cell, then on it; 9/16 of 2 s.
        zones.npc_attack(now, &world, &defs, no_loot(), &mut || 0);
        assert_eq!(drain(&mut enemy_in), [(0x0E03, 1)]);
        zones.npc_attack(now + Duration::from_millis(1000), &world, &defs, no_loot(), &mut || 0);
        assert!(drain(&mut enemy_in).is_empty(), "not yet");
        let t = now + Duration::from_millis(1125);
        zones.npc_attack(t, &world, &defs, no_loot(), &mut || 0);
        // The death, then the blow (0x0E05 for a player) with the whole HP.
        let out = std::iter::from_fn(|| enemy_in.try_recv().ok()).collect::<Vec<_>>();
        assert_eq!(out.iter().map(|p| opcode(p)).collect::<Vec<_>>(), [0x0504, 0x0E05]);
        assert_eq!(payload(&out[0])[4..], [cp::KILLER_NPC, 1, 0, 0, 0], "killed by guard 1");
        assert_eq!(payload(&out[1]), [1, 1, 0, 0, 0, 7, 0, 0, 0, 100, 0]);
        // (0x0E05 starts with the 1 before the guard's id.)
        assert_eq!(drain(&mut friend_in), [(0x0E03, 1), (0x0504, 7), (0x0E05, 257)], "its own side is left alone");
        let died = std::iter::from_fn(|| enemy_events.try_recv().ok()).find(|e| matches!(e, ZoneEvent::Died { .. }));
        assert_eq!(
            died,
            Some(ZoneEvent::Died { killer_kind: cp::KILLER_NPC, killer: 1, loss: None, counted: false, keep_exp: false, drop_exp: 5 })
        );
        assert_eq!(zones.health(1, 7), Some([0; 3]));
        // Nobody else: back to its position, 10 away, in one step (10/16 of 2 s) after the 1 s rest.
        zones.npc_attack(t + Duration::from_millis(500), &world, &defs, no_loot(), &mut || 0);
        assert!(drain(&mut friend_in).is_empty(), "resting");
        zones.npc_attack(t + Duration::from_millis(1000), &world, &defs, no_loot(), &mut || 0);
        assert_eq!(drain(&mut friend_in), [(0x0E03, 1)]);
        assert_eq!(zones.npc(1, 1).map(|n| n.position), Some([100.0, 0.0, 100.0]));
    }

    #[test]
    fn a_walking_npc_steps_on_the_move_tick() {
        let world = ground(9, WALKER, &[[100.0, 0.0, 100.0], [140.0, 0.0, 100.0]]);
        let defs = MobDefs::default();
        let zones = Zones::new(&world, &defs);
        zones.set_smooth_chase(true);
        let (a, mut a_in) = player(7, 100.0, 120.0);
        zones.enter(1, a);
        drain(&mut a_in);
        let now = Instant::now();
        let at = |ms: u64| now + Duration::from_millis(ms);
        // On position 0, then 0 + 8 + 10 s; then a step of 16, which the client walks at 2 a second: 8 s, the next 100 ms before
        // (the original: the 3 s pass after 8 s, at 27 s).
        zones.npc_idle(now, &world, &defs, no_loot(), &mut || 0);
        zones.npc_idle(at(18_000), &world, &defs, no_loot(), &mut || 0);
        assert_eq!(drain(&mut a_in), [(0x0E03, 1), (0x0E03, 1)]);
        zones.move_steps(at(25_800), &world, &defs);
        assert!(drain(&mut a_in).is_empty(), "not yet");
        zones.move_steps(at(25_900), &world, &defs);
        assert_eq!(drain(&mut a_in), [(0x0E03, 1)]);
        assert_eq!(zones.npc(1, 1).map(|n| n.position), Some([132.0, 0.0, 100.0]));
        // 8 left: the arrival (its wait drawn) is the pass's.
        zones.move_steps(at(33_900), &world, &defs);
        assert!(drain(&mut a_in).is_empty());
    }

    #[test]
    fn a_guard_running_steps_on_the_move_tick() {
        let world = ground(GUARD, LIGHT_GUARD, &[[100.0, 0.0, 100.0]]);
        let defs = MobDefs::default();
        let zones = Zones::new(&world, &defs);
        zones.set_smooth_chase(true);
        let (mut enemy, mut enemy_in) = player(7, 118.0, 100.0);
        enemy.shape.country = 1;
        zones.enter(1, enemy);
        drain(&mut enemy_in);
        let now = Instant::now();
        // 18 away: a full step of 16, which the client runs at 16 / 8 × 4 = 8 a second: 2 s, the next 100 ms before.
        zones.npc_attack(now, &world, &defs, no_loot(), &mut || 0);
        assert_eq!(drain(&mut enemy_in), [(0x0E03, 1)]);
        let next = |zones: &Zones| zones.npcs.guard()[&1].npcs[&1].next_chase.unwrap();
        assert_eq!(next(&zones).duration_since(now).as_millis(), 1900);
        // The enemy steps back to 12 from the guard.
        zones.move_to(1, 7, &super::super::tests::step(128.0, 100.0));
        drain(&mut enemy_in);
        zones.move_steps(now + Duration::from_millis(1800), &world, &defs);
        assert!(drain(&mut enemy_in).is_empty(), "not yet");
        // The tick between the passes takes the last, short step; the blow is left to the pass.
        zones.move_steps(now + Duration::from_millis(1900), &world, &defs);
        assert_eq!(drain(&mut enemy_in), [(0x0E03, 1)]);
        zones.move_steps(now + Duration::from_millis(3500), &world, &defs);
        assert_eq!(zones.health(1, 7).map(|h| h[0] > 0), Some(true), "no blow on a move tick");
        zones.npc_attack(now + Duration::from_millis(3500), &world, &defs, no_loot(), &mut || 0);
        assert_eq!(zones.health(1, 7), Some([0; 3]), "the pass strikes");
    }

    #[test]
    fn a_guard_going_back_takes_an_enemy_it_meets() {
        let world = ground(GUARD, LIGHT_GUARD, &[[100.0, 0.0, 100.0]]);
        let defs = MobDefs::default();
        let zones = Zones::new(&world, &defs);
        let (mut first, mut first_in) = player(7, 110.0, 100.0);
        first.shape.country = 1;
        zones.enter(1, first);
        let now = Instant::now();
        zones.npc_attack(now, &world, &defs, no_loot(), &mut || 0);
        let t = now + Duration::from_millis(1125);
        zones.npc_attack(t, &world, &defs, no_loot(), &mut || 0);
        assert_eq!(zones.health(1, 7), Some([0; 3]), "the first one killed; nobody else: going back");
        drain(&mut first_in);
        // A second enemy turns up 5 beyond the guard, 15 from its post: on its way back the guard takes it (deviation 7).
        let (mut second, _second_in) = player(8, 115.0, 100.0);
        second.shape.country = 1;
        zones.enter(1, second);
        zones.npc_attack(t + Duration::from_millis(1000), &world, &defs, no_loot(), &mut || 0);
        let at = zones.npc(1, 1).map(|n| n.position).unwrap();
        assert!(at[0] > 110.0, "it ran at the second one instead of home: {at:?}");
        zones.npc_attack(t + Duration::from_millis(2000), &world, &defs, no_loot(), &mut || 0);
        assert_eq!(zones.health(1, 8), Some([0; 3]));
    }

    #[test]
    fn absorption_keeps_one_hp_for_one_more_blow() {
        let world = ground(GUARD, LIGHT_GUARD, &[[100.0, 0.0, 100.0]]);
        let defs = MobDefs::default();
        let zones = Zones::new(&world, &defs);
        let (mut enemy, mut enemy_in) = player(7, 101.0, 100.0);
        enemy.shape.country = 1;
        enemy.vitals.absorption = 12;
        zones.enter(1, enemy);
        drain(&mut enemy_in);
        let now = Instant::now();
        zones.npc_attack(now, &world, &defs, no_loot(), &mut || 0);
        // 0x0525 with the whole absorption, then the blow showing the 100 HP it had; 1 HP left.
        let ops = |inbox: &mut crate::outbox::OutboxInbox| std::iter::from_fn(|| inbox.try_recv().ok()).collect::<Vec<_>>();
        let out = ops(&mut enemy_in);
        assert_eq!(out.iter().map(|p| opcode(p)).collect::<Vec<_>>(), [0x0525, 0x0E05]);
        assert_eq!(payload(&out[0]), 12u16.to_le_bytes());
        assert_eq!(&payload(&out[1])[9..], 100u16.to_le_bytes());
        assert_eq!(zones.health(1, 7).map(|h| h[0]), Some(1));
        zones.npc_attack(now + Duration::from_millis(500), &world, &defs, no_loot(), &mut || 0);
        assert_eq!(ops(&mut enemy_in).iter().map(|p| opcode(p)).collect::<Vec<_>>(), [0x0504, 0x0E05]);
    }

    #[test]
    fn a_guard_kills_monsters_of_other_countries_for_nothing() {
        let (mob_world, _, defs) = fighting_ground();
        let mut world = ground(GUARD, LIGHT_GUARD, &[[100.0, 0.0, 110.0]]);
        world.set_mob_data([(1, vec![0u8; 2048 * 2048 / 8])].into(), mob_world.spawn_areas().clone());
        let zones = Zones::new(&world, &defs);
        let (a, mut a_in) = player(6, 100.0, 120.0);
        zones.enter(1, a);
        let now = Instant::now();
        zones.respawn(now, &world, &defs, &mut || 0);
        drain(&mut a_in);
        // The monster (country 2) 5 away at (100, 105): a step to 0.3 short, then the kill.
        zones.npc_attack(now, &world, &defs, no_loot(), &mut || 0);
        assert_eq!(drain(&mut a_in), [(0x0E03, 1)]);
        zones.npc_attack(now + Duration::from_millis(500), &world, &defs, no_loot(), &mut || 0);
        let out = std::iter::from_fn(|| a_in.try_recv().ok()).collect::<Vec<_>>();
        assert_eq!(out.iter().map(|p| opcode(p)).collect::<Vec<_>>(), [0x0606, 0x0E06]);
        assert_eq!(payload(&out[0])[4..], [cp::KILLER_NPC, 1, 0, 0, 0]);
        assert_eq!(&payload(&out[1])[9..], 1000u16.to_le_bytes());
        assert!(zones.mob(1, 1).is_some_and(|m| m.dead_until.is_some() && !m.fair));
    }
}
