//! Party instance zones in the zone (`docs/original/instances.md` sections 3 to 6 and 18): entering one, its portal countries, and
//! the one-second pass that releases empty ones with everything in them.

use super::*;
use crate::instance;
use crate::sync::Relock;
use crate::world::InstanceRules;

impl Zones {
    /// A `0x020A` to a party instance map (section 3): the refusal, in the original's order, or the party's zone, found or made now
    /// (with the map's monsters to spawn) and the player booked into it unless already there (deviation 2). `current` is the
    /// player's zone; `open` whether the map's open times allow it now.
    #[allow(clippy::too_many_arguments)]
    pub fn enter_instance(
        &self,
        map: u16,
        me: u32,
        current: u16,
        rules: &InstanceRules,
        open: bool,
        world: &World,
        defs: &MobDefs,
    ) -> Result<u16, u8> {
        let party = self.parties.guard().party_of(me).map(|p| (p.id, p.members.len()));
        let maps = self.maps();
        let mut instances = self.instances.guard();
        let existing = party.and_then(|(id, _)| instances.find(map, instance::Owner::Party(id)));
        let full = existing.is_some_and(|zone| {
            zone != current && maps.get(&zone).map_or(0, HashMap::len) + instances.booked_count(zone) >= rules.max_users as usize
        });
        if let Some(reason) = instance::entry_refusal(open, party.map(|(_, members)| members), full, rules.min_users) {
            return Err(reason);
        }
        let party = party.expect("checked").0;
        let zone = match existing {
            Some(zone) => zone,
            None => {
                let zone = instances.create(map, instance::Owner::Party(party)).ok_or(instance::REFUSED_FULL)?;
                self.mobs.guard().insert(zone, Self::new_mob_map(zone, world, defs));
                // Its own NPCs, with new object ids (`CZone::MapInit`).
                self.npcs.guard().insert(zone, super::npc::for_zone(world, zone, &self.last_npc_id));
                tracing::info!(zone, map, party, "instance zone made");
                zone
            }
        };
        if zone != current {
            instances.book(zone, me);
        }
        Ok(zone)
    }

    /// The guild's house zone of a house map (`docs/original/guild-house.md` section 7), found or made now, the player booked into
    /// it unless already there; the `0x020A` refusal when the player's guild may not go in.
    pub fn enter_guild_house(&self, map: u16, me: u32, current: u16, world: &World, defs: &MobDefs) -> Result<u16, u8> {
        let guild = {
            let guilds = self.guilds.guard();
            let (guild, _, house) = guilds.house_of(me).ok_or(crate::guild_house::portal::NO_GUILD)?;
            if let Some(reason) = house.portal_refusal() {
                return Err(reason);
            }
            guild
        };
        let mut instances = self.instances.guard();
        let owner = instance::Owner::Guild(guild);
        let zone = match instances.find(map, owner) {
            Some(zone) => zone,
            None => {
                // No room for another zone of the map: as the original when its zone pool is empty, no way in.
                let zone = instances.create(map, owner).ok_or(crate::guild_house::portal::NOT_OPEN)?;
                self.mobs.guard().insert(zone, Self::new_mob_map(zone, world, defs));
                self.npcs.guard().insert(zone, super::npc::for_zone(world, zone, &self.last_npc_id));
                tracing::info!(zone, map, guild, "guild house zone made");
                zone
            }
        };
        if zone != current {
            instances.book(zone, me);
        }
        Ok(zone)
    }

    /// The country an instance portal (id above 1000) has in a zone (section 5): the zone's pair, or nobody (3) when it has none or
    /// is not an instance zone.
    pub fn instance_portal_country(&self, zone: u16, id: u32) -> u32 {
        let mut instances = self.instances.guard();
        self.apply_portal_writes(&mut instances);
        instances.get(zone).map_or(instance::PORTAL_NOBODY, |i| i.portal_country(id))
    }

    /// `CZone::UpdateInsZonePortalCountry` (section 5): only in an instance zone. Nothing is sent. Called from inside the `mobs` lock
    /// (the Lua pass, the boss groups), so the change waits for the next taker of `instances`, which applies it first.
    pub fn set_instance_portal(&self, zone: u16, id: u32, country: u32) {
        self.instance_portal_writes.guard().push((zone, id, country));
    }

    /// The instance portal countries set since `instances` was last taken, in order.
    fn apply_portal_writes(&self, instances: &mut instance::Instances) {
        for (zone, id, country) in std::mem::take(&mut *self.instance_portal_writes.guard()) {
            if let Some(instance) = instances.get_mut(zone) {
                instance.set_portal(id, country);
            }
        }
    }

    /// The player is placed or has left the world: no longer on the way into any instance zone.
    pub fn unbook(&self, player: u32) {
        self.instances.guard().unbook(player);
    }

    /// The one-second pass of section 6: instance zones empty for their map's `ExpireTime` go, with their monsters, NPCs, items on
    /// the ground and fields. Returns how many went.
    pub fn expire_instances(&self, now: Instant, world: &World) -> usize {
        let mut maps = self.maps();
        let gone = {
            let occupied = |zone: u16| maps.get(&zone).is_some_and(|players| !players.is_empty());
            let mut instances = self.instances.guard();
            // Pending changes still belong to the zones they were made in, not to a later zone with the same key.
            self.apply_portal_writes(&mut instances);
            instances.expire(now, occupied, |zone| world.instance_rules(zone).expire)
        };
        if gone.is_empty() {
            return 0;
        }
        let mut mobs = self.mobs.guard();
        let mut npcs = self.npcs.guard();
        let mut ground = self.ground.guard();
        let mut fields = self.fields.guard();
        for zone in &gone {
            maps.remove(zone);
            mobs.remove(zone);
            npcs.remove(zone);
            ground.remove(zone);
            fields.remove(zone);
            tracing::info!(zone, map = crate::world::map_of(*zone), "instance zone released");
        }
        gone.len()
    }

    /// The instance zones now open, by kind: parties' and guilds' (`docs/original/operator-console.md` section 5, `zones`).
    pub fn instance_counts(&self) -> (usize, usize) {
        self.instances.guard().counts()
    }

    /// Each zone of a map, the static one and its instances, with its players and living monsters (`map`).
    pub fn map_report(&self, map: u16) -> Vec<(u16, usize, usize)> {
        let maps = self.maps();
        let mobs = self.mobs.guard();
        let mut zones: Vec<u16> = maps.keys().chain(mobs.keys()).copied().filter(|&z| crate::world::map_of(z) == map).collect();
        zones.sort_unstable();
        zones.dedup();
        zones
            .into_iter()
            .map(|z| {
                let players = maps.get(&z).map_or(0, HashMap::len);
                let monsters = mobs.get(&z).map_or(0, |m| m.mobs.values().filter(|mob| mob.dead_until.is_none()).count());
                (z, players, monsters)
            })
            .collect()
    }

    /// The instance zones there are now, for the log.
    pub fn instance_count(&self) -> usize {
        self.instances.guard().len()
    }
}
