#!/usr/bin/env python3
"""Read the client's decrypted Skill, NpcSkill, Monster and Cash SData files and the plain GuildHouse and KillStatus files (US client,
EP4 layouts; docs/client/sdata-generation.md section 3), checking that every byte is used.

usage: client_sdata.py <skill|npcskill|monster|cash|guildhouse|killstatus> <file> [<TSV out>]

The record layouts follow Parsec's readers (Parsec, github.com/matigramirez/Parsec, `src/Parsec/Shaiya`, the EP4 branch where they differ).
Strings are u32 length + text, the text ending in a NUL that is part of the length. Each reader also rebuilds the file from the
parsed records and checks it is byte-identical, so nothing is hidden between records.
"""
import struct
import sys

# Skill record (Parsec SkillDefinition, EP4/EP5: no Effect byte, 3 abilities), after the name and description.
SKILL_FIELDS = [
    ("level", "B"), ("icon", "H"), ("animation", "H"), ("toggle_type", "B"), ("sound", "H"), ("req_level", "H"), ("country", "B"),
    ("attack_fighter", "B"), ("defense_fighter", "B"), ("patrol_rogue", "B"), ("shoot_rogue", "B"), ("attack_mage", "B"),
    ("defense_mage", "B"), ("grow", "B"), ("skill_point", "B"), ("type_show", "B"), ("type_attack", "B"), ("type_effect", "B"),
    ("type_detail", "H"), *[(f"need_weapon{i}", "B") for i in range(1, 16)], ("shield", "B"), ("sp", "H"), ("mp", "H"),
    ("ready_time", "B"), ("reset_time", "H"), ("attack_range", "B"), ("state_type", "B"), ("attr_type", "B"), ("disable", "H"),
    ("prev_skill", "H"), ("success_type", "B"), ("success_value", "B"), ("target_type", "B"), ("apply_range", "B"),
    ("multi_attack", "B"), ("keep_time", "H"), ("weapon1", "B"), ("weapon2", "B"), ("weapon_value", "B"), ("bag", "B"),
    ("arrow", "H"), ("damage_type", "B"), ("damage_hp", "H"), ("damage_sp", "H"), ("damage_mp", "H"), ("time_damage_type", "B"),
    ("time_damage_hp", "H"), ("time_damage_sp", "H"), ("time_damage_mp", "H"), ("add_damage_hp", "H"), ("add_damage_sp", "H"),
    ("add_damage_mp", "H"), ("ability_type1", "B"), ("ability_value1", "H"), ("ability_type2", "B"), ("ability_value2", "H"),
    ("ability_type3", "B"), ("ability_value3", "H"), ("heal_hp", "H"), ("heal_sp", "H"), ("heal_mp", "H"), ("time_heal_hp", "H"),
    ("time_heal_sp", "H"), ("time_heal_mp", "H"), ("defense_type", "B"), ("defense_value", "B"), ("limit_hp", "B"),
    ("fix_range", "B"), ("change_type", "H"), ("change_level", "H"),
]
SKILL_LEVELS = 3

MONSTER_FIELDS = [
    ("model", "h"), ("level", "h"), ("ai", "B"), ("hp", "i"), ("day", "B"), ("size", "B"), ("element", "B"), ("normal_time", "i"),
    ("normal_step", "B"), ("chase_time", "i"), ("chase_step", "B"), ("attack_type1", "B"), ("attack_ani1", "B"),
    ("attack_type2", "B"), ("attack_ani2", "B"), ("attack_type3", "B"), ("attack_ani3", "B"), ("attack_plus3", "B"),
    ("quest_item", "h"),
]

GUILDHOUSE_NPC_FIELDS = [
    ("price_rate", "B"), ("npc_level", "B"), ("lapis_mix_percent_rate", "B"), ("lapis_mix_decrease_rate", "B"), ("min_rank", "B"),
    ("icon", "H"), ("sysmsg", "H"), ("up_price", "H"), ("service_price", "H"), ("npc_type", "B"), ("group", "B"),
]


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
        if n > 65536:
            raise ValueError(f"string length {n} at {self.pos - 4}")
        raw = self.data[self.pos:self.pos + n]
        self.pos += n
        return raw

    def fields(self, spec):
        return {name: self.take(fmt)[0] for name, fmt in spec}


def pack_fields(spec, record):
    return b"".join(struct.pack("<" + fmt, record[name]) for name, fmt in spec)


def pack_text(raw):
    return struct.pack("<I", len(raw)) + raw


def show(raw):
    """The text without its NUL, for a TSV (tabs and line breaks escaped)."""
    return raw.rstrip(b"\0").decode("cp1252", errors="replace").replace("\\", "\\\\").replace("\t", "\\t").replace("\n", "\\n").replace("\r", "\\r")


