//! Pets in the zones (`docs/plans/ext-4-pets.md` sections 5 and 10): the follower a Pet Egg calls. A pet is **not** a monster of
//! `MobMap`: no monster pass sees it, nothing targets, counts or hurts it. To the client it is a monster record (`0x0601`, then
//! `0x0608` with HP 1), small and impossible to click by the record's own fields; the server only keeps where it is and walks it after
//! its owner.
//!
//! Lock order: `pets` is taken after `maps` and `mobs`, before `ground`; nothing is taken inside it.

use super::*;
use crate::backend::CRAFT_NAME_MAX;

/// Units a second the client gives a pet's walk and run (the look records' 1000 ms and 4 or 10 steps, `(Step + 0) / (Time × 0.001) − 0.3`;
/// section 5.3).
pub const PET_WALK: f32 = 3.7;
pub const PET_RUN: f32 = 9.7;
/// The pet stays within this of its owner, and walks to its follow spot beyond it.
const STAY_WITHIN: f32 = 4.0;
/// Beyond this it runs instead of walking.
const RUN_ABOVE: f32 = 8.0;
/// Beyond this it snaps: the client itself snaps a monster whose target is this far (client research entry 9).
const SNAP_ABOVE: f32 = 20.0;
/// The follow spot: this far from the owner on the line to the pet.
const SPOT_AT: f32 = 2.5;
/// A pet that stays looks again after this long.
const IDLE_STEP: Duration = Duration::from_millis(500);
/// How often the `pet` rules are asked for a pet that is out.
const CHECK_EVERY: Duration = Duration::from_secs(5);

/// What a pet fetches (`pet_egg` effect: `loot`, `loot_range`, `gold`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PetLoot {
    pub range: f32,
    pub gold: bool,
}

/// How a pet looks and what it does, from the egg's effect and the monster in the egg.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PetLook {
    /// The client's monster record the pet is drawn as (`base + wild`).
    pub record: u16,
    /// The monster in the egg.
    pub wild: u16,
    pub loot: Option<PetLoot>,
}

/// What a shown pet is doing (section 10.2).
#[derive(Clone, Copy, Debug, PartialEq)]
enum PetState {
    /// Following its owner, looking for a drop to fetch.
    Follow,
    /// Running to a drop; it arrives at `arrive`.
    Fetch { item: u32, arrive: Instant },
    /// Standing on the drop, waiting for its owner's connection to pick it up (at most 5 s).
    Waiting { item: u32, since: Instant },
}

/// How often a pet waiting on a drop looks whether it is picked up.
const WAIT_POLL: Duration = Duration::from_millis(200);
/// The real type of a quest item (`real_type` 27).
const QUEST_REAL_TYPE: u8 = 27;
/// How long a pet waits on a drop for the pick-up before it skips the item.
const WAIT_ON_DROP: Duration = Duration::from_secs(5);
/// A fetch is given up when the owner gets this much farther than `loot_range` from the pet.
const FETCH_SLACK: f32 = 10.0;

/// Why a pet that is out is not shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hide {
    /// Its owner is dead or an invisible staff member.
    Owner,
    /// A `deny pet` rule matches.
    Rule,
}

pub(super) struct Pet {
    id: u32,
    owner: u32,
    look: PetLook,
    /// The craft name of the egg it came out of: which egg's pet is out.
    egg: [u8; CRAFT_NAME_MAX],
    position: [f32; 3],
    /// `None` while hidden: in no cell, no packets.
    cell: Option<(i32, i32)>,
    next_step: Instant,
    next_check: Instant,
    hidden: Option<Hide>,
    state: PetState,
    /// Drops it gave up (no room, no answer): not tried again. Cleared when it hides, pruned of drops gone at each check.
    skipped: Vec<u32>,
}

