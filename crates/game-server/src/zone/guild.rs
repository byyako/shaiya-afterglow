//! Guilds in the zone (`docs/original/guild.md`): what needs the maps and the parties (the requester's party on its map for creating
//! and disbanding, the target on the inviter's map), and the looks after a change. Part of the zone: `guilds` is taken after `maps`
//! and `parties`, held briefly. The bookkeeping and the guild's packets are `crate::guild`'s.

use super::*;
use crate::backend::GuildCreated;
use crate::guild::{Aftermath, Burst, Guilds, Player};
use crate::sync::Relock;
use tokio::sync::oneshot;

type Maps = HashMap<u16, HashMap<u32, Placed>>;

/// A player's guild changed: it leaves its guild war first while still a member (`docs/original/guild-war.md` section 6.5); its
/// looks carry the new id (`0x0201`) and name (`0x0303`), and its view gets the looks again (deviation 8).
fn apply(zones: &Zones, maps: &mut Maps, after: Aftermath) {
    let changed: Vec<u32> = after
        .looks
        .iter()
        .filter(|(id, guild, _)| maps.values().any(|players| players.get(id).is_some_and(|p| p.occupant.guild_id != *guild)))
        .map(|&(id, _, _)| id)
        .collect();
    zones.war_guild_left(maps, &changed);
    for (id, guild, name) in after.looks {
        let Some(players) = maps.values_mut().find(|players| players.contains_key(&id)) else { continue };
        let p = players.get_mut(&id).expect("found");
        p.occupant.guild_id = guild;
        p.occupant.shape.guild_name = name;
        let p = &players[&id];
        let looks = p.looks_packet();
        for o in players.values().filter(|o| in_view(o.cell, p.cell)) {
            o.send(&looks);
        }
    }
}

