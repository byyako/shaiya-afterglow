//! What operator (GM) commands touch in the zones (`docs/original/gm-commands.md`, `docs/plans/done/gm-commands.md` step 3): a staff member's
//! switches, the orders one connection leaves for another, the bans, and the monsters and NPCs the commands make and remove.

use crate::sync::Relock;
use std::collections::HashMap;
use std::time::Instant;

use shaiya_proto::world::{self as w};

use super::{CELL, Placed, ZoneEvent, Zones, cell_of, distance, in_view, mob_enter, reform};
use crate::admin::Until;
use crate::mobs::{self, MobDef, MobDefs, MobMap};
use crate::world::World;
use shaiya_proto::skill as sk;

/// A change one connection asks another to carry out (it is that connection's character that changes). The orders wait in
/// `Zones::admin_orders` (`ZoneEvent` is `Copy`), and `ZoneEvent::Admin` wakes the target.
#[derive(Clone, Debug, PartialEq)]
pub enum AdminOrder {
    /// `F701` `/set`: the stat number and value, for the GM `gm` to be answered.
    Set { gm: u32, stat: u8, value: u32 },
    /// `F601` / `F602`: the target's staff level (`op` is the opcode).
    StaffLevel { gm: u32, op: u16, level: u8 },
    /// `F703`: every item of that type and type id out of the target's bags.
    RecallItem { gm: u32, item_type: u8, type_id: u8 },
    /// `FA0D` to `FA10`: a chat or move ban starts (`until`), or ends when `None`; `op` is the opcode.
    Ban { gm: u32, op: u16, until: Option<Until> },
    /// `FA18`: move to this spot of that zone at `due`.
    Called { position: [f32; 3], zone: u16, due: Instant },
    /// `F801`, `F80D`, `F80E`: heal and cure (`op` is the opcode, for the answer and the whisper).
    Cure { gm: u32, op: u16, kind: CureKind },
    /// `F802`, `F803`: the bags or the equipment emptied (`gm_level` for the staff protection).
    Clear { gm: u32, gm_level: u8, op: u16 },
    /// `F804`, and `F709` for an account in the world: the connection ends (`op` is sent to the target first).
    Kick { gm: u32, gm_level: u8, op: u16 },
    /// `F80B`: the quest started for the target.
    QuestAdd { gm: u32, gm_level: u8, quest: u16 },
    /// `F80C`: the quest dropped, its start items taken.
    QuestRemove { gm: u32, gm_level: u8, quest: u16 },
    /// `F901`: the target's character sheet for the GM.
    Quiry { gm: u32 },
    /// `F904`, `F905`, `F90C`: move to this spot of that zone (`gm`: who is answered; none for a party member).
    Summon { gm: Option<u32>, op: u16, zone: u16, position: [f32; 3] },
}

/// What a `Cure` order does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CureKind {
    /// `F801`: cooldowns and debuffs end, HP, MP and SP fill.
    Debuffs,
    /// `F80D`: HP, MP and SP fill.
    Heal,
    /// `F80E`: cooldowns and every applied skill but those kept through death end, then the fill.
    All,
}

/// Who listens to whose chat (`FA0B`, `docs/original/gm-commands.md` section 8.4, the original's `+0x5780` and `+0x5784`): one GM per
/// player, one player per GM, both ways kept equal (deviations 21 and 23).
#[derive(Debug, Default)]
pub struct Listens {
    /// Player → the GM listening to it.
    listener: HashMap<u32, u32>,
    /// GM → the player it listens to.
    listened: HashMap<u32, u32>,
}

impl Listens {
    /// `gm` now listens to `player`: its earlier link goes, and so does another GM's link to `player`.
    pub fn link(&mut self, gm: u32, player: u32) {
        self.unlink(gm);
        if let Some(other) = self.listener.insert(player, gm) {
            self.listened.remove(&other);
        }
        self.listened.insert(gm, player);
    }

    /// `gm`'s link ends. False when it listened to nobody.
    pub fn unlink(&mut self, gm: u32) -> bool {
        let Some(player) = self.listened.remove(&gm) else { return false };
        self.listener.remove(&player);
        true
    }

    /// A character leaves the world: its links as a GM and as a player end (deviation 21).
    pub fn forget(&mut self, id: u32) {
        self.unlink(id);
        if let Some(gm) = self.listener.remove(&id) {
            self.listened.remove(&gm);
        }
    }

    pub fn listener_of(&self, player: u32) -> Option<u32> {
        self.listener.get(&player).copied()
    }
}

/// The two bans, by character (deviation 9).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bans {
    pub chat: Option<Until>,
    pub moves: Option<Until>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BanKind {
    Chat,
    Move,
}

/// The monsters `/mmake` and `/mera` handle at most.
pub const MOB_COUNT_MAX: u8 = 20;
/// How near to the GM `/mera` and `/nera` reach, in x and z.
const NEAR: f32 = 10.0;

/// A monster that may be created or named: with an id, HP and a level (`CZone::MobGen` checks the same).
pub fn usable_def(defs: &MobDefs, mob_id: u16) -> Option<MobDef> {
    defs.get(mob_id).filter(|d| d.id != 0 && d.hp != 0 && d.level != 0).copied()
}