/// The pets of one map.
#[derive(Default)]
pub(super) struct PetMap {
    pub(super) pets: HashMap<u32, Pet>,
    /// Owner to pet.
    owners: HashMap<u32, u32>,
    cells: HashMap<(i32, i32), Vec<u32>>,
}

impl PetMap {
    pub(super) fn is_empty(&self) -> bool {
        self.pets.is_empty()
    }

    fn put_in_cell(&mut self, id: u32, cell: Option<(i32, i32)>) {
        let Some(pet) = self.pets.get_mut(&id) else { return };
        if let Some(old) = pet.cell.take()
            && let Some(ids) = self.cells.get_mut(&old)
        {
            ids.retain(|&other| other != id);
            if ids.is_empty() {
                self.cells.remove(&old);
            }
        }
        if let Some(cell) = cell {
            pet.cell = Some(cell);
            self.cells.entry(cell).or_default().push(id);
        }
    }

    /// Changes a pet's own fields.
    fn set(&mut self, id: u32, change: impl FnOnce(&mut Pet)) {
        if let Some(pet) = self.pets.get_mut(&id) {
            change(pet);
        }
    }

    /// Takes a pet out of the map; the pet returned still holds the cell it was shown in (`None`: it was hidden).
    fn remove(&mut self, id: u32) -> Option<Pet> {
        let shown = self.pets.get(&id)?.cell;
        self.put_in_cell(id, None);
        let mut pet = self.pets.remove(&id)?;
        pet.cell = shown;
        self.owners.remove(&pet.owner);
        Some(pet)
    }
}

/// `0x0601` of a pet (`new` when it has just come out), then its `0x0608`: max HP 1 and HP 1, since a max HP of 0 divides by zero in the
/// client (entry 9).
fn pet_enter(pet: &Pet, new: bool) -> [Vec<u8>; 2] {
    [w::mob_enter(pet.id, new, pet.look.record, pet.position[0], pet.position[2]), w::mob_max_hp(pet.id, 1, 1)]
}

/// The pets in the cells of `view` that are not in the view of `except` (all of them without one): what `mobs_in_view` does for the
/// monsters.
fn pets_in_view(pets: Option<&PetMap>, view: (i32, i32), except: Option<(i32, i32)>) -> Vec<(u32, [Vec<u8>; 2])> {
    let Some(pets) = pets else { return Vec::new() };
    view_cells(view)
        .filter(|&cell| except.is_none_or(|other| !in_view(other, cell)))
        .filter_map(|cell| pets.cells.get(&cell))
        .flatten()
        .filter_map(|id| pets.pets.get(id))
        .map(|pet| (pet.id, pet_enter(pet, false)))
        .collect()
}

/// The point `SPOT_AT` from the owner on the line to the pet (so it trails behind and to the side it came from); a pet right on its
/// owner goes to the owner's east.
fn follow_spot(owner: [f32; 3], pet: [f32; 3]) -> [f32; 3] {
    let (dx, dz) = (pet[0] - owner[0], pet[2] - owner[2]);
    let length = (dx * dx + dz * dz).sqrt();
    let (ux, uz) = if length < 0.01 { (1.0, 0.0) } else { (dx / length, dz / length) };
    [owner[0] + ux * SPOT_AT, owner[1], owner[2] + uz * SPOT_AT]
}

/// A pet's cell after it moves to `to`: it keeps its cell until it is clear of the cell's margin, as a monster does.
fn cell_after(old: Option<(i32, i32)>, to: [f32; 3]) -> (i32, i32) {
    match old {
        Some(cell) if keeps_cell(cell, to) => cell,
        _ => cell_of(to),
    }
}

/// How long a walk or run of `length` takes.
fn travel(length: f32, speed: f32) -> Duration {
    Duration::from_secs_f32((length / speed).clamp(0.1, 5.0))
}

/// What the pets pass found out about an owner.
struct Owner {
    position: [f32; 3],
    /// Dead or an invisible staff member.
    away: bool,
    who: crate::rules::Who,
}

