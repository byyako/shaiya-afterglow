//! Lapis and enchanting (`docs/original/lapis.md`): the `08xx` requests of `CUser::PacketGem`. The decisions are `crate::lapis`'s;
//! this rolls, saves the changes with the money in one transaction, and sends the answers.

use super::*;

/// The faction grade's bonus to linking and extraction rates (`docs/original/pvp-2.md` section 8).
const LAPIS_RATE: usize = 24;
use crate::lapis::{self, EnchantRequest, ExtractRequest, LinkRequest, Refused};
use items::{BLACKSMITH, ITEM_RANGE};
use shaiya_proto::gem;

/// The original's `rand()`: 0 to 32767.
fn rand15() -> u32 {
    use rand::Rng;
    rand::rng().random::<u32>() & 0x7fff
}

/// `CUser::PacketGem`: nothing while dead; a trade is cancelled first.
fn may_work<S>(ctx: &Context<S>, state: &WorldState, id: u32) -> bool {
    if state.dead {
        return false;
    }
    ctx.zones.cancel_actions(id);
    true
}

/// Linking, extraction and enchanting need a blacksmith in view (deviation 3).
fn at_blacksmith<S>(ctx: &Context<S>, state: &WorldState, id: u32) -> bool {
    let near = ctx.zones.npc_kind_in_view(state.map, id, BLACKSMITH);
    if !near {
        tracing::debug!(id, "lapis work without a blacksmith in view ignored");
    }
    near
}

/// What a saved outcome left for the game log (`docs/original/game-log.md` deviation 4): the bags as they were, and the row each
/// change touched, with its place.
struct Saved {
    /// The items of the used-up places as they were, for their `0x050A`.
    used: Vec<((u8, u8), ItemUnit)>,
    before: Vec<ItemUnit>,
    rows: Vec<((u8, u8), crate::backend::Changed)>,
    /// The rows of places the save only touched to learn them (`save`'s `named`).
    named: Vec<((u8, u8), Option<crate::backend::Stored>)>,
}

impl Saved {
    /// The row of a place: the first change that touched it, else the one named.
    fn row(&self, place: (u8, u8)) -> Option<crate::backend::Stored> {
        let touched = self.rows.iter().find(|(at, changed)| *at == place && changed.row.is_some()).and_then(|(_, changed)| changed.row);
        touched.or_else(|| self.named.iter().find(|(at, _)| *at == place).and_then(|(_, row)| *row))
    }

    /// The item at a place as it was before the save.
    fn was(&self, place: (u8, u8)) -> Option<&ItemUnit> {
        self.before.iter().find(|i| (i.bag, i.slot) == place)
    }
}

/// The place a change is about (a move's target).
fn place_of(change: &SlotChange) -> (u8, u8) {
    match change {
        SlotChange::Insert(n) => (n.bag, n.slot),
        SlotChange::SetCount { bag, slot, .. }
        | SlotChange::SetQuality { bag, slot, .. }
        | SlotChange::SetGems { bag, slot, .. }
        | SlotChange::SetCraftName { bag, slot, .. }
        | SlotChange::Remove { bag, slot } => (*bag, *slot),
        SlotChange::Move { to, .. } => *to,
        SlotChange::Swap { a, .. } => *a,
    }
}

/// Saves an outcome's changes with its price (the equipment's unsaved wear first when the equipment changes) and applies them.
/// `named` are places whose rows the records need: one no change of the outcome touches gets a change that sets its count to what it
/// is, so that the write returns its row.
async fn save<S: Backend>(ctx: &Context<S>, id: u32, state: &mut WorldState, o: &lapis::Outcome, named: &[(u8, u8)]) -> Result<Saved> {
    let used =
        o.used_before.iter().chain(&o.used_after).filter_map(|&(b, s)| state.inventory.get(b, s).map(|i| ((b, s), i.clone()))).collect();
    let before: Vec<ItemUnit> = state.inventory.items().cloned().collect();
    let mut changes = if o.touches_equipment() { take_unsaved_wear(state) } else { Vec::new() };
    let wear = changes.len();
    changes.extend(o.changes.iter().cloned());
    let extra: Vec<(u8, u8)> = named.iter().copied().filter(|&place| !o.changes.iter().any(|c| place_of(c) == place)).collect();
    for &(bag, slot) in &extra {
        if let Some(item) = state.inventory.get(bag, slot) {
            changes.push(SlotChange::SetCount { bag, slot, count: item.count });
        }
    }
    let Some(written) = ctx.store.trade(id, -i64::from(o.price), &changes).await? else {
        bail!("the database refused lapis work the server allowed: {changes:?}");
    };
    state.money -= o.price;
    for change in &changes {
        state.inventory.apply(change);
    }
    let rows = o.changes.iter().map(place_of).zip(written[wear..wear + o.changes.len()].iter().copied()).collect();
    let named = extra.iter().copied().zip(written[wear + o.changes.len()..].iter().map(|c| c.row)).collect();
    Ok(Saved { used, before, rows, named })
}

