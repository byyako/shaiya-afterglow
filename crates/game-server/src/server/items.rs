//! Using items (`docs/original/item-use.md`): the requests `0x050A`, `0x0557` and `0x0559`, the resets (by item and the free ones,
//! `0x0214`, `0x0215`), the moves items make after their casting, and the summon's answer (`0x0224`). The decisions are
//! `crate::item_use`'s; the zone does what touches other players.

use super::*;
use crate::drops::BoxRoll;
use crate::inventory::NewItem;
use crate::item_use::{self, Armed, Effect};
use crate::sync::Relock;
use crate::world::create_type;
use crate::zone::Member;
use shaiya_proto::item as it;

/// Range of the item packets (`CZone::PSendView` with 60).
pub(super) const ITEM_RANGE: f32 = 60.0;
/// The make type of an item from a box (`CUser::ItemCreate`).
const BOX_MAKE_TYPE: u8 = b'S';

/// The pending move (`m_dwMovePositionTime`): `/town` (type 0) or a move by item waiting for its casting (types 3, 4, 5; section 9).
#[derive(Clone, Copy, Debug)]
pub(super) struct ItemMove {
    pub due: Instant,
    /// `None` for `/town`: to the bind position of the map the player is on, with no item (`docs/original/small-items.md` section 6).
    by_item: Option<ByItem>,
}

impl ItemMove {
    pub(super) fn town(due: Instant) -> Self {
        Self { due, by_item: None }
    }
}

#[derive(Clone, Copy, Debug)]
struct ByItem {
    kind: MoveKind,
    /// The item's place and special, looked at again when the move is due (`m_nSavePosUseBag/Slot`).
    bag: u8,
    slot: u8,
    special: u8,
    /// `0x0222`'s result byte (`m_nSavePosUseIndex`).
    result: u8,
    /// The reuse group's time, freed when the move fails or is cancelled (`m_dwItemCoolTimeIndex`).
    armed: Armed,
}

#[derive(Clone, Copy, Debug)]
enum MoveKind {
    To(u16, [f32; 3]),
    /// Into the player's guild house on that map: its zone is found or made when the move is due.
    GuildHouse(u16, [f32; 3]),
    Summon,
}

/// What an effect came to.
pub(super) enum Done {
    /// The item is used up.
    UsedUp,
    /// The item is used up for this new one, saved together (a box, `docs/original/item-origins.md` section 5).
    UsedUpFor(NewItem),
    /// The item was used up and saved before the effect saved something lasting (see `consume`): `0x050A` left to send.
    Consumed(ItemUnit),
    /// A move by item is under way: the item goes when it happens.
    Pending,
    /// The error answer, the reuse time put back.
    Failed,
}

/// The quest item used in the last 5 minutes, by type id (start type 4, `docs/original/quest.md` section 5).
pub(super) fn used_quest_item(state: &WorldState) -> Option<u8> {
    state.quest_item.filter(|&(_, until)| Instant::now() < until).map(|(type_id, _)| type_id)
}

