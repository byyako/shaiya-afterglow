//! Entering the world: the packets the original sends after a character is selected (`CUser::PacketUserDBChar` case
//! `0x407`) and the zone entry that follows the client's `0x0201` (`docs/original/enter-world.md` sections 3 and 4).

use shaiya_proto::game::{self as g};
use shaiya_proto::point as pp;
use shaiya_proto::skill as sk;
use shaiya_proto::world::{self as w, CharData, UserShape};

use crate::backend::{ItemUnit, PlayCharacter};
use crate::guild::Burst;
use crate::stats::Totals;

pub const MAX_LEVEL: usize = 60;
/// Privilege values above this are players; the rest are staff (`m_byUserPermission`).
pub const STAFF_PRIVILEGE_MAX: u8 = 10;

/// Experience totals per mode (0 to 3) and level (0 to 60): `totals[mode][level]` is the experience needed to reach
/// `level + 1`, i.e. the sum of the per-level values of `defs.exp_defs` up to `level` (`CGameData::LoadExp`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpTable {
    totals: [[u32; MAX_LEVEL + 1]; 4],
}

impl ExpTable {
    /// From `defs.exp_defs` rows `(level 1 to 60, grow 1 to 4, exp)`; grow 1 to 4 are modes 0 to 3. Other rows are ignored.
    pub fn from_rows(rows: impl IntoIterator<Item = (u16, u8, u32)>) -> Self {
        let mut totals = [[0u32; MAX_LEVEL + 1]; 4];
        for (level, grow, exp) in rows {
            let (level, mode) = (usize::from(level), usize::from(grow).wrapping_sub(1));
            if (1..=MAX_LEVEL).contains(&level) && mode < 4 {
                totals[mode][level] = exp;
            }
        }
        for mode in &mut totals {
            for level in 1..=MAX_LEVEL {
                mode[level] = mode[level].wrapping_add(mode[level - 1]);
            }
        }
        Self { totals }
    }

    /// `(dwPrevExp, dwNextExp)` of `TP_CHAR_DATA`: the totals for the current and the next level. Out of range, the original
    /// sends a fixed value (the easy mode total at level 60); so does this.
    pub fn bounds(&self, grow: u8, level: u16) -> (u32, u32) {
        (self.total(grow, level.saturating_sub(1)), self.total(grow, level))
    }

    /// The experience total needed to pass `level` (`CGameData::GetExp`); out of range the easy mode total at level 60.
    pub fn total(&self, grow: u8, level: u16) -> u32 {
        let level = usize::from(level);
        if grow < 4 && level <= MAX_LEVEL { self.totals[usize::from(grow)][level] } else { self.totals[0][MAX_LEVEL] }
    }
}

/// Values that are the same for every player at the moment of entering.
pub struct WorldClock {
    /// The world clock in seconds (`0x0404`).
    pub world_seconds: u32,
    /// Local time packed as the original does (`0x0114`).
    pub packed_time: u32,
}

