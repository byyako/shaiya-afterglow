//! The owner's side of pets (`docs/plans/ext-4-pets.md` sections 4 and 8): the egg's use (call the pet, send it back), the pet coming out
//! again after every `enter` and after a login, and the dismissal when its egg leaves the bags. The pet in the zone is `zone/pets.rs`.
//!
//! An egg's craft name is `P<monster id, 5 digits>.<unix seconds, 8 hex digits>.<a counter, 2 hex digits>` (`P00123.6731A2F0.00`): the
//! monster is what the egg calls, the rest makes each egg unique so the server can tell which egg's pet is out without a row id.

use super::items::Done;
use super::*;
use crate::backend::CRAFT_NAME_MAX;
use crate::backend::{Kept, NewItem};
use crate::effects::{Capture, ItemEffect, PetEgg, in_ranges};
use crate::zone::{PetLook, PetLoot};

/// The pet a connection has out: the egg it came from and how it looks, which every later `enter` (a map change) uses to call it again.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ActivePet {
    pub egg: [u8; CRAFT_NAME_MAX],
    pub look: PetLook,
}

/// The craft name of an egg of `monster` made at `seconds` (unix) as the `counter`-th of the zone: 18 characters, padded with zeros.
pub fn egg_name(monster: u16, seconds: u32, counter: u8) -> [u8; CRAFT_NAME_MAX] {
    let text = format!("P{monster:05}.{seconds:08X}.{counter:02X}");
    let mut out = [0; CRAFT_NAME_MAX];
    out[..text.len()].copy_from_slice(text.as_bytes());
    out
}

/// The monster in an egg's craft name; `None` for a name that is not one (an item made by `/make`, a script, or edited): an empty egg.
pub fn parse_egg(craft: &[u8]) -> Option<u16> {
    let end = craft.iter().position(|&b| b == 0).unwrap_or(craft.len());
    let craft = &craft[..end];
    if craft.len() != 18 || craft[0] != b'P' || craft[6] != b'.' || craft[15] != b'.' {
        return None;
    }
    let (digits, stamp, counter) = (&craft[1..6], &craft[7..15], &craft[16..18]);
    if !digits.iter().all(u8::is_ascii_digit) || !stamp.iter().chain(counter).all(u8::is_ascii_hexdigit) {
        return None;
    }
    std::str::from_utf8(digits).ok()?.parse().ok()
}

/// A craft name, padded to the stored width.
fn padded(craft: &[u8]) -> [u8; CRAFT_NAME_MAX] {
    crate::backend::Kept::pad_craft_name(craft)
}

/// The egg's craft name as the database stores it.
fn text(egg: &[u8; CRAFT_NAME_MAX]) -> String {
    let end = egg.iter().position(|&b| b == 0).unwrap_or(CRAFT_NAME_MAX);
    String::from_utf8_lossy(&egg[..end]).into_owned()
}

/// How the pet of `monster` looks and what it fetches, by the egg's effect.
fn look_of(effect: &PetEgg, monster: u16) -> PetLook {
    PetLook {
        record: effect.base.saturating_add(monster),
        wild: monster,
        loot: effect.loot.then_some(PetLoot { range: f32::from(effect.loot_range), gold: effect.gold }),
    }
}

/// Whether any bag holds an item with this craft name.
fn holds_egg(state: &WorldState, egg: &[u8; CRAFT_NAME_MAX]) -> bool {
    state.inventory.items().any(|item| item.craft_name.first() == Some(&b'P') && padded(&item.craft_name) == *egg)
}