/// `0x050A`, `0x0557`, `0x0559` (`CUser::PacketPC`, `CUser::ItemUse`; sections 1 to 4).
pub(super) async fn use_item<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    player: &Player,
    character: &mut PlayCharacter,
    state: &mut WorldState,
    request: it::Use,
) -> Result<()> {
    let id = character.id;
    let (bag, slot) = (request.bag, request.slot);
    if state.dead || bag == 0 || !state.inventory.is_bag((bag, slot)) {
        return Ok(());
    }
    // The trade ends first (`CUser::CancleActionExc`).
    ctx.zones.cancel_actions(id);
    let Some(item) = state.inventory.get(bag, slot).filter(|item| item.count > 0) else { return Ok(()) };
    let Some(def) = ctx.data.items.get(item.item_type, item.type_id).copied() else { return Ok(()) };
    let now = Instant::now();
    let effect = item_use::effect_in_force(&def, &ctx.data.effects);
    if let Effect::QuestItem { used_up } = effect {
        state.quest_item = Some((def.type_id, now + item_use::QUEST_ITEM_MEMORY));
        if used_up {
            use_up(conn, ctx, character, state, bag, slot).await?;
        }
        return Ok(());
    }
    // An item a script took over (`game.on_item`) is asked after the core's level, race and mode test and the reuse time, so the
    // core's own refusal of its effect waits for the script's answer (`docs/plans/ext-5-lua.md` section 6.1).
    let scripted_item = super::script::owns_item(ctx, def.item_type, def.type_id);
    if (effect == Effect::Refused && !scripted_item) || !item_use::may_use(&def, character.level, character.family, character.grow) {
        tracing::debug!(id, bag, slot, item = def.item_id, ?effect, "item use refused");
        return conn.send(it::use_failed(id)).await;
    }
    // The skill-cut fix (deviation): no move by item within 3 s of a skill request, whose animation its casting would cut short.
    if effect.casts() && state.last_skill_request.is_some_and(|at| now < at + item_use::SKILL_CUT_GUARD) {
        tracing::debug!(id, item = def.item_id, "move by item refused: right after a skill");
        return conn.send(it::use_failed(id)).await;
    }
    // The rules' `item_use` point (`docs/plans/ext-3-effects.md` section 6) after the core's checks and before the reuse group, so a
    // refusal leaves the reuse time alone; a rule's refusal is final.
    let country = player.faction.map_or(0, |f| f.country);
    if let Some(refusal) = ctx.zones.rules().item_use(state.map, &rules_who(character, country), def.item_type, def.type_id) {
        tracing::debug!(id, map = state.map, point = "item_use", rule = &*refusal.name, item = def.item_id, "refused by a rule");
        super::refused::tell_refused(conn, state, &refusal).await?;
        return conn.send(it::use_failed(id)).await;
    }
    let Some(armed) = state.reuse.start(def.reuse_group, now) else {
        tracing::debug!(id, item = def.item_id, group = def.reuse_group, "item use refused: reuse time");
        return conn.send(it::use_failed(id)).await;
    };
    // The script that took the item over answers: "use" (one used up), "keep" (done, nothing used up), false (refused) or nothing
    // (the core's own use). The connection's own task, no lock held (section 4.3).
    let scripted = if scripted_item {
        let unit = state.inventory.get(bag, slot).cloned().expect("the item was found above");
        match super::script::item_use(ctx, character, &unit, bag, slot) {
            (crate::script::points::ItemUse::Use, _) => Some(Done::UsedUp),
            (crate::script::points::ItemUse::Keep, _) => Some(Done::Pending),
            (crate::script::points::ItemUse::Refused(_), refusal) => {
                if let Some(refusal) = refusal {
                    super::refused::tell_refused(conn, state, &refusal).await?;
                }
                Some(Done::Failed)
            }
            (crate::script::points::ItemUse::Core, _) if effect == Effect::Refused => Some(Done::Failed),
            (crate::script::points::ItemUse::Core, _) => None,
        }
    } else {
        None
    };
    let done = if let Some(early) = scripted {
        early
    } else {
        match effect {
            Effect::Skill { id: skill_id, level } if request.kind != it::KIND_SELF => {
                let target = (request.kind, request.target.unwrap_or(0));
                item_skill_on(conn, ctx, character, state, country, (skill_id, level), target).await?
            }
            Effect::Skill { id: skill_id, level } => item_skill(conn, ctx, character, state, country, skill_id, level, (bag, slot)).await?,
            Effect::ExpWheel { .. } | Effect::Ext if request.kind != it::KIND_SELF => Done::Failed,
            // An extension's effect (`docs/plans/ext-3-effects.md` section 3.3), after the rules, the reuse time and the script.
            Effect::Ext => super::effects::use_effect(conn, ctx, character, state, def.item_id, (bag, slot)).await?,
            Effect::ExpWheel { id: skill_id, level } => {
                exp_wheel(conn, ctx, character, state, country, skill_id, level, (bag, slot)).await?
            }
            // Recover, Cure, the resurrection of another player and the Message To Server stay effect-then-use-up (audit R13): the effect
            // is transient and its worst case is a free potion on a database failure, which ends the connection anyway.
            Effect::Recover(amounts) => recovered(conn, ctx.zones.recover_add(state.map, id, amounts)).await?,
            Effect::RecoverPercent(percent) => recovered(conn, ctx.zones.recover_percent(state.map, id, percent)).await?,
            Effect::Cure(cure) => {
                for packet in ctx.zones.cure(state.map, id, &cure) {
                    conn.send(packet).await?;
                }
                Done::UsedUp
            }
            Effect::StatReset => {
                let used = consume(ctx, character, state, bag, slot).await?;
                reset_stats(conn, ctx, character, state).await?;
                used
            }
            Effect::SkillReset => {
                let used = consume(ctx, character, state, bag, slot).await?;
                reset_skills(conn, ctx, character, state, false).await?;
                used
            }
            Effect::Recall(special) => match item_use::recall_destination(special, country) {
                Some((to_map, position)) if recall_allowed(ctx, state, character.level, to_map, position) => {
                    start_move(
                        conn,
                        ctx,
                        id,
                        state,
                        MoveKind::To(to_map, position),
                        (bag, slot, def.special),
                        item_use::RESULT_RECALL,
                        armed,
                    )
                    .await?
                }
                _ => Done::Failed,
            },
            Effect::GuildHouseStone => {
                let (to_map, position) = item_use::guild_house_destination(country);
                let allowed = item_use::level_allows(to_map, character.level)
                    && ctx.data.world.is_safe_pos(to_map, position)
                    && ctx.data.world.create_type(to_map) == create_type::GUILD_HOUSE
                    && ctx.zones.guild_house_of(id).is_some_and(|(_, _, house)| house.enabled())
                    && state.item_move.is_none()
                    && state.logout.is_none();
                if allowed {
                    let kind = MoveKind::GuildHouse(to_map, position);
                    start_move(conn, ctx, id, state, kind, (bag, slot, def.special), item_use::RESULT_RECALL, armed).await?
                } else {
                    Done::Failed
                }
            }
            Effect::Teleport => match teleport_target(ctx, character, state, request.target) {
                Some((point, index)) => {
                    start_move(conn, ctx, id, state, MoveKind::To(point.map, point.position), (bag, slot, def.special), index, armed)
                        .await?
                }
                None => Done::Failed,
            },
            Effect::MovementRune => match rune_target(ctx, state, id, character.level, request.target) {
                Some(m) => {
                    start_move(
                        conn,
                        ctx,
                        id,
                        state,
                        MoveKind::To(m.map, m.position),
                        (bag, slot, def.special),
                        item_use::RESULT_MOVEMENT_RUNE,
                        armed,
                    )
                    .await?
                }
                None => Done::Failed,
            },
            Effect::PartySummon => {
                let kind = ctx.data.world.create_type(state.map);
                let refused = matches!(kind, create_type::PARTY | create_type::GUILD_RANKING)
                    || state.map == item_use::MAP_NO_PARTY_MOVE
                    || ctx.zones.party_members_placed(id).is_none()
                    || state.item_move.is_some();
                if refused {
                    Done::Failed
                } else {
                    start_move(conn, ctx, id, state, MoveKind::Summon, (bag, slot, def.special), item_use::RESULT_SUMMON, armed).await?
                }
            }
            // Everything equipped back to full quality, for nothing (`docs/original/repair.md` section 5).
            Effect::RepairAll => {
                let used = consume(ctx, character, state, bag, slot).await?;
                save_repair(conn, ctx, character, state, crate::repair::repair_all_free(&state.inventory, &ctx.data.items)).await?;
                conn.send(it::repaired_all(state.money)).await?;
                used
            }
            Effect::TargetResurrection => match request.target {
                Some(target) if ctx.zones.resurrect_by_item(state.map, id, target) => {
                    tracing::info!(id, target, "resurrected by item");
                    Done::UsedUp
                }
                _ => Done::Failed,
            },
            Effect::Fame(vg) => match u16::try_from(vg).ok().filter(|&vg| vg > 0).and_then(|vg| character.honor.checked_add(vg)) {
                Some(honor) => {
                    let used = consume(ctx, character, state, bag, slot).await?;
                    ctx.store.save_honor(id, honor).await?;
                    character.honor = honor;
                    conn.send(w::honor(honor)).await?;
                    used
                }
                None => Done::Failed,
            },
            Effect::Firecracker => Done::UsedUp,
            // Refused while counting is off or a top grade's ten minutes run (`CWorld::AddKillCountGrade`).
            Effect::Blessing => {
                // The check, the item, then the effect (audit R13): counting may freeze while the item is saved, and `add_grade` checks
                // again; the item is then used up for nothing, which a one-in-a-million race can afford.
                if ctx.kill_counts.guard().can_add_grade(country) {
                    let used = consume(ctx, character, state, bag, slot).await?;
                    let changes = ctx.kill_counts.guard().add_grade(country, crate::kc::BLESSING_GRADES, Instant::now());
                    match changes {
                        Some(changes) => {
                            tracing::info!(id, country, "goddess blessing");
                            kill_count_changes(ctx, changes).await;
                        }
                        None => {
                            tracing::warn!(id, country, "goddess blessing refused after the item was used up: counting froze meanwhile")
                        }
                    }
                    used
                } else {
                    Done::Failed
                }
            }
            Effect::Box(set) => open_box(ctx, id, state, set),
            // Refused while one is armed or the all-server cooldown runs (`docs/original/small-items.md` section 5.1).
            Effect::MessageToServer if state.chat_item_armed || state.next_all_chat.is_some_and(|next| now < next) => Done::Failed,
            Effect::MessageToServer => {
                state.chat_item_armed = true;
                Done::UsedUp
            }
            // Once per world entry; the count and the stone go together, and 255 renames already refuse it (deviation 6).
            Effect::NameChange if state.rename_stone_used => Done::Failed,
            Effect::NameChange => match state.inventory.get(bag, slot).cloned() {
                Some(item) if ctx.store.add_rename(id, &used_change(&item)).await? => {
                    state.inventory.apply(&used_change(&item));
                    state.rename_stone_used = true;
                    tracing::info!(id, "rename allowed");
                    Done::Consumed(item)
                }
                _ => Done::Failed,
            },
            Effect::NotBuilt => {
                tracing::debug!(id, item = def.item_id, special = def.special, "item use: not built");
                Done::Failed
            }
            Effect::QuestItem { .. } | Effect::Refused => unreachable!("handled above"),
        }
    };
    match done {
        Done::UsedUp => {
            tracing::info!(id, bag, slot, item = def.item_id, special = def.special, "item used");
            use_up(conn, ctx, character, state, bag, slot).await?;
        }
        Done::UsedUpFor(new) => {
            let made = new.unit();
            tracing::info!(id, bag, slot, item = def.item_id, got = ?(made.item_type, made.type_id), place = ?(made.bag, made.slot),
                craft = %String::from_utf8_lossy(&made.craft_name), "box opened");
            use_up_for(conn, ctx, character, state, bag, slot, new).await?;
        }
        Done::Consumed(item) => {
            tracing::info!(id, bag, slot, item = def.item_id, special = def.special, "item used");
            send_used(conn, ctx, id, state, &item).await?;
        }
        Done::Pending => tracing::info!(id, bag, slot, item = def.item_id, special = def.special, "move by item cast"),
        Done::Failed => {
            state.reuse.restore(armed);
            conn.send(it::use_failed(id)).await?;
        }
    }
    Ok(())
}