/// The enter-world burst, in the original's order, for a character without save points (the original leaves those packets out
/// when they are empty, as it does the items and the warehouse). The last packet is `0x010B`; the caller switches to XOR after sending it.
/// `totals` are the character's numbers with its equipment, passive skills and applied skills (`crate::stats`); `applied` the
/// applied skills (`0x010A`, none when empty); `lists` the quest, friend and block lists; `guild` the guild list, members and join requests, which go after `0x0404`, and the
/// guild name of `0x0105` (`docs/original/guild.md` section 10); `kill_count` the faction's kill count and the time left of its top grade
/// (`0x0211`, `docs/original/pvp-2.md` section 9).
#[allow(clippy::too_many_arguments)]
pub fn burst(
    c: &PlayCharacter,
    country: u8,
    kill_count: (u32, u32),
    exp: &ExpTable,
    clock: &WorldClock,
    totals: &Totals,
    applied: &[sk::AppliedEntry],
    lists: &[Vec<u8>],
    guild: &Burst,
) -> Vec<Vec<u8>> {
    let (prev_exp, next_exp) = exp.bounds(c.grow, c.level);
    let (attack_speed, move_speed) = totals.speeds();
    let data = CharData {
        stat_point: c.stat_point,
        skill_point: c.skill_point,
        max_health: totals.max_health(),
        dir: c.dir as u16,
        prev_exp,
        next_exp,
        exp: c.exp,
        money: c.money,
        position: c.position,
        kills: c.kills,
        guild_name: guild.name.clone(),
    };
    let mut packets = vec![g::char_success(g::OP_CHAR_SELECT, c.id), w::world_day(clock.world_seconds)];
    packets.extend(guild.packets.iter().cloned());
    packets.push(w::char_data(&data));
    // The billing storage, only when it holds anything (`CUser::SendCharacterBillingItem`, `docs/original/item-mall.md` section 6).
    let billing: Vec<_> = c.mall.billing.iter().map(crate::mall::stored_unit).collect();
    packets.extend(pp::billing_list(&billing));
    packets.extend(w::char_items(&c.items));
    // The warehouse by slot; its money is always 0 (`docs/original/warehouse.md` section 1).
    let mut bank = c.bank.clone();
    bank.sort_by_key(|i| i.slot);
    packets.extend(w::char_bank(0, &bank));
    packets.push(sk::char_skills(
        &c.skills
            .iter()
            .map(|s| sk::SkillEntry { skill_id: s.id, level: s.level, number: s.number, delay: u32::from(s.delay) })
            .collect::<Vec<_>>(),
    ));
    packets.extend(sk::char_applied(applied));
    // The quests in progress (`0x0901`) and finished (`0x0906`), then the friends (`0x2201`) and blocked players (`0x2208`), only
    // when there are any.
    packets.extend(lists.iter().cloned());
    packets.extend([w::speed(c.id, attack_speed, move_speed), w::kc_status(country, kill_count.0, kill_count.1)]);
    // The saved positions, only when any is set (`docs/original/item-windows.md` section 1).
    let points: Vec<_> =
        c.save_points.iter().map(|p| shaiya_proto::item::SavePos { index: p.slot + 1, map: p.map, position: p.position }).collect();
    packets.extend(shaiya_proto::item::save_points(&points));
    packets.extend([
        w::honor(c.honor),
        // The points and this character's last purchases; then the item-mall storage when it holds anything (section 8).
        pp::point_and_log(c.mall.points, &c.mall.log.iter().map(crate::mall::log_unit).collect::<Vec<_>>()),
    ]);
    packets.extend(pp::stored_list(&c.mall.stored.iter().map(crate::mall::stored_unit).collect::<Vec<_>>()));
    packets.extend([
        w::server_time(clock.packed_time),
        // The free stat reset never exists; the free skill reset until used (`docs/original/item-use.md` section 8).
        w::reset_counts(0, if c.free_skill_reset_used { 0 } else { 1 }),
        sk::char_quick(&c.quick_slots),
    ]);
    packets
}