/// `CZone::MobGen` / `MobGenEx`: up to `count * 10` tries, each placing one monster that never respawns at the spot ± 10, or on the spot
/// itself when that is off the map or blocked; a blocked spot counts as one monster done and one more wanted. The monsters are entered
/// into the view. Returns how many were made and the last one's id.
#[allow(clippy::too_many_arguments)]
pub(super) fn generate(
    zones: &Zones,
    world: &World,
    zone: u16,
    mobs: &mut MobMap,
    players: &HashMap<u32, Placed>,
    def: &MobDef,
    spot: [f32; 3],
    count: i64,
    now: Instant,
    rand: &mut impl FnMut() -> u32,
) -> (usize, u32) {
    generate_around(zones, world, zone, mobs, players, def, spot, 10, count, now, rand, &mut Vec::new())
}

/// [`generate`] with the radius round the spot (10 is the original's) and the object ids of the monsters made handed back.
#[allow(clippy::too_many_arguments)]
pub(super) fn generate_around(
    zones: &Zones,
    world: &World,
    zone: u16,
    mobs: &mut MobMap,
    players: &HashMap<u32, Placed>,
    def: &MobDef,
    spot: [f32; 3],
    radius: u32,
    count: i64,
    now: Instant,
    rand: &mut impl FnMut() -> u32,
    ids: &mut Vec<u32>,
) -> (usize, u32) {
    let free =
        |p: [f32; 3]| world.is_safe_pos(zone, p) && !world.is_blocked(zone, p[0].round_ties_even() as i32, p[2].round_ties_even() as i32);
    let (mut tries, mut done, mut limit, mut made, mut last) = (0i64, 0i64, count, 0, 0);
    while done < limit {
        tries += 1;
        if tries > count * 10 {
            break;
        }
        // A radius of 0 puts every monster on the spot itself.
        let (dx, dz) = if radius == 0 {
            (0.0, 0.0)
        } else {
            ((rand() % (2 * radius)) as f32 - radius as f32, (rand() % (2 * radius)) as f32 - radius as f32)
        };
        let mut at = [spot[0] + dx, spot[1], spot[2] + dz];
        if !free(at) {
            at = spot;
            if !free(at) {
                limit += 1;
                done += 1;
                continue;
            }
        }
        let new = zones.new_mob(def, at, usize::MAX, mobs::Origin::Area, now, rand);
        mobs.insert(new);
        ids.push(new.id);
        let packets = mob_enter(&new, true);
        for p in players.values().filter(|p| in_view(p.cell, new.cell)) {
            packets.iter().for_each(|packet| p.send(packet));
        }
        (made, last) = (made + 1, new.id);
        done += 1;
    }
    (made, last)
}

impl Zones {
    /// Leaves an order for a player on `map` and wakes its connection. `false` when it is not there (nothing is left).
    pub fn admin_order(&self, map: u16, target: u32, order: AdminOrder) -> bool {
        let maps = self.maps.guard();
        let Some(p) = maps.get(&map).and_then(|players| players.get(&target)) else { return false };
        let mut orders = self.admin_orders.guard();
        orders.entry(target).or_default().push(order);
        if p.occupant.events.send(ZoneEvent::Admin).is_err() {
            orders.remove(&target);
            return false;
        }
        true
    }

    /// The orders left for a character, in order.
    pub fn take_admin_orders(&self, id: u32) -> Vec<AdminOrder> {
        self.admin_orders.guard().remove(&id).unwrap_or_default()
    }

    /// A packet for one player on any map.
    pub fn send_to_char(&self, id: u32, packet: &[u8]) {
        let maps = self.maps.guard();
        if let Some(p) = maps.values().find_map(|players| players.get(&id)) {
            p.send(packet);
        }
    }

    /// Where a player on `map` stands.
    pub fn position_of(&self, map: u16, id: u32) -> Option<[f32; 3]> {
        self.maps.guard().get(&map)?.get(&id).map(|p| p.occupant.position)
    }

    /// The players of one zone as `/xcall` needs them: `(id, staff, country, position)`.
    pub fn players_of(&self, map: u16) -> Vec<(u32, bool, u8, [f32; 3])> {
        let maps = self.maps.guard();
        maps.get(&map)
            .map(|players| {
                players.values().map(|p| (p.occupant.id, p.occupant.staff, p.occupant.shape.country, p.occupant.position)).collect()
            })
            .unwrap_or_default()
    }

    /// A staff member's switches (`FA01`/`FA02` the visibility, `FA05`/`FA06` protection from monsters and guards with the players'
    /// attacks, `FA16`/`FA17` the players' attacks alone). A change of visibility sends the shape (13 while invisible) to the view; the
    /// returned packets are for the staff member itself.
    pub fn set_gm_flags(&self, map: u16, id: u32, visible: Option<bool>, attackable: Option<bool>, pvp: Option<bool>) -> Vec<Vec<u8>> {
        let mut maps = self.maps.guard();
        let Some(players) = maps.get_mut(&map) else { return Vec::new() };
        let Some(p) = players.get_mut(&id) else { return Vec::new() };
        let was = p.occupant.visible;
        p.occupant.visible = visible.unwrap_or(p.occupant.visible);
        p.occupant.attackable = attackable.unwrap_or(p.occupant.attackable);
        p.occupant.pvp = pvp.unwrap_or(p.occupant.pvp);
        if was == p.occupant.visible {
            return Vec::new();
        }
        let packet = p.shape_packet();
        let (cell, hidden) = (p.cell, !p.occupant.visible);
        for other in players.values().filter(|o| o.occupant.id != id && in_view(o.cell, cell)) {
            other.send(&packet);
            if hidden {
                tracing::debug!(staff = id, viewer = other.occupant.id, path = "char", "staff hidden from viewer");
            }
        }
        vec![packet]
    }

