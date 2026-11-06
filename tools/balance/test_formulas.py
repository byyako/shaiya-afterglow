"""The Rust unit tests replayed against the port (formulas.py), with the same inputs and roll sequences, and the expected-value layer
(expect.py) checked against sampling the exact functions. Run: `python3 -m unittest test_formulas` in tools/balance (no database
needed). Each test names the Rust test it copies."""
import random
import unittest

import expect as ex
import formulas as fm
from formulas import (DEX, HP, MAGIC, MELEE, MP, RANGED, RESULT_CRITICAL, RESULT_HIT, RESULT_MISS, SP, Base, Fighter, Gear, ItemDef,
                      ItemDefs, ItemUnit, MobDef, Numbers, Passives, SkillDef, StatusTable)


def rolls(*values):
    it = iter(values)
    return lambda: next(it)


def table():
    return StatusTable([(0, 1, 205, 125, 50), (4, 1, 150, 60, 170), (0, 60, 5000, 3000, 1000)])


FIGHTER = Base(job=0, level=1, grow=1, family=0, stats=(14, 12, 9, 8, 7, 15))


class Stats(unittest.TestCase):
    """stats.rs tests."""

    def test_maxima_follow_class_level_mode_race_and_stats(self):
        t = fm.compute(table(), FIGHTER, [], ItemDefs(), Passives())
        self.assertEqual(t.max_health, [250, 85, 195])
        self.assertEqual(t.speeds(), (0, 2))
        hard = fm.compute(table(), Base(**{**FIGHTER.__dict__, "grow": 2}), [], ItemDefs(), Passives())
        self.assertEqual(hard.max_health[HP], 225 + 45)
        mage = Base(job=4, level=1, grow=3, family=3, stats=(10,) * 6)
        self.assertEqual(fm.compute(table(), mage, [], ItemDefs(), Passives()).max_health, [225 + 10 + 50, 255 + 50, 90 + 50])
        far = Base(**{**FIGHTER.__dict__, "level": 61})
        self.assertEqual(fm.compute(table(), far, [], ItemDefs(), Passives()).max_health[HP], 5000 + 45)

    def test_the_faction_grade_adds_after_the_equipment(self):
        plain = fm.compute(table(), FIGHTER, [], ItemDefs(), Passives())
        grade = [0] * fm.KC_TYPES
        grade[1], grade[5], grade[8], grade[10], grade[11], grade[13], grade[9] = 10, 2, 3, 5, 5, 1, 10
        t = fm.compute(table(), Base(**{**FIGHTER.__dict__, "grade": tuple(grade)}), [], ItemDefs(), Passives())
        hp, mp, sp = plain.max_health
        self.assertEqual(hp, 250)
        hp_after = hp + 24
        self.assertEqual(t.max_health, [hp_after + 10, mp, sp + 15])
        self.assertEqual(t.stats, [14, 15, 11, 8, 7, 25])
        self.assertEqual(t.grade_recovery, [fm.percent(hp_after, 5), 0, fm.percent(sp, 5)])
        self.assertEqual(t.grade_fight_recovery, [2, 0, 0])

    def test_equipment_adds_its_bonuses(self):
        defs = ItemDefs([
            ItemDef(16, 1, const_stats=(0, 0, 2, 0, 0, 0), const_health=(30, 0, 0), craftable=True),
            ItemDef(1, 1, attack_time=4),
            ItemDef(22, 1, speed=1),
            ItemDef(42, 1, const_health=(999, 0, 0)),
        ])
        helmet = ItemUnit(16, 1, 0, craft_name=b"00030000000001000200")
        items = [helmet, ItemUnit(1, 1, 5), ItemUnit(22, 1, 9), ItemUnit(42, 1, 13), ItemUnit(16, 1, 0, bag=1)]
        t = fm.compute(table(), FIGHTER, items, defs, Passives())
        self.assertEqual(t.stats, [14, 15, 11, 8, 7, 15])
        self.assertEqual(t.max_health, [390, 85, 410])
        self.assertEqual(t.speeds(), (4, 3))

    def test_lapis_add_stats_health_and_speeds(self):
        defs = ItemDefs([ItemDef(16, 1), ItemDef(24, 1),
                         ItemDef(30, 9, const_stats=(3, 0, 2, 0, 0, 0), const_health=(50, 0, 0), attack_time=1)])
        tbl = StatusTable([(0, 1, 205, 125, 50)])
        without = fm.compute(tbl, FIGHTER, [], defs, Passives())
        gems = (9, 0, 0, 0, 0, 0)
        with_ = fm.compute(tbl, FIGHTER, [ItemUnit(16, 1, 0, gems=gems), ItemUnit(24, 1, 7, gems=gems)], defs, Passives())
        self.assertEqual(with_.stats[0] - without.stats[0], 3)
        self.assertEqual(with_.max_health[HP] - without.max_health[HP], 50 + 2 * 5)
        self.assertEqual(with_.attack_speed - without.attack_speed, 1)

    def test_passive_skills_add_after_the_equipment(self):
        defs = ItemDefs([ItemDef(1, 1, attack_time=4)])
        p = Passives()
        p.weapon_speed[1] = 1
        p.weapon_speed[2] = 3
        p.bonuses.add(5, 10, 0)
        p.bonuses.add(2, 20, 0)
        p.bonuses.add(17, 1, 0)
        t = fm.compute(table(), FIGHTER, [ItemUnit(1, 1, 5)], defs, p)
        self.assertEqual(t.stats[2], 19)
        self.assertEqual(t.max_health, [250 + 50, 85 + 20, 195])
        self.assertEqual(t.speeds(), (5, 3))
        self.assertEqual(fm.compute(table(), FIGHTER, [], defs, p).speeds(), (0, 3))

    def test_craft_names_are_ten_pairs(self):
        self.assertEqual(fm.craft_values(b"01020304050607080910"), [1, 2, 3, 4, 5, 6, 7, 8, 9])
        self.assertEqual(fm.craft_values(b"0102030405060708x910"), [1, 2, 3, 4, 5, 6, 7, 8, 0])
        self.assertIsNone(fm.craft_values(b"0102"))


