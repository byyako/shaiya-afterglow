use super::*;
use crate::skills::SkillMatch;
use shaiya_proto::frame::{opcode, payload};
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

/// No drop tables and no item definitions.
pub(super) fn no_loot() -> (&'static DropTables, &'static ItemDefs) {
    static DROPS: std::sync::LazyLock<DropTables> = std::sync::LazyLock::new(DropTables::default);
    static ITEMS: std::sync::LazyLock<ItemDefs> = std::sync::LazyLock::new(ItemDefs::default);
    (&DROPS, &ITEMS)
}

pub(super) fn player(id: u32, x: f32, z: f32) -> (Occupant, crate::outbox::OutboxInbox) {
    let (outbox, inbox) = crate::outbox::channel(usize::MAX);
    let o = Occupant {
        id,
        staff: false,
        visible: true,
        attackable: true,
        pvp: true,
        position: [x, 0.0, z],
        dir: 0,
        guild_id: 0,
        attack_speed: 0,
        move_speed: 2,
        vitals: Vitals { health: [100; 3], max: [100; 3], ..Default::default() },
        shape: UserShape { id, ..Default::default() },
        buffs: PlayerBuffs::default(),
        outbox,
        // Events are checked in the combat tests only; elsewhere they go nowhere.
        events: unbounded_channel().0,
        mount: None,
    };
    (o, inbox)
}

/// Every packet waiting in an outbox.
fn received(inbox: &mut crate::outbox::OutboxInbox) -> Vec<Vec<u8>> {
    std::iter::from_fn(|| inbox.try_recv().ok()).collect()
}

pub(super) fn drain(inbox: &mut crate::outbox::OutboxInbox) -> Vec<(u16, u32)> {
    let mut seen = Vec::new();
    while let Ok(p) = inbox.try_recv() {
        seen.push((opcode(&p), u32::from_le_bytes(payload(&p)[..4].try_into().unwrap())));
    }
    seen
}

pub(super) fn step(x: f32, z: f32) -> Move {
    Move { motion: 0, dir: 0, position: [x, 0.0, z] }
}

#[test]
fn equipment_is_shown_to_the_same_country_only_and_never_for_staff() {
    // docs/original/small-items.md section 7.
    let zones = Zones::default();
    let mut events = Vec::new();
    for (id, country, staff) in [(1, 0, false), (2, 0, false), (3, 1, false), (4, 0, true)] {
        let (mut o, _) = player(id, 100.0, 100.0);
        let (sender, receiver) = unbounded_channel();
        (o.shape.country, o.staff, o.events) = (country, staff, sender);
        zones.enter(1, o);
        events.push(receiver);
    }
    assert!(zones.inspect(1, 1, 2));
    assert!(matches!(events[1].try_recv(), Ok(ZoneEvent::Inspect { asker: 1 })));
    assert!(!zones.inspect(1, 1, 3), "the other country");
    assert!(!zones.inspect(1, 1, 4), "staff");
    assert!(!zones.inspect(2, 1, 2), "not in the asker's zone");
    assert!(!zones.inspect(1, 1, 9), "nobody");
    assert!(events[2].try_recv().is_err() && events[3].try_recv().is_err());
}

#[test]
fn cells_keep_a_ten_unit_margin() {
    assert_eq!(cell_of([45.0, 0.0, 85.0]), (1, 2));
    assert!(keeps_cell((1, 2), [31.0, 0.0, 129.0]), "10 outside on both sides is still the cell");
    assert!(!keeps_cell((1, 2), [29.0, 0.0, 100.0]));
    assert!(!keeps_cell((1, 2), [60.0, 0.0, 131.0]));
}

#[test]
fn players_in_view_see_each_other_enter_and_leave() {
    let zones = Zones::default();
    let (a, mut a_in) = player(1, 100.0, 100.0);
    let (b, mut b_in) = player(2, 150.0, 100.0);
    let (far, mut far_in) = player(3, 1000.0, 1000.0);
    zones.enter(1, a);
    zones.enter(1, far);
    let for_b = zones.enter(1, b);
    let seen: Vec<_> = for_b.iter().map(|p| opcode(p)).collect();
    // own 0x0201 and 0x051C, then A's, then the weather.
    assert_eq!(seen, [0x0201, 0x051C, 0x0201, 0x051C, 0x0451]);
    assert_eq!(payload(&for_b[0])[..5], [2, 0, 0, 0, 1], "B itself, byNew 1");
    assert_eq!(payload(&for_b[2])[..5], [1, 0, 0, 0, 0], "A, already there: byNew 0");
    assert_eq!(drain(&mut a_in), [(0x0201, 2), (0x051C, 2)], "A sees B arrive");
    assert!(drain(&mut far_in).is_empty(), "too far");

    // A asks about B: position, then status or looks.
    assert_eq!(zones.info(1, 2).iter().map(|p| opcode(p)).collect::<Vec<_>>(), [0x0501, 0x0301]);
    zones.set_motion(1, 2, 1);
    let shape = zones.shape(1, 1, 2);
    assert_eq!(payload(&shape[0])[4], w::MOTION_SYNC);
    assert_eq!((opcode(&shape[1]), payload(&shape[1])[5]), (0x0303, 1), "B sits");

    zones.leave(1, 2);
    assert_eq!(drain(&mut a_in), [(0x0202, 2)]);
    assert_eq!(zones.shape(1, 1, 2), [w::user_leave(2)], "gone: 0x0202");
    assert!(zones.info(1, 2).is_empty());
    assert!(drain(&mut b_in).is_empty());
    assert_eq!(zones.count(1), 2);
}

#[test]
fn chat_reaches_the_right_players() {
    let zones = Zones::default();
    let named = |id, x: f32, country, name: &[u8]| {
        let (mut o, inbox) = player(id, x, 100.0);
        o.shape.country = country;
        o.shape.name = name.to_vec();
        (o, inbox)
    };
    let (a, mut a_in) = named(1, 100.0, 0, b"Alice");
    let (b, mut b_in) = named(2, 120.0, 0, b"Bob");
    let (c, mut c_in) = named(3, 180.0, 0, b"Carol");
    let (d, mut d_in) = named(4, 110.0, 1, b"Dark");
    for o in [a, b, c, d] {
        zones.enter(1, o);
    }
    for inbox in [&mut a_in, &mut b_in, &mut c_in, &mut d_in] {
        drain(inbox);
    }
    // Normal chat: within 30 of Alice (Bob 20, Dark 10), not Carol (80).
    zones.send_near(1, 1, w::user_leave(1), NORMAL_CHAT_RANGE);
    assert_eq!((drain(&mut b_in).len(), drain(&mut d_in).len(), drain(&mut c_in).len()), (1, 1, 0));
    // Faction-wide: Alice, Bob and Carol, not Dark.
    zones.send_country(0, None, &w::user_leave(9));
    assert_eq!((drain(&mut a_in).len(), drain(&mut c_in).len(), drain(&mut d_in).len()), (1, 1, 0));
    drain(&mut b_in);
    assert_eq!(zones.find_by_name(b"carol"), Some((1, 3, 0, b"Carol".to_vec())), "ignoring case");
    assert_eq!(zones.find_by_name(b"Nobody"), None);
}

#[test]
fn moving_swaps_the_view_and_reaches_players_in_range() {
    let zones = Zones::default();
    let (a, mut a_in) = player(1, 200.0, 100.0);
    let (b, mut b_in) = player(2, 400.0, 100.0);
    zones.enter(1, a);
    zones.enter(1, b);
    assert!(drain(&mut a_in).is_empty(), "B is 5 cells away");
    drain(&mut b_in);

    // A walks towards B: still out of view in cell 7.
    let out = zones.move_to(1, 1, &step(300.0, 100.0));
    assert_eq!(out.iter().map(|p| opcode(p)).collect::<Vec<_>>(), [0x0501], "only its own move");
    assert!(drain(&mut b_in).is_empty());
    // Past the margin (330 still counts as cell 7), cell 8 brings B into view, and A is within 80 of B.
    let out = zones.move_to(1, 1, &step(335.0, 100.0));
    assert_eq!(out.iter().map(|p| opcode(p)).collect::<Vec<_>>(), [0x0201, 0x051C, 0x0501], "B's enter and speed, then A's move");
    assert_eq!(drain(&mut b_in), [(0x0201, 1), (0x051C, 1), (0x0501, 1)], "B sees A arrive and move");

    // Walking away again: both get 0x0202.
    let out = zones.move_to(1, 1, &step(30.0, 100.0));
    assert_eq!(opcode(&out[0]), 0x0202);
    assert_eq!(drain(&mut b_in), [(0x0202, 1)]);
}

#[test]
fn npcs_come_into_and_leave_the_view() {
    use crate::world::{MapNpcRow, World};
    let npc = |kind, x| MapNpcRow { map: 1, kind, type_id: 1, first: Some([x, 0.0, 100.0, 0.0]), walk: Vec::new() };
    // NPC 1 in cell (2, 2), NPC 2 in cell (12, 2).
    let world = World::build([(1, 2048)], &std::collections::HashSet::from([(1, 1), (2, 1)]), [npc(1, 110.0), npc(2, 500.0)]);
    let zones = Zones::new(&world, &MobDefs::default());
    let (a, _a_in) = player(7, 100.0, 100.0);
    let seen = |out: &[Vec<u8>]| -> Vec<(u16, u32)> {
        // The id, or 0 for the 3-byte weather.
        out.iter().map(|p| (opcode(p), payload(p).get(..4).map_or(0, |b| u32::from_le_bytes(b.try_into().unwrap())))).collect()
    };
    assert_eq!(seen(&zones.enter(1, a)), [(0x0201, 7), (0x051C, 7), (0x0E01, 1), (0x0451, 0)], "only NPC 1 is in view");
    // Cell 7 sees cells 5 to 9: NPC 1 leaves the view.
    assert_eq!(seen(&zones.move_to(1, 7, &step(300.0, 100.0))), [(0x0E02, 1), (0x0501, 7)]);
    // Cell 10 sees cells 8 to 12: NPC 2 comes into view.
    assert_eq!(seen(&zones.move_to(1, 7, &step(420.0, 100.0))), [(0x0E01, 2), (0x0501, 7)]);
    // Within the cell nothing changes.
    assert_eq!(seen(&zones.move_to(1, 7, &step(425.0, 100.0))), [(0x0501, 7)]);
}

#[test]
fn monsters_spawn_and_follow_the_view() {
    use crate::mobs::{MobDef, MobDefs, SpawnArea};
    let mut world = World::build([(1, 2048)], &std::collections::HashSet::new(), []);
    // Monster 7 in a one-spot area at (100, 100); every spot of x 0 to 7, z 0 is blocked.
    let mut blocked = vec![0u8; 2048 * 2048 / 8];
    blocked[0] = 0xff;
    let area = |x: f32, z: f32| SpawnArea::new([x, 0.0, z, x, 0.0, z], vec![(7, 1)]);
    world.set_mob_data([(1, blocked)].into(), [(1, vec![area(100.0, 100.0), area(3.0, 0.0)])].into());
    let defs = MobDefs::new([MobDef { id: 7, level: 1, hp: 50, respawn: std::time::Duration::from_secs(5), ..Default::default() }]);
    let zones = Zones::new(&world, &defs);
    let (a, mut a_in) = player(1, 100.0, 100.0);
    zones.enter(1, a);
    drain(&mut a_in);

    let now = Instant::now();
    assert_eq!(zones.respawn(now, &world, &defs, &mut || 0), 1, "the blocked spot does not spawn");
    assert_eq!(drain(&mut a_in), [(0x0601, 1), (0x0608, 1)], "its max HP after it (docs/original/client-extras.md deviation 1)");
    assert_eq!(zones.respawn(now + std::time::Duration::from_secs(10), &world, &defs, &mut || 0), 0, "full");

    // Walking away: the monster leaves the view; back: it comes into view, not as a new one.
    let away = zones.move_to(1, 1, &step(400.0, 100.0));
    assert_eq!(
        away.iter().map(|p| (opcode(p), u32::from_le_bytes(payload(p)[..4].try_into().unwrap()))).collect::<Vec<_>>(),
        [(0x0602, 1), (0x0501, 1)]
    );
    let back = zones.move_to(1, 1, &step(100.0, 100.0));
    assert_eq!(back[..2], [w::mob_enter(1, false, 7, 100.0, 100.0), w::mob_max_hp(1, 50, 50)]);
    assert_eq!(zones.mob(1, 1).map(|m| m.hp), Some(50));
}

#[test]
fn idle_monsters_walk_near_home() {
    use crate::mobs::{MobDef, MobDefs, SpawnArea};
    use std::time::Duration;
    let mut world = World::build([(1, 2048)], &std::collections::HashSet::new(), []);
    // Only x 100, z 105 is blocked.
    let mut blocked = vec![0u8; 2048 * 2048 / 8];
    let bit = 105 * 2048 + 100;
    blocked[bit / 8] = 1 << (bit % 8);
    world.set_mob_data([(1, blocked)].into(), [(1, vec![SpawnArea::new([100.0, 0.0, 100.0, 100.0, 0.0, 100.0], vec![(7, 1)])])].into());
    let def =
        MobDef { id: 7, level: 1, hp: 50, ai: 1, normal_step: 8, chase_step: 8, normal_time: Duration::from_secs(4), ..Default::default() };
    let defs = MobDefs::new([def]);
    let zones = Zones::new(&world, &defs);
    let (a, mut a_in) = player(1, 100.0, 100.0);
    zones.enter(1, a);
    let now = Instant::now();
    assert_eq!(zones.respawn(now, &world, &defs, &mut || 0), 1, "the first turn is due at once");
    drain(&mut a_in);
    let turn = |rolls: [u32; 3], at: Instant| {
        let mut rolls = rolls.into_iter();
        zones.idle(at, &world, &defs, &mut || rolls.next().unwrap())
    };
    let spot = || zones.mob(1, 1).map(|m| (m.position[0].round(), m.position[2].round()));

    // 0 degrees, 8 units: the line to (100, 108) crosses the blocked square. The next turn is 10 + 4 + 0.5 s later.
    assert_eq!(turn([0, 0, 500], now), 0);
    assert_eq!(turn([0, 0, 0], now + Duration::from_millis(14_499)), 0, "not yet");
    // 180 degrees, 8 units: (100, 92).
    let later = now + Duration::from_millis(14_500);
    assert_eq!(turn([180, 0, 0], later), 1);
    assert_eq!(spot(), Some((100.0, 92.0)));
    assert_eq!(drain(&mut a_in), [(0x0603, 1)]);
    // 180 degrees, 12 units: (100, 80) is 20 from home.
    let later = later + Duration::from_secs(60);
    assert_eq!(turn([180, 499, 0], later), 0);
    assert_eq!(spot(), Some((100.0, 92.0)));

    // Filed under a far cell (as after a chase), the next step files it back here: the player sees it come into view.
    zones.mobs.guard().get_mut(&1).unwrap().change_cell(1, (6, 2));
    let later = later + Duration::from_secs(60);
    assert_eq!(turn([0, 0, 0], later), 1, "back to (100, 100)");
    assert_eq!(drain(&mut a_in), [(0x0601, 1), (0x0608, 1), (0x0603, 1)]);
    assert_eq!(zones.mob(1, 1).map(|m| m.cell), Some((2, 2)));
}

#[test]
fn players_fight_monsters_to_the_death() {
    use crate::combat::{self, Gear};
    use crate::mobs::{MobDef, MobDefs, SpawnArea};
    use std::time::Duration;
    let mut world = World::build([(1, 2048)], &std::collections::HashSet::new(), []);
    world.set_mob_data(
        [(1, vec![0u8; 2048 * 2048 / 8])].into(),
        [(1, vec![SpawnArea::new([100.0, 0.0, 105.0, 100.0, 0.0, 105.0], vec![(7, 1)])])].into(),
    );
    let def = MobDef { id: 7, level: 1, hp: 30, size: 1, respawn: Duration::from_secs(30), country: 2, ..Default::default() };
    let defs = MobDefs::new([def]);
    let zones = Zones::new(&world, &defs);
    let (a, mut a_in) = player(1, 100.0, 100.0);
    let (events, mut a_events) = unbounded_channel();
    zones.enter(1, Occupant { events, ..a });
    let (b, mut b_in) = player(2, 100.0, 150.0);
    zones.enter(1, b);
    let now = Instant::now();
    assert_eq!(zones.respawn(now, &world, &defs, &mut || 0), 1);
    drain(&mut a_in);
    drain(&mut b_in);

    assert!(zones.attackable(1, 1, 0, &defs));
    assert!(!zones.attackable(1, 2, 0, &defs), "no such monster");
    let fighter = combat::player(10, [100, 10, 10, 10, 10, 0], &Gear { kind: Some(combat::MELEE), ..Default::default() });
    let attack =
        Attack { attacker: 1, country: 0, fighter: &fighter, kind: combat::MELEE, slack: 2.0, range: 0, rec: 0, cross_road: false };
    let (no_drops, no_items) = (DropTables::default(), ItemDefs::default());
    let nothing = (&no_drops, &no_items);
    // 5 away: within 2 + 1 + 0 + 2.3.
    let mut rolls = [0, 99].into_iter().cycle();
    let AttackOutcome::Struck { packets, killed, critical } =
        zones.attack_mob(1, 1, &attack, &defs, nothing, now, &mut || rolls.next().unwrap())
    else {
        panic!("a hit")
    };
    assert!(killed, "130 damage against 30 HP");
    assert!(!critical, "roll 99: no critical");
    assert_eq!(packets.iter().map(|p| opcode(p)).collect::<Vec<_>>(), [0x0606, 0x0503]);
    // 0x0503 starts with the result byte (0), then the attacker (1): read as one u32, 256.
    assert_eq!(drain(&mut b_in), [(0x0606, 1), (0x0503, 256)], "B, 50 away, sees the death (view) and the blow (60)");
    assert_eq!(a_events.try_recv().ok(), Some(ZoneEvent::Kill { mob_id: 7, killer: 1 }));
    assert_eq!(zones.attack_mob(1, 1, &attack, &defs, nothing, now, &mut || 0), AttackOutcome::Gone, "dead");
    assert!(!zones.attackable(1, 1, 0, &defs));
    // The body goes after 10 s (it has a respawn time); the full entry may spawn again 30 s after that.
    assert_eq!(zones.remove_bodies(now + Duration::from_secs(9), &defs, &mut || 0), 0);
    assert_eq!(zones.remove_bodies(now + Duration::from_secs(10), &defs, &mut || 0), 1);
    assert_eq!(drain(&mut b_in), [(0x0602, 1)]);
    assert_eq!(zones.respawn(now + Duration::from_secs(39), &world, &defs, &mut || 0), 0);
    assert_eq!(zones.respawn(now + Duration::from_secs(40), &world, &defs, &mut || 0), 1);

    // Too far, and of the player's own country.
    let far = Attack { slack: 0.0, ..attack };
    let (x, z) = (100.0, 105.0);
    assert_eq!(zones.mob(1, 2).map(|m| (m.position[0], m.position[2])), Some((x, z)));
    assert_eq!(zones.attack_mob(1, 2, &Attack { attacker: 2, ..far }, &defs, nothing, now, &mut || 0), AttackOutcome::OutOfRange);
    let light = MobDefs::new([MobDef { country: 0, ..def }]);
    assert!(!zones.attackable(1, 2, 0, &light));
}

/// A map with one monster spawn at (100, 105): id 7, 1000 HP, level 5, one normal attack of 50 reaching 2.
pub(super) fn fighting_ground() -> (World, MobDef, MobDefs) {
    use crate::mobs::{MobAttack, SpawnArea};
    use std::time::Duration;
    let mut world = World::build([(1, 2048)], &std::collections::HashSet::new(), []);
    world.set_mob_data(
        [(1, vec![0u8; 2048 * 2048 / 8])].into(),
        [(1, vec![SpawnArea::new([100.0, 0.0, 105.0, 100.0, 0.0, 105.0], vec![(7, 1)])])].into(),
    );
    let mut attacks = [MobAttack::default(); 3];
    attacks[0] = MobAttack { ok: 1, damage: 50, range: 2, ..Default::default() };
    let def = MobDef {
        id: 7,
        level: 5,
        hp: 1000,
        size: 1,
        country: 2,
        chase_step: 8,
        normal_step: 4,
        chase_time: Duration::from_secs(1),
        chase_range: 20,
        recover: 10,
        attacks,
        ..Default::default()
    };
    let defs = MobDefs::new([def]);
    (world, def, defs)
}

#[test]
fn monsters_use_skills_and_debuff_players() {
    use crate::combat::{self, Gear};
    use crate::mobs::MobAttack;
    use std::time::Duration;
    let (world, mut def, _) = fighting_ground();
    let stun = SkillDef {
        id: 64,
        level: skills::MOB_LEVEL,
        type_effect: skills::DEBUFF,
        kind: buffs::STUN,
        target_type: 3,
        success_type: 1,
        success_value: 100,
        attack_range: 10,
        keep_time: 5,
        reset_time: 30,
        ..Default::default()
    };
    def.attacks[1] = MobAttack { skill: 64, ok: 1, info: Some(stun), ..Default::default() };
    let defs = MobDefs::new([def]);
    let zones = Zones::new(&world, &defs);
    let (a, mut a_in) = player(1, 100.0, 102.0);
    let (events, mut a_events) = unbounded_channel();
    zones.enter(1, Occupant { events, ..a });
    let now = Instant::now();
    zones.respawn(now, &world, &defs, &mut || 0);
    let fighter = combat::player(10, [10; 6], &Gear { kind: Some(combat::MELEE), ..Default::default() });
    let attack =
        Attack { attacker: 1, country: 0, fighter: &fighter, kind: combat::MELEE, slack: 2.0, range: 0, rec: 5, cross_road: false };
    let (no_drops, no_items) = (DropTables::default(), ItemDefs::default());
    zones.attack_mob(1, 1, &attack, &defs, (&no_drops, &no_items), now, &mut || 0);
    drain(&mut a_in);
    while a_events.try_recv().is_ok() {}

    // Taking the target gave the skill entry a ready time below its 30 s; a second later it is used: the stun hits (0x060B, no
    // damage), breaks a cast, and is applied (0x050D).
    let later = now + Duration::from_secs(1);
    zones.mob_fight(later, &world, &defs, no_loot(), &mut || 0);
    let seen: Vec<u16> = drain(&mut a_in).into_iter().map(|(op, _)| op).collect();
    assert_eq!(seen, [sk::OP_MOB_SKILL, sk::OP_APPLIED]);
    assert_eq!(a_events.try_recv().ok(), Some(ZoneEvent::Struck));
    assert_eq!(a_events.try_recv().ok(), Some(ZoneEvent::Worn), "the blow wears the armour");
    assert_eq!(a_events.try_recv().ok(), Some(ZoneEvent::Buffs));
    let effects = zones.buff_effects(1, 1).unwrap();
    assert!(effects.stun);
    assert_eq!(effects.speeds((5, 2)), (buffs::CANNOT, buffs::CANNOT));
    assert_eq!(zones.mob(1, 1).map(|m| m.entry_ready[1]), Some(Some(later + Duration::from_secs(30))));
    // Its time is up: 0x050E, and the player can move again.
    zones.recover(later + Duration::from_secs(6), &defs, no_loot(), &mut || 0);
    assert!(drain(&mut a_in).iter().any(|&(op, _)| op == sk::OP_APPLIED_END));
    assert!(!zones.buff_effects(1, 1).unwrap().stun);
}

#[test]
fn monsters_fight_back_and_kill() {
    use crate::combat::{self, Gear};
    let (world, _, defs) = fighting_ground();
    let zones = Zones::new(&world, &defs);
    let (mut a, mut a_in) = player(1, 100.0, 102.0);
    a.vitals.health = [60, 100, 100];
    let (events, mut a_events) = unbounded_channel();
    zones.enter(1, Occupant { events, ..a });
    let now = Instant::now();
    zones.respawn(now, &world, &defs, &mut || 0);
    drain(&mut a_in);

    // Idle and not aggressive: nothing happens.
    zones.mob_fight(now, &world, &defs, no_loot(), &mut || 0);
    assert!(drain(&mut a_in).is_empty());
    // A hit that does not kill makes it fight back.
    let fighter = combat::player(10, [10; 6], &Gear { kind: Some(combat::MELEE), ..Default::default() });
    let attack =
        Attack { attacker: 1, country: 0, fighter: &fighter, kind: combat::MELEE, slack: 2.0, range: 0, rec: 5, cross_road: false };
    let (no_drops, no_items) = (DropTables::default(), ItemDefs::default());
    let mut rolls = [0, 99].into_iter().cycle();
    let outcome = zones.attack_mob(1, 1, &attack, &defs, (&no_drops, &no_items), now, &mut || rolls.next().unwrap());
    assert!(matches!(outcome, AttackOutcome::Struck { killed: false, .. }));
    assert_eq!(zones.mob(1, 1).map(|m| m.state), Some(MobState::Fighting(1)));
    drain(&mut a_in);
    // 3 away: within 1 + 2 + 1.5. Hit (roll 0), no critical (99): (50 + 50 * 0.305) - 0 = 65.25, more than the 60 HP left.
    let mut rolls = [0, 99].into_iter().cycle();
    zones.mob_fight(now, &world, &defs, no_loot(), &mut || rolls.next().unwrap());
    // 0x0504 (id 1), then 0x0605 (result 0, monster 1: 256 read as one u32).
    assert_eq!(drain(&mut a_in), [(0x0504, 1), (0x0605, 256)]);
    assert_eq!(a_events.try_recv().ok(), Some(ZoneEvent::Struck), "a hit breaks a cast");
    assert_eq!(a_events.try_recv().ok(), Some(ZoneEvent::Worn), "and wears the armour");
    assert_eq!(
        a_events.try_recv().ok(),
        Some(ZoneEvent::Died { killer_kind: cp::KILLER_MOB, killer: 1, loss: None, counted: false, keep_exp: false, drop_exp: 5 })
    );
    assert_eq!(zones.health(1, 1), Some([0; 3]));
    assert_eq!(zones.mob(1, 1).map(|m| m.state), Some(MobState::Idle), "it drops a dead target");

    // Back to life where it stands: full, 0x0551 and 0x0505 with what was gained.
    let back = zones.revive_here(1, 1, [100.0, 0.0, 102.0], 77, cp::REBIRTH_BIND, false);
    assert_eq!(back.iter().map(|p| opcode(p)).collect::<Vec<_>>(), [0x0551, 0x0505]);
    assert_eq!(payload(&back[1])[4..], [100, 0, 100, 0, 100, 0]);
    assert_eq!(zones.health(1, 1), Some([100; 3]));
}

#[test]
fn monsters_chase_and_go_home() {
    use crate::combat::{self, Gear};
    use std::time::Duration;
    let (world, _, defs) = fighting_ground();
    let zones = Zones::new(&world, &defs);
    let (a, mut a_in) = player(1, 100.0, 140.0);
    zones.enter(1, a);
    let now = Instant::now();
    zones.respawn(now, &world, &defs, &mut || 0);
    let fighter = combat::player(10, [10; 6], &Gear { kind: Some(combat::MELEE), ..Default::default() });
    let attack =
        Attack { attacker: 1, country: 0, fighter: &fighter, kind: combat::MELEE, slack: 40.0, range: 0, rec: 0, cross_road: false };
    let (no_drops, no_items) = (DropTables::default(), ItemDefs::default());
    let mut rolls = [0, 99].into_iter().cycle();
    zones.attack_mob(1, 1, &attack, &defs, (&no_drops, &no_items), now, &mut || rolls.next().unwrap());
    drain(&mut a_in);
    // 35 away: a full chase step of 8 towards the player, running (0x0603 motion 1).
    zones.mob_fight(now, &world, &defs, no_loot(), &mut || 0);
    assert_eq!(zones.mob(1, 1).map(|m| m.position), Some([100.0, 0.0, 113.0]));
    assert_eq!(drain(&mut a_in), [(0x0603, 1)]);
    // Not before the chase time.
    zones.mob_fight(now + Duration::from_millis(500), &world, &defs, no_loot(), &mut || 0);
    assert_eq!(zones.mob(1, 1).map(|m| m.position), Some([100.0, 0.0, 113.0]));
    // The player runs off: more than 45 from the monster's home, so it goes home.
    zones.move_to(1, 1, &step(100.0, 160.0));
    zones.mob_fight(now + Duration::from_secs(1), &world, &defs, no_loot(), &mut || 0);
    assert_eq!(zones.mob(1, 1).map(|m| m.state), Some(MobState::Home));
    // It can no longer be hit.
    let mut rolls = [0, 99].into_iter().cycle();
    let far = Attack { slack: 100.0, ..attack };
    let AttackOutcome::Struck { packets, .. } =
        zones.attack_mob(1, 1, &far, &defs, (&no_drops, &no_items), now, &mut || rolls.next().unwrap())
    else {
        panic!()
    };
    assert_eq!(payload(packets.last().unwrap())[0], combat::RESULT_MISS);
    drain(&mut a_in);
    // 8 from home: home. HP full again; 0x0305 and 0x0609 reach the player 47 away (within 60).
    zones.mob_fight(now + Duration::from_secs(2), &world, &defs, no_loot(), &mut || 0);
    let mob = zones.mob(1, 1).unwrap();
    assert_eq!((mob.state, mob.hp), (MobState::Idle, 1000));
    assert_eq!(drain(&mut a_in), [(0x0305, 1), (0x0609, 1)]);
}

#[test]
fn aggressive_monsters_look_and_others_help() {
    use crate::mobs::SpawnArea;
    let (mut world, def, _) = fighting_ground();
    // Two of the aggressive kind (AI 1) and one that helps its own kind (AI 2, id 8), side by side.
    let area = |x| SpawnArea::new([x, 0.0, 105.0, x, 0.0, 105.0], vec![(7, 1), (8, 1)]);
    world.set_mob_data([(1, vec![0u8; 2048 * 2048 / 8])].into(), [(1, vec![area(100.0), area(110.0)])].into());
    let defs = MobDefs::new([MobDef { ai: 1, ..def }, MobDef { id: 8, ai: 2, ..def }]);
    let zones = Zones::new(&world, &defs);
    let now = Instant::now();
    zones.respawn(now, &world, &defs, &mut || 0);
    let (a, _a_in) = player(1, 100.0, 120.0);
    zones.enter(1, a);
    let state = |id| zones.mob(1, id).map(|m| m.state);
    // They only look 5 s after spawning; then the first aggressive one within 20 takes the player.
    zones.idle(now, &world, &defs, &mut || 0);
    assert_eq!(state(1), Some(MobState::Idle));
    zones.idle(now + std::time::Duration::from_secs(5), &world, &defs, &mut || 0);
    assert_eq!((state(1), state(3)), (Some(MobState::Fighting(1)), Some(MobState::Fighting(1))), "both aggressive ones look");
    assert_eq!(state(2), Some(MobState::Idle), "AI 2 does not look");

    // Hitting an AI 2 monster calls the others of its kind within its chase range (20): monster 4, 10 away.
    let fighter = crate::combat::player(10, [10; 6], &crate::combat::Gear { kind: Some(crate::combat::MELEE), ..Default::default() });
    let attack =
        Attack { attacker: 1, country: 0, fighter: &fighter, kind: crate::combat::MELEE, slack: 20.0, range: 0, rec: 0, cross_road: false };
    let (no_drops, no_items) = (DropTables::default(), ItemDefs::default());
    let mut rolls = [0, 99].into_iter().cycle();
    zones.attack_mob(1, 2, &attack, &defs, (&no_drops, &no_items), now, &mut || rolls.next().unwrap());
    assert_eq!((state(2), state(4)), (Some(MobState::Fighting(1)), Some(MobState::Fighting(1))));

    // Players of the monster's own country are passed over.
    let light = MobDefs::new([MobDef { ai: 1, country: 0, ..def }]);
    let zones = Zones::new(&world, &light);
    zones.respawn(now, &world, &light, &mut || 0);
    let (a, _a_in) = player(1, 100.0, 120.0);
    zones.enter(1, a);
    zones.idle(now + std::time::Duration::from_secs(5), &world, &light, &mut || 0);
    assert_eq!(zones.mob(1, 1).map(|m| m.state), Some(MobState::Idle));
}

/// `count` of the monster of `fighting_ground` at (100, 105), and a player at (100, 102) with 100 of everything; a second
/// player 45 away watches.
fn skill_ground(count: u32) -> (Zones, MobDefs, crate::outbox::OutboxInbox, UnboundedReceiver<ZoneEvent>) {
    use crate::mobs::SpawnArea;
    let (mut world, def, _) = fighting_ground();
    world.set_mob_data(
        [(1, vec![0u8; 2048 * 2048 / 8])].into(),
        [(1, vec![SpawnArea::new([100.0, 0.0, 105.0, 100.0, 0.0, 105.0], vec![(7, count)])])].into(),
    );
    let defs = MobDefs::new([MobDef { sp: 20, ..def }]);
    let zones = Zones::new(&world, &defs);
    let (a, a_in) = player(1, 100.0, 102.0);
    let (events, a_events) = unbounded_channel();
    zones.enter(1, Occupant { events, ..a });
    let (b, mut b_in) = player(2, 100.0, 147.0);
    zones.enter(1, b);
    // One spawns per pass.
    for _ in 0..count {
        zones.respawn(Instant::now(), &world, &defs, &mut || 0);
    }
    drain(&mut b_in);
    // The caster's own packets come back from `use_skill`; what reaches its outbox is not looked at.
    drop(a_in);
    (zones, defs, b_in, a_events)
}

/// A skill that always hits for `damage` as it is (damage type 0), costing 10 SP.
fn sure_skill(type_effect: u8, target_type: u8, kind: u16, damage: [u16; 3]) -> SkillDef {
    SkillDef {
        id: 30,
        level: 1,
        type_effect,
        target_type,
        kind,
        damage,
        success_type: 1,
        success_value: 100,
        need_sp: 10,
        ..Default::default()
    }
}

fn use_on(zones: &Zones, defs: &MobDefs, target: SkillTarget, skill: &SkillDef) -> Option<SkillOutcome> {
    use_with(zones, defs, target, skill, None, 0)
}