    /// A staff level change (`F601`) reaches the occupant: the staff packets (`send_staff`) go to it, and the view sees it as staff, only
    /// while it is one (`docs/server/audit.md` R26).
    pub fn set_staff(&self, map: u16, id: u32, staff: bool) {
        if let Some(p) = self.maps.guard().get_mut(&map).and_then(|players| players.get_mut(&id)) {
            p.occupant.staff = staff;
        }
    }

    /// A packet for every player who entered the world as staff, on any map (`CUserQuestionMgr::SendAdminAll`).
    pub fn send_staff(&self, packet: &[u8]) {
        let maps = self.maps.guard();
        for p in maps.values().flat_map(|players| players.values()).filter(|p| p.occupant.staff) {
            p.send(packet);
        }
    }

    /// A packet for every player of one zone (`CZone::SendAll`).
    pub fn send_zone(&self, map: u16, packet: &[u8]) {
        let maps = self.maps.guard();
        for p in maps.get(&map).into_iter().flat_map(|players| players.values()) {
            p.send(packet);
        }
    }

    /// `F801` / `F80E`: applied skills taken off a player (`0x050E` each, returned for the player). `all == false` takes the debuffs;
    /// `all == true` every skill but the ones kept through death (deviation 10 of part 2).
    pub fn clear_applied(&self, map: u16, id: u32, all: bool) -> Vec<Vec<u8>> {
        let mut maps = self.maps.guard();
        let Some(players) = maps.get_mut(&map) else { return Vec::new() };
        let Some(me) = players.get_mut(&id) else { return Vec::new() };
        let gone = if all { me.occupant.buffs.take_all_but_kept() } else { me.occupant.buffs.cure(&[], true) };
        if gone.is_empty() {
            return Vec::new();
        }
        let _ = me.occupant.events.send(ZoneEvent::Buffs);
        reform(players, id, false);
        gone.iter().map(|a| sk::applied_end(a.key)).collect()
    }

    /// A packet for every player on the server, both countries (`CWorld::SendAll`).
    pub fn send_world(&self, packet: &[u8]) {
        let maps = self.maps.guard();
        for p in maps.values().flat_map(|players| players.values()) {
            p.send(packet);
        }
    }

    /// The id of the party a character is in.
    pub fn party_id_of(&self, character: u32) -> Option<u32> {
        self.parties.guard().party_of(character).map(|p| p.id)
    }

    /// A live guild by name, ignoring case.
    pub fn guild_id_by_name(&self, name: &[u8]) -> Option<u32> {
        self.guilds().guild_id_by_name(name)
    }

    /// Whether the guild's house may be entered: a ranking below 31, bought and in use.
    pub fn guild_house_enabled(&self, guild: u32) -> bool {
        self.guilds().house(guild).is_some_and(|h| h.enabled())
    }

    /// `FA13`, `FA14`, `FA15` (see `Guilds`).
    pub fn admin_guild_grade(&self, guild: &[u8], member: &[u8], grade: u8) -> bool {
        self.guilds().admin_grade(guild, member, grade)
    }

    pub fn admin_guild_ranking(&self, guild: &[u8], ranking: u8) -> bool {
        self.guilds().admin_ranking(guild, ranking)
    }

    pub fn admin_guild_points(&self, guild: &[u8], points: u32) -> bool {
        self.guilds().admin_points(guild, points)
    }

    /// The zone of `owner` on `map` for a GM moving in (`FA12`, deviation 8): the existing one, or a new one made as the portals make it
    /// (with the map's monsters and NPCs), without their refusals and without booking: the move places the GM. `None` when there is no
    /// room for another zone of the map.
    pub fn gm_zone(&self, map: u16, owner: crate::instance::Owner, world: &World, defs: &MobDefs) -> Option<u16> {
        let mut instances = self.instances.guard();
        if let Some(zone) = instances.find(map, owner) {
            return Some(zone);
        }
        let zone = instances.create(map, owner)?;
        self.mobs.guard().insert(zone, Self::new_mob_map(zone, world, defs));
        self.npcs.guard().insert(zone, super::npc::for_zone(world, zone, &self.last_npc_id));
        tracing::info!(zone, map, ?owner, "zone made for a GM");
        Some(zone)
    }

    /// The bans a character has (none when it has none).
    pub fn bans_of(&self, id: u32) -> Bans {
        self.bans.guard().get(&id).copied().unwrap_or_default()
    }

    pub fn set_ban(&self, id: u32, kind: BanKind, until: Until) {
        let mut bans = self.bans.guard();
        let entry = bans.entry(id).or_default();
        match kind {
            BanKind::Chat => entry.chat = Some(until),
            BanKind::Move => entry.moves = Some(until),
        }
    }