/// `0x0402` for every item the work used up (`ItemRemove`, `ItemGemAdd`, `ItemLapisianAdd`, `ItemRemake` and the others): the lapis,
/// hammer, charm, rune, material or item that went, by the count that went.
fn log_consumed<S>(ctx: &Context<S>, character: &PlayCharacter, state: &WorldState, o: &lapis::Outcome, saved: &Saved) {
    for (change, (place, changed)) in o.changes.iter().zip(&saved.rows) {
        let Some(was) = saved.was(*place) else { continue };
        let count = match change {
            SlotChange::Remove { .. } => was.count,
            SlotChange::SetCount { count, .. } if *count < was.count => was.count - count,
            _ => continue,
        };
        let item = ctx.game_log.item(&ItemUnit { count, ..was.clone() }, changed.row);
        ctx.game_log.action(crate::game_log::item_drop(record::header(character, state), &item, place.0, place.1, count));
    }
}

/// The item at a place as the records name it now, else as it was (it was destroyed), with its row.
fn named_item<S>(ctx: &Context<S>, state: &WorldState, saved: &Saved, place: (u8, u8)) -> Option<crate::game_log::ItemPart> {
    let unit = state.inventory.get(place.0, place.1).or_else(|| saved.was(place))?;
    Some(ctx.game_log.item(unit, saved.row(place)))
}

/// The item that was at a place before the save, as the records name it.
fn was_item<S>(ctx: &Context<S>, saved: &Saved, place: (u8, u8)) -> Option<crate::game_log::ItemPart> {
    saved.was(place).map(|unit| ctx.game_log.item(unit, saved.row(place)))
}

/// `0x050A` for one item used up at a place (`CUser::ItemUseNSend`), to the players within 60.
async fn send_used<S>(
    conn: &mut Connection,
    ctx: &Context<S>,
    id: u32,
    state: &WorldState,
    (place, before): &((u8, u8), ItemUnit),
) -> Result<()> {
    let left = state.inventory.get(place.0, place.1).map_or(0, |i| i.count);
    let packet = it::used(id, place.0, place.1, before.item_type, before.type_id, left);
    conn.send(ctx.zones.send_near(state.map, id, packet, ITEM_RANGE)).await
}

async fn send_used_at<S>(
    conn: &mut Connection,
    ctx: &Context<S>,
    id: u32,
    state: &WorldState,
    used: &[((u8, u8), ItemUnit)],
    places: &[(u8, u8)],
) -> Result<()> {
    for entry in used.iter().filter(|(p, _)| places.contains(p)) {
        send_used(conn, ctx, id, state, entry).await?;
    }
    Ok(())
}

/// What the view sees of an equipment slot again (`CUser::SendEquipment`).
async fn show_equipment<S>(conn: &mut Connection, ctx: &Context<S>, id: u32, state: &WorldState, (bag, slot): (u8, u8)) -> Result<()> {
    if bag == 0 && slot < VISIBLE_SLOTS {
        conn.send(ctx.zones.change_equipment(state.map, id, slot, state.inventory.get(0, slot))).await?;
    }
    Ok(())
}

/// After the answer: the item destroyed (section 9), then the numbers again when the equipment changed.
async fn finish<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    o: &lapis::Outcome,
) -> Result<()> {
    if let Some(place) = o.destroyed {
        tracing::info!(id = character.id, ?place, "item destroyed by lapis work");
        conn.send(gp::item_left(place.0, place.1, None)).await?;
        show_equipment(conn, ctx, character.id, state, place).await?;
    }
    if o.touches_equipment() {
        renumber(conn, ctx, character, state).await?;
    }
    Ok(())
}