/// One of the item at (`bag`, `slot`) used up (`CUser::ItemUseNSend`, section 3): saved, then `0x050A` to the players within 60, or
/// to the player alone when it is not in a zone (after moving to another map).
pub(super) async fn use_up<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    bag: u8,
    slot: u8,
) -> Result<()> {
    match consume(ctx, character, state, bag, slot).await? {
        Done::Consumed(item) => send_used(conn, ctx, character.id, state, &item).await,
        _ => Ok(()),
    }
}

/// `0x050A` for an item `consume` used up, once whatever came between has happened.
async fn send_consumed<S: Backend>(conn: &mut Connection, ctx: &Context<S>, id: u32, state: &WorldState, used: Done) -> Result<()> {
    match used {
        Done::Consumed(item) => send_used(conn, ctx, id, state, &item).await,
        _ => Ok(()),
    }
}

/// One of the item at (`bag`, `slot`) used up and saved with nothing sent (a player leaving the world).
pub(super) async fn use_up_unsent<S: Backend>(
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    bag: u8,
    slot: u8,
) -> Result<()> {
    consume(ctx, character, state, bag, slot).await.map(|_| ())
}

/// One of the item at (`bag`, `slot`) used up and saved, `0x050A` not sent yet: `Done::Consumed`, or `Done::Failed` for an empty
/// slot. An effect that saves something lasting (fame, a reset, a repair) runs after this, so a failed write never leaves both the
/// effect and the item (the original saves the effect first).
pub(super) async fn consume<S: Backend>(
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    bag: u8,
    slot: u8,
) -> Result<Done> {
    let Some(item) = state.inventory.get(bag, slot).cloned() else { return Ok(Done::Failed) };
    let change = used_change(&item);
    let Some(rows) = ctx.store.trade(character.id, 0, std::slice::from_ref(&change)).await? else {
        bail!("the database refused an item use the server allowed: {change:?}");
    };
    // `0x040E` (`CUser::ItemUseNSend`): one of the item used up, named by its row.
    let part = ctx.game_log.item(&ItemUnit { count: 1, ..item.clone() }, rows[0].row);
    ctx.game_log.action(crate::game_log::item_used(record::header(character, state), &part, bag, slot));
    state.inventory.apply(&change);
    Ok(Done::Consumed(item))
}

