"""The game definitions for the model, read from PostgreSQL (read-only) the way the server's loaders read them
(crates/game-server/src/pg.rs: `load_item_defs`, `load_skill_defs`, `load_mob_defs`, the status and experience tables,
`load_kill_status`), plus the client's own names (`client.items`, `client.skills`; English, unlike most of `defs`).

The queries run through psql as the other tools do (tools/dbconn.py). Each result is cached as
CSV under data/balance/cache/ (git-ignored); `refresh()` or `--refresh` on the command line reads the database again.
"""
from __future__ import annotations

import csv
import io
import subprocess
import sys
from functools import lru_cache
from pathlib import Path

import formulas as fm

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import dbconn  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]
CACHE = ROOT / "data" / "balance" / "cache"

QUERIES = {
    "items": """SELECT itemid, itemname, type, typeid, count, quality, buy, sell, reqlevel, country, attackfighter, defensefighter,
                       patrolrogue, shootrogue, attackmage, defensemage, grow, special, conststr, constdex, constrec, constint, constwis,
                       constluc, consthp, constmp, constsp, attacktime, speed, server, reqwis, effect1, effect2, effect3, effect4, range,
                       attrib, grade, drop, reqog, reqig, reqvg, reqluc, reqstr, reqdex, slot, exp
                  FROM defs.items ORDER BY type, typeid""",
    "skills": """SELECT * FROM defs.skills WHERE skilllevel BETWEEN 1 AND 3 OR skilllevel = 100 ORDER BY skillid, skilllevel""",
    "mobs": """SELECT * FROM defs.mobs ORDER BY mobid""",
    "status": """SELECT job, level, hp, sp, mp FROM defs.status_defs ORDER BY job, level""",
    "exp": """SELECT level, grow, exp FROM defs.exp_defs ORDER BY grow, level""",
    "kill_status": """SELECT seq, faction, kills, bonus_types, bonus_values FROM world.kill_status ORDER BY seq""",
    "item_enchant": """SELECT step, weapon_rate, armour_rate, weapon_add, armour_add FROM world.item_enchant ORDER BY step""",
    "client_items": """SELECT type, typeid, name FROM client.items""",
    "client_skills": """SELECT skill_id, level, name FROM client.skills""",
    "shop_items": """SELECT DISTINCT s.item_type, s.item_type_id, s.npc_type_id FROM world.npc_shop_items s
                       JOIN world.map_npcs m ON m.type = 1 AND m.type_id = s.npc_type_id""",
    "placed_mobs": """SELECT mob_id, 'area' AS how FROM world.mob_area_mobs
                      UNION SELECT unnest(mobs), 'boss' FROM world.boss_groups
                      UNION SELECT unnest(mobs), 'boss follower' FROM world.boss_followers
                      UNION SELECT unnest(ARRAY[mob_neutral, mob_light, mob_fury]), 'obelisk' FROM world.obelisks
                      UNION SELECT unnest(ARRAY[mob_neutral, mob_light, mob_fury]), 'obelisk guard' FROM world.obelisk_guards""",
    "mob_items": """SELECT mobid, itemorder, grade, droprate FROM defs.mob_items WHERE grade > 0 AND droprate > 0 AND itemorder > 0""",
    "item_create": """SELECT set_no, grade, weight FROM world.item_create""",
    "quest_items": """SELECT DISTINCT item_type, item_type_id FROM world.quest_results WHERE item_type > 0""",
    "start_items": """SELECT DISTINCT type, typeid FROM defs.base_items_defs""",
    "products": """SELECT productcode, buycost, unnest(ARRAY[itemid1, itemid2, itemid3, itemid4, itemid5, itemid6, itemid7, itemid8,
                          itemid9, itemid10, itemid11, itemid12, itemid13, itemid14, itemid15, itemid16, itemid17, itemid18, itemid19,
                          itemid20, itemid21, itemid22, itemid23, itemid24]) AS itemid FROM defs.product_list""",
    "spawns": """SELECT a.mob_id, sum(a.count) AS count, count(DISTINCT r.map) AS maps, string_agg(DISTINCT r.map::text, ' ') AS map_list
                   FROM world.mob_area_mobs a JOIN world.mob_areas r ON r.id = a.mob_area_id GROUP BY a.mob_id""",
    # Part 2 (content.py): where each thing is, with its map and side.
    "placements": """SELECT a.mob_id, r.map, a.count, 'area' AS how FROM world.mob_area_mobs a JOIN world.mob_areas r ON r.id = a.mob_area_id
                     UNION ALL SELECT unnest(mobs), map, 1, 'boss' FROM world.boss_groups
                     UNION ALL SELECT unnest(f.mobs), g.map, f.count, 'boss follower' FROM world.boss_followers f
                                      JOIN world.boss_groups g ON g.seq = f.group_seq
                     UNION ALL SELECT unnest(ARRAY[mob_neutral, mob_light, mob_fury]), map, 1, 'obelisk' FROM world.obelisks
                     UNION ALL SELECT unnest(ARRAY[g.mob_neutral, g.mob_light, g.mob_fury]), o.map, g.count, 'obelisk guard'
                                 FROM world.obelisk_guards g JOIN world.obelisks o ON o.seq = g.obelisk_seq""",
    "merchants": """SELECT DISTINCT s.npc_type_id, s.item_type, s.item_type_id, n.country, m.map FROM world.npc_shop_items s
                      JOIN world.map_npcs m ON m.type = 1 AND m.type_id = s.npc_type_id
                      LEFT JOIN world.npcs n ON n.type = 1 AND n.type_id = s.npc_type_id""",
    "placed_npcs": """SELECT DISTINCT type, type_id FROM world.map_npcs""",
    "mob_drops": """SELECT mobid, itemorder, grade, droprate FROM defs.mob_items
                      WHERE grade > 0 AND droprate > 0 AND itemorder BETWEEN 1 AND 9""",
    "sequence_groups": """SELECT group_no, sequence, array_to_string(grades, ' ') AS grades FROM world.sequence_groups ORDER BY group_no""",
    "quest_rewards": """SELECT r.quest_id, r.item_type, r.item_type_id, q.level_min, q.level_max, q.country, q.grow,
                               array_to_string(q.jobs, ' ') AS jobs, q.start_type, q.start_npc_type, q.start_npc_id,
                               q.start_item_type, q.start_item_id
                          FROM world.quest_results r JOIN world.quests q ON q.id = r.quest_id WHERE r.item_type > 0""",
    "product_items": """SELECT p.productcode, p.buycost, x.itemid, x.n FROM defs.product_list p,
                          unnest(ARRAY[itemid1, itemid2, itemid3, itemid4, itemid5, itemid6, itemid7, itemid8, itemid9, itemid10,
                                       itemid11, itemid12, itemid13, itemid14, itemid15, itemid16, itemid17, itemid18, itemid19,
                                       itemid20, itemid21, itemid22, itemid23, itemid24]) WITH ORDINALITY AS x(itemid, n)
                         WHERE x.itemid > 0""",
}