/// As `use_on`, with what a disguise copies and an illusion's monster.
fn use_with(
    zones: &Zones,
    defs: &MobDefs,
    target: SkillTarget,
    skill: &SkillDef,
    look: Option<&UserShape>,
    illusion_mob: u16,
) -> Option<SkillOutcome> {
    let fighter = Fighter { level: 10, wis: 10, ..Default::default() };
    let cast = SkillCast {
        caster: 1,
        country: 0,
        skill,
        fighter: &fighter,
        rec: 0,
        cross_road: false,
        cost_reduction: [0; 2],
        fighting: None,
        look,
        illusion_mob,
        war_map: false,
    };
    let (no_drops, no_items) = (DropTables::default(), ItemDefs::default());
    zones.use_skill(1, target, &cast, defs, (&no_drops, &no_items), Instant::now(), &mut || 0)
}

/// Changes a player in the zone (tests only).
fn edit(zones: &Zones, id: u32, change: impl FnOnce(&mut Placed)) {
    change(zones.maps.guard().get_mut(&1).unwrap().get_mut(&id).unwrap());
}

fn opcodes(packets: &[Vec<u8>]) -> Vec<u16> {
    packets.iter().map(|p| opcode(p)).collect()
}

#[test]
fn skills_hit_one_monster() {
    let (zones, defs, mut b_in, mut a_events) = skill_ground(1);
    let one = sure_skill(skills::ATTACK, skills::TARGET_ONE, 21, [300, 5, 0]);
    let out = use_on(&zones, &defs, SkillTarget::Mob(1), &one).unwrap();
    assert_eq!(out.packets, [sk::skill_use(sk::OP_USE_MOB, 0, 1, 1, 30, 1, [300, 5, 0]), sk::costs(10, 0)]);
    assert_eq!((out.killed, out.died), (false, false));
    let mob = zones.mob(1, 1).unwrap();
    assert_eq!((mob.hp, mob.sp, mob.state), (700, 15, MobState::Fighting(1)));
    assert_eq!(zones.health(1, 1), Some([100, 100, 90]), "10 SP spent");
    // The watcher, 45 away, sees the hit (within 60) but not the costs.
    assert_eq!(drain(&mut b_in), [(sk::OP_USE_MOB, 256)]);

    // Additional damage after the hit; a miss shows nothing but result 3.
    let add = SkillDef { add_damage: [50, 0, 0], ..one };
    let out = use_on(&zones, &defs, SkillTarget::Mob(1), &add).unwrap();
    assert_eq!(opcodes(&out.packets), [sk::OP_USE_MOB, sk::OP_ADD_MOB, sk::OP_COSTS]);
    assert_eq!(out.packets[1], sk::add_damage(1, 1, 30, 1, [50, 0, 0]));
    assert_eq!(zones.mob(1, 1).unwrap().hp, 350);
    let miss = SkillDef { success_value: 0, ..one };
    let out = use_on(&zones, &defs, SkillTarget::Mob(1), &miss).unwrap();
    assert_eq!(out.packets[0], sk::skill_use(sk::OP_USE_MOB, combat::RESULT_MISS, 1, 1, 30, 1, [0; 3]));
    assert_eq!(zones.mob(1, 1).unwrap().hp, 350);

    // A drain heals the caster by the damage, up to its maxima.
    zones.maps.guard().get_mut(&1).unwrap().get_mut(&1).unwrap().occupant.vitals.health = [50, 100, 100];
    let drain_skill = SkillDef { kind: skills::DRAIN, damage: [30, 0, 4], need_sp: 0, ..one };
    let out = use_on(&zones, &defs, SkillTarget::Mob(1), &drain_skill).unwrap();
    assert_eq!(out.packets, [sk::drain(0, 1, 1, 30, 1, [30, 0, 4], [30, 0, 0])], "MP was full; no costs");
    assert_eq!(zones.health(1, 1), Some([80, 100, 100]));

    // A kill (320 HP left): 0x0606 first, then the hit; the player with the most damage gets the experience.
    let out = use_on(&zones, &defs, SkillTarget::Mob(1), &SkillDef { damage: [400, 0, 0], ..one }).unwrap();
    assert!(out.killed);
    assert_eq!(opcodes(&out.packets), [cp::OP_MOB_DEATH, sk::OP_USE_MOB, sk::OP_COSTS]);
    assert_eq!(a_events.try_recv().ok(), Some(ZoneEvent::Kill { mob_id: 7, killer: 1 }));
    assert_eq!(use_on(&zones, &defs, SkillTarget::Mob(1), &one), None, "dead");
}

#[test]
fn multi_hits_areas_and_the_desperate_counterattack() {
    let (zones, defs, _, _) = skill_ground(3);
    // Three hits on one monster: the area animation on it, then three area hits.
    let multi = SkillDef { multi_attack: 3, ..sure_skill(skills::ATTACK, skills::TARGET_ONE, skills::MULTI_HIT, [100, 0, 0]) };
    let out = use_on(&zones, &defs, SkillTarget::Mob(2), &multi).unwrap();
    assert_eq!(out.packets[0], sk::skill_use(sk::OP_USE_MOB, skills::RESULT_AREA, 1, 2, 30, 1, [0; 3]));
    assert_eq!(opcodes(&out.packets[1..]), [sk::OP_AREA_MOB, sk::OP_AREA_MOB, sk::OP_AREA_MOB, sk::OP_COSTS]);
    assert_eq!(zones.mob(1, 2).unwrap().hp, 700);

    // Around the target: all three within 5 of it.
    let around = SkillDef { apply_range: 5, ..sure_skill(skills::DEBUFF, skills::TARGET_AROUND_TARGET, 61, [10, 0, 0]) };
    let out = use_on(&zones, &defs, SkillTarget::Mob(2), &around).unwrap();
    assert_eq!(out.packets[0], sk::skill_use(sk::OP_USE_MOB, skills::RESULT_AREA, 1, 2, 30, 1, [0; 3]));
    assert_eq!(opcodes(&out.packets[1..]), [sk::OP_AREA_MOB; 3].into_iter().chain([sk::OP_COSTS]).collect::<Vec<_>>());
    assert_eq!([1, 2, 3].map(|id| zones.mob(1, id).unwrap().hp), [990, 690, 990]);
    // Around oneself, 3 from them: within 2, nothing is hit; within 4, all.
    let mine = |apply_range| SkillDef { apply_range, ..sure_skill(skills::ATTACK, skills::TARGET_AROUND_ONESELF, 21, [10, 0, 0]) };
    let out = use_on(&zones, &defs, SkillTarget::Player(1), &mine(2)).unwrap();
    assert_eq!(out.packets, [sk::skill_use(sk::OP_USE_USER, skills::RESULT_AREA, 1, 0, 30, 1, [0; 3]), sk::costs(10, 0)]);
    let out = use_on(&zones, &defs, SkillTarget::Player(1), &mine(4)).unwrap();
    assert_eq!(out.packets.len(), 1 + 3 + 1);

    // The desperate counterattack: the hit, then the caster dies by its own hand.
    let desperate = sure_skill(skills::ATTACK, skills::TARGET_ONE, skills::DESPERATE, [10, 0, 0]);
    let out = use_on(&zones, &defs, SkillTarget::Mob(1), &desperate).unwrap();
    assert!(out.died);
    assert_eq!(opcodes(&out.packets), [sk::OP_USE_MOB, cp::OP_DEATH, sk::OP_COSTS]);
    assert_eq!(out.packets[1], cp::death(1, cp::KILLER_PLAYER, 1));
    assert_eq!(out.packets[2], sk::costs(0, 0), "nothing left to pay with");
    assert_eq!(zones.health(1, 1), Some([0; 3]));
    assert_eq!(use_on(&zones, &defs, SkillTarget::Player(1), &mine(4)), None, "dead");
}

#[test]
fn heals_and_sacrifices() {
    let (zones, defs, _, _) = skill_ground(0);
    zones.maps.guard().get_mut(&1).unwrap().get_mut(&1).unwrap().occupant.vitals.health = [50, 90, 100];
    // HP heal 20 plus WIS 10 x 2; MP 5.
    let heal = SkillDef { heal: [20, 0, 5], ..sure_skill(skills::HEAL, skills::TARGET_ONESELF, skills::HEAL_HEALTH, [0; 3]) };
    let out = use_on(&zones, &defs, SkillTarget::Player(1), &heal).unwrap();
    assert_eq!(out.packets, [sk::skill_use(sk::OP_USE_USER, 0, 1, 1, 30, 1, [40, 0, 5]), sk::costs(10, 0)]);
    assert_eq!(zones.health(1, 1), Some([90, 95, 90]));
    // Level 2 sacrifice on the other player: the caster's maximum SP and MP, all the caster's SP as the price.
    zones.maps.guard().get_mut(&1).unwrap().get_mut(&2).unwrap().occupant.vitals.health = [100, 10, 20];
    let sacrifice = SkillDef { level: 2, need_sp: 1, ..sure_skill(skills::HEAL, skills::TARGET_ONE, skills::SACRIFICE, [0; 3]) };
    let out = use_on(&zones, &defs, SkillTarget::Player(2), &sacrifice).unwrap();
    assert_eq!(out.packets, [sk::skill_use(sk::OP_USE_USER, 0, 1, 2, 30, 2, [0, 80, 90]), sk::costs(90, 0)]);
    assert_eq!(zones.health(1, 2), Some([100, 100, 100]));
    assert_eq!(zones.health(1, 1), Some([90, 95, 0]));
    // The party around, without a party: only the animation.
    let party = sure_skill(skills::HEAL, skills::TARGET_PARTY_AROUND, skills::HEAL_HEALTH, [0; 3]);
    let out = use_on(&zones, &defs, SkillTarget::Player(1), &SkillDef { need_sp: 0, heal: [50, 0, 0], ..party }).unwrap();
    assert_eq!(out.packets, [sk::skill_use(sk::OP_USE_USER, skills::RESULT_AREA, 1, 0, 30, 1, [0; 3])]);
}

/// A buff on oneself lasting `keep` seconds.
fn sure_buff(target_type: u8, kind: u16, keep: u32) -> SkillDef {
    SkillDef { keep_time: keep, need_sp: 0, ..sure_skill(skills::BUFF, target_type, kind, [0; 3]) }
}

#[test]
fn buffs_apply_refresh_cancel_and_end() {
    use std::time::Duration;
    let (zones, defs, _, mut a_events) = skill_ground(0);
    let strength = SkillDef { abilities: [(4, 10), (0, 0), (0, 0)], ..sure_buff(skills::TARGET_ONESELF, buffs::ABILITIES, 10) };
    let out = use_on(&zones, &defs, SkillTarget::Player(1), &strength).unwrap();
    assert_eq!(out.packets, [sk::skill_use(sk::OP_USE_USER, 0, 1, 1, 30, 1, [0; 3]), sk::applied(1, 30, 1)]);
    assert_eq!(a_events.try_recv().ok(), Some(ZoneEvent::Buffs));
    assert_eq!(zones.buff_effects(1, 1).unwrap().bonuses.stats[0], 10);
    // Again: a refresh under the same key.
    let out = use_on(&zones, &defs, SkillTarget::Player(1), &strength).unwrap();
    assert_eq!(out.packets[1], sk::applied(1, 30, 1));
    // Through the party type without a party: the area animation, then 0x0513 on oneself.
    let party = SkillDef { id: 31, ..sure_buff(skills::TARGET_PARTY, buffs::ABILITIES, 10) };
    let out = use_on(&zones, &defs, SkillTarget::Player(1), &party).unwrap();
    assert_eq!(opcodes(&out.packets), [sk::OP_USE_USER, sk::OP_USE_PARTY, sk::OP_APPLIED]);
    assert_eq!(payload(&out.packets[0])[0], skills::RESULT_AREA);
    // Cancelling: only a buff the player has; a heal over time pays for the time it ran (6 s: two ticks of 7).
    assert!(zones.cancel_buff(1, 1, 99, Instant::now()).is_empty());
    zones.maps.guard().get_mut(&1).unwrap().get_mut(&1).unwrap().occupant.vitals.health = [50, 100, 100];
    let regen = SkillDef { id: 124, time_heal: [7, 0, 0], ..sure_buff(skills::TARGET_ONESELF, buffs::HEAL_OVER_TIME, 18) };
    let out = use_on(&zones, &defs, SkillTarget::Player(1), &regen).unwrap();
    assert_eq!(out.packets[1], sk::applied(3, 124, 1));
    let back = zones.cancel_buff(1, 1, 3, Instant::now() + Duration::from_secs(6));
    assert_eq!(back, [sk::applied_end(3), sk::keep(sk::OP_KEEP_USER, 1, 124, 1, [14, 0, 0])]);
    assert_eq!(zones.health(1, 1), Some([64, 100, 100]));
    // The others run out in the one-second pass.
    while a_events.try_recv().is_ok() {}
    zones.recover(Instant::now() + Duration::from_secs(11), &defs, no_loot(), &mut || 0);
    assert_eq!(a_events.try_recv().ok(), Some(ZoneEvent::Buffs));
    assert_eq!(zones.buff_effects(1, 1), Some(buffs::BuffEffects::default()));
}

#[test]
fn hiding_shows_and_ends_by_attacking_but_evolution_stays() {
    let (zones, defs, mut b_in, _) = skill_ground(0);
    let hide = sure_buff(skills::TARGET_ONESELF, buffs::HIDE, 10);
    let out = use_on(&zones, &defs, SkillTarget::Player(1), &hide).unwrap();
    assert_eq!(opcodes(&out.packets), [sk::OP_USE_USER, sk::OP_APPLIED]);
    assert_eq!(zones.whereabouts(1, 1).unwrap().shape, sk::SHAPE_HIDDEN);
    assert!(received(&mut b_in).into_iter().any(|p| p == sk::shape(1, sk::SHAPE_HIDDEN)), "the view sees the shape");
    // A player coming into view gets the shape after the 0x0201.
    let entered = zones.enter(1, player(4, 100.0, 104.0).0);
    let at = entered.iter().position(|p| opcode(p) == w::OP_USER_ENTER && payload(p)[..4] == 1u32.to_le_bytes()).unwrap();
    assert_eq!(entered[at + 2], sk::shape(1, sk::SHAPE_HIDDEN), "after its 0x0201 and 0x051C");
    // Attacking ends it.
    drain(&mut b_in);
    assert!(zones.end_shape_by_attack(1, 1));
    assert_eq!(zones.whereabouts(1, 1).unwrap().shape, 0);
    assert!(received(&mut b_in).into_iter().any(|p| p == sk::shape(1, 0)));
    assert!(!zones.end_shape_by_attack(1, 1), "nothing left to end");
    // Evolution at levels 1 to 3 survives fighting; its monster's numbers come with it.
    let evolve = SkillDef { id: 127, change_type: 10, change_level: 7, ..sure_buff(skills::TARGET_ONE, skills::TRANSFORMATION, 60) };
    use_on(&zones, &defs, SkillTarget::Player(1), &evolve).unwrap();
    let effects = zones.buff_effects(1, 1).unwrap();
    assert_eq!((effects.shape, effects.shape_mob), (10, 7));
    assert!(!zones.end_shape_by_attack(1, 1));
    // Hiding pushes the transformation off (one at a time).
    use_on(&zones, &defs, SkillTarget::Player(1), &hide).unwrap();
    assert_eq!(zones.buff_effects(1, 1).unwrap().shape, sk::SHAPE_HIDDEN);
}

#[test]
fn detect_finds_hidden_enemies_close_by() {
    let (zones, defs, _, _) = skill_ground(0);
    let hide = sure_buff(skills::TARGET_ONESELF, buffs::HIDE, 10);
    let hidden = |id: u32, country: u8, z: f32| {
        let (mut o, inbox) = player(id, 100.0, z);
        o.shape.country = country;
        o.buffs.apply(&hide, id, Instant::now(), &mut || 50 + id);
        zones.enter(1, o);
        inbox
    };
    let mut enemy = hidden(3, 1, 106.0);
    let _friend = hidden(4, 0, 106.0);
    let _far = hidden(5, 1, 125.0);
    drain(&mut enemy);
    let detect = SkillDef { apply_range: 5, ..sure_buff(skills::TARGET_ONESELF, buffs::DETECT, 30) };
    use_on(&zones, &defs, SkillTarget::Player(1), &detect).unwrap();
    let shapes = [3, 4, 5].map(|id| zones.whereabouts(1, id).unwrap().shape);
    assert_eq!(shapes, [0, sk::SHAPE_HIDDEN, sk::SHAPE_HIDDEN], "the enemy within 5 only");
    let seen = received(&mut enemy);
    assert!(seen.contains(&sk::applied_end(53)) && seen.contains(&sk::shape(3, 0)) && seen.contains(&sk::run_mode(true)));
    // Every three seconds while it lasts.
    edit(&zones, 5, |p| p.occupant.position = [100.0, 0.0, 105.0]);
    zones.keep_effects(Instant::now(), &World::default(), &defs, no_loot(), &mut || 0);
    assert_eq!(zones.whereabouts(1, 5).unwrap().shape, 0);
}

#[test]
fn disguises_and_illusions_show_other_looks() {
    let (zones, defs, mut b_in, _) = skill_ground(1);
    edit(&zones, 2, |p| {
        p.occupant.shape.country = 1;
        p.occupant.shape.name = b"Enemy".to_vec();
    });
    assert_eq!(zones.copy_looks(1, 1, 2, 0, 10).map(|l| (l.id, l.name)), Some((1, b"Enemy".to_vec())));
    assert_eq!(zones.copy_looks(1, 1, 2, 1, 10), None, "own country");
    edit(&zones, 2, |p| p.occupant.vitals.guard.level = 11);
    assert_eq!(zones.copy_looks(1, 1, 2, 0, 10), None, "higher level");
    edit(&zones, 2, |p| p.occupant.vitals.guard.level = 10);
    let look = zones.copy_looks(1, 1, 2, 0, 10).unwrap();
    let disguise = SkillDef { change_type: 100, ..sure_buff(skills::TARGET_ONESELF, skills::TRANSFORMATION, 180) };
    drain(&mut b_in);
    use_with(&zones, &defs, SkillTarget::Player(1), &disguise, Some(&look), 0).unwrap();
    let seen = received(&mut b_in);
    assert!(seen.contains(&sk::shape(1, sk::SHAPE_DISGUISE)) && seen.contains(&w::user_shape(&look)));
    assert_eq!(zones.shape(1, 1, 1)[1], w::user_shape(&look), "the looks reply shows the copy");
    // An illusion of a monster of a level not above the player's: 0x0524 with it.
    assert_eq!(zones.illusion_mob(1, 1, 5, &defs), Some(7));
    assert_eq!(zones.illusion_mob(1, 1, 4, &defs), None);
    let illusion = SkillDef { id: 77, change_type: 102, ..sure_buff(skills::TARGET_ONESELF, skills::TRANSFORMATION, 60) };
    use_with(&zones, &defs, SkillTarget::Player(1), &illusion, None, 7).unwrap();
    let seen = received(&mut b_in);
    assert!(seen.contains(&sk::shape_mob(1, 7)));
    assert!(seen.contains(&w::user_shape(&UserShape { id: 1, ..Default::default() })), "the disguise's end shows the own looks");
    let effects = zones.buff_effects(1, 1).unwrap();
    assert_eq!((effects.shape, effects.shape_mob), (102, 7));
}

#[test]
fn fields_hit_the_monsters_around() {
    use std::time::Duration;
    let (zones, defs, mut b_in, _) = skill_ground(1);
    let trap = SkillDef { apply_range: 2, state_type: 7, damage_type: 5, ..sure_skill(skills::FIELD, 1, skills::TRAP, [15, 0, 0]) };
    let trap = SkillDef { keep_time: 30, ..trap };
    let out = use_on(&zones, &defs, SkillTarget::Player(1), &trap).unwrap();
    assert_eq!(out.packets[0], sk::skill_use(sk::OP_USE_USER, 0, 1, 0, 30, 1, [0; 3]));
    // The trap is at the caster's feet (100, 102), the monster 3 away: out of reach.
    let now = Instant::now();
    zones.fields(now, &World::default(), &defs, no_loot(), &mut || 0);
    assert_eq!(zones.mob(1, 1).unwrap().hp, 1000);
    let fire_wall = SkillDef { apply_range: 4, keep_time: 12, ..sure_skill(skills::FIELD, 1, skills::FIRE_WALL, [32, 0, 0]) };
    use_on(&zones, &defs, SkillTarget::Player(1), &fire_wall).unwrap();
    let trap = SkillDef { apply_range: 4, ..trap };
    use_on(&zones, &defs, SkillTarget::Player(1), &trap).unwrap();
    drain(&mut b_in);
    // Now both reach, in the order laid: the fire wall's 32, then 15 % of the 968 left.
    zones.fields(now, &World::default(), &defs, no_loot(), &mut || 0);
    assert_eq!(zones.mob(1, 1).unwrap().hp, 1000 - 32 - 145);
    assert_eq!(drain(&mut b_in).iter().filter(|(op, _)| *op == sk::OP_AREA_MOB).count(), 2);
    assert!(matches!(zones.mob(1, 1).unwrap().state, MobState::Fighting(1)), "it turns on the owner");
    // The traps are used up (or out of reach); the fire wall goes on until its time is up.
    zones.fields(now + Duration::from_secs(1), &World::default(), &defs, no_loot(), &mut || 0);
    assert_eq!(zones.mob(1, 1).unwrap().hp, 1000 - 32 - 145 - 32);
    // Past its time it hits once more, in the pass that removes it (`CZone::UpdateTrab` checks the end last).
    zones.fields(now + Duration::from_secs(13), &World::default(), &defs, no_loot(), &mut || 0);
    zones.fields(now + Duration::from_secs(14), &World::default(), &defs, no_loot(), &mut || 0);
    assert_eq!(zones.mob(1, 1).unwrap().hp, 1000 - 32 - 145 - 32 - 32);
    // Only the first trap, out of reach, is left until its 30 s are up.
    assert_eq!(zones.fields.guard()[&1].iter().map(|f| f.skill.apply_range).collect::<Vec<_>>(), [2]);
    zones.fields(now + Duration::from_secs(31), &World::default(), &defs, no_loot(), &mut || 0);
    assert!(zones.fields.guard().is_empty());
}

/// Player 2 (45 from player 1, the caster) of the other country, and the Fire Thorns buff on player 1.
fn thorns_ground(damage: u16) -> (Zones, MobDefs, crate::outbox::OutboxInbox, UnboundedReceiver<ZoneEvent>) {
    let (zones, defs, b_in, a_ev) = skill_ground(0);
    edit(&zones, 2, |p| p.occupant.shape.country = 1);
    let thorns = SkillDef { id: 47, apply_range: 50, damage: [damage, 0, 0], ..sure_buff(skills::TARGET_ONESELF, buffs::FIRE_THORNS, 300) };
    use_on(&zones, &defs, SkillTarget::Player(1), &thorns).unwrap();
    (zones, defs, b_in, a_ev)
}

#[test]
fn fire_thorns_hit_enemy_players() {
    // docs/original/thorns-fields-pvp.md section 1.
    use std::time::Duration;
    let (zones, defs, mut b_in, _) = thorns_ground(8);
    let now = Instant::now();
    let world = World::default();
    drain(&mut b_in);
    zones.keep_effects(now + Duration::from_secs(3), &world, &defs, no_loot(), &mut || 0);
    assert_eq!(zones.health(1, 2).unwrap()[0], 92);
    assert_eq!(received(&mut b_in), [sk::skill_use(sk::OP_USE_PARTY, combat::RESULT_HIT, 1, 2, 47, 1, [8, 0, 0])]);
    // The caster's own country, a target beyond reach and a staff member who cannot be attacked are not hit.
    let (c, _c_in) = player(3, 100.0, 103.0);
    zones.enter(1, c);
    edit(&zones, 3, |p| p.occupant.shape.country = 0);
    edit(&zones, 2, |p| p.occupant.position = [100.0, 0.0, 160.0]);
    zones.keep_effects(now + Duration::from_secs(6), &world, &defs, no_loot(), &mut || 0);
    assert_eq!((zones.health(1, 2).unwrap()[0], zones.health(1, 3).unwrap()[0]), (92, 100));
    edit(&zones, 2, |p| {
        p.occupant.position = [100.0, 0.0, 147.0];
        p.occupant.pvp = false;
    });
    zones.keep_effects(now + Duration::from_secs(9), &world, &defs, no_loot(), &mut || 0);
    assert_eq!(zones.health(1, 2).unwrap()[0], 92, "not attackable");
    // A war map: nobody is hit.
    edit(&zones, 2, |p| p.occupant.pvp = true);
    let mut war = World::default();
    war.set_war_maps([1]);
    zones.keep_effects(now + Duration::from_secs(12), &war, &defs, no_loot(), &mut || 0);
    assert_eq!(zones.health(1, 2).unwrap()[0], 92);
    // A hidden enemy is revealed, then hit.
    let hide = sure_buff(skills::TARGET_ONESELF, buffs::HIDE, 100);
    edit(&zones, 2, |p| {
        p.occupant.buffs.apply(&hide, 77, Instant::now(), &mut || 9);
        p.form.shape = sk::SHAPE_HIDDEN;
    });
    received(&mut b_in);
    zones.keep_effects(now + Duration::from_secs(15), &world, &defs, no_loot(), &mut || 0);
    assert_eq!(zones.health(1, 2).unwrap()[0], 84);
    assert_eq!(zones.whereabouts(1, 2).unwrap().shape, 0);
    assert!(received(&mut b_in).iter().any(|p| *p == sk::applied_end(9)), "its Stealth ends");
}

#[test]
fn fire_thorns_kill_a_player_without_stopping_the_attack() {
    use std::time::Duration;
    let (zones, defs, mut b_in, mut a_ev) = thorns_ground(150);
    edit(&zones, 1, |p| p.attacking = true);
    drain(&mut b_in);
    zones.keep_effects(Instant::now() + Duration::from_secs(3), &World::default(), &defs, no_loot(), &mut || 0);
    assert_eq!(zones.health(1, 2), Some([0; 3]));
    let seen = received(&mut b_in);
    let death = seen.iter().position(|p| *p == cp::death(2, cp::KILLER_PLAYER, 1)).expect("0x0504 naming the caster");
    let hit = seen.iter().position(|p| opcode(p) == sk::OP_USE_PARTY).expect("0x0513");
    assert!(death < hit, "0x0513 after 0x0504");
    assert!(events(&mut a_ev).iter().any(|e| matches!(e, ZoneEvent::PlayerKill { victim: 2, last_hit: 1, .. })));
    edit(&zones, 1, |p| assert!(p.attacking, "deviation 3"));
}

#[test]
fn fire_thorns_go_through_absorption() {
    use std::time::Duration;
    let (zones, defs, _, _) = thorns_ground(8);
    edit(&zones, 2, |p| p.occupant.vitals.absorption = 5);
    zones.keep_effects(Instant::now() + Duration::from_secs(3), &World::default(), &defs, no_loot(), &mut || 0);
    assert_eq!(zones.health(1, 2).unwrap()[0], 97);
}

#[test]
fn fields_hit_enemy_players() {
    // docs/original/thorns-fields-pvp.md section 2.
    use std::time::Duration;
    let (zones, defs, mut b_in, mut a_ev) = skill_ground(0);
    edit(&zones, 2, |p| p.occupant.shape.country = 1);
    let wall = SkillDef { apply_range: 50, keep_time: 20, ..sure_skill(skills::FIELD, 1, skills::FIRE_WALL, [32, 0, 0]) };
    use_on(&zones, &defs, SkillTarget::Player(1), &wall).unwrap();
    let now = Instant::now();
    drain(&mut b_in);
    let world = World::default();
    zones.fields(now, &world, &defs, no_loot(), &mut || 0);
    zones.fields(now + Duration::from_secs(1), &world, &defs, no_loot(), &mut || 0);
    assert_eq!(zones.health(1, 2).unwrap()[0], 100 - 64, "32 every pass");
    let seen = received(&mut b_in);
    assert_eq!(seen, vec![sk::skill_use(sk::OP_USE_PARTY, combat::RESULT_HIT, 1, 2, 30, 1, [32, 0, 0]); 2]);
    // The owner's country is not hit, nor is its duel partner; a war map hits nobody.
    edit(&zones, 2, |p| {
        p.occupant.shape.country = 0;
        p.occupant.vitals.health = [100; 3];
    });
    zones.fields(now + Duration::from_secs(2), &world, &defs, no_loot(), &mut || 0);
    assert_eq!(zones.health(1, 2).unwrap()[0], 100);
    edit(&zones, 2, |p| p.occupant.shape.country = 1);
    let mut war = World::default();
    war.set_war_maps([1]);
    zones.fields(now + Duration::from_secs(3), &war, &defs, no_loot(), &mut || 0);
    assert_eq!(zones.health(1, 2).unwrap()[0], 100);
    // A kill with the owner gone from the zone kills with no PlayerKill.
    edit(&zones, 2, |p| p.occupant.vitals.health = [20, 100, 100]);
    zones.leave(1, 1);
    while a_ev.try_recv().is_ok() {}
    zones.fields(now + Duration::from_secs(4), &world, &defs, no_loot(), &mut || 0);
    assert_eq!(zones.health(1, 2), Some([0; 3]));
    assert!(!events(&mut a_ev).iter().any(|e| matches!(e, ZoneEvent::PlayerKill { .. })));
}

#[test]
fn a_trap_takes_a_player_once_whatever_its_state() {
    let (zones, defs, mut b_in, _) = skill_ground(1);
    edit(&zones, 2, |p| p.occupant.shape.country = 1);
    let trap = SkillDef {
        apply_range: 50,
        state_type: 7,
        damage_type: 5,
        keep_time: 30,
        ..sure_skill(skills::FIELD, 1, skills::TRAP, [15, 0, 0])
    };
    use_on(&zones, &defs, SkillTarget::Player(1), &trap).unwrap();
    drain(&mut b_in);
    let now = Instant::now();
    zones.fields(now, &World::default(), &defs, no_loot(), &mut || 0);
    assert_eq!(zones.health(1, 2).unwrap()[0], 100 - 15);
    assert!(zones.fields.guard().is_empty(), "used up");
    assert_eq!(zones.mob(1, 1).unwrap().hp, 1000, "the monster in reach afterwards is not hit");
}

#[test]
fn item_skills_hit_one_enemy_as_an_area_hit_and_pay_nothing() {
    // docs/original/small-items.md section 11.
    let (zones, defs, mut b_in, _) = skill_ground(1);
    let strike = sure_skill(skills::ATTACK, skills::TARGET_ONE, 0, [40, 0, 0]);
    let fighter = Fighter { level: 10, wis: 10, ..Default::default() };
    let cast = SkillCast {
        caster: 1,
        country: 0,
        skill: &strike,
        fighter: &fighter,
        rec: 0,
        cross_road: false,
        cost_reduction: [0; 2],
        fighting: None,
        look: None,
        illusion_mob: 0,
        war_map: false,
    };
    let (no_drops, no_items) = (DropTables::default(), ItemDefs::default());
    let hit = |target| zones.item_skill_hit(1, target, &cast, &defs, (&no_drops, &no_items), Instant::now(), &mut || 0);
    // The monster: 0x0519 as an area hit, not 0x0517; no SP paid.
    let out = hit(SkillTarget::Mob(1)).unwrap();
    assert!(
        opcodes(&out.packets).contains(&sk::OP_AREA_MOB) && !opcodes(&out.packets).contains(&sk::OP_USE_MOB),
        "{:x?}",
        opcodes(&out.packets)
    );
    assert_eq!(zones.mob(1, 1).unwrap().hp, 1000 - 40);
    assert_eq!(zones.health(1, 1), Some([100; 3]));
    // A player of one's own side is no target; one of the other side gets 0x0513; not while hidden.
    assert!(hit(SkillTarget::Player(2)).is_none());
    edit(&zones, 2, |p| p.occupant.shape.country = 1);
    drain(&mut b_in);
    let out = hit(SkillTarget::Player(2)).unwrap();
    assert!(opcodes(&out.packets).contains(&sk::OP_USE_PARTY), "{:x?}", opcodes(&out.packets));
    assert_eq!(zones.health(1, 2).unwrap()[0], 100 - 40);
    edit(&zones, 2, |p| p.form.shape = sk::SHAPE_HIDDEN);
    assert!(hit(SkillTarget::Player(2)).is_none(), "hidden");
}

#[test]
fn three_second_effects() {
    use std::time::Duration;
    let (zones, defs, mut b_in, _) = skill_ground(1);
    // A heal over time pays every three seconds for the time since it last paid.
    edit(&zones, 1, |p| p.occupant.vitals.health = [50, 100, 100]);
    let regen = SkillDef { id: 124, time_heal: [7, 0, 0], ..sure_buff(skills::TARGET_ONESELF, buffs::HEAL_OVER_TIME, 18) };
    use_on(&zones, &defs, SkillTarget::Player(1), &regen).unwrap();
    let now = Instant::now();
    drain(&mut b_in);
    zones.keep_effects(now + Duration::from_secs(6), &World::default(), &defs, no_loot(), &mut || 0);
    assert_eq!(zones.health(1, 1), Some([64, 100, 100]));
    assert!(drain(&mut b_in).contains(&(sk::OP_KEEP_USER, 1)));
    zones.keep_effects(now + Duration::from_secs(9), &World::default(), &defs, no_loot(), &mut || 0);
    assert_eq!(zones.health(1, 1), Some([71, 100, 100]), "only the 3 s since");
    // Fire Thorns: its damage as it is to the monsters within reach, which turn on the player.
    let thorns =
        SkillDef { id: 47, apply_range: 5, damage: [8, 0, 0], aggro: 50, ..sure_buff(skills::TARGET_ONESELF, buffs::FIRE_THORNS, 300) };
    use_on(&zones, &defs, SkillTarget::Player(1), &thorns).unwrap();
    zones.keep_effects(now + Duration::from_secs(12), &World::default(), &defs, no_loot(), &mut || 0);
    assert_eq!(zones.mob(1, 1).unwrap().hp, 992);
    assert!(matches!(zones.mob(1, 1).unwrap().state, MobState::Fighting(1)));
    assert!(received(&mut b_in).contains(&sk::skill_use(sk::OP_AREA_MOB, 0, 1, 1, 47, 1, [8, 0, 0])));
    // Damage over time on a monster strikes every three seconds too, and the rest at its end.
    let poison = SkillDef {
        keep_time: 30,
        time_damage: [10, 0, 0],
        ..sure_skill(skills::DEBUFF, skills::TARGET_ONE, buffs::DAMAGE_OVER_TIME, [0; 3])
    };
    use_on(&zones, &defs, SkillTarget::Mob(1), &poison).unwrap();
    let at = Instant::now();
    zones.keep_effects(at + Duration::from_secs(6), &World::default(), &defs, no_loot(), &mut || 0);
    assert_eq!(zones.mob(1, 1).unwrap().hp, 992 - 8 - 20);
    assert!(drain(&mut b_in).contains(&(sk::OP_KEEP_MOB, 1)));
}

