//! Player kills on the connection's side (`docs/original/pvp.md`): the kill and death counters, what a victim loses to its killer,
//! and the reward panels `0x0218` to `0x021A`. The rules are `crate::pvp`'s; the zone does what touches other players.

use super::*;
use crate::pvp;
use crate::sync::Relock;

/// Chance in percent that a victim loses gold (`CUser::ItemDropByUser`).
const GOLD_LOSS_CHANCE: u32 = 30;

/// A kill counted (`CUser::KillPCTo`, section 7): the player-kill quest, then below the cap kills + 1, saved, `0x020E` and
/// `0x0522` to the view. The console's kill rate adds more than 1, times the kill boost's multiplier while the killer has one of the
/// kill-boost skills applied (`docs/original/operator-console.md` section 6 step 2); the faction's count and the quest stay 1. The
/// rules' `pvp_gain.kills` first (`docs/plans/ext-2-rules.md` point 6): 0 counts nothing at all, not even the quest; otherwise it
/// multiplies the kills added.
pub(super) async fn kill_counted<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &mut PlayCharacter,
    state: &mut WorldState,
    country: u8,
) -> Result<()> {
    let times = ctx.zones.rules().pvp_kills(state.map, &rules_who(character, country));
    if times.value == 0 {
        tracing::debug!(id = character.id, map = state.map, point = "pvp_kills", rules = ?times.rules, before = 1, after = 0, "changed by rules");
        return Ok(());
    }
    // The scripts after the rules' `times` (a rule's 0 is final): their whole numbers multiply it; 0 counts nothing, the quest
    // included. A connection's `async fn`, no lock held (`docs/plans/ext-5-lua.md` section 4.3).
    let times = times.value.saturating_mul(super::script::kills_times(ctx, character, state.map, times.value));
    if times == 0 {
        return Ok(());
    }
    quest_player_kill(conn, ctx, character, state).await?;
    let by = ctx
        .switches
        .kill_rate()
        .saturating_mul(ctx.zones.kill_boost(state.map, character.id, &ctx.config.kill_boost_skills))
        .saturating_mul(times);
    let Some(kills) = pvp::counted(character.kills[0], by) else { return Ok(()) };
    character.kills[0] = kills;
    // The faction's kill count (`CWorld::AddKillCount`, `docs/original/pvp-2.md` section 9).
    let changes = ctx.kill_counts.guard().add(country, 1, Instant::now());
    kill_count_changes(ctx, changes).await;
    ctx.store.save_kills(character.id, character.kills).await?;
    conn.send(cp::kill_count(cp::COUNT_KILLS, kills)).await?;
    conn.send(ctx.zones.set_kills(state.map, character.id, kills)).await?;
    Ok(())
}

/// A death counted (`CUser::KillPCFrom`): below the cap deaths + 1, saved, `0x020E`.
pub(super) async fn death_counted<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &mut PlayCharacter,
    _state: &mut WorldState,
) -> Result<()> {
    let Some(deaths) = pvp::counted(character.kills[1], 1) else { return Ok(()) };
    character.kills[1] = deaths;
    ctx.store.save_kills(character.id, character.kills).await?;
    conn.send(cp::kill_count(cp::COUNT_DEATHS, deaths)).await?;
    Ok(())
}

/// `CUser::QuestUpdateKillPC` (`docs/original/quest.md` section 6): the first player-kill quest in progress counts it; without an
/// end NPC it ends when the count is reached.
async fn quest_player_kill<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &mut PlayCharacter,
    state: &mut WorldState,
) -> Result<()> {
    let Some(((quest_id, index, count), complete)) = state.quests.player_killed(&ctx.data.quests) else { return Ok(()) };
    conn.send(qp::update(quest_id, index, count)).await?;
    if let Some(active) = state.quests.find(quest_id) {
        ctx.store.quest_counts(character.id, quest_id, active.counts).await?;
    }
    if complete && let (Some(def), Some(active)) = (ctx.data.quests.get(quest_id), state.quests.find(quest_id).copied()) {
        match quests::outcome(def, active.counts, &state.inventory) {
            EndOutcome::Choose => {
                state.quests.set_choosing(quest_id);
                conn.send(qp::choose(quest_id)).await?;
            }
            EndOutcome::Finish(result) => finish_quest(conn, ctx, character, state, def, result, 0).await?,
            EndOutcome::Ignored => {}
        }
    }
    Ok(())
}

/// What a victim of a player loses (`CUser::ItemDropByUser`, section 6.2): nothing at level 10 or below; a player maybe one
/// weapon, armour or accessory by its VG, staff one to three items; then 30 % of the time some gold. Each goes to the killer or its
/// party (`Zones::victim_item`, `Zones::victim_gold`).
pub(super) async fn lose_to<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    player: &Player,
    character: &mut PlayCharacter,
    state: &mut WorldState,
    killer: u32,
) -> Result<()> {
    if !pvp::can_lose(character.level) {
        return Ok(());
    }
    use rand::{Rng, SeedableRng};
    // Held across the database calls, so not the thread's generator.
    let mut rng = rand::rngs::StdRng::from_rng(&mut rand::rng());
    let mut rand = move || rng.random::<u32>() & 0x7fff;
    let staff = player.privilege.load(std::sync::atomic::Ordering::Relaxed) <= enter::STAFF_PRIVILEGE_MAX;
    let rounds = if staff { rand() % 3 + 1 } else { u32::from(rand() % 100 < pvp::loss_chance(character.vg)) };
    for _ in 0..rounds {
        let found = pvp::loss_candidates(state.inventory.items(), state.inventory.max_bag(), staff, &ctx.data.items);
        if found.is_empty() {
            continue;
        }
        let (bag, slot) = found[(rand() as usize) % found.len()];
        lose_item(conn, ctx, character, state, killer, bag, slot, &mut rand).await?;
    }
    if state.money > 0 && rand() % 100 < GOLD_LOSS_CHANCE {
        let loss = pvp::gold_loss(character.level, state.money, rand());
        if ctx.store.trade(character.id, -i64::from(loss), &[]).await?.is_none() {
            bail!("the database refused a gold loss the server allowed");
        }
        state.money -= loss;
        conn.send(w::set_money(state.money)).await?;
        if let Some(packet) = ctx.zones.victim_gold(state.map, character.id, killer, loss, Instant::now(), &mut rand) {
            conn.send(packet).await?;
        }
        tracing::info!(id = character.id, killer, loss, "gold lost to a player");
    }
    Ok(())
}

