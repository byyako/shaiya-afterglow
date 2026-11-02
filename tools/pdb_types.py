#!/usr/bin/env python3
"""Print struct, class, union and enum layouts from the type stream (TPI) of an MSF 7.00 PDB.

usage: pdb_types.py <file.pdb> <regex> [--list]

<regex> is matched (re.search) against type names. With --list only the matching names and sizes are printed.
Each member is shown with its byte offset, size and type, so packet layouts can be read directly.
Requires tools/pdb_check.py next to it. Reads the PDB only.
"""
import re
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from pdb_check import streams, read_blocks  # noqa: E402

BASIC = {
    0x03: ("void", 0), 0x10: ("char", 1), 0x20: ("uchar", 1), 0x70: ("char", 1), 0x68: ("int8", 1), 0x69: ("uint8", 1),
    0x11: ("short", 2), 0x21: ("ushort", 2), 0x72: ("short", 2), 0x73: ("ushort", 2), 0x71: ("wchar", 2),
    0x12: ("long", 4), 0x22: ("ulong", 4), 0x74: ("int", 4), 0x75: ("uint", 4),
    0x13: ("int64", 8), 0x23: ("uint64", 8), 0x76: ("int64", 8), 0x77: ("uint64", 8),
    0x40: ("float", 4), 0x41: ("double", 8), 0x30: ("bool", 1), 0x31: ("bool16", 2), 0x32: ("bool32", 4),
    0x08: ("HRESULT", 4),
}


def numeric(buf, off):
    (v,) = struct.unpack_from("<H", buf, off)
    if v < 0x8000:
        return v, off + 2
    fmt = {0x8000: "<b", 0x8001: "<h", 0x8002: "<H", 0x8003: "<i", 0x8004: "<I", 0x8009: "<q", 0x800A: "<Q"}[v]
    (val,) = struct.unpack_from(fmt, buf, off + 2)
    return val, off + 2 + struct.calcsize(fmt)


def cstr(buf, off):
    end = buf.index(b"\0", off)
    return buf[off:end].decode("latin1"), end + 1


