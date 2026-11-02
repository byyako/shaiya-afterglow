#!/usr/bin/env python3
"""Disassemble functions of a PE by PDB name or address, with call targets and data labels named.

usage: disasm.py <file.exe> <symbols.tsv> <name-or-0xaddress> [...]

The symbols file is the output of tools/pdb_symbols.py. Uses objdump (binutils); never executes the binary.
A name matches every procedure with that exact name (overloads are all printed).
"""
import re
import subprocess
import sys


def load(tsv):
    procs, names = [], {}
    with open(tsv) as fh:
        for line in fh:
            va, size, kind, name = line.rstrip("\n").split("\t")
            a = int(va, 16)
            if kind == "proc":
                procs.append((a, int(size), name))
                names[a] = name
            elif a not in names:
                names[a] = name
    return procs, names


def main():
    exe, tsv, wanted = sys.argv[1], sys.argv[2], sys.argv[3:]
    procs, names = load(tsv)
    for w in wanted:
        if w.startswith("0x"):
            a = int(w, 16)
            hits = [p for p in procs if p[0] == a] or [(a, 0x100, f"sub_{a:x}")]
        else:
            hits = [p for p in procs if p[2] == w]
        if not hits:
            print(f"# {w}: not found")
        for a, size, name in hits:
            print(f"# {name} @ 0x{a:08x} size {size}")
            out = subprocess.run(["objdump", "-d", "-M", "intel", "--no-show-raw-insn",
                                  f"--start-address=0x{a:x}", f"--stop-address=0x{a + max(size, 1):x}", exe],
                                 capture_output=True, text=True, check=True).stdout
            for line in out.splitlines():
                if not re.match(r"\s+[0-9a-f]+:", line):
                    continue

                def label(m):
                    v = int(m.group(0), 16)
                    return f"{m.group(0)} <{names[v]}>" if v in names else m.group(0)
                print(re.sub(r"0x[0-9a-f]{6,8}", label, line))


if __name__ == "__main__":
    main()
