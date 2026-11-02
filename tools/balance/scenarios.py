#!/usr/bin/env python3
"""Sample runs of the model for docs/plans/balance/1-formulas.md: every class of both factions at levels 15, 30 and 60 (ultimate
mode, the default build of model.py: free points into the damage stat, the rough gear picker, every learnable skill) against a common
monster of that level, and the mirrored class pairs against each other. Prints Markdown tables.

    python3 scenarios.py [--mode ultimate] [--without 180,174]
"""
import argparse

import data
import model

# The most placed monster with experience at each level (spawn counts from world.mob_area_mobs).
REFERENCE = {15: 2125, 30: 2183, 60: 1547}
PAIRS = [("fighter", "warrior"), ("defender", "guardian"), ("ranger", "assassin"), ("archer", "hunter"), ("mage", "pagan"),
         ("priest", "oracle")]


def fmt(x, digits=0):
    if x is None:
        return "-"
    if x == float("inf"):
        return "never"
    return f"{x:.{digits}f}"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--mode", default="ultimate")
    ap.add_argument("--without", default="")
    ap.add_argument("--levels", default="15,30,60")
    args = ap.parse_args()
    extra = f" without={args.without}" if args.without else ""
    for level in (int(x) for x in args.levels.split(",")):
        d = data.mobs()[REFERENCE[level]]
        print(f"\n#### Level {level} against monster {d.id} (level {d.level}, HP {d.hp}, defence {d.defense}, magic defence {d.magic}, "
              f"DEX {d.dex}, attack {d.attacks[0].damage} + up to {d.attacks[0].damage_add} every {d.attacks[0].time_ms} ms)\n")
        print("| class | HP | hit % | normal DPS | best skill | kill s (SP/MP pool) | kill s (unlimited) | monster DPS | dies in s "
              "| mitigation | effective HP |")
        print("|---|---|---|---|---|---|---|---|---|---|---|")
        for light, fury in PAIRS:
            for name in (light, fury):
                c = model.evaluate(model.parse_build(f"{name} {args.mode} {level}{extra}"))
                r = model.vs_mob(c, d)
                normal = next((a for a in r["actions"] if a["name"] == "normal attack"), None)
                best = max(r["actions"], key=lambda a: a["dps"]) if r["actions"] else None
                print(f"| {c.build.label} | {c.max_hp} | {r['hit_rate_normal']} | {fmt(normal['dps'] if normal else None)} | "
                      f"{best['name'].strip()} {fmt(best['dps'])}/s | {fmt(r['time_to_kill_s'], 1)} | {fmt(r['time_to_kill_unlimited_s'], 1)} | "
                      f"{fmt(r['mob_dps'])} | {fmt(r['time_to_die_s'], 1)} | {r['mitigation']:.2f} | {fmt(r['effective_hp'])} |")
        print(f"\n#### Level {level}, mirrored pairs (Light against Fury, SP/MP pool, at range)\n")
        print("| pair | Light kills in s | Fury kills in s | Light DPS | Fury DPS | share of HP taken at the first kill |")
        print("|---|---|---|---|---|---|")
        for light, fury in PAIRS:
            a = model.evaluate(model.parse_build(f"{light} {args.mode} {level}{extra}"))
            b = model.evaluate(model.parse_build(f"{fury} {args.mode} {level}{extra}"))
            r = model.vs_player(a, b)
            print(f"| {light} / {fury} | {fmt(r['a_hits_b']['time_to_kill_s'], 1)} | {fmt(r['b_hits_a']['time_to_kill_s'], 1)} | "
                  f"{fmt(r['a_hits_b']['dps'])} | {fmt(r['b_hits_a']['dps'])} | {r['share_at_first_kill'][0]} / {r['share_at_first_kill'][1]} |")


if __name__ == "__main__":
    main()
