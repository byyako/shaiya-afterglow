#!/usr/bin/env python3
"""Extract function/data symbols from an MSF 7.00 PDB and map them to virtual addresses.

usage: pdb_symbols.py <file.exe> <file.pdb> <out.tsv>

Output columns: va, size, kind, name
  kind = proc (S_GPROC32/S_LPROC32, undecorated name, with size), pub (S_PUB32, decorated), data
Requires pefile (run with the project venv) and tools/pdb_check.py next to it.
Never executes the binary.
"""
import struct
import sys
from pathlib import Path

import pefile

sys.path.insert(0, str(Path(__file__).parent))
from pdb_check import streams, read_blocks  # noqa: E402


def stream_bytes(fh, block_size, table, idx):
    sz, blocks = table[idx]
    if sz == 0xFFFFFFFF:
        return b""
    return read_blocks(fh, block_size, blocks)[:sz]


def cstr(buf, off):
    end = buf.index(b"\0", off)
    return buf[off:end].decode("latin1"), end + 1


def parse_records(data, base=0):
    off = base
    while off + 4 <= len(data):
        (ln, kind) = struct.unpack_from("<HH", data, off)
        if ln < 2:
            break
        yield kind, data[off + 4: off + 2 + ln]
        off += 2 + ln


def main():
    exe, pdb, out = sys.argv[1], sys.argv[2], sys.argv[3]
    pe = pefile.PE(exe, fast_load=True)
    base = pe.OPTIONAL_HEADER.ImageBase
    secs = [s.VirtualAddress for s in pe.sections]

    def va(seg, off):
        if seg == 0 or seg > len(secs):
            return None
        return base + secs[seg - 1] + off

    rows = {}
    with open(pdb, "rb") as fh:
        bs, table = streams(fh)
        dbi = stream_bytes(fh, bs, table, 3)
        (_sig, _ver, _age, _gsi, _build, pubi, _pdbver, symrec, _rbld, modsz, contrib, secmap, srcinfo, tsm, _mfc,
         optdbg, ec, _flags, _machine, _pad) = struct.unpack_from("<iIIHHHHHHiiiiiIiiHHI", dbi, 0)
        symbols = stream_bytes(fh, bs, table, symrec)
        # public symbols
        for kind, rec in parse_records(symbols):
            if kind == 0x110E:  # S_PUB32
                _flags2, off, seg = struct.unpack_from("<IIH", rec, 0)
                name, _ = cstr(rec, 10)
                a = va(seg, off)
                if a:
                    rows[(a, "pub")] = (0, "pub", name)
            elif kind in (0x110C, 0x110D):  # S_LDATA32, S_GDATA32
                _ti, off, seg = struct.unpack_from("<IIH", rec, 0)
                name, _ = cstr(rec, 10)
                a = va(seg, off)
                if a:
                    rows[(a, "data")] = (0, "data", name)
        # per-module procedures
        pos = 64
        end = 64 + modsz
        while pos < end:
            (_u1,) = struct.unpack_from("<I", dbi, pos)
            (symstream,) = struct.unpack_from("<H", dbi, pos + 34)
            (symsize,) = struct.unpack_from("<I", dbi, pos + 36)
            name, p2 = cstr(dbi, pos + 64)
            _obj, p3 = cstr(dbi, p2)
            pos = (p3 + 3) & ~3
            if symstream == 0xFFFF or symsize == 0:
                continue
            mod = stream_bytes(fh, bs, table, symstream)[:symsize]
            for kind, rec in parse_records(mod, 4):
                if kind in (0x1110, 0x110F, 0x1147, 0x1146):  # GPROC32/LPROC32 (+ _ID)
                    (_p, _e, _n, length, _ds, _de, _ti, off, seg) = struct.unpack_from("<IIIIIIIIH", rec, 0)
                    nm, _ = cstr(rec, 35)
                    a = va(seg, off)
                    if a:
                        rows[(a, "proc")] = (length, "proc", nm)
    with open(out, "w") as fh:
        for (a, _k), (size, kind, nm) in sorted(rows.items()):
            fh.write(f"0x{a:08x}\t{size}\t{kind}\t{nm}\n")
    kinds = {}
    for _a, (_s, k, _n) in rows.items():
        kinds[k] = kinds.get(k, 0) + 1
    print(f"{len(rows)} symbols: {kinds}")


if __name__ == "__main__":
    main()