class Types:
    def __init__(self, pdb):
        with open(pdb, "rb") as fh:
            bs, table = streams(fh)
            sz, blocks = table[2]
            tpi = read_blocks(fh, bs, blocks)[:sz]
        (_ver, hdr, self.begin, self.end, _nbytes) = struct.unpack_from("<5I", tpi, 0)
        self.recs = {}
        off, ti = hdr, self.begin
        while off + 4 <= len(tpi) and ti < self.end:
            (ln, kind) = struct.unpack_from("<HH", tpi, off)
            self.recs[ti] = (kind, tpi[off + 4: off + 2 + ln])
            off += 2 + ln
            ti += 1
        self.by_name = {}
        for ti, (kind, d) in self.recs.items():
            if kind in (0x1504, 0x1505, 0x1506, 0x1507):
                name, prop = self.agg_name(kind, d)
                if not prop & 0x80:  # not a forward reference
                    self.by_name.setdefault(name, ti)

    def agg_name(self, kind, d):
        if kind in (0x1504, 0x1505):
            _cnt, prop = struct.unpack_from("<HH", d, 0)
            _size, o = numeric(d, 16)
            return cstr(d, o)[0], prop
        if kind == 0x1506:
            _cnt, prop = struct.unpack_from("<HH", d, 0)
            _size, o = numeric(d, 8)
            return cstr(d, o)[0], prop
        _cnt, prop = struct.unpack_from("<HH", d, 0)
        return cstr(d, 12)[0], prop

    def resolve(self, ti):
        kind, d = self.recs.get(ti, (None, None))
        if kind in (0x1504, 0x1505, 0x1506, 0x1507):
            name, prop = self.agg_name(kind, d)
            if prop & 0x80 and name in self.by_name:
                return self.by_name[name]
        return ti

    def size(self, ti):
        if ti < self.begin:
            if ti & 0x0F00:
                return 4  # 32-bit pointer to a basic type
            return BASIC.get(ti & 0xFF, ("?", 0))[1]
        ti = self.resolve(ti)
        kind, d = self.recs[ti]
        if kind in (0x1504, 0x1505):
            return numeric(d, 16)[0]
        if kind == 0x1506:
            return numeric(d, 8)[0]
        if kind == 0x1503:
            return numeric(d, 8)[0]
        if kind == 0x1507:
            return self.size(struct.unpack_from("<I", d, 4)[0])
        if kind == 0x1001:
            return self.size(struct.unpack_from("<I", d, 0)[0])
        if kind == 0x1002:
            return 4
        if kind == 0x1205:
            return self.size(struct.unpack_from("<I", d, 0)[0])
        return 0

    def name(self, ti):
        if ti < self.begin:
            base = BASIC.get(ti & 0xFF, (f"basic_{ti:#x}", 0))[0]
            return base + ("*" if ti & 0x0F00 else "")
        ti = self.resolve(ti)
        kind, d = self.recs[ti]
        if kind in (0x1504, 0x1505, 0x1506, 0x1507):
            return self.agg_name(kind, d)[0]
        if kind == 0x1002:
            return self.name(struct.unpack_from("<I", d, 0)[0]) + "*"
        if kind == 0x1001:
            return self.name(struct.unpack_from("<I", d, 0)[0])
        if kind == 0x1503:
            elem = struct.unpack_from("<I", d, 0)[0]
            total = numeric(d, 8)[0]
            es = self.size(elem)
            return f"{self.name(elem)}[{total // es if es else '?'}]"
        if kind == 0x1205:
            base, length, pos = struct.unpack_from("<IBB", d, 0)
            return f"{self.name(base)} : {length} (bit {pos})"
        if kind == 0x1008:
            return "procedure"
        return f"type_{ti:#x}(kind {kind:#x})"

    def fields(self, flist):
        out = []
        while flist:
            kind, d = self.recs[flist]
            assert kind == 0x1203, hex(kind)
            off, flist = 0, None
            while off < len(d):
                if d[off] >= 0xF0:
                    off += d[off] & 0x0F
                    continue
                (sk,) = struct.unpack_from("<H", d, off)
                if sk == 0x150D:  # LF_MEMBER
                    _attr, ty = struct.unpack_from("<HI", d, off + 2)
                    mo, o = numeric(d, off + 8)
                    nm, off = cstr(d, o)
                    out.append(("member", mo, ty, nm))
                elif sk == 0x1400:  # LF_BCLASS
                    _attr, ty = struct.unpack_from("<HI", d, off + 2)
                    mo, off = numeric(d, off + 8)
                    out.append(("base", mo, ty, ""))
                elif sk == 0x1502:  # LF_ENUMERATE
                    v, o = numeric(d, off + 4)
                    nm, off = cstr(d, o)
                    out.append(("enum", v, 0, nm))
                elif sk == 0x150E:  # LF_STMEMBER
                    nm, off = cstr(d, off + 8)
                elif sk == 0x150F:  # LF_METHOD
                    nm, off = cstr(d, off + 8)
                elif sk == 0x1511:  # LF_ONEMETHOD
                    (attr,) = struct.unpack_from("<H", d, off + 2)
                    o = off + 8 + (4 if (attr >> 2) & 7 in (4, 6) else 0)
                    nm, off = cstr(d, o)
                elif sk == 0x1510:  # LF_NESTTYPE
                    nm, off = cstr(d, off + 8)
                elif sk == 0x1409:  # LF_VFUNCTAB
                    off += 8
                elif sk == 0x1404:  # LF_INDEX continuation
                    (flist,) = struct.unpack_from("<I", d, off + 4)
                    off += 8
                else:
                    raise ValueError(f"unknown field leaf {sk:#x}")
        return out

    def dump(self, ti, show):
        kind, d = self.recs[ti]
        name, _ = self.agg_name(kind, d)
        size = self.size(ti)
        word = {0x1504: "class", 0x1505: "struct", 0x1506: "union", 0x1507: "enum"}[kind]
        show(f"{word} {name}  // size {size} (0x{size:x})")
        flist = struct.unpack_from("<I", d, 4 if kind != 0x1507 else 8)[0]
        if kind == 0x1506:
            flist = struct.unpack_from("<I", d, 4)[0]
        for what, mo, ty, nm in self.fields(flist):
            if what == "enum":
                show(f"    {nm} = {mo}")
            elif what == "base":
                show(f"    +0x{mo:04x}  base {self.name(ty)}  (size {self.size(ty)})")
            else:
                show(f"    +0x{mo:04x}  {self.size(ty):5d}  {self.name(ty):28s} {nm}")
        show("")


def main():
    pdb, pattern = sys.argv[1], re.compile(sys.argv[2])
    only_list = "--list" in sys.argv[3:]
    t = Types(pdb)
    for name in sorted(t.by_name):
        if pattern.search(name):
            ti = t.by_name[name]
            if only_list:
                print(f"{t.size(ti):6d}  {name}")
            else:
                t.dump(ti, print)


if __name__ == "__main__":
    main()
