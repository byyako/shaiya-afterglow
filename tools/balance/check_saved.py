#!/usr/bin/env python3
"""End-to-end check of the model against the server: for every character in the database, the maxima the model computes from its
saved level, stats, equipment, skills, applied skills and its faction's kill-count grade, next to the HP, MP and SP the server saved.
A character saved at full health shows the server's own maxima, so those rows must agree exactly; a lower saved value is a character
that was hurt or had spent SP/MP when it was saved. Read-only.

    python3 check_saved.py
"""
import data
import formulas as fm
import model

QUERY = {
    "characters": "SELECT id, name, family, grow, job, level, str, dex, rec, int, wis, luc, hp, mp, sp FROM characters WHERE deleted_at IS NULL ORDER BY id",
    "char_items": "SELECT character_id, bag, slot, type, type_id, quality, encode(gems, 'hex') AS gems, craft_name FROM character_items WHERE bag = 0",
    "char_skills": "SELECT character_id, skill_id, level FROM character_skills",
    "char_applied": "SELECT character_id, skill_id, level FROM character_applied_skills",
    "kill_counts": "SELECT country, count FROM faction_kill_counts",
}


def fresh(name):
    """These tables change with play: never from the cache."""
    data.QUERIES[name] = QUERY[name]
    path = data.CACHE / f"{name}.csv"
    if path.exists():
        path.unlink()
    return data.rows(name)


def main():
    chars = fresh("characters")
    items, skills, applied = {}, {}, {}
    for r in fresh("char_items"):
        gems = tuple(bytes.fromhex(r["gems"])) if r["gems"] else ()
        craft = r["craft_name"].encode() if r["craft_name"] else b""
        items.setdefault(int(r["character_id"]), []).append(
            fm.ItemUnit(int(r["type"]), int(r["type_id"]), int(r["slot"]), 0, int(r["quality"]), (gems + (0,) * 6)[:6], craft))
    for r in fresh("char_skills"):
        skills.setdefault(int(r["character_id"]), []).append((int(r["skill_id"]), int(r["level"])))
    for r in fresh("char_applied"):
        applied.setdefault(int(r["character_id"]), []).append((int(r["skill_id"]), int(r["level"])))
    counts = {int(r["country"]): int(r["count"]) for r in fresh("kill_counts")}
    agree = full = 0
    print(f"{'id':>4} {'name':<14} {'build':<28} {'saved HP/MP/SP':<20} {'model HP/MP/SP':<20} note")
    for c in chars:
        cid, family = int(c["id"]), int(c["family"])
        faction = model.faction_of(family)
        grades = data.grades()[faction]
        grade = max(i for i, (kills, _) in enumerate(grades) if kills <= counts.get(faction, 0))
        b = model.Build(int(c["job"]), family, int(c["grow"]), int(c["level"]),
                        [int(c[k]) for k in ("str", "dex", "rec", "int", "wis", "luc")], items.get(cid, []), skills.get(cid, []),
                        applied.get(cid, []), grade)
        ch = model.evaluate(b)
        mx = ch.totals.max_health
        saved = [int(c["hp"]), int(c["mp"]), int(c["sp"])]
        same = saved == mx
        within = all(s <= m for s, m in zip(saved, mx))
        full += same
        agree += within
        note = "equal (saved at full)" if same else "saved below the maxima" if within else "SAVED ABOVE THE MODEL"
        label = f"{model.class_name(b.job, family)} {model.MODES[b.grow]} {b.level}"
        print(f"{cid:>4} {c['name']:<14} {label:<28} {'/'.join(map(str, saved)):<20} {'/'.join(map(str, mx)):<20} {note}")
    print(f"\n{len(chars)} characters: {full} equal, {agree} at or below the model's maxima")


if __name__ == "__main__":
    main()