/// The use of a Pet Egg (section 4.3). The answer is `Done::Kept` with the egg unchanged; a refusal is `Done::Failed` (the reuse time
/// put back) with a whisper where the player should be told.
#[allow(clippy::too_many_arguments)]
pub(super) async fn use_pet_egg<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    country: u8,
    effect: &ItemEffect,
    egg: &PetEgg,
    at: (u8, u8),
) -> Result<Done> {
    let Some(unit) = state.inventory.get(at.0, at.1).cloned() else { return Ok(Done::Failed) };
    let Some(monster) = parse_egg(&unit.craft_name) else {
        super::effects::refuse(conn, state, effect, "This egg is empty.").await?;
        return Ok(Done::Failed);
    };
    let name = padded(&unit.craft_name);
    let id = character.id;
    // 1. This egg's pet is out: sent back, the stored pet cleared; the egg stays.
    if state.pet.is_some_and(|pet| pet.egg == name) {
        ctx.store.set_pet(id, None).await?;
        ctx.zones.dismiss_pet(state.map, id);
        state.pet = None;
        tracing::info!(id, monster, "pet sent back");
        return Ok(Done::Kept(unit));
    }
    // 2. The rules' `pet` point.
    if let Some(refusal) = ctx.zones.rules().denied(crate::rules::Point::Pet, state.map, &rules_who(character, country)) {
        tracing::debug!(id, map = state.map, point = "pet", rule = &*refusal.name, "refused by a rule");
        super::refused::tell_refused(conn, state, &refusal).await?;
        return Ok(Done::Failed);
    }
    // 3. An egg from an earlier configuration.
    if !in_ranges(&egg.monsters, monster) {
        super::effects::refuse(conn, state, effect, "This pet cannot come out.").await?;
        return Ok(Done::Failed);
    }
    // 4. Another pet goes first (the zone replaces it); this egg's pet comes out and is stored.
    let look = look_of(egg, monster);
    ctx.store.set_pet(id, Some(&text(&name))).await?;
    if !ctx.zones.summon_pet(state.map, id, look, name, Instant::now(), false) {
        ctx.store.set_pet(id, None).await?;
        return Ok(Done::Failed);
    }
    state.pet = Some(ActivePet { egg: name, look });
    tracing::info!(id, monster, record = look.record, "pet out");
    Ok(Done::Kept(unit))
}

/// A whisper from `Server`, sent every time (it answers a roll; it is not a refusal, which the limit of ten seconds covers).
pub(super) async fn tell(conn: &mut Connection, text: &str) -> Result<()> {
    let mut bytes = text.as_bytes().to_vec();
    bytes.push(0);
    conn.send(chat::whisper(false, SERVER_NAME, &bytes)).await
}

/// The make type of an egg a catch makes (the original's Maketype letter of an item from a box).
const CATCH_MAKE_TYPE: u8 = b'S';

