//! The guild ranking battle in the zone (`docs/original/guild-ranking.md`): going into the guild's zone of the battle map, the
//! arrival that counts the entry, the close that moves everyone out, and the clock's state. The clock and the arithmetic are
//! `crate::ranking`'s; the guilds keep the points (`crate::guild`). Part of the zone: `ranking` is taken alone or last, held briefly.

use super::*;
use crate::guild::Placed as PlacedGuild;
use crate::instance;
use crate::ranking::{Battle, Step, Tag};
use crate::sync::Relock;

/// `0x020A` reasons for the ranking map (section 4).
pub mod portal {
    /// The player has no guild.
    pub const NO_GUILD: u8 = 0;
    /// The battle is not open.
    pub const CLOSED: u8 = 1;
    /// The player took part in this battle already.
    pub const ENTERED: u8 = 2;
    /// Deviation 2: the client asks first (its question 8043); its yes is `0x0227`.
    pub const ASK: u8 = 5;
}

impl Zones {
    /// The battle map's id, if the world has one.
    pub fn ranking_map(&self) -> Option<u16> {
        self.ranking_map
    }

    /// Whether a zone is of the battle map.
    pub fn is_ranking_zone(&self, zone: u16) -> bool {
        self.ranking_map == Some(crate::world::map_of(zone))
    }

    /// The tag of the battle that is open now.
    pub fn ranking_open(&self) -> Option<Tag> {
        self.ranking.guard().open()
    }

    /// Starts from an update left due (deviation 4).
    pub fn set_ranking_due(&self, due: Option<i64>) {
        *self.ranking.guard() = Battle::with_due(due);
    }

    /// One local minute of the clock (`crate::ranking::Battle::step`).
    pub fn ranking_step(&self, open: Option<Tag>, now: i64, delay: i64) -> Step {
        self.ranking.guard().step(open, now, delay)
    }

    /// Writes when the update is due, or that nothing waits any more.
    pub fn write_ranking_due(&self, due: Option<i64>) {
        self.guilds().write_ranking_due(due);
    }

    /// The ranking update (section 8): the guilds are scored, placed and written, and the players told.
    pub fn ranking_update(&self) -> Vec<PlacedGuild> {
        self.guilds().apply_ranking()
    }

    /// A `0x020A` or `0x0227` to the battle map (section 4, deviation 2), in the original's order: the refusal, or the zone to go
    /// to. A player in a zone of the map already moves within it (the map's own portals). Otherwise the guild's zone of the map,
    /// found or made now with the map's monsters, the player booked into it; `confirmed` is the client's yes (`0x0227`).
    pub fn enter_ranking(&self, map: u16, me: u32, current: u16, confirmed: bool, world: &World, defs: &MobDefs) -> Result<u16, u8> {
        let open = self.ranking_open();
        let guild = {
            let guilds = self.guilds();
            let guild = guilds.guild_of(me).map(|g| g.id).ok_or(portal::NO_GUILD)?;
            let tag = open.ok_or(portal::CLOSED)?;
            if self.is_ranking_zone(current) {
                return Ok(current);
            }
            if guilds.entered(me, tag) {
                return Err(portal::ENTERED);
            }
            guild
        };
        if !confirmed {
            return Err(portal::ASK);
        }
        let mut instances = self.instances.guard();
        let owner = instance::Owner::Guild(guild);
        let zone = match instances.find(map, owner) {
            Some(zone) => zone,
            None => {
                let zone = instances.create(map, owner).ok_or(portal::CLOSED)?;
                self.mobs.guard().insert(zone, Self::new_mob_map(zone, world, defs));
                self.npcs.guard().insert(zone, super::npc::for_zone(world, zone, &self.last_npc_id));
                tracing::info!(zone, map, guild, "ranking zone made");
                zone
            }
        };
        instances.book(zone, me);
        Ok(zone)
    }

    /// A player arrives in a zone of the battle map while the battle is open: its entry is counted, once per battle.
    pub(super) fn ranking_arrival(&self, zone: u16, player: u32) {
        if !self.is_ranking_zone(zone) {
            return;
        }
        if let Some(tag) = self.ranking_open() {
            self.guilds().count_entry(player, tag);
        }
    }

    /// The battle has closed (section 6, deviation 1): everyone in a zone of the map is called to the bind position of its country.
    /// A player with a logout pending stays (its connection decides). Returns how many were called.
    pub fn ranking_close(&self, world: &World, rand: &mut impl FnMut() -> u32) -> usize {
        let maps = self.maps.guard();
        let mut moved = 0;
        for (&zone, players) in maps.iter().filter(|&(&zone, _)| self.is_ranking_zone(zone)) {
            for p in players.values() {
                let (to, position) = world.bind_pos(zone, p.occupant.shape.country, p.occupant.position, rand);
                if p.occupant.events.send(ZoneEvent::Recalled { zone: to, position }).is_ok() {
                    moved += 1;
                }
            }
        }
        moved
    }

    /// Battle points for the guild of the player whose kill dropped a monster of the map (section 7), and `0x0D27` to the guild's
    /// members in this zone (the only R zone a guild has). Returns the guild's points for the log.
    pub(super) fn battle_points(&self, players: &HashMap<u32, Placed>, killer: u32, points: u16) -> Option<(u32, u32)> {
        let mut guilds = self.guilds();
        let guild = guilds.guild_of(killer)?.id;
        let score = guilds.add_battle_points(guild, u32::from(points))?;
        guilds.record_battle(killer, guild, u32::from(points), score.points);
        let packet = score.packet();
        for p in players.values().filter(|p| p.occupant.guild_id == guild) {
            p.send(&packet);
        }
        Some((guild, score.points))
    }
}
