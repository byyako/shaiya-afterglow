//! The zones' side of the extensions' scripts (`docs/plans/ext-5-lua.md` sections 4.1 and 4.2): the set in force, the script thread's
//! queue, and the reads of the world the API makes. Nothing here runs a script: an event is queued with `try_send`, and the reads
//! take one lock and give a copy back, so the zone's code never waits for a script (section 8.3).

use std::sync::Arc;

use super::*;
use crate::mobs::{MobDefs, MobMap};
use crate::script::event::{Job, Killer, MobInfo, Payload, PlayerRef};
use crate::script::runner;
use crate::script::world::PlayerView;
use crate::script::{Event, ScriptSet};
use crate::world::World;

/// What a script thread needs of a player the zone holds.
fn view(map: u16, p: &Placed) -> PlayerView {
    let o = &p.occupant;
    PlayerView {
        id: o.id,
        name: String::from_utf8_lossy(&o.shape.name).trim_end_matches('\0').to_owned(),
        level: o.vitals.guard.level,
        country: o.shape.country,
        map,
        position: o.position,
        kills: o.shape.kills,
        job: o.shape.job,
        staff: if o.staff { 1 } else { 0 },
        guild: (o.guild_id != 0).then_some(o.guild_id),
    }
}

impl Zones {
    /// The scripts in force now: one snapshot per operation.
    pub fn ext_scripts(&self) -> Arc<ScriptSet> {
        self.scripts.get()
    }

    /// Puts `set` in force (the start and `reload`); the script thread looks at its timers again.
    pub fn set_ext_scripts(&self, set: ScriptSet) {
        self.scripts.put(set);
        self.script_job(Job::Wake);
    }

    /// Starts the script thread (once). Until then events are not queued.
    pub fn start_script_thread(&self) {
        self.script_jobs.get_or_init(|| runner::start(self.scripts.clone()));
    }

    /// Queues a job for the script thread; dropped (with a warning now and then) when the queue is full, and ignored before the
    /// thread started.
    pub fn script_job(&self, job: Job) {
        if let Some(jobs) = self.script_jobs.get() {
            jobs.send(job);
        }
    }

    /// The stored values of the scripts (`docs/plans/ext-5-lua.md` section 7.6).
    pub fn script_store(&self) -> Arc<crate::script::store::Store> {
        self.scripts.store.clone()
    }

    /// A character's stored values leave the cache once its `logout` handlers ran (queued behind them); at once when the queue
    /// cannot take the job.
    pub fn script_forget(&self, id: u32, session: u64) {
        let queued = self.script_jobs.get().is_some_and(|jobs| jobs.send(Job::Forget { id, session }));
        if !queued {
            self.scripts.store.end_char(id, session);
        }
    }

    /// Raises an event: queued when an enabled extension handles it, `make` building the payload only then.
    pub fn raise(&self, event: Event, make: impl FnOnce() -> Payload) {
        if self.script_jobs.get().is_none() || self.scripts.get().for_event(event).is_empty() {
            return;
        }
        self.script_job(Job::Event { event, payload: make() });
    }

    /// Leaves an order for a player and wakes its connection (the way a GM's order goes, `admin_order`). False when the player is not
    /// in a zone here (nothing is left).
    pub fn script_order(&self, target: u32, queued: crate::script::event::Queued) -> bool {
        let maps = self.maps.guard();
        let Some(p) = maps.values().find_map(|players| players.get(&target)) else { return false };
        let mut orders = self.script_orders.guard();
        orders.entry(target).or_default().push(queued);
        if p.occupant.events.send(ZoneEvent::Script).is_err() {
            orders.remove(&target);
            return false;
        }
        true
    }

    /// The orders left for a character, in order.
    pub fn take_script_orders(&self, id: u32) -> Vec<crate::script::event::Queued> {
        self.script_orders.guard().remove(&id).unwrap_or_default()
    }

    /// The staff level a player entered the world with (0 for a player): what `p:staff()` answers.
    pub fn set_script_staff(&self, id: u32, level: Option<u8>) {
        let mut staff = self.script_staff.guard();
        match level {
            Some(level) => staff.insert(id, level),
            None => staff.remove(&id),
        };
    }

    /// The online staff whose level, when they entered, is 1 to `level` (those who may type console commands).
    pub fn script_staff_upto(&self, level: u8) -> Vec<u32> {
        self.script_staff.guard().iter().filter(|(_, l)| (1..=level).contains(*l)).map(|(&id, _)| id).collect()
    }

    fn with_staff(&self, mut views: Vec<PlayerView>) -> Vec<PlayerView> {
        let staff = self.script_staff.guard();
        for v in &mut views {
            v.staff = staff.get(&v.id).copied().unwrap_or(0);
        }
        views
    }