/// The use of a capture item (section 3.1): the rules' `capture` point, the weakest monster fighting the user, the tests of 3.2 and of
/// the HP, room for the egg, the roll; a miss uses the item up, a catch makes the egg and uses the item up in one save. A refusal keeps
/// the item (`Done::Failed`, the reuse time put back).
pub(super) async fn use_capture<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    country: u8,
    effect: &ItemEffect,
    net: &Capture,
) -> Result<Done> {
    let id = character.id;
    // 1. The rules' `capture` point.
    if let Some(refusal) = ctx.zones.rules().denied(crate::rules::Point::Capture, state.map, &rules_who(character, country)) {
        tracing::debug!(id, map = state.map, point = "capture", rule = &*refusal.name, "refused by a rule");
        super::refused::tell_refused(conn, state, &refusal).await?;
        return Ok(Done::Failed);
    }
    // Its egg's effect and item: the load checked the link, so both are there; a server that lost one refuses.
    let egg_item = crate::effects::item_id(net.egg.0, net.egg.1);
    let (Some(egg_def), Some(egg_effect)) = (ctx.data.items.get(net.egg.0, net.egg.1), ctx.data.effects.get(egg_item)) else {
        tracing::warn!(id, egg = egg_item, "capture: the egg is not an item with an effect");
        return Ok(Done::Failed);
    };
    let ItemEffect::PetEgg(egg) = &**egg_effect else { return Ok(Done::Failed) };
    // 2 and 3. The weakest monster that is fighting the user.
    let Some(mob) = ctx.zones.weakest_fighting(state.map, id, f32::from(net.range)) else {
        super::effects::refuse(conn, state, effect, "No monster is fighting you.").await?;
        return Ok(Done::Failed);
    };
    // 4. Whether it can be caught.
    let live = ctx.data.live();
    let Some(def) = live.mobs.get(mob.mob_id) else { return Ok(Done::Failed) };
    if !ctx.zones.can_catch(&mob, def, &ctx.data.world, net, egg, character.level) {
        super::effects::refuse(conn, state, effect, "That monster cannot be caught.").await?;
        return Ok(Done::Failed);
    }
    // 5. Weak enough.
    let percent = crate::zone::hp_percent(mob.hp, mob.max_hp);
    if percent > u32::from(net.below) {
        let text = format!("It is too strong to catch. Bring it below {}% HP.", net.below);
        super::effects::refuse(conn, state, effect, &text).await?;
        return Ok(Done::Failed);
    }
    // 6. Room for the egg.
    let Some(&(bag, slot)) = state.inventory.free_slots().first() else {
        super::effects::refuse(conn, state, effect, "Your bags are full.").await?;
        return Ok(Done::Failed);
    };
    // 7. The roll. The item is used up on a miss as on a catch.
    let chance = crate::zone::catch_chance(net.chance, net.below, percent);
    let rand = {
        use rand::Rng;
        rand::rng().random::<u32>() & 0x7fff
    };
    let hit = crate::zone::catch_roll(chance, rand);
    let caught = if hit { ctx.zones.capture_take(state.map, id, mob.id, net.below, Instant::now()) } else { None };
    let Some(monster) = caught else {
        tracing::info!(id, monster = mob.mob_id, percent, chance, hit, "catch missed");
        tell(conn, "It broke free!").await?;
        return Ok(Done::UsedUp);
    };
    let seconds = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as u32);
    let kept = Kept { make_type: CATCH_MAKE_TYPE, craft_name: egg_name(monster, seconds, ctx.zones.next_egg_counter()), ..Kept::default() };
    tracing::info!(id, monster, percent, chance, "monster caught");
    state.after_use = Some("Caught! The egg is in your bag.");
    Ok(Done::UsedUpFor(NewItem {
        bag,
        slot,
        item_id: egg_def.item_id,
        item_type: egg_def.item_type,
        type_id: egg_def.type_id,
        quality: egg_def.quality,
        count: 1,
        kept: Some(kept),
    }))
}

/// After the player has entered a zone (a login, a map change): the pet comes out again beside it. At the first entering of a login, a
/// stored pet whose egg is in the bags is called out first (an egg no longer there clears the row); a pet whose egg has no effect
/// any more (the extension is disabled) stays stored and does not come out.
pub(super) async fn after_enter<S: Backend>(
    conn: &mut Connection,
    ctx: &Context<S>,
    character: &PlayCharacter,
    state: &mut WorldState,
    country: u8,
) -> Result<()> {
    let id = character.id;
    if !state.pet_loaded {
        state.pet_loaded = true;
        if let Some(stored) = ctx.store.pet(id).await? {
            let name = padded(stored.as_bytes());
            let found =
                state.inventory.items().find(|item| item.craft_name.first() == Some(&b'P') && padded(&item.craft_name) == name).cloned();
            match found {
                None => ctx.store.set_pet(id, None).await?,
                Some(item) => {
                    let effect = ctx.data.effects.get(crate::effects::item_id(item.item_type, item.type_id));
                    if let (Some(effect), Some(monster)) = (effect, parse_egg(&item.craft_name))
                        && let ItemEffect::PetEgg(egg) = &**effect
                        && in_ranges(&egg.monsters, monster)
                    {
                        state.pet = Some(ActivePet { egg: name, look: look_of(egg, monster) });
                    }
                }
            }
        }
    }
    let Some(pet) = state.pet else { return Ok(()) };
    // The arrival asks the `pet` point at once: a match whispers the rule's message and the pet is kept hidden until it stops matching.
    let refusal = ctx.zones.rules().denied(crate::rules::Point::Pet, state.map, &rules_who(character, country));
    if let Some(refusal) = &refusal {
        super::refused::tell_refused(conn, state, refusal).await?;
    }
    if !ctx.zones.summon_pet(state.map, id, pet.look, pet.egg, Instant::now(), refusal.is_some()) {
        tracing::debug!(id, map = state.map, "the pet could not come out: the player is not in the zone");
    }
    Ok(())
}