#[test]
fn resurrection_scouting_and_return() {
    let (zones, defs, _, _) = skill_ground(0);
    let (events, mut b_events) = unbounded_channel();
    edit(&zones, 2, |p| {
        p.occupant.events = events;
        p.occupant.vitals.guard_element = 3;
        p.occupant.vitals.guard.level = 30;
        p.occupant.shape.grow = 2;
    });
    // Resurrection needs a dead target.
    let resurrect = SkillDef { id: 126, ..sure_skill(skills::ONCE, skills::TARGET_ONE, skills::RESURRECTION, [0; 3]) };
    let out = use_on(&zones, &defs, SkillTarget::Player(2), &resurrect).unwrap();
    assert_eq!(out.packets[0], sk::skill_use(sk::OP_USE_USER, skills::RESULT_WRONG_TARGET, 1, 0, 126, 1, [0; 3]));
    edit(&zones, 2, |p| p.dead = true);
    let out = use_on(&zones, &defs, SkillTarget::Player(2), &resurrect).unwrap();
    assert_eq!(out.packets[0], sk::skill_use(sk::OP_USE_USER, 0, 1, 2, 126, 1, [0; 3]));
    assert_eq!(b_events.try_recv().ok(), Some(ZoneEvent::Resurrected { position: [100.0, 0.0, 102.0], caster: 1, skill: 126 }));
    edit(&zones, 2, |p| p.dead = false);
    // Scouting: result 11 on one's own side, the numbers of the other side.
    let scout = SkillDef { id: 100, ..sure_skill(skills::ONCE, skills::TARGET_ONE, skills::SCOUT, [0; 3]) };
    let out = use_on(&zones, &defs, SkillTarget::Player(2), &scout).unwrap();
    assert_eq!(payload(&out.packets[0])[0], skills::RESULT_WRONG_TARGET);
    edit(&zones, 2, |p| p.occupant.shape.country = 1);
    let out = use_on(&zones, &defs, SkillTarget::Player(2), &scout).unwrap();
    assert_eq!(out.packets[..2], [sk::skill_use(sk::OP_USE_USER, 0, 1, 2, 100, 1, [0; 3]), sk::scouting(3, 30, 2)]);
    // Return: the animation, and the caster is to go.
    let back = SkillDef { id: 192, ..sure_skill(skills::ONCE, skills::TARGET_ONESELF, skills::RETURN, [0; 3]) };
    let out = use_on(&zones, &defs, SkillTarget::Player(1), &back).unwrap();
    assert_eq!(out.packets[0], sk::skill_use(sk::OP_USE_USER, 0, 1, 0, 192, 1, [0; 3]));
    assert!(out.recall);
}

#[test]
fn monster_debuffs_hold_and_strike_at_the_end() {
    use std::time::Duration;
    let (zones, defs, mut b_in, _) = skill_ground(1);
    let stun = SkillDef { keep_time: 5, ..sure_skill(skills::DEBUFF, skills::TARGET_ONE, buffs::STUN, [1, 0, 0]) };
    use_on(&zones, &defs, SkillTarget::Mob(1), &stun).unwrap();
    assert_eq!(zones.mob_speeds(1, 1), (buffs::CANNOT, buffs::CANNOT));
    // The watcher sees the hit, then the speeds (0x0609).
    assert_eq!(drain(&mut b_in).iter().map(|p| p.0).collect::<Vec<_>>(), [sk::OP_USE_MOB, cp::OP_MOB_SPEED]);
    let dot = SkillDef {
        id: 82,
        keep_time: 6,
        time_damage: [100, 0, 0],
        ..sure_skill(skills::DEBUFF, skills::TARGET_ONE, buffs::DAMAGE_OVER_TIME, [1, 0, 0])
    };
    use_on(&zones, &defs, SkillTarget::Mob(1), &dot).unwrap();
    assert_eq!(zones.mob(1, 1).unwrap().hp, 998);
    drain(&mut b_in);
    // 7 s on both have run out: the stun lets go, the damage over time strikes for 7 s / 3 s × 100, truncated.
    zones.recover(Instant::now() + Duration::from_secs(7), &defs, no_loot(), &mut || 0);
    assert_eq!(zones.mob(1, 1).unwrap().hp, 998 - 233);
    assert_eq!(zones.mob_speeds(1, 1), (mobs::ATTACK_SPEED, mobs::MOVE_SPEED));
    let seen = drain(&mut b_in).iter().map(|p| p.0).collect::<Vec<_>>();
    assert!(seen.contains(&sk::OP_KEEP_MOB) && seen.contains(&cp::OP_MOB_SPEED), "{seen:x?}");
}

#[test]
fn block_all_and_reflection_in_monster_attacks() {
    use crate::combat::{self, Gear};
    let (world, _, defs) = fighting_ground();
    let now = Instant::now();
    let mut key = 0;
    let mut next = || {
        key += 1;
        key
    };
    let block_all = SkillDef { id: 90, level: 1, type_effect: skills::BUFF, kind: buffs::BLOCK_ALL, keep_time: 60, ..Default::default() };
    let counter =
        SkillDef { id: 89, kind: buffs::MIRROR, abilities: [(0, 1), (0, 0), (0, 0)], success_type: 1, success_value: 100, ..block_all };
    for (buff, blocked) in [(block_all, true), (counter, false)] {
        let zones = Zones::new(&world, &defs);
        let (a, mut a_in) = player(1, 100.0, 102.0);
        let mut buffs = PlayerBuffs::default();
        buffs.apply(&buff, 1, now, &mut next);
        zones.enter(1, Occupant { buffs, ..a });
        zones.respawn(now, &world, &defs, &mut || 0);
        let fighter = combat::player(10, [10; 6], &Gear { kind: Some(combat::MELEE), ..Default::default() });
        let attack =
            Attack { attacker: 1, country: 0, fighter: &fighter, kind: combat::MELEE, slack: 2.0, range: 0, rec: 0, cross_road: false };
        let mut rolls = [0, 99].into_iter().cycle();
        zones.attack_mob(1, 1, &attack, &defs, no_loot(), now, &mut || rolls.next().unwrap());
        let hp = zones.mob(1, 1).unwrap().hp;
        drain(&mut a_in);
        let mut rolls = [0, 99].into_iter().cycle();
        zones.mob_fight(now, &world, &defs, no_loot(), &mut || rolls.next().unwrap());
        assert_eq!(zones.health(1, 1), Some([100; 3]), "no damage taken");
        let seen: Vec<Vec<u8>> = std::iter::from_fn(|| a_in.try_recv().ok()).collect();
        if blocked {
            assert_eq!(opcodes(&seen), [0x0605]);
            assert_eq!(payload(&seen[0])[0], combat::RESULT_MISS);
            assert_eq!(zones.mob(1, 1).unwrap().hp, hp);
        } else {
            // The blow goes back onto the monster: 0x0605 with no damage, then 0x051B with what it took.
            assert_eq!(opcodes(&seen), [0x0605, sk::OP_REFLECT]);
            assert_eq!(payload(&seen[0])[9..], [0; 6]);
            let back = u16::from_le_bytes(payload(&seen[1])[8..10].try_into().unwrap());
            assert!(back > 0);
            assert_eq!(zones.mob(1, 1).unwrap().hp, hp - u32::from(back));
        }
    }
}

#[test]
fn players_and_monsters_recover() {
    use std::time::Duration;
    let (world, _, defs) = fighting_ground();
    let zones = Zones::new(&world, &defs);
    let (mut a, mut a_in) = player(1, 100.0, 100.0);
    a.vitals.health = [50, 100, 98];
    zones.enter(1, a);
    let now = Instant::now();
    zones.respawn(now, &world, &defs, &mut || 0);
    drain(&mut a_in);
    // Standing: 4 each, SP capped at 100; something is at its maximum, so 0x0521 follows.
    zones.recover(now, &defs, no_loot(), &mut || 0);
    assert_eq!(zones.health(1, 1), Some([54, 100, 100]));
    assert_eq!(drain(&mut a_in), [(0x0505, 1), (0x0521, 54 | 100 << 16)]);
    zones.recover(now + Duration::from_secs(4), &defs, no_loot(), &mut || 0);
    assert!(drain(&mut a_in).is_empty(), "every 5 s");
    // Sitting (from the next turn on): 28 + 5 % of 100.
    zones.set_motion(1, 1, w::MOTION_SIT);
    zones.recover(now + Duration::from_secs(5), &defs, no_loot(), &mut || 0);
    zones.recover(now + Duration::from_secs(10), &defs, no_loot(), &mut || 0);
    assert_eq!(zones.health(1, 1), Some([91, 100, 100]));
    // Fighting: nothing.
    zones.set_attacking(1, 1, true);
    drain(&mut a_in);
    zones.recover(now + Duration::from_secs(15), &defs, no_loot(), &mut || 0);
    assert!(drain(&mut a_in).is_empty());

    // A hurt idle monster regains 10 every 5 s (0x0610).
    let mut rolls = [0, 99].into_iter().cycle();
    let fighter = crate::combat::player(10, [10; 6], &crate::combat::Gear { kind: Some(crate::combat::MELEE), ..Default::default() });
    let attack =
        Attack { attacker: 1, country: 0, fighter: &fighter, kind: crate::combat::MELEE, slack: 10.0, range: 0, rec: 0, cross_road: false };
    let (no_drops, no_items) = (DropTables::default(), ItemDefs::default());
    zones.attack_mob(1, 1, &attack, &defs, (&no_drops, &no_items), now, &mut || rolls.next().unwrap());
    let hurt = zones.mob(1, 1).unwrap().hp;
    zones.recover(now + Duration::from_secs(20), &defs, no_loot(), &mut || 0);
    assert_eq!(zones.mob(1, 1).unwrap().hp, hurt, "fighting: no recovery");
    let mut all = zones.mobs.guard();
    all.get_mut(&1).unwrap().mobs.get_mut(&1).unwrap().state = MobState::Idle;
    drop(all);
    drain(&mut a_in);
    zones.recover(now + Duration::from_secs(20), &defs, no_loot(), &mut || 0);
    assert_eq!(zones.mob(1, 1).unwrap().hp, hurt + 10);
    assert_eq!(drain(&mut a_in), [(0x0610, 1)]);
}

#[test]
fn drops_lie_on_the_ground_for_their_owner_first() {
    use crate::combat::{self, Gear};
    use crate::inventory::ItemDef;
    use crate::mobs::{MobDef, MobDefs, SpawnArea};
    let mut world = World::build([(1, 2048)], &std::collections::HashSet::new(), []);
    world.set_mob_data(
        [(1, vec![0u8; 2048 * 2048 / 8])].into(),
        [(1, vec![SpawnArea::new([100.0, 0.0, 105.0, 100.0, 0.0, 105.0], vec![(7, 1)])])].into(),
    );
    let mut drops = [(0, 0); 10];
    drops[1] = (3, 100);
    let def = MobDef { id: 7, level: 1, hp: 1, size: 1, country: 2, money: 50, drops, ..Default::default() };
    let defs = MobDefs::new([def]);
    let items = ItemDefs::new([ItemDef { item_type: 5, type_id: 1, grade: 3, quality: 900, ..Default::default() }]);
    let tables = DropTables::new(&items);
    let zones = Zones::new(&world, &defs);
    let (a, mut a_in) = player(1, 100.0, 100.0);
    zones.enter(1, a);
    let (b, mut b_in) = player(2, 100.0, 106.0);
    zones.enter(1, b);
    let now = Instant::now();
    zones.respawn(now, &world, &defs, &mut || 0);
    drain(&mut a_in);
    drain(&mut b_in);

    let fighter = combat::player(10, [100, 10, 10, 10, 10, 0], &Gear { kind: Some(combat::MELEE), ..Default::default() });
    let attack =
        Attack { attacker: 1, country: 0, fighter: &fighter, kind: combat::MELEE, slack: 2.0, range: 0, rec: 0, cross_road: false };
    // Every roll 0: a hit, entry 1 drops the sword, gold 50; both land half a unit before the monster on x and z.
    let AttackOutcome::Struck { packets, .. } = zones.attack_mob(1, 1, &attack, &defs, (&tables, &items), now, &mut || 0) else {
        panic!("a hit")
    };
    let sword = gp::item_enter(1, gp::KIND_DROPPED, 5, 1, 1, [99.5, 0.0, 104.5]);
    let gold = gp::item_enter(2, gp::KIND_DROPPED, GOLD_TYPE, 0, 1, [99.5, 0.0, 104.5]);
    assert_eq!(packets[..3], [cp::mob_death(1, cp::KILLER_PLAYER, 1), sword, gold]);
    assert_eq!(opcode(&packets[3]), cp::OP_ATTACK_MOB);
    // The blow is a critical (roll 0): result 1, attacker 1, read as 257.
    assert_eq!(drain(&mut b_in), [(0x0606, 1), (0x0401, 1), (0x0401, 2), (0x0503, 257)]);

    // B may not take A's items for 30 s; A may, from within 10.
    assert_eq!(zones.pick_up(1, 2, 1, now, |_, _| true), Err(gp::PICK_NOT_YOURS));
    assert_eq!(zones.pick_up(1, 2, 1, now + Duration::from_secs(30), |_, _| true), Err(gp::PICK_NOT_YOURS), "still A's at exactly 30 s");
    assert_eq!(
        zones.pick_up(1, 2, 1, now + Duration::from_millis(30_001), |_, _| true).map(|(item, _)| item.loot),
        Ok(Loot::Item {
            item_type: 5,
            type_id: 1,
            count: 1,
            quality: 900,
            kept: Some(crate::inventory::Kept { make_type: b'D', ..crate::inventory::Kept::default() })
        })
    );
    assert_eq!(drain(&mut a_in), [(0x0402, 1)]);
    assert_eq!(zones.pick_up(1, 2, 1, now, |_, _| true), Err(gp::PICK_GONE));
    let (taken, leave) = zones.pick_up(1, 1, 2, now, |_, _| true).unwrap();
    assert_eq!((taken.loot, leave), (Loot::Gold(50), gp::item_leave(2)));
    // Back on the ground (no room): kind 1, another 60 s.
    assert_eq!(
        zones.put_back(1, 1, taken, taken.loot, now),
        Some(gp::item_enter(2, gp::KIND_PUT_BACK, GOLD_TYPE, 0, 1, [99.5, 0.0, 104.5]))
    );
    // A player coming into view sees it as lying there (kind 0).
    let (c, _) = player(3, 100.0, 100.0);
    assert!(zones.enter(1, c).contains(&gp::item_enter(2, gp::KIND_IN_VIEW, GOLD_TYPE, 0, 1, [99.5, 0.0, 104.5])));
    // Someone far away, once it is no longer only A's: too far.
    let (d, _) = player(4, 500.0, 500.0);
    zones.enter(1, d);
    assert_eq!(zones.pick_up(1, 4, 2, now + Duration::from_millis(30_001), |_, _| true), Err(gp::PICK_TOO_FAR));
    // It goes after 60 s.
    assert_eq!(zones.expire_items(now + Duration::from_secs(60)), 0);
    assert_eq!(zones.expire_items(now + Duration::from_millis(60_001)), 1);
    assert_eq!(zones.pick_up(1, 1, 2, now, |_, _| true), Err(gp::PICK_GONE));
}

/// The zones, the ground as (owner, party) and the events of players 1 and 2 after `quest_kill`.
type QuestKill = (Zones, Vec<(u32, u32)>, UnboundedReceiver<ZoneEvent>, UnboundedReceiver<ZoneEvent>);

/// Kills a monster carrying quest item 27/2 (quality 100; started by quest 5 when `starts`, else ended by quest 9) with player 2,
/// after `before` ran on the zones with players 1 and 2 in; returns the zones, the items on the ground as (owner, party) and
/// the two players' events.
fn quest_kill(starts: bool, before: impl FnOnce(&Zones)) -> QuestKill {
    use crate::combat::{self, Gear};
    use crate::inventory::ItemDef;
    use crate::mobs::{MobDef, MobDefs, SpawnArea};
    let mut world = World::build([(1, 2048)], &std::collections::HashSet::new(), []);
    world.set_mob_data(
        [(1, vec![0u8; 2048 * 2048 / 8])].into(),
        [(1, vec![SpawnArea::new([100.0, 0.0, 155.0, 100.0, 0.0, 155.0], vec![(7, 1)])])].into(),
    );
    let defs = MobDefs::new([MobDef { id: 7, level: 1, hp: 30, size: 1, country: 2, quest_item: Some((27, 2)), ..Default::default() }]);
    let items = ItemDefs::new([crate::inventory::ItemDef { item_type: 27, type_id: 2, quality: 100, ..ItemDef::default() }]);
    let mut tables = DropTables::new(&items);
    tables.load_quest_links(&[(27, 2, if starts { vec![5] } else { vec![] }, vec![9])]);
    let zones = Zones::new(&world, &defs);
    let (a, _a_in, a_events) = member(1, 100.0, 100.0, 0, b"Alice", 10);
    let (b, _b_in, b_events) = member(2, 100.0, 150.0, 0, b"Bob", 10);
    zones.enter(1, a);
    zones.enter(1, b);
    before(&zones);
    let now = Instant::now();
    zones.respawn(now, &world, &defs, &mut || 0);
    let fighter = combat::player(10, [100, 10, 10, 10, 10, 0], &Gear { kind: Some(combat::MELEE), ..Default::default() });
    let attack =
        Attack { attacker: 2, country: 0, fighter: &fighter, kind: combat::MELEE, slack: 2.0, range: 0, rec: 0, cross_road: false };
    let outcome = zones.attack_mob(1, 1, &attack, &defs, (&tables, &items), now, &mut || 0);
    let AttackOutcome::Struck { killed, .. } = outcome else { panic!("a hit: {outcome:?}") };
    assert!(killed);
    let ground = zones.ground.guard().get(&1).map_or(Vec::new(), |g| g.items.values().map(|i| (i.owner, i.party)).collect());
    (zones, ground, a_events, b_events)
}

#[test]
fn a_quest_item_drops_for_a_quest_in_progress_or_a_starting_item() {
    // Linked to a quest it ends: nothing without the quest, and not for another quest.
    assert!(quest_kill(false, |_| {}).1.is_empty());
    assert!(quest_kill(false, |z| z.set_quests(2, vec![4, 10])).1.is_empty());
    // With quest 9 in progress: on the ground, the killer's, no party.
    assert_eq!(quest_kill(false, |z| z.set_quests(2, vec![4, 9])).1, [(2, 0)]);
    // Another player's quest does not count without a party; a quest forgotten no more.
    assert!(quest_kill(false, |z| z.set_quests(1, vec![9])).1.is_empty());
    assert!(
        quest_kill(false, |z| {
            z.set_quests(2, vec![9]);
            z.forget_quests(2)
        })
        .1
        .is_empty()
    );
    // A quest-starting item needs no quest at all.
    assert_eq!(quest_kill(true, |_| {}).1, [(2, 0)]);
}

#[test]
fn a_party_members_quest_counts_and_the_item_stays_on_the_ground() {
    let party = |z: &Zones| {
        join(z, 1, 2);
        z.set_quests(1, vec![9]);
    };
    let (zones, ground, mut a_events, mut b_events) = quest_kill(false, party);
    let id = zones.party_id(2);
    assert_ne!(id, 0);
    assert_eq!(ground, [(2, id)], "the killer's, with the party id");
    // Not into any member's bags (a loot rule would have): no loot event.
    for events in [&mut a_events, &mut b_events] {
        assert!(!std::iter::from_fn(|| events.try_recv().ok()).any(|e| matches!(e, ZoneEvent::Loot { .. })));
    }
    // The member with the quest is somewhere else (not in the killer's zone): it does not count.
    let away = |z: &Zones| {
        join(z, 1, 2);
        z.set_quests(1, vec![9]);
        z.leave(1, 1);
    };
    assert!(quest_kill(false, away).1.is_empty());
}

#[test]
fn has_quest_looks_at_the_killer_and_the_members_present() {
    let zones = Zones::default();
    let (a, _, _) = member(1, 100.0, 100.0, 0, b"Alice", 10);
    let (b, _, _) = member(2, 100.0, 100.0, 0, b"Bob", 10);
    zones.enter(1, a);
    zones.enter(1, b);
    let present = || zones.maps.guard().get(&1).map(|p| p.keys().copied().collect::<Vec<_>>()).unwrap();
    let has = |killer, wanted: &[u16]| {
        let maps = zones.maps.guard();
        zones.has_quest(&maps[&1], killer, wanted)
    };
    assert_eq!(present().len(), 2);
    zones.set_quests(2, vec![3, 9]);
    assert!(has(2, &[9]), "the killer's own");
    assert!(has(2, &[1, 3]));
    assert!(!has(2, &[]), "an empty list never");
    assert!(!has(1, &[9]), "a lone killer: not another player's");
    join(&zones, 1, 2);
    assert!(has(1, &[9]), "a party member in the zone");
    zones.leave(1, 2);
    zones.set_quests(2, vec![9]);
    let maps = zones.maps.guard();
    assert!(!zones.has_quest(&maps[&1], 1, &[9]), "a member that is not in the zone");
}

#[test]
fn a_quest_item_needs_the_pickers_quest() {
    let zones = Zones::default();
    let (a, _a_in, _) = member(1, 100.0, 100.0, 0, b"Alice", 10);
    zones.enter(1, a);
    let now = Instant::now();
    let loot = |type_id| Loot::Item { item_type: 27, type_id, count: 1, quality: 100, kept: None };
    zones.drop_on_ground(1, loot(2), [100.0, 0.0, 100.0], 0, 0, now, gp::KIND_DROPPED, Dropped::MOB);
    zones.drop_on_ground(1, loot(3), [100.0, 0.0, 100.0], 9, 0, now, gp::KIND_DROPPED, Dropped::MOB);
    zones.drop_on_ground(
        1,
        Loot::Item { item_type: 5, type_id: 1, count: 1, quality: 100, kept: None },
        [100.0, 0.0, 100.0],
        0,
        0,
        now,
        gp::KIND_DROPPED,
        Dropped::MOB,
    );
    assert_eq!(zones.pick_up(1, 1, 1, now, |_, _| false), Err(gp::PICK_NO_QUEST), "refused, and left on the ground");
    assert_eq!(zones.pick_up(1, 1, 1, now, |t, i| (t, i) == (27, 2)).map(|(item, _)| item.loot), Ok(loot(2)));
    // Someone else's item for its first 30 s: the owner error (0) comes before the quest error.
    assert_eq!(zones.pick_up(1, 1, 2, now, |_, _| false), Err(gp::PICK_NOT_YOURS));
    // Other items are not asked.
    assert!(zones.pick_up(1, 1, 3, now, |_, _| false).is_ok());
}

/// A named player of `country` with an events inbox.
fn member(
    id: u32,
    x: f32,
    z: f32,
    country: u8,
    name: &[u8],
    level: u16,
) -> (Occupant, crate::outbox::OutboxInbox, UnboundedReceiver<ZoneEvent>) {
    let (mut o, inbox) = player(id, x, z);
    let (events, events_in) = unbounded_channel();
    o.shape.country = country;
    o.shape.name = name.to_vec();
    o.vitals.guard.level = level;
    (Occupant { events, ..o }, inbox, events_in)
}

/// `invitee` joins the party of `inviter` on map 1.
fn join(zones: &Zones, inviter: u32, invitee: u32) {
    zones.party_invite(1, inviter, Invitee::Id(invitee), false, Instant::now());
    zones.party_answer(1, invitee, true, false, Instant::now());
}

#[test]
fn party_instance_zones_take_a_party_count_its_players_and_go_when_empty() {
    use crate::instance::{REFUSED_CLOSED, REFUSED_FEW_MEMBERS, REFUSED_FULL, REFUSED_NO_PARTY};
    use crate::mobs::{MobDef, MobDefs, SpawnArea};
    use crate::world::{InstanceRules, MapNpcRow, map_of};
    // A merchant on map 1 (object id 1) and one on map 65 (2).
    let merchant = |map| MapNpcRow { map, kind: 1, type_id: 1, first: Some([110.0, 0.0, 110.0, 0.0]), walk: Vec::new() };
    let defined = std::collections::HashSet::from([(1, 1)]);
    let mut world = World::build([(1, 1024), (65, 1024)], &defined, [merchant(1), merchant(65)]);
    world.set_portals(HashMap::new(), [(65, crate::world::create_type::PARTY)].into());
    let area = SpawnArea::new([100.0, 0.0, 100.0, 100.0, 0.0, 100.0], vec![(7, 1)]);
    world.set_mob_data([(65, vec![0u8; 1024 * 1024 / 8])].into(), [(65, vec![area])].into());
    let rules = InstanceRules::new(&[], 1, 2, 5);
    world.set_instance_rules([(65, rules.clone())].into());
    let defs = MobDefs::new([MobDef { id: 7, level: 1, hp: 50, respawn: Duration::from_secs(5), ..Default::default() }]);
    let zones = Zones::new(&world, &defs);
    let now = Instant::now();
    assert_eq!(zones.respawn(now, &world, &defs, &mut || 0), 0, "a P map has no static zone");
    for (id, name) in [(1, &b"Alice"[..]), (2, b"Bob"), (3, b"Carol")] {
        zones.enter(1, member(id, 100.0, 100.0, 0, name, 30).0);
    }
    join(&zones, 1, 2);
    let enter = |me: u32, open: bool| zones.enter_instance(65, me, 1, &rules, open, &world, &defs);
    assert_eq!(enter(3, true), Err(REFUSED_NO_PARTY));
    assert_eq!(enter(1, false), Err(REFUSED_CLOSED), "closed comes first");
    let zone = enter(1, true).expect("the party's zone");
    assert_eq!((map_of(zone), zone >> 8), (65, 1));
    assert_eq!(zones.respawn(now, &world, &defs, &mut || 0), 1, "the zone's own monsters");
    // Its own NPCs with new object ids (`CZone::MapInit`); map 65 has none of its own.
    assert_eq!(zones.npc(zone, 3).map(|n| (n.kind, n.position)), Some((1, [110.0, 0.0, 110.0])));
    assert!(zones.npc(65, 2).is_none() && zones.npc(zone, 2).is_none() && zones.npc(1, 1).is_some());
    assert_eq!(enter(2, true), Ok(zone), "one booked of two");
    join(&zones, 1, 3);
    assert_eq!(enter(3, true), Err(REFUSED_FULL), "two booked");
    assert_eq!(
        zones.enter_instance(65, 3, 1, &InstanceRules::new(&[], 4, 9, 5), true, &world, &defs),
        Err(REFUSED_FEW_MEMBERS),
        "three of four"
    );
    // Alice arrives (no longer booked, now in it); Bob gives up on the way.
    zones.leave(1, 1);
    zones.enter(zone, member(1, 100.0, 100.0, 0, b"Alice", 30).0);
    zones.unbook(2);
    assert_eq!(zones.enter_instance(65, 1, zone, &rules, true, &world, &defs), Ok(zone), "her own zone: a portal on its map");
    assert_eq!(zones.expire_instances(now, &world), 0);
    zones.leave(zone, 1);
    assert_eq!(zones.expire_instances(now, &world), 0, "empty: its time starts");
    assert_eq!(zones.expire_instances(now + Duration::from_secs(5 * 60 + 29), &world), 0);
    assert_eq!(zones.expire_instances(now + Duration::from_secs(5 * 60 + 30), &world), 1, "5 min 30 s");
    assert_eq!((zones.mob_count(zone), zones.instance_count()), (0, 0));
    assert!(zones.npc(zone, 3).is_none(), "its NPCs went with it");
    assert_eq!(enter(1, true), Ok(zone), "a new zone for the party");
    assert!(zones.npc(zone, 4).is_some(), "new NPCs, new ids");
}

#[test]
fn guild_houses_open_one_zone_per_guild() {
    use crate::backend::{StoredGuild, StoredMember};
    use crate::guild_house::{HOUSE_OPEN, House, portal};
    use crate::mobs::MobDefs;
    use crate::world::{MapNpcRow, map_of};
    let merchant = MapNpcRow { map: 51, kind: 1, type_id: 1, first: Some([110.0, 0.0, 110.0, 0.0]), walk: Vec::new() };
    let defined = std::collections::HashSet::from([(1, 1)]);
    let mut world = World::build([(1, 1024), (51, 1024)], &defined, [merchant]);
    world.set_portals(HashMap::new(), [(51, crate::world::create_type::GUILD_HOUSE)].into());
    let defs = MobDefs::new([]);
    let zones = Zones::new(&world, &defs);
    assert!(zones.npc(51, 1).is_none(), "no static zone");
    let m = |id, rank| StoredMember { id, name: format!("p{id}"), level: 20, job: 0, rank };
    let open = House { ranking: 4, use_house: HOUSE_OPEN, bought: true, ..House::default() };
    let guild = |id, name: &str, members, house| StoredGuild {
        id,
        name: name.into(),
        country: 0,
        remark: String::new(),
        members,
        house,
        bank: Vec::new(),
    };
    let stored = vec![
        guild(10, "Open", vec![m(1, 1), m(2, 9)], open.clone()),
        guild(20, "Unranked", vec![m(3, 1)], House { ranking: 31, ..open.clone() }),
        guild(30, "Closed", vec![m(4, 1)], House { use_house: 2, ..open.clone() }),
    ];
    zones.load_guilds(crate::guild::Guilds::load(stored, unbounded_channel().0));
    let enter = |me| zones.enter_guild_house(51, me, 1, &world, &defs);
    assert_eq!(enter(5), Err(portal::NO_GUILD));
    assert_eq!(enter(3), Err(portal::UNRANKED));
    assert_eq!(enter(4), Err(portal::CLOSED));
    let zone = enter(1).expect("the guild's zone");
    assert_eq!((map_of(zone), zone >> 8), (51, 1));
    assert_eq!(enter(2), Ok(zone), "the same guild, the same zone");
    assert!(zones.npc(zone, 2).is_some(), "its own NPCs");
    assert_eq!(zones.instance_count(), 1);
}

#[test]
fn the_ranking_battle_takes_each_guild_into_its_zone_once() {
    use crate::backend::{StoredGuild, StoredMember};
    use crate::mobs::MobDefs;
    use crate::ranking::Step;
    use crate::world::map_of;
    let mut world = World::build([(1, 1024), (50, 1024)], &std::collections::HashSet::new(), []);
    world.set_portals(HashMap::new(), [(50, crate::world::create_type::GUILD_RANKING)].into());
    let defs = MobDefs::new([]);
    let zones = Zones::new(&world, &defs);
    assert!(zones.ranking_map() == Some(50) && zones.is_ranking_zone(50 | 3 << 8) && !zones.is_ranking_zone(1));
    let m = |id, rank| StoredMember { id, name: format!("p{id}"), level: 20, job: 0, rank };
    let guild = |id, name: &str, members| StoredGuild {
        id,
        name: name.into(),
        country: 0,
        remark: String::new(),
        members,
        house: Default::default(),
        bank: Vec::new(),
    };
    let stored = vec![guild(10, "Alpha", vec![m(1, 1), m(2, 9)]), guild(20, "Beta", vec![m(3, 1)])];
    zones.load_guilds(crate::guild::Guilds::load(stored, unbounded_channel().0));
    let mut events = Vec::new();
    for id in 1..=4 {
        let (o, _inbox, events_in) = member(id, 100.0, 100.0, 0, format!("p{id}").as_bytes(), 30);
        events.push(events_in);
        zones.enter(1, o);
    }
    let go = |me, current, confirmed| zones.enter_ranking(50, me, current, confirmed, &world, &defs);
    // Closed, no guild.
    assert_eq!((go(4, 1, true), go(1, 1, true)), (Err(0), Err(1)));
    let tag = 99;
    assert_eq!(zones.ranking_step(Some(tag), 1000, 1800), Step::Nothing);
    assert_eq!(zones.ranking_open(), Some(tag));
    // Deviation 2: the first request asks; the yes goes in, into the guild's own zone.
    assert_eq!(go(1, 1, false), Err(5));
    let alpha = go(1, 1, true).expect("Alpha's zone");
    assert_eq!((map_of(alpha), zones.instance_count()), (50, 1));
    assert_eq!(go(2, 1, true), Ok(alpha), "the same guild, the same zone");
    let beta = go(3, 1, true).expect("Beta's zone");
    assert_ne!(alpha, beta);
    // Arriving counts the entry, once per character.
    zones.enter(alpha, member(1, 508.0, 98.0, 0, b"p1", 30).0);
    assert_eq!(zones.guilds().house(10).unwrap().battle_entries, 1);
    zones.enter(alpha, member(2, 508.0, 98.0, 0, b"p2", 30).0);
    zones.leave(alpha, 2);
    let (p2, _inbox, mut p2_events) = member(2, 508.0, 98.0, 0, b"p2", 30);
    zones.enter(alpha, p2);
    assert_eq!(zones.guilds().house(10).unwrap().battle_entries, 2, "p2 counted once");
    // Inside, a portal of the map stays in the zone; outside again, the second entry is refused.
    assert_eq!(go(1, alpha, true), Ok(alpha));
    assert_eq!(go(1, alpha, false), Ok(alpha));
    zones.leave(alpha, 1);
    zones.enter(1, member(1, 100.0, 100.0, 0, b"p1", 30).0);
    assert_eq!(go(1, 1, true), Err(2));
    // Kill points reach the guild's members in the zone.
    let points = zones.battle_points(&HashMap::new(), 2, 3);
    assert_eq!(points, Some((10, 3)));
    // The close calls everyone left inside out; the update ranks the guild.
    assert_eq!(zones.ranking_step(None, 1060, 1800), Step::Closed { due: 2860 });
    assert_eq!(zones.ranking_close(&world, &mut || 0), 1, "only p2 is still inside");
    assert!(matches!(p2_events.try_recv(), Ok(ZoneEvent::Recalled { .. })));
    assert_eq!(go(2, 1, true), Err(1), "closed");
    assert_eq!(zones.ranking_step(None, 2860, 1800), Step::Update);
    let placed = zones.ranking_update();
    assert_eq!(placed.iter().map(|p| (p.guild, p.place)).collect::<Vec<_>>(), [(10, 1)]);
    assert_eq!(zones.guilds().house(10).unwrap().ranking, 1);
}