def mob(level):
    return MobDef(id=1, level=level, hp=100, dex=10, luc=10, defense=10, exp=20)


class Combat(unittest.TestCase):
    """combat.rs tests."""

    def test_attack_waits(self):
        self.assertEqual(fm.attack_interval_ms(0), 4000)
        self.assertEqual(fm.attack_interval_ms(9), 1000)
        self.assertEqual(fm.attack_interval_ms(200), 1000)

    def test_numbers_follow_set_attack(self):
        g = Gear(attack=30, defence=5, magic_defence=3, kind=MELEE)
        p = fm.player(10, [20, 10, 8, 4, 6, 16], g)
        self.assertEqual(p.kinds[MELEE], Numbers(15, 17, 57, 13))
        self.assertEqual(p.kinds[RANGED].attack, 56)
        self.assertEqual(p.kinds[MAGIC], Numbers(0, 0, 36, 9))
        self.assertEqual(p.critical, 3)
        m = fm.monster(MobDef(**{**mob(8).__dict__, "defense_add": 4, "wis": 7, "magic": 2}))
        self.assertEqual(m.kinds[MELEE], Numbers(13, 10, 0, 10))
        self.assertEqual(m.kinds[RANGED], Numbers(13, 14, 0, 14))
        self.assertEqual(m.kinds[MAGIC], Numbers(0, 0, 15, 2))

    def test_worn_equipment_gives_less(self):
        defs = ItemDefs([ItemDef(13, 1, quality=100, effects=(40, 10, 0, 0), range=20, attrib=3),
                         ItemDef(16, 1, quality=100, effects=(0, 0, 9, 4))])
        g = fm.gear([ItemUnit(13, 1, 5, quality=51), ItemUnit(16, 1, 0, quality=50)], defs, Passives())
        self.assertEqual((g.attack, g.attack_var, g.defence, g.magic_defence, g.range, g.element, g.kind), (40, 10, 4, 2, 20, 3, RANGED))
        g = fm.gear([ItemUnit(13, 1, 5, quality=10)], defs, Passives())
        self.assertEqual((g.attack, g.attack_var), (0, 0))
        self.assertIsNone(fm.gear([], defs, Passives()).kind)

    def test_lapis_and_enchant_steps_add_to_the_gear(self):
        defs = ItemDefs([
            ItemDef(13, 1, quality=100, effects=(40, 0, 0, 0), craftable=True),
            ItemDef(17, 1, quality=100, effects=(0, 0, 9, 0), craftable=True),
            ItemDef(24, 1),
            ItemDef(30, 1, effects=(5, 0, 0, 0), range=2, attrib=1),
            ItemDef(30, 2, attrib=2),
            ItemDef(30, 3, absorption=4, special=7, effects=(0, 0, 3, 0)),
        ])
        bow = ItemUnit(13, 1, 5, quality=100, gems=(1, 0, 0, 0, 0, 0), craft_name=b"00000000000000000003")
        armour = ItemUnit(17, 1, 1, quality=100, gems=(3, 0, 0, 0, 0, 0), craft_name=b"00000000000000000052")
        cloak = ItemUnit(24, 1, 7, quality=100, gems=(1, 1, 3, 0, 0, 0))
        g = fm.gear([bow, armour, cloak], defs, Passives())
        self.assertEqual((g.attack, g.range, g.element), (40 + 5 + 21, 2, 1))
        self.assertEqual((g.defence, g.absorption, g.state_blocks), (12, 4 + 10, 1 << 7))
        mixed = ItemUnit(13, 1, 5, quality=100, gems=(1, 2, 0, 0, 0, 0))
        self.assertEqual(fm.gear([mixed], defs, Passives()).element, 0)
        self.assertEqual(fm.absorb(30, 14), (16, 14))
        self.assertEqual(fm.absorb(10, 14), (1, 9))
        self.assertEqual(fm.absorb(0, 14), (0, 0))

    def test_passive_skills_add_to_the_gear_and_numbers(self):
        defs = ItemDefs([ItemDef(1, 1, quality=100, effects=(40, 0, 0, 0)), ItemDef(19, 1, quality=100, effects=(0, 0, 6, 0))])
        p = Passives()
        p.weapon_damage[1] = 12
        p.weapon_damage[2] = 50
        p.shield = 7
        for ability, value in ((15, 2), (18, 4), (20, 3), (23, 30), (26, 5), (29, 6), (25, 9)):
            p.bonuses.add(ability, value, 0)
        g = fm.gear([ItemUnit(1, 1, 5, quality=100), ItemUnit(19, 1, 6, quality=100)], defs, p)
        self.assertEqual((g.attack, g.defence, g.range), (40 + 12, 6 + 7, 2))
        self.assertEqual(fm.gear([ItemUnit(1, 1, 5, quality=100)], defs, p).defence, 0)
        g2 = Gear(**{**g.__dict__, "defence": 5, "magic_defence": 3, "kind": MELEE})
        pl = fm.player(10, [20, 10, 8, 4, 6, 16], g2)
        self.assertEqual(pl.kinds[MELEE], Numbers(18, 23, 26 + 30 + 52 + 2 - 1, 18))
        self.assertEqual(pl.kinds[RANGED], Numbers(15, 17, 78, 13))
        self.assertEqual(pl.kinds[MAGIC].attack, 36 + 22 + 9)
        self.assertEqual(pl.critical, 3 + 4)

    def test_hits_follow_levels_and_numbers(self):
        p = Fighter(level=10, kinds=[Numbers(15, 17, 58, 13) for _ in range(3)])
        m = fm.monster(mob(10))
        self.assertEqual(fm.hit_rate(p, m, MELEE), 90)
        self.assertEqual(fm.hit_rate(Fighter(), Fighter(), MELEE), 20)
        self.assertEqual(fm.strike(p, m, MELEE, False, rolls(90)), (RESULT_MISS, 0))

    def test_damage_follows_the_formula(self):
        p = Fighter(level=10, luc=16, kinds=[Numbers(99, 0, 100, 0) for _ in range(3)])
        m = Fighter(level=10, kinds=[Numbers(defence=20) for _ in range(3)])
        self.assertEqual(fm.strike(p, m, MELEE, False, rolls(0, 4, 50)), (RESULT_HIT, 79))
        self.assertEqual(fm.strike(p, m, MELEE, False, rolls(0, 4, 0, 1)), (RESULT_CRITICAL, 121))
        p.element, m.element = 1, 2
        self.assertEqual(fm.strike(p, m, RANGED, True, rolls(0, 4, 50))[1], 14)
        weak = Fighter(level=10, luc=16, element=1, kinds=[Numbers(hit=99, attack=1) for _ in range(3)])
        self.assertEqual(fm.strike(weak, m, MELEE, False, rolls(0, 50, 6, 6))[1], 7)

    def test_skills_hit_by_their_own_rules(self):
        p = Fighter(level=10, wis=10, kinds=[Numbers(15, 17, 58, 13) for _ in range(3)])
        d = MobDef(**{**mob(20).__dict__, "wis": 30})
        m = fm.monster(d)
        skill = SkillDef(id=9, attack_kind=2)
        self.assertEqual(fm.magic_resist_rate(p, m), 67)
        self.assertFalse(fm.skill_hits(p, m, d, skill, rolls(66)))
        self.assertTrue(fm.skill_hits(p, m, d, skill, rolls(67)))
        sure = SkillDef(id=9, attack_kind=2, success_type=1, success_value=30)
        self.assertTrue(fm.skill_hits(p, m, d, sure, rolls(29)))
        self.assertFalse(fm.skill_hits(p, m, d, sure, rolls(30)))
        melee = SkillDef(id=9, attack_kind=0)
        rate = fm.hit_rate(p, m, MELEE)
        self.assertTrue(fm.skill_hits(p, m, d, melee, rolls(rate - 1)))
        self.assertFalse(fm.skill_hits(p, m, d, melee, rolls(rate)))
        states = list(d.resist_states)
        states[2] = True
        d.resist_states = tuple(states)
        self.assertFalse(fm.skill_hits(p, m, d, SkillDef(id=9, attack_kind=2, success_type=1, success_value=30, state_type=3), rolls(0)))
        self.assertTrue(fm.skill_hits(p, m, d, SkillDef(id=9, attack_kind=2, success_type=1, success_value=30, state_type=4), rolls(0)))
        d.resist_skills = (0, 0, 0, 0, 0, 9)
        self.assertFalse(fm.skill_hits(p, m, d, sure, rolls(0)))

    def test_skill_damage_by_damage_type(self):
        p = Fighter(level=10, luc=16, kinds=[Numbers(99, 0, 100, 0) for _ in range(3)])
        d = MobDef(**{**mob(10).__dict__, "hp": 1000, "sp": 40})
        m = Fighter(level=10, kinds=[Numbers(defence=20) for _ in range(3)])

        def blow_on(target, skill, *r):
            return fm.skill_damage(p, target, d, [300, 40, 0], [100, 50, 30], skill, False, rolls(*r))

        def sk(damage_type, damage, **kw):
            return SkillDef(damage_type=damage_type, damage=damage, **kw)

        self.assertEqual(blow_on(m, sk(0, (7, 0, 0))), (RESULT_HIT, [7, 0, 0]))
        self.assertEqual(blow_on(m, sk(1, (50, 3, 0)), 4, 50), (RESULT_HIT, [128, 3, 0]))
        self.assertEqual(blow_on(m, sk(2, (2, 0, 0)), 4, 50), (RESULT_HIT, [158, 0, 0]))
        self.assertEqual(blow_on(m, sk(3, (2, 0, 1))), (RESULT_HIT, [200, 0, 30]))
        self.assertEqual(blow_on(m, sk(4, (10, 1, 0))), (RESULT_HIT, [100, 1, 0]))
        self.assertEqual(blow_on(m, sk(5, (50, 0, 0))), (RESULT_HIT, [150, 0, 0]))
        water = Fighter(level=10, element=2, kinds=[Numbers(defence=20) for _ in range(3)])
        self.assertEqual(blow_on(water, sk(1, (50, 0, 0), element=1), 4, 50)[1][0], 54)
        self.assertEqual(blow_on(water, sk(1, (50, 0, 0), element=1, kind=fm.NEUTRAL_ELEMENT), 4, 50)[1][0], 158)
        self.assertEqual(blow_on(m, sk(1, (0, 0, 0)), 4, 0, 1)[0], RESULT_CRITICAL)

    def test_monsters_hit_back(self):
        p = Fighter(level=10, luc=16, kinds=[Numbers(15, 17, 0, 13) for _ in range(3)])
        m = fm.monster(MobDef(**{**mob(12).__dict__, "dex": 10, "luc": 10}))
        self.assertEqual(fm.mob_hit_rate(m, p, MELEE), 81)
        self.assertEqual(fm.mob_hit_rate(Fighter(), Fighter(), MELEE), 5)
        self.assertEqual(fm.mob_strike(m, 30, 0, 1.0, p, 0, rolls(83)), (RESULT_MISS, 0))
        self.assertEqual(fm.mob_strike(m, 30, 10, 1.0, p, 0, rolls(0, 4, 2, 50)), (RESULT_HIT, 26))
        self.assertEqual(fm.mob_strike(m, 30, 10, 1.0, p, 5, rolls(0, 4, 2, 50))[1], 15)
        self.assertEqual(fm.mob_strike(m, 30, 10, 1.0, p, 0, rolls(0, 4, 2, 4, 2)), (RESULT_CRITICAL, 44))

    def test_experience_by_level_difference(self):
        d = mob(10)
        self.assertEqual(fm.mob_exp(d, 10), 24)
        self.assertEqual(fm.mob_exp(d, 1), 60)
        self.assertEqual(fm.mob_exp(mob(20), 10), 0)
        self.assertEqual(fm.mob_exp(d, 15), 3)
        self.assertEqual(fm.mob_exp(d, 16), 0)
        self.assertEqual(fm.kill_exp(d, 10), 28)
        self.assertEqual((fm.mob_exp(d, 10, 2.0), fm.mob_exp(d, 10, 0.0), fm.mob_exp(d, 10, 1.5)), (48, 0, 36))
        self.assertEqual(fm.mob_exp(d, 15, 2.0), 6)
        self.assertEqual(fm.kill_exp(d, 10, 2.0), 57)

    def test_levels_come_with_points_and_a_class_stat(self):
        t = fm.ExpTable([(1, 2, 100), (2, 2, 200), (3, 2, 500)])
        start = fm.Progress(1, 90, 0, 0, [10] * 6)
        self.assertIsNone(fm.add_exp(start, 0, 1, 0, t))
        p, levels = fm.add_exp(start, 0, 1, 5, t)
        self.assertEqual((p.level, p.exp, levels), (1, 95, 0))
        p, levels = fm.add_exp(start, 2, 1, 250, t)
        self.assertEqual((p.level, p.exp, levels, p.stat_point, p.skill_point), (3, 340, 2, 10, 6))
        self.assertEqual(p.stats, [10, 12, 10, 10, 10, 10])
        self.assertIsNone(fm.add_exp(fm.Progress(30, 90, 0, 0, [10] * 6), 0, 0, 1000, t))

    def test_monster_skills_roll_and_hurt(self):
        d = MobDef(level=10)
        m = fm.monster(d)
        pl = Fighter(level=10)
        sure = SkillDef(success_type=1, success_value=50)
        self.assertTrue(fm.mob_skill_hits(m, d, pl, sure, rolls(49)))
        self.assertFalse(fm.mob_skill_hits(m, d, pl, sure, rolls(50)))
        magic = SkillDef(attack_kind=MAGIC)
        self.assertFalse(fm.mob_skill_hits(m, d, pl, magic, rolls(0)))
        self.assertTrue(fm.mob_skill_hits(m, d, pl, magic, rolls(1)))
        m1, p1 = Fighter(level=1), Fighter(level=1)

        def blow(skill, roll):
            return fm.mob_skill_damage(m1, 50, 0, [500, 0, 0], p1, 0, [80, 40, 20], [200, 100, 50], skill, lambda: roll)

        def sk(damage_type, damage):
            return SkillDef(damage_type=damage_type, damage=damage)

        self.assertEqual(blow(sk(0, (7, 0, 3)), 0)[1], [7, 0, 3])
        self.assertEqual(blow(sk(4, (10, 0, 50)), 0)[1], [20, 0, 25])
        self.assertEqual(blow(sk(5, (50, 0, 0)), 0)[1], [40, 0, 0])
        self.assertEqual(blow(sk(3, (2, 0, 0)), 0)[1], [1000, 0, 0])
        self.assertEqual(blow(sk(1, (10, 4, 5)), 99), (RESULT_HIT, [60, 4, 5]))
        self.assertEqual(blow(sk(2, (2, 9, 9)), 0), (RESULT_CRITICAL, [150, 0, 0]))

    def test_magic_skills_on_players_are_cut_by_level_and_magic_defence(self):
        skill = SkillDef(damage_type=2, attack_kind=MAGIC)
        caster = Fighter(level=20, int_=50, item_attack=50, wis=10)
        caster.kinds[MAGIC].attack = 200
        target = Fighter(level=20, wis=10)
        target.kinds[MAGIC].defence = 100
        h = fm.PvpSkillHit(caster, target, 0, (100,) * 3, (100,) * 3, (100,) * 3, True, False)
        self.assertEqual(fm.pvp_skill_damage(h, skill, lambda: 99), (RESULT_HIT, [58, 0, 0]))
        self.assertFalse(fm.pvp_skill_hits(caster, target, skill, rolls(0)))
        self.assertTrue(fm.pvp_skill_hits(caster, target, skill, rolls(2)))
        pct = SkillDef(damage_type=4, damage=(10, 0, 0), success_type=1, success_value=100)
        self.assertEqual(fm.pvp_skill_damage(h, pct, lambda: 0)[1], [10, 0, 0])
        self.assertTrue(fm.pvp_skill_hits(caster, target, pct, rolls(99)))