/// A box used up for the item it gave (`CUser::ItemCreate`, then `ItemUseNSend`; `docs/original/item-origins.md` sections 5.2 and 5.3):
/// both saved in one transaction, then `0x0205` with the new item, then `0x050A`.
async fn use_up_for<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    bag: u8,
    slot: u8,
    new: NewItem,
) -> Result<()> {
    let id = character.id;
    let Some(item) = state.inventory.get(bag, slot).cloned() else { return Ok(()) };
    let made = new.unit();
    let changes = [SlotChange::Insert(new), used_change(&item)];
    let Some(rows) = ctx.store.trade(id, 0, &changes).await? else {
        bail!("the database refused a box the server allowed: {changes:?}");
    };
    for change in &changes {
        state.inventory.apply(change);
    }
    // `0x040E` for the box and `0x0401` (`'ItemCreate'`) for the item it gave: a new item always has the record that made it.
    let header = record::header(character, state);
    let box_part = ctx.game_log.item(&ItemUnit { count: 1, ..item.clone() }, rows[1].row);
    ctx.game_log.action(crate::game_log::item_used(header.clone(), &box_part, bag, slot));
    let made_part = ctx.game_log.item(&made, rows[0].row);
    ctx.game_log.action(crate::game_log::item_created(header, &made_part, made.count));
    conn.send(gp::picked_up(&made)).await?;
    send_used(conn, ctx, id, state, &item).await
}

/// One fewer of `item`: its count lowered, or the slot emptied.
fn used_change(item: &ItemUnit) -> SlotChange {
    let (bag, slot) = (item.bag, item.slot);
    match item.count.saturating_sub(1) {
        0 => SlotChange::Remove { bag, slot },
        left => SlotChange::SetCount { bag, slot, count: left },
    }
}

/// `0x050A` for one of `item` used.
async fn send_used<S: Backend>(conn: &mut Connection, ctx: &Context<S>, id: u32, state: &WorldState, item: &ItemUnit) -> Result<()> {
    let left = item.count.saturating_sub(1);
    let (bag, slot) = (item.bag, item.slot);
    let packet = it::used(id, bag, slot, item.item_type, item.type_id, left);
    if state.entered { conn.send(ctx.zones.send_near(state.map, id, packet, ITEM_RANGE)).await } else { conn.send(packet).await }
}

/// What a box of set `set` gives (`docs/original/item-origins.md` sections 5.2 and 5.3): nothing for grade 0, else a new item of make
/// type `'S'` with random craft stats in the first empty slot (no stacking); the error answer for no set, no item or no room.
fn open_box<S>(ctx: &Context<S>, id: u32, state: &WorldState, set: i16) -> Done {
    use rand::Rng;
    let mut rng = rand::rng();
    let mut rand = || rng.random::<u32>() & 0x7fff;
    let (item_type, type_id) = match ctx.data.live().drops.open_box(set, &mut rand) {
        BoxRoll::Item(item_type, type_id) => (item_type, type_id),
        BoxRoll::Nothing => {
            tracing::info!(id, set, "box gave nothing");
            return Done::UsedUp;
        }
        roll => {
            tracing::debug!(id, set, ?roll, "box refused");
            return Done::Failed;
        }
    };
    let Some(def) = ctx.data.items.get(item_type, type_id) else { return Done::Failed };
    let Some(&(bag, slot)) = state.inventory.free_slots().first() else {
        tracing::debug!(id, set, "box refused: no empty slot");
        return Done::Failed;
    };
    let kept = Kept { make_type: BOX_MAKE_TYPE, craft_name: crate::lapis::new_craft_name(def, &mut rand), ..Kept::default() };
    Done::UsedUpFor(NewItem { bag, slot, item_id: def.item_id, item_type, type_id, quality: def.quality, count: 1, kept: Some(kept) })
}

/// HP, SP and MP added: `0x0505` when something changed; the item is used up either way.
async fn recovered(conn: &mut Connection, packet: Option<Vec<u8>>) -> Result<Done> {
    if let Some(packet) = packet {
        conn.send(packet).await?;
    }
    Ok(Done::UsedUp)
}

/// An item's buff on the player itself (`CUser::UseItemSkill`, section 7): buffs only, with the refusals of the item flags, the
/// charms and higher levels; then the item at `at` used up (saved), then as a buff skill on oneself (`CUser::BuffUser(self, 1)`:
/// `0x0511`, the skill applied). The item goes first, so a failed write never leaves both the buff and the item (audit R13).
#[allow(clippy::too_many_arguments)]
async fn item_skill<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &mut PlayCharacter,
    state: &mut WorldState,
    country: u8,
    skill_id: u16,
    level: u8,
    at: (u8, u8),
) -> Result<Done> {
    let (id, map) = (character.id, state.map);
    let Some(def) = ctx.data.skills.get(skill_id, level).copied() else { return Ok(Done::Failed) };
    if def.type_effect != skills::BUFF || ctx.zones.item_skill_refused(map, id, &def) {
        tracing::debug!(id, skill_id, level, "item skill refused");
        return Ok(Done::Failed);
    }
    let used = consume(ctx, character, state, at.0, at.1).await?;
    buff_self(conn, ctx, character, state, country, &def).await?;
    Ok(used)
}

/// A buff skill applied to the character itself (`CUser::BuffUser(self, 1)`): `0x0511`, the skill applied. The item path's second half
/// and the scripts' `add_buff`; the caller has checked that it is a buff and not refused.
pub(super) async fn buff_self<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &mut PlayCharacter,
    state: &mut WorldState,
    country: u8,
    def: &SkillDef,
) -> Result<()> {
    let (id, map) = (character.id, state.map);
    let gear = gear(ctx, state);
    let fighter = combat::player(character.level, state.totals.stats, &gear);
    let cast = SkillCast {
        caster: id,
        country,
        skill: def,
        fighter: &fighter,
        rec: state.totals.stats[2],
        cross_road: gear.cross_road,
        cost_reduction: state.passives.bonuses.cost_reduction,
        fighting: state.prev_mob(),
        look: None,
        illusion_mob: 0,
        war_map: ctx.data.world.is_war_map(map),
    };
    let outcome = {
        use rand::Rng;
        let mut rng = rand::rng();
        let live = ctx.data.live();
        let loot = (&live.drops, &ctx.data.items);
        ctx.zones.use_skill(map, SkillTarget::Player(id), &cast, &live.mobs, loot, Instant::now(), &mut || rng.random::<u32>() & 0x7fff)
    };
    // Nothing to apply when the player is not in a zone any more (it cannot happen on this task); the item is used.
    let Some(outcome) = outcome else { return Ok(()) };
    for packet in outcome.packets {
        conn.send(packet).await?;
    }
    if let Some(effects) = ctx.zones.buff_effects(map, id) {
        buffs_changed(conn, ctx, character, state, effects).await?;
    }
    Ok(())
}