#[test]
fn a_lua_monster_speaks_summons_walks_off_and_opens_its_zones_portal() {
    use crate::mobs::{MobDef, MobDefs, SpawnArea};
    use crate::world::InstanceRules;
    // Like 115.lua: unbeatable; a line when it sees a player; on a combat turn a summon and a walk; at the walk's end it deletes
    // itself, and its death opens portal 1001.
    let script = br#"
            Mob = LuaMob(CMob)
            said = 0
            function Init() Mob:LuaSetUnBeatable(1) end
            function OnAttackable(t, id) if said == 0 then Mob:LuaSayByIndex(6401, 500.0) said = 1 end end
            function WhileCombat(t, hp, n)
                if said == 1 then Mob:LuaCreateMob(8, 1, 0.0, 0.0) Mob:LuaFixedMove(1, 120, 100) said = 2 end
            end
            function OnMoveEnd(t) Mob:LuaSetHoldPosion(1) Mob:LuaDeleteMob(7, 1, 0.0, 0.0) end
            function OnDeath(t, n) Mob:LuaUpdateInsZonePortal(1001, 0) end
        "#;
    let mut world = World::build([(1, 1024), (65, 1024)], &std::collections::HashSet::new(), []);
    world.set_portals(HashMap::new(), [(65, crate::world::create_type::PARTY)].into());
    let area = SpawnArea::new([100.0, 0.0, 100.0, 100.0, 0.0, 100.0], vec![(7, 1)]);
    world.set_mob_data([(65, vec![0u8; 1024 * 1024 / 8])].into(), [(65, vec![area])].into());
    world.set_mob_lua(crate::mob_lua::Scripts([(115, script.to_vec())].into()));
    let boss = MobDef {
        id: 7,
        level: 64,
        hp: 9999,
        ai: 115,
        // Of neither country, as the Cailum Sacra monsters are.
        country: 2,
        chase_range: 6,
        normal_step: 8,
        chase_step: 6,
        normal_time: Duration::from_secs(1),
        respawn: Duration::from_secs(600),
        ..Default::default()
    };
    let defs = MobDefs::new([boss, MobDef { id: 8, level: 1, hp: 10, ..Default::default() }]);
    let zones = Zones::new(&world, &defs);
    let (drops, items) = (DropTables::default(), ItemDefs::default());
    for (id, name) in [(1, &b"Alice"[..]), (2, b"Bob")] {
        zones.enter(1, member(id, 100.0, 100.0, 0, name, 30).0);
    }
    join(&zones, 1, 2);
    let rules = InstanceRules::new(&[], 1, 7, 5);
    let zone = zones.enter_instance(65, 1, 1, &rules, true, &world, &defs).expect("the zone");
    let t0 = Instant::now();
    assert_eq!(zones.respawn(t0, &world, &defs, &mut || 0), 1);
    zones.leave(1, 1);
    let (alice, mut inbox, _events) = member(1, 103.0, 100.0, 0, b"Alice", 30);
    zones.enter(zone, alice);
    drain(&mut inbox);
    assert_eq!(zones.instance_portal_country(zone, 1001), crate::instance::PORTAL_NOBODY);

    // It sees her (within its chase range of 6): its line, `0x110A` 6401, and it is in combat.
    zones.lua(t0, &world, &defs, (&drops, &items), &mut || 0);
    let said = drain(&mut inbox);
    assert!(said.contains(&(0x110A, 1)), "{said:?}");
    assert!(zones.mob(zone, 1).is_some_and(|m| m.unbeatable), "Init ran");
    // Its first combat turn is its attack time (no entry to use); the next one runs WhileCombat: monster 8 comes, the walk starts.
    let mut t = t0;
    for _ in 0..2 {
        t += Duration::from_secs(1);
        zones.lua(t, &world, &defs, (&drops, &items), &mut || 0);
    }
    assert!(zones.mob(zone, 2).is_some_and(|m| m.mob_id == 8), "the summon");
    // It walks 8 a step towards (120, 100) and ends within 5 of it; then it deletes itself and its death opens the portal.
    for _ in 0..10 {
        t += Duration::from_secs(1);
        zones.lua(t, &world, &defs, (&drops, &items), &mut || 0);
        zones.remove_bodies(t, &defs, &mut || 0);
    }
    assert!(zones.mob(zone, 1).is_none(), "gone");
    assert_eq!(zones.instance_portal_country(zone, 1001), 0, "the portal is open");
    let seen = drain(&mut inbox);
    assert!(seen.contains(&(0x0603, 1)) && seen.contains(&(0x0602, 1)), "walked and left: {seen:?}");
    assert!(!seen.contains(&(0x0606, 1)), "a deletion is no death");
}

#[test]
fn lua_monsters_answer_hits_fight_and_die() {
    use crate::combat::{self, Gear};
    use crate::mobs::{MobAttack, MobDef, MobDefs, SpawnArea};
    let script = br#"
            Mob = LuaMob(CMob)
            function OnAttacked(t, id) Mob:LuaSayByIndex(1, 100) end
            function WhileCombat(t, hp, n) if n >= 1 and hp < 100 then Mob:LuaSayByIndex(2, 100) end end
            function OnDeath(t, n) Mob:LuaSayByIndex(10 + n, 100) end
        "#;
    let mut world = World::build([(1, 2048)], &std::collections::HashSet::new(), []);
    world.set_mob_data(
        [(1, vec![0u8; 2048 * 2048 / 8])].into(),
        [(1, vec![SpawnArea::new([100.0, 0.0, 105.0, 100.0, 0.0, 105.0], vec![(7, 1)])])].into(),
    );
    world.set_mob_lua(crate::mob_lua::Scripts([(111, script.to_vec())].into()));
    let blow = MobAttack { ok: 1, range: 10, damage: 5, time: Duration::from_secs(1), ..Default::default() };
    let def = MobDef {
        id: 7,
        level: 1,
        hp: 300,
        size: 1,
        ai: 111,
        country: 2,
        // No idle steps (it would walk off before the blow), but it chases.
        normal_step: 0,
        chase_step: 8,
        chase_time: Duration::from_secs(1),
        attacks: [blow, MobAttack::default(), MobAttack::default()],
        respawn: Duration::from_secs(30),
        ..Default::default()
    };
    let defs = MobDefs::new([def]);
    let zones = Zones::new(&world, &defs);
    let (a, mut inbox, _events) = member(1, 100.0, 100.0, 0, b"Alice", 30);
    zones.enter(1, a);
    let now = Instant::now();
    assert_eq!(zones.respawn(now, &world, &defs, &mut || 0), 1);
    let (no_drops, no_items) = (DropTables::default(), ItemDefs::default());
    let nothing = (&no_drops, &no_items);
    zones.lua(now, &world, &defs, nothing, &mut || 0);
    drain(&mut inbox);
    let lines = |inbox: &mut crate::outbox::OutboxInbox| -> Vec<u16> {
        let mut out = Vec::new();
        while let Ok(p) = inbox.try_recv() {
            if opcode(&p) == 0x110A {
                out.push(u16::from_le_bytes(payload(&p)[4..6].try_into().unwrap()));
            }
        }
        out
    };

    // Her blow (130) does not kill: SetTarget gives event 1 (OnAttacked) for the pass, which also fights at once.
    let fighter = combat::player(10, [100, 10, 10, 10, 10, 0], &Gear { kind: Some(combat::MELEE), ..Default::default() });
    let attack =
        Attack { attacker: 1, country: 0, fighter: &fighter, kind: combat::MELEE, slack: 2.0, range: 0, rec: 0, cross_road: false };
    let mut rolls = [0, 99].into_iter().cycle();
    assert!(matches!(
        zones.attack_mob(1, 1, &attack, &defs, nothing, now, &mut || rolls.next().unwrap()),
        AttackOutcome::Struck { killed: false, .. }
    ));
    assert_eq!(zones.mob(1, 1).map(|m| m.state), Some(MobState::Lua(Some(1))));
    zones.lua(now, &world, &defs, nothing, &mut || 0);
    let (raw, said): (Vec<u16>, Vec<u16>) = {
        let mut raw = Vec::new();
        let mut said = Vec::new();
        while let Ok(p) = inbox.try_recv() {
            raw.push(opcode(&p));
            if opcode(&p) == 0x110A {
                said.push(u16::from_le_bytes(payload(&p)[4..6].try_into().unwrap()));
            }
        }
        (raw, said)
    };
    assert_eq!(said, [1], "OnAttacked");
    assert!(raw.contains(&0x0605), "its blow: {raw:?}");
    // Before its next attack time, WhileCombat runs with HP 57 % and one attack counted.
    zones.lua(now + Duration::from_millis(500), &world, &defs, nothing, &mut || 0);
    assert_eq!(lines(&mut inbox), [2]);
    // Two more blows kill it: OnDeath with two attacks counted (the killing blow does not count).
    for _ in 0..2 {
        let _ = zones.attack_mob(1, 1, &attack, &defs, nothing, now, &mut || rolls.next().unwrap());
    }
    zones.lua(now + Duration::from_millis(600), &world, &defs, nothing, &mut || 0);
    assert_eq!(lines(&mut inbox), [12]);
    assert_eq!(zones.mob(1, 1).map(|m| m.hp), Some(0));
}

#[test]
fn unions_form_take_members_and_break_up() {
    use shaiya_proto::party as pp;
    let zones = Zones::default();
    let mut inboxes = Vec::new();
    for (id, name) in [(1, &b"Alice"[..]), (2, b"Bob"), (3, b"Carol"), (4, b"Dave"), (5, b"Eve")] {
        let (o, inbox, _) = member(id, 100.0 + id as f32, 100.0, 0, name, 10);
        zones.enter(1, o);
        inboxes.push(inbox);
    }
    let now = Instant::now();
    join(&zones, 1, 2);
    join(&zones, 1, 3);
    let all = |inboxes: &mut Vec<crate::outbox::OutboxInbox>| inboxes.iter_mut().map(received).collect::<Vec<_>>();
    all(&mut inboxes);
    let without_types =
        |packets: Vec<Vec<u8>>| packets.into_iter().filter(|p| opcodes(std::slice::from_ref(p)) != [pp::OP_PARTY_TYPE]).collect::<Vec<_>>();

    // Only the leader makes one; the sub-leader is index 2 (Carol), as the original picks it.
    zones.union_set(2, 1, false, 1);
    assert!(all(&mut inboxes).iter().all(Vec::is_empty));
    zones.union_set(1, 1, false, 1);
    let set = pp::union_set(pp::UnionSettings { leader_slot: 0, sub_leader_slot: 2, rule: 1, auto_join: false });
    let got = all(&mut inboxes);
    assert!(got[..3].iter().all(|p| *p == [set.clone()]));
    assert!(got[3].is_empty());

    // The sub-leader invites Dave by id; Dave says yes with 0x0B15: the others get 0x0B0C, Dave the 0x0B0B list of all four
    // and its own 0x0B0C (deviation 9).
    zones.party_invite(1, 3, Invitee::Id(4), true, now);
    assert_eq!(received(&mut inboxes[3]), [pp::union_by_id(pp::OP_UNION_INVITE, 3)]);
    zones.party_answer(1, 4, true, true, now);
    let got = all(&mut inboxes);
    for p in &got[..3] {
        assert_eq!(opcodes(&without_types(p.clone())), [pp::OP_UNION_ENTER]);
    }
    let list = without_types(got[3].clone());
    assert_eq!(opcodes(&list), [pp::OP_UNION_LIST, pp::OP_UNION_ENTER]);
    assert_eq!(shaiya_proto::frame::payload(&list[0])[7], 4, "itself included");
    assert_eq!(list[1], got[0][0], "the 0x0B0C the others got");
    // Set up again: ignored (deviation 8).
    zones.union_set(1, 1, true, 2);
    assert!(all(&mut inboxes).iter().all(Vec::is_empty));
    // A member who does not lead invites nobody: 0x0B19 2.
    zones.party_invite(1, 2, Invitee::Id(5), true, now);
    assert_eq!(received(&mut inboxes[1]), [pp::union_failed(u32::from(pp::NOT_LEADER), 2)]);

    // Status and chat use the union's packets.
    zones.party_level(2, 11);
    assert_eq!(received(&mut inboxes[0]), [pp::level(true, 2, 11)]);
    assert!(zones.union_chat(3, b"hi") && !zones.union_chat(2, b"hi"), "the leader's or the sub-leader's");
    all(&mut inboxes);

    // /join: not without auto-join (13); then Eve joins from the name alone.
    zones.union_join(5, b"Alice", now);
    assert_eq!(received(&mut inboxes[4]), [pp::union_failed(pp::UNION_NO_AUTO_JOIN, 0)]);
    zones.union_auto_join(1, true);
    assert_eq!(received(&mut inboxes[1]), [pp::union_auto_join(true)]);
    zones.union_join(5, b"alice", now);
    assert_eq!(opcodes(&without_types(received(&mut inboxes[4]))), [pp::OP_UNION_LIST, pp::OP_UNION_ENTER]);
    all(&mut inboxes);

    // The sub-leader kicked: 0x0B1A to everyone, it included, then Bob (the first not leading) is sub-leader.
    zones.union_kick(1, 3);
    let got = all(&mut inboxes);
    assert_eq!(without_types(got[1].clone()), [pp::union_by_id(pp::OP_UNION_KICK, 3), pp::union_by_id(pp::OP_UNION_SUB_LEADER, 2)]);
    assert_eq!(without_types(got[2].clone()), [pp::union_by_id(pp::OP_UNION_KICK, 3)]);
    // 0x0B06 acts as 0x0B1A in a union (deviation 5).
    zones.party_kick(1, 4);
    assert_eq!(without_types(received(&mut inboxes[0])), [pp::union_by_id(pp::OP_UNION_KICK, 4)]);
    all(&mut inboxes);

    // Groups, for the party heal without the caster: Eve (slot 4) moved to group 1 leaves Alice and Bob in group 0.
    zones.union_move(2, 4, 6);
    assert_eq!(zones.union_group(1), Some(vec![1, 2]));
    assert_eq!(zones.union_group(5), Some(vec![5]));
    assert_eq!(zones.union_group(4), None, "kicked: no union");
    all(&mut inboxes);

    // Dismantled: 0x0B16 to all, and nobody is left in a party.
    zones.union_break(2);
    assert!(all(&mut inboxes).iter().all(Vec::is_empty), "the leader's");
    zones.union_break(1);
    let got = all(&mut inboxes);
    for i in [0, 1, 4] {
        assert_eq!(without_types(got[i].clone()), [pp::union_break()]);
    }
    assert!(!zones.in_party(1) && !zones.in_party(2) && !zones.in_party(5));
}

#[test]
fn parties_form_change_leaders_and_break_up() {
    use shaiya_proto::party as pp;
    let zones = Zones::default();
    let (a, mut a_in, _) = member(1, 100.0, 100.0, 0, b"Alice", 10);
    let (b, mut b_in, _) = member(2, 110.0, 100.0, 0, b"Bob", 10);
    let (c, mut c_in, _) = member(3, 120.0, 100.0, 0, b"Carol", 10);
    let (d, mut d_in, _) = member(4, 130.0, 100.0, 1, b"Dark", 10);
    for o in [a, b, c, d] {
        zones.enter(1, o);
    }
    let now = Instant::now();
    for inbox in [&mut a_in, &mut b_in, &mut c_in, &mut d_in] {
        drain(inbox);
    }
    // Failures: oneself, the other country (code 4), someone with an invitation pending (3).
    zones.party_invite(1, 1, Invitee::Id(1), false, now);
    zones.party_invite(1, 1, Invitee::Id(4), false, now);
    zones.party_invite(1, 1, Invitee::Id(2), false, now);
    assert_eq!(received(&mut b_in), [pp::invitation(1)]);
    zones.party_invite(1, 3, Invitee::Id(2), false, now);
    assert_eq!(received(&mut a_in), [pp::failed(4, 1), pp::failed(4, 1)]);
    assert_eq!(received(&mut c_in), [pp::failed(pp::IN_ANOTHER_PARTY, 2)]);
    assert!(received(&mut d_in).is_empty());

    // Bob says yes: Alice's new party lists nobody, then hears of Bob; Bob gets the list with Alice; both types to the view.
    zones.party_answer(1, 2, true, false, now);
    assert_eq!(opcodes(&received(&mut a_in)), [pp::OP_LIST, pp::OP_PARTY_TYPE, pp::OP_ENTER, pp::OP_PARTY_TYPE]);
    let for_bob = received(&mut b_in);
    assert_eq!(opcodes(&for_bob), [pp::OP_PARTY_TYPE, pp::OP_LIST, pp::OP_PARTY_TYPE]);
    assert_eq!(payload(&for_bob[1])[..6], [0, 1, 1, 0, 0, 0], "leader slot 0, one member: Alice");
    assert_eq!(received(&mut c_in), [pp::party_type(1, pp::TYPE_LEADER), pp::party_type(2, pp::TYPE_MEMBER)]);
    assert_eq!(payload(&zones.shape(1, 1, 1)[1])[13], pp::TYPE_LEADER, "the looks carry the party type");
    assert!(zones.in_party(2));

    // Party chat reaches both; a member who does not lead cannot invite or hand over the lead.
    assert!(zones.party_chat(2, b"hi"));
    assert_eq!((received(&mut a_in), received(&mut b_in)), (vec![b"hi".to_vec()], vec![b"hi".to_vec()]));
    assert!(!zones.party_chat(3, b"hi"));
    zones.party_invite(1, 2, Invitee::Id(3), false, now);
    zones.party_leader(2, 1);
    assert_eq!(received(&mut b_in), [pp::failed(pp::NOT_LEADER, 2), pp::leader(0)]);

    // Carol by name: she gets Alice's name; once in, she is told of both, and they of her.
    zones.party_invite(1, 1, Invitee::Name(b"carol"), false, now);
    assert_eq!(received(&mut c_in), [pp::invitation_by_name(b"Alice")]);
    zones.party_answer(1, 3, true, false, now);
    assert_eq!(opcodes(&received(&mut a_in)), [pp::OP_ENTER, pp::OP_PARTY_TYPE]);
    assert_eq!(opcodes(&received(&mut b_in)), [pp::OP_ENTER, pp::OP_PARTY_TYPE]);
    assert_eq!(opcodes(&received(&mut c_in)), [pp::OP_LIST, pp::OP_PARTY_TYPE]);
    zones.party_invite(1, 1, Invitee::Id(2), false, now);
    assert_eq!(received(&mut a_in), [pp::failed(pp::IN_THE_PARTY, 2)]);

    // Bob leads; Alice kicks nobody (no longer leader); Bob kicks Alice; Carol leaving ends the party for Bob too.
    zones.party_leader(1, 2);
    assert_eq!(received(&mut c_in), [pp::leader(2), pp::party_type(2, pp::TYPE_LEADER), pp::party_type(1, pp::TYPE_MEMBER)]);
    zones.party_kick(1, 3);
    zones.party_kick(2, 1);
    assert_eq!(received(&mut c_in), [pp::kicked(1), pp::party_type(1, pp::TYPE_NONE)]);
    assert!(!zones.in_party(1));
    drain(&mut b_in);
    zones.party_leave(3);
    assert_eq!(received(&mut b_in), [pp::left(3), pp::left(2), pp::party_type(2, pp::TYPE_NONE), pp::party_type(3, pp::TYPE_NONE)]);
    assert!(!zones.in_party(2) && !zones.in_party(3));
}

#[test]
fn invitations_need_an_answer_within_30_seconds() {
    let zones = Zones::default();
    let (a, mut a_in, _) = member(1, 100.0, 100.0, 0, b"Alice", 10);
    let (b, _, _) = member(2, 110.0, 100.0, 0, b"Bob", 10);
    zones.enter(1, a);
    zones.enter(1, b);
    let now = Instant::now();
    zones.party_invite(1, 1, Invitee::Id(2), false, now);
    zones.expire_invites(now + Duration::from_secs(30));
    zones.party_answer(1, 2, true, false, now);
    assert!(!zones.in_party(2), "too late");
    drain(&mut a_in);
    zones.party_invite(1, 1, Invitee::Id(2), false, now);
    zones.party_answer(1, 2, false, false, now);
    assert_eq!(received(&mut a_in), [shaiya_proto::party::failed(shaiya_proto::party::REJECTED, 2)]);
}

#[test]
fn a_party_kill_is_shared_near_the_killer() {
    use crate::combat::{self, Gear};
    use crate::mobs::{MobDef, MobDefs, SpawnArea};
    let mut world = World::build([(1, 2048)], &std::collections::HashSet::new(), []);
    world.set_mob_data(
        [(1, vec![0u8; 2048 * 2048 / 8])].into(),
        [(1, vec![SpawnArea::new([100.0, 0.0, 105.0, 100.0, 0.0, 105.0], vec![(7, 1)])])].into(),
    );
    let defs = MobDefs::new([MobDef { id: 7, level: 1, hp: 30, size: 1, country: 2, money: 90, ..Default::default() }]);
    let zones = Zones::new(&world, &defs);
    let (a, _a_in, mut a_events) = member(1, 100.0, 100.0, 0, b"Alice", 10);
    let (b, _b_in, mut b_events) = member(2, 100.0, 199.0, 0, b"Bob", 20);
    let (c, _c_in, mut c_events) = member(3, 100.0, 201.0, 0, b"Carol", 12);
    for o in [a, b, c] {
        zones.enter(1, o);
    }
    join(&zones, 1, 2);
    join(&zones, 1, 3);
    // The game log's records of joining the party are not the test's business.
    for events in [&mut a_events, &mut b_events, &mut c_events] {
        while events.try_recv().is_ok() {}
    }
    let now = Instant::now();
    zones.respawn(now, &world, &defs, &mut || 0);
    let fighter = combat::player(10, [100, 10, 10, 10, 10, 0], &Gear { kind: Some(combat::MELEE), ..Default::default() });
    let attack =
        Attack { attacker: 1, country: 0, fighter: &fighter, kind: combat::MELEE, slack: 2.0, range: 0, rec: 0, cross_road: false };
    let (no_drops, no_items) = (DropTables::default(), ItemDefs::default());
    // Rolls of 0: a hit, and the 30 % gold roll passes.
    let AttackOutcome::Struck { killed, .. } = zones.attack_mob(1, 1, &attack, &defs, (&no_drops, &no_items), now, &mut || 0) else {
        panic!("a hit")
    };
    assert!(killed);
    let share = ZoneEvent::PartyKill { mob_id: 7, killer: 1, members: 3, union: false, max_level: 20 };
    // Bob is 99 from Alice, Carol 101: out of the share.
    assert_eq!(a_events.try_recv().ok(), Some(share.clone()));
    assert_eq!(a_events.try_recv().ok(), Some(ZoneEvent::Gold { amount: 30, from_mob: true }), "90 over three members");
    assert_eq!(b_events.try_recv().ok(), Some(share.clone()));
    assert_eq!(b_events.try_recv().ok(), Some(ZoneEvent::Gold { amount: 30, from_mob: true }));
    assert!(c_events.try_recv().is_err());
    assert!(zones.ground.guard().get(&1).is_none_or(|g| g.items.is_empty()), "no gold on the ground");
}

#[test]
fn party_gold_for_a_member_whose_connection_is_gone_lies_on_the_ground() {
    use crate::combat::{self, Gear};
    use crate::mobs::{MobDef, MobDefs, SpawnArea};
    // Audit R14: Bob's connection has ended (its events receiver is dropped) but he is still in the zone.
    let mut world = World::build([(1, 2048)], &std::collections::HashSet::new(), []);
    world.set_mob_data(
        [(1, vec![0u8; 2048 * 2048 / 8])].into(),
        [(1, vec![SpawnArea::new([100.0, 0.0, 105.0, 100.0, 0.0, 105.0], vec![(7, 1)])])].into(),
    );
    let defs = MobDefs::new([MobDef { id: 7, level: 1, hp: 30, size: 1, country: 2, money: 90, ..Default::default() }]);
    let zones = Zones::new(&world, &defs);
    let (a, _a_in, mut a_events) = member(1, 100.0, 100.0, 0, b"Alice", 10);
    let (b, _b_in, b_events) = member(2, 100.0, 150.0, 0, b"Bob", 20);
    for o in [a, b] {
        zones.enter(1, o);
    }
    join(&zones, 1, 2);
    while a_events.try_recv().is_ok() {}
    drop(b_events);
    let now = Instant::now();
    zones.respawn(now, &world, &defs, &mut || 0);
    let fighter = combat::player(10, [100, 10, 10, 10, 10, 0], &Gear { kind: Some(combat::MELEE), ..Default::default() });
    let attack =
        Attack { attacker: 1, country: 0, fighter: &fighter, kind: combat::MELEE, slack: 2.0, range: 0, rec: 0, cross_road: false };
    let (no_drops, no_items) = (DropTables::default(), ItemDefs::default());
    let AttackOutcome::Struck { killed, .. } = zones.attack_mob(1, 1, &attack, &defs, (&no_drops, &no_items), now, &mut || 0) else {
        panic!("a hit")
    };
    assert!(killed);
    assert!(
        std::iter::from_fn(|| a_events.try_recv().ok()).any(|e| e == ZoneEvent::Gold { amount: 45, from_mob: true }),
        "Alice's half reaches her"
    );
    let ground: Vec<(Loot, u32)> = ground_items(&zones).iter().map(|i| (i.loot, i.owner)).collect();
    assert_eq!(ground, [(Loot::Gold(45), 2)], "Bob's half lies at his feet, his alone");
}

#[test]
fn a_union_led_for_loot_hands_the_leader_its_items() {
    use crate::combat::{self, Gear};
    use crate::inventory::ItemDef;
    use crate::mobs::{MobDef, MobDefs, SpawnArea};
    // Loot rule 4 with a sword of `req_dex`: the leader (Alice, 50 from Bob the killer) gets it, or it lies on the ground as Bob's.
    let kill = |req_dex: u16| {
        let mut world = World::build([(1, 2048)], &std::collections::HashSet::new(), []);
        world.set_mob_data(
            [(1, vec![0u8; 2048 * 2048 / 8])].into(),
            [(1, vec![SpawnArea::new([100.0, 0.0, 155.0, 100.0, 0.0, 155.0], vec![(7, 1)])])].into(),
        );
        let mut drops = [(0, 0); 10];
        drops[1] = (5, 100);
        let defs = MobDefs::new([MobDef { id: 7, level: 1, hp: 30, size: 1, country: 2, drops, ..Default::default() }]);
        let items = ItemDefs::new([ItemDef { item_type: 1, type_id: 1, grade: 5, quality: 50, req_dex, ..Default::default() }]);
        let tables = DropTables::new(&items);
        let zones = Zones::new(&world, &defs);
        let (a, _a_in, mut a_events) = member(1, 100.0, 100.0, 0, b"Alice", 10);
        let (b, _b_in, mut b_events) = member(2, 100.0, 150.0, 0, b"Bob", 10);
        let (c, _c_in, _) = member(3, 100.0, 400.0, 0, b"Carol", 10);
        for o in [a, b, c] {
            zones.enter(1, o);
        }
        join(&zones, 1, 2);
        join(&zones, 1, 3);
        zones.union_set(1, 1, false, crate::party::RULE_LEADER);
        for events in [&mut a_events, &mut b_events] {
            while events.try_recv().is_ok() {}
        }
        let now = Instant::now();
        zones.respawn(now, &world, &defs, &mut || 0);
        let fighter = combat::player(10, [100, 10, 10, 10, 10, 0], &Gear { kind: Some(combat::MELEE), ..Default::default() });
        let attack =
            Attack { attacker: 2, country: 0, fighter: &fighter, kind: combat::MELEE, slack: 2.0, range: 0, rec: 0, cross_road: false };
        let outcome = zones.attack_mob(1, 1, &attack, &defs, (&tables, &items), now, &mut || 0);
        let AttackOutcome::Struck { killed, .. } = outcome else { panic!("a hit: {outcome:?}") };
        assert!(killed);
        let ground: Vec<(u32, u32)> =
            zones.ground.guard().get(&1).map_or(Vec::new(), |g| g.items.values().map(|i| (i.owner, i.party)).collect());
        (a_events, b_events, ground)
    };
    let (mut a_events, mut b_events, ground) = kill(3);
    let share = ZoneEvent::PartyKill { mob_id: 7, killer: 2, members: 3, union: true, max_level: 10 };
    assert_eq!(b_events.try_recv().ok(), Some(share.clone()), "a union's kill: its table");
    assert_eq!(a_events.try_recv().ok(), Some(share));
    let loot = std::iter::from_fn(|| a_events.try_recv().ok()).find(|e| matches!(e, ZoneEvent::Loot { .. }));
    assert!(matches!(loot, Some(ZoneEvent::Loot { owner: 1, .. })), "straight to the leader, its owner: {loot:?}");
    assert!(ground.is_empty());
    let (mut a_events, _, ground) = kill(0);
    assert!(!std::iter::from_fn(|| a_events.try_recv().ok()).any(|e| matches!(e, ZoneEvent::Loot { .. })));
    assert_eq!(ground.len(), 1, "ReqDex 0: on the ground");
    assert_eq!(ground[0].0, 2, "the killer's");
}

#[test]
fn members_see_each_others_health_death_and_steps() {
    use shaiya_proto::party as pp;
    let zones = Zones::default();
    let (a, mut a_in, _) = member(1, 100.0, 100.0, 0, b"Alice", 10);
    let (b, mut b_in, _) = member(2, 110.0, 100.0, 0, b"Bob", 10);
    zones.enter(1, a);
    zones.enter(1, b);
    join(&zones, 1, 2);
    zones.party_status();
    received(&mut a_in);
    received(&mut b_in);
    zones.party_status();
    assert!(received(&mut a_in).is_empty(), "nothing changed");
    edit(&zones, 2, |p| p.occupant.vitals.health = [50, 40, 30]);
    zones.party_status();
    assert_eq!(received(&mut a_in), [pp::status_all(false, false, 2, [50, 30, 40])], "HP, SP, MP");
    assert!(received(&mut b_in).is_empty(), "not to the player itself");
    edit(&zones, 2, |p| p.dead = true);
    zones.party_status();
    assert_eq!(received(&mut a_in), [pp::death(false, 2)]);
    // Every fourth step goes to the members on the map.
    for x in [111.0, 112.0, 113.0] {
        zones.move_to(1, 2, &step(x, 100.0));
    }
    assert!(!received(&mut a_in).iter().any(|p| opcode(p) == pp::OP_POSITION));
    zones.move_to(1, 2, &step(114.0, 100.0));
    assert!(received(&mut a_in).contains(&pp::position(false, 2, 1, [114.0, 0.0, 100.0])));
    zones.party_level(2, 11);
    assert_eq!(received(&mut a_in), [pp::level(false, 2, 11)]);
    assert_eq!(received(&mut b_in).last(), Some(&pp::level(false, 2, 11)), "itself included");
    assert_eq!(zones.party_leader_spot(1, 2), Some([100.0, 0.0, 100.0]), "back to life next to Alice");
    assert_eq!(zones.party_leader_spot(1, 1), None, "the leader itself");
}

#[test]
fn party_buffs_reach_the_members_near() {
    let (zones, defs, mut b_in, _) = skill_ground(0);
    edit(&zones, 2, |p| p.occupant.shape.name = b"Bob".to_vec());
    join(&zones, 1, 2);
    received(&mut b_in);
    let party = SkillDef { id: 31, apply_range: 50, ..sure_buff(skills::TARGET_PARTY, buffs::ABILITIES, 10) };
    let out = use_on(&zones, &defs, SkillTarget::Player(1), &party).unwrap();
    // The announcement, then the caster's own 0x0513; Bob, 45 away, gets his.
    assert_eq!(opcodes(&out.packets)[..2], [sk::OP_USE_USER, sk::OP_USE_PARTY]);
    assert!(received(&mut b_in).iter().any(|p| opcode(p) == sk::OP_APPLIED), "Bob is buffed");
    let around = SkillDef { id: 32, apply_range: 40, ..sure_buff(skills::TARGET_PARTY_AROUND, buffs::ABILITIES, 10) };
    let out = use_on(&zones, &defs, SkillTarget::Player(1), &around).unwrap();
    assert_eq!(opcodes(&out.packets)[..1], [sk::OP_USE_USER]);
    assert!(!opcodes(&out.packets).contains(&sk::OP_APPLIED), "not the caster");
    assert!(!received(&mut b_in).iter().any(|p| opcode(p) == sk::OP_APPLIED), "Bob is 45 away, the range 40");
}

