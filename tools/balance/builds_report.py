#!/usr/bin/env python3
"""The Markdown tables of docs/plans/balance/3-classes.md, from the CSVs builds.py writes (data/balance/builds*.csv).

    python3 builds_report.py [--section builds,pve,pvp,...]
"""
import argparse
import csv
import statistics
from collections import defaultdict

import data
import formulas as fm
import model

OUT = data.ROOT / "data" / "balance"
CLASSES = model.LIGHT + model.FURY
SHORT = {"fighter": "Fig", "defender": "Def", "ranger": "Ran", "archer": "Arc", "mage": "Mag", "priest": "Pri", "warrior": "War",
         "guardian": "Gua", "assassin": "Asn", "hunter": "Hun", "pagan": "Pag", "oracle": "Ora"}
WEAPON_KIND = {1: "1H sword", 2: "2H sword", 3: "1H axe", 4: "2H axe", 5: "double sword", 6: "spear", 7: "1H mace", 8: "2H hammer",
               9: "short blade", 10: "short sword", 11: "javelin", 12: "staff", 13: "bow", 14: "crossbow", 15: "claw"}
NEVER = 300


def load(name):
    with open(OUT / name, newline="") as f:
        return list(csv.DictReader(f))


def table(head, rows):
    print("| " + " | ".join(head) + " |")
    print("|" + "---|" * len(head))
    for r in rows:
        print("| " + " | ".join(str(x) for x in r) + " |")
    print()


def s(x):
    """Seconds as the document shows them."""
    if x in (None, ""):
        return "-"
    x = float(x)
    if x >= NEVER or x == float("inf"):
        return "never"
    return f"{x:.1f}" if x < 10 else f"{x:.0f}"


def n(x):
    return f"{int(round(float(x))):,}" if x not in (None, "") else "-"


def points(cls, mode, level, alloc):
    """The free points of each stat a split in quarters gives (`model.alloc_stats`)."""
    shares = {k: float(v) for k, v in (t.split(":") for t in alloc.split(","))}
    job = CLASSES.index(cls) % 6
    family = model.parse_build(f"{cls} {mode} {level} gear=none skills=none").family
    free, _ = fm.reset_stats(job, family, model.MODES.index(mode), level)
    total = sum(shares.values())
    out = {k: int(free * v / total) for k, v in shares.items()}
    out[max(shares, key=shares.get)] += free - sum(out.values())
    return " ".join(f"{k.upper()} {v}" for k, v in out.items())


# --- sections ------------------------------------------------------------------------------------------------------------------------


def s_builds():
    builds = {(r["class"], r["level"]): r for r in load("builds.csv") if r["mode"] == "ultimate"}
    pve = {(r["class"], r["level"]): r for r in load("builds_pve.csv") if r["gear"] == "expected" and r["party"] == "0"}
    plans = {(r["class"], r["level"]): r for r in load("builds_plans.csv") if r["mode"] == "ultimate"}
    for level in ("15", "30", "60"):
        print(f"#### Level {level} (ultimate)\n")
        rows = []
        for cls in CLASSES:
            b, p = builds[(cls, level)], plans[(cls, level)]
            # The sheet as the PvE string has it (lasting buffs only); the PvP string adds the opening buffs.
            c = model.evaluate(model.parse_build(b["pve_spec"]))
            r = __import__("builds").sheet_row(c)
            kind = WEAPON_KIND.get(int(p["real"]), p["real"]) + (" + shield" if p["shield"] == "1" else "")
            rows.append([cls, f"{r['weapon'].strip()} ({kind})", points(cls, "ultimate", int(level), b["alloc"]), b["lapis"],
                         f"{r['skill_points']}/{r['skill_points_max']}", n(r["hp"]), f"{n(r['sp'])} / {n(r['mp'])}", r["attack"],
                         r["hit"], r["evasion"], r["defence"], r["magic_defence"], r["critical"], r["absorption"], r["interval_ms"]])
        table(["class", "weapon", "free points", "lapis first", "skill points", "HP", "SP / MP", "attack", "hit", "evasion", "defence",
               "magic def.", "crit %", "absorb", "interval ms"], rows)


def s_pve():
    rows_all = [r for r in load("builds_pve.csv")]
    for level, mobs in (("15", ["2125"]), ("30", ["2183"]), ("55", ["1501", "982"]), ("60", ["1501", "1547", "982"])):
        print(f"#### Level {level}: kill and death against monster{'s' if len(mobs) > 1 else ''} {', '.join(mobs)}\n")
        head = ["class"]
        for m in mobs:
            head += [f"{m}: kill s (pool / potions)", f"{m}: dies in s", f"{m}: exp / h"]
        if level not in ("60",):
            head += ["hours a level"]
        rows = []
        for cls in CLASSES:
            row = [cls]
            hours = ""
            for m in mobs:
                r = next(x for x in rows_all if x["class"] == cls and x["level"] == level and x["mob"] == m and x["gear"] == "expected"
                         and x["party"] == "0")
                row += [f"{s(r['kill_s'])} / {s(r['kill_unlimited_s'])}", s(r["die_s"]), n(r["exp_h"])]
                if m == mobs[0]:
                    hours = r["hours_per_level"] or "-"
            if level != "60":
                row.append(hours)
            rows.append(row)
        table(head, rows)


