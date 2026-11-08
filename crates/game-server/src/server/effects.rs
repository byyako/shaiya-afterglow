//! The item effects of the extensions at an item's use (`docs/plans/ext-3-effects.md` sections 3.3 and 5.1): the handler `use_item`
//! calls for an item with an effect of the `use` request. The registry and the pure parts are `crate::effects`'s; a handler here
//! answers with the `Done` of `use_item` and whispers a refusal under its effect type's id (at most once in ten seconds).

use super::items::{Done, consume};
use super::*;
use crate::effects::{ItemEffect, LevelUp};
use crate::rules::Refusal;

/// Carries out the `use` effect of the item `item_id` for the character: `Done::Failed` for an item with none (it cannot happen,
/// `Effect::Ext` is only chosen for one), and for an effect that acts in another request.
pub(super) async fn use_effect<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &mut PlayCharacter,
    state: &mut WorldState,
    item_id: u32,
    at: (u8, u8),
    country: u8,
) -> Result<Done> {
    let Some(effect) = ctx.data.effects.for_use(item_id).cloned() else { return Ok(Done::Failed) };
    match &*effect {
        ItemEffect::LevelUp(stone) => level_up(conn, ctx, character, state, &effect, stone, item_id, at).await,
        ItemEffect::PetEgg(egg) => super::pets::use_pet_egg(conn, ctx, character, state, country, &effect, egg, at).await,
        other => {
            tracing::debug!(item = item_id, effect = other.type_name(), "item use: an effect of another request");
            Ok(Done::Failed)
        }
    }
}

/// Whispers `text` as the refusal of `effect`'s type, within the limit of `refused::tell_refused`.
pub(super) async fn refuse(conn: &mut Connection, state: &mut WorldState, effect: &ItemEffect, text: &str) -> Result<()> {
    let refusal = Refusal { rule: effect.refusal_id(), name: Arc::from(effect.type_name()), text: Some(Arc::from(text)) };
    super::refused::tell_refused(conn, state, &refusal).await
}

/// The level stone (section 5.1). The target level is the stone's, cut by its `max` and the mode's cap; not above the level, the
/// stone is refused and kept. Otherwise the item is used up and saved first (audit R13), the experience becomes the start of the target
/// level through `combat::add_exp` (so the stat and skill points of every level come as from levelling), saved with the level, one
/// `0x0308` record per level, `0x0207` with the gain and the level-up packets as for a level not made by the player's own kill. The
/// answer is `Done::Consumed`, so `use_item` sends `0x050A` last. A level is not experience: the rules' and the scripts' `exp` points
/// and the item flags' factor are not asked.
#[allow(clippy::too_many_arguments)]
async fn level_up<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &mut PlayCharacter,
    state: &mut WorldState,
    effect: &ItemEffect,
    stone: &LevelUp,
    item_id: u32,
    at: (u8, u8),
) -> Result<Done> {
    let (id, from) = (character.id, character.level);
    let Some(target) = crate::effects::level_target(from, character.grow, stone) else {
        refuse(conn, state, effect, "This item cannot raise your level any further.").await?;
        return Ok(Done::Failed);
    };
    let used = consume(ctx, character, state, at.0, at.1).await?;
    let before = Progress {
        level: character.level,
        exp: character.exp,
        stat_point: character.stat_point,
        skill_point: character.skill_point,
        stats: character.stats,
    };
    // The start of the target level; progress inside the current level is not carried over (as the GM's `/set level`).
    let start = ctx.data.exp.total(character.grow, target - 1);
    let gain = start.saturating_sub(before.exp).max(1);
    let Some((after, levels)) = combat::add_exp(before, character.job, character.grow, gain, &ctx.data.exp) else { return Ok(used) };
    character.level = after.level;
    character.exp = after.exp;
    character.stat_point = after.stat_point;
    character.skill_point = after.skill_point;
    character.stats = after.stats;
    let health = ctx.zones.health(state.map, id).unwrap_or(state.health);
    ctx.store.save_progress(id, &after, health).await?;
    // `0x0308` (`CUser::LevelUp`): one record per level gained, each at its own level.
    for step in 1..=levels {
        let mut header = record::header(character, state);
        header.level = before.level + step;
        ctx.game_log.action(crate::game_log::level_up(header));
    }
    tracing::info!(id, item = item_id, from, to = after.level, "level by item");
    let gained = after.exp.saturating_sub(before.exp);
    if gained > 0 {
        conn.send(cp::exp_gain(gained, 0)).await?;
    }
    if levels > 0 {
        announce_level(conn, ctx, character, state, true).await?;
    }
    Ok(used)
}