#[test]
fn trades_need_the_same_faction_except_where_allowed() {
    use shaiya_proto::trade as tp;
    let zones = Zones::default();
    let (a, mut a_in, _) = member(1, 100.0, 100.0, 0, b"Alice", 10);
    let (b, mut b_in, _) = member(2, 105.0, 100.0, 0, b"Bob", 10);
    let (d, mut d_in, _) = member(4, 103.0, 100.0, 1, b"Dark", 10);
    for o in [a, b, d] {
        zones.enter(1, o);
    }
    for inbox in [&mut a_in, &mut b_in, &mut d_in] {
        drain(inbox);
    }
    let now = Instant::now();
    // Oneself, the other faction, nobody of that id on the map (no answer at all).
    zones.trade_request(1, 1, 1, now);
    zones.trade_request(1, 1, 4, now);
    zones.trade_request(1, 1, 9, now);
    assert_eq!(received(&mut a_in), [tp::start(1, tp::START_BUSY), tp::start(4, tp::START_BUSY)]);
    assert!(received(&mut d_in).is_empty());
    // Where both factions may trade (the Auction House by default).
    put_rules(&zones, &[("any", serde_json::json!({ "type": "cross_faction_trade" }))]);
    zones.trade_request(1, 1, 4, now);
    put_rules(&zones, &[]);
    assert_eq!(received(&mut d_in), [tp::request(1)]);
    // Dark is busy now; so is Alice.
    zones.trade_request(1, 2, 4, now);
    zones.trade_request(1, 1, 2, now);
    assert_eq!(received(&mut b_in), [tp::start(4, tp::START_BUSY)]);
    assert_eq!(received(&mut a_in), [tp::start(2, tp::START_BUSY)]);
    // Dying while asked refuses for the asked player (`CancleActionExc`, state 2).
    assert!(zones.trade_end(4));
    assert_eq!(received(&mut a_in), [tp::start(4, tp::START_REFUSED)]);
    // Expired requests free both, silently.
    zones.trade_request(1, 1, 2, now);
    drain(&mut b_in);
    zones.expire_trades(now + crate::trade::REQUEST_TIME);
    zones.trade_answer(2, true);
    assert!(received(&mut a_in).is_empty() && received(&mut b_in).is_empty());
}

#[test]
fn two_ride_one_mount_and_a_hit_puts_both_down() {
    use shaiya_proto::vehicle as vp;
    let zones = Zones::default();
    let (a, mut a_in) = player(1, 100.0, 100.0);
    let (b, mut b_in) = player(2, 105.0, 100.0);
    zones.enter(1, a);
    zones.enter(1, b);
    drain(&mut a_in);
    drain(&mut b_in);
    let now = Instant::now();

    // A summons (0x0217 to B), then rides a two-seat mount (0x051D 24 to both).
    assert_eq!(zones.start_summon(1, 1), vp::ready(1));
    assert_eq!(received(&mut b_in), [vp::ready(1)]);
    let two_seat = Mount { shape: 24, two_seat: true };
    assert_eq!(zones.mount(1, 1, two_seat), Some(sk::shape(1, 24)));
    assert_eq!(received(&mut b_in), [sk::shape(1, 24)]);
    assert_eq!(zones.mount(1, 1, two_seat), None, "only a summon completes");

    // A asks B (0x021D); B says yes: A gets the answer, both get 0x021C (B behind A).
    assert_eq!(zones.ask_to_ride(1, 1, 2, now), None);
    assert_eq!(received(&mut b_in), [vp::ask(1)]);
    assert_eq!(zones.ask_to_ride(1, 1, 2, now), Some(vp::answer(vp::ASK_FAILED)), "B is asked already");
    zones.answer_ride(1, 2, vp::ANSWER_YES, now);
    assert_eq!(received(&mut a_in), [vp::answer(vp::ANSWER_YES), vp::behind(2, 1)]);
    assert_eq!(received(&mut b_in), [vp::behind(2, 1)]);
    assert_eq!(zones.riding(1, 2), Riding::Behind);
    assert!(zones.whereabouts(1, 2).unwrap().mounted);

    // A hit on the passenger puts it down, and its driver gets off too.
    vehicle::casting_stop(zones.maps.guard().get_mut(&1).unwrap(), 2);
    assert_eq!(received(&mut a_in), [vp::behind(2, 0), sk::shape(1, 0)]);
    assert_eq!((zones.riding(1, 1), zones.riding(1, 2)), (Riding::OnFoot, Riding::OnFoot));

    // Riding alone, A gets off with 0x0216: the shape to B; nothing to get off from afterwards.
    zones.start_summon(1, 1);
    zones.mount(1, 1, Mount { shape: 14, two_seat: false });
    drain(&mut b_in);
    assert_eq!(zones.ask_to_ride(1, 1, 2, now), Some(vp::answer(vp::ASK_FAILED)), "one seat only");
    assert_eq!(zones.dismount(1, 1), Some(sk::shape(1, 0)));
    assert_eq!(received(&mut b_in), [sk::shape(1, 0)]);
    assert_eq!(zones.dismount(1, 1), None);
}

#[test]
fn leaving_puts_a_passenger_down_and_tells_an_asker() {
    use shaiya_proto::vehicle as vp;
    let zones = Zones::default();
    let (a, _a_in) = player(1, 100.0, 100.0);
    let (b, mut b_in) = player(2, 105.0, 100.0);
    let (c, mut c_in) = player(3, 110.0, 100.0);
    for o in [a, b, c] {
        zones.enter(1, o);
    }
    let now = Instant::now();
    zones.start_summon(1, 1);
    zones.mount(1, 1, Mount { shape: 24, two_seat: true });
    zones.ask_to_ride(1, 1, 2, now);
    zones.answer_ride(1, 2, vp::ANSWER_YES, now);
    // A, with B behind, leaves: B is put down (0x021C with 0 to its view).
    received(&mut b_in);
    zones.leave(1, 1);
    assert!(received(&mut b_in).contains(&vp::behind(2, 0)));
    assert_eq!(zones.riding(1, 2), Riding::OnFoot);
    // C rides and asks B; B leaves before answering: C is told the asking failed.
    zones.start_summon(1, 3);
    zones.mount(1, 3, Mount { shape: 24, two_seat: true });
    assert_eq!(zones.ask_to_ride(1, 3, 2, now), None);
    received(&mut c_in);
    zones.leave(1, 2);
    assert!(received(&mut c_in).contains(&vp::answer(vp::ASK_FAILED)));
}

/// A player's normal blows on map 1: STR `str` and nothing else, so its melee attack is `trunc(str × 1.3)`.
fn blows(level: u16, str: i32) -> combat::Fighter {
    combat::player(level, [str, 0, 0, 0, 0, 0], &combat::Gear { kind: Some(combat::MELEE), ..Default::default() })
}

fn strike(zones: &Zones, defs: &MobDefs, attacker: u32, fighter: &combat::Fighter, target: u32, war: bool) -> AttackOutcome {
    let attack = Attack { attacker, country: 1, fighter, kind: combat::MELEE, slack: 2.0, range: 0, rec: 0, cross_road: false };
    // Hit (0), no critical (99), then a reflection's roll (0).
    let mut rolls = [0, 99, 0].into_iter().cycle();
    zones.attack_player(1, target, &attack, war, defs, no_loot(), Instant::now(), &mut || rolls.next().unwrap())
}

fn events(inbox: &mut UnboundedReceiver<ZoneEvent>) -> Vec<ZoneEvent> {
    std::iter::from_fn(|| inbox.try_recv().ok()).collect()
}

#[test]
fn players_kill_players_of_the_other_country() {
    let (_, _, defs) = fighting_ground();
    let zones = Zones::default();
    let (a, mut a_in, mut a_ev) = member(1, 100.0, 100.0, 0, b"Alice", 20);
    let (b, mut b_in, mut b_ev) = member(2, 100.0, 102.0, 1, b"Bob", 20);
    let (c, mut c_in, _) = member(3, 100.0, 150.0, 0, b"Carol", 20);
    let (d, mut d_in, _) = member(4, 100.0, 300.0, 0, b"Dave", 20);
    for o in [a, b, c, d] {
        zones.enter(1, o);
    }
    for inbox in [&mut a_in, &mut b_in, &mut c_in, &mut d_in] {
        received(inbox);
    }

    // Enemies by country; nobody on a war map; never oneself.
    assert!(zones.may_attack_player(1, 2, 1, false));
    assert!(zones.may_attack_player(1, 1, 2, false));
    assert!(!zones.may_attack_player(1, 1, 3, false), "same country");
    assert!(!zones.may_attack_player(1, 2, 1, true), "a war map");
    assert!(!zones.may_attack_player(1, 2, 2, false));
    assert_eq!(strike(&zones, &defs, 2, &blows(20, 100), 1, true), AttackOutcome::Gone);
    assert_eq!(strike(&zones, &defs, 1, &blows(20, 100), 3, false), AttackOutcome::Gone);
    assert!(received(&mut a_in).is_empty() && received(&mut c_in).is_empty());

    // Out of range: 2 + 0 + 2.3 from Bob.
    edit(&zones, 2, |p| p.occupant.position = [100.0, 0.0, 105.0]);
    assert_eq!(strike(&zones, &defs, 2, &blows(20, 100), 1, false), AttackOutcome::OutOfRange);
    edit(&zones, 2, |p| p.occupant.position = [100.0, 0.0, 102.0]);

    // 129 (100 × 1.3f32, truncated) against 100 HP: Alice dies by Bob's hand. 0x0504 to the view first, then 0x0502 to those within 60, the attacker's
    // own through its outbox; Dave, 200 away, sees neither.
    let out = strike(&zones, &defs, 2, &blows(20, 100), 1, false);
    assert_eq!(out, AttackOutcome::Struck { packets: Vec::new(), killed: true, critical: false });
    let death = cp::death(1, cp::KILLER_PLAYER, 2);
    let blow = cp::attack_user(combat::RESULT_HIT, 2, 1, [129, 0, 0]);
    for inbox in [&mut a_in, &mut b_in, &mut c_in] {
        assert_eq!(received(inbox), [death.clone(), blow.clone()]);
    }
    assert!(received(&mut d_in).is_empty());
    assert_eq!(
        events(&mut a_ev),
        [
            ZoneEvent::Struck,
            ZoneEvent::Worn,
            ZoneEvent::Died { killer_kind: cp::KILLER_PLAYER, killer: 2, loss: Some(2), counted: true, keep_exp: false, drop_exp: 5 }
        ]
    );
    assert_eq!(events(&mut b_ev), [ZoneEvent::PlayerKill { victim: 1, victim_level: 20, last_hit: 2, party: None }]);
    assert_eq!(zones.health(1, 1), Some([0; 3]));
    edit(&zones, 1, |p| assert!(p.harm.is_empty(), "the record goes at death"));
    assert_eq!(strike(&zones, &defs, 2, &blows(20, 100), 1, false), AttackOutcome::Gone, "dead");
    assert!(!zones.may_attack_player(1, 2, 1, false));

    // The kills show in the view and in the looks.
    assert_eq!(zones.set_kills(1, 2, 1), cp::kill_info(2, 1));
    assert_eq!(received(&mut a_in), [cp::kill_info(2, 1)]);
    assert!(received(&mut b_in).is_empty(), "its own copy is returned");

    let mut world = World::build([(40, 2048), (42, 2048)], &std::collections::HashSet::new(), []);
    world.set_war_maps([40, 42]);
    assert!(world.is_war_map(42) && !world.is_war_map(1));
}

#[test]
fn the_killer_of_record_did_the_most_damage() {
    let (world, _, defs) = fighting_ground();
    // Alice (level 0, as in `monsters_fight_back_and_kill`) fights the monster at (100, 105) and Bob (Fury) strikes her: the
    // monster's 70 against Bob's 40 or 80.
    let ground = || {
        let zones = Zones::new(&world, &defs);
        let (a, mut a_in, a_ev) = member(1, 100.0, 102.0, 0, b"Alice", 0);
        let (b, mut b_in, b_ev) = member(2, 100.0, 100.0, 1, b"Bob", 20);
        zones.enter(1, a);
        zones.enter(1, b);
        zones.respawn(Instant::now(), &world, &defs, &mut || 0);
        let attack = Attack {
            attacker: 1,
            country: 0,
            fighter: &blows(20, 1),
            kind: combat::MELEE,
            slack: 2.0,
            range: 0,
            rec: 0,
            cross_road: false,
        };
        let mut rolls = [0, 99].into_iter().cycle();
        zones.attack_mob(1, 1, &attack, &defs, no_loot(), Instant::now(), &mut || rolls.next().unwrap());
        received(&mut a_in);
        received(&mut b_in);
        (zones, a_in, a_ev, b_ev)
    };
    let monster_hits = |zones: &Zones| {
        let mut rolls = [0, 99].into_iter().cycle();
        zones.mob_fight(Instant::now(), &world, &defs, no_loot(), &mut || rolls.next().unwrap());
    };

    // The monster first (70), Bob's 40 finishes her: the monster killed her. No item loss, no counts, Bob gets nothing.
    let (zones, mut a_in, mut a_ev, mut b_ev) = ground();
    monster_hits(&zones);
    assert_eq!(zones.health(1, 1).map(|h| h[0]), Some(30));
    let out = strike(&zones, &defs, 2, &blows(20, 31), 1, false);
    assert!(matches!(out, AttackOutcome::Struck { killed: true, .. }));
    let seen = received(&mut a_in);
    assert!(seen.contains(&cp::death(1, cp::KILLER_MOB, 1)));
    assert!(events(&mut a_ev).contains(&ZoneEvent::Died {
        killer_kind: cp::KILLER_MOB,
        killer: 1,
        loss: None,
        counted: false,
        keep_exp: false,
        drop_exp: 5
    }));
    assert!(events(&mut b_ev).is_empty());

    // Bob first (80), the monster finishes her: Bob killed her, though not by the last blow; the monster drops its target.
    let (zones, mut a_in, mut a_ev, mut b_ev) = ground();
    strike(&zones, &defs, 2, &blows(20, 62), 1, false);
    assert_eq!(zones.health(1, 1).map(|h| h[0]), Some(20));
    monster_hits(&zones);
    assert_eq!(zones.health(1, 1), Some([0; 3]));
    assert!(received(&mut a_in).contains(&cp::death(1, cp::KILLER_PLAYER, 2)));
    assert!(events(&mut a_ev).contains(&ZoneEvent::Died {
        killer_kind: cp::KILLER_PLAYER,
        killer: 2,
        loss: Some(2),
        counted: true,
        keep_exp: false,
        drop_exp: 5
    }));
    assert_eq!(events(&mut b_ev), [ZoneEvent::PlayerKill { victim: 1, victim_level: 0, last_hit: 0, party: None }]);
    assert_eq!(zones.mob(1, 1).map(|m| m.state), Some(MobState::Idle));

    // Bob's 80, then full HP again: the record is cleared, so the monster's 70 outweighs Bob's 40 at the end.
    let (zones, mut a_in, mut a_ev, _) = ground();
    strike(&zones, &defs, 2, &blows(20, 62), 1, false);
    edit(&zones, 1, |p| assert_eq!(p.harm, [pvp::Harm { kind: cp::KILLER_PLAYER, id: 2, total: 80 }]));
    zones.fill(1, 1, [100; 3]);
    edit(&zones, 1, |p| assert!(p.harm.is_empty(), "full HP clears the record"));
    monster_hits(&zones);
    strike(&zones, &defs, 2, &blows(20, 31), 1, false);
    assert!(received(&mut a_in).contains(&cp::death(1, cp::KILLER_MOB, 1)));
    assert!(events(&mut a_ev).contains(&ZoneEvent::Died {
        killer_kind: cp::KILLER_MOB,
        killer: 1,
        loss: None,
        counted: false,
        keep_exp: false,
        drop_exp: 5
    }));
}

#[test]
fn a_reflected_blow_kills_the_attacker() {
    let (_, _, defs) = fighting_ground();
    let zones = Zones::default();
    let counter = SkillDef {
        id: 89,
        level: 1,
        type_effect: skills::BUFF,
        kind: buffs::MIRROR,
        keep_time: 60,
        abilities: [(0, 1), (0, 0), (0, 0)],
        success_type: 1,
        success_value: 100,
        ..Default::default()
    };
    let (mut a, mut a_in, mut a_ev) = member(1, 100.0, 100.0, 0, b"Alice", 20);
    a.buffs.apply(&counter, 1, Instant::now(), &mut || 1);
    let (b, mut b_in, mut b_ev) = member(2, 100.0, 102.0, 1, b"Bob", 20);
    zones.enter(1, a);
    zones.enter(1, b);
    received(&mut a_in);
    received(&mut b_in);

    // Alice takes nothing (0x0502 shows 0); Bob takes his own 129 and dies by her (0x0504), then 0x0515.
    let out = strike(&zones, &defs, 2, &blows(20, 100), 1, false);
    assert_eq!(out, AttackOutcome::Struck { packets: Vec::new(), killed: false, critical: false });
    assert_eq!(zones.health(1, 1), Some([100; 3]));
    assert_eq!(zones.health(1, 2), Some([0; 3]));
    let order = [cp::attack_user(combat::RESULT_HIT, 2, 1, [0; 3]), cp::death(2, cp::KILLER_PLAYER, 1), cp::mirror_user(1, 2, [129, 0, 0])];
    assert_eq!(received(&mut b_in), order);
    assert_eq!(received(&mut a_in), order);
    assert!(events(&mut b_ev).contains(&ZoneEvent::Died {
        killer_kind: cp::KILLER_PLAYER,
        killer: 1,
        loss: Some(1),
        counted: true,
        keep_exp: false,
        drop_exp: 5
    }));
    assert!(events(&mut a_ev).contains(&ZoneEvent::PlayerKill { victim: 2, victim_level: 20, last_hit: 1, party: None }));
}

#[test]
fn a_party_shares_a_player_kill_and_what_the_victim_loses() {
    let (_, _, defs) = fighting_ground();
    let zones = Zones::default();
    let (a, mut a_in, mut a_ev) = member(1, 100.0, 100.0, 0, b"Alice", 20);
    let (b, _b_in, mut b_ev) = member(2, 100.0, 102.0, 1, b"Bob", 20);
    let (e, _e_in, mut e_ev) = member(5, 150.0, 102.0, 1, b"Eve", 25);
    let (f, _f_in, mut f_ev) = member(6, 100.0, 400.0, 1, b"Fay", 20);
    for o in [a, b, e, f] {
        zones.enter(1, o);
    }
    join(&zones, 2, 5);
    join(&zones, 2, 6);
    for inbox in [&mut b_ev, &mut e_ev, &mut f_ev] {
        events(inbox);
    }
    received(&mut a_in);

    // Bob kills Alice: Bob and Eve (50 from him) count a kill and share by the party's three; Fay, 298 away, does not.
    strike(&zones, &defs, 2, &blows(20, 100), 1, false);
    assert!(events(&mut a_ev).contains(&ZoneEvent::Died {
        killer_kind: cp::KILLER_PLAYER,
        killer: 2,
        loss: Some(2),
        counted: true,
        keep_exp: false,
        drop_exp: 5
    }));
    for inbox in [&mut b_ev, &mut e_ev] {
        assert!(matches!(
            events(inbox)[..],
            [ZoneEvent::PlayerKill { victim: 1, victim_level: 20, last_hit: 2, party: Some((3, false, _)) }]
        ));
    }
    assert!(events(&mut f_ev).is_empty());

    // Her items go into the bags of the members near Bob in turn; her gold is split three ways among them.
    let sword = Loot::Item { item_type: 1, type_id: 1, count: 1, quality: 100, kept: None };
    let now = Instant::now();
    let mut rand = || 0;
    assert_eq!(zones.victim_item(1, 1, 2, sword, now, &mut rand), None);
    assert_eq!(zones.victim_item(1, 1, 2, sword, now, &mut rand), None);
    let given = |inbox: &mut UnboundedReceiver<ZoneEvent>| {
        events(inbox).into_iter().filter(|e| matches!(e, ZoneEvent::Loot { loot, owner: 2, .. } if *loot == sword)).count()
    };
    assert_eq!((given(&mut b_ev), given(&mut e_ev), given(&mut f_ev)), (1, 1, 0));
    assert_eq!(zones.victim_gold(1, 1, 2, 100, now, &mut rand), None);
    assert_eq!(events(&mut b_ev), [ZoneEvent::Gold { amount: 33, from_mob: false }]);
    assert_eq!(events(&mut e_ev), [ZoneEvent::Gold { amount: 33, from_mob: false }]);
    assert!(events(&mut f_ev).is_empty());

    // Without a party: on the ground at her feet ± 0.5, kind 1, Bob's.
    zones.party_leave(2);
    let seen = zones.victim_item(1, 1, 2, sword, now, &mut || 999).expect("her own 0x0401");
    assert_eq!(opcode(&seen), gp::OP_ITEM_ENTER);
    assert_eq!(payload(&seen)[4..8], [gp::KIND_PLAYER_DROP, 1, 1, 1]);
    let x = f32::from_le_bytes(payload(&seen)[8..12].try_into().unwrap());
    assert!((x - 100.499).abs() < 1e-3);
    assert!(zones.victim_gold(1, 1, 2, 50, now, &mut rand).is_some());
}

/// Bob (2, Fury) uses `skill` on `target` with numbers that never miss.
fn bob_casts(zones: &Zones, target: SkillTarget, skill: &SkillDef) -> Option<SkillOutcome> {
    let (_, _, defs) = fighting_ground();
    let fighter = Fighter { level: 20, wis: 10, ..Default::default() };
    let cast = SkillCast {
        caster: 2,
        country: 1,
        skill,
        fighter: &fighter,
        rec: 0,
        cross_road: false,
        cost_reduction: [0; 2],
        fighting: None,
        look: None,
        illusion_mob: 0,
        war_map: false,
    };
    zones.use_skill(1, target, &cast, &defs, no_loot(), Instant::now(), &mut || 0)
}

type Inboxes = [crate::outbox::OutboxInbox; 3];
type EventInboxes = [UnboundedReceiver<ZoneEvent>; 3];

/// Alice (1, Light) beside Bob (2, Fury), and Carol (3, Fury) a little further: all at 100 HP.
fn duel_ground() -> (Zones, Inboxes, EventInboxes) {
    let zones = Zones::default();
    let (a, mut a_in, a_ev) = member(1, 100.0, 100.0, 0, b"Alice", 20);
    let (b, mut b_in, b_ev) = member(2, 100.0, 102.0, 1, b"Bob", 20);
    let (c, mut c_in, c_ev) = member(3, 100.0, 104.0, 1, b"Carol", 20);
    for o in [a, b, c] {
        zones.enter(1, o);
    }
    for inbox in [&mut a_in, &mut b_in, &mut c_in] {
        received(inbox);
    }
    (zones, [a_in, b_in, c_in], [a_ev, b_ev, c_ev])
}

#[test]
fn skills_hit_enemy_players() {
    let (zones, [mut a_in, mut b_in, mut c_in], [mut a_ev, mut b_ev, _]) = duel_ground();
    // One target: 0x0511 with the damage to those within 60; a debuff stays on Alice as Bob's.
    let poison = SkillDef { keep_time: 30, ..sure_skill(skills::DEBUFF, skills::TARGET_ONE, buffs::DAMAGE_OVER_TIME, [30, 0, 0]) };
    let out = bob_casts(&zones, SkillTarget::Player(1), &poison).unwrap();
    let hit = sk::skill_use(sk::OP_USE_USER, combat::RESULT_HIT, 2, 1, 30, 1, [30, 0, 0]);
    assert_eq!(out.packets[0], hit);
    assert_eq!(opcode(out.packets.last().unwrap()), sk::OP_COSTS);
    assert_eq!(zones.health(1, 1).map(|h| h[0]), Some(70));
    let seen = received(&mut a_in);
    assert_eq!(seen[0], hit);
    assert!(seen.iter().any(|p| opcode(p) == sk::OP_APPLIED), "0x050D to Alice");
    edit(&zones, 1, |p| {
        let a = p.occupant.buffs.list.iter().find(|a| a.def.id == 30).expect("applied");
        assert_eq!(a.attacker_of(), (cp::KILLER_PLAYER, 2));
        assert_eq!(p.harm, [pvp::Harm { kind: cp::KILLER_PLAYER, id: 2, total: 30 }]);
    });
    assert_eq!(received(&mut c_in), std::slice::from_ref(&hit));
    received(&mut b_in);
    events(&mut a_ev);

    // Not on one's own side: Carol is untouched, nothing is spent, and nobody hears of it.
    let strike = sure_skill(skills::ATTACK, skills::TARGET_ONE, 21, [50, 0, 0]);
    assert!(bob_casts(&zones, SkillTarget::Player(3), &strike).is_none());
    assert_eq!(zones.health(1, 3).map(|h| h[0]), Some(100));

    // Around oneself: Alice (0x0513), not Carol.
    let around = SkillDef { apply_range: 10, ..sure_skill(skills::ATTACK, skills::TARGET_AROUND_ONESELF, 21, [20, 0, 0]) };
    let out = bob_casts(&zones, SkillTarget::Player(2), &around).unwrap();
    assert_eq!(opcodes(&out.packets), [sk::OP_USE_USER, sk::OP_USE_PARTY, sk::OP_COSTS], "the animation, then the hit on Alice");
    assert_eq!(out.packets[1], sk::skill_use(sk::OP_USE_PARTY, combat::RESULT_HIT, 2, 1, 30, 1, [20, 0, 0]));
    assert_eq!(zones.health(1, 1).map(|h| h[0]), Some(50));
    assert_eq!(zones.health(1, 3).map(|h| h[0]), Some(100));

    // A kill: Alice's 0x0504 reaches Bob before his 0x0511, the kill is his, and he stops.
    let out = bob_casts(&zones, SkillTarget::Player(1), &strike).unwrap();
    assert!(out.killed);
    assert_eq!(out.packets[..2], [cp::death(1, cp::KILLER_PLAYER, 2), sk::skill_use(sk::OP_USE_USER, 0, 2, 1, 30, 1, [50, 0, 0])]);
    assert!(events(&mut b_ev).contains(&ZoneEvent::PlayerKill { victim: 1, victim_level: 20, last_hit: 2, party: None }));
    assert!(events(&mut a_ev).iter().any(|e| matches!(e, ZoneEvent::Died { killer_kind: cp::KILLER_PLAYER, killer: 2, .. })));
    assert!(bob_casts(&zones, SkillTarget::Player(1), &strike).is_none(), "dead");
}

#[test]
fn a_drain_heals_and_a_reflection_comes_back() {
    let (zones, [mut a_in, mut b_in, _], _) = duel_ground();
    edit(&zones, 2, |p| p.occupant.vitals.health = [60, 100, 100]);
    let drain = sure_skill(skills::ATTACK, skills::TARGET_ONE, skills::DRAIN, [25, 0, 0]);
    let out = bob_casts(&zones, SkillTarget::Player(1), &drain).unwrap();
    assert_eq!(out.packets[0], sk::drain_user(combat::RESULT_HIT, 2, 1, 30, 1, [25, 0, 0], [25, 0, 0]));
    assert_eq!(zones.health(1, 2).map(|h| h[0]), Some(85));
    assert_eq!(zones.health(1, 1).map(|h| h[0]), Some(75));
    received(&mut a_in);
    received(&mut b_in);

    // Alice reflects melee: she takes nothing, Bob takes his own 40 and 0x0515 goes round; the additional damage still lands.
    let counter = SkillDef {
        id: 89,
        level: 1,
        type_effect: skills::BUFF,
        kind: buffs::MIRROR,
        keep_time: 60,
        abilities: [(0, 1), (0, 0), (0, 0)],
        success_type: 1,
        success_value: 100,
        ..Default::default()
    };
    edit(&zones, 1, |p| {
        p.occupant.buffs.apply(&counter, 1, Instant::now(), &mut || 5);
    });
    let strike = SkillDef { add_damage: [5, 0, 0], ..sure_skill(skills::ATTACK, skills::TARGET_ONE, 21, [40, 0, 0]) };
    let out = bob_casts(&zones, SkillTarget::Player(1), &strike).unwrap();
    assert_eq!(
        out.packets[..3],
        [
            sk::skill_use(sk::OP_USE_USER, combat::RESULT_HIT, 2, 1, 30, 1, [0; 3]),
            sk::add_damage_user(2, 1, 30, 1, [5, 0, 0]),
            cp::mirror_user(1, 2, [40, 0, 0]),
        ]
    );
    assert_eq!(zones.health(1, 1).map(|h| h[0]), Some(70));
    assert_eq!(zones.health(1, 2).map(|h| h[0]), Some(45));
}

#[test]
fn a_players_damage_over_time_goes_back_when_reflected() {
    let (zones, [mut a_in, mut b_in, _], _) = duel_ground();
    let poison = SkillDef {
        keep_time: 30,
        time_damage: [10, 0, 0],
        ..sure_skill(skills::DEBUFF, skills::TARGET_ONE, buffs::DAMAGE_OVER_TIME, [1, 0, 0])
    };
    bob_casts(&zones, SkillTarget::Player(1), &poison).unwrap();
    let counter = SkillDef {
        id: 89,
        level: 1,
        type_effect: skills::BUFF,
        kind: buffs::MIRROR,
        keep_time: 60,
        abilities: [(0, 1), (0, 0), (0, 0)],
        success_type: 1,
        success_value: 100,
        ..Default::default()
    };
    edit(&zones, 1, |p| {
        p.occupant.buffs.apply(&counter, 1, Instant::now(), &mut || 5);
    });
    received(&mut a_in);
    received(&mut b_in);
    let (_, _, defs) = fighting_ground();
    zones.keep_effects(Instant::now() + Duration::from_secs(3), &World::default(), &defs, no_loot(), &mut || 0);
    assert_eq!(zones.health(1, 1).map(|h| h[0]), Some(99), "Alice keeps her HP");
    assert!(zones.health(1, 2).is_some_and(|h| h[0] < 100), "Bob pays");
    let seen = received(&mut b_in);
    assert!(seen.contains(&sk::keep(sk::OP_KEEP_USER, 1, 30, 1, [0; 3])));
    assert!(seen.iter().any(|p| opcode(p) == cp::OP_MIRROR_USER));
}

/// The stakes window open between Bob (2) and Carol (3), both Fury, on map 1.
fn duel_window(zones: &Zones, inboxes: &mut Inboxes) {
    zones.duel_request(1, 2, 3, Instant::now());
    zones.duel_answer(1, 3, true);
    for inbox in inboxes.iter_mut() {
        received(inbox);
    }
}

/// Bob and Carol fighting: both ready, Bob's 100 gold and 2 of 5 potions staked, Carol's nothing; the countdown over.
fn duel_fight(zones: &Zones, inboxes: &mut Inboxes) -> Instant {
    duel_window(zones, inboxes);
    let now = Instant::now();
    let potions = w::ItemUnit { bag: 1, slot: 0, item_type: 25, type_id: 1, count: 5, ..Default::default() };
    assert!(zones.duel_stake(1, 2, shaiya_proto::trade::Offer { bag: 1, slot: 0, count: 2, trade_slot: 0 }, &potions));
    zones.duel_money(1, 2, 100);
    assert!(!zones.duel_ready(1, 2, shaiya_proto::duel::READY));
    assert!(zones.duel_ready(1, 3, shaiya_proto::duel::READY));
    let staked = crate::duel::Held { money: 100, items: vec![Loot::Item { item_type: 25, type_id: 1, count: 2, quality: 0, kept: None }] };
    assert_eq!(zones.duel_staked(1, 2, staked, now), None);
    assert_eq!(zones.duel_staked(1, 3, crate::duel::Held::default(), now), None);
    zones.duel_timers(now + crate::duel::COUNTDOWN);
    for inbox in inboxes.iter_mut() {
        received(inbox);
    }
    now
}

fn ground_items(zones: &Zones) -> Vec<GroundItem> {
    let mut items: Vec<GroundItem> = zones.ground.guard().get(&1).map(|g| g.items.values().copied().collect()).unwrap_or_default();
    items.sort_by_key(|i| i.id);
    items
}

