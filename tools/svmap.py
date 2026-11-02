#!/usr/bin/env python3
"""Parse server map files (*.svmap) as ps_game.exe's CMap::LoadMap (0x425780) reads them; never executes anything.

usage: svmap.py <file.svmap> [...]          summary per file (fails if a file does not end where the layout says)
       svmap.py --npcs <file.svmap> [...]   also list NPC placements
       svmap.py --json <file.svmap>         everything as JSON

Layout (all little-endian), as read in that order:
  u32 size (side of the map: 2048, 1024, ...), then ceil(size * size / 8) bytes: a bitmap, one bit per unit square
  u32 n, n x { f32 x, y, z; u32; u32 }                                 section A
  u32 n, n x { f32 x, y, z }                                           section B
  u32 n, n x { f32 x1, y1, z1, x2, y2, z2; u32 m; m x { u32 mob, u32 count } }   monster areas
  u32 n, n x { u32 type, u32 type id, u32 m; m x { f32 x, y, z; f32 dir (radians) } }  NPCs
  u32 n, n x { f32 x, y, z; u32 country; u16 level min, max; u32 map; f32 x, y, z }   portals (stMapPortal)
  u32 n, n x { u32 m; u32 country; u32 attr; m x { f32 x1, y1, z1, x2, y2, z2 } }   bind areas (stMapBind, SBoxAxis boxes)
  u32 n, n x { f32 x1, y1, z1, x2, y2, z2; u32 a; u8 b; u8 c; u16 d }  named regions
"""
import json
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

    def raw(self, n):
        chunk = self.data[self.pos:self.pos + n]
        if len(chunk) != n:
            raise ValueError("file ends early")
        self.pos += n
        return chunk


def parse(data):
    r = Reader(data)
    size = r.u32()
    bitmap = r.raw((size * size + 7) // 8)
    out = {"size": size, "set_bits": sum(bin(b).count("1") for b in bitmap)}
    out["a"] = [r.take("3fII") for _ in range(r.u32())]
    out["b"] = [r.take("3f") for _ in range(r.u32())]
    areas = []
    for _ in range(r.u32()):
        box = r.take("6f")
        mobs = [r.take("II") for _ in range(r.u32())]
        areas.append({"box": box, "mobs": mobs})
    out["mob_areas"] = areas
    npcs = []
    for _ in range(r.u32()):
        kind, type_id, count = r.take("III")
        npcs.append({"type": kind, "type_id": type_id, "at": [r.take("4f") for _ in range(count)]})
    out["npcs"] = npcs
    portals = []
    for _ in range(r.u32()):
        x, y, z, country, level_min, level_max, to_map, tx, ty, tz = r.take("3fIHHI3f")
        portals.append({"at": (x, y, z), "country": country, "levels": (level_min, level_max), "map": to_map, "to": (tx, ty, tz)})
    out["portals"] = portals
    binds = []
    for _ in range(r.u32()):
        m, country, attr = r.take("III")
        binds.append({"country": country, "attr": attr, "boxes": [r.take("6f") for _ in range(m)]})
    out["binds"] = binds
    out["regions"] = [r.take("6fIBBH") for _ in range(r.u32())]
    out["trailing"] = len(data) - r.pos
    return out


def main():
    args = sys.argv[1:]
    show_npcs = "--npcs" in args
    as_json = "--json" in args
    files = [a for a in args if not a.startswith("--")]
    bad = 0
    for path in files:
        with open(path, "rb") as fh:
            data = fh.read()
        try:
            m = parse(data)
        except (ValueError, struct.error) as e:
            print(f"{path}: PARSE ERROR {e}")
            bad += 1
            continue
        if as_json:
            print(json.dumps(m))
            continue
        print(f"{path}: size={m['size']} set={100 * m['set_bits'] / max(1, m['size'] ** 2):.0f}% A={len(m['a'])} B={len(m['b'])} "
              f"mob_areas={len(m['mob_areas'])} npcs={len(m['npcs'])} ({sum(len(n['at']) for n in m['npcs'])} placed) "
              f"portals={len(m['portals'])} binds={len(m['binds'])} regions={len(m['regions'])} trailing={m['trailing']}")
        if m["trailing"]:
            bad += 1
        if show_npcs:
            for n in m["npcs"]:
                for x, y, z, d in n["at"]:
                    print(f"    npc type {n['type']} id {n['type_id']} at ({x:.1f}, {y:.1f}, {z:.1f}) dir {d:.3f}")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
