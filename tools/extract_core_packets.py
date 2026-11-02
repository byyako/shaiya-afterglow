#!/usr/bin/env python3
"""Extract a packet catalogue from the Shaiya-Core shared headers.

usage: extract_core_packets.py <shaiya/include dir> <output.md>

Walks network/{login,game,dbAgent,gameLog}, resolves enums and nested structs
across the whole include tree, and writes one table row per struct that
declares an opcode, plus the field layout and packed size where computable.
Run with `python3 -I`.
"""
import re
import sys
from pathlib import Path

PRIM = {
    "uint8_t": 1, "int8_t": 1, "char": 1, "bool": 1, "unsigned char": 1,
    "uint16_t": 2, "int16_t": 2, "short": 2, "unsigned short": 2, "wchar_t": 2,
    "uint32_t": 4, "int32_t": 4, "int": 4, "unsigned int": 4, "float": 4,
    "bool32_t": 4, "tick32_t": 4, "time32_t": 4, "unsigned long": 4,
    "uint64_t": 8, "int64_t": 8, "double": 8,
}

COMMENT_RE = re.compile(r"//[^\n]*")
BLOCK_RE = re.compile(r"/\*.*?\*/", re.S)
ENUM_RE = re.compile(r"enum\s+(?:struct|class)?\s*(\w+)\s*:\s*([\w ]+?)\s*\{", re.S)
STRUCT_RE = re.compile(r"((?:template\s*<[^>]*>\s*)?)struct\s+(\w+)\s*\{(.*?)\n\s*\};", re.S)
FIELD_RE = re.compile(r"^\s*([A-Za-z_][\w:]*(?:<[^;>]*(?:<[^;>]*>[^;>]*)*>)?)\s+(\w+)\s*(?:\[(\w+)\])?\s*(?:\{[^}]*\})?\s*;", re.M)
OPCODE_RE = re.compile(r"uint16_t\s+opcode\s*\{\s*(0x[0-9A-Fa-f]+|\d+)\s*\}")


def strip(text):
    return BLOCK_RE.sub("", text)


USING_RE = re.compile(r"using\s+(\w+)\s*=\s*(?:std::array<\s*([\w ]+?)\s*,\s*(\w+)\s*>|(\w+))\s*;")


def load(root):
    enums, structs = {}, {}
    aliases = {}
    for path in sorted(root.rglob("*.h")):
        raw = path.read_text(errors="replace")
        for m in USING_RE.finditer(raw):
            name, elem, n, other = m.groups()
            aliases[name] = f"Array<{elem}, {n}>" if elem else other
        for m in ENUM_RE.finditer(raw):
            enums[m.group(1)] = m.group(2).strip()
        text = strip(raw)
        lines = text.split("\n")
        for m in STRUCT_RE.finditer(text):
            tmpl, name, body = m.groups()
            # original server name: closest `// X` comment line above the struct
            start = text[: m.start()].rstrip().split("\n")
            note = ""
            for ln in reversed(start[-4:]):
                s = ln.strip()
                if s.startswith("//"):
                    note = s[2:].strip()
                    break
                if s.startswith("#pragma"):
                    continue
                break
            fields = []
            body_nc = COMMENT_RE.sub("", body)
            for f in FIELD_RE.finditer(body_nc):
                ty, nm, arr = f.groups()
                fields.append((re.sub(r"\s+", " ", ty), nm, arr))
            for p in re.finditer(r"PAD\((\w+)\)", body_nc):
                fields.append(("PAD", "pad", p.group(1)))
            op = OPCODE_RE.search(body_nc)
            structs[name] = {
                "file": str(path.relative_to(root)),
                "template": bool(tmpl.strip()),
                "note": note,
                "fields": fields,
                "opcode": int(op.group(1), 0) if op else None,
            }
    return enums, structs, aliases


ALIASES = {}


def size_of(ty, enums, structs, memo, depth=0):
    if depth > 12:
        return None
    ty = ty.replace("shaiya::", "").strip()
    if ty in ALIASES:
        ty = ALIASES[ty]
    if ty in PRIM:
        return PRIM[ty]
    if ty in enums:
        return PRIM.get(enums[ty])
    m = re.fullmatch(r"String<\s*(\w+)\s*>", ty)
    if m:
        return int(m.group(1)) if m.group(1).isdigit() else None
    m = re.fullmatch(r"Array<\s*(.+),\s*(\w+)\s*>", ty)
    if m:
        if not m.group(2).isdigit():
            return None
        inner = size_of(m.group(1), enums, structs, memo, depth + 1)
        return None if inner is None else inner * int(m.group(2))
    if ty in structs:
        return struct_size(ty, enums, structs, memo, depth + 1)
    return None


def struct_size(name, enums, structs, memo, depth=0):
    if name in memo:
        return memo[name]
    s = structs[name]
    if s["template"]:
        memo[name] = None
        return None
    total = 0
    for ty, nm, arr in s["fields"]:
        if ty == "PAD":
            n = int(arr) if arr.isdigit() else None
            if n is None:
                memo[name] = None
                return None
            total += n
            continue
        sz = size_of(ty, enums, structs, memo, depth)
        if sz is None:
            memo[name] = None
            return None
        if arr:
            if not arr.isdigit():
                memo[name] = None
                return None
            sz *= int(arr)
        total += sz
    memo[name] = total
    return total


def main():
    root, out = Path(sys.argv[1]), Path(sys.argv[2])
    enums, structs, aliases = load(root)
    ALIASES.update(aliases)
    memo = {}
    rows = []
    for name, s in structs.items():
        if s["opcode"] is None:
            continue
        parts = Path(s["file"]).parts
        if parts[0] != "network" or len(parts) < 3:
            continue
        service = parts[1]
        direction = parts[2] if parts[2] in ("incoming", "outgoing") else ""
        sz = struct_size(name, enums, structs, memo)
        rows.append((service, s["opcode"], direction, name, s["note"], sz, s))
    rows.sort(key=lambda r: (r[0], r[1], r[2]))

    with out.open("w") as fh:
        fh.write("# Packet catalogue extracted from Shaiya-Core headers\n\n")
        fh.write("Generated by tools/extract_core_packets.py. `incoming` = received by the server, "
                 "`outgoing` = sent by the server. Sizes are packed bytes including the 2-byte opcode, "
                 "excluding the 2-byte length prefix; `?` = not computable from the headers "
                 "(template, symbolic array size or unresolved type).\n\n")
        fh.write("| service | opcode | dir | struct | original name | size | fields |\n")
        fh.write("|---|---|---|---|---|---|---|\n")
        for service, op, d, name, note, sz, s in rows:
            fl = ", ".join(f"{ty} {nm}" + (f"[{arr}]" if arr else "") for ty, nm, arr in s["fields"] if nm != "opcode")
            fh.write(f"| {service} | 0x{op:04X} | {d} | {name} | {note} | {sz if sz is not None else '?'} | {fl} |\n")
    unknown = sum(1 for r in rows if r[5] is None)
    print(f"{len(rows)} opcode structs, {unknown} with unresolved size, {len(enums)} enums, {len(structs)} structs")


if __name__ == "__main__":
    main()