class Others(unittest.TestCase):
    """lapis.rs, kc.rs, item_use.rs tests, and hand-worked PvP blows (docs/original/pvp.md section 4)."""

    def test_enchant_steps(self):
        self.assertEqual(fm.enchant_bonus(b"00000000000000000020"), (140, 0))
        self.assertEqual(fm.enchant_bonus(b"00000000000000000055"), (0, 25))
        self.assertEqual(fm.enchant_bonus(b"00000000000000000030"), (0, 0))

    def test_enchant_tables_from_the_file(self):
        """lapis.rs `enchant_tables_from_the_file`: a step's value is the table's total, the rates are the table's."""
        attack = [0, 7, 14, 21, 31, 41, 51, 64, 77, 90, 106, 122, 138, 157, 176, 195, 217, 239, 261, 286, 311]
        rates = [900000, 800000, 700000, 600000, 400000, 200000, 100000, 80000, 60000, 10000, 6000, 2000, 1000, 600] + [200] * 6 + [0]
        table = fm.EnchantTable.from_rows([(s, rates[s], 0, attack[s], 5 * s) for s in range(21)])
        self.assertEqual(fm.enchant_bonus(b"00000000000000000004", table), (31, 0))
        self.assertEqual(fm.enchant_bonus(b"00000000000000000063", table), (0, 65))
        self.assertEqual((table.rate(0, False), table.rate(14, False)), (900000, 200))
        self.assertEqual(fm.EnchantTable.from_rows([]).rates, fm.BUILT_IN.rates, "no rows: the built-in tables")
        with self.assertRaises(ValueError):
            fm.EnchantTable.from_rows([(s, 0, 0, 0, 0) for s in range(20)])
        sword = ItemDef(5, 1, quality=100, effects=(50, 0, 0, 0), craftable=True)
        worn = [ItemUnit(5, 1, 5, quality=100, craft_name=b"00000000000000000004")]
        self.assertEqual(fm.gear(worn, fm.ItemDefs([sword], table), fm.Passives()).attack, 50 + 31)
        self.assertEqual(fm.gear(worn, fm.ItemDefs([sword]), fm.Passives()).attack, 50 + 28, "built in: 7 a step")

    def test_percent(self):
        self.assertEqual(fm.percent(1000, 6), 59)
        self.assertEqual(fm.percent(1234, 5), 61)
        self.assertEqual(fm.percent(0, 50), 0)

    def test_resets(self):
        self.assertEqual(fm.reset_stats(0, 0, 1, 10), (45, [23, 12, 9, 8, 7, 15]))
        self.assertEqual(fm.reset_stats(1, 3, 2, 1), (0, [12, 9, 14, 10, 10, 10]))
        self.assertEqual(fm.reset_stats(5, 2, 3, 60), (531, [8, 9, 10, 14, 75, 8]))
        self.assertEqual(fm.reset_skill_points(1, 10), 32)
        self.assertEqual(fm.reset_skill_points(2, 10), 41)
        self.assertEqual(fm.reset_skill_points(3, 1), 5)

    def test_pvp_blow_by_hand(self):
        # Attack 300, defence 120, LUC 20: a hit (rate 96); d = 180, + trunc(R × 0.1) with R = 5 + 1 = 6: 180; no critical (roll 99).
        a = Fighter(level=60, luc=20, critical=20, kinds=[Numbers(100, 50, 300, 0) for _ in range(3)])
        t = Fighter(level=60, luc=10, kinds=[Numbers(50, 50, 0, 120) for _ in range(3)])
        self.assertEqual(fm.pvp_hit_rate(a, t, MELEE), 96, "80 − 50 + 66.7")
        self.assertEqual(fm.pvp_strike(a, t, MELEE, False, False, 0, rolls(0, 5, 99)), (RESULT_HIT, 180))
        # Critical: chance trunc(20 − 10 × 0.034) = 19; roll 3 hits; R2 = 10: trunc(180 × 0.5 + 10 × 1.4f + 180) = 283 (1.4f is a hair below 1.4).
        self.assertEqual(fm.pvp_strike(a, t, MELEE, False, False, 0, rolls(0, 5, 3, 9)), (RESULT_CRITICAL, 283))
        # Ranged within 3 without the passive: 180 × 0.7f = 125 (0.7f is a hair below 0.7).
        self.assertEqual(fm.pvp_strike(a, t, RANGED, True, False, 0, rolls(0, 5, 99)), (RESULT_HIT, 125))