    pub fn end_ban(&self, id: u32, kind: BanKind) {
        let mut bans = self.bans.guard();
        if let Some(entry) = bans.get_mut(&id) {
            match kind {
                BanKind::Chat => entry.chat = None,
                BanKind::Move => entry.moves = None,
            }
            if *entry == Bans::default() {
                bans.remove(&id);
            }
        }
    }

    /// `FA0B`: `gm` listens to `player` from now on.
    pub fn listen(&self, gm: u32, player: u32) {
        self.listens.guard().link(gm, player);
    }

    /// `FA0C`, a GM leaving the world, or a GM's level going above `admin::LISTEN_LEVEL_MAX`: its link ends. False when there was none.
    pub fn listen_off(&self, gm: u32) -> bool {
        self.listens.guard().unlink(gm)
    }

    /// A character leaves the world: whoever listened to it stops, and it stops listening.
    pub fn forget_listens(&self, id: u32) {
        self.listens.guard().forget(id);
    }

    /// `CUser::SendListenChat`: a copy of `player`'s line for the GM listening to it, wherever that GM is. Nothing when nobody listens.
    pub fn send_listener(&self, player: u32, packet: impl FnOnce() -> Vec<u8>) {
        let Some(gm) = self.listens.guard().listener_of(player) else { return };
        self.send_to_char(gm, &packet());
    }

    /// `CZone::MobGen` (`/mmake`): up to `count` (at most 20) monsters that never respawn around the spot. Returns how many were made;
    /// nothing for a monster without a usable definition.
    #[allow(clippy::too_many_arguments)]
    pub fn mob_gen(
        &self,
        zone: u16,
        mob_id: u16,
        spot: [f32; 3],
        count: u8,
        now: Instant,
        world: &World,
        defs: &MobDefs,
        rand: &mut impl FnMut() -> u32,
    ) -> usize {
        let Some(def) = usable_def(defs, mob_id) else { return 0 };
        let maps = self.maps.guard();
        let mut all = self.mobs.guard();
        let Some(players) = maps.get(&zone) else { return 0 };
        let mobs = all.entry(zone).or_default();
        generate(self, world, zone, mobs, players, &def, spot, i64::from(count.min(MOB_COUNT_MAX)), now, rand).0
    }

    /// `CZone::MobRemove` (`/mera`): up to `count` living monsters of that id in the 3 x 3 cells around the spot and within 10 of it die
    /// quietly at the next body pass (deviation 13); an obelisk falls to its own country, a boss's chain moves on
    /// (`docs/original/obelisks.md` sections 11.5 and 11.3). Returns how many.
    pub fn mob_remove_near(&self, zone: u16, mob_id: u16, at: [f32; 3], count: u8, now: Instant) -> usize {
        let maps = self.maps.guard();
        let mut all = self.mobs.guard();
        let Some(mobs) = all.get_mut(&zone) else { return 0 };
        let cell = cell_of(at);
        let mut gone = Vec::new();
        'cells: for cz in cell.1 - 1..=cell.1 + 1 {
            for cx in cell.0 - 1..=cell.0 + 1 {
                for &other in mobs.cells.get(&(cx, cz)).into_iter().flatten() {
                    if gone.len() >= usize::from(count) {
                        break 'cells;
                    }
                    let Some(m) = mobs.mobs.get(&other) else { continue };
                    if m.mob_id == mob_id && m.dead_until.is_none() && distance(m.position, at) <= NEAR {
                        gone.push(other);
                    }
                }
            }
        }
        for &id in &gone {
            self.remove_quietly(zone, maps.get(&zone), mobs, id, false, now);
        }
        gone.len()
    }

    /// `CZone::MobRemoveByID` (`/mera t`): that monster of the zone dies quietly; an obelisk falls to its base country, a boss's chain
    /// moves on. `false` when it is dead or not there.
    pub fn mob_remove_id(&self, zone: u16, object_id: u32, now: Instant) -> bool {
        let maps = self.maps.guard();
        let mut all = self.mobs.guard();
        let Some(mobs) = all.get_mut(&zone) else { return false };
        if mobs.mobs.get(&object_id).is_none_or(|m| m.dead_until.is_some()) {
            return false;
        }
        self.remove_quietly(zone, maps.get(&zone), mobs, object_id, true, now);
        true
    }

    /// An operator's removal of one monster: what it does to an obelisk (to its base with `to_base`) or a boss group, then
    /// `kill_quietly`.
    fn remove_quietly(&self, zone: u16, players: Option<&HashMap<u32, Placed>>, mobs: &mut MobMap, id: u32, to_base: bool, now: Instant) {
        if let Some(mob) = mobs.mobs.get(&id).copied() {
            self.obelisk_removed(zone, players, mobs, &mob, to_base);
            self.boss_death(zone, players, mobs, &mob, true);
        }
        kill_quietly(mobs, id, now);
    }

    /// `CZone::NpcCreate` (`/nmake`): an NPC of that kind enters the zone at the spot, 0.9 lower (deviation 11: this zone only).
    /// `false` for an unknown NPC or zone.
    pub fn npc_create(&self, zone: u16, kind: u8, type_id: u16, position: [f32; 3], world: &World) -> bool {
        if !world.has_npc(kind, type_id) {
            return false;
        }
        let maps = self.maps.guard();
        let mut all = self.npcs.guard();
        if !maps.contains_key(&zone) {
            return false;
        }
        let map = all.entry(zone).or_default();
        let object_id = self.last_npc_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        let enter = w::NpcEnter { object_id, kind, type_id, position: [position[0], position[1] - 0.9, position[2]], dir: 0 };
        let made = super::npc::place(world, zone, &[enter], |_| object_id);
        let Some(npc) = made.npcs.into_values().next() else { return false };
        let packet = w::npc_enter(&npc.enter);
        for p in maps.get(&zone).into_iter().flat_map(|players| players.values()).filter(|p| in_view(p.cell, npc.cell)) {
            p.send(&packet);
        }
        map.cells.entry(npc.cell).or_default().push(object_id);
        map.npcs.insert(object_id, npc);
        true
    }

    /// `CZone::NpcRemove` (`/nera`): up to `count` NPCs of that kind in the 3 x 3 cells around the spot and within 10 of it leave the
    /// zone (`0x0E02` to the view). Returns how many.
    pub fn npc_remove_near(&self, zone: u16, kind: u8, type_id: u16, at: [f32; 3], count: u8) -> usize {
        let mut maps = self.maps.guard();
        let mut all = self.npcs.guard();
        let Some(map) = all.get_mut(&zone) else { return 0 };
        let cell = cell_of(at);
        let mut gone = Vec::new();
        'cells: for cz in cell.1 - 1..=cell.1 + 1 {
            for cx in cell.0 - 1..=cell.0 + 1 {
                for &id in map.cells.get(&(cx, cz)).into_iter().flatten() {
                    if gone.len() >= usize::from(count) {
                        break 'cells;
                    }
                    let Some(n) = map.npcs.get(&id) else { continue };
                    if n.enter.kind == kind && n.enter.type_id == type_id && distance(n.enter.position, at) <= NEAR {
                        gone.push((id, n.cell));
                    }
                }
            }
        }
        for &(id, cell) in &gone {
            map.npcs.remove(&id);
            if let Some(list) = map.cells.get_mut(&cell) {
                list.retain(|&n| n != id);
            }
            let packet = w::npc_leave(id);
            for p in maps.get_mut(&zone).into_iter().flat_map(|players| players.values_mut()) {
                if p.npc == Some(id) {
                    p.npc = None;
                }
                if in_view(p.cell, cell) {
                    p.send(&packet);
                }
            }
        }
        gone.len()
    }
}