    /// A player online here, by character id.
    pub fn script_view(&self, id: u32) -> Option<PlayerView> {
        let found = {
            let maps = self.maps.guard();
            maps.iter().find_map(|(&map, players)| players.get(&id).map(|p| view(map, p)))
        };
        found.and_then(|v| self.with_staff(vec![v]).pop())
    }

    /// A player online here, by name, ignoring case.
    pub fn script_find(&self, name: &str) -> Option<PlayerView> {
        let found = {
            let maps = self.maps.guard();
            maps.iter().find_map(|(&map, players)| {
                players.values().find(|p| p.occupant.shape.name.eq_ignore_ascii_case(name.as_bytes())).map(|p| view(map, p))
            })
        };
        found.and_then(|v| self.with_staff(vec![v]).pop())
    }

    /// The players online here, at most `max`, by character id.
    pub fn script_online(&self, max: usize) -> Vec<PlayerView> {
        let mut views: Vec<PlayerView> = {
            let maps = self.maps.guard();
            maps.iter().flat_map(|(&map, players)| players.values().map(move |p| view(map, p))).collect()
        };
        views.sort_by_key(|v| v.id);
        views.truncate(max);
        self.with_staff(views)
    }

    pub fn script_online_count(&self) -> usize {
        self.maps.guard().values().map(HashMap::len).sum()
    }

    /// The members of the player's party or union (itself included, `[id]` without one), whoever is in the world.
    pub fn script_party(&self, id: u32) -> Vec<u32> {
        let maps = self.maps.guard();
        if !maps.values().any(|players| players.contains_key(&id)) {
            return Vec::new();
        }
        match self.parties.guard().party_of(id) {
            Some(p) => p.members.iter().copied().filter(|m| maps.values().any(|players| players.contains_key(m))).collect(),
            None => vec![id],
        }
    }
}

/// Per extension, the monsters alive at most (`game.spawn`, section 7.4), and the most one call makes.
pub const SPAWNS_ALIVE: usize = 300;
pub const SPAWNS_PER_CALL: usize = 50;

/// A monster an extension spawned: who, until when, and whether it gives experience and drops.
#[derive(Clone, Debug)]
pub(super) struct ExtSpawn {
    pub extension: String,
    pub map: u16,
    pub until: Instant,
    pub exp: bool,
    pub drops: bool,
}

fn player_ref(p: &Placed) -> PlayerRef {
    PlayerRef { id: p.occupant.id, name: String::from_utf8_lossy(&p.occupant.shape.name).trim_end_matches('\0').to_owned() }
}

/// A player id as a reference: its name while it is in the zone; empty when it has left (the zone's lock is held, so the name cannot
/// be asked elsewhere).
fn named(players: &HashMap<u32, Placed>, id: u32) -> PlayerRef {
    players.get(&id).map_or_else(|| PlayerRef { id, name: String::new() }, player_ref)
}