# Monsters that no spawn area, boss group or obelisk places but a Lua script makes (`docs/original/instances.md` section 8: the
# Cailum Sacra floor bosses summoned by 2471, the final boss's second form and its copies). The AI files' own summons (maps 45 to
# 47, `docs/original/scripted-mobs.md` section 9) are not read here.
SCRIPT_SUMMONED = {2464: 65, 2465: 65, 2466: 65, 2467: 65, 2470: 67, 2472: 67, 2473: 67}


def _query(name: str) -> list[dict]:
    sql = " ".join(QUERIES[name].split())
    out = subprocess.run(dbconn.psql_cmd() + ["-c", f"COPY ({sql}) TO STDOUT WITH (FORMAT csv, HEADER)"], capture_output=True, text=True)
    if out.returncode != 0:
        raise SystemExit(f"psql failed for {name}: {out.stderr.strip()}")
    return list(csv.DictReader(io.StringIO(out.stdout)))


def rows(name: str) -> list[dict]:
    """A query's rows, from the cache when it is there."""
    path = CACHE / f"{name}.csv"
    if path.exists():
        with path.open(newline="", encoding="utf-8") as f:
            return list(csv.DictReader(f))
    result = _query(name)
    CACHE.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="", encoding="utf-8") as f:
        if result:
            w = csv.DictWriter(f, fieldnames=list(result[0].keys()))
            w.writeheader()
            w.writerows(result)
    return result