/// `0x0801` (`CUser::ItemGemAdd`, section 3).
pub(super) async fn link<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    places: [(u8, u8); 3],
) -> Result<()> {
    let id = character.id;
    if !may_work(ctx, state, id) || !at_blacksmith(ctx, state, id) {
        return Ok(());
    }
    let house = match super::guild_house::lapis_rates(ctx, state.map, id, true) {
        Ok(house) => house,
        Err(refused) => return conn.send(refused).await,
    };
    let request = LinkRequest { lapis: places[0], item: places[1], hammer: places[2] };
    let kc = (character.level, state.grade[LAPIS_RATE]);
    let Some(link) =
        lapis::link(&state.inventory, &ctx.data.items, state.money, character.vg, kc, request, house, matches!(state.buffs.premium, 2 | 3))
    else {
        tracing::debug!(id, ?places, "link ignored");
        return Ok(());
    };
    let success = rand15() % 100 < link.rate as u32;
    let o = link.outcome(&state.inventory, success);
    let saved = save(ctx, id, state, &o, &[request.item, request.lapis]).await?;
    let used = saved.used.clone();
    // `0x0409` (`CUser::ItemGemAdd`): the item with its lapis, then `0x0402` for what was used up.
    if let (Some(item), lapis) = (named_item(ctx, state, &saved, request.item), was_item(ctx, &saved, request.lapis)) {
        ctx.game_log.action(crate::game_log::gem_add(record::header(character, state), &item, lapis.as_ref(), None, link.price, success));
    }
    log_consumed(ctx, character, state, &o, &saved);
    tracing::info!(id, ?places, rate = link.rate, success, price = link.price, "lapis link");
    send_used_at(conn, ctx, id, state, &used, &o.used_before).await?;
    let left = state.inventory.get(request.lapis.0, request.lapis.1).map_or(0, |i| i.count);
    let hammer = link.hammer.unwrap_or((0, 0));
    conn.send(gem::linked(success, (request.lapis, left), request.item, (link.position, link.lapis), state.money, hammer)).await?;
    send_used_at(conn, ctx, id, state, &used, &o.used_after).await?;
    finish(conn, ctx, character, state, &o).await
}

/// `0x0802` (`CUser::ItemGemRemove`, section 4).
pub(super) async fn extract<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    request: gem::Extract,
) -> Result<()> {
    let id = character.id;
    if !may_work(ctx, state, id) || !at_blacksmith(ctx, state, id) {
        return Ok(());
    }
    let long_form = request.long_form;
    let request = match (request.flag, request.position) {
        (0, _) => ExtractRequest::All { place: request.place },
        (1, position @ 0..6) => ExtractRequest::One { place: request.place, position, hammer: request.hammer },
        _ => return Ok(()),
    };
    let house = match super::guild_house::lapis_rates(ctx, state.map, id, false) {
        Ok(house) => house,
        Err(refused) => return conn.send(refused).await,
    };
    let kc = (character.level, state.grade[LAPIS_RATE]);
    let Some(e) = lapis::extract(&state.inventory, &ctx.data.items, state.money, character.vg, kc, request, house) else {
        tracing::debug!(id, ?request, "extraction ignored");
        return Ok(());
    };
    let success = rand15() % 100 < e.rate as u32;
    let o = match e.outcome(&state.inventory, &ctx.data.items, success) {
        Ok(o) => o,
        Err(hammer) => {
            // Success without room: only the hammer is gone, and nothing is answered.
            tracing::info!(id, ?request, "extraction succeeded without room");
            let o =
                lapis::Outcome { changes: hammer.into_iter().collect(), used_before: e.hammer.into_iter().collect(), ..Default::default() };
            let saved = save(ctx, id, state, &o, &[]).await?;
            log_consumed(ctx, character, state, &o, &saved);
            return send_used_at(conn, ctx, id, state, &saved.used, &o.used_before).await;
        }
    };
    let saved = save(ctx, id, state, &o, &[e.place]).await?;
    let used = saved.used.clone();
    // `0x040A` (`CUser::ItemGemRemoveAll`, `ItemGemRemovePos`): the item with each lapis taken out (none named when it failed), then
    // `0x0402` for what was used up.
    if let Some(item) = named_item(ctx, state, &saved, e.place) {
        let header = record::header(character, state);
        let taken: Vec<_> = o
            .changes
            .iter()
            .zip(&saved.rows)
            .filter_map(|(change, (place, changed))| match change {
                SlotChange::Insert(_) => state.inventory.get(place.0, place.1).map(|unit| ctx.game_log.item(unit, changed.row)),
                _ => None,
            })
            .collect();
        if taken.is_empty() {
            ctx.game_log.action(crate::game_log::gem_remove(header.clone(), &item, None, None, e.price, success));
        }
        for lapis in &taken {
            ctx.game_log.action(crate::game_log::gem_remove(header.clone(), &item, Some(lapis), None, e.price, success));
        }
    }
    log_consumed(ctx, character, state, &o, &saved);
    tracing::info!(id, ?request, rate = e.rate, success, price = e.price, "lapis extraction");
    send_used_at(conn, ctx, id, state, &used, &o.used_before).await?;
    conn.send(gem::extracted(success, e.place, long_form.then_some(e.position), &o.extracted, state.money)).await?;
    finish(conn, ctx, character, state, &o).await
}

