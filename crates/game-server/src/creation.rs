//! New characters, as `CUser::ProcessCharacterCreate` in `ps_game.exe` builds them
//! (`docs/original/ps_game.md` section 6.4).

use std::collections::HashMap;

use shaiya_proto::game::{CreateRequest, SLOTS};

use crate::backend::{Faction, NewCharacter, NewItem};

/// Base HP, SP and MP of a level 1 character per class (job 0 to 5), from `defs.status_defs`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClassBase {
    pub hp: u16,
    pub sp: u16,
    pub mp: u16,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StartPosition {
    pub map: u16,
    pub position: [f32; 3],
}

/// Everything creation needs besides the request.
#[derive(Clone, Debug)]
pub struct CreationData {
    pub class_base: [ClassBase; 6],
    /// Start position per race (family 0 to 3).
    pub start: [StartPosition; 4],
    /// Starting items per (race, class), already placed (`place_base_items`).
    pub base_items: HashMap<(u8, u8), Vec<NewItem>>,
}

/// A row of `defs.base_items_defs`, with the quality taken from the item's definition (the original ignores the
/// table's own `Slot` and `Quality` columns).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BaseItemDef {
    pub family: u8,
    pub job: u8,
    pub item_id: u32,
    pub item_type: u8,
    pub type_id: u8,
    pub count: u8,
    pub quality: u16,
}

/// Highest item type that counts as a weapon when placing starting items.
const WEAPON_TYPE_MAX: u8 = 19;
const WEAPON_SLOT: u8 = 5;
const INVENTORY_SLOTS: u8 = 24;

/// The original's placement (`CGameData::LoadBaseItem`): per race and class, in row order, the first weapon (type 1 to 19)
/// is equipped in bag 0 slot 5 and every other item goes to bag 1, slots 0, 1, 2, ... Rows with type 0 are skipped.
pub fn place_base_items(rows: impl IntoIterator<Item = BaseItemDef>) -> HashMap<(u8, u8), Vec<NewItem>> {
    let mut placed: HashMap<(u8, u8), Vec<NewItem>> = HashMap::new();
    for row in rows {
        if row.item_type == 0 {
            continue;
        }
        let items = placed.entry((row.family, row.job)).or_default();
        let has_weapon = items.iter().any(|i| i.bag == 0);
        let (bag, slot) = if !has_weapon && row.item_type <= WEAPON_TYPE_MAX {
            (0, WEAPON_SLOT)
        } else {
            (1, items.iter().filter(|i| i.bag == 1).count() as u8)
        };
        if bag == 1 && slot >= INVENTORY_SLOTS {
            continue;
        }
        items.push(NewItem {
            bag,
            slot,
            item_id: row.item_id,
            item_type: row.item_type,
            type_id: row.type_id,
            quality: row.quality,
            count: row.count,
            kept: None,
        });
    }
    placed
}

impl CreationData {
    /// The original's `[BASE]` start positions in `Data/Map.ini` (`HU`, `EL`, `BA`, `DE` GameStartMapPos):
    /// humans and elves start on map 1, Vail and Deatheaters on map 2. The original picks randomly among several
    /// positions per race when more are configured; the shipped file has one each.
    pub fn original_start_positions() -> [StartPosition; 4] {
        [
            StartPosition { map: 1, position: [577.98, 78.56, 1762.12] },
            StartPosition { map: 1, position: [1678.92, 38.92, 1776.91] },
            StartPosition { map: 2, position: [165.25, 118.50, 397.89] },
            StartPosition { map: 2, position: [1838.93, 129.33, 443.43] },
        ]
    }
}

/// Starting attributes per class: str, dex, rec, int, wis, luc.
const CLASS_STATS: [[u16; 6]; 6] = [
    [14, 12, 9, 8, 7, 11],   // fighter
    [10, 9, 12, 10, 10, 10], // defender
    [10, 15, 9, 7, 8, 12],   // ranger
    [11, 10, 10, 7, 10, 13], // archer
    [7, 9, 9, 15, 12, 9],    // mage
    [8, 9, 10, 12, 14, 8],   // priest
];