def refresh() -> None:
    for name in QUERIES:
        path = CACHE / f"{name}.csv"
        if path.exists():
            path.unlink()
    for cached in (items, skills, mobs, status, exp_table, grades, item_names, skill_names, spawns, sources, placed, grade_items,
                   origins):
        cached.cache_clear()


def _i(v) -> int:
    return int(v) if v not in (None, "") else 0


@lru_cache(maxsize=None)
def item_names() -> dict:
    return {(_i(r["type"]), _i(r["typeid"])): r["name"] for r in rows("client_items")}


@lru_cache(maxsize=None)
def skill_names() -> dict:
    return {(_i(r["skill_id"]), _i(r["level"])): r["name"].strip() for r in rows("client_skills")}


@lru_cache(maxsize=None)
def items() -> fm.ItemDefs:
    """As `pg::load_item_defs`: stats and health below 0 count as 0; craftable when `server` ≠ 0 and `reqwis` is 1 to 99."""
    names = item_names()
    out = []
    for r in rows("items"):
        t, tid = _i(r["type"]), _i(r["typeid"])
        out.append(fm.ItemDef(
            item_type=t, type_id=tid, level=_i(r["reqlevel"]), country=_i(r["country"]),
            jobs=tuple(_i(r[c]) != 0 for c in ("attackfighter", "defensefighter", "patrolrogue", "shootrogue", "attackmage", "defensemage")),
            grow=_i(r["grow"]), special=_i(r["special"]),
            const_stats=tuple(max(_i(r[c]), 0) for c in ("conststr", "constdex", "constrec", "constint", "constwis", "constluc")),
            const_health=tuple(max(_i(r[c]), 0) for c in ("consthp", "constmp", "constsp")),
            attack_time=_i(r["attacktime"]), speed=_i(r["speed"]),
            craftable=_i(r["server"]) != 0 and 1 <= _i(r["reqwis"]) <= 99,
            effects=tuple(max(_i(r[c]), 0) for c in ("effect1", "effect2", "effect3", "effect4")),
            range=_i(r["range"]), attrib=_i(r["attrib"]), quality=_i(r["quality"]), absorption=max(0, min(_i(r["exp"]), 255)),
            item_id=_i(r["itemid"]), name=names.get((t, tid)) or r["itemname"], buy=_i(r["buy"]), grade=_i(r["grade"]),
            craft_max=max(_i(r["reqwis"]), 0), room=max(0, min(_i(r["slot"]), 255)),
            server=_i(r["server"]), req_str=_i(r["reqstr"]), reqvg=_i(r["reqvg"]), reqig=_i(r["reqig"]),
        ))
    return fm.ItemDefs(out, enchant())


def enchant() -> fm.EnchantTable:
    """As `pg::load_item_enchant`: `world.item_enchant` (the imported `Data/ItemEnchant.ini`), the built-in tables without rows."""
    return fm.EnchantTable.from_rows([tuple(_i(r[c]) for c in ("step", "weapon_rate", "armour_rate", "weapon_add", "armour_add"))
                                      for r in rows("item_enchant")])