def read_skills(data):
    r = Reader(data)
    records = []
    for skill_id in range(r.u32()):
        for level in range(SKILL_LEVELS):
            rec = {"skill_id": skill_id, "name": r.text(), "description": r.text()}
            rec.update(r.fields(SKILL_FIELDS))
            records.append(rec)
    rebuilt = struct.pack("<I", len(records) // SKILL_LEVELS) + b"".join(
        pack_text(x["name"]) + pack_text(x["description"]) + pack_fields(SKILL_FIELDS, x) for x in records)
    return records, r.pos, rebuilt, ["skill_id", "name", "description"] + [n for n, _ in SKILL_FIELDS]


def read_monsters(data):
    r = Reader(data)
    records = []
    for mob_id in range(r.u32()):
        rec = {"mob_id": mob_id, "name": r.text()}
        rec.update(r.fields(MONSTER_FIELDS))
        records.append(rec)
    rebuilt = struct.pack("<I", len(records)) + b"".join(pack_text(x["name"]) + pack_fields(MONSTER_FIELDS, x) for x in records)
    return records, r.pos, rebuilt, ["mob_id", "name"] + [n for n, _ in MONSTER_FIELDS]


def read_cash(data):
    """Parsec Cash: u32 n products; each u32 index, bag, icon, cost; 24 x (u32 item id, u8 count); name, code, description."""
    r = Reader(data)
    records = []
    for _ in range(r.u32()):
        index, bag, icon, cost = r.take("4I")
        items = [r.take("IB") for _ in range(24)]
        records.append({"index": index, "bag": bag, "icon": icon, "cost": cost, "items": items,
                        "name": r.text(), "code": r.text(), "description": r.text()})
    rebuilt = struct.pack("<I", len(records)) + b"".join(
        struct.pack("<4I", x["index"], x["bag"], x["icon"], x["cost"]) + b"".join(struct.pack("<IB", *i) for i in x["items"])
        + pack_text(x["name"]) + pack_text(x["code"]) + pack_text(x["description"]) for x in records)
    for x in records:
        # item id = type * 1000 + type id (as in the item mall's tables), count
        x["items"] = " ".join(f"{i // 1000}/{i % 1000}x{n}" for i, n in x["items"] if i)
    return records, r.pos, rebuilt, ["index", "bag", "icon", "cost", "items", "name", "code", "description"]


def read_guildhouse(data):
    """Parsec GuildHouse: u32 unknown, u32 house price, u32 service price, 36 NPC infos (16 bytes), 24 x u32 NPC id."""
    r = Reader(data)
    head = r.take("3I")
    infos = [r.fields(GUILDHOUSE_NPC_FIELDS) for _ in range(36)]
    npcs = list(r.take("24I"))
    rebuilt = struct.pack("<3I", *head) + b"".join(pack_fields(GUILDHOUSE_NPC_FIELDS, x) for x in infos) + struct.pack("<24I", *npcs)
    records = [{"seq": i, **x} for i, x in enumerate(infos)]
    print(f"head (unknown, house price, service price): {head}; npc ids: {npcs}")
    return records, r.pos, rebuilt, ["seq"] + [n for n, _ in GUILDHOUSE_NPC_FIELDS]


def read_killstatus(data):
    """Parsec KillStatus: u32 n; each u8 faction, i32 bless value, u16 index, 6 x (u8 type, u16 value)."""
    r = Reader(data)
    records = []
    for _ in range(r.u32()):
        faction, bless, index = r.take("BiH")
        bonuses = [r.take("BH") for _ in range(6)]
        records.append({"faction": faction, "bless": bless, "index": index, "bonuses": bonuses})
    rebuilt = struct.pack("<I", len(records)) + b"".join(
        struct.pack("<BiH", x["faction"], x["bless"], x["index"]) + b"".join(struct.pack("<BH", *b) for b in x["bonuses"])
        for x in records)
    for x in records:
        x["bonuses"] = " ".join(f"{t}:{v}" for t, v in x["bonuses"] if t or v)
    return records, r.pos, rebuilt, ["faction", "bless", "index", "bonuses"]


READERS = {"skill": read_skills, "npcskill": read_skills, "monster": read_monsters, "cash": read_cash,
           "guildhouse": read_guildhouse, "killstatus": read_killstatus}


def main():
    kind, path = sys.argv[1], sys.argv[2]
    data = open(path, "rb").read()
    records, end, rebuilt, columns = READERS[kind](data)
    print(f"{kind}: {len(records)} records, ending at byte {end} of {len(data)}; rebuilt byte-identical: {rebuilt == data}")
    if end != len(data) or rebuilt != data:
        sys.exit("layout does not explain the whole file")
    if len(sys.argv) > 3:
        with open(sys.argv[3], "w", encoding="utf-8") as out:
            out.write("\t".join(columns) + "\n")
            for x in records:
                out.write("\t".join(show(x[c]) if isinstance(x[c], bytes) else str(x[c]) for c in columns) + "\n")


if __name__ == "__main__":
    main()