#[test]
fn a_duel_is_asked_answered_staked_and_fought() {
    use shaiya_proto::duel as dp;
    let (zones, mut inboxes, [_, mut b_ev, mut c_ev]) = duel_ground();
    let [a_in, b_in, c_in] = &mut inboxes;
    // Not across countries (but on maps 39 and 40), not oneself.
    zones.duel_request(1, 1, 2, Instant::now());
    assert_eq!(received(a_in), [dp::answer(dp::NOT_POSSIBLE, 2)]);
    zones.duel_request(1, 2, 2, Instant::now());
    assert_eq!(received(b_in), [dp::answer(dp::NOT_POSSIBLE, 2)]);

    zones.duel_request(1, 2, 3, Instant::now());
    assert_eq!(received(b_in), [dp::request(2, 3)]);
    assert_eq!(received(c_in), [dp::request(2, 3)]);
    zones.duel_request(1, 1, 3, Instant::now());
    assert_eq!(received(a_in), [dp::answer(dp::NOT_POSSIBLE, 3)], "Carol is busy (and of the other country)");
    zones.duel_answer(1, 2, true);
    assert!(received(b_in).is_empty(), "the requester cannot answer");
    zones.duel_answer(1, 3, true);
    assert_eq!(received(b_in), [dp::answer(dp::ACCEPTED, 3), dp::window(3)]);
    assert_eq!(received(c_in), [dp::window(2)]);

    // The window: each change cancels ready first.
    let potions = w::ItemUnit { bag: 1, slot: 0, item_type: 25, type_id: 1, count: 5, ..Default::default() };
    let offer = shaiya_proto::trade::Offer { bag: 1, slot: 0, count: 2, trade_slot: 0 };
    assert!(zones.duel_stake(1, 2, offer, &potions));
    assert_eq!(received(b_in), [dp::staked(&offer)]);
    assert_eq!(received(c_in), [dp::partner_staked(0, &potions, 2)]);
    assert!(!zones.duel_ready(1, 3, dp::READY));
    assert_eq!(received(c_in), [dp::stake_ready(dp::WHO_ME, false)]);
    zones.duel_money(1, 2, 100);
    assert_eq!(
        received(b_in),
        [dp::stake_ready(dp::WHO_PARTNER, false), dp::stake_ready(dp::WHO_BOTH, true), dp::stake_money(dp::WHO_ME, 100)]
    );
    received(c_in);
    assert!(!zones.duel_ready(1, 2, dp::READY));
    assert!(zones.duel_ready(1, 3, dp::READY), "both ready: Carol's connection takes hers");
    assert!(events(&mut b_ev).contains(&ZoneEvent::DuelStakes));
    assert!(!zones.duel_ready(1, 3, dp::READY), "only once");
    assert_eq!(zones.duel_offers(1, 2).map(|(_, money)| money), Some(100));
    received(b_in);
    received(c_in);

    // Both sides' stakes out: the countdown at their midpoint; they are not enemies yet.
    let now = Instant::now();
    let staked = crate::duel::Held { money: 100, items: vec![Loot::Item { item_type: 25, type_id: 1, count: 2, quality: 0, kept: None }] };
    zones.duel_staked(1, 2, staked, now);
    assert!(received(b_in).is_empty(), "Carol's are not out yet");
    zones.duel_staked(1, 3, crate::duel::Held::default(), now);
    assert_eq!(received(b_in), [dp::ready(100.0, 103.0)]);
    assert_eq!(received(c_in), [dp::ready(100.0, 103.0)]);
    assert!(!zones.may_attack_player(1, 2, 3, false));
    zones.duel_timers(now + Duration::from_secs(4));
    assert!(received(b_in).is_empty());
    zones.duel_timers(now + crate::duel::COUNTDOWN);
    assert_eq!(received(b_in), [dp::start()]);
    assert_eq!(received(c_in), [dp::start()]);

    // Fighting: enemies though of one country, on a war map too; no party help either way.
    assert!(zones.may_attack_player(1, 2, 3, false) && zones.may_attack_player(1, 3, 2, true));
    assert!(zones.whereabouts(1, 2).is_some_and(|w| w.fighting));
    assert!(zones.copy_looks(1, 2, 3, 0, 99).is_none(), "Disguise never copies the partner");

    // Bob wins: both stakes on the ground for him, a victory and a defeat, no player-kill rules, no experience lost.
    let (_, _, defs) = fighting_ground();
    events(&mut b_ev);
    events(&mut c_ev);
    assert!(matches!(strike(&zones, &defs, 2, &blows(20, 100), 3, false), AttackOutcome::Struck { killed: true, .. }));
    let carol = events(&mut c_ev);
    assert!(carol.iter().any(|e| matches!(e, ZoneEvent::DuelResult { won: false, .. })) && carol.contains(&ZoneEvent::DuelEnded));
    assert!(carol.contains(&ZoneEvent::Died {
        killer_kind: cp::KILLER_PLAYER,
        killer: 2,
        loss: None,
        counted: false,
        keep_exp: true,
        drop_exp: 5
    }));
    let bob = events(&mut b_ev);
    assert!(bob.iter().any(|e| matches!(e, ZoneEvent::DuelResult { won: true, .. })) && bob.contains(&ZoneEvent::DuelEnded));
    assert!(!bob.iter().any(|e| matches!(e, ZoneEvent::PlayerKill { .. })));
    let seen = received(b_in);
    let (result, death) =
        (seen.iter().position(|p| *p == dp::result(dp::WON)), seen.iter().position(|p| *p == cp::death(3, cp::KILLER_PLAYER, 2)));
    assert!(result.is_some() && result < death, "the result before the death");
    assert!(received(c_in).contains(&dp::result(dp::LOST)));
    let items = ground_items(&zones);
    assert_eq!(
        items.iter().map(|i| (i.loot, i.owner, i.party)).collect::<Vec<_>>(),
        [(Loot::Gold(100), 2, 0), (Loot::Item { item_type: 25, type_id: 1, count: 2, quality: 0, kept: None }, 2, 0),]
    );
    assert!(items.iter().all(|i| distance(i.position, [100.0, 0.0, 102.0]) <= 1.0), "at Bob's feet: his own stake");
    assert!(!zones.may_attack_player(1, 2, 3, false), "the duel is over");
}

/// The ids of a player's applied skills.
fn applied_ids(zones: &Zones, id: u32) -> Vec<u16> {
    let maps = zones.maps.guard();
    maps[&1][&id].occupant.buffs.list.iter().map(|a| a.def.id).collect()
}

/// A gambling table is refused to the dead, with a personal shop, in a trade, a duel or a guild war
/// (`docs/original/mini-game.md` decision 3).
#[test]
fn a_gambling_table_is_refused_while_busy() {
    let (zones, _inboxes, _) = duel_ground();
    assert!(zones.may_gamble(1, 1) && zones.may_gamble(1, 2) && !zones.may_gamble(1, 99) && !zones.may_gamble(2, 1));
    zones.shop_begin(1, 1);
    assert!(!zones.may_gamble(1, 1), "a personal shop");
    zones.shop_end(1);
    assert!(zones.may_gamble(1, 1));
    edit(&zones, 1, |p| p.dead = true);
    assert!(!zones.may_gamble(1, 1), "dead");
    // A duel, once the window is up and when it fights.
    zones.duel_request(1, 2, 3, Instant::now());
    zones.duel_answer(1, 3, true);
    assert!(!zones.may_gamble(1, 2) && !zones.may_gamble(1, 3), "in a duel");
}

/// The kill boost looks for an applied skill by id, or by id and level, and gives its multiplier (`docs/original/operator-console.md`
/// section 4, `docs/plans/done/client-patches.md` decision 1).
#[test]
fn the_kill_boost_finds_an_applied_skill_by_id_and_level() {
    let (zones, mut inboxes, _) = duel_ground();
    duel_fight(&zones, &mut inboxes);
    let any = |id, times| KillBoost { skill: SkillMatch { id, level: None }, times };
    let at = |id, level, times| KillBoost { skill: SkillMatch { id, level: Some(level) }, times };
    let level =
        |level| SkillDef { id: 310, level, type_effect: skills::BUFF, kind: buffs::ABILITIES, keep_time: 600, ..Default::default() };
    let stones = [at(310, 1, 2), at(310, 2, 5), at(310, 3, 10)];
    assert_eq!(zones.kill_boost(1, 3, &stones), 1, "none applied");
    assert_eq!(zones.kill_boost(1, 3, &[any(310, 4), any(311, 2)]), 1);
    for (applied, times) in [(1, 2), (2, 5), (3, 10)] {
        edit(&zones, 3, |p| {
            p.occupant.buffs.apply(&level(applied), 3, Instant::now(), &mut || 1);
        });
        assert_eq!(zones.kill_boost(1, 3, &stones), times, "level {applied}");
    }
    // Entries by id only, and two matching entries give the higher.
    assert_eq!(zones.kill_boost(1, 3, &[any(310, 3)]), 3);
    assert_eq!(zones.kill_boost(1, 3, &[any(310, 3), at(310, 3, 7), at(310, 1, 50)]), 7);
    assert_eq!(zones.kill_boost(1, 3, &[any(311, 4)]), 1, "another skill");
    assert_eq!(zones.kill_boost(1, 3, &[]), 1, "none listed");
    assert_eq!(zones.kill_boost(1, 2, &stones), 1, "another player");
    assert_eq!(zones.kill_boost(2, 3, &stones), 1, "another map");
    // A monster's skill of the same id (level 100) on the player does not count for `320/1`.
    let curse = SkillDef {
        id: 320,
        level: skills::MOB_LEVEL,
        type_effect: skills::BUFF,
        kind: buffs::ABILITIES,
        keep_time: 60,
        ..Default::default()
    };
    edit(&zones, 2, |p| {
        p.occupant.buffs.apply(&curse, 3, Instant::now(), &mut || 2);
    });
    assert_eq!(zones.kill_boost(1, 2, &[any(320, 4)]), 4);
    assert_eq!(zones.kill_boost(1, 2, &[at(320, 1, 4)]), 1, "the item's level only");
}

/// `docs/original/skills-buffs.md` section 16: a death that costs no experience keeps the skills above id 200 (an elixir),
/// one by a monster does not.
#[test]
fn a_duel_death_keeps_the_item_buffs() {
    let (_, _, defs) = fighting_ground();
    let (zones, mut inboxes, _) = duel_ground();
    duel_fight(&zones, &mut inboxes);
    let buff = |id| SkillDef { id, level: 2, type_effect: skills::BUFF, kind: buffs::ABILITIES, keep_time: 600, ..Default::default() };
    edit(&zones, 3, |p| {
        p.occupant.buffs.apply(&buff(31), 3, Instant::now(), &mut || 1);
        p.occupant.buffs.apply(&buff(207), 3, Instant::now(), &mut || 2);
    });
    assert_eq!(applied_ids(&zones, 3), [31, 207]);
    assert!(matches!(strike(&zones, &defs, 2, &blows(20, 100), 3, false), AttackOutcome::Struck { killed: true, .. }));
    assert_eq!(applied_ids(&zones, 3), [207], "the class buff went, the elixir stayed");

    let (zones, mut inboxes, _) = duel_ground();
    let now = duel_fight(&zones, &mut inboxes);
    edit(&zones, 3, |p| {
        p.occupant.buffs.apply(&buff(207), 3, Instant::now(), &mut || 2);
    });
    {
        let mut maps = zones.maps.guard();
        let players = maps.get_mut(&1).unwrap();
        players.get_mut(&3).unwrap().dead = true;
        pvp::player_died(&zones, 1, &mut MobMap::default(), players, 3, (cp::KILLER_MOB, 9), &Default::default(), None, false, now);
    }
    assert!(applied_ids(&zones, 3).is_empty(), "a monster's kill costs experience: everything goes");
}

#[test]
fn duels_end_without_a_winner() {
    use shaiya_proto::duel as dp;
    let (_, _, defs) = fighting_ground();
    // Hit by someone else: each side's stakes back to itself, 0x2406 3 to both.
    let (zones, mut inboxes, _) = duel_ground();
    duel_fight(&zones, &mut inboxes);
    strike(&zones, &defs, 1, &blows(20, 1), 3, false);
    let [_, b_in, c_in] = &mut inboxes;
    assert!(received(b_in).contains(&dp::cancel(dp::CANCEL_INTERRUPTED, 3)));
    assert!(received(c_in).contains(&dp::cancel(dp::CANCEL_INTERRUPTED, 3)));
    assert!(ground_items(&zones).iter().all(|i| i.owner == 2), "Bob's own stakes back to him");
    assert_eq!(ground_items(&zones).len(), 2);

    // Out of the area: more than 40 from the centre (100, 103).
    let (zones, mut inboxes, _) = duel_ground();
    duel_fight(&zones, &mut inboxes);
    edit(&zones, 3, |p| p.occupant.position = [100.0, 0.0, 143.0]);
    zones.duel_moved(1, 3, Instant::now());
    assert!(received(&mut inboxes[2]).is_empty(), "40 is still in");
    edit(&zones, 3, |p| p.occupant.position = [100.0, 0.0, 144.0]);
    zones.duel_moved(1, 3, Instant::now());
    assert_eq!(received(&mut inboxes[1]).last(), Some(&dp::cancel(dp::CANCEL_OUT_OF_AREA, 3)));
    assert_eq!(received(&mut inboxes[2]).last(), Some(&dp::cancel(dp::CANCEL_OUT_OF_AREA, 3)));

    // Giving up: only while fighting.
    let (zones, mut inboxes, _) = duel_ground();
    duel_fight(&zones, &mut inboxes);
    zones.duel_give_up(1, 2, Instant::now());
    assert_eq!(received(&mut inboxes[2]).last(), Some(&dp::cancel(dp::CANCEL_GAVE_UP, 2)));
    assert!(zones.whereabouts(1, 3).is_some_and(|w| !w.fighting));

    // Leaving the map: the partner is told, the leaver gets its stakes back for its bags (deviation 3).
    let (zones, mut inboxes, _) = duel_ground();
    duel_fight(&zones, &mut inboxes);
    let left = zones.leave(1, 2).expect("in the zone");
    assert_eq!(left.stakes.map(|h| h.money), Some(100));
    assert_eq!(
        received(&mut inboxes[2]).iter().filter(|p| opcode(p) == dp::OP_CANCEL).collect::<Vec<_>>(),
        [&dp::cancel(dp::CANCEL_LEFT, 2)]
    );
    assert!(ground_items(&zones).is_empty());

    // A monster's death of one side: 0x2406 3 from the death.
    let (zones, mut inboxes, _) = duel_ground();
    let now = duel_fight(&zones, &mut inboxes);
    {
        let mut maps = zones.maps.guard();
        let players = maps.get_mut(&1).unwrap();
        players.get_mut(&3).unwrap().dead = true;
        pvp::player_died(&zones, 1, &mut MobMap::default(), players, 3, (cp::KILLER_MOB, 9), &Default::default(), None, false, now);
    }
    assert!(received(&mut inboxes[1]).contains(&dp::cancel(dp::CANCEL_INTERRUPTED, 3)));

    // No answer in 10 s: 0x2402 3 with the asked player's id to both.
    let (zones, mut inboxes, _) = duel_ground();
    let now = Instant::now();
    zones.duel_request(1, 2, 3, now);
    received(&mut inboxes[1]);
    received(&mut inboxes[2]);
    zones.duel_timers(now + crate::duel::REQUEST_TIME);
    assert_eq!(received(&mut inboxes[1]), [dp::answer(dp::NO_ANSWER, 3)]);
    assert_eq!(received(&mut inboxes[2]), [dp::answer(dp::NO_ANSWER, 3)]);
    zones.duel_answer(1, 3, true);
    assert!(received(&mut inboxes[1]).is_empty(), "too late");
}

#[test]
fn the_stakes_window_closes_on_trade_ends_and_steps_away() {
    use shaiya_proto::duel as dp;
    let (zones, mut inboxes, _) = duel_ground();
    duel_window(&zones, &mut inboxes);
    // Anything that ends a trade closes it.
    assert!(!zones.trade_end(3));
    assert_eq!(received(&mut inboxes[2]), [dp::window_closed(dp::WINDOW_CLOSED_BY_ME)]);
    assert_eq!(received(&mut inboxes[1]), [dp::window_closed(dp::WINDOW_CLOSED_BY_PARTNER)]);

    // The area's centre is where the two stood when the request was accepted (deviation 2): (100, 103).
    let (zones, mut inboxes, _) = duel_ground();
    duel_window(&zones, &mut inboxes);
    edit(&zones, 2, |p| p.occupant.position = [130.0, 0.0, 103.0]);
    zones.duel_moved(1, 2, Instant::now());
    assert!(received(&mut inboxes[1]).is_empty());
    edit(&zones, 2, |p| p.occupant.position = [141.0, 0.0, 103.0]);
    zones.duel_moved(1, 2, Instant::now());
    assert_eq!(received(&mut inboxes[1]), [dp::window_closed(dp::WINDOW_CLOSED_BY_ME)]);
    assert_eq!(received(&mut inboxes[2]), [dp::window_closed(dp::WINDOW_CLOSED_BY_PARTNER)]);

    // A side whose stakes came out after the duel ended gets them back on the ground at its feet.
    let (zones, mut inboxes, _) = duel_ground();
    duel_window(&zones, &mut inboxes);
    zones.duel_ready(1, 2, dp::READY);
    zones.duel_ready(1, 3, dp::READY);
    zones.duel_staked(1, 2, crate::duel::Held { money: 5, items: Vec::new() }, Instant::now());
    // Carol's connection is still taking hers when a monster hits her: Bob's are out, his client was told 0x2408 0 already.
    {
        let mut maps = zones.maps.guard();
        duel::hit(&zones, 1, maps.get_mut(&1).unwrap(), 3, None, Instant::now());
    }
    assert_eq!(received(&mut inboxes[1]).last(), Some(&dp::cancel(dp::CANCEL_INTERRUPTED, 3)));
    assert_eq!(received(&mut inboxes[2]).last(), Some(&dp::window_closed(dp::WINDOW_CLOSED_BY_ME)));
    assert_eq!(ground_items(&zones).iter().map(|i| (i.loot, i.owner)).collect::<Vec<_>>(), [(Loot::Gold(5), 2)]);
    assert_eq!(zones.duel_staked(1, 3, crate::duel::Held { money: 7, items: Vec::new() }, Instant::now()), None);
    assert_eq!(ground_items(&zones).len(), 2, "Carol's too, at her feet");
}

#[test]
fn a_trade_and_a_duel_stakes_window_exclude_each_other() {
    use shaiya_proto::duel as dp;
    use shaiya_proto::trade as tp;
    let now = Instant::now();
    // The accomplice case of audit R3: Bob (2) stakes against Carol (3); Alice (1) cannot trade with Bob meanwhile, nor Bob with her.
    let (zones, mut inboxes, _) = duel_ground();
    put_rules(&zones, &[("any", serde_json::json!({ "type": "cross_faction_trade" }))]);
    duel_window(&zones, &mut inboxes);
    zones.trade_request(1, 1, 2, now);
    assert_eq!(received(&mut inboxes[0]), [tp::start(2, tp::START_BUSY)]);
    zones.trade_request(1, 2, 1, now);
    assert_eq!(received(&mut inboxes[1]), [tp::start(1, tp::START_BUSY)]);
    assert!(received(&mut inboxes[2]).is_empty());
    assert!(!zones.trades.guard().busy(1) && !zones.trades.guard().busy(2));

    // The other way round: Bob trades with Alice, so neither Carol nor Alice can duel him, and Bob cannot ask Carol.
    let (zones, mut inboxes, _) = duel_ground();
    put_rules(&zones, &[("any", serde_json::json!({ "type": "cross_faction_trade" }))]);
    zones.trade_request(1, 1, 2, now);
    zones.trade_answer(2, true);
    for inbox in inboxes.iter_mut() {
        received(inbox);
    }
    zones.duel_request(1, 3, 2, now);
    assert_eq!(received(&mut inboxes[2]), [dp::answer(dp::NOT_POSSIBLE, 2)]);
    zones.duel_request(1, 2, 3, now);
    assert_eq!(received(&mut inboxes[1]), [dp::answer(dp::NOT_POSSIBLE, 3)]);
    assert!(received(&mut inboxes[0]).is_empty());
    assert!(zones.duel_partner(1, 2).is_none() && zones.duel_partner(1, 3).is_none());

    // A request waiting for its answer is cancelled when the other kind opened in between.
    let (zones, mut inboxes, _) = duel_ground();
    put_rules(&zones, &[("any", serde_json::json!({ "type": "cross_faction_trade" }))]);
    zones.duel_request(1, 2, 3, now);
    zones.trade_request(1, 1, 2, now);
    zones.trade_answer(2, true);
    for inbox in inboxes.iter_mut() {
        received(inbox);
    }
    zones.duel_answer(1, 3, true);
    assert_eq!(received(&mut inboxes[1]), [dp::answer(dp::NOT_POSSIBLE, 3)], "Bob trades now: no duel");
    assert!(zones.duel_partner(1, 2).is_none());
}

#[test]
fn a_personal_shop_excludes_a_duel() {
    use shaiya_proto::duel as dp;
    let (zones, mut inboxes, _) = duel_ground();
    zones.shop_begin(1, 3);
    received(&mut inboxes[2]);
    zones.duel_request(1, 2, 3, Instant::now());
    assert_eq!(received(&mut inboxes[1]), [dp::answer(dp::NOT_POSSIBLE, 3)]);
    zones.duel_request(1, 3, 2, Instant::now());
    assert_eq!(received(&mut inboxes[2]), [dp::answer(dp::NOT_POSSIBLE, 2)]);
    // A shop begun after the request: the answer is refused.
    let (zones, mut inboxes, _) = duel_ground();
    zones.duel_request(1, 2, 3, Instant::now());
    zones.shop_begin(1, 2);
    for inbox in inboxes.iter_mut() {
        received(inbox);
    }
    zones.duel_answer(1, 3, true);
    assert_eq!(received(&mut inboxes[1]), [dp::answer(dp::NOT_POSSIBLE, 3)]);
    assert!(zones.duel_partner(1, 2).is_none());
    // Opening a shop closes a stakes window (every trade end does).
    let (zones, mut inboxes, _) = duel_ground();
    duel_window(&zones, &mut inboxes);
    assert!(!zones.trade_end(2));
    assert_eq!(received(&mut inboxes[1]), [dp::window_closed(dp::WINDOW_CLOSED_BY_ME)]);
}

#[test]
fn stakes_that_cannot_be_taken_cancel_the_duel_for_both() {
    use shaiya_proto::duel as dp;
    let (zones, mut inboxes, _) = duel_ground();
    duel_window(&zones, &mut inboxes);
    let now = Instant::now();
    zones.duel_ready(1, 2, dp::READY);
    assert!(zones.duel_ready(1, 3, dp::READY));
    for inbox in inboxes.iter_mut() {
        received(inbox);
    }
    // Bob's stakes came out; Carol's connection finds hers gone: the window closes on both screens, Bob's go to his feet.
    assert_eq!(zones.duel_staked(1, 2, crate::duel::Held { money: 5, items: Vec::new() }, now), None);
    zones.duel_stakes_failed(1, 3, now);
    assert_eq!(received(&mut inboxes[1]).last(), Some(&dp::cancel(dp::CANCEL_INTERRUPTED, 3)));
    assert_eq!(received(&mut inboxes[2]).last(), Some(&dp::window_closed(dp::WINDOW_CLOSED_BY_ME)));
    assert_eq!(ground_items(&zones).iter().map(|i| (i.loot, i.owner)).collect::<Vec<_>>(), [(Loot::Gold(5), 2)]);
    assert!(zones.duel_partner(1, 2).is_none() && zones.duel_partner(1, 3).is_none(), "the duel is over");
    // Nothing is staked and nothing is lost when neither side had taken: the windows close.
    let (zones, mut inboxes, _) = duel_ground();
    duel_window(&zones, &mut inboxes);
    zones.duel_ready(1, 2, dp::READY);
    assert!(zones.duel_ready(1, 3, dp::READY));
    for inbox in inboxes.iter_mut() {
        received(inbox);
    }
    zones.duel_stakes_failed(1, 2, now);
    assert_eq!(received(&mut inboxes[1]), [dp::window_closed(dp::WINDOW_CLOSED_BY_ME)]);
    assert_eq!(received(&mut inboxes[2]), [dp::window_closed(dp::WINDOW_CLOSED_BY_PARTNER)]);
    assert!(ground_items(&zones).is_empty());
}

#[test]
fn a_death_is_the_partners_win_by_the_record_or_the_flag() {
    let (zones, _, _) = duel_ground();
    let mut maps = zones.maps.guard();
    let players = maps.get_mut(&1).unwrap();
    let fight = |players: &mut HashMap<u32, Placed>, a: u32, b: u32| {
        let (side, _) = crate::duel::Side::request(a, b, Instant::now());
        players.get_mut(&a).unwrap().duel = Some(crate::duel::Side { stage: crate::duel::Stage::Fighting, partner: b, ..side });
    };
    fight(players, 2, 3);
    fight(players, 3, 2);
    let p = cp::KILLER_PLAYER;
    assert_eq!(duel::winner(players, 3, (p, 2), false), Some(2), "killed by the partner");
    assert_eq!(duel::winner(players, 3, (p, 1), false), None, "someone else");
    assert_eq!(duel::winner(players, 3, (cp::KILLER_MOB, 2), false), None);
    assert_eq!(duel::winner(players, 3, (p, 3), false), None, "itself, no flag");
    assert_eq!(duel::winner(players, 3, (p, 3), true), Some(2), "the desperate counterattack: its partner wins");
    assert_eq!(duel::winner(players, 3, (p, 2), true), Some(2));
    players.get_mut(&2).unwrap().duel = None;
    assert_eq!(duel::winner(players, 3, (p, 2), true), None, "with the flag both must still fight");
    assert_eq!(duel::winner(players, 3, (p, 2), false), Some(2), "without it the victim's side decides (`PvPGetDeathType`)");
}

fn shop_item(count: u8) -> crate::backend::ItemUnit {
    crate::backend::ItemUnit { bag: 1, slot: 2, item_type: 100, type_id: 1, quality: 0, gems: [0; 6], count, craft_name: Vec::new() }
}

/// Bob (2) sets up a shop with one item at 10 a piece and opens it as "Bob's"; every inbox emptied.
fn bobs_shop(zones: &Zones, inboxes: &mut Inboxes) {
    use shaiya_proto::my_shop as mp;
    zones.shop_begin(1, 2);
    assert!(zones.shop_place(1, 2, mp::Add { bag: 1, slot: 2, shop_slot: 0, price: 10 }, shop_item(5)));
    zones.shop_open(1, 2, 6, b"Bob's".to_vec());
    for inbox in inboxes.iter_mut() {
        received(inbox);
    }
}

#[test]
fn a_personal_shop_is_set_up_opened_visited_and_closed() {
    use shaiya_proto::my_shop as mp;
    let (zones, mut inboxes, _) = duel_ground();
    zones.shop_begin(1, 2);
    let add = mp::Add { bag: 1, slot: 2, shop_slot: 0, price: 10 };
    assert!(zones.shop_place(1, 2, add, shop_item(5)));
    assert!(!zones.shop_place(1, 2, mp::Add { shop_slot: 1, ..add }, shop_item(5)), "the same place twice");
    assert!(!zones.shop_place(1, 3, add, shop_item(5)), "Carol has no shop");
    zones.shop_open(1, 2, 6, b"Bob's".to_vec());
    let sign = mp::sign(2, 6, b"Bob's");
    assert_eq!(received(&mut inboxes[1]), [mp::begin(), mp::added(&add), mp::started(), sign.clone()]);
    assert_eq!(received(&mut inboxes[0]), std::slice::from_ref(&sign), "the open sign goes to both countries in view");
    assert_eq!(received(&mut inboxes[2]), std::slice::from_ref(&sign));

    // The sign with the looks: to Bob's country only off map 42.
    assert_eq!(zones.shape(1, 3, 2).last(), Some(&sign));
    assert_eq!(zones.shape(1, 1, 2).len(), 2, "Alice is of the other country");

    // Visits: Alice (the other country), Bob himself (deviation 5) get nothing; Carol the window and the items.
    zones.shop_visit(1, 1, 2);
    zones.shop_visit(1, 2, 2);
    assert!(received(&mut inboxes[0]).is_empty() && received(&mut inboxes[1]).is_empty());
    zones.shop_visit(1, 3, 2);
    assert_eq!(received(&mut inboxes[2]), [mp::visiting(2), mp::list(&[(0, 10, &shop_item(5))])]);
    assert_eq!(zones.shop_listing(1, 3, 0).map(|(s, l)| (s, l.map(|l| l.price))), Some((2, Some(10))));
    assert_eq!(zones.shop_listing(1, 3, 1).map(|(s, l)| (s, l.is_none())), Some((2, true)), "visiting, an empty slot");
    assert_eq!(zones.shop_listing(1, 1, 0), None, "Alice visits nothing");

    // Closing: back to setting up, the sign down, the visitor told.
    zones.shop_close(1, 2);
    assert_eq!(received(&mut inboxes[1]), [mp::stopped(), mp::sign_down(2)]);
    assert_eq!(received(&mut inboxes[2]), [mp::sign_down(2), mp::closed_to_visitor()]);
    assert_eq!(zones.shop_listing(1, 3, 0), None, "the visit ended");
    assert!(zones.has_shop(1, 2));

    // Anything that ends a trade ends it (`stMyShop::Ended`): 0x2302 and the sign down.
    zones.cancel_actions(2);
    assert_eq!(received(&mut inboxes[1]), [mp::end(), mp::sign_down(2)]);
    assert!(!zones.has_shop(1, 2));
}

#[test]
fn beginning_again_or_ending_an_open_shop_takes_the_sign_down() {
    use shaiya_proto::my_shop as mp;
    let (zones, mut inboxes, _) = duel_ground();
    bobs_shop(&zones, &mut inboxes);
    zones.shop_visit(1, 3, 2);
    received(&mut inboxes[2]);
    zones.shop_begin(1, 2);
    assert_eq!(received(&mut inboxes[1]), [mp::sign_down(2), mp::begin()]);
    assert_eq!(received(&mut inboxes[2]), [mp::closed_to_visitor(), mp::sign_down(2)]);
    assert_eq!(zones.shop_slot(1, 2, 0), None, "a fresh setup");

    bobs_shop(&zones, &mut inboxes);
    zones.shop_finish(1, 2);
    assert_eq!(received(&mut inboxes[1]), [mp::end(), mp::sign_down(2)]);
    zones.shop_begin(1, 2);
    received(&mut inboxes[1]);
    zones.shop_finish(1, 2);
    assert_eq!(received(&mut inboxes[1]), [mp::end()], "never opened: no sign");
}

#[test]
fn leaving_the_map_takes_the_visitor_off_the_sellers_list() {
    use shaiya_proto::my_shop as mp;
    let (zones, mut inboxes, _) = duel_ground();
    bobs_shop(&zones, &mut inboxes);
    zones.shop_visit(1, 3, 2);
    received(&mut inboxes[2]);
    zones.leave(1, 3);
    edit(&zones, 2, |p| assert!(p.shop.as_ref().unwrap().visitors.is_empty(), "deviation 4"));
    // 0x230A with the shop gone still answers (deviation 4).
    let (zones, mut inboxes, _) = duel_ground();
    bobs_shop(&zones, &mut inboxes);
    zones.shop_visit(1, 3, 2);
    received(&mut inboxes[2]);
    zones.leave(1, 2);
    assert_eq!(received(&mut inboxes[2]), [mp::closed_to_visitor(), mp::sign_down(2), w::user_leave(2)]);
    zones.shop_leave(1, 3);
    assert!(received(&mut inboxes[2]).is_empty(), "the seller's leaving ended the visit already");
}

#[test]
fn purchases_wait_for_the_seller_in_order() {
    use crate::my_shop::{self as ms, Purchase};
    use shaiya_proto::my_shop as mp;
    let (zones, mut inboxes, [_, mut b_ev, _]) = duel_ground();
    let purchase = |buyer: u32| {
        let (reply, rx) = tokio::sync::oneshot::channel();
        let seen = ms::Listed { bag: 1, slot: 2, price: 10, item: shop_item(5) };
        (Purchase { buyer, buyer_money: 100, shop_slot: 0, count: 2, seen, total: 20, to: (1, 0), reply }, rx)
    };
    let (p, _rx) = purchase(3);
    assert!(zones.shop_queue(1, 2, p).is_err(), "no shop open");
    bobs_shop(&zones, &mut inboxes);
    zones.shop_visit(1, 3, 2);
    received(&mut inboxes[2]);
    let (p, _rx3) = purchase(3);
    assert!(zones.shop_queue(1, 2, p).is_ok());
    let (p, mut rx1) = purchase(1);
    assert!(zones.shop_queue(1, 2, p).is_ok());
    assert_eq!(std::iter::from_fn(|| b_ev.try_recv().ok()).collect::<Vec<_>>(), [ZoneEvent::Sale, ZoneEvent::Sale]);
    assert!(zones.shop_withdraw(2, 1), "Alice's wait ran out");
    assert!(!zones.shop_withdraw(2, 1));
    assert!(matches!(rx1.try_recv(), Err(tokio::sync::oneshot::error::TryRecvError::Closed)));
    let next = zones.shop_next_sale(2).expect("Carol's");
    assert_eq!(next.buyer, 3);
    assert!(zones.shop_next_sale(2).is_none());

    // Sold: 0x230D to the buyer, the count to every visitor, the slot keeps what is left.
    let bought = mp::bought(80, 0, 3, (1, 0), &shop_item(5), 2);
    zones.shop_sold(1, 2, 0, 3, 3, &bought);
    assert_eq!(received(&mut inboxes[2]), [bought, mp::slot_count(0, 3)]);
    assert_eq!(zones.shop_slot(1, 2, 0).map(|l| l.item.count), Some(3));

    // A shop that ends drops what waits: the buyer's reply is gone.
    let (p, mut rx) = purchase(3);
    zones.shop_queue(1, 2, p).unwrap();
    zones.shop_close(1, 2);
    assert!(matches!(rx.try_recv(), Err(tokio::sync::oneshot::error::TryRecvError::Closed)));
}

#[test]
fn a_player_with_a_shop_is_busy_for_parties() {
    let (zones, mut inboxes, _) = duel_ground();
    bobs_shop(&zones, &mut inboxes);
    zones.party_invite(1, 2, Invitee::Id(3), false, Instant::now());
    assert_eq!(received(&mut inboxes[1]), [shaiya_proto::party::failed(shaiya_proto::party::CANNOT_INVITE, 2)], "the inviter busy");
    zones.party_invite(1, 3, Invitee::Id(2), false, Instant::now());
    assert_eq!(received(&mut inboxes[2]), [shaiya_proto::party::failed(shaiya_proto::party::CANNOT_INVITE, 2)], "the target busy");
    assert!(received(&mut inboxes[1]).is_empty());
}

type WarInboxes = [crate::outbox::OutboxInbox; 6];

/// Guild 10 (Alice 1 its master, Ann 2, Abe 5 far away) and guild 20 (Bob 3 of rank 3, Ben 4), all Fury on map 1, and Cid 6
/// without a guild. Alice and Bob stand 30 apart: the centre would be at (100, 115).
fn war_ground() -> (Zones, WarInboxes, [UnboundedReceiver<ZoneEvent>; 6]) {
    use crate::backend::{StoredGuild, StoredMember};
    let zones = Zones::default();
    let m = |id, rank| StoredMember { id, name: format!("p{id}"), level: 20, job: 0, rank };
    let guild = |id, name: &str, members| StoredGuild {
        id,
        name: name.into(),
        country: 1,
        remark: String::new(),
        members,
        house: Default::default(),
        bank: Vec::new(),
    };
    let stored = vec![guild(10, "Red", vec![m(1, 1), m(2, 9), m(5, 9)]), guild(20, "Blue", vec![m(3, 3), m(4, 9)])];
    zones.load_guilds(crate::guild::Guilds::load(stored, unbounded_channel().0));
    let spots = [(1, 100.0, 100.0), (2, 100.0, 110.0), (3, 100.0, 130.0), (4, 100.0, 150.0), (5, 300.0, 300.0), (6, 100.0, 105.0)];
    let (mut inboxes, mut events) = (Vec::new(), Vec::new());
    for (id, x, z) in spots {
        let (o, inbox, ev) = member(id, x, z, 1, format!("p{id}").as_bytes(), 20);
        let online = crate::guild::Player {
            outbox: o.outbox.clone(),
            events: o.events.clone(),
            name: o.shape.name.clone(),
            country: 1,
            job: 0,
            level: 20,
            grow: 1,
            penalty_until: None,
        };
        zones.enter(1, o);
        zones.guild_connect(id, online);
        inboxes.push(inbox);
        events.push(ev);
    }
    for inbox in &mut inboxes {
        received(inbox);
    }
    (zones, inboxes.try_into().unwrap(), events.try_into().unwrap())
}