class Expectations(unittest.TestCase):
    """expect.py against sampling the exact functions (within 1.5 % and 0.6 points)."""

    N = 40_000

    def sample(self, f):
        rng = random.Random(7)
        total = 0.0
        for _ in range(self.N):
            total += f(lambda: rng.randrange(32768))
        return total / self.N

    def close(self, model, sampled):
        self.assertLess(abs(model - sampled), max(0.015 * sampled, 0.6), f"model {model:.2f}, sampled {sampled:.2f}")

    def test_normal_attack_on_a_monster(self):
        p = Fighter(level=60, luc=40, critical=12, attack_var=30, kinds=[Numbers(120, 80, 650, 200) for _ in range(3)])
        for lvl in (58, 60, 61, 63):
            m = fm.monster(MobDef(level=lvl, hp=10000, dex=70, luc=40, defense=200))
            e = ex.pve_attack(p, m, MELEE)
            self.close(e.per_attempt, self.sample(lambda r: fm.strike(p, m, MELEE, False, r)[1]))

    def test_weak_attack_clamps(self):
        p = Fighter(level=30, luc=8, critical=1, attack_var=5, kinds=[Numbers(60, 40, 90, 50) for _ in range(3)])
        m = fm.monster(MobDef(level=33, hp=4000, dex=40, luc=30, defense=80))
        self.close(ex.pve_attack(p, m, MELEE).per_attempt, self.sample(lambda r: fm.strike(p, m, MELEE, False, r)[1]))

    def test_monster_attack_with_absorption(self):
        m = fm.monster(MobDef(level=61, hp=10000, dex=90, luc=60, defense=200))
        p = Fighter(level=60, luc=30, kinds=[Numbers(100, 90, 0, 260) for _ in range(3)])
        e = ex.mob_attack(m, 420, 120, p, 0, absorption=40)

        def one(r):
            res, d = fm.mob_strike(m, 420, 120, 1.0, p, 0, r)
            return fm.absorb(d, 40)[0] if res != RESULT_MISS else 0
        self.close(e.per_attempt, self.sample(one))

    def test_pvp_blow_and_magic(self):
        a = Fighter(level=60, luc=60, critical=15, attack_var=40, int_=200, item_attack=300,
                    kinds=[Numbers(140, 90, 700, 250), Numbers(140, 90, 640, 250), Numbers(0, 0, 900, 300)])
        t = Fighter(level=60, luc=20, wis=80, kinds=[Numbers(120, 110, 600, 380), Numbers(120, 110, 0, 380), Numbers(0, 0, 0, 260)])
        self.close(ex.pvp_attack(a, t, MELEE, absorption=30).per_attempt,
                   self.sample(lambda r: (lambda res: fm.absorb(res[1], 30)[0])(fm.pvp_strike(a, t, MELEE, False, False, 0, r))))
        skill = SkillDef(damage_type=1, damage=(150, 0, 0), attack_kind=MAGIC)
        h = fm.PvpSkillHit(a, t, 0, (1,) * 3, (1,) * 3, (1,) * 3)

        def one(r):
            if not fm.pvp_skill_hits(a, t, skill, r):
                return 0
            return fm.pvp_skill_damage(h, skill, r)[1][0]
        self.close(ex.pvp_skill(h, skill).per_attempt, self.sample(one))

    def test_pvp_blow_mean_is_exact(self):
        """A physical PvP blow's mean against every spread and luck value: the whole-number grid keeps it exact (fractional midpoints
        ran about 0.5 low)."""
        a = Fighter(level=60, luc=15, critical=2, attack_var=54, kinds=[Numbers(147, 149, 932, 165) for _ in range(3)])
        t = Fighter(level=60, luc=14, kinds=[Numbers(71, 72, 521, 614) for _ in range(3)])
        exact = [fm.pvp_strike(a, t, MELEE, False, False, 0, iter((0, u, r, 99)).__next__)[1] for u in range(54) for r in range(15)]
        self.assertAlmostEqual(ex.pvp_attack(a, t, MELEE).normal, sum(exact) / len(exact), delta=0.05)


if __name__ == "__main__":
    unittest.main()