/// `0x0805` (`CUser::ItemLapisianAdd`, section 5).
pub(super) async fn enchant<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    gem::Enchant { lapisia, material, count, item }: gem::Enchant,
) -> Result<()> {
    let id = character.id;
    if !may_work(ctx, state, id) || !at_blacksmith(ctx, state, id) {
        return Ok(());
    }
    let request = EnchantRequest { lapisia, material, count, item };
    let Some(mut e) = lapis::enchant(&state.inventory, &ctx.data.items, state.money, request) else {
        tracing::debug!(id, ?request, "enchant ignored");
        return Ok(());
    };
    let success = {
        use rand::Rng;
        e.succeeds(rand::rng().random_range(0..lapis::ENCHANT_DRAW))
    };
    // A success may rise more than one step: the first `enchant` rule in scope draws the jump with a draw of its own
    // (`docs/plans/ext-2-rules.md` point 8). A failure is as it was.
    if success {
        use rand::Rng;
        let who = rules_who(character, character.family / 2);
        let draw = rand::rng().random_range(0..lapis::ENCHANT_DRAW);
        let jump = ctx.zones.rules().enchant_jump(state.map, &who, e.armour, draw);
        if jump.value != 1 {
            tracing::debug!(id, map = state.map, point = "enchant", rules = ?jump.rules, before = 1, after = jump.value, "changed by rules");
        }
        // The scripts after the rules: the jump so far in, a jump out (1 to 19); no lock is held here (section 4.3).
        let item_unit = state.inventory.get(item.0, item.1).map(|i| (i.item_type, i.type_id, e.step));
        e.jump = match item_unit {
            Some(unit) => super::script::enchant_jump(ctx, character, state.map, unit, jump.value),
            None => jump.value,
        };
    }
    let o = e.outcome(&state.inventory, &ctx.data.items, success);
    let name_before = state.inventory.get(item.0, item.1).map(|i| i.craft_name.clone()).unwrap_or_default();
    let saved = save(ctx, id, state, &o, &[item, lapisia]).await?;
    // `0x0426` (`CUser::ItemLapisianAdd`): the item with the Lapisia, then `0x0402` for what was used up.
    if let Some(named) = named_item(ctx, state, &saved, item) {
        let lapisia = was_item(ctx, &saved, lapisia);
        ctx.game_log.action(crate::game_log::lapisian_add(
            record::header(character, state),
            &named,
            lapisia.as_ref(),
            None,
            e.price,
            success,
        ));
    }
    log_consumed(ctx, character, state, &o, &saved);
    tracing::info!(id, ?request, step = e.step, success, jump = e.jump, price = e.price, "enchant");
    let left = |place: (u8, u8)| state.inventory.get(place.0, place.1).map_or(0, |i| i.count);
    let material_left = (material, if e.material.is_some() { left(material) } else { 0 });
    let name = state.inventory.get(item.0, item.1).map_or(name_before, |i| i.craft_name.clone());
    conn.send(gem::enchanted(success, (lapisia, left(lapisia)), material_left, item, state.money, &name)).await?;
    if success {
        show_equipment(conn, ctx, id, state, item).await?;
    }
    finish(conn, ctx, character, state, &o).await
}