@lru_cache(maxsize=None)
def skills() -> dict:
    """As `pg::load_skill_defs` and `SkillDefs::new`: `(id, level) -> SkillDef`, corrected."""
    names = skill_names()
    out = {}
    for r in rows("skills"):
        sid, level = _i(r["skillid"]), _i(r["skilllevel"])
        if sid == 0:
            continue
        keep = _i(r["keeptime"]) & 0xFFFF
        raw = fm.SkillDef(
            id=sid, level=level, country=_i(r["country"]),
            jobs=tuple(_i(r[c]) != 0 for c in ("attackfighter", "defensefighter", "patrolrogue", "shootrogue", "attackmage", "defensemage")),
            grow=_i(r["grow"]), points=_i(r["skillpoint"]), required_level=_i(r["reqlevel"]), type_show=_i(r["typeshow"]),
            type_effect=_i(r["typeeffect"]), kind=_i(r["typedetail"]), weapons=(_i(r["weapon1"]), _i(r["weapon2"])),
            weapon_value=_i(r["weaponvalue"]),
            abilities=tuple((_i(r[f"abilitytype{i}"]), _i(r[f"abilityvalue{i}"])) for i in (1, 2, 3)),
            prev_skill=_i(r["prevskillid"]), attack_kind=_i(r["typeattack"]),
            need_weapons=tuple(_i(r[f"needweapon{i}"]) != 0 for i in range(1, 16)), need_shield=_i(r["shield"]) != 0,
            need_sp=_i(r["sp"]), need_mp=_i(r["mp"]), ready_time=_i(r["readytime"]), reset_time=_i(r["resettime"]),
            attack_range=_i(r["attackrange"]), apply_range=_i(r["applyrange"]), state_type=_i(r["statetype"]), element=_i(r["attrtype"]),
            success_type=_i(r["successtype"]), success_value=_i(r["successvalue"]), target_type=_i(r["targettype"]),
            multi_attack=_i(r["multiattack"]), damage_type=_i(r["damagetype"]),
            damage=tuple(_i(r[c]) for c in ("damagehp", "damagesp", "damagemp")),
            add_damage=tuple(_i(r[c]) for c in ("adddamagehp", "adddamagesp", "adddamagemp")),
            heal=tuple(_i(r[c]) for c in ("healhp", "healsp", "healmp")), keep_time=keep,
            time_damage_type=_i(r["timedamagetype"]), time_damage=tuple(_i(r[c]) for c in ("timedamagehp", "timedamagesp", "timedamagemp")),
            time_heal=tuple(_i(r[c]) for c in ("timehealhp", "timehealsp", "timehealmp")), defense_type=_i(r["defencetype"]),
            defense_value=_i(r["defencevalue"]), limit_hp=_i(r["limithp"]), keep_on_death=_i(r["fixrange"]) == 1,
            name=names.get((sid, level)) or r["skillname"],
        )
        out[(sid, level)] = fm.corrected(raw)
    return out


@lru_cache(maxsize=None)
def mobs() -> dict:
    """As `pg::load_mob_defs`: `id -> MobDef` (all rows; `spawns()` tells which appear in the world)."""
    out = {}
    for r in rows("mobs"):
        def entry(n):
            return fm.MobAttack(skill=_i(r[f"attacktype{n}"]), ok=_i(r[f"attackok{n}"]), time_ms=max(_i(r[f"attacktime{n}"]), 0),
                                range=_i(r[f"attackrange{n}"]), damage=_i(r[f"attack{n}"]), damage_add=_i(r[f"attackplus{n}"]))
        a3 = _i(r["attackattrib3"])
        out[_i(r["mobid"])] = fm.MobDef(
            id=_i(r["mobid"]), level=_i(r["level"]), hp=_i(r["hp"]), sp=_i(r["sp"]), mp=_i(r["mp"]), exp=_i(r["exp"]), dex=_i(r["dex"]),
            luc=_i(r["luc"]), wis=_i(r["wis"]), defense=_i(r["defense"]), magic=_i(r["magic"]), attrib=_i(r["attrib"]), size=_i(r["size"]),
            defense_add=max(_i(r["attacktime3"]), 0), country={1: 0, 2: 1}.get(a3, 2),
            resist_states=tuple(_i(r[f"resiststate{i}"]) != 0 for i in range(1, 16)),
            resist_skills=tuple(max(_i(r[f"resistskill{i}"]), 0) for i in range(1, 7)),
            attacks=(entry(1), entry(2), entry(3)), recover=max(_i(r["attack3"]), 0), ai=_i(r["ai"]), name=r["mobname"],
        )
    return out