/// A recall rune's destination (section 9): its level cap, the map loaded, of no `CreateType`, the position on it, and no logout
/// pending. (The portal check of `EnableMoveByPortal` concerns portals linked to obelisks, which do not exist yet: every map is
/// open.)
fn recall_allowed<S>(ctx: &Context<S>, state: &WorldState, level: u16, to_map: u16, position: [f32; 3]) -> bool {
    item_use::level_allows(to_map, level)
        && ctx.data.world.is_safe_pos(to_map, position)
        && ctx.data.world.create_type(to_map) == 0
        && state.logout.is_none()
}

/// The saved position a Teleportation Stone goes to (`docs/original/item-windows.md` section 1), and the result byte (its index 1 to
/// 4): a set slot, with the recall runes' checks of the destination.
fn teleport_target<S>(ctx: &Context<S>, character: &PlayCharacter, state: &WorldState, target: Option<u32>) -> Option<(SavePoint, u8)> {
    let index = target.and_then(|t| u8::try_from(t).ok()).filter(|i| (1..=4).contains(i))?;
    let point = *character.save_points.iter().find(|p| p.slot == index - 1)?;
    recall_allowed(ctx, state, character.level, point.map, point.position).then_some((point, index))
}

/// An experience wheel (`docs/original/item-windows.md` section 3): at least halfway through the level, on map 18 or 30, its buff
/// applied; then the experience back to the level's start total, `0x0207` with the difference (negative, as `u32`), saved. The item is
/// used up before either (audit R13).
#[allow(clippy::too_many_arguments)]
async fn exp_wheel<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &mut PlayCharacter,
    state: &mut WorldState,
    country: u8,
    skill_id: u16,
    level: u8,
    at: (u8, u8),
) -> Result<Done> {
    let (start, end) = ctx.data.exp.bounds(character.grow, character.level);
    if !item_use::wheel_allows(character.exp, start, end) || !item_use::EXP_WHEEL_MAPS.contains(&state.map) {
        return Ok(Done::Failed);
    }
    let done = item_skill(conn, ctx, character, state, country, skill_id, level, at).await?;
    if matches!(done, Done::Consumed(_)) {
        conn.send(cp::exp_gain(start.wrapping_sub(character.exp), 0)).await?;
        tracing::info!(id = character.id, from = character.exp, to = start, "experience wheel");
        character.exp = start;
        let health = ctx.zones.health(state.map, character.id).unwrap_or(state.health);
        ctx.store.save_progress(character.id, &progress(character), health).await?;
    }
    Ok(done)
}

/// An item's skill on another player (kind 1) or a monster (kind 2) (`docs/original/small-items.md` section 11, deviation 7): only on
/// a target the user may attack with a skill (a player without a personal shop) and within the skill's range, as for a skill; then
/// one hit, and the item is used up whatever it did.
async fn item_skill_on<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    country: u8,
    (skill_id, level): (u16, u8),
    (kind, target): (u8, u32),
) -> Result<Done> {
    let (id, map, now) = (character.id, state.map, Instant::now());
    let Some(def) = ctx.data.skills.get(skill_id, level).copied() else { return Ok(Done::Failed) };
    let verdict =
        if kind == it::KIND_PLAYER { Some(ctx.zones.attack_verdict(map, id, target, ctx.data.world.is_war_map(map))) } else { None };
    if let Some(crate::zone::Verdict::Ruled(refusal)) = &verdict {
        tracing::debug!(id, map, point = "attack", rule = &*refusal.name, "refused by a rule");
        super::refused::tell_refused(conn, state, refusal).await?;
    }
    let target = match kind {
        it::KIND_PLAYER if verdict.is_some_and(|v| v.is_yes()) && !ctx.zones.has_shop(map, target) => SkillTarget::Player(target),
        // Whether it may be attacked is `target_spot`'s check.
        it::KIND_MOB => SkillTarget::Mob(target),
        _ => {
            tracing::debug!(id, skill_id, kind, target, "item skill refused: not a target to attack");
            return Ok(Done::Failed);
        }
    };
    let gear = gear(ctx, state);
    let in_range = target_spot(ctx, state, country, target, false).is_some_and(|(spot, size)| {
        let slack = combat::range_slack(state.totals.speeds().1, state.last_move.map(|at| now - at));
        combat::in_range(crate::zone::distance(state.position, spot), slack, size, gear.range + i32::from(def.attack_range))
    });
    if !in_range {
        tracing::debug!(id, skill_id, ?target, "item skill refused: not attackable or out of range");
        return Ok(Done::Failed);
    }
    let fighter = combat::player(character.level, state.totals.stats, &gear);
    let cast = SkillCast {
        caster: id,
        country,
        skill: &def,
        fighter: &fighter,
        rec: state.totals.stats[2],
        cross_road: gear.cross_road,
        cost_reduction: state.passives.bonuses.cost_reduction,
        fighting: state.prev_mob(),
        look: None,
        illusion_mob: 0,
        war_map: ctx.data.world.is_war_map(map),
    };
    let outcome = {
        use rand::Rng;
        let mut rng = rand::rng();
        let live = ctx.data.live();
        let loot = (&live.drops, &ctx.data.items);
        ctx.zones.item_skill_hit(map, target, &cast, &live.mobs, loot, now, &mut || rng.random::<u32>() & 0x7fff)
    };
    let Some(outcome) = outcome else { return Ok(Done::Failed) };
    tracing::info!(id, skill_id, level, ?target, killed = outcome.killed, "item skill on a target");
    for packet in outcome.packets {
        conn.send(packet).await?;
    }
    if outcome.died {
        died(ctx, state, id);
    }
    Ok(Done::UsedUp)
}