/// `0x0806` (`CUser::ItemCompose`, section 6): new craft stats by Recreation Rune.
pub(super) async fn compose<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    (rune, place): ((u8, u8), (u8, u8)),
) -> Result<()> {
    let id = character.id;
    if !may_work(ctx, state, id) {
        return Ok(());
    }
    let make_type = ctx.store.make_types(id, &[place]).await?[0];
    match lapis::compose(&state.inventory, &ctx.data.items, rune, place, make_type) {
        Err(Refused::Ignored) => Ok(()),
        Err(Refused::Result(_)) => conn.send(gem::rerolled(gem::OP_COMPOSE, None)).await,
        Ok(place) => {
            let rune_item = state.inventory.get(rune.0, rune.1).cloned();
            let use_rune = rune_item.as_ref().map(|r| {
                if r.count <= 1 {
                    SlotChange::Remove { bag: rune.0, slot: rune.1 }
                } else {
                    SlotChange::SetCount { bag: rune.0, slot: rune.1, count: r.count - 1 }
                }
            });
            reroll(conn, ctx, character, state, gem::OP_COMPOSE, place, 2, use_rune, rune_item.clone()).await?;
            if let Some(before) = rune_item {
                send_used(conn, ctx, id, state, &(rune, before)).await?;
            }
            Ok(())
        }
    }
}

/// `0x0808` (`CUser::ItemConsider`, section 6): craft stats for an item without any, free.
pub(super) async fn consider<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    place: (u8, u8),
) -> Result<()> {
    let id = character.id;
    if !may_work(ctx, state, id) {
        return Ok(());
    }
    let make_type = ctx.store.make_types(id, &[place]).await?[0];
    match lapis::consider(&state.inventory, &ctx.data.items, place, make_type) {
        Err(Refused::Ignored) => Ok(()),
        Err(Refused::Result(_)) => conn.send(gem::rerolled(gem::OP_CONSIDER, None)).await,
        Ok(()) => reroll(conn, ctx, character, state, gem::OP_CONSIDER, place, 3, None, None).await,
    }
}

/// The new craft numbers of an item, saved with `extra` (the rune used), answered with `op`.
#[allow(clippy::too_many_arguments)]
async fn reroll<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    op: u16,
    place: (u8, u8),
    draws: u8,
    extra: Option<SlotChange>,
    rune: Option<ItemUnit>,
) -> Result<()> {
    let Some(item) = state.inventory.get(place.0, place.1).cloned() else { return Ok(()) };
    let Some(def) = ctx.data.items.get(item.item_type, item.type_id) else { return Ok(()) };
    let values = lapis::roll_craft(def, draws, &mut rand15);
    let craft_name = lapis::rerolled_name(&item, values);
    let mut changes = vec![SlotChange::SetCraftName { bag: place.0, slot: place.1, craft_name }];
    changes.extend(extra);
    let o = lapis::Outcome { success: true, changes, ..Default::default() };
    let saved = save(ctx, character.id, state, &o, &[place]).await?;
    // `0x0427` (`CUser::ItemCompose`: the rune and the item's gems with the old craft name) or `0x0428` (`ItemConsider`), then
    // `0x0402` for the rune used up.
    if let Some(named) = named_item(ctx, state, &saved, place) {
        let header = record::header(character, state);
        if op == gem::OP_COMPOSE {
            let old = crate::game_log::ItemPart { craft_name: item.craft_name.clone(), gems: item.gems, ..Default::default() };
            let rune_part = rune.as_ref().map(|r| ctx.game_log.item(r, saved.row((r.bag, r.slot))));
            ctx.game_log.action(crate::game_log::item_compose(header, &named, rune_part.as_ref(), &old.gems_text()));
        } else {
            ctx.game_log.action(crate::game_log::item_consider(header, &named));
        }
    }
    log_consumed(ctx, character, state, &o, &saved);
    tracing::info!(id = character.id, ?place, op, name = %String::from_utf8_lossy(&craft_name), "new craft stats");
    let name = state.inventory.get(place.0, place.1).map(|i| i.craft_name.clone()).unwrap_or_default();
    conn.send(gem::rerolled(op, Some((place, &name)))).await?;
    finish(conn, ctx, character, state, &o).await
}