def s_pve_variants():
    rows_all = load("builds_pve.csv")
    print("#### Level 55 against 1501: gear and a party's buffs\n")
    rows = []
    for cls in CLASSES:
        row = [cls]
        for gear, party in (("plain", "0"), ("expected", "0"), ("good", "0"), ("expected", "1")):
            r = next(x for x in rows_all if x["class"] == cls and x["level"] == "55" and x["mob"] == "1501" and x["gear"] == gear
                     and x["party"] == party)
            row.append(f"{n(r['hp'])}: {s(r['kill_s'])} / {s(r['die_s'])}")
        rows.append(row)
    table(["class", "plain gear: HP: kill / die s", "expected gear", "good gear", "expected + party buffs"], rows)


def matrix(level, variant):
    rows = [r for r in load("builds_pvp.csv") if r["level"] == str(level) and r["variant"] == variant]
    return {(r["attacker"], r["target"]): float(r["kill_s"]) for r in rows}


def s_pvp():
    for level, variant, title in (("15", "all", "level 15"), ("30", "all", "level 30"), ("30", "no_percent_dot", "level 30 without the mage's damage over time"),
                                  ("60", "all", "level 60, every skill"), ("60", "no_percent", "level 60 without the percentage skills 180 to 184"),
                                  ("60", "no_percent_dot", "level 60 without the percentage skills and the mage's damage over time")):
        m = matrix(level, variant)
        if not m:
            continue
        print(f"#### PvP, {title}: seconds for the row to kill the column\n")
        rows = []
        for a in CLASSES:
            rows.append([f"**{SHORT[a]}**"] + [s(m[(a, b)]) for b in CLASSES])
        table(["kills →"] + [SHORT[b] for b in CLASSES], rows)


def summary(level, variant):
    m = matrix(level, variant)
    out = {}
    for a in CLASSES:
        dealt = [m[(a, b)] for b in CLASSES if b != a]
        taken = [m[(b, a)] for b in CLASSES if b != a]
        wins = sum(1 for b in CLASSES if b != a and m[(a, b)] < m[(b, a)])
        out[a] = (wins, statistics.median(dealt), statistics.median(taken))
    return out


def s_pvp_summary():
    for level, variants in (("15", ["all"]), ("30", ["all", "no_percent_dot"]),
                            ("60", ["all", "no_percent", "no_percent_dot", "debuffs", "plain", "good"])):
        sums = {v: summary(level, v) for v in variants if matrix(level, v)}
        print(f"#### PvP summary, level {level}: duels won of 11, median seconds to kill / to be killed\n")
        head = ["class"] + [{"all": "every skill", "no_percent": "without 180-184", "no_percent_dot": "without 180-184 and mage DoT",
                             "debuffs": "with opening debuffs", "plain": "plain gear", "good": "good gear"}[v] for v in sums]
        rows = []
        for a in CLASSES:
            rows.append([a] + [f"{sums[v][a][0]}: {s(sums[v][a][1])} / {s(sums[v][a][2])}" for v in sums])
        table(head, rows)


def s_factions():
    rows = load("builds_factions.csv")
    for level in ("15", "30", "60"):
        print(f"#### Level {level}: Light kills Fury / Fury kills Light, seconds\n")
        steps = list(dict.fromkeys(r["step"] for r in rows))
        out = []
        for pair in dict.fromkeys(r["pair"] for r in rows):
            row = [pair]
            for st in steps:
                r = next(x for x in rows if x["pair"] == pair and x["level"] == level and x["step"] == st)
                row.append(f"{s(r['light_kills_s'])} / {s(r['fury_kills_s'])}")
            out.append(row)
        table(["pair"] + steps, out)
    print("#### The real builds side by side (HP, attack, defence; Light / Fury)\n")
    out = []
    for r in rows:
        if r["step"] == "+ own builds (the real ones)":
            out.append([r["pair"], r["level"], f"{n(r['light_hp'])} / {n(r['fury_hp'])}", f"{r['light_attack']} / {r['fury_attack']}",
                        f"{r['light_defence']} / {r['fury_defence']}"])
    table(["pair", "level", "HP", "attack", "defence"], out)


