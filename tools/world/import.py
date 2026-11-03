#!/usr/bin/env python3
"""Import the original server's maps and NPCs into PostgreSQL schema `original_world`, the frozen copy (db/world/schema.sql).

usage: import.py <PS_Game Data folder> [--dry-run] [--source TEXT] [--reset | --adopt]

Reads Map.ini, Map/<id>.svmap for every map Map.ini lists, NpcQuest.SData (NPCs and quests), with the parsers in tools/svmap.py
and tools/npcquest.py (docs/original/maps-npcs.md, docs/original/quest.md), and ItemCreate.ini (the boxes' sets,
docs/original/item-origins.md section 5.1), KillStatus.SData (the factions' kill-count grades, docs/original/pvp-2.md
section 8), Obelisk.ini (the obelisks and their guards, and the boss groups, docs/original/obelisks.md sections 1 and 11.1), mob_ai.ini with the AI files it names
and kr_string.DB (the scripted monsters, docs/original/scripted-mobs.md section 2), Lua/*.lua (the Lua monsters,
docs/original/instances.md section 9), GuildHouse.ini (the guild houses, docs/original/guild-house.md section 2), the item -> quest links of NpcQuest.SData and
SequenceItemCreate.ini (docs/original/quest-drops.md sections 1.3 and 4.1). Runs psql as tools/dbconn.py says (the PostgreSQL
container by default), like tools/defs/load_into_postgres.py. Everything is imported as it is; the game server applies the original's
rules (which NPCs are created) when it loads the tables, so edited rows follow the same rules.

The original data is imported once (docs/plans/ext-1-foundation.md section 3.2), as tools/defs/load_into_postgres.py does it: the import
is recorded in content.imports with the SHA-256 of every file read, in the transaction that fills the frozen copy; the same files again
do nothing; other files are refused unless --reset, which replaces the copy and rebuilds the live `world` from it, the installed
extensions and the data editor's edits (`data-editor content rebuild world`, which ends the import). --adopt records a copy loaded
from the same files on a box whose `world` already holds them. --source is the text recorded as where the files came from.
"""
import csv
import io
import re
import struct
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import content_import  # noqa: E402
import npcquest  # noqa: E402
import svmap  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]
SCHEMA = ROOT / "db" / "world" / "schema.sql"
FROZEN = "original_world"
# The server's text files and NpcQuest.SData are Korean (code page 949).
ENCODING = "cp949"
# Map.ini CreateType letters as CMap::LoadConfig numbers them; anything else is 0.
CREATE_TYPES = {"S": 1, "F": 2, "P": 3, "G": 4, "R": 5, "H": 6}


def read_ini(path):
    """Map.ini as {section: {key: value}}; // comments and the /* */ block at the top are skipped."""
    text = path.read_bytes().decode(ENCODING)
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    sections, current = {}, None
    for line in text.splitlines():
        line = line.split("//")[0].strip()
        if line.startswith("[") and "]" in line:
            current = sections.setdefault(line[1:line.index("]")].strip(), {})
        elif "=" in line and current is not None:
            key, value = line.split("=", 1)
            current[key.strip()] = value.strip()
    return sections


def position(value):
    """'map, x, y, z' -> (map, [x, y, z]), or (None, None) when the map is -1 or missing."""
    if not value:
        return None, None
    parts = [p.strip() for p in value.replace("(", "").replace(")", "").split(",")]
    if len(parts) != 4 or int(parts[0]) < 0:
        return None, None
    return int(parts[0]), [float(p) for p in parts[1:]]


def atol(token):
    """The C library's atol: an optional sign and the leading digits; 0 when there are none."""
    match = re.match(r"[+-]?\d+", token)
    return int(match.group()) if match else 0