impl Zones {
    /// A monster died with a player of most damage and a fair kill: `kill`, then `boss_kill` for a boss group's boss (section 5).
    /// Called with the zone's locks held; it only queues (section 4.3).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn raise_kill(&self, players: &HashMap<u32, Placed>, top: u32, mobs: &MobMap, uid: u32, level: u16, map: u16, near: &[u32]) {
        let Some(mob) = mobs.mobs.get(&uid).copied() else { return };
        let boss = matches!(mob.origin, crate::mobs::Origin::Boss(_));
        let make = |zones: &Zones| Payload::Kill {
            p: named(players, top),
            mob: MobInfo {
                id: mob.mob_id,
                uid,
                name: String::new(),
                level,
                map,
                x: mob.position[0],
                z: mob.position[2],
                boss,
                spawned: zones.ext_spawns.guard().get(&uid).map(|s| s.extension.clone()),
            },
            near: near.iter().map(|&m| named(players, m)).collect(),
        };
        self.raise(Event::Kill, || make(self));
        if boss {
            self.raise(Event::BossKill, || make(self));
        }
    }

    /// A player killed another for the faction (`ZoneEvent::PlayerKill` goes): `pvp_kill`.
    pub(super) fn raise_pvp_kill(&self, players: &HashMap<u32, Placed>, killer: u32, victim: u32, last_hit: u32, near: &[u32]) {
        self.raise(Event::PvpKill, || Payload::PvpKill {
            killer: named(players, killer),
            victim: named(players, victim),
            last_hit: (last_hit != 0).then(|| named(players, last_hit)),
            near: near.iter().map(|&m| named(players, m)).collect(),
        });
    }

    /// A player died: `death` with who killed it of record (`kind` is the combat packet's killer kind).
    pub(super) fn raise_death(&self, players: &HashMap<u32, Placed>, victim: u32, kind: u8, killer: u32, pvp: bool) {
        self.raise(Event::Death, || Payload::Death {
            victim: named(players, victim),
            killer: match kind {
                shaiya_proto::combat::KILLER_PLAYER => Killer::Player(named(players, killer)),
                shaiya_proto::combat::KILLER_NPC => Killer::Npc(killer),
                _ => Killer::Monster(killer),
            },
            pvp,
        });
    }

    /// What a spawned monster gives when it dies: `(experience, drops)`; both for any other monster.
    pub(super) fn spawn_gives(&self, uid: u32) -> (bool, bool) {
        self.ext_spawns.guard().get(&uid).map_or((true, true), |s| (s.exp, s.drops))
    }

    /// `game.spawn` (section 7.4): monsters like `LuaCreateMob`'s, never respawning, within `radius` of the spot, removed quietly at
    /// `lifetime`; at most [`SPAWNS_PER_CALL`] a call and [`SPAWNS_ALIVE`] alive for the extension. Their object ids.
    #[allow(clippy::too_many_arguments)]
    pub fn ext_spawn(
        &self,
        extension: &str,
        world: &World,
        defs: &MobDefs,
        mob_id: u16,
        count: usize,
        zone: u16,
        spot: [f32; 3],
        radius: u32,
        lifetime: std::time::Duration,
        gives: (bool, bool),
        now: Instant,
        rand: &mut impl FnMut() -> u32,
    ) -> Vec<u32> {
        let Some(def) = admin::usable_def(defs, mob_id) else { return Vec::new() };
        let room = SPAWNS_ALIVE.saturating_sub(self.ext_spawns.guard().values().filter(|s| s.extension == extension).count());
        let count = count.min(SPAWNS_PER_CALL).min(room);
        if count == 0 {
            return Vec::new();
        }
        let maps = self.maps.guard();
        let mut all = self.mobs.guard();
        let Some(players) = maps.get(&zone) else { return Vec::new() };
        let mobs = all.entry(zone).or_default();
        let mut ids = Vec::new();
        admin::generate_around(self, world, zone, mobs, players, &def, spot, radius, count as i64, now, rand, &mut ids);
        let mut spawns = self.ext_spawns.guard();
        for &id in &ids {
            spawns.insert(id, ExtSpawn { extension: extension.to_owned(), map: zone, until: now + lifetime, exp: gives.0, drops: gives.1 });
        }
        ids
    }

    /// The extension's monsters of `uids` that are still alive.
    pub fn ext_alive(&self, extension: &str, uids: &[u32]) -> usize {
        let spawns = self.ext_spawns.guard();
        let mine: Vec<(u32, u16)> =
            uids.iter().filter_map(|u| spawns.get(u).filter(|s| s.extension == extension).map(|s| (*u, s.map))).collect();
        drop(spawns);
        let all = self.mobs.guard();
        mine.iter().filter(|(u, map)| all.get(map).and_then(|m| m.mobs.get(u)).is_some_and(|m| m.dead_until.is_none())).count()
    }

    /// How many monsters the extension has alive.
    pub fn ext_spawn_count(&self, extension: &str) -> usize {
        self.ext_spawns.guard().values().filter(|s| s.extension == extension).count()
    }

    /// Takes the extension's monsters of `uids` away quietly (as `MobRemove2`: removed at the next body pass with `0x0602`, nothing
    /// gained). How many were its own and living.
    pub fn ext_despawn(&self, extension: &str, uids: &[u32], now: Instant) -> usize {
        let mine: Vec<(u32, u16)> = {
            let spawns = self.ext_spawns.guard();
            uids.iter().filter_map(|u| spawns.get(u).filter(|s| s.extension == extension).map(|s| (*u, s.map))).collect()
        };
        self.remove_spawns(&mine, now)
    }

    /// The pass that ends the spawns whose lifetime is over (the monster updater's second).
    pub fn expire_ext_spawns(&self, now: Instant) -> usize {
        let due: Vec<(u32, u16)> = self.ext_spawns.guard().iter().filter(|(_, s)| s.until <= now).map(|(&u, s)| (u, s.map)).collect();
        if due.is_empty() {
            return 0;
        }
        self.remove_spawns(&due, now)
    }

    fn remove_spawns(&self, list: &[(u32, u16)], now: Instant) -> usize {
        let maps = self.maps.guard();
        let mut all = self.mobs.guard();
        let mut removed = 0;
        for &(uid, zone) in list {
            let Some(mobs) = all.get_mut(&zone) else { continue };
            let Some(m) = mobs.mobs.get(&uid).copied() else { continue };
            if m.dead_until.is_some() {
                continue;
            }
            // A boss group's boss would be taken away as `MobRemove2` does; these are not bosses, but the call is the same one.
            self.boss_death(zone, maps.get(&zone), mobs, &m, true);
            if let Some(m) = mobs.mobs.get_mut(&uid) {
                m.dead_until = Some(now);
            }
            removed += 1;
        }
        removed
    }

    /// A body is gone: the spawn's record goes with it.
    pub(super) fn forget_ext_spawn(&self, uid: u32) {
        self.ext_spawns.guard().remove(&uid);
    }
}