/// A class's starting stats with its race's bonus (STR, DEX, REC, INT, WIS, LUC): human LUC + 4, elf DEX + 4, vail INT and WIS + 2,
/// deatheater STR and REC + 2. Also what a stat reset starts from (`docs/original/item-use.md` section 8).
pub fn starting_stats(job: u8, family: u8) -> [u16; 6] {
    let mut stats = CLASS_STATS[usize::from(job.min(5))];
    let [str, dex, rec, int, wis, luc] = [0, 1, 2, 3, 4, 5];
    match family {
        0 => stats[luc] += 4,
        1 => stats[dex] += 4,
        2 => {
            stats[int] += 2;
            stats[wis] += 2;
        }
        _ => {
            stats[rec] += 2;
            stats[str] += 2;
        }
    }
    stats
}

/// Classes each race may play, in class numbers (fighter 0, defender 1, ranger 2, archer 3, mage 4, priest 5).
/// Races 0 and 1 belong to faction 0 (Light), races 2 and 3 to faction 1 (Fury).
pub const RACE_CLASSES: [[u8; 3]; 4] = [
    [0, 1, 5], // human
    [2, 3, 4], // elf
    [2, 4, 5], // vail
    [0, 1, 3], // deatheater
];

/// Names: our rule, stricter than the original (which only required two characters). ASCII letters and digits fit every
/// client code page, and 18 characters keep a terminating zero inside the 19 bytes the character list carries.
pub const NAME_MIN_LEN: usize = 2;
pub const NAME_MAX_LEN: usize = 18;

/// The name rule above, for creation and the name change (`docs/original/small-items.md` deviation 3).
pub fn name_allowed(name: &[u8]) -> bool {
    (NAME_MIN_LEN..=NAME_MAX_LEN).contains(&name.len()) && name.iter().all(u8::is_ascii_alphanumeric)
}

