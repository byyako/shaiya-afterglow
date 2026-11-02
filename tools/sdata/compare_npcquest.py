#!/usr/bin/env python3
"""Compare the client's NpcQuest.SData (decrypted) with the server's (docs/client/npcquest-sdata-vs-world.md).

usage: compare_npcquest.py <client NpcQuest, decrypted> <server NpcQuest.SData> <placed NPCs TSV> <details out>

Both files have the server's layout and are read with tools/npcquest.py. The server's file is what tools/world/import.py put into
`world.*` unchanged, so it stands for the database. Placed NPCs TSV: `type<TAB>type id` for every NPC on a map:
  COPY (SELECT DISTINCT type, type_id FROM world.map_npcs) TO STDOUT
Shops are compared by position: a buy (0x0702) names the position in the merchant's list, so the client must list the same items in
the same order. Names and texts are left out (the client's are English, the server's Korean; the server never sends them).
"""
import collections
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import npcquest  # noqa: E402

NPC_FIELDS = ("country", "a", "b", "c", "extra")
SKIP_QUEST = ("name", "summary", "texts", "results")


def load(path):
    npcs, links, quests, end = npcquest.parse_all(Path(path).read_bytes())
    return {(n["type"], n["type_id"]): n for n in npcs}, links, {q["id"]: q for q in quests}


def main():
    client, client_links, client_quests = load(sys.argv[1])
    server, server_links, server_quests = load(sys.argv[2])
    placed = {tuple(int(v) for v in line.split("\t")) for line in Path(sys.argv[3]).read_text().split("\n") if line}
    out = open(sys.argv[4], "w", encoding="utf-8")
    mark = lambda k: "placed" if k in placed else "unplaced"

    def note(section, key, text):
        out.write(f"{section}\t{key[0]}/{key[1]}\t{mark(key) if len(key) == 2 else ''}\t{text}\n")

    only_server = sorted(set(server) - set(client))
    only_client = sorted(set(client) - set(server))
    by_type = collections.Counter(k[0] for k in only_server)
    print(f"NPCs: client {len(client)}, server {len(server)}; only on the server {len(only_server)} "
          f"({sum(k in placed for k in only_server)} placed) per type {dict(sorted(by_type.items()))}; only in the client {len(only_client)}")
    for k in only_server:
        note("npc only on the server", k, server[k]["name"])
    for k in only_client:
        note("npc only in the client", k, client[k]["name"])

    both = sorted(set(client) & set(server))
    field_diffs = collections.Counter()
    for k in both:
        for f in NPC_FIELDS:
            if client[k][f] != server[k][f]:
                field_diffs[f] += 1
                note(f"npc {f}", k, f"server {server[k][f]} client {client[k][f]} ({client[k]['name']})")
    print(f"NPC fields differing (of {len(both)} in both): {dict(field_diffs)}")

    shops = [k for k in both if k[0] == 1]
    same = differ = 0
    for k in shops:
        s, c = server[k]["items"], client[k]["items"]
        if s == c:
            same += 1
            continue
        differ += 1
        first = next((i for i in range(min(len(s), len(c))) if s[i] != c[i]), min(len(s), len(c)))
        only_s = [f"{t}/{i}" for t, i in s if (t, i) not in c]
        only_c = [f"{t}/{i}" for t, i in c if (t, i) not in s]
        note("shop", k, f"{client[k]['name']}: server {len(s)} items, client {len(c)}; first differing position {first}; "
                        f"only server {only_s}; only client {only_c}")
    print(f"shops in both: {len(shops)}; same list {same}, different {differ} "
          f"({sum(1 for k in shops if server[k]['items'] != client[k]['items'] and k in placed)} of them placed)")

    gates = [k for k in both if k[0] == 2]
    gate_diffs = collections.Counter()
    for k in gates:
        for seq, (s, c) in enumerate(zip(server[k]["gates"], client[k]["gates"])):
            for f in ("map", "pos", "cost"):
                if s[f] != c[f]:
                    gate_diffs[f] += 1
                    note(f"gate {f}", k, f"{client[k]['name']} destination {seq} ({c['name']}): server {s[f]} client {c[f]}")
    print(f"gatekeepers in both: {len(gates)}; destination fields differing: {dict(gate_diffs)}")

    quest_lists = sum(1 for k in both if (client[k]["quests_a"], client[k]["quests_b"]) != (server[k]["quests_a"], server[k]["quests_b"]))
    for k in both:
        if (client[k]["quests_a"], client[k]["quests_b"]) != (server[k]["quests_a"], server[k]["quests_b"]):
            note("npc quest lists", k, f"server {server[k]['quests_a']} / {server[k]['quests_b']} "
                                       f"client {client[k]['quests_a']} / {client[k]['quests_b']}")
    print(f"NPC quest lists differing: {quest_lists}")

    link_diffs = sorted(k for k in set(client_links) | set(server_links) if client_links.get(k) != server_links.get(k))
    for k in link_diffs:
        out.write(f"item quest links\t{k[0]}/{k[1]}\t\tserver {server_links.get(k)} client {client_links.get(k)}\n")
    print(f"item quest links: client {len(client_links)}, server {len(server_links)}, differing {len(link_diffs)}")

    quest_field_diffs = collections.Counter()
    quests_differing = set()
    for qid in sorted(set(client_quests) & set(server_quests)):
        s, c = server_quests[qid], client_quests[qid]
        for f in s:
            if f in SKIP_QUEST or s[f] == c[f]:
                continue
            quest_field_diffs[f] += 1
            quests_differing.add(qid)
            out.write(f"quest {f}\t{qid}\t\tserver {s[f]} client {c[f]} ({c['name']})\n")
        for seq, (sr, cr) in enumerate(zip(s["results"], c["results"])):
            for f in sr:
                if sr[f] != cr[f]:
                    quest_field_diffs[f"results.{f}"] += 1
                    quests_differing.add(qid)
                    out.write(f"quest result {seq} {f}\t{qid}\t\tserver {sr[f]} client {cr[f]} ({c['name']})\n")
    print(f"quests: client {len(client_quests)}, server {len(server_quests)}; differing {len(quests_differing)}; "
          f"fields {dict(quest_field_diffs.most_common())}")


if __name__ == "__main__":
    main()