@lru_cache(maxsize=None)
def spawns() -> dict:
    """`mob id -> (count placed, maps)` from the spawn areas."""
    return {_i(r["mob_id"]): (_i(r["count"]), r["map_list"]) for r in rows("spawns")}


@lru_cache(maxsize=None)
def status() -> fm.StatusTable:
    return fm.StatusTable((_i(r["job"]), _i(r["level"]), _i(r["hp"]), _i(r["sp"]), _i(r["mp"])) for r in rows("status"))


@lru_cache(maxsize=None)
def exp_table() -> fm.ExpTable:
    return fm.ExpTable((_i(r["level"]), _i(r["grow"]), _i(r["exp"])) for r in rows("exp"))


@lru_cache(maxsize=None)
def grades() -> list:
    """Per faction (0 Light, 1 Fury), the grades 0 to n as (kills, bonus array of 28): each grade takes the larger of its own value and
    the grade below's (`kc.rs`, `docs/original/pvp-2.md` section 8)."""
    by_faction = {0: [(0, [0] * fm.KC_TYPES)], 1: [(0, [0] * fm.KC_TYPES)]}
    for r in rows("kill_status"):
        faction = _i(r["faction"])
        types = [int(x) for x in r["bonus_types"].strip("{}").split(",") if x]
        values = [int(x) for x in r["bonus_values"].strip("{}").split(",") if x]
        prev = by_faction[faction][-1][1]
        bonus = list(prev)
        own = [0] * fm.KC_TYPES
        for t, v in zip(types, values):
            if 0 < t < fm.KC_TYPES:
                own[t] = v
        bonus = [max(a, b) for a, b in zip(bonus, own)]
        by_faction[faction].append((_i(r["kills"]), bonus))
    return [by_faction[0], by_faction[1]]




# --- where items come from -----------------------------------------------------------------------------------------------------------

# Item types of real type 27 (quest and similar items): never in a drop grade (`drops::NO_DROP_REAL_TYPE`).
NO_GRADE_TYPES = (27, 28, 29)
# A merchant's `country` (`world.npcs`) as the side it serves. Inferred from what they sell: the 0 merchants sell Light items
# (country 2: 565 rows) and no Fury ones, the 1 merchants Fury items (country 5: 581 rows), the 2 merchants neither.
MERCHANT_SIDES = {0: "light", 1: "fury", 2: "both"}
# Drop entries 1 to 6 roll in percent, 7 to 9 in millionths (`drops::PERCENT_ENTRIES`).
PERCENT_ENTRIES = range(1, 7)


def side_of_country(country: int) -> str:
    """An item's, skill's or quest's `country` (a race rule, `byCountry`) as the side it allows: 0 human, 1 elf and 2 both are Light; 3
    deatheater, 4 vail and 5 both are Fury; 6 and above anyone."""
    return "light" if country <= 2 else "fury" if country <= 5 else "both"


@lru_cache(maxsize=None)
def placed() -> dict:
    """`mob id -> [(map, count, how)]`: spawn areas, boss groups and their followers, obelisks and their guards (`placements`), and the
    monsters the Lua scripts make (`SCRIPT_SUMMONED`)."""
    out: dict = {}
    for r in rows("placements"):
        if _i(r["mob_id"]):
            out.setdefault(_i(r["mob_id"]), []).append((_i(r["map"]), _i(r["count"]), r["how"]))
    for mob, m in SCRIPT_SUMMONED.items():
        out.setdefault(mob, []).append((m, 1, "summoned"))
    return out


@lru_cache(maxsize=None)
def grade_items() -> dict:
    """`grade -> [(type, type id, limited)]` as `DropTables::new`: grades 1 to 1024, type and type id not 0, not real type 27. A limited
    item (`drop` > 0) is in the list, so it counts in the pick, but a monster's entry never gives it."""
    out: dict = {}
    for r in rows("items"):
        t, tid, g = _i(r["type"]), _i(r["typeid"]), _i(r["grade"])
        if 1 <= g <= 1024 and t and tid and t not in NO_GRADE_TYPES:
            out.setdefault(g, []).append((t, tid, _i(r["drop"]) > 0))
    return out