/// With a pet out, whether its egg is still in the bags; when it is not (traded, sold, put in the warehouse, the guild bank or the
/// market, dropped, lost at a death, deleted) the pet is sent back and the stored pet cleared (section 4.3). The loop calls it before
/// it waits for the next request or zone event, so every path that moves items is covered, those another connection starts included.
pub(super) async fn check_egg<S: Backend>(ctx: &Context<S>, character: &PlayCharacter, state: &mut WorldState) -> Result<()> {
    let Some(pet) = state.pet else { return Ok(()) };
    if holds_egg(state, &pet.egg) {
        return Ok(());
    }
    ctx.store.set_pet(character.id, None).await?;
    ctx.zones.dismiss_pet(state.map, character.id);
    state.pet = None;
    tracing::info!(id = character.id, "pet sent back: its egg left the bags");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_eggs_craft_name_holds_the_monster_and_makes_the_egg_unique() {
        let name = egg_name(123, 0x6731A2F0, 0);
        assert_eq!(&name[..18], b"P00123.6731A2F0.00");
        assert_eq!(&name[18..], [0, 0], "padded with zeros to the stored width");
        assert_eq!(parse_egg(&name), Some(123));
        assert_eq!(parse_egg(&name[..18]), Some(123), "the item's own name has no padding");
        assert_eq!(parse_egg(&egg_name(2473, 1, 255)), Some(2473));
        assert_eq!(parse_egg(&egg_name(65535, u32::MAX, 15)), Some(65535));
        assert_ne!(egg_name(5, 100, 0), egg_name(5, 100, 1), "two catches in one second differ by the counter");
        assert_eq!(parse_egg(b"p00123.6731a2f0.00"), None, "the P is upper case");
    }

    #[test]
    fn a_name_that_is_not_an_egg_is_empty() {
        for bad in [
            &b""[..],
            b"P",
            b"00000000000000000000",
            b"01000000000000000000",
            b"P00123.6731A2F0.0",
            b"P00123.6731A2F0.000",
            b"P0123.6731A2F0.00",
            b"P001234.6731A2F0.00",
            b"P00123-6731A2F0.00",
            b"P00123.6731A2F0-00",
            b"P00123.6731A2G0.00",
            b"P00123.6731A2F0.0Z",
            b"Pabcde.6731A2F0.00",
            b"P99999.6731A2F0.00",
            b"P00123.6731A2F0.00 ",
            b"x00123.6731A2F0.00",
        ] {
            assert_eq!(parse_egg(bad), None, "{:?}", String::from_utf8_lossy(bad));
        }
        assert_eq!(parse_egg(b"P99999.6731A2F0.00"), None, "99999 is no u16 monster");
        assert_eq!(parse_egg(b"P65535.6731A2F0.00"), Some(65535));
    }

    #[test]
    fn the_look_follows_the_effect() {
        let egg = |base: u16, loot: bool| PetEgg { base, monsters: vec![(1, 2473)], loot, loot_range: 12, gold: false };
        let look = look_of(&egg(3000, true), 123);
        assert_eq!((look.record, look.wild), (3123, 123));
        assert_eq!(look.loot, Some(PetLoot { range: 12.0, gold: false }));
        assert_eq!(look_of(&egg(0, false), 123).record, 123, "base 0: the wild record");
        assert_eq!(look_of(&egg(0, false), 123).loot, None);
    }

    #[test]
    fn the_stored_text_is_the_name_without_padding() {
        assert_eq!(text(&egg_name(7, 0x10, 0x2)), "P00007.00000010.02");
        assert_eq!(padded(b"P00007.00000010.02"), egg_name(7, 0x10, 0x2));
    }
}