/// The party member a movement rune goes to (section 9): another member, its map within the level cap, not a party or guild ranking
/// map, a guild house only within the same guild, not map 67; no move pending. A logout pending refuses it too (deviation).
fn rune_target<S>(ctx: &Context<S>, state: &WorldState, id: u32, level: u16, target: Option<u32>) -> Option<Member> {
    let target = target.filter(|&t| t != 0 && t != id)?;
    let members = ctx.zones.party_members_placed(id)?;
    let me = members.iter().find(|m| m.id == id)?;
    let m = *members.iter().find(|m| m.id == target)?;
    let kind = ctx.data.world.create_type(m.map);
    let allowed = item_use::level_allows(m.map, level)
        && !matches!(kind, create_type::PARTY | create_type::GUILD_RANKING)
        && if kind == create_type::GUILD_HOUSE {
            m.guild_id != 0 && m.guild_id == me.guild_id
        } else {
            m.map != item_use::MAP_NO_PARTY_MOVE
        }
        && state.item_move.is_none()
        && state.logout.is_none();
    allowed.then_some(m)
}

/// The casting of a move by item starts (section 9): due in 5 s, `0x0221` to the players within 60.
#[allow(clippy::too_many_arguments)]
async fn start_move<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    id: u32,
    state: &mut WorldState,
    kind: MoveKind,
    (bag, slot, special): (u8, u8, u8),
    result: u8,
    armed: Armed,
) -> Result<Done> {
    let by_item = ByItem { kind, bag, slot, special, result, armed };
    state.item_move = Some(ItemMove { due: Instant::now() + item_use::MOVE_CAST, by_item: Some(by_item) });
    conn.send(ctx.zones.send_near(state.map, id, it::move_cast(id), ITEM_RANGE)).await?;
    Ok(Done::Pending)
}

/// A pending move by item cancelled (`CUser::CancleMove`: a step, an attack or skill, a hit that breaks a cast): the reuse group is
/// ready again and `0x0222` with 0 goes to the players within 60; returns it for the player. `/town` is dropped without a word.
pub(super) fn cancel_item_move<S>(ctx: &Context<S>, state: &mut WorldState, id: u32) -> Option<Vec<u8>> {
    let Some(m) = state.item_move.take()?.by_item else {
        tracing::debug!(id, "town move cancelled");
        return None;
    };
    state.reuse.clear(m.armed);
    tracing::debug!(id, "move by item cancelled");
    Some(ctx.zones.send_near(state.map, id, it::move_end(id, 0), ITEM_RANGE))
}

/// A move by item is due (`CUser::UpdateResetPosition`, section 9): with the item still in its place, the move (or the summon's
/// questions) and the item used up; otherwise `0x0222` with 0. `/town` goes to the bind position of the map the player is on now,
/// as the Return skill, with no logout or death check (`docs/original/small-items.md` section 6).
pub(super) async fn finish_item_move<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    country: u8,
) -> Result<()> {
    let Some(m) = state.item_move.take() else { return Ok(()) };
    let (id, map) = (character.id, state.map);
    if !state.entered {
        return Ok(());
    }
    let Some(m) = m.by_item else {
        let (to_map, position) = {
            use rand::Rng;
            let mut rng = rand::rng();
            ctx.data.world.bind_pos(map, country, state.position, &mut || rng.random::<u32>() & 0x7fff)
        };
        tracing::info!(id, from = map, to = to_map, ?position, "town");
        return move_player(conn, ctx, state, id, to_map, position, None).await;
    };
    let still_there = state
        .inventory
        .get(m.bag, m.slot)
        .and_then(|item| ctx.data.items.get(item.item_type, item.type_id))
        .is_some_and(|def| def.special == m.special);
    let fail = |state: &mut WorldState| {
        state.reuse.clear(m.armed);
        ctx.zones.send_near(map, id, it::move_end(id, 0), ITEM_RANGE)
    };
    if !still_there {
        return conn.send(fail(state)).await;
    }
    match m.kind {
        MoveKind::To(to_map, position) => {
            if !ctx.data.world.is_safe_pos(to_map, position) {
                return conn.send(fail(state)).await;
            }
            // The item first (audit R13): a trade, a duel stakes window or a shop that held it ends, the item is saved as used, then
            // the move happens and `0x050A` follows it.
            ctx.zones.cancel_actions(id);
            let used = consume(ctx, character, state, m.bag, m.slot).await?;
            conn.send(ctx.zones.send_near(map, id, it::move_end(id, m.result), ITEM_RANGE)).await?;
            tracing::info!(id, from = map, to = to_map, ?position, "moved by item");
            move_player(conn, ctx, state, id, to_map, position, None).await?;
            send_consumed(conn, ctx, id, state, used).await
        }
        MoveKind::GuildHouse(to_map, position) => {
            // The guild may have lost its house in the 5 s: then the move fails (deviation 5).
            let Ok(zone) = ctx.zones.enter_guild_house(to_map, id, map, &ctx.data.world, &ctx.data.live().mobs) else {
                return conn.send(fail(state)).await;
            };
            ctx.zones.cancel_actions(id);
            let used = consume(ctx, character, state, m.bag, m.slot).await?;
            conn.send(ctx.zones.send_near(map, id, it::move_end(id, m.result), ITEM_RANGE)).await?;
            tracing::info!(id, from = map, to = zone, ?position, "moved into the guild house by item");
            move_player(conn, ctx, state, id, zone, position, None).await?;
            send_consumed(conn, ctx, id, state, used).await
        }
        MoveKind::Summon => {
            let now = Instant::now();
            let me = ctx.zones.locate(id);
            let asked = ctx
                .zones
                .party_members_placed(id)
                .unwrap_or_default()
                .into_iter()
                .filter(|member| member.id != id && !member.dead && item_use::level_allows(map, member.level))
                // On a guild or guild house map, only the summoner's guild (`CUser::UpdateResetPosition` type 4).
                .filter(|member| {
                    !matches!(ctx.data.world.create_type(map), create_type::GUILD | create_type::GUILD_HOUSE)
                        || me.is_some_and(|me| me.guild_id == member.guild_id)
                })
                .filter(|member| ctx.zones.ask_party_call(member, id, now))
                .count();
            if asked == 0 {
                return conn.send(fail(state)).await;
            }
            tracing::info!(id, asked, "party members summoned");
            use_up(conn, ctx, character, state, m.bag, m.slot).await
        }
    }
}

