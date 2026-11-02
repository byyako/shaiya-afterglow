#!/usr/bin/env python3
"""Parse the NPC part of NpcQuest.SData as ps_game.exe's CNpcData::Load (0x4c9410) reads it; never executes anything.

usage: npcquest.py <NpcQuest.SData> [--list]

Layout (little-endian), in this order:
  u32 n merchants (type 1), each: npc head with an extra u8 after the id; u32 m, m x { u8 item type, u8 item type id }; quest lists
  u32 n gatekeepers (type 2), each: npc head; 3 x { u16 map, f32 x, y, z, u32 len, name[len], u32 cost }; quest lists
  11 x { u32 n, n x { npc head; quest lists } }   types 3 to 13
npc head: u8 type, u16 type id, [u8 merchant only], u32 a, u32 b, u32 c, u32 country (1 light, 2 fury, else both),
          u32 len, name[len], u32 len, text[len]
quest lists: u32 n, n x u16; u32 m, m x u16
Then, as CItemQuestData::Load (0x4c9d90) and CQuestData::Load (0x4ca5d0) read it:
  256 x 256 item links (item type, then type id): u32 n, n x u16 quests started; u32 m, m x u16 quests ended
  u32 n quests, each: u16 id, u32 len, name[len], u32 len, summary[len], the fixed fields (QUEST_FIELDS),
                      3 x result (RESULT_FIELDS), 7 x { u32 len, text[len] }
"""
import struct
import sys


class Reader:
    def __init__(self, data):
        self.data, self.pos = data, 0

    def take(self, fmt):
        values = struct.unpack_from("<" + fmt, self.data, self.pos)
        self.pos += struct.calcsize("<" + fmt)
        return values

    def u32(self):
        return self.take("I")[0]

    def text(self):
        n = self.u32()
        if n > 4096:
            raise ValueError(f"string length {n} at {self.pos - 4}")
        raw = self.data[self.pos:self.pos + n]
        self.pos += n
        text = raw.split(b"\0")[0]
        decoded = text.decode("cp949")
        if decoded.encode("cp949") != text:
            raise ValueError(f"name at {self.pos - n} does not round-trip as cp949")
        return decoded


def head(r, merchant=False):
    kind = r.take("B")[0]
    type_id = r.take("H")[0]
    extra = r.take("B")[0] if merchant else None
    a, b, c, country = r.take("4I")
    return {"type": kind, "type_id": type_id, "extra": extra, "a": a, "b": b, "c": c, "country": country,
            "name": r.text(), "text": r.text()}


def quests(r, npc):
    npc["quests_a"] = list(r.take(f"{r.u32()}H"))
    npc["quests_b"] = list(r.take(f"{r.u32()}H"))


# stQuestInfo fields in file order (name, struct format); runs of the same kind are read one at a time by the original.
QUEST_FIELDS = [
    ("level_min", "H"), ("level_max", "H"), ("country", "B"), ("grow", "B"), ("sex", "2B"), ("job", "6B"),
    ("hg", "H"), ("vg", "h"), ("cg", "B"), ("og", "B"), ("ig", "B"), ("prev_quest", "H"), ("party", "B"),
    ("party_job", "6B"), ("time_min", "I"), ("time", "I"), ("tick_start_term", "I"), ("tick_keep_time", "I"),
    ("tick_recv_count", "I"), ("start_type", "B"), ("start_npc_type", "B"), ("start_npc_id", "H"),
    ("start_item_type", "B"), ("start_item_id", "B"), ("get_items", "9B"), ("end_type", "B"), ("end_npc_type", "B"),
    ("end_npc_id", "H"), ("end_items", "9B"), ("pc_kill_count", "B"), ("mob_kill_1", "HB"), ("mob_kill_2", "HB"),
    ("result_type", "B"), ("result_user_select", "B"),
]
RESULT_FIELDS = [
    ("need_mob_id", "H"), ("need_mob_count", "B"), ("need_item_id", "B"), ("need_item_count", "B"),
    ("need_time", "I"), ("need_hg", "H"), ("need_vg", "h"), ("need_og", "B"), ("exp", "I"), ("money", "I"),
    ("item_type", "B"), ("item_type_id", "B"), ("next_quest", "H"),
]


def fields(r, spec):
    out = {}
    for name, fmt in spec:
        values = r.take(fmt)
        out[name] = values[0] if len(values) == 1 else list(values)
    return out


def text_raw(r):
    n = r.u32()
    if n > 4096:
        raise ValueError(f"string length {n} at {r.pos - 4}")
    raw = r.data[r.pos:r.pos + n]
    r.pos += n
    return raw.split(b"\0")[0].decode("cp949", errors="replace")


def parse_quests(r):
    """The item links and the quests after the NPC part. Returns (links, quests); links maps (type, type id) to
    (started, ended) for the items that have any."""
    links = {}
    for item_type in range(256):
        for type_id in range(256):
            started = list(r.take(f"{r.u32()}H"))
            ended = list(r.take(f"{r.u32()}H"))
            if started or ended:
                links[(item_type, type_id)] = (started, ended)
    quests = []
    for _ in range(r.u32()):
        q = {"id": r.take("H")[0], "name": text_raw(r), "summary": text_raw(r)}
        q.update(fields(r, QUEST_FIELDS))
        q["results"] = [fields(r, RESULT_FIELDS) for _ in range(3)]
        q["texts"] = [text_raw(r) for _ in range(7)]
        quests.append(q)
    return links, quests


def parse(data):
    r = Reader(data)
    npcs = []
    for _ in range(r.u32()):
        npc = head(r, merchant=True)
        npc["items"] = [r.take("BB") for _ in range(r.u32())]
        quests(r, npc)
        npcs.append(npc)
    for _ in range(r.u32()):
        npc = head(r)
        npc["gates"] = []
        for _ in range(3):
            map_id, x, y, z = r.take("H3f")
            npc["gates"].append({"map": map_id, "pos": (x, y, z), "name": r.text(), "cost": r.u32()})
        quests(r, npc)
        npcs.append(npc)
    for list_index in range(11):
        for _ in range(r.u32()):
            npc = head(r)
            quests(r, npc)
            npc["list"] = list_index + 3
            npcs.append(npc)
    return npcs, r.pos


def parse_all(data):
    """NPCs, item links and quests; the whole file."""
    npcs, pos = parse(data)
    r = Reader(data)
    r.pos = pos
    links, quests = parse_quests(r)
    return npcs, links, quests, r.pos


def main():
    path = sys.argv[1]
    with open(path, "rb") as fh:
        data = fh.read()
    npcs, end = parse(data)
    by_type = {}
    for n in npcs:
        by_type.setdefault(n["type"], 0)
        by_type[n["type"]] += 1
    print(f"{len(npcs)} npcs, ending at byte {end} of {len(data)}; per type: {dict(sorted(by_type.items()))}")
    mismatched = [n for n in npcs if "list" in n and n["list"] != n["type"]]
    print(f"records whose type differs from their list: {len(mismatched)}")
    _, links, quests, end = parse_all(data)
    print(f"{len(links)} items with quest links, {len(quests)} quests, ending at byte {end} of {len(data)}")
    print(f"quests whose id is not their position: {sum(1 for i, q in enumerate(quests) if q['id'] != i + 1)}")
    if "--list" in sys.argv:
        for n in npcs:
            extra = f" items={len(n['items'])}" if "items" in n else ""
            extra += f" gates={[g['map'] for g in n['gates']]}" if "gates" in n else ""
            print(f"type {n['type']:2} id {n['type_id']:4} country {n['country']} {n['name']!r}{extra}")


if __name__ == "__main__":
    main()