def open_times(value):
    """CreateTime as ParseMapOpenTimeArray reads it: numbers (atol) at the strtok delimiters, up to 7 (day, hour, hours) triples, the
    parse ending at the first missing number. None when the key is missing (always open)."""
    if value is None:
        return None
    numbers = [atol(t) for t in re.split(r"[ ,=()\[\]{}\t\r\n]+", value) if t]
    triples = min(len(numbers) // 3, 7)
    return numbers[:3 * triples]


def read_mob_lua(data_dir):
    """Data/Lua/<n>.lua as (ai, bytes) rows, the files as they are."""
    folder = data_dir / "Lua"
    files = sorted(folder.glob("*.lua"), key=lambda f: f.stem) if folder.is_dir() else []
    return [[int(f.stem), "\\x" + f.read_bytes().hex()] for f in files if f.stem.isdigit()]


def read_item_create(path):
    """ItemCreate.ini as (set, seq, grade, weight) rows, read as CItemDrop::InitItemCreate reads it: `//` and `--` comments,
    `[ItemCreate_n]` sections (any case), the first two tokens of each other line. A set given twice keeps only its last section,
    as the original's. Ranges and weights are left to the game server, which applies the original's rules when it loads them."""
    delimiters = r"[ ,=()\[\]{}\t\r\n]+"
    sets, current = {}, None
    for line in path.read_bytes().decode(ENCODING).split("\n"):
        if len(line) + 1 <= 1:
            continue
        text = line.lstrip(" \t")
        if text.startswith("//") or text.startswith("--"):
            continue
        if text[:12].lower() == "[itemcreate_":
            tokens = [t for t in re.split(delimiters, text[12:]) if t]
            current = atol(tokens[0]) if tokens else None
            if current is not None:
                sets[current] = []
            continue
        tokens = [t for t in re.split(delimiters, text) if t]
        if current is not None and len(tokens) >= 2:
            sets[current].append((atol(tokens[0]), atol(tokens[1])))
    return [[n, seq, grade, weight] for n, lines in sets.items() for seq, (grade, weight) in enumerate(lines)]


def read_sequence_groups(path):
    """SequenceItemCreate.ini as (group, sequence, grades) rows, read as CItemDrop::InitSequenceItemCreate reads it with
    GetPrivateProfileInt: `;` comments, sections and keys in any case, a missing key or a line without `=` is 0, a value is its
    leading `[-]digits` (else 0). COUNT from [SEQUENCE_GROUP_COUNT], then SEQUENCE and GRADE_0..GRADE_9 of groups 1 to COUNT, as read
    (no clamping: the game server applies the rules)."""
    sections, current = {}, None
    for line in path.read_bytes().decode(ENCODING).splitlines():
        line = line.strip()
        if not line or line.startswith(";"):
            continue
        if line.startswith("[") and "]" in line:
            current = sections.setdefault(line[1:line.index("]")].strip().lower(), {})
        elif "=" in line and current is not None:
            key, value = line.split("=", 1)
            current.setdefault(key.strip().lower(), value.strip())

    def number(section, key):
        match = re.match(r"-?\d+", sections.get(section, {}).get(key, ""))
        return int(match.group()) if match else 0

    rows = []
    for n in range(1, number("sequence_group_count", "count") + 1):
        section = f"sequence_group_{n}"
        rows.append([n, number(section, "sequence"), pg_array([number(section, f"grade_{g}") for g in range(10)])])
    return rows


def read_kill_status(path):
    """KillStatus.SData as (seq, faction, kills, index, types, values) rows: u32 n, then per record u8 faction, i32 kills, u16 index,
    6 x (u8 type, u16 value), as CGameData::LoadKCStatus reads it."""
    data = path.read_bytes()
    count = struct.unpack_from("<I", data, 0)[0]
    rows, offset = [], 4
    for seq in range(count):
        faction, kills, index = struct.unpack_from("<BiH", data, offset)
        offset += 7
        pairs = [struct.unpack_from("<BH", data, offset + 3 * i) for i in range(6)]
        offset += 18
        rows.append([seq, faction, kills, index, pg_array([t for t, _ in pairs]), pg_array([v for _, v in pairs])])
    if offset != len(data):
        sys.exit(f"{path}: {len(data) - offset} bytes left over; not a KillStatus.SData as documented")
    return rows


# CGuildMgr::InitGuildHouseIniFile's built-in tables (0x541b10 to 0x541d50, 0x5471e0): what a key missing from GuildHouse.ini keeps.
GUILD_HOUSE_KEYS = {"GateKeeper": 2, "Smith": 3, "Bank": 6, "ShopWeapon": 0x0101, "ShopDefen": 0x0201, "General": 0x0301,
                    "ShopPosion": 0x0401}
GUILD_HOUSE_KEEP = {2: [200, 500, 800], 0x0101: [], 0x0201: [], 0x0301: [], 3: [100, 200, 500, 800, 1000], 0x0401: [50, 200, 500],
                    6: [100, 200, 500, 800, 1000]}
GUILD_HOUSE_UPGRADE = {2: [1000, 1500, 2000], 0x0101: [100, 200, 500], 0x0201: [100, 200, 500], 0x0301: [50, 100, 200],
                       3: [500, 1000, 2000, 10000, 15000], 0x0401: [2000, 5000, 10000], 6: [200, 500, 1000, 2000, 5000]}
# (section, key) -> (count, built-in values); the gate rates have four places but the file fills three.
GUILD_HOUSE_RATES = {
    "gate_rates": ("GateKeeper", "MoveDecreRate", 3, [0.0, 0.0, 0.0, 0.0]),
    "shop_rates": ("ShopDefault", "BuyDecreRate", 4, [0.99, 0.98, 0.95, 0.90]),
    "potion_rates": ("ShopDefault", "BuyPotionDecreRate", 4, [0.90, 0.80, 0.70, 0.60]),
    "repair_rates": ("Smith", "RepairDecreRate", 6, [0.99, 0.95, 0.95, 0.90, 0.90, 0.90]),
    "mix_rates": ("Smith", "RapiceMixDecreRate", 6, [1.0, 1.0, 0.95, 0.95, 0.95, 0.90]),
    "mix_bonus": ("Smith", "RapiceMixPercentRate", 6, [0, 0, 0, 0, 1, 2]),
    "bank_slots": ("Bank", "PermitSlot", 6, [40, 80, 120, 160, 200, 240]),
}
GUILD_HOUSE_LEVEL_KINDS = ["GateKeeper", "ShopWeapon", "ShopDefen", "ShopGeneral"]
GUILD_HOUSE_LEVELS = {0: [[37, 38, 39], [196, 197, 198], [200, 201, 202], [200, 201, 202]],
                      1: [[41, 42, 43], [207, 208, 209], [211, 212, 213], [200, 201, 202]]}


def read_profile(path):
    """An ini file as GetPrivateProfileString reads it: {section: {key: value}}, lines starting with ; skipped."""
    sections, current = {}, None
    for line in path.read_bytes().decode(ENCODING).splitlines():
        line = line.strip()
        if not line or line.startswith(";"):
            continue
        if line.startswith("[") and "]" in line:
            current = sections.setdefault(line[1:line.index("]")].strip(), {})
        elif "=" in line and current is not None:
            key, value = line.split("=", 1)
            current.setdefault(key.strip(), value.strip())
    return sections


def read_guild_house(path):
    """GuildHouse.ini as (the one settings row, NPC key rows, level rows), the built-in values where a key is missing."""
    ini = read_profile(path)

    def numbers(section, key, count, default, kind):
        value = ini.get(section, {}).get(key)
        if value is None:
            return list(default)
        parsed = [kind(atof(v.strip()) if kind is float else atol(v.strip())) for v in value.split(",")[:count]]
        return parsed + list(default[len(parsed):])

    house = ini.get("DefaultSet", {})
    settings = [1, atol(house.get("HouseBuyMoney", "50000000")), atol(house.get("HouseKeepEtin", "2000"))]
    for column, (section, key, count, default) in GUILD_HOUSE_RATES.items():
        kind = int if column in ("mix_bonus", "bank_slots") else float
        settings.append(pg_array(numbers(section, key, count, default, kind)))
    npcs = []
    for section, npc_key in GUILD_HOUSE_KEYS.items():
        keep = GUILD_HOUSE_KEEP[npc_key]
        upgrade = GUILD_HOUSE_UPGRADE[npc_key]
        keep = numbers(section, "KeepEtin", len(keep), keep, int) if keep else []
        npcs.append([npc_key, pg_array(keep), pg_array(numbers(section, "UpgradeEtin", len(upgrade), upgrade, int))])
    levels = []
    for country, section in ((0, "NpcLevel_Light"), (1, "NpcLevel_Dark")):
        for kind, name in enumerate(GUILD_HOUSE_LEVEL_KINDS):
            for level in (1, 2, 3):
                default = GUILD_HOUSE_LEVELS[country][kind][level - 1]
                levels.append([country, kind, level, atol(ini.get(section, {}).get(f"{name}{level}", str(default)))])
    return settings, npcs, levels


# CMap::LoadObelisk's strtok delimiters.
OBELISK_DELIMITERS = re.compile(r"[ ,=()\]\[{}\t\r\n]+")
# Its limits for a boss group (stMapBoth): stages, follower lines, followers per line.
BOSS_STAGES, BOSS_FOLLOWER_GROUPS, BOSS_FOLLOWERS = 15, 32, 16


def atof(token):
    """The C library's atof on a decimal number: the longest leading number; 0 when there is none."""
    match = re.match(r"[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?", token)
    return float(match.group()) if match else 0.0


def read_obelisks(path):
    """Obelisk.ini's obelisks and guard groups as CMap::LoadObelisk reads them: lines of at most one character, and lines
    starting // or -- (after spaces and tabs) are skipped; the section heads set the state, and [Obelisk_K] and [Mob] count only
    in a [zone_N] section, an obelisk or a group only when all eight of its values are there (the original leaves a short line
    unused). [Portal_…] sections (with [light], [fury]) are another state, read and dropped (no links, docs/original/obelisks.md
    section 11.7). [bothmob_n, (map, stages, respawn, (pmap, portal))] = (x,y,z),(id1, …) starts a boss group (section 11.1):
    stages cut to 15, exactly `stages` ids (fewer: the group is dropped); in its state, [Mob] = (x,y,z,r),(id1, …, id stages+1), n
    is a follower group (n cut to 16, at most 32, a short line dropped) and [RandomRespawn] = (min, max),(x,y,z)×3 makes the last
    group random. Returns (obelisks, guards, boss groups, followers) rows."""
    obelisks, guards, bosses, followers = [], [], [], []
    state, zone, current, groups = 0, None, None, 0
    boss = None

    def tokens(rest):
        return [t for t in OBELISK_DELIMITERS.split(rest) if t]

    for raw in path.read_bytes().decode(ENCODING).splitlines(keepends=True):
        if len(raw.encode(ENCODING)) <= 1:
            continue
        line = raw.lstrip(" \t")
        if line.startswith("//") or line.startswith("--"):
            continue
        before, head = state, line.lower()
        if head.startswith("[portal_"):
            state = 3
            continue
        if before == 3 and (head.startswith("[light]") or head.startswith("[fury]")):
            continue
        if head.startswith("[bothmob_"):
            state, boss = 4, None
            t = tokens(line[9:])
            if len(t) < 9:
                print(f"Obelisk.ini: boss group line too short, dropped: {line.strip()}", file=sys.stderr)
                continue
            stages = min(atol(t[2]), BOSS_STAGES)
            ids = t[9:9 + max(stages, 0)]
            if len(ids) < stages:
                print(f"Obelisk.ini: boss group {t[0]} has fewer than {stages} monsters, dropped", file=sys.stderr)
                continue
            boss = len(bosses)
            bosses.append([boss, atol(t[0]), atol(t[1]), stages, atol(t[3]), atol(t[4]), atol(t[5]), atof(t[6]), atof(t[7]),
                           atof(t[8]), pg_array([atol(i) for i in ids]), None, None, None])
            continue
        if before == 4 and (head.startswith("[mob]") or head.startswith("[randomrespawn]")):
            if boss is None:
                continue
            stages = bosses[boss][3]
            if head.startswith("[mob]"):
                t = tokens(line[5:])
                count = sum(1 for f in followers if f[0] == boss)
                if len(t) < 4 + stages + 2:
                    print(f"Obelisk.ini: boss group {bosses[boss][1]}: follower line too short, dropped", file=sys.stderr)
                elif count >= BOSS_FOLLOWER_GROUPS:
                    print(f"Obelisk.ini: boss group {bosses[boss][1]}: more than {BOSS_FOLLOWER_GROUPS} follower lines, dropped",
                          file=sys.stderr)
                else:
                    followers.append([boss, count, atof(t[0]), atof(t[1]), atof(t[2]), atof(t[3]),
                                      pg_array([atol(i) for i in t[4:4 + stages + 1]]), min(atol(t[4 + stages + 1]), BOSS_FOLLOWERS)])
            elif head.startswith("[randomrespawn]"):
                t = tokens(line[15:])
                if len(t) < 11:
                    print(f"Obelisk.ini: boss group {bosses[boss][1]}: [RandomRespawn] too short, dropped", file=sys.stderr)
                else:
                    bosses[boss][11:14] = [atol(t[0]), atol(t[1]), pg_array([atof(v) for v in t[2:11]])]
            continue
        if head.startswith("[zone_"):
            state, current = 1, None
            t = tokens(line[6:])
            if t:
                zone = atol(t[0])
            continue
        if before != 1 or zone is None:
            continue
        if head.startswith("[obelisk_"):
            t = tokens(line[9:])
            current = None
            if len(t) >= 8:
                current, groups = len(obelisks), 0
                obelisks.append([current, zone, atol(t[0]), atof(t[1]), atof(t[2]), atof(t[3]), atol(t[4]), atol(t[5]), atol(t[6]),
                                 atol(t[7])])
        elif head.startswith("[mob]") and current is not None:
            t = tokens(line[5:])
            if len(t) >= 8:
                guards.append([current, groups, atof(t[0]), atof(t[1]), atof(t[2]), atof(t[3]), atol(t[4]), atol(t[5]), atol(t[6]),
                               atol(t[7])])
                groups += 1
    return obelisks, guards, bosses, followers


# English for kr_string.DB's lines (docs/original/scripted-mobs.md, appendix; deviation 1): the client cannot show the Korean.
AI_STRINGS_ENGLISH = {
    1: "Smite the heretics!",
    2: "Li laka Amon-Ra",
    3: "I'll tear the flesh from your bones!!",
    4: "A feast of blood and death for the traitors!!",
    5: "Die, fool!",
    6: "Let's see what you can do.",
    7: "It is time to die.",
    8: "No one escapes death.",
    9: "You hasten down the road to the underworld!!",
    10: "Heh heh heh~ Fresh meat, at long last.",
    11: "Your deaths shall pay for the wrongs of the past~",
    12: "Now you shall feel our pain~",
    13: "A storm of blood and madness!!!",
    14: "F-for the g-glory of the Helltooths!!",
    15: "I-I'm the b-boss around h-here!!",
    16: "I-I'm gonna kill you.",
    17: "Welcome to the world of dreams.",
    18: "You dare defy a god! Fools~",
    19: "If you think this is the end, you are mistaken!!",
    20: "I shall rise again in the fires of Gehenna.",
    21: "I'll grant you credit for driving me this far. But the game ends here!!",
    22: "Welcome to the festival of blood, foolish creatures~",
    23: "Swatting insects is tiresome. Perhaps I'll rest a while.",
    24: "The power of fire shall wreathe me.",
    25: "Worship me, creatures of Etain!",
    26: "Absorption Shield, deploy.",
    27: "The power of water shall protect me.",
    28: "The strength of the earth shall protect me.",
    29: "Wind~ wrap yourself around me.",
    30: "All shall return to nothing.",
    31: "You have crossed a line that must not be crossed. Behold! Your feeble power...",
}


def read_mob_ai(data_dir):
    """mob_ai.ini as CAIFactory::LoadMobAI reads it ("%d %s" per line) and, for the lines numbered 10 and up, the four files InitFSM
    loads. The paths are a Windows server's: the files are found without regard to case. Returns rows."""
    files = {p.name.lower(): p for p in (data_dir / "AI").iterdir()}
    rows = []
    for line in (data_dir / "mob_ai.ini").read_bytes().decode(ENCODING).splitlines():
        parts = line.split()
        if len(parts) < 2 or not parts[0].lstrip("-").isdigit():
            continue
        number, path = int(parts[0]), parts[1]
        if number < 10:
            continue
        base = path.replace("\\", "/").rsplit("/", 1)[-1].lower()
        texts = []
        for ext in ("alo", "afs", "ai", "ah"):
            found = files.get(f"{base}.{ext}")
            if found is None:
                sys.exit(f"mob_ai.ini {number}: {path}.{ext} missing (the original would not start)")
            texts.append(found.read_bytes().decode(ENCODING).replace("\r\n", "\n"))
        rows.append([number, path, *texts])
    return rows


def read_ai_strings(path):
    """kr_string.DB as CAIStringDB::LoadStringDB reads it: a number line, then the text line (kept without its line end)."""
    lines = path.read_bytes().split(b"\n")
    rows = []
    for i in range(0, len(lines) - 1, 2):
        head = lines[i].strip()
        if not head.isdigit():
            continue
        text = lines[i + 1].rstrip(b"\r")
        if text:
            number = int(head)
            rows.append([number, "\\x" + text.hex(), AI_STRINGS_ENGLISH.get(number)])
    return rows


def pg_array(values):
    return "{" + ",".join(repr(v) for v in values) + "}" if values is not None else None


COPIES = []  # the COPY statements, run in one transaction with the schema and the import record


def copy(table, columns, rows):
    buffer = io.StringIO()
    writer = csv.writer(buffer, lineterminator="\n")
    for row in rows:
        writer.writerow(["\\N" if v is None else v for v in row])
    COPIES.append((table, f"COPY {FROZEN}.{table} ({', '.join(columns)}) FROM STDIN WITH (FORMAT csv, NULL '\\N');\n{buffer.getvalue()}\\.\n"))
    return len(rows)


def read_recorder(data_dir):
    """Records the SHA-256 of every file under data_dir that is read through Path.read_bytes (all the importer's reads go through it)."""
    seen = {}
    plain = Path.read_bytes

    def recording(self):
        data = plain(self)
        try:
            seen[self.resolve().relative_to(data_dir.resolve()).as_posix()] = content_import.sha256_bytes(data)
        except ValueError:
            pass  # not under the Data folder
        return data

    Path.read_bytes = recording
    return seen


def main():
    args = sys.argv[1:]
    dry_run, reset, adopt = "--dry-run" in args, "--reset" in args, "--adopt" in args
    source = None
    if "--source" in args:
        at = args.index("--source")
        source = args[at + 1] if at + 1 < len(args) else sys.exit("--source needs a text")
        del args[at:at + 2]
    args = [a for a in args if a not in ("--dry-run", "--reset", "--adopt")]
    if len(args) != 1 or (reset and adopt):
        sys.exit(__doc__)
    data_dir = Path(args[0])
    files = read_recorder(data_dir)
    ini = read_ini(data_dir / "Map.ini")
    map_count = int(ini["BASE"]["MapCount"])
    defaults = ini.get("DEFAULT_SET_ZONE", {})

    maps, map_npcs, npc_positions, areas, area_mobs, portals, binds = [], [], [], [], [], [], []
    npc_row_id = area_row_id = 0
    for map_id in range(map_count):
        cfg = {**defaults, **ini.get(f"SET_ZONE_{map_id}", {})}
        name = cfg.get("MapName", "-1")
        file_name = f"{map_id}.svmap" if name in ("-1", "") else name
        path = data_dir / "Map" / file_name
        if not path.exists():
            print(f"map {map_id}: {file_name} missing, skipped")
            continue
        raw = path.read_bytes()
        parsed = svmap.parse(raw)
        if parsed["trailing"]:
            sys.exit(f"{path}: {parsed['trailing']} bytes left over; not an svmap as documented")
        size = parsed["size"]
        blocked = raw[4:4 + (size * size + 7) // 8]
        rebirth = position(cfg.get("RebirthMapPos"))
        light = position(cfg.get("RebirthMapPos1"))
        fury = position(cfg.get("RebirthMapPos2"))
        maps.append([
            map_id, size, cfg.get("MapType", "F")[:1], CREATE_TYPES.get(cfg.get("CreateType", "").upper(), 0),
            # CMap::LoadConfig: WarType "P" in any case is 1, anything else 0.
            1 if cfg.get("WarType", "").upper() == "P" else 0, "\\x" + blocked.hex(),
            rebirth[0], pg_array(rebirth[1]), light[0], pg_array(light[1]), fury[0], pg_array(fury[1]),
            int(cfg.get("WeatherState", 0)), int(cfg.get("WeatherRate", 0)), int(cfg.get("WeatherPower", 0)),
            int(cfg.get("WeatherDelay", 0)), int(cfg.get("WeatherNoneDelay", 0)),
            pg_array(open_times(cfg.get("CreateTime"))), atol(cfg.get("CreateMinUser", "0")), atol(cfg.get("EnterMaxUser", "30")),
            atol(cfg.get("ExpireTime", "0")), file_name,
        ])
        for seq, npc in enumerate(parsed["npcs"]):
            npc_row_id += 1
            map_npcs.append([npc_row_id, map_id, seq, npc["type"], npc["type_id"]])
            for pos_seq, (x, y, z, d) in enumerate(npc["at"]):
                npc_positions.append([npc_row_id, pos_seq, x, y, z, d])
        for seq, portal in enumerate(parsed["portals"]):
            portals.append([map_id, seq, *portal["at"], portal["country"], *portal["levels"], portal["map"], *portal["to"]])
        for seq, bind in enumerate(parsed["binds"]):
            binds.append([map_id, seq, bind["country"], bind["attr"], pg_array([v for box in bind["boxes"] for v in box])])
        for seq, area in enumerate(parsed["mob_areas"]):
            area_row_id += 1
            areas.append([area_row_id, map_id, seq, *area["box"]])
            for mob_seq, (mob, count) in enumerate(area["mobs"]):
                area_mobs.append([area_row_id, mob_seq, mob, count])

    npcs, links, quests, _ = npcquest.parse_all((data_dir / "NpcQuest.SData").read_bytes())
    npc_rows, shop_rows, gate_rows = [], [], []
    for n in npcs:
        country = {1: 0, 2: 1}.get(n["country"], 2)
        npc_rows.append([n["type"], n["type_id"], n["name"], country, n["a"], n["b"], n["c"], n["extra"]])
        for seq, (item_type, item_type_id) in enumerate(n.get("items", [])):
            shop_rows.append([n["type_id"], seq, item_type, item_type_id])
        for seq, gate in enumerate(n.get("gates", [])):
            gate_rows.append([n["type_id"], seq, gate["map"], *gate["pos"], gate["name"], gate["cost"]])

    quest_rows, result_rows = [], []
    for q in quests:
        quest_rows.append([
            q["id"], q["name"], q["level_min"], q["level_max"], q["country"], q["grow"], pg_array(q["sex"]), pg_array(q["job"]),
            q["prev_quest"], q["hg"], q["vg"],
            pg_array([q["cg"], q["og"], q["ig"], q["party"], *q["party_job"], q["tick_start_term"], q["tick_keep_time"]]),
            q["time_min"], q["time"], q["tick_recv_count"], q["start_type"], q["start_npc_type"], q["start_npc_id"],
            q["start_item_type"], q["start_item_id"], pg_array(q["get_items"]), q["end_type"], q["end_npc_type"], q["end_npc_id"],
            pg_array(q["end_items"]), q["pc_kill_count"], pg_array([*q["mob_kill_1"], *q["mob_kill_2"]]), q["result_type"],
            q["result_user_select"],
        ])
        for seq, r in enumerate(q["results"]):
            result_rows.append([
                q["id"], seq, r["need_mob_id"], r["need_mob_count"], r["need_item_id"], r["need_item_count"],
                pg_array([r["need_time"], r["need_hg"], r["need_vg"], r["need_og"]]), r["exp"], r["money"], r["item_type"],
                r["item_type_id"], r["next_quest"],
            ])

    link_rows = [[t, i, pg_array(started), pg_array(ended)] for (t, i), (started, ended) in sorted(links.items()) if started or ended]
    box_rows = read_item_create(data_dir / "ItemCreate.ini")
    sequence_rows = read_sequence_groups(data_dir / "SequenceItemCreate.ini")
    kill_rows = read_kill_status(data_dir / "KillStatus.SData")
    obelisk_rows, guard_rows, boss_rows, follower_rows = read_obelisks(data_dir / "Obelisk.ini")
    ai_rows = read_mob_ai(data_dir)
    string_rows = read_ai_strings(data_dir / "kr_string.DB")
    lua_rows = read_mob_lua(data_dir)
    house_row, house_npc_rows, house_level_rows = read_guild_house(data_dir / "GuildHouse.ini")

    print(f"{len(maps)} maps, {len(map_npcs)} map NPCs ({len(npc_positions)} positions), {len(areas)} monster areas "
          f"({len(area_mobs)} entries), {len(portals)} portals, {len(binds)} bind areas, {len(npc_rows)} NPCs, {len(shop_rows)} shop items, "
          f"{len(gate_rows)} gates, {len(quest_rows)} quests, {len(box_rows)} box set lines, {len(link_rows)} item quest links "
          f"({sum(1 for r in link_rows if r[3] != '{}')} ending, {sum(1 for r in link_rows if r[2] != '{}')} starting quests), "
          f"{len(sequence_rows)} sequence groups, {len(kill_rows)} kill-count grades, "
          f"{len(obelisk_rows)} obelisks ({len(guard_rows)} guard groups), {len(boss_rows)} boss groups ({len(follower_rows)} follower "
          f"lines), {len(ai_rows)} monster AIs, {len(string_rows)} AI lines, "
          f"{len(lua_rows)} Lua scripts, {len(house_npc_rows)} guild house NPC keys")
    if dry_run:
        return

    if content_import.decide("world", files, "world", reset=reset, adopt=adopt) == "skip":
        return
    copy("maps", ["id", "size", "map_type", "create_type", "war_type", "blocked", "rebirth_map", "rebirth_pos", "rebirth_light_map", "rebirth_light_pos",
                  "rebirth_fury_map", "rebirth_fury_pos", "weather_state", "weather_rate", "weather_power", "weather_delay",
                  "weather_none_delay", "create_time", "create_min_user", "enter_max_user", "expire_time", "source_file"], maps)
    copy("npcs", ["type", "type_id", "name", "country", "value_a", "value_b", "value_c", "merchant_byte"], npc_rows)
    copy("npc_shop_items", ["npc_type_id", "seq", "item_type", "item_type_id"], shop_rows)
    copy("npc_gates", ["npc_type_id", "seq", "map", "pos_x", "pos_y", "pos_z", "name", "cost"], gate_rows)
    copy("map_npcs", ["id", "map", "seq", "type", "type_id"], map_npcs)
    copy("map_npc_positions", ["map_npc_id", "seq", "pos_x", "pos_y", "pos_z", "direction"], npc_positions)
    copy("mob_areas", ["id", "map", "seq", "x1", "y1", "z1", "x2", "y2", "z2"], areas)
    copy("map_portals", ["map", "seq", "pos_x", "pos_y", "pos_z", "country", "level_min", "level_max", "to_map", "to_x", "to_y", "to_z"],
         portals)
    copy("map_binds", ["map", "seq", "country", "attr", "boxes"], binds)
    copy("mob_area_mobs", ["mob_area_id", "seq", "mob_id", "count"], area_mobs)
    copy("quests", ["id", "name", "level_min", "level_max", "country", "grow", "sex", "jobs", "prev_quest", "prev_quest_2", "prev_quest_3",
                    "unused", "time_min", "time_limit", "repeat_count", "start_type", "start_npc_type", "start_npc_id", "start_item_type",
                    "start_item_id", "start_items", "end_type", "end_npc_type", "end_npc_id", "end_items", "pc_kill_count", "mob_kills",
                    "result_type", "result_user_select"], quest_rows)
    copy("quest_results", ["quest_id", "seq", "need_mob_id", "need_mob_count", "need_item_id", "need_item_count", "unused", "exp", "money",
                           "item_type", "item_type_id", "next_quest"], result_rows)
    copy("item_create", ["set_no", "seq", "grade", "weight"], box_rows)
    copy("item_quest_links", ["item_type", "item_type_id", "started", "ended"], link_rows)
    copy("sequence_groups", ["group_no", "sequence", "grades"], sequence_rows)
    copy("kill_status", ["seq", "faction", "kills", "file_index", "bonus_types", "bonus_values"], kill_rows)
    copy("obelisks", ["seq", "map", "number", "pos_x", "pos_y", "pos_z", "mob_neutral", "mob_light", "mob_fury", "country"], obelisk_rows)
    copy("obelisk_guards", ["obelisk_seq", "seq", "pos_x", "pos_y", "pos_z", "radius", "mob_neutral", "mob_light", "mob_fury", "count"],
         guard_rows)
    copy("boss_groups", ["seq", "number", "map", "stages", "respawn_secs", "portal_map", "portal_id", "pos_x", "pos_y", "pos_z", "mobs",
                         "random_min", "random_max", "random_spots"], boss_rows)
    copy("boss_followers", ["group_seq", "seq", "pos_x", "pos_y", "pos_z", "radius", "mobs", "count"], follower_rows)
    copy("mob_ai", ["ai", "path", "alo", "afs", "ai_text", "ah"], ai_rows)
    copy("ai_strings", ["id", "text", "english"], string_rows)
    copy("mob_lua", ["ai", "source"], lua_rows)
    copy("guild_house", ["id", "buy_money", "keep_etin", *GUILD_HOUSE_RATES], [house_row])
    copy("guild_house_npcs", ["npc_key", "keep_etin", "upgrade_etin"], house_npc_rows)
    copy("guild_house_levels", ["country", "kind", "level", "npc_type_id"], house_level_rows)
    script = ["BEGIN;", f"DROP SCHEMA IF EXISTS {FROZEN} CASCADE;",
              SCHEMA.read_text().replace("CREATE SCHEMA world;", f"CREATE SCHEMA {FROZEN};").replace("world.", f"{FROZEN}.")]
    script += [text for _, text in COPIES]
    script.append(content_import.record_sql("world", FROZEN, re.findall(r"CREATE TABLE world\.(\w+)", SCHEMA.read_text()), source or str(data_dir), files, adopted=adopt))
    script.append("COMMIT;")
    content_import.run_psql(stdin="\n".join(script) + "\n")
    print(f"{FROZEN}: the frozen copy is loaded and recorded (content.imports 'world'; {len(files)} files read)")
    if adopt:
        print("adopted: world was left as it is (compare it with the frozen copy: docs/plans/ext-1-foundation.md section 9 step 4)")
    else:
        content_import.rebuild("world")


if __name__ == "__main__":
    main()