/// `0x0224`: the answer to a summon (`CUser::PacketMaininterface`, section 9). The summoner hears it when it is still in the world
/// on a map that allows it; with yes, the player goes to it at once, dead or alive, as in the original.
pub(super) async fn answer_party_call<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    answer: u8,
) -> Result<()> {
    let id = character.id;
    let Some(summoner) = ctx.zones.take_party_call(id, None) else { return Ok(()) };
    state.party_call_due = None;
    let Some(s) = ctx.zones.locate(summoner) else { return Ok(()) };
    let kind = ctx.data.world.create_type(s.map);
    let my_guild = ctx.zones.locate(id).map_or(0, |me| me.guild_id);
    let allowed = !matches!(kind, create_type::PARTY | create_type::GUILD_RANKING)
        && (kind != create_type::GUILD_HOUSE || (s.guild_id != 0 && s.guild_id == my_guild));
    if !allowed {
        return Ok(());
    }
    ctx.zones.send_to(s.map, summoner, &it::party_call_answer(id, answer));
    if answer == 0 && item_use::level_allows(s.map, character.level) && state.logout.is_none() {
        tracing::info!(id, summoner, to = s.map, position = ?s.position, "summoned");
        move_player(conn, ctx, state, id, s.map, s.position, None).await?;
    }
    Ok(())
}

/// No answer in time (`CUser::UpdateTimeover`): the summoner gets `0x0224` with 1.
pub(super) fn party_call_ran_out<S>(ctx: &Context<S>, state: &mut WorldState, id: u32) {
    state.party_call_due = None;
    if let Some(summoner) = ctx.zones.take_party_call(id, Some(Instant::now()))
        && let Some(s) = ctx.zones.locate(summoner)
    {
        ctx.zones.send_to(s.map, summoner, &it::party_call_answer(id, 1));
    }
}

/// A stat reset (`CUser::StatResetStatus`, section 8): attacking stops, the protections go, the base stats and points as for a
/// character that never spent any, saved; `0x0214`, then what changed (speeds, maxima) and the current values (`0x0521`).
pub(super) async fn reset_stats<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &mut PlayCharacter,
    state: &mut WorldState,
) -> Result<()> {
    let (id, map) = (character.id, state.map);
    if state.target.is_some() {
        stop_attacking(ctx, state, id);
    }
    ctx.zones.clear_protections(map, id);
    let (stat_point, stats) = item_use::reset_stats(character.job, character.family, character.grow, character.level);
    let progress = Progress { stat_point, stats, ..progress(character) };
    let health = ctx.zones.health(map, id).unwrap_or(state.health);
    // The kill rewards can be taken again from the first step (`docs/original/pvp.md` section 8); saved with the points (audit R12).
    let steps = [0, character.kill_steps[1]];
    ctx.store.save_stat_reset(id, &progress, health, steps).await?;
    character.stat_point = stat_point;
    character.stats = stats;
    character.kill_steps = steps;
    tracing::info!(id, ?stats, stat_point, "stats reset");
    conn.send(it::stats_reset(stat_point, stats)).await?;
    renumber(conn, ctx, character, state).await?;
    conn.send(cp::recover_set(ctx.zones.health(map, id).unwrap_or(state.health))).await
}

/// A skill reset (`CUser::StatResetSkill`, section 8): attacking stops, the protections go, every learnt skill is forgotten and the
/// points are those of the level, saved; `0x0215`, then what the passives' loss changed and the current values. `free` is the
/// free reset (`0x0215` from the client).
pub(super) async fn reset_skills<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &mut PlayCharacter,
    state: &mut WorldState,
    free: bool,
) -> Result<()> {
    let (id, map) = (character.id, state.map);
    if state.target.is_some() {
        stop_attacking(ctx, state, id);
    }
    ctx.zones.clear_protections(map, id);
    let points = item_use::reset_skill_points(character.grow, character.level);
    ctx.store.reset_skills(id, points, free).await?;
    character.skills.clear();
    character.skill_point = points;
    character.free_skill_reset_used |= free;
    state.cooldowns.clear();
    state.skill_passives = skills::passives(&character.skills, &ctx.data.skills);
    state.passives = with_buffs(state.skill_passives, &state.effects);
    state.inventory.set_max_bag(state.passives.bags.unwrap_or(crate::inventory::DEFAULT_MAX_BAG));
    ctx.zones.set_low_hp(map, id, None);
    tracing::info!(id, points, free, "skills reset");
    conn.send(it::skills_reset(points, free)).await?;
    renumber(conn, ctx, character, state).await?;
    conn.send(cp::recover_set(ctx.zones.health(map, id).unwrap_or(state.health))).await
}

/// NPC type of the blacksmiths, who repair (`docs/original/maps-npcs.md` section 3).
pub(super) const BLACKSMITH: u8 = 3;

/// `0x0554` (`CUser::PacketPC`, `CUser::ItemRepair`; `docs/original/repair.md` sections 4 and 5): one item or everything equipped
/// back to full quality for its price. Ignored while dead, without a blacksmith in view (deviation 1), and when the original does
/// nothing.
pub(super) async fn repair_items<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    bag: u8,
    slot: u8,
) -> Result<()> {
    let id = character.id;
    if state.dead {
        return Ok(());
    }
    ctx.zones.cancel_actions(id);
    if !ctx.zones.npc_kind_in_view(state.map, id, BLACKSMITH) {
        tracing::debug!(id, bag, slot, "repair without a blacksmith in view ignored");
        return Ok(());
    }
    // In a guild house the blacksmith's rate, or its refusal (`docs/original/guild-house.md` section 9).
    let rate = match super::guild_house::repair_rate(ctx, state.map, id) {
        Ok(rate) => rate,
        Err(refused) => return conn.send(refused).await,
    };
    let Some(repair) = crate::repair::repair(&state.inventory, &ctx.data.items, state.money, bag, slot, rate) else {
        tracing::debug!(id, bag, slot, money = state.money, "repair ignored");
        return Ok(());
    };
    tracing::debug!(id, bag, slot, cost = repair.cost, "repaired");
    save_repair(conn, ctx, character, state, repair).await?;
    conn.send(it::repaired(bag, slot, state.money)).await
}