impl Zones {
    /// Calls the pet of `look` out beside its owner on `map` (the egg's use, a login, a map change): the one already out goes first. The
    /// view gets `0x0601` with `new = 1` and `0x0608`, the owner included. `false` when the owner is not in the zone. An owner who is
    /// dead or invisible has the pet kept hidden until it is alive and visible; so does a `deny pet` rule the caller has found to match
    /// (`ruled`: the arrival on a map; the caller has told the owner).
    pub fn summon_pet(&self, map: u16, owner: u32, look: PetLook, egg: [u8; CRAFT_NAME_MAX], now: Instant, ruled: bool) -> bool {
        let maps = self.maps.guard();
        let Some(players) = maps.get(&map) else { return false };
        let Some(me) = players.get(&owner) else { return false };
        let mut all = self.pets.guard();
        let pets = all.entry(map).or_default();
        if let Some(old) = pets.owners.get(&owner).copied() {
            Self::pet_gone(pets, players, old);
        }
        let id = self.last_mob_id.fetch_add(1, Ordering::Relaxed) + 1;
        let hidden = if me.dead || !me.occupant.visible {
            Some(Hide::Owner)
        } else if ruled {
            Some(Hide::Rule)
        } else {
            None
        };
        let position = follow_spot(me.occupant.position, me.occupant.position);
        pets.pets.insert(
            id,
            Pet {
                id,
                owner,
                look,
                egg,
                position,
                cell: None,
                next_step: now,
                next_check: now + CHECK_EVERY,
                hidden,
                state: PetState::Follow,
                skipped: Vec::new(),
            },
        );
        pets.owners.insert(owner, id);
        if hidden.is_none() {
            Self::pet_appears(pets, players, id, true);
        }
        tracing::debug!(owner, pet = id, record = look.record, wild = look.wild, map, "pet out");
        true
    }

    /// Sends the pet back (the egg used again, its egg gone, a safety net): `0x0602` to its view. `false` when none was out.
    pub fn dismiss_pet(&self, map: u16, owner: u32) -> bool {
        let maps = self.maps.guard();
        let mut all = self.pets.guard();
        let Some(pets) = all.get_mut(&map) else { return false };
        let Some(id) = pets.owners.get(&owner).copied() else { return false };
        match maps.get(&map) {
            Some(players) => Self::pet_gone(pets, players, id),
            None => {
                pets.remove(id);
            }
        }
        true
    }

    /// The craft name of the egg whose pet `owner` has out on `map`.
    pub fn pet_egg(&self, map: u16, owner: u32) -> Option<[u8; CRAFT_NAME_MAX]> {
        let all = self.pets.guard();
        let pets = all.get(&map)?;
        pets.pets.get(pets.owners.get(&owner)?).map(|pet| pet.egg)
    }

    /// Where a shown pet is, for `0x0304`/`0x0305`, which the client asks of a monster it has just seen.
    pub fn pet_position(&self, map: u16, id: u32) -> Option<[f32; 3]> {
        self.pets.guard().get(&map)?.pets.get(&id).filter(|pet| pet.cell.is_some()).map(|pet| pet.position)
    }

    /// Whether any map has a pet out (the pass returns at once without).
    fn any_pets(&self) -> bool {
        self.pets.guard().values().any(|pets| !pets.is_empty())
    }

    /// The owner leaves the zone: its pet goes, `0x0602` to its view (`Zones::leave`; the caller holds `maps` and has taken the owner out).
    pub(super) fn pet_left_with(&self, map: u16, players: &HashMap<u32, Placed>, owner: u32) {
        let mut all = self.pets.guard();
        let Some(pets) = all.get_mut(&map) else { return };
        if let Some(id) = pets.owners.get(&owner).copied() {
            Self::pet_gone(pets, players, id);
        }
    }

