//! The game log's records beside the actions that make them (`docs/original/game-log.md` section 5). The game calls these where
//! the original's senders sit; each writes through `Context::game_log`, which does nothing when the log is off.

use super::*;
use crate::game_log::{self, Header, ItemPart};
use crate::zone::Dropped;

/// The header of the character as it is now (`CUser::SetGameLogMain`): its map and position are the world state's.
pub(super) fn header(character: &PlayCharacter, state: &WorldState) -> Header {
    Header::of(character, state.map, state.position)
}

/// A ground item picked up (section 3.12): by its drop type, `0x0401` for a monster's drop or a player's own, `0x0408` for one
/// dropped at a death. `count` is how many went into the bags by this change.
pub(super) fn picked_up<S>(ctx: &Context<S>, character: &PlayCharacter, state: &WorldState, dropped: Dropped, item: &ItemPart, count: u8) {
    let h = header(character, state);
    match dropped.kind {
        1 | 2 => ctx.game_log.action(game_log::item_get(h, item, count)),
        3 => {
            let name = ctx.zones.name_of(dropped.by).unwrap_or_default();
            ctx.game_log.action(game_log::death_pickup(h, item, dropped.by, &name));
        }
        _ => {}
    }
}

/// Gold picked up or shared (`0x0801`, always, whatever its drop type): the amount and the money after.
pub(super) fn gold_picked_up<S>(ctx: &Context<S>, character: &PlayCharacter, state: &WorldState, amount: u32) {
    ctx.game_log.action(game_log::money_get(header(character, state), amount, state.money));
}

/// The item in a slot as the records name it, with the row a write left for it.
pub(super) fn item_in<S>(
    ctx: &Context<S>,
    state: &WorldState,
    place: (u8, u8),
    stored: Option<crate::backend::Stored>,
) -> Option<ItemPart> {
    state.inventory.get(place.0, place.1).map(|unit| ctx.game_log.item(unit, stored))
}

/// A saved exchange as this side saw it (`CUser::Exchange`, `ExchangeItemAdd`): the gold it put in (`0x0804`) and got (`0x0803`), and
/// each item it gave (`0x0406`, from the place it left) and got (`0x0405`). The giving side's records come first (section 8). The
/// other side's address has no source here (NULL).
pub(super) fn traded<S>(ctx: &Context<S>, character: &PlayCharacter, state: &WorldState, settled: &crate::trade::Settled) {
    let other = settled.partner;
    let name = ctx.zones.name_of(other).unwrap_or_default();
    let h = header(character, state);
    let money_after = u32::try_from(i64::from(state.money) + settled.money).unwrap_or(state.money);
    if settled.sent > 0 {
        ctx.game_log.action(game_log::money_give(h.clone(), settled.sent, money_after, other, &name));
    }
    if settled.got > 0 {
        ctx.game_log.action(game_log::money_receive(h.clone(), settled.got, money_after, other, &name, None));
    }
    for gave in [true, false] {
        for t in settled.traded.iter().filter(|t| t.gave == gave) {
            let part = ctx.game_log.item(&t.item, t.row);
            ctx.game_log.action(if gave {
                game_log::trade_give(h.clone(), &part, other, t.item.count, 0, &name, t.item.bag, t.item.slot)
            } else {
                game_log::trade_receive(h.clone(), &part, other, t.item.count, 0, &name)
            });
        }
    }
}

/// What `0x0306` and `0x0307` say of the character now. The warehouse holds no gold here (`bank` 0); `cash` is the account's points
/// as the connection knows them.
fn summary(character: &PlayCharacter, state: &WorldState) -> game_log::Summary {
    game_log::Summary {
        money: state.money,
        bank: 0,
        cash: state.mall.points,
        level: character.level,
        skill_points: character.skill_point,
        stat_points: character.stat_point,
        stats: character.stats,
    }
}

/// The character enters the world (`CUser::EnterWorld`): `0x0306`, then `0x030F`. The billing type is the premium kind in effect; the
/// billing time has no source (NULL); the address is the client's. `0x030F` is written whatever the move position type (not kept).
pub(super) fn entered<S>(ctx: &Context<S>, character: &PlayCharacter, state: &WorldState, ip: std::net::IpAddr) {
    let billing = character.mall.premium.kind_at(chrono::Utc::now().timestamp());
    ctx.game_log.action(game_log::enter_world(header(character, state), billing, &ip.to_string(), &summary(character, state)));
    ctx.game_log.action(game_log::map_enter(header(character, state)));
}

/// The character leaves the world (`CUser::LeaveWorld`): `0x0307` with the money at entry, then `0x0310`.
pub(super) fn left<S>(ctx: &Context<S>, character: &PlayCharacter, state: &WorldState) {
    ctx.game_log.action(game_log::leave_world(header(character, state), &summary(character, state), state.first_money));
    ctx.game_log.action(game_log::map_leave(header(character, state)));
}

/// A chat line relayed (`CUser::SendChatLogA` and `SendChatLogB`, section 3.5), by the chat type of the original: 1 normal, 2 whisper
/// (with the target), 3 guild, 4 party, 5 to all (trade chat, a Message To Server), 6 shout, 7 zone. The text is kept as typed.
pub(super) fn chat<S>(ctx: &Context<S>, character: &PlayCharacter, state: &WorldState, chat_type: u8, target: Option<&str>, text: &[u8]) {
    let text = String::from_utf8_lossy(text.strip_suffix(&[0]).unwrap_or(text));
    ctx.game_log.chat(game_log::chat_line(&header(character, state), chat_type, target, &text));
}

/// New items the server made (a quest's start items and its reward, a staff command's): `0x0401` with `'ItemCreate'` for each insert of
/// `changes`, named by the row the write left; call it once the changes are applied. The original's creation record is never stored
/// (section 1); here a new item always has the record that made it.
pub(super) fn created<S>(
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &WorldState,
    changes: &[SlotChange],
    rows: &[crate::backend::Changed],
) {
    for (change, changed) in changes.iter().zip(rows) {
        if let SlotChange::Insert(new) = change
            && new.kept.is_none_or(|k| k.origin.is_none())
            && let Some(item) = item_in(ctx, state, (new.bag, new.slot), changed.row)
        {
            ctx.game_log.action(game_log::item_created(header(character, state), &item, new.count));
        }
    }
}

/// Items taken away whole (a staff command's recall or clearing, `CUser::ItemRemove`): `0x0402` for each `Remove` of `changes`, named
/// by the row it removed. Call it before the changes are applied: the item is read from its slot.
pub(super) fn removed<S>(
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &WorldState,
    changes: &[SlotChange],
    rows: &[crate::backend::Changed],
) {
    for (change, changed) in changes.iter().zip(rows) {
        if let SlotChange::Remove { bag, slot } = change
            && let Some(unit) = state.inventory.get(*bag, *slot)
        {
            let item = ctx.game_log.item(unit, changed.row);
            ctx.game_log.action(game_log::item_drop(header(character, state), &item, *bag, *slot, unit.count));
        }
    }
}
