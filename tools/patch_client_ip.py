#!/usr/bin/env python3
"""Make a copy of the US client's game.exe that connects to a different login server.

usage: patch_client_ip.py <game.exe> <output.exe> <ipv4>

The client picks its login server from a region table. With no `[LOGIN] VERSION=` in
CONFIG.INI it uses region 6, whose address is the 16-byte string "174.142.118.105" in
.rdata (file offset 0x2a5b34). The connection is then made to that address on port
30800 (0x7850), see docs/original/client-login.md.

Only that string is changed, in a copy. The input must be the exact build this was
written for (SHA-256 below) and the original bytes must match, otherwise nothing is written.
"""
import hashlib
import ipaddress
import sys
from pathlib import Path

EXPECTED_SHA256 = "5f045bcb4a2e29562ac79effc4108fbaa6404ab29258df02c4dfc6ed7e3401a6"
OFFSET = 0x2A5B34
ORIGINAL = b"174.142.118.105\0"
SLOT = 16


def main():
    if len(sys.argv) != 4:
        sys.exit(__doc__)
    source, target, ip = Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3]

    address = ipaddress.IPv4Address(ip)
    text = str(address).encode("ascii")
    if len(text) + 1 > SLOT:
        sys.exit("address does not fit the 16-byte slot")

    data = bytearray(source.read_bytes())
    digest = hashlib.sha256(data).hexdigest()
    if digest != EXPECTED_SHA256:
        sys.exit(f"unexpected game.exe (sha256 {digest}); this patch is only for the build in docs/original/client-login.md")
    if bytes(data[OFFSET:OFFSET + SLOT]) != ORIGINAL:
        sys.exit("original address bytes not found at the expected offset")
    if source.resolve() == target.resolve():
        sys.exit("refusing to overwrite the input file")

    data[OFFSET:OFFSET + SLOT] = text.ljust(SLOT, b"\0")
    target.write_bytes(data)
    changed = sum(1 for a, b in zip(source.read_bytes(), data) if a != b)
    print(f"wrote {target}: login server {address}, {changed} bytes differ from the original")


if __name__ == "__main__":
    main()