/// The guild war packets waiting in an outbox.
fn war_packets(inbox: &mut crate::outbox::OutboxInbox) -> Vec<Vec<u8>> {
    received(inbox).into_iter().filter(|p| opcode(p) >> 8 == 0x25).collect()
}

fn at_war(zones: &Zones, id: u32) -> Option<crate::guild_war::Part> {
    zones.maps.guard()[&1][&id].war
}

fn enemies(zones: &Zones, a: u32, b: u32) -> bool {
    zones.may_attack_player(1, a, b, false)
}

/// Alice asks Bob's guild and Bob accepts: the countdown runs.
fn war_countdown(zones: &Zones, inboxes: &mut WarInboxes) -> Instant {
    let now = Instant::now();
    zones.war_request(1, 1, 3, now);
    zones.war_answer(1, 3, true, now);
    for inbox in inboxes.iter_mut() {
        received(inbox);
    }
    now
}

#[test]
fn a_guild_war_is_asked_accepted_and_fought_in_the_area() {
    use shaiya_proto::guild_war as wp;
    let (zones, mut inboxes, _) = war_ground();
    let now = Instant::now();
    zones.war_request(1, 2, 3, now);
    assert!(received(&mut inboxes[1]).is_empty(), "rank 9 is ignored");
    zones.war_request(1, 1, 2, now);
    assert_eq!(received(&mut inboxes[0]), [wp::answer(wp::NOT_POSSIBLE, 2)], "one's own guild");
    zones.war_request(1, 1, 6, now);
    assert_eq!(received(&mut inboxes[0]), [wp::answer(wp::NOT_POSSIBLE, 6)], "no guild");
    zones.war_request(1, 1, 3, now);
    assert_eq!(received(&mut inboxes[0]), [wp::request(1, 3)]);
    assert_eq!(received(&mut inboxes[2]), [wp::request(1, 3)]);
    assert!(at_war(&zones, 1).is_some_and(|w| w.enemy == 20 && !w.ready));
    assert!(!enemies(&zones, 1, 3), "not before the answer");
    zones.duel_request(1, 1, 2, now);
    assert!(received(&mut inboxes[0]).is_empty(), "no duel for a player in a guild war");
    zones.war_answer(1, 4, true, now);
    assert!(received(&mut inboxes[0]).is_empty(), "only the asked player answers (deviation 2)");

    zones.war_answer(1, 3, true, now);
    assert_eq!(received(&mut inboxes[0]), [wp::answer(wp::ACCEPTED, 3), wp::ready(100.0, 115.0, 20)]);
    assert_eq!(received(&mut inboxes[1]), [wp::ready(100.0, 115.0, 20)]);
    assert_eq!(received(&mut inboxes[2]), [wp::ready(100.0, 115.0, 10)]);
    assert_eq!(received(&mut inboxes[3]), [wp::ready(100.0, 115.0, 10)], "35 from the centre");
    assert!(received(&mut inboxes[4]).is_empty() && at_war(&zones, 5).is_none(), "Abe is too far to take part");
    assert!(received(&mut inboxes[5]).is_empty());
    // Enemies from the countdown on, though all are Fury; nobody else is.
    assert!(enemies(&zones, 1, 3) && enemies(&zones, 4, 2));
    assert!(!enemies(&zones, 1, 2) && !enemies(&zones, 5, 3) && !enemies(&zones, 6, 3));
    let w = zones.whereabouts(1, 2).unwrap();
    assert_eq!((w.war, zones.whereabouts(1, 6).unwrap().war), (20, 0));

    zones.war_timers(now + Duration::from_secs(4));
    assert!(received(&mut inboxes[0]).is_empty());
    zones.war_timers(now + crate::guild_war::COUNTDOWN);
    for inbox in &mut inboxes[..4] {
        assert_eq!(received(inbox), [wp::start()]);
    }
    assert!(received(&mut inboxes[4]).is_empty());
}

#[test]
fn a_guild_war_ends_when_a_side_runs_out() {
    use shaiya_proto::guild_war as wp;
    let (zones, mut inboxes, mut evs) = war_ground();
    let now = war_countdown(&zones, &mut inboxes);
    zones.war_timers(now + crate::guild_war::COUNTDOWN);
    for inbox in inboxes.iter_mut() {
        received(inbox);
    }
    // Ben steps out of the area: everyone in the war hears it, Ben too.
    edit(&zones, 4, |p| p.occupant.position = [100.0, 0.0, 156.0]);
    zones.war_moved(1, 4);
    for inbox in &mut inboxes[..4] {
        assert_eq!(received(inbox), [wp::left(wp::OUT_OF_AREA, 4)]);
    }
    assert!(at_war(&zones, 4).is_none() && !enemies(&zones, 4, 1));
    // Bob kills Ann: a defeat, nothing lost, no player-kill rules.
    events(&mut evs[1]);
    {
        let mut maps = zones.maps.guard();
        let players = maps.get_mut(&1).unwrap();
        players.get_mut(&2).unwrap().harm.push(pvp::Harm { kind: cp::KILLER_PLAYER, id: 3, total: 100 });
        pvp::player_died(&zones, 1, &mut MobMap::default(), players, 2, (cp::KILLER_PLAYER, 3), &Default::default(), None, false, now);
    }
    assert!(events(&mut evs[1]).contains(&ZoneEvent::Died {
        killer_kind: cp::KILLER_PLAYER,
        killer: 3,
        loss: None,
        counted: false,
        keep_exp: true,
        drop_exp: 5
    }));
    assert!(received(&mut inboxes[0]).contains(&wp::left(wp::DEFEATED, 2)));
    assert!(received(&mut inboxes[1]).contains(&wp::left(wp::DEFEATED, 2)));
    assert!(war_packets(&mut inboxes[3]).is_empty(), "Ben is out of it");
    received(&mut inboxes[2]);
    // Bob, Blue's officer, leaves the map: Blue loses at once, though he is not its last player.
    zones.leave(1, 3);
    assert_eq!(war_packets(&mut inboxes[0]), [wp::left(wp::OUT_OF_AREA, 3), wp::result(wp::WON)]);
    assert_eq!(war_packets(&mut inboxes[1]), [wp::result(wp::WON)], "Ann took part");
    assert_eq!(war_packets(&mut inboxes[2]), [wp::left(wp::OUT_OF_AREA, 3), wp::result(wp::LOST)]);
    assert_eq!(war_packets(&mut inboxes[3]), [wp::result(wp::LOST)], "Ben took part");
    assert!(war_packets(&mut inboxes[4]).is_empty());
    assert!(at_war(&zones, 1).is_none() && zones.wars.guard().get(10).is_none());
    // Peace: Alice may ask again.
    zones.war_request(1, 1, 4, now);
    assert_eq!(received(&mut inboxes[0]), [wp::answer(wp::NOT_POSSIBLE, 4)], "Ben is rank 9");
}

#[test]
fn the_last_player_out_loses_and_a_monster_death_counts() {
    use shaiya_proto::guild_war as wp;
    let (zones, mut inboxes, _) = war_ground();
    let now = war_countdown(&zones, &mut inboxes);
    // Ben dies to a monster during the countdown: defeated.
    {
        let mut maps = zones.maps.guard();
        let players = maps.get_mut(&1).unwrap();
        pvp::player_died(&zones, 1, &mut MobMap::default(), players, 4, (cp::KILLER_MOB, 9), &Default::default(), None, false, now);
    }
    assert_eq!(war_packets(&mut inboxes[2]), [wp::left(wp::DEFEATED, 4)]);
    // Bob logs out: the last of Blue.
    zones.war_left_world(1, 3);
    assert_eq!(war_packets(&mut inboxes[0]), [wp::left(wp::DEFEATED, 4), wp::left(wp::LEFT_WORLD, 3), wp::result(wp::WON)]);
    assert_eq!(war_packets(&mut inboxes[2]), [wp::left(wp::LEFT_WORLD, 3), wp::result(wp::LOST)]);
    zones.war_timers(now + crate::guild_war::COUNTDOWN);
    assert!(received(&mut inboxes[0]).is_empty(), "no fight after the end");
}

#[test]
fn guild_war_requests_lapse_are_refused_or_too_far_apart() {
    use shaiya_proto::guild_war as wp;
    let (zones, mut inboxes, _) = war_ground();
    let now = Instant::now();
    zones.war_request(1, 1, 3, now);
    received(&mut inboxes[0]);
    received(&mut inboxes[2]);
    zones.war_timers(now + crate::guild_war::REQUEST_TIME);
    assert_eq!(received(&mut inboxes[0]), [wp::answer(wp::NO_ANSWER, 3)]);
    assert_eq!(received(&mut inboxes[2]), [wp::answer(wp::NO_ANSWER, 3)]);
    assert!(at_war(&zones, 1).is_none() && at_war(&zones, 3).is_none());

    zones.war_request(1, 1, 3, now);
    zones.war_answer(1, 3, false, now);
    assert_eq!(received(&mut inboxes[0]), [wp::request(1, 3), wp::answer(wp::REFUSED, 3)]);
    assert_eq!(received(&mut inboxes[2]), [wp::request(1, 3)]);

    edit(&zones, 3, |p| p.occupant.position = [100.0, 0.0, 180.0]);
    zones.war_request(1, 1, 3, now);
    zones.war_answer(1, 3, true, now);
    assert_eq!(received(&mut inboxes[0]), [wp::request(1, 3), wp::answer(wp::NOT_POSSIBLE, 3)], "80 apart (deviation 1)");
    assert_eq!(received(&mut inboxes[2]), [wp::request(1, 3), wp::answer(wp::NO_ANSWER, 3)]);
    assert!(zones.wars.guard().get(10).is_none());

    // The officer of a request leaves the map: the request ends.
    zones.war_request(1, 1, 3, now);
    received(&mut inboxes[2]);
    zones.leave(1, 1);
    assert_eq!(war_packets(&mut inboxes[2]), [wp::answer(wp::NO_ANSWER, 3)]);
    assert!(zones.wars.guard().get(20).is_none());
}

#[test]
fn leaving_the_guild_leaves_the_war_as_a_member() {
    use shaiya_proto::guild_war as wp;
    let (zones, mut inboxes, _) = war_ground();
    war_countdown(&zones, &mut inboxes);
    // Alice kicks Ann: Ann is told as any player leaving the war (deviation 6), and the war goes on.
    zones.guild_kick(1, 2);
    assert_eq!(war_packets(&mut inboxes[1]), [wp::left(wp::OUT_OF_AREA, 2)]);
    assert_eq!(war_packets(&mut inboxes[2]), [wp::left(wp::OUT_OF_AREA, 2)]);
    assert!(at_war(&zones, 2).is_none() && at_war(&zones, 1).is_some());
    assert_eq!(war_packets(&mut inboxes[3]), [wp::left(wp::OUT_OF_AREA, 2)]);
    // Ben leaves his guild: out of the war, Bob fights on.
    zones.guild_leave(4);
    assert_eq!(war_packets(&mut inboxes[3]), [wp::left(wp::OUT_OF_AREA, 4)]);
    assert_eq!(war_packets(&mut inboxes[2]), [wp::left(wp::OUT_OF_AREA, 4)]);
    assert!(zones.wars.guard().get(20).is_some_and(|w| w.fighters == [3]));
    // Bob, Blue's officer and its last player, leaves too: Blue loses.
    zones.guild_leave(3);
    assert_eq!(war_packets(&mut inboxes[2]), [wp::left(wp::OUT_OF_AREA, 3), wp::result(wp::LOST)]);
    assert_eq!(
        war_packets(&mut inboxes[0]),
        [wp::left(wp::OUT_OF_AREA, 2), wp::left(wp::OUT_OF_AREA, 4), wp::left(wp::OUT_OF_AREA, 3), wp::result(wp::WON)]
    );
    assert!(zones.wars.guard().get(10).is_none() && at_war(&zones, 1).is_none());
}
/// Level-100 skills that always land: 205 a debuff, 7 an attack of 100 HP as it is.
fn lapis_skills(zones: &Zones) {
    let def = |id, type_effect, damage| SkillDef {
        id,
        level: skills::MOB_LEVEL,
        type_effect,
        target_type: skills::TARGET_ONE,
        kind: if type_effect == skills::ATTACK { 21 } else { 62 },
        damage,
        success_type: 1,
        success_value: 100,
        keep_time: 10,
        ..Default::default()
    };
    zones.load_item_skills(&skills::SkillDefs::new([def(205, skills::DEBUFF, [0; 3]), def(7, skills::ATTACK, [100, 0, 0])]));
}

fn with_options(o: Occupant, weapon: [u16; 6], upper: u16) -> Occupant {
    let mut options = OptionSkills { weapon, ..Default::default() };
    options.armour[12] = upper;
    Occupant { vitals: Vitals { options, ..o.vitals }, ..o }
}

#[test]
fn weapon_lapis_strike_the_monster_hit_and_armour_lapis_the_monster_hitting() {
    use crate::combat::Gear;
    let (world, _, defs) = fighting_ground();
    let zones = Zones::new(&world, &defs);
    lapis_skills(&zones);
    let (a, mut a_in) = player(1, 100.0, 102.0);
    // An unknown skill (99) and empty gems are passed over.
    zones.enter(1, with_options(a, [205, 0, 99, 7, 0, 0], 205));
    let now = Instant::now();
    zones.respawn(now, &world, &defs, &mut || 0);
    drain(&mut a_in);

    // A normal blow that hits: 0x0503, then each weapon lapis in gem order, level 100, the caster's copies among its packets.
    let fighter = combat::player(10, [10; 6], &Gear { kind: Some(combat::MELEE), ..Default::default() });
    let attack =
        Attack { attacker: 1, country: 0, fighter: &fighter, kind: combat::MELEE, slack: 2.0, range: 0, rec: 0, cross_road: false };
    let mut rolls = [0, 99].into_iter().cycle();
    let AttackOutcome::Struck { packets, killed: false, .. } =
        zones.attack_mob(1, 1, &attack, &defs, no_loot(), now, &mut || rolls.next().unwrap())
    else {
        panic!("a hit")
    };
    assert_eq!(opcodes(&packets), [0x0503, sk::OP_USE_MOB, sk::OP_USE_MOB]);
    assert_eq!(packets[1], sk::skill_use(sk::OP_USE_MOB, combat::RESULT_HIT, 1, 1, 205, skills::MOB_LEVEL, [0; 3]));
    assert_eq!(packets[2], sk::skill_use(sk::OP_USE_MOB, combat::RESULT_HIT, 1, 1, 7, skills::MOB_LEVEL, [100, 0, 0]));
    let blow = u32::from(u16::from_le_bytes(payload(&packets[0])[9..11].try_into().unwrap()));
    assert_eq!(zones.mob(1, 1).map(|m| m.hp), Some(1000 - blow - 100));
    assert!(received(&mut a_in).is_empty(), "nothing through the outbox");

    // The monster hits back: 0x0605, then the upper armour's lapis at it, through the player's outbox.
    let mut rolls = [0, 99].into_iter().cycle();
    zones.mob_fight(now, &world, &defs, no_loot(), &mut || rolls.next().unwrap());
    let got = received(&mut a_in);
    assert_eq!(opcodes(&got), [0x0605, sk::OP_USE_MOB]);
    assert_eq!(got[1], sk::skill_use(sk::OP_USE_MOB, combat::RESULT_HIT, 1, 1, 205, skills::MOB_LEVEL, [0; 3]));

    // A miss sets nothing off.
    let mut rolls = [99].into_iter().cycle();
    zones.attack_mob(1, 1, &attack, &defs, no_loot(), now, &mut || rolls.next().unwrap());
    assert!(received(&mut a_in).is_empty());
}

#[test]
fn a_skill_hit_sets_off_the_weapon_lapis_before_the_costs() {
    let (zones, defs, _, _) = skill_ground(1);
    lapis_skills(&zones);
    edit(&zones, 1, |p| p.occupant.vitals.options.weapon = [7, 0, 0, 0, 0, 0]);
    let one = sure_skill(skills::ATTACK, skills::TARGET_ONE, 21, [300, 0, 0]);
    let out = use_on(&zones, &defs, SkillTarget::Mob(1), &one).unwrap();
    assert_eq!(
        out.packets,
        [
            sk::skill_use(sk::OP_USE_MOB, 0, 1, 1, 30, 1, [300, 0, 0]),
            sk::skill_use(sk::OP_USE_MOB, 0, 1, 1, 7, skills::MOB_LEVEL, [100, 0, 0]),
            sk::costs(10, 0)
        ]
    );
    assert_eq!(zones.mob(1, 1).map(|m| m.hp), Some(600));
}

#[test]
fn armour_lapis_strike_back_at_players_until_both_guards_are_set() {
    let (_, _, defs) = fighting_ground();
    let zones = Zones::default();
    lapis_skills(&zones);
    let (a, mut a_in, _) = member(1, 100.0, 100.0, 0, b"Alice", 20);
    let (b, mut b_in, _) = member(2, 100.0, 102.0, 1, b"Bob", 20);
    zones.enter(1, with_options(a, [0; 6], 205));
    zones.enter(1, with_options(b, [0; 6], 205));
    received(&mut a_in);
    received(&mut b_in);

    // Bob hits Alice (no weapon lapis); Alice's armour strikes Bob; that hit sets off Bob's armour at Alice; Alice's guard is still
    // set, so it ends there.
    assert!(matches!(strike(&zones, &defs, 2, &blows(20, 10), 1, false), AttackOutcome::Struck { killed: false, .. }));
    let lapis = |caster, target| sk::skill_use(sk::OP_USE_USER, combat::RESULT_HIT, caster, target, 205, skills::MOB_LEVEL, [0; 3]);
    for inbox in [&mut a_in, &mut b_in] {
        let got: Vec<Vec<u8>> = received(inbox).into_iter().filter(|p| opcode(p) == sk::OP_USE_USER).collect();
        assert_eq!(got, [lapis(1, 2), lapis(2, 1)]);
    }
    edit(&zones, 1, |p| assert!(!p.options_running && p.occupant.buffs.has_debuff(), "debuffed"));
    edit(&zones, 2, |p| assert!(!p.options_running && p.occupant.buffs.has_debuff(), "debuffed"));
}

/// A map with a neutral-based obelisk at (100, 105) (monsters 50 neutral, 51 light, 52 fury, 30 HP, 3 min respawn) and a
/// group of two guards at (120, 120), radius 1, only for a faction owner (61 light, 62 fury).
fn obelisk_ground() -> (World, MobDefs) {
    use crate::mobs::MobDef;
    use crate::obelisk::{FileGuards, FileObelisk};
    use std::time::Duration;
    let mut world = World::build([(1, 2048)], &std::collections::HashSet::new(), []);
    world.set_mob_data([(1, vec![0u8; 2048 * 2048 / 8])].into(), HashMap::new());
    let mob = |id, country| MobDef {
        id,
        level: 1,
        hp: 30,
        size: 1,
        respawn: Duration::from_secs(180),
        country,
        obelisk: true,
        ..Default::default()
    };
    let defs = MobDefs::new([mob(50, 2), mob(51, 0), mob(52, 1), mob(61, 0), mob(62, 1)]);
    let guards = vec![FileGuards { centre: [120.0, 0.0, 120.0], radius: 1.0, mobs: [0, 61, 62], count: 2 }];
    world.set_obelisks(&[FileObelisk { map: 1, number: 1, position: [100.0, 0.0, 105.0], mobs: [50, 51, 52], country: 0, guards }], &defs);
    (world, defs)
}

#[test]
fn obelisks_fall_to_their_killer_rise_with_guards_and_reset() {
    use crate::combat::{self, Gear};
    use crate::obelisk::{Event, LIGHT, NEUTRAL, Owner};
    use std::time::Duration;
    let (world, defs) = obelisk_ground();
    let zones = Zones::new(&world, &defs);
    let (a, mut a_in) = player(1, 100.0, 100.0);
    let entering = zones.enter(1, a);
    let list = entering.iter().find(|p| opcode(p) == 0x2101).expect("the obelisk list");
    assert_eq!(&payload(list)[..6], &[1, 1, 0, 0, 0, 0], "one row: id 1, neutral");
    let now = Instant::now();
    assert_eq!(zones.respawn(now, &world, &defs, &mut || 0), 1, "the neutral obelisk has no guards");
    drain(&mut a_in);

    // A light player kills it: light owns it.
    let fighter = combat::player(10, [100, 10, 10, 10, 10, 0], &Gear { kind: Some(combat::MELEE), ..Default::default() });
    let attack =
        Attack { attacker: 1, country: LIGHT, fighter: &fighter, kind: combat::MELEE, slack: 2.0, range: 0, rec: 0, cross_road: false };
    let mut rolls = [0, 99].into_iter().cycle();
    let outcome = zones.attack_mob(1, 1, &attack, &defs, no_loot(), now, &mut || rolls.next().unwrap());
    assert!(matches!(outcome, AttackOutcome::Struck { killed: true, .. }), "{outcome:?}");
    assert_eq!(received(&mut a_in), [shaiya_proto::obelisk::change(1, 1)], "0x2102 to the map");
    let owner = Owner { map: 1, number: 1, country: LIGHT, last: LIGHT };
    assert_eq!(zones.take_obelisk_events(), [Event::Fell { mob_id: 50, from: NEUTRAL, to: LIGHT }, Event::Owner(owner)]);
    // It rises 3 minutes after its body went, as light's, with light's guards.
    assert_eq!(zones.respawn(now + Duration::from_secs(600), &world, &defs, &mut || 0), 0, "the body still lies");
    assert_eq!(zones.remove_bodies(now + Duration::from_secs(10), &defs, &mut || 0), 1);
    let up = now + Duration::from_secs(190);
    assert_eq!(zones.respawn(up - Duration::from_millis(1), &world, &defs, &mut || 0), 0);
    assert_eq!(zones.respawn(up, &world, &defs, &mut || 0), 3);
    let kinds: Vec<u16> = (2..=4).filter_map(|id| zones.mob(1, id)).map(|m| m.mob_id).collect();
    assert_eq!(kinds, [51, 61, 61]);
    assert_eq!(zones.mob(1, 3).map(|m| m.position), Some([119.0, 0.0, 119.0]), "rand 0: centre − radius");
    assert_eq!(zones.respawn(up, &world, &defs, &mut || 0), 0, "the group is full");

    // A guard dies: its body counts until it goes, then the group is due again after its respawn time.
    let guard = Attack { slack: 40.0, ..attack };
    let light_guard_defs = MobDefs::new([61, 51, 50, 52, 62].map(|id| crate::mobs::MobDef { country: 2, ..*defs.get(id).unwrap() }));
    assert!(matches!(
        zones.attack_mob(1, 3, &guard, &light_guard_defs, no_loot(), up, &mut || 0),
        AttackOutcome::Struck { killed: true, .. }
    ));
    assert!(zones.take_obelisk_events().is_empty(), "a guard's death tells nothing");
    assert_eq!(zones.remove_bodies(up + Duration::from_secs(10), &defs, &mut || 0), 1);
    assert_eq!(zones.respawn(up + Duration::from_secs(189), &world, &defs, &mut || 0), 0);
    assert_eq!(zones.respawn(up + Duration::from_secs(190), &world, &defs, &mut || 0), 1);
    drain(&mut a_in);

    // The reset: the obelisk and both guards go; down for ten minutes at its base country.
    let reset = up + Duration::from_secs(200);
    zones.reset_obelisks(reset, &world);
    let got = drain(&mut a_in);
    assert_eq!(got.iter().filter(|(op, _)| *op == 0x0602).count(), 3);
    assert_eq!(got.last(), Some(&(0x2102, 1)), "neutral: state 0 after id 1");
    assert_eq!(zones.take_obelisk_events(), [Event::Owner(Owner { country: NEUTRAL, ..owner })]);
    assert_eq!(zones.mob_count(1), 0);
    assert_eq!(zones.respawn(reset + Duration::from_secs(599), &world, &defs, &mut || 0), 0);
    assert_eq!(zones.respawn(reset + Duration::from_secs(600), &world, &defs, &mut || 0), 1);
}

#[test]
fn saved_owners_raise_their_obelisks_and_a_capture_takes_the_guards() {
    use crate::obelisk::{Event, FURY, LIGHT, Owner};
    let (world, defs) = obelisk_ground();
    let zones = Zones::new(&world, &defs);
    assert_eq!(
        zones.set_obelisk_owners(&[
            Owner { map: 1, number: 1, country: FURY, last: FURY },
            Owner { map: 1, number: 2, country: 0, last: 0 }
        ]),
        1
    );
    let (a, _a_in) = player(1, 100.0, 100.0);
    zones.enter(1, a);
    let now = Instant::now();
    assert_eq!(zones.respawn(now, &world, &defs, &mut || 0), 3);
    assert_eq!(zones.mob(1, 1).map(|m| m.mob_id), Some(52), "fury's obelisk");
    assert!(!zones.attackable(1, 1, FURY, &defs), "not by its own faction");
    let fighter = combat::player(10, [100, 10, 10, 10, 10, 0], &combat::Gear { kind: Some(combat::MELEE), ..Default::default() });
    let attack =
        Attack { attacker: 1, country: LIGHT, fighter: &fighter, kind: combat::MELEE, slack: 2.0, range: 0, rec: 0, cross_road: false };
    let outcome = zones.attack_mob(1, 1, &attack, &defs, no_loot(), now, &mut || 0);
    assert!(matches!(outcome, AttackOutcome::Struck { killed: true, .. }), "{outcome:?}");
    assert_eq!(zones.mob_count(1), 1, "the guards went at once, the body lies");
    assert_eq!(zones.take_obelisk_events()[0], Event::Fell { mob_id: 52, from: FURY, to: LIGHT });
}

/// Maps 1 (static) and 65 (a party instance map), each with portal 0 of id 1001 and a two-stage boss group at (100, 100):
/// bosses 70 (respawn 0 s) then 71 (2 h), on the second stage two followers 72 (1 min) at (110, 110) radius 1, and three
/// chests 73 when the last stage falls; the header's wait 1 h. Every monster 30 HP, of neither country.
fn boss_ground() -> (World, MobDefs) {
    use crate::boss::{FileBossGroup, FileFollowers};
    use crate::mobs::MobDef;
    use crate::world::Portal;
    use std::time::Duration;
    let mut world = World::build([(1, 1024), (65, 1024)], &std::collections::HashSet::new(), []);
    let door = Portal { position: [90.0, 0.0, 90.0], country: 1001, level_min: 0, level_max: 0, to_map: 1, to: [95.0, 0.0, 95.0] };
    world.set_portals([(1, vec![door]), (65, vec![door])].into(), [(65, crate::world::create_type::PARTY)].into());
    world.set_mob_data([(1, vec![0u8; 1024 * 1024 / 8]), (65, vec![0u8; 1024 * 1024 / 8])].into(), HashMap::new());
    let mob = |id, secs| MobDef { id, level: 1, hp: 30, size: 1, respawn: Duration::from_secs(secs), country: 2, ..Default::default() };
    let defs = MobDefs::new([mob(70, 0), MobDef { respawn_code: 13, ..mob(71, 7200) }, mob(72, 60), mob(73, 5)]);
    let line = |mobs: Vec<u16>, count| FileFollowers { centre: [110.0, 0.0, 110.0], radius: 1.0, mobs, count };
    let group = |map| FileBossGroup {
        number: 1,
        map,
        respawn_secs: 3600,
        portal: (map, 1001),
        position: [100.0, 0.0, 100.0],
        mobs: vec![70, 71],
        random: None,
        followers: vec![line(vec![0, 72, 0], 2), line(vec![0, 0, 73], 3)],
    };
    world.set_boss_groups(&[group(1), group(65)], &defs);
    (world, defs)
}

fn kill(zones: &Zones, zone: u16, id: u32, defs: &MobDefs, now: Instant) {
    let fighter = combat::player(10, [100, 10, 10, 10, 10, 0], &combat::Gear { kind: Some(combat::MELEE), ..Default::default() });
    let attack =
        Attack { attacker: 1, country: 0, fighter: &fighter, kind: combat::MELEE, slack: 40.0, range: 0, rec: 0, cross_road: false };
    let mut rolls = [0, 99].into_iter().cycle();
    let outcome = zones.attack_mob(zone, id, &attack, defs, no_loot(), now, &mut || rolls.next().unwrap());
    assert!(matches!(outcome, AttackOutcome::Struck { killed: true, .. }), "{outcome:?}");
}

fn kinds(zones: &Zones, zone: u16) -> Vec<u16> {
    let mut kinds: Vec<u16> = zones.mobs.guard()[&zone].mobs.values().filter(|m| m.dead_until.is_none()).map(|m| m.mob_id).collect();
    kinds.sort_unstable();
    kinds
}

#[test]
fn a_boss_chain_on_a_static_map_opens_its_portal_for_everyone() {
    use std::time::Duration;
    let (world, defs) = boss_ground();
    let zones = Zones::new(&world, &defs);
    let (a, mut a_in) = player(1, 100.0, 100.0);
    zones.enter(1, a);
    let t = Instant::now();
    assert_eq!(zones.respawn(t, &world, &defs, &mut || 0), 1, "the first stage's boss alone");
    assert_eq!(zones.portal_country(1, 0, 1001), crate::boss::PORTAL_SHUT);
    kill(&zones, 1, 1, &defs, t);
    assert_eq!(zones.respawn(t, &world, &defs, &mut || 0), 0, "its body still lies");
    assert_eq!(zones.remove_bodies(t + Duration::from_secs(5), &defs, &mut || 0), 1);
    let t = t + Duration::from_secs(5);
    assert_eq!(zones.respawn(t, &world, &defs, &mut || 0), 3, "1974's own time (0 s): the second stage with its followers");
    assert_eq!(kinds(&zones, 1), [71, 72, 72]);
    assert_eq!(zones.mob(1, 3).map(|m| m.position), Some([109.0, 0.0, 109.0]));

    // A follower comes back after its respawn time while the boss stands.
    kill(&zones, 1, 3, &defs, t);
    assert_eq!(zones.remove_bodies(t + Duration::from_secs(10), &defs, &mut || 0), 1);
    assert_eq!(zones.respawn(t + Duration::from_secs(69), &world, &defs, &mut || 0), 0);
    assert_eq!(zones.respawn(t + Duration::from_secs(70), &world, &defs, &mut || 0), 1);
    drain(&mut a_in);

    // The last stage falls: the followers go, the portal opens for everyone, the chests come.
    let t = t + Duration::from_secs(100);
    kill(&zones, 1, 2, &defs, t);
    assert_eq!(drain(&mut a_in).iter().filter(|(op, _)| *op == 0x0602).count(), 2);
    assert_eq!(zones.portal_country(1, 0, 1001), crate::boss::PORTAL_OPEN);
    assert_eq!(zones.respawn(t, &world, &defs, &mut || 0), 3);
    assert_eq!(kinds(&zones, 1), [73, 73, 73]);
    assert_eq!(zones.respawn(t + Duration::from_secs(600), &world, &defs, &mut || 0), 0, "chests never come back");

    // Its body goes: the chain waits the header's hour (rand 60: no spread); then the first stage rises, the door shuts, the
    // chests go.
    assert_eq!(zones.remove_bodies(t + Duration::from_secs(10), &defs, &mut || 60), 1);
    let due = t + Duration::from_secs(10 + 3600);
    assert_eq!(zones.respawn(due - Duration::from_secs(1), &world, &defs, &mut || 60), 0);
    drain(&mut a_in);
    assert_eq!(zones.respawn(due, &world, &defs, &mut || 60), 1);
    assert_eq!(drain(&mut a_in).iter().filter(|(op, _)| *op == 0x0602).count(), 3, "the chests leave");
    assert_eq!(kinds(&zones, 1), [70]);
    assert_eq!(zones.portal_country(1, 0, 1001), crate::boss::PORTAL_SHUT);
}

#[test]
fn a_boss_chain_in_an_instance_zone_opens_the_zones_pair() {
    use std::time::Duration;
    let (world, defs) = boss_ground();
    let zones = Zones::new(&world, &defs);
    let zone = zones.gm_zone(65, crate::instance::Owner::Party(1), &world, &defs).expect("a zone");
    let (a, _a_in) = player(1, 100.0, 100.0);
    zones.enter(zone, a);
    let t = Instant::now();
    assert_eq!(zones.respawn(t, &world, &defs, &mut || 0), 2, "map 1's chain and the zone's");
    // The ids follow the order the maps are visited, which is not fixed.
    let first = zones.mobs.guard()[&zone].mobs.values().find(|m| m.mob_id == 70).map(|m| m.id).expect("the first stage");
    kill(&zones, zone, first, &defs, t);
    zones.remove_bodies(t + Duration::from_secs(5), &defs, &mut || 0);
    zones.respawn(t + Duration::from_secs(5), &world, &defs, &mut || 0);
    let boss = zones.mobs.guard()[&zone].mobs.values().find(|m| m.mob_id == 71).map(|m| m.id).expect("the second stage");
    kill(&zones, zone, boss, &defs, t + Duration::from_secs(5));
    assert_eq!(zones.instance_portal_country(zone, 1001), crate::boss::PORTAL_OPEN, "the zone's pair");
    assert_eq!(zones.portal_country(65, 0, 1001), 1001, "the map's portal is not changed");
    assert_eq!(zones.portal_country(1, 0, 1001), crate::boss::PORTAL_SHUT, "nor map 1's");
}