/// One item out of its slot whole (`CUser::ItemDropByUserDeath`): saved as a drop keeping all it is, `0x0206` with the slot
/// empty, then into the killer's hands or onto the ground.
#[allow(clippy::too_many_arguments)]
async fn lose_item<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    killer: u32,
    bag: u8,
    slot: u8,
    rand: &mut impl FnMut() -> u32,
) -> Result<()> {
    let id = character.id;
    let Some(count) = state.inventory.get(bag, slot).map(|i| i.count) else { return Ok(()) };
    let Some(drop) = state.inventory.drop_from(&ctx.data.items, bag, slot, count) else { return Ok(()) };
    let Some((kept, row)) = ctx.store.drop_item(id, bag, slot, drop.had, drop.count).await? else {
        bail!("the database refused a death loss the server allowed: {drop:?}");
    };
    // `0x0407` (`CUser::ItemDropByUserDeath`): the item lost, named by the row it had, and who it went to.
    if let Some(unit) = state.inventory.get(bag, slot) {
        let item = ctx.game_log.item(unit, Some(row));
        let name = ctx.zones.name_of(killer).unwrap_or_default();
        ctx.game_log.action(crate::game_log::death_drop(record::header(character, state), &item, killer, &name, bag, slot));
    }
    state.inventory.apply(&drop.change);
    conn.send(gp::item_left(bag, slot, state.inventory.get(bag, slot))).await?;
    let loot = Loot::Item { item_type: drop.item_type, type_id: drop.type_id, count: drop.count, quality: drop.quality, kept: Some(kept) };
    if let Some(packet) = ctx.zones.victim_item(state.map, id, killer, loot, Instant::now(), rand) {
        conn.send(packet).await?;
    }
    tracing::info!(id, killer, bag, slot, ?loot, "item lost to a player");
    Ok(())
}

/// The reward panels (section 8): `0x0218` the next steps, `0x0219` the next kill step's stat points, `0x021A` the next death
/// step's gold. Nothing in easy mode or below level 16; one step per request.
pub(super) async fn reward_panel<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &mut PlayCharacter,
    state: &mut WorldState,
    opcode: u16,
) -> Result<()> {
    if character.grow == 0 || character.level < pvp::PANEL_LEVEL {
        return Ok(());
    }
    let id = character.id;
    match opcode {
        cp::OP_STATUS_INFO => {
            let (kill, death) = pvp::next_steps(character.grow, character.kill_steps);
            conn.send(cp::status_info(kill, death)).await?;
        }
        cp::OP_STATUS_KILL => {
            let Some(reward) = pvp::claim(true, character.grow, character.kills[0], character.kill_steps[0]) else { return Ok(()) };
            let given = match reward {
                Some(r) => {
                    // The points and the step in one write that holds only while the step is the old one (audit R12).
                    let points = character.stat_point.saturating_add(r.value as u16);
                    if !ctx.store.claim_kill_reward(id, points, character.kill_steps[0], r.step).await? {
                        bail!("the database refused a kill reward the server allowed: step {}", r.step);
                    }
                    character.stat_point = points;
                    character.kill_steps[0] = r.step;
                    // `0x030B` (`CUser::StatusKillResult`): the step and the points it gave.
                    ctx.game_log.action(crate::game_log::kill_step(record::header(character, state), r.step, r.value));
                    tracing::info!(id, step = r.step, points = r.value, "kill reward taken");
                    r.value
                }
                None => 0,
            };
            conn.send(cp::status_kill(reward.is_some(), given as u8)).await?;
        }
        _ => {
            let Some(reward) = pvp::claim(false, character.grow, character.kills[1], character.kill_steps[1]) else { return Ok(()) };
            let given = match reward {
                Some(r) => {
                    // At most 2^32 - 1 in all.
                    let gold = r.value.min(u32::MAX - state.money);
                    // The gold and the step in one write that holds only while the step is the old one (audit R12).
                    if !ctx.store.claim_death_reward(id, gold, character.kill_steps[1], r.step).await? {
                        bail!("the database refused a death reward the server allowed: step {}", r.step);
                    }
                    state.money += gold;
                    character.kill_steps[1] = r.step;
                    // `0x030C` (`CUser::StatusDeathResult`): the step and the gold it gave.
                    ctx.game_log.action(crate::game_log::death_step(record::header(character, state), r.step, r.value));
                    tracing::info!(id, step = r.step, gold = r.value, "death reward taken");
                    r.value
                }
                None => 0,
            };
            conn.send(cp::status_death(reward.is_some(), given)).await?;
        }
    }
    Ok(())
}