/// Builds the character or says why not. The checks are the original's (`CUser::PacketCharacter` and
/// `ProcessCharacterCreate`), plus the name rule above.
pub fn new_character(
    user_id: u32,
    server_id: u8,
    faction: Option<Faction>,
    request: &CreateRequest,
    data: &CreationData,
) -> Result<NewCharacter, &'static str> {
    let Some(faction) = faction else { return Err("no faction chosen") };
    let r = request;
    if r.slot >= SLOTS {
        return Err("slot out of range");
    }
    if r.grow > faction.max_grow {
        return Err("mode above the account's maximum");
    }
    if r.family > 3 || r.job > 5 {
        return Err("race or class out of range");
    }
    if !RACE_CLASSES[usize::from(r.family)].contains(&r.job) {
        return Err("class not available to this race");
    }
    // Not checked by the original server; the client offers only these races (docs/original/enter-world.md 1.3).
    if r.family / 2 != faction.country {
        return Err("race not in the account's faction");
    }
    if r.sex > 1 {
        return Err("sex out of range");
    }
    if !name_allowed(&r.name) {
        return Err("name must be 2 to 18 ASCII letters or digits");
    }

    let [str, dex, rec, int, wis, luc] = starting_stats(r.job, r.family);
    let base = data.class_base[usize::from(r.job)];
    // Harder modes start with more HP, SP and MP: x1.1 for hard, x1.5 for ultimate, truncated (as the stats are computed).
    let scale = |value: u16| crate::stats::scaled(value, r.grow) as u32;
    let (mut hp, mut sp, mut mp) = (scale(base.hp), scale(base.sp), scale(base.mp));
    match r.family {
        0 => sp += 10,
        1 => mp += 10,
        2 => {
            sp += 5;
            mp += 5;
        }
        _ => hp += 10,
    }
    hp += u32::from(rec) * 5;
    sp += u32::from(dex) * 5;
    mp += u32::from(wis) * 5;

    let start = &data.start[usize::from(r.family)];
    let clamp = |v: u32| u16::try_from(v).unwrap_or(u16::MAX);
    Ok(NewCharacter {
        user_id,
        server_id,
        slot: r.slot,
        name: String::from_utf8(r.name.clone()).expect("checked to be ASCII"),
        family: r.family,
        grow: r.grow,
        // TODO(client-research-queue.md entry 13 (a)): clamp hair, face and size to what the creation screen sends once the ranges
        // are known; until then they are stored as sent, as in the original (`docs/server/audit.md` R21).
        hair: r.hair,
        face: r.face,
        size: r.size,
        job: r.job,
        sex: r.sex,
        level: 1,
        stat_point: 0,
        // The original's create procedure forced 5 skill points.
        skill_point: 5,
        stats: [str, dex, rec, int, wis, luc],
        health: [clamp(hp), clamp(mp), clamp(sp)],
        map: start.map,
        dir: 0,
        position: start.position,
        money: 0,
        items: data.base_items.get(&(r.family, r.job)).cloned().unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `defs.status_defs` level 1, as loaded from PS_GameDefs.
    fn data() -> CreationData {
        let b = |hp, sp, mp| ClassBase { hp, sp, mp };
        CreationData {
            class_base: [b(205, 125, 50), b(250, 65, 65), b(170, 150, 60), b(175, 135, 70), b(150, 60, 170), b(160, 70, 150)],
            start: CreationData::original_start_positions(),
            base_items: place_base_items(base_rows()),
        }
    }

    /// Human fighter and defender rows of `defs.base_items_defs`, in row order.
    fn base_rows() -> Vec<BaseItemDef> {
        let row = |job, item_type, type_id, count| BaseItemDef {
            family: 0,
            job,
            item_id: u32::from(item_type) * 1000 + u32::from(type_id),
            item_type,
            type_id,
            count,
            quality: 100,
        };
        vec![row(0, 5, 1, 1), row(0, 25, 1, 10), row(1, 1, 1, 1), row(1, 19, 1, 1)]
    }

    #[test]
    fn names_are_2_to_18_ascii_letters_or_digits() {
        assert!(name_allowed(b"Ab") && name_allowed(b"Ranger2") && name_allowed(&[b'a'; 18]));
        assert!(!name_allowed(b"x") && !name_allowed(&[b'a'; 19]) && !name_allowed(b"Bad_Name") && !name_allowed("Él".as_bytes()));
    }

    #[test]
    fn base_items_are_placed_like_the_original() {
        let placed = place_base_items(base_rows());
        let spots = |job| placed[&(0, job)].iter().map(|i| (i.bag, i.slot, i.item_type)).collect::<Vec<_>>();
        // Fighter: the sword is equipped, the potions go to the first inventory slot.
        assert_eq!(spots(0), [(0, 5, 5), (1, 0, 25)]);
        // Defender: only the first weapon-type item is equipped; the shield (type 19) goes to the inventory.
        assert_eq!(spots(1), [(0, 5, 1), (1, 0, 19)]);
        // A type 0 row is skipped, and a second weapon after the first goes to the inventory.
        let more = place_base_items([
            BaseItemDef { family: 1, job: 2, item_id: 0, item_type: 0, type_id: 0, count: 1, quality: 0 },
            BaseItemDef { family: 1, job: 2, item_id: 25001, item_type: 25, type_id: 1, count: 10, quality: 0 },
            BaseItemDef { family: 1, job: 2, item_id: 9001, item_type: 9, type_id: 1, count: 1, quality: 0 },
            BaseItemDef { family: 1, job: 2, item_id: 9002, item_type: 9, type_id: 2, count: 1, quality: 0 },
        ]);
        let spots = more[&(1, 2)].iter().map(|i| (i.bag, i.slot, i.item_id)).collect::<Vec<_>>();
        assert_eq!(spots, [(1, 0, 25001), (0, 5, 9001), (1, 1, 9002)]);
    }

    #[test]
    fn new_characters_get_their_base_items() {
        let c = new_character(1, 1, LIGHT, &request(0, 1, 0), &data()).unwrap();
        assert_eq!(c.items.len(), 2);
        assert_eq!((c.items[0].bag, c.items[0].slot, c.items[0].item_id), (0, 5, 1001));
        let none = new_character(1, 1, LIGHT, &request(0, 5, 0), &data()).unwrap();
        assert!(none.items.is_empty(), "no rows for human priests in this test table");
    }

    fn request(family: u8, job: u8, grow: u8) -> CreateRequest {
        CreateRequest { slot: 0, family, grow, hair: 1, face: 2, size: 3, job, sex: 0, name: b"Hero".to_vec(), sent_len: 13 }
    }

    const LIGHT: Option<Faction> = Some(Faction { country: 0, max_grow: 3 });
    const FURY: Option<Faction> = Some(Faction { country: 1, max_grow: 3 });

    #[test]
    fn human_fighter_on_normal() {
        let c = new_character(1, 1, LIGHT, &request(0, 0, 1), &data()).unwrap();
        // fighter 14/12/9/8/7/11, human +4 luc
        assert_eq!(c.stats, [14, 12, 9, 8, 7, 15]);
        // hp 205 + 9*5, mp 50 + 7*5, sp 125 + 10 + 12*5
        assert_eq!(c.health, [250, 85, 195]);
        assert_eq!((c.level, c.skill_point, c.map), (1, 5, 1));
        assert_eq!(c.position, [577.98, 78.56, 1762.12]);
    }

    #[test]
    fn modes_scale_the_class_base_only() {
        // Deatheater defender: rec 12+2, str 10+2; hp base 250.
        let hard = new_character(1, 1, FURY, &request(3, 1, 2), &data()).unwrap();
        assert_eq!(hard.stats, [12, 9, 14, 10, 10, 10]);
        // hp: trunc(250*1.1)=275 +10 +14*5=355; mp: trunc(65*1.1)=71 +10*5=121; sp: 71 +9*5=116
        assert_eq!(hard.health, [355, 121, 116]);
        let ultimate = new_character(1, 1, FURY, &request(3, 1, 3), &data()).unwrap();
        // hp 375+10+70, mp 97+50, sp 97+45
        assert_eq!(ultimate.health, [455, 147, 142]);
        assert_eq!(ultimate.map, 2);
    }

    #[test]
    fn elf_and_vail_bonuses() {
        let elf_mage = new_character(1, 1, LIGHT, &request(1, 4, 0), &data()).unwrap();
        assert_eq!(elf_mage.stats, [7, 13, 9, 15, 12, 9]);
        assert_eq!(elf_mage.health, [150 + 45, 170 + 10 + 60, 60 + 65]);
        let vail_priest = new_character(1, 1, FURY, &request(2, 5, 0), &data()).unwrap();
        assert_eq!(vail_priest.stats, [8, 9, 10, 14, 16, 8]);
        assert_eq!(vail_priest.health, [160 + 50, 150 + 5 + 80, 70 + 5 + 45]);
    }

    #[test]
    fn refusals() {
        let d = data();
        assert!(new_character(1, 1, None, &request(0, 0, 0), &d).is_err(), "no faction");
        assert!(new_character(1, 1, LIGHT, &request(0, 2, 0), &d).is_err(), "humans cannot be rangers");
        assert!(new_character(1, 1, LIGHT, &request(4, 0, 0), &d).is_err(), "race 4");
        assert!(new_character(1, 1, LIGHT, &request(3, 0, 0), &d).is_err(), "deatheater in Light");
        assert!(new_character(1, 1, FURY, &request(1, 4, 0), &d).is_err(), "elf in Fury");
        let low = Some(Faction { country: 0, max_grow: 1 });
        assert!(new_character(1, 1, low, &request(0, 0, 3), &d).is_err(), "mode above maximum");
        let mut r = request(0, 0, 0);
        r.slot = 5;
        assert!(new_character(1, 1, LIGHT, &r, &d).is_err(), "slot 5");
        for bad in [&b"A"[..], b"Bad-Name", b"Nineteen1234567890X", b"caf\xe9"] {
            let mut r = request(0, 0, 0);
            r.name = bad.to_vec();
            assert!(new_character(1, 1, LIGHT, &r, &d).is_err(), "name {bad:?}");
        }
    }
}