    /// The pets in the view of `cell` and not in that of `except`, as the packets a player entering or moving gets.
    pub(super) fn pets_in_view(&self, map: u16, cell: (i32, i32), except: Option<(i32, i32)>) -> Vec<(u32, [Vec<u8>; 2])> {
        pets_in_view(self.pets.guard().get(&map), cell, except)
    }

    /// The drop `item_id` as the pet of `owner` may take it (section 10.2): still on the ground, its monster-drop owner `owner`, within
    /// the pick-up range (10) of the pet, and a quest item only when `quest_ok` says so. `Err` carries the `0x0205` code as `pick_up`'s.
    pub fn pet_peek(&self, map: u16, owner: u32, item_id: u32, quest_ok: impl Fn(u8, u8) -> bool) -> Result<GroundItem, u8> {
        let pets = self.pets.guard();
        let ground = self.ground.guard();
        let pet = pets.get(&map).and_then(|m| m.owners.get(&owner).and_then(|id| m.pets.get(id))).ok_or(gp::PICK_GONE)?;
        let item = *ground.get(&map).and_then(|g| g.items.get(&item_id)).ok_or(gp::PICK_GONE)?;
        if !item.from_mob() || item.owner != owner {
            return Err(gp::PICK_NOT_YOURS);
        }
        if distance(pet.position, item.position) > PICK_RANGE {
            return Err(gp::PICK_TOO_FAR);
        }
        if let Loot::Item { item_type, type_id, .. } = item.loot
            && real_type(item_type) == QUEST_REAL_TYPE
            && !quest_ok(item_type, type_id)
        {
            return Err(gp::PICK_NO_QUEST);
        }
        Ok(item)
    }

    /// Takes the drop off the ground for the owner of a pet standing on it (`pick_up` for a pet): the checks of [`Zones::pet_peek`], then
    /// `0x0402` to the view, the owner's own returned.
    pub fn pet_pick_up(&self, map: u16, owner: u32, item_id: u32, quest_ok: impl Fn(u8, u8) -> bool) -> Result<(GroundItem, Vec<u8>), u8> {
        let item = self.pet_peek(map, owner, item_id, quest_ok)?;
        let maps = self.maps.guard();
        let mut ground = self.ground.guard();
        let ground = ground.get_mut(&map).ok_or(gp::PICK_GONE)?;
        ground.remove(item_id).ok_or(gp::PICK_GONE)?;
        let packet = gp::item_leave(item_id);
        for p in maps.get(&map).into_iter().flat_map(HashMap::values).filter(|p| p.occupant.id != owner && in_view(p.cell, item.cell)) {
            p.send(&packet);
        }
        Ok((item, packet))
    }

    /// The pet gives a drop up (no room for it): it stays on the ground, untouched, and is not tried again.
    pub fn pet_skip(&self, map: u16, owner: u32, item_id: u32) {
        let mut pets = self.pets.guard();
        if let Some(m) = pets.get_mut(&map)
            && let Some(id) = m.owners.get(&owner).copied()
        {
            m.set(id, |pet| {
                pet.skipped.push(item_id);
                pet.state = PetState::Follow;
            });
        }
    }

    /// A pet leaves: `0x0602` to the players in view of it, and it is forgotten.
    fn pet_gone(pets: &mut PetMap, players: &HashMap<u32, Placed>, id: u32) {
        let Some(pet) = pets.remove(id) else { return };
        if let Some(cell) = pet.cell {
            let packet = w::mob_leave(id);
            players.values().filter(|p| in_view(p.cell, cell)).for_each(|p| p.send(&packet));
        }
    }

    /// A pet appears at its position: its cell is taken and the players in view get `0x0601` and `0x0608`.
    fn pet_appears(pets: &mut PetMap, players: &HashMap<u32, Placed>, id: u32, new: bool) {
        let Some(position) = pets.pets.get(&id).map(|pet| pet.position) else { return };
        let cell = cell_of(position);
        pets.put_in_cell(id, Some(cell));
        let Some(pet) = pets.pets.get(&id) else { return };
        let packets = pet_enter(pet, new);
        for p in players.values().filter(|p| in_view(p.cell, cell)) {
            packets.iter().for_each(|packet| p.send(packet));
        }
    }

