//! The zones' side of the extensions' scripts (`docs/plans/ext-5-lua.md` sections 4.1 and 4.2): the set in force, the script thread's
//! queue, and the reads of the world the API makes. Nothing here runs a script: an event is queued with `try_send`, and the reads
//! take one lock and give a copy back, so the zone's code never waits for a script (section 8.3).

use std::sync::Arc;

use super::*;
use crate::script::event::{Job, Payload};
use crate::script::runner;
use crate::script::world::PlayerView;
use crate::script::{Event, ScriptSet};

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
            maps.iter().find_map(|(&map, players)| players.values().find(|p| p.occupant.shape.name.eq_ignore_ascii_case(name.as_bytes())).map(|p| view(map, p)))
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