/// A repair saved with the wear not yet saved, money and qualities together; a changed wear level works the numbers out again.
async fn save_repair<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    repair: crate::repair::Repair,
) -> Result<()> {
    let mut changes = take_unsaved_wear(state);
    changes.extend(repair.changes);
    if ctx.store.trade(character.id, -i64::from(repair.cost), &changes).await?.is_none() {
        bail!("the database refused a repair the server allowed: {changes:?}");
    }
    state.money -= repair.cost;
    for change in &changes {
        state.inventory.apply(change);
    }
    if repair.renumber {
        renumber(conn, ctx, character, state).await?;
    }
    Ok(())
}

/// `0x0220` (`CUser::PacketMaininterface`, `docs/original/item-windows.md` section 1): a saved position, refused in a dungeon (1), with
/// an index not 1 to 4 (2), away from where the player stands (1) or on a map of another create type (2). The position kept is the
/// server's (deviation 2).
pub(super) async fn save_point<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &mut PlayCharacter,
    state: &WorldState,
    request: it::SavePos,
) -> Result<()> {
    let map = state.map;
    let refused = |result| it::save_point_result(result, it::SavePos { index: request.index, map: 0, position: [0.0; 3] });
    let [x, _, z] = request.position;
    let gap = ((x - state.position[0]).powi(2) + (z - state.position[2]).powi(2)).sqrt();
    let result = if ctx.data.world.is_dungeon(map) {
        1
    } else if !(1..=4).contains(&request.index) {
        2
    } else if request.map != crate::world::map_of(map) || gap.is_nan() || gap > SAVE_POINT_RANGE {
        1
    } else if ctx.data.world.create_type(map) != 0 {
        2
    } else {
        0
    };
    if result != 0 {
        tracing::debug!(id = character.id, ?request, result, "save point refused");
        return conn.send(refused(result)).await;
    }
    let point = SavePoint { slot: request.index - 1, map, position: state.position };
    ctx.store.save_point(character.id, point).await?;
    character.save_points.retain(|p| p.slot != point.slot);
    character.save_points.push(point);
    character.save_points.sort_by_key(|p| p.slot);
    tracing::info!(id = character.id, index = request.index, map, position = ?state.position, "position saved");
    conn.send(it::save_point_result(0, it::SavePos { index: request.index, map, position: state.position })).await
}

/// How far from the player a saved position may be (`SVector::Distance2` against 50).
const SAVE_POINT_RANGE: f32 = 50.0;
/// Item specials of the scrolls (`docs/original/item-windows.md` section 2), and of the equipment that refuses a sex change.
const APPEARANCE_CHANGE: u8 = 44;
const SEX_CHANGE: u8 = 45;
const SEX_BOUND: [u8; 2] = [64, 65];
/// The usable items' real type (`LITEM_TYPE` 26).
const REAL_TYPE_USABLE: u8 = 26;

/// `0x0226` (`docs/original/item-windows.md` section 2): hair, face and size (special 44), or those and the sex (45, refused with
/// `0x0228` while sex-bound equipment is worn), saved with the scroll used up; `0x0226` to the view. A sex other than 0 or 1 is
/// refused (deviation 1).
pub(super) async fn change_body<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &mut PlayCharacter,
    state: &mut WorldState,
    (bag, slot, looks): (u8, u8, [u8; 4]),
) -> Result<()> {
    let id = character.id;
    if state.dead || bag == 0 || !state.inventory.is_bag((bag, slot)) {
        return Ok(());
    }
    // As every item action: a trade, a duel stakes window or a shop that held the item ends (audit R3).
    ctx.zones.cancel_actions(id);
    let Some(item) = state.inventory.get(bag, slot).cloned() else { return Ok(()) };
    let Some(def) = ctx.data.items.get(item.item_type, item.type_id).copied() else { return Ok(()) };
    // TODO(client-research-queue.md entry 13 (a)): clamp hair, face and size once the ranges are known; stored as sent until then.
    let [hair, face, size, sex] = looks;
    let body = match def.special {
        _ if real_type(def.item_type) != REAL_TYPE_USABLE => None,
        APPEARANCE_CHANGE => Some(Body { hair, face, size, sex: character.sex }),
        SEX_CHANGE => {
            let bound = (0..13u8).any(|s| {
                state
                    .inventory
                    .get(0, s)
                    .and_then(|i| ctx.data.items.get(i.item_type, i.type_id))
                    .is_some_and(|d| SEX_BOUND.contains(&d.special))
            });
            if bound {
                tracing::debug!(id, "sex change refused: sex-bound equipment worn");
                return conn.send(it::body_change_failed()).await;
            }
            Some(Body { hair, face, size, sex })
        }
        _ => None,
    };
    let Some(body) = body.filter(|b| b.sex <= 1) else {
        tracing::debug!(id, bag, slot, special = def.special, ?looks, "body change refused");
        return conn.send(it::use_failed(id)).await;
    };
    if !ctx.store.change_body(id, body, (bag, slot), item.count).await? {
        bail!("the database refused a body change the server allowed");
    }
    let left = item.count - 1;
    state.inventory.apply(&if left == 0 { SlotChange::Remove { bag, slot } } else { SlotChange::SetCount { bag, slot, count: left } });
    (character.hair, character.face, character.size, character.sex) = (body.hair, body.face, body.size, body.sex);
    conn.send(ctx.zones.send_near(state.map, id, it::used(id, bag, slot, item.item_type, item.type_id, left), ITEM_RANGE)).await?;
    tracing::info!(id, ?body, "body changed");
    conn.send(ctx.zones.change_body(state.map, id, body)).await
}
