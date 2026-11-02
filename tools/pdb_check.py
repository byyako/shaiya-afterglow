#!/usr/bin/env python3
"""Print the identity (GUID/signature, age) of MSF 7.00 PDB files, and the
named stream count, so they can be matched against an executable's CodeView id.

usage: pdb_check.py <file.pdb> [...]
"""
import struct
import sys

MAGIC = b"Microsoft C/C++ MSF 7.00\r\n\x1aDS\0\0\0"


def read_blocks(fh, block_size, blocks):
    out = bytearray()
    for b in blocks:
        fh.seek(b * block_size)
        out += fh.read(block_size)
    return bytes(out)


def streams(fh):
    head = fh.read(56)
    if not head.startswith(MAGIC):
        raise ValueError("not an MSF 7.00 file")
    block_size, _fpm, num_blocks, dir_size, _res, map_addr = struct.unpack_from("<6I", head, 32)
    dir_blocks_n = (dir_size + block_size - 1) // block_size
    map_blocks_n = (dir_blocks_n * 4 + block_size - 1) // block_size
    fh.seek(map_addr * block_size)
    map_data = fh.read(map_blocks_n * block_size)
    dir_blocks = struct.unpack_from(f"<{dir_blocks_n}I", map_data, 0)
    directory = read_blocks(fh, block_size, dir_blocks)[:dir_size]
    n_streams = struct.unpack_from("<I", directory, 0)[0]
    sizes = struct.unpack_from(f"<{n_streams}I", directory, 4)
    off = 4 + 4 * n_streams
    table = []
    for sz in sizes:
        if sz == 0xFFFFFFFF:
            table.append((sz, []))
            continue
        nb = (sz + block_size - 1) // block_size
        table.append((sz, list(struct.unpack_from(f"<{nb}I", directory, off))))
        off += 4 * nb
    return block_size, table


def identity(path):
    with open(path, "rb") as fh:
        block_size, table = streams(fh)
        sz, blocks = table[1]
        info = read_blocks(fh, block_size, blocks)[:sz]
    version, signature, age = struct.unpack_from("<3I", info, 0)
    guid = struct.unpack_from("<IHH8s", info, 12)
    guid_s = f"{guid[0]:08X}{guid[1]:04X}{guid[2]:04X}{guid[3].hex().upper()}"
    return {"streams": len(table), "version": version, "signature": signature, "age": age, "guid": guid_s}


if __name__ == "__main__":
    for p in sys.argv[1:]:
        try:
            i = identity(p)
            print(f"{p}\tstreams={i['streams']}\tguid={i['guid']}\tage={i['age']}\tid={i['guid']}{i['age']}")
        except Exception as exc:
            print(f"{p}\tERROR {exc}")