impl Zones {
    pub(super) fn guilds(&self) -> std::sync::MutexGuard<'_, Guilds> {
        self.guilds.guard()
    }

    /// The guilds loaded at start.
    /// Tests only (`docs/server/audit.md` R5): panics with the players' lock held, which poisons it.
    #[doc(hidden)]
    pub fn panic_holding_locks(&self) {
        let _maps = self.maps();
        panic!("a test panic inside the zone lock");
    }

    pub fn load_guilds(&self, guilds: Guilds) {
        *self.guilds() = guilds;
    }

    /// Asks the guild and friend writers to answer once what is queued now is written (the shutdown waits for them; audit R16).
    pub fn flush_writers(&self) -> Vec<tokio::sync::oneshot::Receiver<()>> {
        let guilds = self.guilds().flush();
        let friends = self.friends().flush();
        guilds.into_iter().chain(friends).collect()
    }

    pub fn guild_count(&self) -> usize {
        self.guilds().len()
    }

    /// The guild id and name a player entering a map shows.
    pub(super) fn guild_looks(&self, id: u32) -> (u32, Vec<u8>) {
        self.guilds().looks(id)
    }

    /// The guild packets of the enter-world burst and the guild name of `0x0105` (section 10).
    pub fn guild_burst(&self, id: u32, country: u8) -> Burst {
        self.guilds().burst(id, country)
    }

    /// Entering the world, right after the burst: the guilds know the player from now on.
    pub fn guild_connect(&self, id: u32, player: Player) {
        self.guilds().connect(id, player);
    }

    /// Leaving the world (`CUser::ReleaseGuild`).
    /// A character renamed at the character screen: its guild lists show the new name.
    pub fn guild_renamed(&self, id: u32, name: &[u8]) {
        self.guilds().renamed(id, name);
    }

    pub fn guild_leave_world(&self, id: u32) {
        self.guilds().disconnect(id);
    }

    pub fn guild_level(&self, id: u32, level: u16) {
        self.guilds().level_up(id, level);
    }

    /// Guild chat: false without a guild.
    pub fn guild_chat(&self, id: u32, packet: &[u8]) -> bool {
        self.guilds().chat(id, packet)
    }

    pub fn guild_etin(&self, id: u32) {
        self.guilds().etin(id);
    }

    /// The player's guild: its id, country and house (`docs/original/guild-house.md`).
    pub fn guild_house_of(&self, id: u32) -> Option<(u32, u8, crate::guild_house::House)> {
        self.guilds().house_of(id)
    }

    /// `0x0D25` (section 4).
    pub fn guild_npc_upgrade(&self, id: u32, key: u16, level: u8, rules: &crate::guild_house::HouseRules) {
        self.guilds().upgrade(id, key, level, rules);
    }

    /// `0x0D34`'s checks (section 5): the guild, or the refusal.
    pub fn guild_house_purchase(&self, id: u32, money: u32, rules: &crate::guild_house::HouseRules) -> Result<u32, u8> {
        self.guilds().house_purchase(id, money, rules)
    }

    /// The house paid for and written (`GuildStore::buy_guild_house`).
    pub fn guild_house_bought(&self, guild: u32, rules: &crate::guild_house::HouseRules) {
        self.guilds().house_bought(guild, rules);
    }

    /// Etin returned by `id` to `guild` and written (`ItemStore::guild_return_etin`; section 3).
    pub fn guild_return_etin(&self, guild: u32, id: u32, sum: u32) {
        self.guilds().return_etin(guild, id, sum);
    }

    /// A guild's name and the item in one of its bank's slots, as text and a unit (the game log's guild bank records).
    pub fn guild_bank_view(&self, guild: u32, slot: u8) -> Option<(String, Option<crate::backend::ItemUnit>)> {
        let guilds = self.guilds();
        let g = guilds.by_id(guild)?;
        Some((String::from_utf8_lossy(&g.name).trim_end_matches('\0').to_owned(), g.bank.get(slot).cloned()))
    }

    /// The guild bank's turn (`docs/original/guild-bank.md` section 14): held from the plan to the apply of a move with bag 255.
    pub async fn guild_bank_turn(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.guild_bank_turn.lock().await
    }

    /// A move with bag 255 on one side, planned (`Guilds::bank_plan`).
    #[allow(clippy::too_many_arguments)]
    pub fn guild_bank_plan(
        &self,
        id: u32,
        inventory: &crate::inventory::Inventory,
        defs: &crate::inventory::ItemDefs,
        rules: &crate::guild_house::HouseRules,
        money: u32,
        from: (u8, u8),
        to: (u8, u8),
    ) -> Option<(u32, Result<crate::guild_bank::Planned, crate::guild_bank::Refusal>)> {
        self.guilds().bank_plan(id, inventory, defs, rules, money, from, to)
    }

    /// The planned move made once the database has it; the mover's answer (`Guilds::bank_apply`).
    #[allow(clippy::too_many_arguments)]
    pub fn guild_bank_apply(
        &self,
        guild: u32,
        id: u32,
        inventory: &mut crate::inventory::Inventory,
        planned: &crate::guild_bank::Planned,
        from: (u8, u8),
        to: (u8, u8),
        money_after: u32,
    ) -> Option<Vec<u8>> {
        self.guilds().bank_apply(guild, id, inventory, planned, from, to, money_after)
    }

    /// `0x0D21` (section 4): the requester's party, each member with whether it is on the requester's map.
    pub fn guild_create(&self, map: u16, me: u32, name: &[u8], remark: &[u8], party_size: usize, now: Instant) {
        let maps = self.maps();
        let parties = self.parties.guard();
        let here = |m: u32| maps.get(&map).is_some_and(|players| players.contains_key(&m));
        let party: Vec<(u32, bool)> = parties.party_of(me).map(|p| p.members.iter().map(|&m| (m, here(m))).collect()).unwrap_or_default();
        self.guilds().create_request(me, &party, party_size, name, remark, now);
    }

    pub fn guild_create_answer(&self, me: u32, yes: bool) {
        self.guilds().create_answer(me, yes);
    }

    /// After `ZoneEvent::GuildAgreed`, with the money: the creation goes to the database.
    pub fn guild_create_write(&self, me: u32, server_id: u8) -> Option<oneshot::Receiver<anyhow::Result<GuildCreated>>> {
        self.guilds().create_write(me, server_id)
    }

    pub fn guild_create_failed(&self, me: u32, code: u8) {
        self.guilds().create_failed(me, code);
    }

    pub fn guild_create_done(&self, me: u32, id: u32, money: u32, now: Instant) {
        let mut maps = self.maps();
        let after = self.guilds().create_done(me, id, money, now);
        apply(self, &mut maps, after);
    }

    /// `0x0D05` (section 5): the target looked up on the inviter's map.
    pub fn guild_invite(&self, map: u16, me: u32, target: u32, now: Instant) {
        let maps = self.maps();
        let here = maps.get(&map).is_some_and(|players| players.contains_key(&target));
        self.guilds().invite(me, target, here, now);
    }

    /// `0x0D06`: the inviter looked up on the answerer's map.
    pub fn guild_invite_answer(&self, map: u16, me: u32, yes: bool, now: Instant) {
        let mut maps = self.maps();
        let after = {
            let here = |inviter: u32| maps.get(&map).is_some_and(|players| players.contains_key(&inviter));
            self.guilds().invite_answer(me, yes, here, now)
        };
        apply(self, &mut maps, after);
    }

    pub fn guild_join_request(&self, me: u32, guild: u32, now: Instant) {
        self.guilds().join_request(me, guild, now);
    }

    pub fn guild_join_answer(&self, me: u32, accept: bool, target: u32, now: Instant) {
        let mut maps = self.maps();
        let after = self.guilds().join_answer(me, accept, target, now);
        apply(self, &mut maps, after);
    }

    pub fn guild_leave(&self, me: u32) {
        let mut maps = self.maps();
        let after = self.guilds().leave(me);
        apply(self, &mut maps, after);
    }

    pub fn guild_kick(&self, me: u32, target: u32) {
        let mut maps = self.maps();
        let after = self.guilds().kick(me, target);
        apply(self, &mut maps, after);
    }

    pub fn guild_rank(&self, me: u32, down: u8, target: u32) {
        self.guilds().change_rank(me, down, target);
    }

    pub fn guild_remark(&self, me: u32, remark: &[u8]) {
        self.guilds().set_remark(me, remark);
    }

    /// `0x0D03` (section 11): the members of the master's party on its map; the guilds pick its officers.
    pub fn guild_disband(&self, map: u16, me: u32, now: Instant) {
        let mut maps = self.maps();
        let after = {
            let parties = self.parties.guard();
            let here: Vec<u32> = parties
                .party_of(me)
                .map(|p| p.members.iter().copied().filter(|m| maps.get(&map).is_some_and(|players| players.contains_key(m))).collect())
                .unwrap_or_default();
            self.guilds().disband_request(me, &here, now)
        };
        apply(self, &mut maps, after);
    }

    pub fn guild_disband_answer(&self, me: u32, yes: bool) {
        let mut maps = self.maps();
        let after = self.guilds().disband_answer(me, yes);
        apply(self, &mut maps, after);
    }

    /// A character deleted at the character screen.
    pub fn guild_deleted(&self, id: u32) {
        let mut maps = self.maps();
        let after = self.guilds().deleted(id);
        apply(self, &mut maps, after);
    }

    /// The one-second pass: lapsed invitations and agreements.
    pub fn expire_guilds(&self, now: Instant) {
        self.guilds().expire(now);
    }
}