/// A monster dies at the next body pass without experience or drops (`dead_until = now`, as `LuaDeleteMob`); a Lua one gets its death
/// event.
fn kill_quietly(mobs: &mut MobMap, id: u32, now: Instant) {
    if let Some(m) = mobs.mobs.get_mut(&id) {
        m.dead_until = Some(now);
    }
    super::lua::lua_killed(mobs, id);
}

const _: () = assert!(CELL > NEAR);

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::time::Duration;

    use shaiya_proto::frame::{opcode, payload};

    use super::super::tests::{drain, fighting_ground, no_loot, player};
    use super::super::{Occupant, Zones};
    use super::*;
    use crate::mobs::{MobDefs, MobState, SpawnArea};
    use crate::world::NpcDef;
    use shaiya_proto::admin as ap;

    fn staff(id: u32, x: f32, z: f32) -> (Occupant, crate::outbox::OutboxInbox) {
        let (o, inbox) = player(id, x, z);
        (Occupant { staff: true, visible: false, attackable: false, pvp: false, ..o }, inbox)
    }

    fn counter() -> impl FnMut() -> u32 {
        let mut n = 0;
        move || {
            n += 7;
            n
        }
    }

    #[test]
    fn staff_are_no_target_for_monsters_until_switched_on() {
        let (mut world, def, _) = fighting_ground();
        world.set_mob_data(
            [(1, vec![0u8; 2048 * 2048 / 8])].into(),
            [(1, vec![SpawnArea::new([100.0, 0.0, 105.0, 100.0, 0.0, 105.0], vec![(7, 1)])])].into(),
        );
        let defs = MobDefs::new([MobDef { ai: 1, ..def }]);
        let zones = Zones::new(&world, &defs);
        let now = Instant::now();
        zones.respawn(now, &world, &defs, &mut || 0);
        let (a, _a_in) = staff(1, 100.0, 120.0);
        zones.enter(1, a);
        let state = || zones.mob(1, 1).map(|m| m.state);
        zones.idle(now + Duration::from_secs(5), &world, &defs, &mut || 0);
        assert_eq!(state(), Some(MobState::Idle), "invisible and untouchable");
        zones.set_gm_flags(1, 1, Some(true), None, None);
        zones.idle(now + Duration::from_secs(10), &world, &defs, &mut || 0);
        assert_eq!(state(), Some(MobState::Idle), "visible but still untouchable");
        zones.set_gm_flags(1, 1, None, Some(true), None);
        zones.idle(now + Duration::from_secs(15), &world, &defs, &mut || 0);
        assert_eq!(state(), Some(MobState::Fighting(1)), "both on");
    }

    #[test]
    fn a_guard_leaves_an_untouchable_enemy_alone() {
        use crate::world::MapNpcRow;
        let row = MapNpcRow {
            map: 1,
            kind: i16::from(crate::guard::GUARD),
            type_id: 1,
            first: Some([100.0, 0.0, 100.0, 0.0]),
            walk: vec![[100.0, 0.0, 100.0]],
        };
        let mut world = World::build([(1, 2048)], &HashSet::from([(crate::guard::GUARD, 1)]), [row]);
        world.set_npc_defs([((crate::guard::GUARD, 1), NpcDef { move_dist: 16, move_speed: 8000, country: 0 })].into());
        let defs = MobDefs::default();
        let zones = Zones::new(&world, &defs);
        let (mut enemy, mut enemy_in) = staff(7, 110.0, 100.0);
        enemy.shape.country = 1;
        zones.enter(1, enemy);
        drain(&mut enemy_in);
        let now = Instant::now();
        zones.npc_attack(now, &world, &defs, no_loot(), &mut || 0);
        assert!(drain(&mut enemy_in).is_empty(), "never picked");
        zones.set_gm_flags(1, 7, Some(true), Some(true), None);
        drain(&mut enemy_in);
        zones.npc_attack(now + Duration::from_secs(1), &world, &defs, no_loot(), &mut || 0);
        assert_eq!(drain(&mut enemy_in), [(0x0E03, 1)], "now the guard runs at it");
    }

    #[test]
    fn players_see_staff_arrive_with_kind_two_and_the_hidden_shape() {
        let zones = Zones::default();
        let (gm, mut gm_in) = staff(1, 100.0, 100.0);
        let for_gm = zones.enter(1, gm);
        // Its own 0x0201, 0x051C and, last, its shape (13).
        assert_eq!(for_gm.iter().map(|p| opcode(p)).collect::<Vec<_>>(), [0x0201, 0x051C, 0x0451, 0x051D]);
        assert_eq!(payload(&for_gm[3]), [1, 0, 0, 0, 13]);
        let (b, mut b_in) = player(2, 120.0, 100.0);
        let for_b = zones.enter(1, b);
        let seen: Vec<_> = for_b.iter().map(|p| opcode(p)).collect();
        assert_eq!(seen, [0x0201, 0x051C, 0x0201, 0x051C, 0x051D, 0x0451]);
        assert_eq!(payload(&for_b[2])[..5], [1, 0, 0, 0, 2], "the staff member comes in as kind 2");
        assert_eq!(payload(&for_b[4]), [1, 0, 0, 0, 13]);
        assert_eq!(drain(&mut gm_in), [(0x0201, 2), (0x051C, 2)]);
        drop(b_in.try_recv());

        // /char off: the view gets the real shape.
        drain(&mut b_in);
        let own = zones.set_gm_flags(1, 1, Some(true), None, None);
        assert_eq!(own.len(), 1);
        assert_eq!(payload(&own[0]), [1, 0, 0, 0, 0]);
        assert_eq!(drain(&mut b_in), [(0x051D, 1)]);
        assert!(drain(&mut gm_in).is_empty(), "the staff member has it from the return value");
        // Nothing changed: nothing sent.
        assert!(zones.set_gm_flags(1, 1, Some(true), None, None).is_empty());
        // And back to invisible.
        zones.set_gm_flags(1, 1, Some(false), None, None);
        assert_eq!(drain(&mut b_in), [(0x051D, 1)]);
    }

    #[test]
    fn players_attack_staff_only_when_it_is_visible_and_pvp_is_on() {
        let zones = Zones::default();
        let (mut a, _a_in) = player(1, 100.0, 100.0);
        a.shape.country = 0;
        zones.enter(1, a);
        let (mut gm, _gm_in) = staff(2, 105.0, 100.0);
        gm.shape.country = 1;
        zones.enter(1, gm);
        let may = || zones.may_attack_player(1, 1, 2, false);
        assert!(!may(), "invisible");
        zones.set_gm_flags(1, 2, Some(true), None, None);
        assert!(!may(), "visible, pvp still off");
        zones.set_gm_flags(1, 2, None, None, Some(true));
        assert!(may(), "/pvpon");
        zones.set_gm_flags(1, 2, None, None, Some(false));
        assert!(!may(), "/pvpoff");
        // /attack off sets both again.
        zones.set_gm_flags(1, 2, None, Some(true), Some(true));
        assert!(may());
    }

    #[test]
    fn orders_reach_the_target_once() {
        let zones = Zones::default();
        let (a, _a_in) = player(1, 100.0, 100.0);
        let (events, mut events_in) = tokio::sync::mpsc::unbounded_channel();
        zones.enter(1, Occupant { events, ..a });
        let order = AdminOrder::Set { gm: 9, stat: 2, value: 5 };
        assert!(!zones.admin_order(1, 2, order.clone()), "nobody 2");
        assert!(!zones.admin_order(2, 1, order.clone()), "not on that map");
        assert!(zones.admin_order(1, 1, order.clone()));
        assert!(zones.admin_order(1, 1, AdminOrder::StaffLevel { gm: 9, op: 0xF601, level: 3 }));
        assert_eq!((events_in.try_recv().ok(), events_in.try_recv().ok()), (Some(ZoneEvent::Admin), Some(ZoneEvent::Admin)));
        assert_eq!(zones.take_admin_orders(1), vec![order, AdminOrder::StaffLevel { gm: 9, op: 0xF601, level: 3 }]);
        assert!(zones.take_admin_orders(1).is_empty());
    }

    #[test]
    fn staff_notices_reach_staff_on_any_map_and_zone_notices_one_map() {
        let zones = Zones::default();
        let (gm, mut gm_in) = staff(1, 100.0, 100.0);
        let (other_gm, mut other_in) = staff(2, 100.0, 100.0);
        let (a, mut a_in) = player(3, 100.0, 100.0);
        let (b, mut b_in) = player(4, 100.0, 100.0);
        zones.enter(1, gm);
        zones.enter(1, a);
        zones.enter(2, other_gm);
        zones.enter(2, b);
        for inbox in [&mut gm_in, &mut other_in, &mut a_in, &mut b_in] {
            drain(inbox);
        }
        zones.send_staff(&w::position(9, 1, [0.0; 3]));
        assert_eq!((drain(&mut gm_in).len(), drain(&mut other_in).len()), (1, 1));
        assert!(drain(&mut a_in).is_empty() && drain(&mut b_in).is_empty(), "players do not get it");
        zones.send_zone(2, &w::position(9, 1, [0.0; 3]));
        assert_eq!((drain(&mut other_in).len(), drain(&mut b_in).len()), (1, 1));
        assert!(drain(&mut gm_in).is_empty() && drain(&mut a_in).is_empty(), "the other map");
    }

    #[test]
    fn clearing_all_keeps_what_survives_death() {
        use crate::buffs::ABILITIES;
        use crate::skills::{BUFF, DEBUFF, SkillDef};
        let def = |id: u16, type_effect: u8, keep_on_death: bool| SkillDef {
            id,
            level: 1,
            type_effect,
            kind: ABILITIES,
            keep_time: 3600,
            keep_on_death,
            ..Default::default()
        };
        let apply = |zones: &Zones, defs: &[SkillDef]| {
            let mut next = 0;
            let mut maps = zones.maps.guard();
            let me = maps.get_mut(&1).unwrap().get_mut(&1).unwrap();
            for d in defs {
                me.occupant.buffs.apply(d, 0, Instant::now(), &mut || {
                    next += 1;
                    next
                });
            }
        };
        let zones = Zones::default();
        let (a, mut a_in) = player(1, 100.0, 100.0);
        zones.enter(1, a);
        drain(&mut a_in);
        let all = [def(250, BUFF, true), def(251, BUFF, false), def(252, DEBUFF, false)];
        apply(&zones, &all);
        // /cure: the debuff alone.
        let gone = zones.clear_applied(1, 1, false);
        assert_eq!(gone.len(), 1);
        assert_eq!(opcode(&gone[0]), 0x050E);
        assert!(zones.clear_applied(1, 1, false).is_empty(), "nothing left to cure");
        // /event off: the plain buff goes, the item buff that survives death stays.
        apply(&zones, &all[2..]);
        assert_eq!(zones.clear_applied(1, 1, true).len(), 2, "the debuff and the plain buff");
        assert!(zones.clear_applied(1, 1, true).is_empty(), "the kept one stays");
        assert!(zones.clear_applied(2, 1, true).is_empty() && zones.clear_applied(1, 9, true).is_empty());
    }

    #[test]
    fn bans_are_kept_by_character_until_ended() {
        let zones = Zones::default();
        assert_eq!(zones.bans_of(1), Bans::default());
        zones.set_ban(1, BanKind::Chat, Until::Forever);
        zones.set_ban(1, BanKind::Move, Until::At(Instant::now()));
        assert_eq!(zones.bans_of(1).chat, Some(Until::Forever));
        zones.end_ban(1, BanKind::Chat);
        zones.end_ban(1, BanKind::Move);
        assert_eq!(zones.bans_of(1), Bans::default());
        assert!(zones.bans.guard().is_empty(), "emptied entries go");
    }

    #[test]
    fn listen_links_go_both_ways_and_end_with_either_side() {
        let mut l = Listens::default();
        l.link(9, 1);
        assert_eq!(l.listener_of(1), Some(9));
        // A new target drops the old one.
        l.link(9, 2);
        assert_eq!((l.listener_of(1), l.listener_of(2)), (None, Some(9)));
        // Another GM takes the player (deviation 23): the first one listens to nobody, so its `FA0C` fails.
        l.link(8, 2);
        assert_eq!(l.listener_of(2), Some(8));
        assert!(!l.unlink(9) && l.unlink(8) && !l.unlink(8));
        // Leaving the world ends the links on both sides (deviation 21).
        l.link(9, 1);
        l.forget(1);
        assert!(l.listener_of(1).is_none() && !l.unlink(9));
        l.link(9, 1);
        l.forget(9);
        assert!(l.listener_of(1).is_none() && l.listener.is_empty() && l.listened.is_empty());
    }

    #[test]
    fn a_copy_reaches_the_listening_gm_only() {
        let zones = Zones::default();
        let (gm, mut gm_inbox) = staff(9, 100.0, 100.0);
        let (talker, mut talker_inbox) = player(1, 100.0, 100.0);
        zones.enter(1, gm);
        zones.enter(1, talker);
        drain(&mut gm_inbox);
        drain(&mut talker_inbox);
        let copy = || ap::listen_chat(b"Talker", b"NORMAL", None, b"hi\0");
        zones.send_listener(1, copy);
        assert!(gm_inbox.try_recv().is_err(), "nobody listens yet");
        zones.listen(9, 1);
        zones.send_listener(1, copy);
        zones.send_listener(9, copy);
        assert_eq!(gm_inbox.try_recv().ok(), Some(copy()));
        assert!(gm_inbox.try_recv().is_err() && talker_inbox.try_recv().is_err(), "the GM's own lines are not copied to it");
        assert!(zones.listen_off(9) && !zones.listen_off(9));
    }

    #[test]
    fn gm_monsters_never_respawn_and_removals_are_quiet() {
        let (mut world, def, _) = fighting_ground();
        // Two spawn entries of monster 7, 30 apart.
        let area = |x| SpawnArea::new([x, 0.0, 105.0, x, 0.0, 105.0], vec![(7, 1)]);
        world.set_mob_data([(1, vec![0u8; 2048 * 2048 / 8])].into(), [(1, vec![area(100.0), area(130.0)])].into());
        let defs = MobDefs::new([def]);
        let zones = Zones::new(&world, &defs);
        let now = Instant::now();
        zones.respawn(now, &world, &defs, &mut || 0);
        let (gm, mut gm_in) = player(9, 100.0, 105.0);
        zones.enter(1, gm);
        drain(&mut gm_in);
        let before = zones.mob_count(1);
        let spot = [100.0, 0.0, 105.0];
        assert_eq!(zones.mob_gen(1, 99, spot, 3, now, &world, &defs, &mut counter()), 0, "unknown monster");
        assert_eq!(zones.mob_gen(2, 7, spot, 3, now, &world, &defs, &mut counter()), 0, "unknown zone");
        assert_eq!(zones.mob_gen(1, 7, spot, 30, now, &world, &defs, &mut counter()), 20, "at most 20");
        assert_eq!(zones.mob_count(1), before + 20);
        let made: Vec<_> = (1..=before as u32 + 20).filter_map(|id| zones.mob(1, id)).filter(|m| m.entry == usize::MAX).collect();
        assert_eq!(made.len(), 20);
        assert!(made.iter().all(|m| (m.position[0] - 100.0).abs() <= 10.0 && (m.position[2] - 105.0).abs() <= 10.0));
        assert_eq!(drain(&mut gm_in).iter().filter(|&&(op, _)| op == 0x0601).count(), 20);

        // /mera 7 3: three of the ones near, none of the one 30 away.
        assert_eq!(zones.mob_remove_near(1, 7, spot, 3, now), 3);
        let quiet = |id| zones.mob(1, id).is_some_and(|m| m.dead_until.is_some());
        let gone = (1..=before as u32 + 20).filter(|&id| quiet(id)).collect::<Vec<_>>();
        assert_eq!(gone.len(), 3);
        assert!(!gone.contains(&2), "monster 2 stands 30 away at (130, 105)");
        // Within 10 is a circle: the box's corners may lie outside it.
        let near = (1..=before as u32 + 20)
            .filter_map(|id| zones.mob(1, id))
            .filter(|m| m.dead_until.is_none() && distance(m.position, spot) <= 10.0)
            .count();
        assert!(near > 0);
        assert_eq!(zones.mob_remove_near(1, 7, spot, 255, now), near, "the rest near the spot; the entry 30 away stays");
        assert!(!quiet(2));

        // /mera t.
        assert!(zones.mob_remove_id(1, 2, now));
        assert!(!zones.mob_remove_id(1, 2, now), "already dead");
        assert!(!zones.mob_remove_id(1, 9999, now));
    }

    #[test]
    fn gm_npcs_come_and_go_with_their_packets() {
        let mut world = World::build([(1, 2048)], &HashSet::new(), []);
        world.set_npc_defs([((6, 27), NpcDef::default())].into());
        let zones = Zones::new(&world, &MobDefs::default());
        let (a, mut a_in) = player(1, 100.0, 100.0);
        zones.enter(1, a);
        drain(&mut a_in);
        assert!(!zones.npc_create(1, 6, 28, [100.0, 12.5, 100.0], &world), "unknown NPC");
        assert!(!zones.npc_create(2, 6, 27, [100.0, 12.5, 100.0], &world), "unknown zone");
        assert!(zones.npc_create(1, 6, 27, [100.0, 12.5, 100.0], &world));
        let out = drain(&mut a_in);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0, 0x0E01);
        let id = out[0].1;
        let n = zones.npc(1, id).unwrap();
        assert_eq!((n.kind, n.type_id, n.position), (6, 27, [100.0, 11.6, 100.0]));
        assert_eq!(zones.npc_remove_near(1, 6, 27, [150.0, 0.0, 150.0], 1), 0, "too far");
        assert_eq!(zones.npc_remove_near(1, 6, 26, [100.0, 0.0, 100.0], 1), 0, "another kind");
        assert_eq!(zones.npc_remove_near(1, 6, 27, [100.0, 0.0, 100.0], 1), 1);
        assert_eq!(drain(&mut a_in), [(0x0E02, id)]);
        assert!(zones.npc(1, id).is_none());
        assert_eq!(zones.npc_remove_near(1, 6, 27, [100.0, 0.0, 100.0], 1), 0);
    }
}