    /// A pet leaves the view but stays out: `0x0602`, in no cell.
    fn pet_hides(pets: &mut PetMap, players: &HashMap<u32, Placed>, id: u32, why: Hide) {
        let Some(pet) = pets.pets.get_mut(&id) else { return };
        pet.hidden = Some(why);
        pet.state = PetState::Follow;
        pet.skipped.clear();
        let Some(cell) = pet.cell else { return };
        pets.put_in_cell(id, None);
        let packet = w::mob_leave(id);
        players.values().filter(|p| in_view(p.cell, cell)).for_each(|p| p.send(&packet));
    }

    /// Moves a shown pet (`walk_mob` for a pet): a pet changing cells leaves and enters the views around, then everyone in the view of its
    /// new cell gets `0x0603` with the motion (0 walk, 1 run).
    fn pet_walks(pets: &mut PetMap, players: &HashMap<u32, Placed>, id: u32, to: [f32; 3], motion: u8) {
        let Some(pet) = pets.pets.get_mut(&id) else { return };
        let Some(old_cell) = pet.cell else { return };
        let old = pet_enter(pet, false);
        pet.position = to;
        let cell = cell_after(Some(old_cell), to);
        if cell != old_cell {
            pets.put_in_cell(id, Some(cell));
            for p in players.values() {
                match (in_view(p.cell, old_cell), in_view(p.cell, cell)) {
                    (true, false) => p.send(&w::mob_leave(id)),
                    (false, true) => old.iter().for_each(|packet| p.send(packet)),
                    _ => {}
                }
            }
        }
        let packet = w::mob_move(id, motion, to[0], to[2]);
        players.values().filter(|p| in_view(p.cell, cell)).for_each(|p| p.send(&packet));
    }