@lru_cache(maxsize=None)
def origins() -> dict:
    """`(type, type id) -> [dict]`: every way a player gets the item, each with `kind` (shop, drop, sequence, box, quest, start, mall,
    mall box), `detail`, `side` (light, fury, both), `level` (the lowest level it needs, when one applies) and `chance` (per kill, per
    monster made or per box; 1 for a purchase, a start item or a quest).

    - shop: a merchant (NPC type 1) placed on a map, by its goods list; its side from `MERCHANT_SIDES`.
    - drop: a placed monster's drop entries (`drops::roll`): chance = the entry's rate × 1 / the grade's item count.
    - sequence: `world.sequence_groups` (`DropTables::sequence_item`): every `sequence`-th monster made, by the level tens of a placed
      monster with money (`money1` > 0); chance per monster made = 1 / sequence × 1 / the grade's item count.
    - box: an item of special 42 opens set `reqvg` of `world.item_create` (`DropTables::open_box`; grades without items count as
      nothing); a box counts when it has an origin itself (repeated until nothing changes); "mall box" when its only origins are the mall,
      "sequence box" when they are only the mall and sequence items.
    - quest: a reward of a quest that can be started: its starting NPC placed (start types 0 to 2) and, for types 2 to 4, its start
      item obtainable.
    - start: `defs.base_items_defs`. mall: `defs.product_list` (points come only from the operator: nothing in the game gives them)."""
    out: dict = {}

    def add(key, kind, detail, side="both", level=None, chance=1.0):
        out.setdefault(key, []).append({"kind": kind, "detail": detail, "side": side, "level": level, "chance": chance})

    # Merchants: one origin per merchant, with its maps.
    shops: dict = {}
    for r in rows("merchants"):
        shops.setdefault(((_i(r["item_type"]), _i(r["item_type_id"])), _i(r["npc_type_id"]), r["country"]), set()).add(_i(r["map"]))
    for (key, npc, country), maps in sorted(shops.items(), key=lambda kv: (kv[0][0], kv[0][1])):
        side = MERCHANT_SIDES.get(_i(country), "both") if country not in (None, "") else "both"
        add(key, "shop", f"merchant {npc} (map {' '.join(map(str, sorted(maps)))})", side)
    # Drops of placed monsters.
    grades = grade_items()
    where = placed()
    defs_mobs = {_i(r["mobid"]): r for r in rows("mobs")}
    per: dict = {}
    for r in rows("mob_drops"):
        mob = _i(r["mobid"])
        if mob not in where:
            continue
        order, grade, rate = _i(r["itemorder"]), _i(r["grade"]), _i(r["droprate"])
        listed = grades.get(grade, [])
        if not listed:
            continue
        p = (min(rate, 100) / 100.0 if order in PERCENT_ENTRIES else min(rate, 1_000_000) / 1_000_000.0) / len(listed)
        for t, tid, limited in listed:
            if not limited:
                per[((t, tid), mob)] = per.get(((t, tid), mob), 0.0) + p
    for (key, mob), p in sorted(per.items()):
        m = defs_mobs.get(mob, {})
        how = sorted({h for _, _, h in where[mob]})
        maps = sorted({mp for mp, _, _ in where[mob]})
        add(key, "drop", f"mob {mob} level {_i(m.get('level'))} ({', '.join(how)}; map {' '.join(map(str, maps))})", "both",
            _i(m.get("level")), p)
    # Sequence items: by the level tens of placed monsters with money.
    tens = {_i(defs_mobs[m]["level"]) // 10 for m in where if m in defs_mobs and _i(defs_mobs[m]["money1"]) > 0
            and 1 <= _i(defs_mobs[m]["level"]) <= 99}
    for r in rows("sequence_groups"):
        seq = _i(r["sequence"])
        if seq == 0:
            break
        bands: dict = {}
        for band, g in enumerate(int(x) for x in r["grades"].split()):
            if g and band in tens and grades.get(g):
                bands.setdefault(g, []).append(band)
        for g, used in bands.items():
            listed = grades[g]
            for t, tid, limited in listed:
                if not limited:
                    add((t, tid), "sequence", f"group {r['group_no']}: every {seq:,}th monster made, monster levels "
                        f"{max(min(used) * 10, 1)}-{max(used) * 10 + 9} (grade {g})", "both", None, 1.0 / seq / len(listed))
    # Starting items.
    for r in rows("start_items"):
        add((_i(r["type"]), _i(r["typeid"])), "start", "starting item", "both", 1)
    # The mall.
    by_id = {_i(r["itemid"]): (_i(r["type"]), _i(r["typeid"])) for r in rows("items")}
    for r in rows("product_items"):
        key = by_id.get(_i(r["itemid"]))
        if key:
            add(key, "mall", f"product {r['productcode']} ({r['buycost']} points)")
    # Boxes, until nothing new comes.
    sets: dict = {}
    for r in rows("item_create"):
        g, w = _i(r["grade"]), _i(r["weight"])
        if 1 <= _i(r["set_no"]) <= 255 and 0 <= g <= 1024 and w > 0:
            g = g if grades.get(g) else 0
            lines = sets.setdefault(_i(r["set_no"]), {})
            lines[g] = lines.get(g, 0) + w
    boxes = {(_i(r["type"]), _i(r["typeid"])): _i(r["reqvg"]) for r in rows("items") if _i(r["special"]) == 42}
    # Quests (`QuestStartRequest`, `docs/original/quest.md` section 5): start types 0 and 1 need the starting NPC placed (type 0 with
    # NPC type 0: any NPC), type 2 that NPC and the start item, types 3 and 4 the start item only. A start item counts when it is
    # obtainable (not only the mall or sequence items); start items come from quests and boxes too, so quests and boxes are taken
    # together until nothing changes.
    npcs = {(_i(r["type"]), _i(r["type_id"])) for r in rows("placed_npcs")}
    unplanned = {"mall", "mall box", "sequence", "sequence box"}

    def quest_open(r) -> bool:
        st = _i(r["start_type"])
        npc = (_i(r["start_npc_type"]), _i(r["start_npc_id"])) in npcs or (_i(r["start_npc_type"]) == 0 and st == 0)
        item = any(o["kind"] not in unplanned for o in out.get((_i(r["start_item_type"]), _i(r["start_item_id"])), []))
        return npc if st <= 1 else (npc and item) if st == 2 else item

    quests_done, done = set(), set()
    while True:
        opened = [r for r in rows("quest_rewards") if (r["quest_id"], r["item_type"], r["item_type_id"]) not in quests_done and quest_open(r)]
        for r in opened:
            quests_done.add((r["quest_id"], r["item_type"], r["item_type_id"]))
            add((_i(r["item_type"]), _i(r["item_type_id"])), "quest",
                f"quest {r['quest_id']} (levels {r['level_min']}-{r['level_max']}, mode {r['grow']}+)",
                side_of_country(_i(r["country"])), _i(r["level_min"]))
        new = [(k, n) for k, n in boxes.items() if k in out and k not in done and n in sets]
        if not new and not opened:
            break
        for key, n in new:
            done.add(key)
            kinds = {o["kind"] for o in out[key]}
            kind = ("mall box" if kinds <= {"mall", "mall box"} else
                    "sequence box" if kinds <= {"mall", "mall box", "sequence", "sequence box"} else "box")
            total = sum(sets[n].values())
            for g, w in sets[n].items():
                if g == 0:
                    continue
                listed = grades[g]
                for t, tid, _ in listed:
                    add((t, tid), kind, f"box {key[0]}/{key[1]} (set {n}, grade {g})", "both", None,
                        w / total / len(listed))
    return out


@lru_cache(maxsize=None)
def sources() -> dict:
    """`(type, type id) -> set of kinds` of `origins()` (shop, drop, sequence, box, sequence box, quest, start, mall, mall box)."""
    return {key: {o["kind"] for o in found} for key, found in origins().items()}
