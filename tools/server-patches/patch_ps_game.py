#!/usr/bin/env python3
"""Patch the original EP4.5 Shaiya game server (ps_game.exe) on disk.

This is a checked, reversible byte patcher for people still running the original
server files. It refuses any file whose SHA-256 is not the one build these patches
were made for, keeps a backup, applies fixes by id, can list and revert them, and
prints exactly what it changed. It never runs the executable.

    patch_ps_game.py list [ps_game.exe]      show every fix and its state
    patch_ps_game.py status <ps_game.exe>    which fixes are applied
    patch_ps_game.py apply  <ps_game.exe> [id ...]
    patch_ps_game.py revert <ps_game.exe> [id ...]|all

`apply` with no ids applies all clear-bug fixes (the default set). Design-choice
fixes are applied only when named. `revert` with no ids reverts every applied fix.

Safety: before any change the tool reconstructs the pristine image by undoing every
patch it recognises; that image must hash to the recorded SHA-256, so the tool only
ever edits the exact build, whether pristine or already carrying some of these fixes.
The byte lists live in fixes.json next to this script; the executable is never
distributed with the kit.
"""
import argparse
import hashlib
import json
import os
import shutil
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
FIXES = os.path.join(HERE, "fixes.json")


def load():
    with open(FIXES) as fh:
        return json.load(fh)


def is_cave(patch):
    return "fill" in patch


def patches_of(fix):
    return fix["inplace"]


def state(data, fix):
    """'applied', 'clean', or 'mixed' for one fix against the file bytes."""
    applied = clean = True
    for p in patches_of(fix):
        off = p["off"]
        cur = data[off:off + len(bytes.fromhex(p["patched"]))]
        if cur != bytes.fromhex(p["patched"]):
            applied = False
        if cur != bytes.fromhex(p["original"]):
            clean = False
    if applied:
        return "applied"
    if clean:
        return "clean"
    return "mixed"


def pristine(doc, data):
    """Return the image with every recognised patch undone, or None if that image is
    not the expected build (unknown edits, or the wrong file)."""
    img = bytearray(data)
    header = doc["header"]
    allpatches = [header] + [p for f in doc["fixes"] for p in patches_of(f)]
    for p in allpatches:
        off = p["off"]
        patched = bytes.fromhex(p["patched"])
        if img[off:off + len(patched)] == patched:
            img[off:off + len(patched)] = bytes.fromhex(p["original"])
    if hashlib.sha256(bytes(img)).hexdigest() != doc["target"]["sha256"]:
        return None
    return img


def read(path):
    with open(path, "rb") as fh:
        return bytearray(fh.read())


def require_build(doc, path):
    data = read(path)
    if len(data) != doc["target"]["size"]:
        sys.exit("%s: size %d, expected %d (not the right file)"
                 % (path, len(data), doc["target"]["size"]))
    if pristine(doc, data) is None:
        sys.exit("%s: not the expected ps_game.exe build, or it has edits this tool "
                 "does not recognise (SHA-256 of the reconstructed original does not "
                 "match %s). Nothing was changed." % (path, doc["target"]["sha256"][:16]))
    return data


def finalize_header(doc, data):
    """The .text VirtualSize bump is on iff at least one cave fix is applied."""
    any_cave = any(
        state(data, f) == "applied" and any(is_cave(p) for p in patches_of(f))
        for f in doc["fixes"])
    h = doc["header"]
    off = h["off"]
    want = bytes.fromhex(h["patched"] if any_cave else h["original"])
    if data[off:off + len(want)] != want:
        data[off:off + len(want)] = want
        return True
    return False


def write_patch(data, p):
    off = p["off"]
    data[off:off + len(bytes.fromhex(p["patched"]))] = bytes.fromhex(p["patched"])


def write_original(data, p):
    off = p["off"]
    data[off:off + len(bytes.fromhex(p["original"]))] = bytes.fromhex(p["original"])


def backup(path):
    bak = path + ".bak"
    if not os.path.exists(bak):
        shutil.copy2(path, bak)
        print("backup: %s" % bak)


def cmd_list(doc, path):
    data = require_build(doc, path) if path else None
    width = max(len(f["id"]) for f in doc["fixes"])
    for f in doc["fixes"]:
        tag = {"clear": "clear bug", "design": "design choice"}.get(f["category"], f["category"])
        st = (" [%s]" % state(data, f)) if data is not None else ""
        dflt = "" if f["default"] else " (opt-in)"
        print("%-*s  %-13s %s%s" % (width, f["id"], tag + dflt, f["status"], st))
        print("%s    %s" % (" " * width, f["symptom"]))


def cmd_status(doc, path):
    data = require_build(doc, path)
    for f in doc["fixes"]:
        print("%-22s %s" % (f["id"], state(data, f)))


def select(doc, ids, default_only):
    by_id = {f["id"]: f for f in doc["fixes"]}
    if ids:
        bad = [i for i in ids if i not in by_id]
        if bad:
            sys.exit("unknown fix id(s): %s" % ", ".join(bad))
        return [by_id[i] for i in ids]
    return [f for f in doc["fixes"] if (f["default"] or not default_only)]


def cmd_apply(doc, path, ids):
    data = require_build(doc, path)
    chosen = select(doc, ids, default_only=True)
    backup(path)
    changed = False
    for f in chosen:
        st = state(data, f)
        if st == "applied":
            print("already applied: %s" % f["id"])
            continue
        if st == "mixed":
            sys.exit("%s is half-applied; revert it first. Nothing more changed." % f["id"])
        for p in patches_of(f):
            write_patch(data, p)
        changed = True
        print("applied: %-22s %s" % (f["id"], f["title"]))
        for p in patches_of(f):
            print("    0x%06x: %s -> %s" % (p["va"], p["original"], p["patched"]))
    if finalize_header(doc, data):
        print("    .text VirtualSize -> 0x120000 (maps the code cave)")
    if changed:
        with open(path, "wb") as fh:
            fh.write(data)
        print("wrote %s" % path)
    else:
        print("no change")


def cmd_revert(doc, path, ids):
    data = require_build(doc, path)
    if ids == ["all"]:
        ids = []
    chosen = select(doc, ids, default_only=False)
    changed = False
    for f in chosen:
        st = state(data, f)
        if st == "clean":
            if ids:
                print("not applied: %s" % f["id"])
            continue
        for p in patches_of(f):
            write_original(data, p)
        changed = True
        print("reverted: %s" % f["id"])
    if finalize_header(doc, data):
        print("    .text VirtualSize restored")
    if changed:
        with open(path, "wb") as fh:
            fh.write(data)
        print("wrote %s" % path)
    else:
        print("no change")


def main():
    doc = load()
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("list"); p.add_argument("exe", nargs="?")
    p = sub.add_parser("status"); p.add_argument("exe")
    p = sub.add_parser("apply"); p.add_argument("exe"); p.add_argument("ids", nargs="*")
    p = sub.add_parser("revert"); p.add_argument("exe"); p.add_argument("ids", nargs="*")
    args = ap.parse_args()
    if args.cmd == "list":
        cmd_list(doc, args.exe)
    elif args.cmd == "status":
        cmd_status(doc, args.exe)
    elif args.cmd == "apply":
        cmd_apply(doc, args.exe, args.ids)
    elif args.cmd == "revert":
        cmd_revert(doc, args.exe, args.ids)


if __name__ == "__main__":
    main()