/// `0x0807` (`CUser::ItemRemake`, section 7): one new item from three of the same.
pub(super) async fn remake<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    places: [(u8, u8); 3],
) -> Result<()> {
    let id = character.id;
    if !may_work(ctx, state, id) {
        return Ok(());
    }
    let types = ctx.store.make_types(id, &places).await?;
    let make_types = [types[0], types[1], types[2]];
    let at = match lapis::remake(&state.inventory, &ctx.data.items, places, make_types) {
        Err(Refused::Ignored) => return Ok(()),
        Err(Refused::Result(result)) => return conn.send(gem::remade(result, None)).await,
        Ok(at) => at,
    };
    let Some(def) = state.inventory.get(places[0].0, places[0].1).and_then(|i| ctx.data.items.get(i.item_type, i.type_id)).copied() else {
        return Ok(());
    };
    let values = lapis::roll_craft(&def, 3, &mut rand15);
    let mut changes: Vec<SlotChange> = places.iter().map(|&(bag, slot)| SlotChange::Remove { bag, slot }).collect();
    changes.push(SlotChange::Insert(lapis::remade(&def, at, values)));
    let o = lapis::Outcome { success: true, changes, ..Default::default() };
    let saved = save(ctx, id, state, &o, &[at]).await?;
    // `0x0402` for the three used up, then `0x0401` (`'Remake'`) for the new item and `0x0429` with the three by unique id
    // (`CUser::ItemRemake`).
    log_consumed(ctx, character, state, &o, &saved);
    if let Some(made) = named_item(ctx, state, &saved, at) {
        let count = state.inventory.get(at.0, at.1).map_or(1, |i| i.count);
        let used = places.map(|place| saved.row(place).map(|row| row.uid));
        let header = record::header(character, state);
        ctx.game_log.action(crate::game_log::item_remade_get(header.clone(), &made, count));
        ctx.game_log.action(crate::game_log::item_remake(header, &made, used));
    }
    tracing::info!(id, ?places, ?at, "remake");
    for place in places {
        conn.send(gp::item_left(place.0, place.1, None)).await?;
        show_equipment(conn, ctx, id, state, place).await?;
    }
    conn.send(gem::remade(0, state.inventory.get(at.0, at.1))).await?;
    finish(conn, ctx, character, state, &o).await
}

/// `0x0803` (`CUser::ItemCloakAdd`, section 8): a badge on a cloak for 450000.
pub(super) async fn badge<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    (place, badge): ((u8, u8), [u8; 6]),
) -> Result<()> {
    let id = character.id;
    if !may_work(ctx, state, id) || !lapis::cloak_at(&state.inventory, &ctx.data.items, place) {
        return Ok(());
    }
    if state.money < lapis::BADGE_PRICE {
        return conn.send(gem::badge_result(false, place, badge, state.money)).await;
    }
    let o = lapis::Outcome {
        success: true,
        price: lapis::BADGE_PRICE,
        changes: vec![SlotChange::SetGems { bag: place.0, slot: place.1, gems: badge }],
        ..Default::default()
    };
    let saved = save(ctx, id, state, &o, &[]).await?;
    // `0x0412` (`CUser::ItemCloakAdd`): the cloak with the badge, the price and the money after.
    if let Some(item) = named_item(ctx, state, &saved, place) {
        ctx.game_log.action(crate::game_log::cloak_add(record::header(character, state), &item, 1, lapis::BADGE_PRICE, state.money));
    }
    tracing::info!(id, ?place, ?badge, "cloak badge");
    conn.send(gem::badge_result(true, place, badge, state.money)).await?;
    if place == (0, lapis::CLOAK_SLOT) {
        show_equipment(conn, ctx, id, state, place).await?;
    }
    Ok(())
}

/// `0x0804` (`CUser::ItemCloakRemove`, section 8): a cloak's badge off, free.
pub(super) async fn remove_badge<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    place: (u8, u8),
) -> Result<()> {
    let id = character.id;
    if !may_work(ctx, state, id) || !lapis::cloak_at(&state.inventory, &ctx.data.items, place) {
        return Ok(());
    }
    let o = lapis::Outcome {
        success: true,
        changes: vec![SlotChange::SetGems { bag: place.0, slot: place.1, gems: [0; 6] }],
        ..Default::default()
    };
    save(ctx, id, state, &o, &[]).await?;
    conn.send(gem::badge_removed(place)).await?;
    if place == (0, lapis::CLOAK_SLOT) {
        show_equipment(conn, ctx, id, state, place).await?;
    }
    Ok(())
}