/// The looks other players get (`0x0303`), with the equipment from `items` (the bags as they are now).
pub fn user_shape<'a>(c: &PlayCharacter, country: u8, items: impl IntoIterator<Item = &'a ItemUnit>) -> UserShape {
    let mut equipment = [[0u8; 3]; 8];
    let mut cloak = None;
    for item in items.into_iter().filter(|i| i.bag == 0 && i.slot < 8) {
        equipment[usize::from(item.slot)] = [item.item_type, item.type_id, w::enchant_level(&item.craft_name)];
        if item.slot == 7 {
            cloak = Some(item.gems);
        }
    }
    UserShape {
        id: c.id,
        country,
        family: c.family,
        hair: c.hair,
        face: c.face,
        size: c.size,
        job: c.job,
        sex: c.sex,
        grow: c.grow,
        kills: c.kills[0],
        equipment,
        name: c.name.clone().into_bytes(),
        cloak,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shaiya_proto::frame::{opcode, payload};
    use shaiya_proto::world::ItemUnit;

    fn table() -> ExpTable {
        // defs.exp_defs: easy (grow 1) 70, 130, 330 ...; normal (grow 2) 100, 200, 500 ...
        ExpTable::from_rows([(1, 1, 70), (2, 1, 130), (3, 1, 330), (1, 2, 100), (2, 2, 200), (3, 2, 500), (60, 1, 5)])
    }

    #[test]
    fn exp_bounds_are_running_totals() {
        let t = table();
        assert_eq!(t.bounds(1, 1), (0, 100), "normal mode, level 1");
        assert_eq!(t.bounds(1, 3), (300, 800));
        assert_eq!(t.bounds(0, 2), (70, 200));
        // Out of range: the easy total at level 60 (here 70 + 130 + 330 + 5).
        assert_eq!(t.bounds(0, 61), (535, 535));
        assert_eq!(t.bounds(4, 1), (535, 535));
    }

    #[test]
    fn burst_order_matches_the_original() {
        let c = PlayCharacter {
            id: 42,
            grow: 1,
            level: 1,
            health: [250, 85, 195],
            items: vec![ItemUnit { bag: 0, slot: 5, item_type: 5, type_id: 1, count: 1, ..Default::default() }],
            ..Default::default()
        };
        let clock = WorldClock { world_seconds: 7, packed_time: 9 };
        let totals = Totals { max_health: [300, 90, 200], attack_speed: 4, move_speed: 2, ..Default::default() };
        let packets = burst(&c, 0, (0, 0), &table(), &clock, &totals, &[], &[], &Burst::default());
        let opcodes: Vec<u16> = packets.iter().map(|p| opcode(p)).collect();
        assert_eq!(opcodes, [0x0104, 0x0404, 0x0105, 0x0106, 0x0108, 0x051C, 0x0211, 0x0230, 0x2601, 0x0114, 0x021B, 0x010B]);
        assert_eq!(payload(&packets[0]), [0, 42, 0, 0, 0]);
        // prev and next exp at struct offsets 14 and 18 (payload offsets 12 and 16)
        assert_eq!(payload(&packets[2])[12..20], [0, 0, 0, 0, 100, 0, 0, 0]);
        // The computed maxima (HP, MP, SP) at struct offsets 6 to 11, and the computed speeds.
        assert_eq!(payload(&packets[2])[4..10], [44, 1, 90, 0, 200, 0]);
        assert_eq!(packets[5], w::speed(42, 4, 2));
        // without items there is no 0x0106
        let bare =
            burst(&PlayCharacter { items: Vec::new(), ..c.clone() }, 0, (0, 0), &table(), &clock, &totals, &[], &[], &Burst::default());
        assert!(bare.iter().all(|p| opcode(p) != 0x0106));
        // The warehouse (0x0711) follows the items, only when it holds something.
        assert!(packets.iter().all(|p| opcode(p) != 0x0711));
        let stored = PlayCharacter {
            bank: vec![ItemUnit { bag: 100, slot: 3, item_type: 25, type_id: 1, count: 2, ..Default::default() }],
            ..c.clone()
        };
        let with_bank = burst(&stored, 0, (0, 0), &table(), &clock, &totals, &[], &[], &Burst::default());
        assert_eq!(opcode(&with_bank[3]), 0x0106);
        assert_eq!(opcode(&with_bank[4]), 0x0711);
        // Applied skills come right after the skill list.
        let buffed = burst(
            &c,
            0,
            (0, 0),
            &table(),
            &clock,
            &totals,
            &[sk::AppliedEntry { key: 1, skill_id: 30, level: 1, seconds: 290 }],
            &[],
            &Burst::default(),
        );
        assert_eq!(opcode(&buffed[5]), 0x010A);
        assert_eq!(opcode(&buffed[6]), 0x051C);
        // A guild's packets come before 0x0105, which carries its name.
        let guild =
            Burst { packets: vec![shaiya_proto::encode(0x0D0D, &[]), shaiya_proto::encode(0x0D0E, &[])], name: b"Knights".to_vec() };
        let member = burst(&c, 0, (0, 0), &table(), &clock, &totals, &[], &[], &guild);
        assert_eq!(member.iter().map(|p| opcode(p)).take(5).collect::<Vec<_>>(), [0x0104, 0x0404, 0x0D0D, 0x0D0E, 0x0105]);
        assert!(payload(&member[4]).ends_with(b"Knights\0"));
        // The item mall (`docs/original/item-mall.md` section 8): 0xB101 after 0x0105, the points and log, 0x2606 after them.
        let row = crate::backend::StoredPointItem { slot: 2, item_id: 100042, count: 1 };
        let log = crate::backend::PointLogRow { code: "A1".to_owned(), at: 0, points: 300 };
        let mall = crate::backend::Mall { points: 900, log: vec![log], stored: vec![row], billing: vec![row], ..Default::default() };
        let shopper = burst(&PlayCharacter { mall, ..c.clone() }, 0, (0, 0), &table(), &clock, &totals, &[], &[], &Burst::default());
        let opcodes: Vec<u16> = shopper.iter().map(|p| opcode(p)).collect();
        assert_eq!(
            opcodes,
            [0x0104, 0x0404, 0x0105, 0xB101, 0x0106, 0x0108, 0x051C, 0x0211, 0x0230, 0x2601, 0x2606, 0x0114, 0x021B, 0x010B]
        );
        assert_eq!(payload(&shopper[9])[..5], [0x84, 3, 0, 0, 1]);
        assert_eq!(payload(&shopper[10]), [1, 2, 100, 42, 1]);
    }
}