def s_modes():
    rows = load("builds_modes.csv")
    for level in ("15", "30", "60"):
        print(f"#### Level {level}: the modes (normal / hard / ultimate)\n")
        out = []
        for cls in CLASSES:
            by = {r["mode"]: r for r in rows if r["class"] == cls and r["level"] == level}
            row = [cls]
            for key, f in (("hp", n), ("attack", str), ("skill_points_max", str)):
                row.append(" / ".join(f(by[m][key]) for m in ("normal", "hard", "ultimate")))
            row.append(" / ".join(f"{s(by[m]['kill_s'])}:{s(by[m]['die_s'])}" for m in ("normal", "hard", "ultimate")))
            row.append(" / ".join(str(by[m]["hours_per_level"] or "-") for m in ("normal", "hard", "ultimate")))
            row.append(" / ".join(f"{s(by[m]['kills_ultimate_median_s'])}:{s(by[m]['killed_by_ultimate_median_s'])}"
                                  for m in ("normal", "hard")))
            out.append(row)
        table(["class", "HP", "attack", "skill points", "field monster kill:die s", "hours a level",
               "normal, hard against the ultimate twelve (median kill:killed s)"], out)


def s_levers():
    wall = load("builds_lever_wall.csv")
    steps = list(dict.fromkeys((r["defence_x"], r["attack_x"]) for r in wall))
    print("#### The 51-58 wall: level-55 builds with plain gear against monster 1501, its defence and attack scaled (kill / die s; ✗ dies first)\n")
    out = []
    for cls in CLASSES:
        row = [cls]
        for d, a in steps:
            r = next(x for x in wall if x["class"] == cls and x["defence_x"] == d and x["attack_x"] == a)
            mark = "" if float(r["die_s"]) > float(r["kill_s"]) else " ✗"
            row.append(f"{s(r['kill_s'])} / {s(r['die_s'])}{mark}")
        out.append(row)
    table(["class"] + [f"defence × {d}, attack × {a}" for d, a in steps], out)
    sk = load("builds_lever_skills.csv")
    print("#### Skill levers: median and fastest seconds to kill the other eleven\n")
    out = []
    for r in sk:
        out.append([r["class"], r["level"], r["lever"], s(r["median_kill_s"]), s(r["fastest_s"])])
    table(["class", "level", "lever", "median", "fastest"], out)
    hp = load("builds_lever_hp.csv")
    print("#### HP from gear: the real build, with crafted HP/MP/SP at 50 a point, and with plain gear\n")
    out = [[r["class"], r["level"], n(r["hp"]), n(r["hp_craft_50"]), n(r["hp_plain_gear"])] for r in hp]
    table(["class", "level", "HP", "crafted at 50", "plain gear"], out)


def s_search():
    rows = [r for r in load("builds_search.csv") if r["round"] == "2"]
    chosen = {(r["class"], r["level"]): r for r in load("builds.csv") if r["mode"] == "ultimate"}
    for level in ("15", "30", "60"):
        print(f"#### Level {level}: the chosen split against the best for PvE alone, PvP alone and all into the damage stat\n")
        out = []
        for cls in CLASSES:
            mine = [r for r in rows if r["class"] == cls and r["level"] == level]
            best = max(mine, key=lambda r: float(r["score"]))
            pve = max(mine, key=lambda r: float(r["pve_rel"]))
            pvp = max(mine, key=lambda r: float(r["pvp_rel"]))
            dmg = "int:4" if CLASSES.index(cls) % 6 >= 4 else "str:4"
            pure = next((r for r in mine if r["alloc"] == dmg and r["lapis"] == "stat"), None)

            def cell(r):
                return f"{r['alloc']} {r['lapis']}: {float(r['pve_rel']):.2f} / {float(r['pvp_rel']):.2f}"
            out.append([cls, cell(best), cell(pve), cell(pvp), cell(pure) if pure else "-", chosen[(cls, level)]["round1"]])
        table(["class", "chosen (PvE / PvP, 1 = the best)", "best PvE", "best PvP", "all damage stat", "round 1 choice"], out)


def s_strings():
    for r in load("builds.csv"):
        if r["mode"] == "ultimate" and r["level"] in ("15", "30", "60"):
            print(f"- {r['class']} {r['level']}: `{r['pvp_spec']}`")
    print()


SECTIONS = {"builds": s_builds, "pve": s_pve, "pve_variants": s_pve_variants, "pvp": s_pvp, "pvp_summary": s_pvp_summary,
            "factions": s_factions, "modes": s_modes, "levers": s_levers, "search": s_search, "strings": s_strings}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--section", default=",".join(SECTIONS))
    args = ap.parse_args()
    for name in args.section.split(","):
        print(f"<!-- {name} -->\n")
        SECTIONS[name]()


if __name__ == "__main__":
    main()
