//! Mounts (`docs/original/vehicles.md`): `0x0216` and the summon's completion, the two-seat requests `0x021C` to `0x021E`, and what
//! changing the mount slot does. The vehicle status, the pair and the packets to others are the zone's (`crate::zone::Riding`); the
//! connection keeps the summon's due time and the next allowed summon.

use super::*;
use crate::zone::{Mount, Riding};
use shaiya_proto::vehicle as vp;

/// The mount's equipment slot.
const MOUNT_SLOT: u8 = 13;
/// Summoning takes 3 s with a mount of mode 3, 5 s otherwise.
const SUMMON_FAST: Duration = Duration::from_secs(3);
const SUMMON: Duration = Duration::from_secs(5);
/// Shapes that make a summon fail when it completes (`CUser::UpdateRebirth`).
const NO_MOUNT_SHAPES: std::ops::RangeInclusive<u8> = 10..=12;

/// The equipped mount's definition.
fn mount_def<'a, S>(ctx: &'a Context<S>, state: &WorldState) -> Option<&'a crate::inventory::ItemDef> {
    state.inventory.get(0, MOUNT_SLOT).and_then(|item| ctx.data.items.get(item.item_type, item.type_id))
}

/// The mount's shape and seats (`CUser::UpdateRebirth`, section 1): type 15 (two seats) when the player's mode and the mount's are
/// both at least 2, else 14; plus `range + 7` from range 2 on, else `range × 2`.
pub(super) fn mount_of(def: &crate::inventory::ItemDef, player_grow: u8) -> Mount {
    let two_seat = player_grow >= 2 && def.grow >= 2;
    let add = if def.range >= 2 { u32::from(def.range) + 7 } else { u32::from(def.range) * 2 };
    Mount { shape: (if two_seat { 15 } else { 14 } + add) as u8, two_seat }
}

/// `0x0216` (`CUser::PacketMaininterface` 0x46940b, section 1).
pub(super) async fn use_vehicle<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
) -> Result<()> {
    let (id, map, now) = (character.id, state.map, Instant::now());
    let riding = ctx.zones.riding(map, id);
    if riding == Riding::Behind {
        return Ok(());
    }
    let Some(def) = mount_def(ctx, state).copied() else { return conn.send(vp::used(vp::SUMMON_REFUSED)).await };
    if state.motion != w::MOTION_STAND || state.effects.shape != 0 {
        return conn.send(vp::used(vp::SUMMON_REFUSED)).await;
    }
    stop_attacking(ctx, state, id);
    if riding == Riding::OnFoot {
        if state.next_summon.is_some_and(|next| now <= next) {
            return conn.send(vp::used(vp::TOO_SOON)).await;
        }
        let due = now + if def.grow == 3 { SUMMON_FAST } else { SUMMON };
        state.next_summon = Some(due);
        state.summon_due = Some(due);
        tracing::debug!(id, mount = def.item_id, "summoning a mount");
        return conn.send(ctx.zones.start_summon(map, id)).await;
    }
    // Summoning again cancels it; riding, the player gets off.
    if riding == Riding::Summoning {
        state.next_summon = None;
    }
    state.summon_due = None;
    get_off(conn, ctx, state, id).await
}

/// Off the mount, or the summon stopped: `0x0216` 1, 0, then the shape (`CUser::SendShape`).
async fn get_off<S>(conn: &mut Connection, ctx: &Context<S>, state: &WorldState, id: u32) -> Result<()> {
    let Some(shape) = ctx.zones.dismount(state.map, id) else { return Ok(()) };
    conn.send(vp::used(vp::GOT_OFF)).await?;
    tracing::debug!(id, "off the mount");
    conn.send(shape).await
}

/// The summon is due (`CUser::UpdateRebirth`): riding, `0x0216` 1, 1 and the shape, the next summon allowed after the mount's `reqig`
/// seconds; without the mount or in shapes 10 to 12, `0x0216` 0, 1.
pub(super) async fn summon_due<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
) -> Result<()> {
    let (id, map, now) = (character.id, state.map, Instant::now());
    state.summon_due = None;
    let def = mount_def(ctx, state).copied().filter(|_| !NO_MOUNT_SHAPES.contains(&state.effects.shape));
    let Some(def) = def else {
        ctx.zones.stop_summon(map, id);
        state.next_summon = None;
        return conn.send(vp::used(vp::SUMMON_REFUSED)).await;
    };
    let mount = mount_of(&def, character.grow);
    let Some(shape) = ctx.zones.mount(map, id, mount) else { return Ok(()) };
    state.next_summon = Some(now + Duration::from_secs(u64::from(def.reuse_group)));
    tracing::info!(id, mount = def.item_id, shape = mount.shape, two_seat = mount.two_seat, "mounted");
    conn.send(vp::used(vp::MOUNTED)).await?;
    conn.send(shape).await
}

/// A summon stops without a word (an accepted step, a hit, a map change).
pub(super) fn cancel_summon<S>(ctx: &Context<S>, state: &mut WorldState, id: u32) {
    if state.summon_due.take().is_some() {
        state.next_summon = None;
        ctx.zones.stop_summon(state.map, id);
    }
}

/// The mount slot changed (`CUser::ItemBagToBag`, section 2): summoning stops or the rider gets off, `0x0216` 1, 0 and the shape.
pub(super) async fn mount_slot_changed<S>(conn: &mut Connection, ctx: &Context<S>, state: &mut WorldState, id: u32) -> Result<()> {
    match ctx.zones.riding(state.map, id) {
        Riding::Summoning => {
            state.summon_due = None;
            state.next_summon = None;
        }
        Riding::Riding(_) => {}
        Riding::OnFoot | Riding::Behind => return Ok(()),
    }
    get_off(conn, ctx, state, id).await
}

/// Whether a slot change touches the mount.
pub(super) fn touches_mount(places: [(u8, u8); 2]) -> bool {
    places.contains(&(0, MOUNT_SLOT))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::ItemDef;

    #[test]
    fn mount_shapes_follow_the_modes_and_the_range() {
        let mount = |grow, range| ItemDef { item_type: 42, grow, range, ..Default::default() };
        assert_eq!(mount_of(&mount(1, 0), 3), Mount { shape: 14, two_seat: false });
        assert_eq!(mount_of(&mount(1, 1), 1), Mount { shape: 16, two_seat: false });
        // Mode 2 or more on both sides: the two-seat type 15.
        assert_eq!(mount_of(&mount(2, 2), 2), Mount { shape: 24, two_seat: true });
        assert_eq!(mount_of(&mount(3, 3), 1), Mount { shape: 24, two_seat: false });
        assert_eq!(mount_of(&mount(3, 6), 3), Mount { shape: 28, two_seat: true });
    }
}