#[test]
fn mera_takes_an_obelisk_away_and_it_rises_again() {
    use crate::obelisk::{Event, LIGHT, NEUTRAL, Owner};
    use std::time::Duration;
    let (world, defs) = obelisk_ground();
    let zones = Zones::new(&world, &defs);
    zones.set_obelisk_owners(&[Owner { map: 1, number: 1, country: LIGHT, last: LIGHT }]);
    let (a, mut a_in) = player(1, 100.0, 100.0);
    zones.enter(1, a);
    let t = Instant::now();
    assert_eq!(zones.respawn(t, &world, &defs, &mut || 0), 3);
    drain(&mut a_in);

    // `/mera`: the guards go, `0x2102` with its own country, no fall for the server; 3 minutes after its body, light's again.
    assert_eq!(zones.mob_remove_near(1, 51, [100.0, 0.0, 105.0], 1, t), 1);
    let got = drain(&mut a_in);
    assert_eq!(got.iter().filter(|(op, _)| *op == 0x0602).count(), 2, "{got:?}");
    assert_eq!(got.last(), Some(&(0x2102, 1)));
    assert!(zones.take_obelisk_events().is_empty(), "no kill count, no 0x2103, the owner unchanged");
    assert_eq!(zones.remove_bodies(t, &defs, &mut || 0), 1);
    assert_eq!(zones.respawn(t + Duration::from_secs(179), &world, &defs, &mut || 0), 0);
    assert_eq!(zones.respawn(t + Duration::from_secs(180), &world, &defs, &mut || 0), 3, "with its guards");
    assert_eq!(kinds(&zones, 1), [51, 61, 61]);

    // `/mera t`: back to its base, the owner saved; it rises neutral.
    let t = t + Duration::from_secs(200);
    let id = zones.mobs.guard()[&1].mobs.values().find(|m| m.mob_id == 51).map(|m| m.id).unwrap();
    assert!(zones.mob_remove_id(1, id, t));
    assert_eq!(zones.take_obelisk_events(), [Event::Owner(Owner { map: 1, number: 1, country: NEUTRAL, last: LIGHT })]);
    zones.remove_bodies(t, &defs, &mut || 0);
    assert_eq!(zones.respawn(t + Duration::from_secs(180), &world, &defs, &mut || 0), 1);
    assert_eq!(kinds(&zones, 1), [50]);
}

#[test]
fn mera_on_a_boss_moves_its_chain_on() {
    use std::time::Duration;
    let (world, defs) = boss_ground();
    let zones = Zones::new(&world, &defs);
    let (a, mut a_in) = player(1, 100.0, 100.0);
    zones.enter(1, a);
    let t = Instant::now();
    zones.respawn(t, &world, &defs, &mut || 0);
    kill(&zones, 1, 1, &defs, t);
    zones.remove_bodies(t + Duration::from_secs(5), &defs, &mut || 0);
    let t = t + Duration::from_secs(5);
    assert_eq!(zones.respawn(t, &world, &defs, &mut || 0), 3);
    drain(&mut a_in);
    // The last stage taken away: its followers go, the portal opens and the chests come, but the chain waits only 71's own 2 h.
    assert_eq!(zones.mob_remove_near(1, 71, [100.0, 0.0, 100.0], 1, t), 1);
    assert_eq!(drain(&mut a_in).iter().filter(|(op, _)| *op == 0x0602).count(), 2);
    assert_eq!(zones.portal_country(1, 0, 1001), crate::boss::PORTAL_OPEN);
    assert_eq!(zones.remove_bodies(t, &defs, &mut || 60), 1);
    assert_eq!(zones.respawn(t, &world, &defs, &mut || 0), 3, "the chests");
    assert_eq!(zones.respawn(t + Duration::from_secs(7199), &world, &defs, &mut || 0), 0);
    assert_eq!(zones.respawn(t + Duration::from_secs(7200), &world, &defs, &mut || 0), 1);
    assert_eq!(kinds(&zones, 1), [70]);
}

/// The fighting ground with its monster (`AI` 11) running `machine`, and the lines it may say.
fn scripted_ground(machine: crate::mob_ai::Machine) -> (World, MobDefs) {
    let (mut world, mut def, _) = fighting_ground();
    def.ai = 11;
    // `Battle` runs on entering and on every update: with no wait after a blow it would strike twice in a turn.
    def.attacks[0].time = std::time::Duration::from_secs(2);
    let mut library = crate::mob_ai::Library::default();
    library.machines.insert(11, machine);
    library.lines.insert(2, b"Die, fool!".to_vec());
    world.set_mob_ai(library);
    (world, MobDefs::new([def]))
}

fn script_of(zones: &Zones) -> crate::mob_ai::Script {
    zones.mobs.guard()[&1].scripts[&1].clone()
}

#[test]
fn scripted_monsters_run_their_machines() {
    use std::time::Duration;
    let (world, defs) = scripted_ground(crate::mob_ai::samples::croco1());
    let zones = Zones::new(&world, &defs);
    let (a, mut a_in) = player(1, 100.0, 102.0);
    zones.enter(1, a);
    let now = Instant::now();
    zones.respawn(now, &world, &defs, &mut || 0);
    assert_eq!(zones.mob(1, 1).map(|m| (m.state, m.scripted)), Some((MobState::Scripted(None), true)));
    drain(&mut a_in);
    // The idle pass leaves it alone; its first turn enters "init" and takes "init -> idle" (an action, no test).
    zones.idle(now, &world, &defs, &mut || 0);
    zones.scripts(now, &world, &defs, no_loot(), &mut || 0);
    assert_eq!(script_of(&zones).state, 1);
    // "idle -> battle": IsFirstAttack() finds the player 3 away (chase range 20) and takes it as target; entering "battle"
    // runs `Battle`: in reach, a normal attack (0x0605).
    let mut rolls = [0, 99].into_iter().cycle();
    zones.scripts(now + Duration::from_secs(1), &world, &defs, no_loot(), &mut || rolls.next().unwrap());
    assert_eq!(script_of(&zones).state, 2);
    assert_eq!(zones.mob(1, 1).map(|m| m.state), Some(MobState::Scripted(Some(1))));
    assert!(drain(&mut a_in).iter().any(|&(op, _)| op == 0x0605));
    // The attack pass does not fight for it.
    zones.mob_fight(now + Duration::from_secs(5), &world, &defs, no_loot(), &mut || 0);
    assert!(drain(&mut a_in).is_empty());
    // At half HP "battle -> BeStrong": SetStr(1.5) and nCount = 1; then back to "battle".
    zones.mobs.guard().get_mut(&1).unwrap().mobs.get_mut(&1).unwrap().hp = 500;
    zones.scripts(now + Duration::from_secs(2), &world, &defs, no_loot(), &mut || 50);
    let s = script_of(&zones);
    assert_eq!((s.state, s.flags.strength, s.vars[0]), (3, 1.5, crate::mob_ai::Value::int(1)));
    zones.scripts(now + Duration::from_secs(3), &world, &defs, no_loot(), &mut || 50);
    assert_eq!(script_of(&zones).state, 2);
    // Home again: it starts over (the variables at 0, "init", then "idle"), the strength kept.
    zones.mobs.guard().get_mut(&1).unwrap().mobs.get_mut(&1).unwrap().state = MobState::Home;
    zones.scripts(now + Duration::from_secs(4), &world, &defs, no_loot(), &mut || 50);
    assert_eq!(zones.mob(1, 1).map(|m| m.state), Some(MobState::Scripted(None)));
    zones.scripts(now + Duration::from_secs(5), &world, &defs, no_loot(), &mut || 50);
    let s = script_of(&zones);
    assert_eq!((s.vars[0], s.flags.strength), (crate::mob_ai::Value::int(0), 1.5));
}

#[test]
fn players_hits_mark_scripted_monsters() {
    use crate::combat::{self, Gear};
    let (world, defs) = scripted_ground(crate::mob_ai::samples::croco1());
    let zones = Zones::new(&world, &defs);
    let (a, _a_in) = player(1, 100.0, 140.0);
    zones.enter(1, a);
    let now = Instant::now();
    zones.respawn(now, &world, &defs, &mut || 0);
    let fighter = combat::player(10, [10; 6], &Gear { kind: Some(combat::MELEE), ..Default::default() });
    let attack =
        Attack { attacker: 1, country: 0, fighter: &fighter, kind: combat::MELEE, slack: 40.0, range: 0, rec: 0, cross_road: false };
    let mut rolls = [0, 99].into_iter().cycle();
    zones.attack_mob(1, 1, &attack, &defs, no_loot(), now, &mut || rolls.next().unwrap());
    // Out of its chase range, but the hit made the player its target and marked it attacked; it stays in its machine.
    let f = script_of(&zones).flags;
    assert!(f.attacked && f.battle && f.at_moment);
    assert_eq!(zones.mob(1, 1).map(|m| m.state), Some(MobState::Scripted(Some(1))));
}

#[test]
fn scripted_monsters_speak() {
    let (world, defs) = scripted_ground(crate::mob_ai::samples::sayer());
    let zones = Zones::new(&world, &defs);
    let (a, mut a_in) = player(1, 100.0, 102.0);
    zones.enter(1, a);
    let now = Instant::now();
    zones.respawn(now, &world, &defs, &mut || 0);
    drain(&mut a_in);
    let mut inbox = Vec::new();
    // "talk" says line 1 to a target it does not have, and there is no line 1; "shout" says line 2 to everyone within 200.
    zones.scripts(now, &world, &defs, no_loot(), &mut || 0);
    while let Ok(p) = a_in.try_recv() {
        inbox.push(p);
    }
    assert_eq!(inbox.len(), 1);
    assert_eq!(opcode(&inbox[0]), 0x1109);
    assert_eq!(payload(&inbox[0]), [&[1, 1, 0, 0, 0, 11][..], b"Die, fool!\0"].concat());
}

#[test]
fn scripted_monsters_keep_their_machine_and_their_shields() {
    use crate::combat::{self, Gear};
    let (world, defs) = scripted_ground(crate::mob_ai::samples::croco1());
    let zones = Zones::new(&world, &defs);
    let (a, _a_in) = player(1, 100.0, 140.0);
    let (b, _b_in) = player(2, 100.0, 108.0);
    zones.enter(1, a);
    zones.enter(1, b);
    let now = Instant::now();
    zones.respawn(now, &world, &defs, &mut || 0);
    let fighter = combat::player(10, [10; 6], &Gear { kind: Some(combat::MELEE), ..Default::default() });
    let hit = |attacker| Attack {
        attacker,
        country: 0,
        fighter: &fighter,
        kind: combat::MELEE,
        slack: 40.0,
        range: 0,
        rec: 5,
        cross_road: false,
    };
    let mut rolls = [0, 99].into_iter().cycle();
    zones.attack_mob(1, 1, &hit(1), &defs, no_loot(), now, &mut || rolls.next().unwrap());
    assert_eq!(zones.mob(1, 1).map(|m| m.state), Some(MobState::Scripted(Some(1))));
    // Player 2 out-aggros player 1: the monster turns to it and stays in its machine (deviation 4).
    {
        let mut all = zones.mobs.guard();
        let mobs = all.get_mut(&1).unwrap();
        mobs.add_damage(1, 2, 10, 1000, 0);
        let players = zones.maps.guard();
        retarget(mobs, &players[&1], 1, &defs.get(7).copied().unwrap(), now);
    }
    assert_eq!(zones.mob(1, 1).map(|m| m.state), Some(MobState::Scripted(Some(2))));
    // Losing it leaves the machine running without a target (deviation 6).
    scripted::lose_target(zones.mobs.guard().get_mut(&1).unwrap(), 1);
    assert_eq!(zones.mob(1, 1).map(|m| m.state), Some(MobState::Scripted(None)));
    // With every attack kind turned off (`SetEnableOnlyAttackType(5)`), blows miss (deviation 5); melee only lets melee through.
    let shield = |kinds| zones.mobs.guard().get_mut(&1).unwrap().scripts.get_mut(&1).unwrap().flags.attack_kinds = kinds;
    shield(crate::mob_ai::attack_kinds(5));
    let before = zones.mob(1, 1).unwrap().hp;
    let outcome = zones.attack_mob(1, 1, &hit(2), &defs, no_loot(), now, &mut || 0);
    assert!(matches!(outcome, AttackOutcome::Struck { killed: false, .. }));
    assert_eq!(zones.mob(1, 1).unwrap().hp, before, "a miss");
    shield(crate::mob_ai::attack_kinds(0));
    zones.attack_mob(1, 1, &hit(2), &defs, no_loot(), now, &mut || 0);
    assert!(zones.mob(1, 1).unwrap().hp < before, "melee gets through");
    assert_eq!(crate::mob_ai::attack_kinds(2), [false, false, true, false]);
}

// ------------------------------------------------------------------------------------------------------------------------------
// The attack point (docs/plans/ext-2-rules.md section 5, point 1)
// ------------------------------------------------------------------------------------------------------------------------------

fn put_rules(zones: &Zones, rules: &[(&str, serde_json::Value)]) {
    zones.set_rules(crate::rules::RuleSet::from_values(rules).unwrap());
}

fn level_of(zones: &Zones, id: u32, level: u16) {
    edit(zones, id, |p| p.occupant.vitals.guard.level = level);
}

#[test]
fn a_deny_attack_rule_refuses_on_its_map_only() {
    use serde_json::json;
    let (_, _, defs) = fighting_ground();
    let (zones, _, _) = duel_ground();
    assert!(zones.may_attack_player(1, 1, 2, false) && zones.rules().is_empty());
    put_rules(&zones, &[("safe", json!({ "type": "deny", "point": "attack", "maps": [1], "message": "No." }))]);
    for (me, target) in [(1, 2), (2, 1)] {
        let Verdict::Ruled(refusal) = zones.attack_verdict(1, me, target, false) else { panic!("refused") };
        assert_eq!((&*refusal.name, refusal.text.as_deref()), ("safe", Some("No.")));
        assert!(!zones.may_attack_player(1, me, target, false));
    }
    assert!(matches!(zones.attack_verdict(1, 2, 3, false), Verdict::No), "one country: the core says no, not a rule");
    assert!(matches!(zones.attack_verdict(1, 1, 1, false), Verdict::No));
    let health = zones.health(1, 1);
    assert!(matches!(strike(&zones, &defs, 2, &blows(20, 100), 1, false), AttackOutcome::Gone), "a later blow is dropped silently");
    assert_eq!(zones.health(1, 1), health);
    put_rules(&zones, &[("elsewhere", json!({ "type": "deny", "point": "attack", "maps": [2] }))]);
    assert!(zones.may_attack_player(1, 1, 2, false));
    assert!(matches!(strike(&zones, &defs, 2, &blows(20, 100), 1, false), AttackOutcome::Struck { .. }));
    put_rules(&zones, &[]);
    assert!(zones.may_attack_player(1, 1, 2, false));
}

#[test]
fn level_gap_protects_both_ways() {
    use serde_json::json;
    let (zones, _, _) = duel_ground();
    put_rules(&zones, &[("gap", json!({ "type": "level_gap", "gap": 20 }))]);
    level_of(&zones, 1, 20);
    level_of(&zones, 3, 41);
    assert!(!zones.may_attack_player(1, 1, 3, false) && !zones.may_attack_player(1, 3, 1, false), "21 apart: neither may attack");
    let Verdict::Ruled(refusal) = zones.attack_verdict(1, 3, 1, false) else { panic!("refused") };
    assert_eq!(refusal.text.as_deref(), Some("The level difference is too great to fight."));
    level_of(&zones, 3, 40);
    assert!(zones.may_attack_player(1, 1, 3, false) && zones.may_attack_player(1, 3, 1, false), "20 apart: both may");
}

#[test]
fn a_duel_and_a_guild_war_still_fight_across_the_gap_and_the_rules() {
    use serde_json::json;
    let (zones, mut inboxes, _) = duel_ground();
    put_rules(&zones, &[("gap", json!({ "type": "level_gap", "gap": 20 })), ("none", json!({ "type": "deny", "point": "attack" }))]);
    level_of(&zones, 3, 60);
    duel_window(&zones, &mut inboxes);
    duel_fight(&zones, &mut inboxes);
    assert!(zones.may_attack_player(1, 2, 3, false) && zones.may_attack_player(1, 3, 2, false), "the duel is agreed to");
    assert!(!zones.may_attack_player(1, 1, 3, false), "but Alice is still ruled out");
    // A guild war: the sides are enemies whatever the rules say.
    let (zones, mut inboxes, _) = war_ground();
    put_rules(&zones, &[("none", json!({ "type": "deny", "point": "attack" }))]);
    war_countdown(&zones, &mut inboxes);
    assert!(enemies(&zones, 1, 3) && enemies(&zones, 4, 2) && !enemies(&zones, 1, 2));
}

#[test]
fn an_area_skill_skips_a_protected_victim_and_hits_the_others() {
    use serde_json::json;
    let (zones, _, _) = duel_ground();
    let (dave, _dave_in, _) = member(4, 100.0, 103.0, 0, b"Dave", 60);
    zones.enter(1, dave);
    put_rules(&zones, &[("gap", json!({ "type": "level_gap", "gap": 20 }))]);
    level_of(&zones, 2, 20);
    let around = SkillDef { apply_range: 10, ..sure_skill(skills::ATTACK, skills::TARGET_AROUND_ONESELF, 21, [20, 0, 0]) };
    bob_casts(&zones, SkillTarget::Player(2), &around).unwrap();
    assert_eq!(zones.health(1, 1).map(|h| h[0]), Some(80), "Alice, 20 and 20, is hit");
    assert_eq!(zones.health(1, 4).map(|h| h[0]), Some(100), "Dave, 40 levels above, is skipped");
    // One target: the cast does not go off at all; the item skill of a lapis finds no target either.
    let strike = sure_skill(skills::ATTACK, skills::TARGET_ONE, 21, [50, 0, 0]);
    assert!(bob_casts(&zones, SkillTarget::Player(4), &strike).is_none());
    assert_eq!(zones.health(1, 4).map(|h| h[0]), Some(100));
    put_rules(&zones, &[]);
    bob_casts(&zones, SkillTarget::Player(2), &around).unwrap();
    assert_eq!(zones.health(1, 4).map(|h| h[0]), Some(80), "without the rule Dave is hit");
}

#[test]
fn thorns_and_fields_skip_a_protected_victim_even_after_the_owner_left() {
    use serde_json::json;
    use std::time::Duration;
    let gap = [("gap", json!({ "type": "level_gap", "gap": 20 }))];
    // Fire Thorns.
    let (zones, defs, _, _) = skill_ground(0);
    edit(&zones, 2, |p| p.occupant.shape.country = 1);
    level_of(&zones, 1, 10);
    level_of(&zones, 2, 50);
    put_rules(&zones, &gap);
    let thorns = SkillDef { id: 47, apply_range: 50, damage: [8, 0, 0], ..sure_buff(skills::TARGET_ONESELF, buffs::FIRE_THORNS, 300) };
    use_on(&zones, &defs, SkillTarget::Player(1), &thorns).unwrap();
    let now = Instant::now();
    zones.keep_effects(now + Duration::from_secs(3), &World::default(), &defs, no_loot(), &mut || 0);
    assert_eq!(zones.health(1, 2).unwrap()[0], 100, "protected");
    level_of(&zones, 2, 30);
    zones.keep_effects(now + Duration::from_secs(6), &World::default(), &defs, no_loot(), &mut || 0);
    assert_eq!(zones.health(1, 2).unwrap()[0], 92, "20 apart: hit");
    // A fire wall: the owner's level is the one the wall was made with.
    let (zones, defs, _, _) = skill_ground(0);
    edit(&zones, 2, |p| p.occupant.shape.country = 1);
    level_of(&zones, 1, 10);
    level_of(&zones, 2, 50);
    put_rules(&zones, &gap);
    let wall = SkillDef { apply_range: 50, keep_time: 20, ..sure_skill(skills::FIELD, 1, skills::FIRE_WALL, [32, 0, 0]) };
    use_on(&zones, &defs, SkillTarget::Player(1), &wall).unwrap();
    zones.leave(1, 1);
    let world = World::default();
    zones.fields(now, &world, &defs, no_loot(), &mut || 0);
    assert_eq!(zones.health(1, 2).unwrap()[0], 100, "the wall remembers its owner's level");
    put_rules(&zones, &[]);
    zones.fields(now + Duration::from_secs(1), &world, &defs, no_loot(), &mut || 0);
    assert_eq!(zones.health(1, 2).unwrap()[0], 68);
}

#[test]
fn the_rules_are_swapped_whole() {
    use serde_json::json;
    let zones = Zones::default();
    assert!(zones.rules().is_empty());
    put_rules(&zones, &[("a", json!({ "type": "deny", "point": "attack" }))]);
    let before = zones.rules();
    put_rules(&zones, &[]);
    assert_eq!((before.len(), zones.rules().len()), (1, 0), "a snapshot taken earlier keeps what it had");
}

#[test]
fn trade_requests_ask_the_rules_after_the_cores_own_checks() {
    use serde_json::json;
    use shaiya_proto::trade as tp;
    let now = Instant::now();
    // Alice (1, Light), Bob (2, Fury) and Carol (3, Fury).
    let (zones, mut inboxes, _) = duel_ground();
    let [a_in, b_in, c_in] = &mut inboxes;
    // No rule: the other faction is refused with START_BUSY and nothing more; a rule for the permission lets it through.
    assert!(zones.trade_request(1, 1, 2, now).is_none());
    assert_eq!(received(a_in), [tp::start(2, tp::START_BUSY)]);
    put_rules(&zones, &[("grant", json!({ "type": "cross_faction_trade", "level": "60-" }))]);
    // Who defaults to both: one trader at 59 is refused, both at 60 may.
    level_of(&zones, 1, 60);
    level_of(&zones, 2, 59);
    assert!(zones.trade_request(1, 1, 2, now).is_none());
    assert_eq!(received(a_in), [tp::start(2, tp::START_BUSY)]);
    level_of(&zones, 2, 60);
    assert!(zones.trade_request(1, 1, 2, now).is_none());
    assert_eq!(received(b_in), [tp::request(1)]);
    assert!(received(a_in).is_empty());
    assert!(zones.trade_end(2));
    drain(a_in);
    // A deny trade rule beats the permission: START_BUSY for the requester, the refusal returned for the whisper, nothing for Bob.
    put_rules(
        &zones,
        &[("grant", json!({ "type": "cross_faction_trade" })), ("busy", json!({ "type": "deny", "point": "trade", "maps": [1], "message": "Not here." }))],
    );
    let refusal = zones.trade_request(1, 1, 2, now).expect("refused by a rule");
    assert_eq!((&*refusal.name, refusal.text.as_deref()), ("busy", Some("Not here.")));
    assert_eq!(received(a_in), [tp::start(2, tp::START_BUSY)]);
    assert!(received(b_in).is_empty() && !zones.trades.guard().busy(1));
    // The same faction too, and the core's own refusal (oneself) says nothing more than START_BUSY.
    assert!(zones.trade_request(1, 2, 3, now).is_some());
    drain(b_in);
    assert!(zones.trade_request(1, 2, 2, now).is_none(), "the core's check comes first");
    assert_eq!(received(b_in), [tp::start(2, tp::START_BUSY)]);
    assert!(received(c_in).is_empty());
    // Elsewhere the deny does not hold.
    put_rules(&zones, &[("busy", json!({ "type": "deny", "point": "trade", "maps": [2] }))]);
    assert!(zones.trade_request(1, 2, 3, now).is_none());
    assert_eq!(received(c_in), [tp::request(2)]);
}

// ------------------------------------------------------------------------------------------ the scripts' events and spawns
// (docs/plans/ext-5-lua.md section 12, step 7). The events are queued by the zone and run on the script thread, so the tests let a
// script write what it saw into the extension's own stored values and wait for it.

/// Starts the script thread over `zones` with one extension `w` running `source`, whose `note(text)` appends to the stored `log`.
fn watch(zones: &Zones, source: &str) {
    use crate::script::{ExtInput, LoadContext, ScriptSet};
    zones.start_script_thread();
    let ctx = LoadContext { store: Some(zones.script_store()), ..LoadContext::default() };
    let prelude = "local function note(text) game.set('log', (game.get('log') or '') .. text .. ';') end\n";
    let input = ExtInput { name: "w".into(), scripts: vec![("main.lua".into(), format!("{prelude}{source}"))], options: Vec::new() };
    let set = ScriptSet::load(&[input], ctx, 16);
    assert!(set.failed().is_empty(), "{:?}", set.failed());
    zones.set_ext_scripts(set);
}

/// The script's log once it has `count` entries (within five seconds).
fn logged(zones: &Zones, count: usize) -> Vec<String> {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let log = zones.script_store().get_global("w", "log").and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default();
        let entries: Vec<String> = log.split(';').filter(|e| !e.is_empty()).map(str::to_owned).collect();
        if entries.len() >= count || std::time::Instant::now() > until {
            return entries;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[test]
fn a_kill_queues_one_kill_event_with_the_party_near() {
    use crate::combat::{self, Gear};
    use crate::mobs::{MobDef, MobDefs, SpawnArea};
    let mut world = World::build([(1, 2048)], &std::collections::HashSet::new(), []);
    world.set_mob_data(
        [(1, vec![0u8; 2048 * 2048 / 8])].into(),
        [(1, vec![SpawnArea::new([100.0, 0.0, 105.0, 100.0, 0.0, 105.0], vec![(7, 1)])])].into(),
    );
    let defs = MobDefs::new([MobDef { id: 7, level: 4, hp: 30, size: 1, country: 2, respawn: Duration::from_secs(30), ..Default::default() }]);
    let zones = Zones::new(&world, &defs);
    let (a, _a_in, _) = member(1, 100.0, 100.0, 0, b"Alice", 10);
    let (b, _b_in, _) = member(2, 100.0, 199.0, 0, b"Bob", 20);
    let (c, _c_in, _) = member(3, 100.0, 201.0, 0, b"Carol", 12);
    for o in [a, b, c] {
        zones.enter(1, o);
    }
    watch(
        &zones,
        "game.on('kill', function(p, mob, near)
           local names = {} for _, n in ipairs(near) do names[#names + 1] = n.name end
           note('kill ' .. p.name .. ' ' .. mob.id .. ' ' .. mob.uid .. ' L' .. mob.level .. ' map ' .. mob.map .. ' boss ' .. tostring(mob.boss) .. ' near ' .. table.concat(names, ','))
         end)
         game.on('boss_kill', function() note('boss') end)",
    );
    let fighter = combat::player(10, [100, 10, 10, 10, 10, 0], &Gear { kind: Some(combat::MELEE), ..Default::default() });
    let attack =
        Attack { attacker: 1, country: 0, fighter: &fighter, kind: combat::MELEE, slack: 2.0, range: 0, rec: 0, cross_road: false };
    let now = Instant::now();
    // Alone: the near list is the killer.
    zones.respawn(now, &world, &defs, &mut || 0);
    let AttackOutcome::Struck { killed, .. } = zones.attack_mob(1, 1, &attack, &defs, no_loot(), now, &mut || 0) else { panic!("a hit") };
    assert!(killed);
    // In a party of three with Carol too far: the members near.
    join(&zones, 1, 2);
    join(&zones, 1, 3);
    let later = now + Duration::from_secs(60);
    zones.remove_bodies(later, &defs, &mut || 0);
    zones.respawn(later + Duration::from_secs(60), &world, &defs, &mut || 0);
    let AttackOutcome::Struck { killed, .. } = zones.attack_mob(1, 2, &attack, &defs, no_loot(), later, &mut || 0) else { panic!("a hit") };
    assert!(killed);
    assert_eq!(logged(&zones, 2), ["kill Alice 7 1 L4 map 1 boss false near Alice", "kill Alice 7 2 L4 map 1 boss false near Alice,Bob"]);
    // Once per death: nothing else came.
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(logged(&zones, 3).len(), 2);
}

#[test]
fn a_boss_groups_boss_queues_kill_then_boss_kill() {
    let (world, defs) = boss_ground();
    let zones = Zones::new(&world, &defs);
    let (a, _a_in) = player(1, 100.0, 100.0);
    zones.enter(1, a);
    watch(&zones, "game.on('kill', function(p, mob) note('kill ' .. mob.id .. ' ' .. tostring(mob.boss)) end) game.on('boss_kill', function(p, mob) note('boss_kill ' .. mob.id) end)");
    let t = Instant::now();
    zones.respawn(t, &world, &defs, &mut || 0);
    kill(&zones, 1, 1, &defs, t);
    assert_eq!(logged(&zones, 2), ["kill 70 true", "boss_kill 70"]);
}

#[test]
fn a_faction_kill_queues_pvp_kill_and_death_and_a_duels_death_only_death() {
    // A player kill: Alice (country 0) kills Bob (country 1) with Fire Thorns.
    let (zones, defs, _b_in, _a_ev) = thorns_ground(150);
    edit(&zones, 1, |p| p.occupant.shape.name = b"Alice".to_vec());
    edit(&zones, 2, |p| p.occupant.shape.name = b"Bob".to_vec());
    watch(
        &zones,
        "game.on('pvp_kill', function(killer, victim, last, near) note('pvp ' .. killer.name .. '>' .. victim.name .. ' last ' .. tostring(last and last.name) .. ' near ' .. #near) end)
         game.on('death', function(victim, by, killer, pvp) note('death ' .. victim.name .. ' ' .. by .. ' ' .. tostring(type(killer) == 'table' and killer.name or killer) .. ' ' .. tostring(pvp)) end)",
    );
    zones.keep_effects(Instant::now() + Duration::from_secs(3), &World::default(), &defs, no_loot(), &mut || 0);
    assert_eq!(logged(&zones, 2), ["pvp Alice>Bob last Alice near 1", "death Bob player Alice true"]);

    // A duel: the loser's death is a death and nothing else.
    let (zones, mut inboxes, _events) = duel_ground();
    edit(&zones, 2, |p| p.occupant.shape.name = b"Bob".to_vec());
    edit(&zones, 3, |p| p.occupant.shape.name = b"Carol".to_vec());
    duel_fight(&zones, &mut inboxes);
    watch(&zones, "game.on('pvp_kill', function() note('pvp') end) game.on('death', function(victim, by, killer, pvp) note('death ' .. victim.name .. ' ' .. tostring(pvp)) end)");
    let (_, _, defs) = fighting_ground();
    assert!(matches!(strike(&zones, &defs, 2, &blows(20, 100), 3, false), AttackOutcome::Struck { killed: true, .. }));
    assert_eq!(logged(&zones, 1), ["death Carol false"]);
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(logged(&zones, 2), ["death Carol false"], "no pvp_kill");
}

#[test]
fn spawned_monsters_are_capped_tagged_expire_quietly_and_give_what_they_were_told_to() {
    use crate::mobs::{MobDef, MobDefs, SpawnArea};
    let mut world = World::build([(1, 2048)], &std::collections::HashSet::new(), []);
    world.set_mob_data(
        [(1, vec![0u8; 2048 * 2048 / 8])].into(),
        [(1, vec![SpawnArea::new([100.0, 0.0, 105.0, 100.0, 0.0, 105.0], vec![(7, 1)])])].into(),
    );
    let defs = MobDefs::new([MobDef { id: 7, level: 1, hp: 30, size: 1, country: 2, money: 90, ..Default::default() }]);
    let zones = Zones::new(&world, &defs);
    let (a, mut a_in, mut a_ev) = member(1, 100.0, 100.0, 0, b"Alice", 10);
    zones.enter(1, a);
    drain(&mut a_in);
    watch(&zones, "game.on('kill', function(p, mob) note('kill ' .. tostring(mob.spawned)) end)");
    let now = Instant::now();
    let life = Duration::from_secs(60);
    let spawn = |count, gives| zones.ext_spawn("w", &world, &defs, 7, count, 1, [100.0, 0.0, 100.0], 10, life, gives, now, &mut || 5);
    // 50 a call at most, 300 alive.
    let first = spawn(80, (true, true));
    assert_eq!(first.len(), 50, "50 a call");
    for _ in 0..5 {
        assert_eq!(spawn(50, (true, true)).len(), 50);
    }
    assert_eq!(zones.ext_spawn_count("w"), 300);
    assert!(spawn(1, (true, true)).is_empty(), "300 alive");
    assert_eq!(zones.ext_alive("w", &first), 50);
    assert_eq!(zones.ext_alive("other", &first), 0, "its own only");
    assert_eq!(zones.ext_despawn("other", &first, now), 0);
    // Despawning is quiet: 0x0602 at the next body pass, nothing gained.
    assert_eq!(zones.ext_despawn("w", &first[..10], now), 10);
    assert_eq!(zones.ext_alive("w", &first), 40);
    drain(&mut a_in);
    assert_eq!(zones.remove_bodies(now, &defs, &mut || 0), 10);
    assert!(drain(&mut a_in).iter().all(|(op, _)| *op == 0x0602));
    assert_eq!(zones.ext_spawn_count("w"), 290);
    assert!(a_ev.try_recv().is_err(), "no experience, no gold");
    // Their lifetime: removed quietly when it ends.
    assert_eq!(zones.expire_ext_spawns(now + Duration::from_secs(59)), 0);
    assert_eq!(zones.expire_ext_spawns(now + Duration::from_secs(60)), 290);
    assert_eq!(zones.remove_bodies(now + Duration::from_secs(61), &defs, &mut || 0), 290);
    assert_eq!(zones.ext_spawn_count("w"), 0);
    assert!(a_ev.try_recv().is_err());
    // A kill of a spawned monster: the script's `kill` names the extension; no experience with exp = false, no gold with drops = false.
    let quiet = spawn(1, (false, false));
    let fighter = combat::player(10, [100, 10, 10, 10, 10, 0], &combat::Gear { kind: Some(combat::MELEE), ..Default::default() });
    let attack = Attack { attacker: 1, country: 0, fighter: &fighter, kind: combat::MELEE, slack: 40.0, range: 0, rec: 0, cross_road: false };
    let AttackOutcome::Struck { killed, .. } = zones.attack_mob(1, quiet[0], &attack, &defs, no_loot(), now, &mut || 0) else { panic!("a hit") };
    assert!(killed);
    assert_eq!(logged(&zones, 1), ["kill w"]);
    assert!(a_ev.try_recv().is_err(), "exp = false: no Kill event for the experience");
    assert!(zones.ground.guard().get(&1).is_none_or(|g| g.items.is_empty()));
    // The default gives both.
    let normal = spawn(1, (true, true));
    let AttackOutcome::Struck { killed, .. } = zones.attack_mob(1, normal[0], &attack, &defs, no_loot(), now, &mut || 0) else { panic!("a hit") };
    assert!(killed);
    assert!(matches!(a_ev.try_recv().ok(), Some(ZoneEvent::Kill { mob_id: 7, .. })));
}