    /// The pets pass (section 5.3; `server/tasks.rs` runs it with the monsters'): for each pet whose time has come, hide or show it by its
    /// owner's state and the `pet` rules (asked every 5 s), then snap it to its owner beyond 20, walk or run it to its follow spot beyond
    /// 4, or let it stay. A pet whose owner is no longer on the map is removed. Returns at once when no map has a pet.
    ///
    /// A pet whose egg has `loot` fetches its owner's drops while it follows (section 10): the nearest drop of its owner's within its
    /// range of the owner, gold only with `gold`, a quest item only when `quest_ok(item type, type id, the owner's quests)` says the
    /// owner's quests take it. The owner must be alive and in no trade or personal shop.
    pub fn pets(&self, now: Instant, quest_ok: &dyn Fn(u8, u8, &[u16]) -> bool) {
        if !self.any_pets() {
            return;
        }
        let rules = self.rules();
        let maps = self.maps.guard();
        // Who cannot be served a drop now: in a trade (the trades lock comes after `maps`, so it is read before `pets`), a personal shop or
        // looking at one.
        let busy: std::collections::HashSet<u32> = {
            let trades = self.trades.guard();
            maps.values()
                .flatten()
                .filter(|(id, p)| p.shop.is_some() || p.visiting.is_some() || trades.busy(**id))
                .map(|(id, _)| *id)
                .collect()
        };
        let mut all = self.pets.guard();
        let ground = self.ground.guard();
        for (&map, pets) in all.iter_mut() {
            if pets.pets.is_empty() {
                continue;
            }
            let empty = HashMap::new();
            let players = maps.get(&map).unwrap_or(&empty);
            let due: Vec<u32> = pets.pets.values().filter(|pet| pet.next_step <= now).map(|pet| pet.id).collect();
            for id in due {
                let Some((owner_id, hidden, shown, position, next_check)) =
                    pets.pets.get(&id).map(|pet| (pet.owner, pet.hidden, pet.cell.is_some(), pet.position, pet.next_check))
                else {
                    continue;
                };
                let Some(me) = players.get(&owner_id) else {
                    // It cannot happen through the normal paths (`leave` takes the pet with its owner): a safety net.
                    Self::pet_gone(pets, players, id);
                    continue;
                };
                let owner = Owner { position: me.occupant.position, away: me.dead || !me.occupant.visible, who: pvp::who(me) };
                // The rules, every five seconds; the owner is told once, with the rule's message, when a rule starts to match.
                let mut ruled = hidden == Some(Hide::Rule);
                if next_check <= now {
                    let refusal = rules.denied(crate::rules::Point::Pet, map, &owner.who);
                    if let Some(refusal) = &refusal
                        && !ruled
                    {
                        let _ = me.occupant.events.send(ZoneEvent::PetRuled {
                            rule: refusal.rule,
                            name: refusal.name.clone(),
                            text: refusal.text.clone(),
                        });
                    }
                    ruled = refusal.is_some();
                    // The drops it gave up that are gone from the ground are forgotten.
                    let on_ground = |item: &u32| ground.get(&map).is_some_and(|g| g.items.contains_key(item));
                    pets.set(id, |pet| {
                        pet.next_check = now + CHECK_EVERY;
                        pet.skipped.retain(on_ground);
                    });
                }
                let why = if owner.away {
                    Some(Hide::Owner)
                } else if ruled {
                    Some(Hide::Rule)
                } else {
                    None
                };
                match why {
                    Some(why) => {
                        if shown {
                            Self::pet_hides(pets, players, id, why);
                        } else {
                            pets.set(id, |pet| pet.hidden = Some(why));
                        }
                        pets.set(id, |pet| pet.next_step = now + IDLE_STEP);
                        continue;
                    }
                    None if hidden.is_some() || !shown => {
                        // Back at the follow spot, as if it had just come out.
                        let spot = follow_spot(owner.position, position);
                        pets.set(id, |pet| {
                            pet.hidden = None;
                            pet.position = spot;
                            pet.next_step = now + IDLE_STEP;
                        });
                        Self::pet_appears(pets, players, id, true);
                        continue;
                    }
                    None => {}
                }
                // Fetching a drop (section 10.2).
                if let Some((state, loot)) = pets.pets.get(&id).map(|pet| (pet.state, pet.look.loot)) {
                    let items = ground.get(&map).map(|g| &g.items);
                    let mine = |item: u32| items.and_then(|items| items.get(&item)).filter(|i| i.from_mob() && i.owner == owner_id);
                    let range = loot.map_or(0.0, |l| l.range);
                    match state {
                        PetState::Fetch { item, arrive } => {
                            let valid = loot.is_some()
                                && !busy.contains(&owner_id)
                                && mine(item).is_some()
                                && distance(owner.position, position) <= range + FETCH_SLACK;
                            if !valid {
                                pets.set(id, |pet| pet.state = PetState::Follow);
                            } else if now >= arrive {
                                let _ = me.occupant.events.send(ZoneEvent::PetLoot { item });
                                pets.set(id, |pet| {
                                    pet.state = PetState::Waiting { item, since: now };
                                    pet.next_step = now + WAIT_POLL;
                                });
                                continue;
                            } else {
                                pets.set(id, |pet| pet.next_step = arrive);
                                continue;
                            }
                        }
                        PetState::Waiting { item, since } => {
                            if mine(item).is_none() {
                                pets.set(id, |pet| pet.state = PetState::Follow);
                            } else if now >= since + WAIT_ON_DROP {
                                // No answer: skipped, and the pet follows again.
                                pets.set(id, |pet| {
                                    pet.skipped.push(item);
                                    pet.state = PetState::Follow;
                                });
                            } else {
                                pets.set(id, |pet| pet.next_step = now + WAIT_POLL);
                                continue;
                            }
                        }
                        PetState::Follow => {}
                    }
                    if let (Some(loot), Some(items), PetState::Follow) =
                        (loot, items, pets.pets.get(&id).map_or(PetState::Follow, |pet| pet.state))
                        && !busy.contains(&owner_id)
                    {
                        let skipped = pets.pets.get(&id).map(|pet| pet.skipped.clone()).unwrap_or_default();
                        let wanted = items
                            .values()
                            .filter(|i| i.from_mob() && i.owner == owner_id && !skipped.contains(&i.id))
                            .filter(|i| distance(owner.position, i.position) <= loot.range)
                            .filter(|i| match i.loot {
                                Loot::Gold(_) => loot.gold,
                                Loot::Item { item_type, type_id, .. } => {
                                    real_type(item_type) != QUEST_REAL_TYPE
                                        || quest_ok(item_type, type_id, self.quests.guard().get(&owner_id).map_or(&[][..], Vec::as_slice))
                                }
                            })
                            .min_by(|a, b| distance(a.position, position).total_cmp(&distance(b.position, position)).then(a.id.cmp(&b.id)))
                            .copied();
                        if let Some(item) = wanted {
                            // Run to it; the item's spot is where the pet is, then.
                            let arrive = now + travel(distance(position, item.position), PET_RUN);
                            pets.set(id, |pet| {
                                pet.state = PetState::Fetch { item: item.id, arrive };
                                pet.next_step = arrive;
                            });
                            Self::pet_walks(pets, players, id, item.position, 1);
                            continue;
                        }
                    }
                }
                let position = pets.pets.get(&id).map_or(position, |pet| pet.position);
                let apart = distance(owner.position, position);
                let spot = follow_spot(owner.position, position);
                if apart > SNAP_ABOVE {
                    // `0x0602`, then `0x0601` again at the spot: a walk that far is not worth sending.
                    let old = pets.pets.get(&id).and_then(|pet| pet.cell);
                    pets.set(id, |pet| {
                        pet.position = spot;
                        pet.next_step = now + IDLE_STEP;
                    });
                    if let Some(old) = old {
                        let packet = w::mob_leave(id);
                        players.values().filter(|p| in_view(p.cell, old)).for_each(|p| p.send(&packet));
                    }
                    Self::pet_appears(pets, players, id, false);
                } else if apart > STAY_WITHIN {
                    let (motion, speed) = if apart > RUN_ABOVE { (1, PET_RUN) } else { (0, PET_WALK) };
                    pets.set(id, |pet| pet.next_step = now + travel(distance(position, spot), speed));
                    Self::pet_walks(pets, players, id, spot, motion);
                } else {
                    pets.set(id, |pet| pet.next_step = now + IDLE_STEP);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_follow_spot_is_on_the_line_to_the_pet() {
        assert_eq!(follow_spot([100.0, 5.0, 100.0], [110.0, 0.0, 100.0]), [102.5, 5.0, 100.0]);
        assert_eq!(follow_spot([100.0, 0.0, 100.0], [100.0, 0.0, 60.0]), [100.0, 0.0, 97.5]);
        let spot = follow_spot([0.0, 0.0, 0.0], [30.0, 0.0, 40.0]);
        assert!((spot[0] - 1.5).abs() < 1e-4 && (spot[2] - 2.0).abs() < 1e-4, "{spot:?}");
        assert_eq!(follow_spot([10.0, 0.0, 10.0], [10.0, 0.0, 10.0]), [12.5, 0.0, 10.0], "on the owner: to its east");
    }

    #[test]
    fn a_walk_takes_its_length_over_the_speed() {
        assert_eq!(travel(3.7, PET_WALK), Duration::from_secs(1));
        assert!(travel(0.0, PET_WALK) >= Duration::from_millis(100), "never a busy loop");
        assert_eq!(travel(1000.0, PET_RUN), Duration::from_secs(5), "never longer than 5 s");
    }
}
