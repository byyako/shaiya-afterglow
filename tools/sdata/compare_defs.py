#!/usr/bin/env python3
"""Compare the client's Skill, NpcSkill or Monster records (client_sdata.py TSV) with defs.skills or defs.mobs
(docs/client/skill-monster-sdata-vs-defs.md).

usage: compare_defs.py <skill|npcskill|monster> <client TSV> <defs TSV with header> <details out> [<spawned mob ids, one per line>]

defs as TSV: COPY (SELECT * FROM defs.skills ORDER BY 2, 3) TO STDOUT WITH (FORMAT csv, HEADER, DELIMITER E'\\t'), likewise defs.mobs.
Keys: a Skill.SData group n is skill id n + 1, its three records levels 1 to 3; a NpcSkill.SData group n is skill id n + 1 at level 100
(its first record; the other two are empty); a Monster.SData record n is mob id n. Values are compared at the client field's width
(defs keeps u16 fields as signed smallint). Fields only the client has (icon, animation, sound, model, attack animations) are left out.
"""
import collections
import csv
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import client_sdata  # noqa: E402

RENAME = {"prev_skill": "prevskillid", "defense_type": "defencetype", "defense_value": "defencevalue", "element": "attrib",
          "quest_item": "questitemid"}
CLIENT_ONLY = {"icon", "animation", "toggle_type", "sound", "model", "attack_ani1", "attack_ani2", "attack_ani3"}
WIDTH = {"B": 0xff, "H": 0xffff, "h": 0xffff, "i": 0xffffffff}


def main():
    kind, client_path, defs_path, out_path = sys.argv[1:5]
    spawned = {int(v) for v in Path(sys.argv[5]).read_text().split()} if len(sys.argv) > 5 else None
    spec = client_sdata.MONSTER_FIELDS if kind == "monster" else client_sdata.SKILL_FIELDS
    widths = {name: WIDTH[fmt] for name, fmt in spec}
    with open(client_path, newline="", encoding="utf-8") as f:
        client_rows = list(csv.DictReader(f, delimiter="\t", quoting=csv.QUOTE_NONE))
    with open(defs_path, newline="", encoding="utf-8") as f:
        defs_rows = list(csv.DictReader(f, delimiter="\t"))

    if kind == "monster":
        client = {(int(r["mob_id"]),): r for r in client_rows}
        server = {(int(r["mobid"]),): r for r in defs_rows}
        name_field = "mobname"
    else:
        client = {}
        for r in client_rows:
            if kind == "skill":
                client[(int(r["skill_id"]) + 1, int(r["level"]))] = r
            elif r["level"] != "0":
                client[(int(r["skill_id"]) + 1, 100)] = r
        server = {(int(r["skillid"]), int(r["skilllevel"])): r for r in defs_rows
                  if (int(r["skilllevel"]) == 100) == (kind == "npcskill")}
        name_field = "skillname"
    fields = [(n, RENAME.get(n, n.replace("_", ""))) for n, _ in spec if n not in CLIENT_ONLY and n != "level"]
    missing = [d for _, d in fields if d not in defs_rows[0]]
    assert not missing, f"defs has no column for {missing}"

    only_server = sorted(set(server) - set(client))
    only_client = sorted(set(client) - set(server))
    print(f"{kind}: client {len(client)}, defs {len(server)}; only in defs {len(only_server)}"
          + (f" ({sum(k[0] in spawned for k in only_server)} spawned)" if spawned else "")
          + f"; only in the client {len(only_client)}")
    out = open(out_path, "w", encoding="utf-8")
    for k in only_server:
        out.write(f"only in defs\t{'/'.join(map(str, k))}\t{server[k][name_field]}\n")
    for k in only_client:
        out.write(f"only in the client\t{'/'.join(map(str, k))}\t{client[k]['name']}\n")

    diffs = {}
    for k in sorted(set(client) & set(server)):
        c, s = client[k], server[k]
        d = [(cf, int(s[df]) & widths[cf], int(c[cf])) for cf, df in fields if int(s[df]) & widths[cf] != int(c[cf])]
        if d:
            diffs[k] = d
    print(f"in both: {len(set(client) & set(server))}; differing: {len(diffs)}"
          + (f" ({sum(k[0] in spawned for k in diffs)} spawned)" if spawned else ""))
    by_field = collections.Counter(f for d in diffs.values() for f, _, _ in d)
    print("fields:", dict(by_field.most_common()))
    for k, d in diffs.items():
        tag = ("spawned" if k[0] in spawned else "not spawned") if spawned else ""
        out.write(f"differ\t{'/'.join(map(str, k))}\t{tag}\t{client[k]['name']}\t"
                  + ", ".join(f"{f} {a}->{b}" for f, a, b in d) + "\n")


if __name__ == "__main__":
    main()
