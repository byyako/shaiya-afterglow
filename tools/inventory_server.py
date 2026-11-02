#!/usr/bin/env python3
"""Static inventory of a directory tree of untrusted Shaiya server files.

usage: inventory_server.py <root> <out_dir>

Never executes anything. Writes:
  <out_dir>/files.csv       one row per file (path, size, sha256, kind)
  <out_dir>/pe.csv          one row per PE file (build time, linker, subsystem, pdb, ...)
  <out_dir>/pe_imports.txt  imported DLLs per PE
  <out_dir>/network_strings.txt  URLs, IPv4 addresses and hostnames found in PE files
Requires pefile (run with the project venv).
"""
import csv
import hashlib
import re
import sys
import datetime
from pathlib import Path

import pefile

URL_RE = re.compile(rb"(?:https?|ftp)://[\x21-\x7e]{4,200}")
IP_RE = re.compile(rb"(?<![\d.])(?:(?:25[0-5]|2[0-4]\d|1?\d?\d)\.){3}(?:25[0-5]|2[0-4]\d|1?\d?\d)(?![\d.])")
HOST_RE = re.compile(rb"\b[a-zA-Z0-9-]{2,40}(?:\.[a-zA-Z0-9-]{2,40})*\.(?:com|net|org|info|biz|cn|kr|ru|tk|xyz|us|de|io|me|cc|ws)\b")
IGNORE_IP = {b"0.0.0.0", b"127.0.0.1", b"255.255.255.255"}


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def kind_of(path):
    with open(path, "rb") as fh:
        head = fh.read(4)
    if head[:2] == b"MZ":
        return "PE"
    return path.suffix.lower().lstrip(".") or "none"


def pe_info(path):
    pe = pefile.PE(str(path), fast_load=False)
    ts = pe.FILE_HEADER.TimeDateStamp
    try:
        built = datetime.datetime.fromtimestamp(ts, datetime.timezone.utc).strftime("%Y-%m-%d %H:%M:%S")
    except (OverflowError, OSError, ValueError):
        built = f"bad({ts})"
    pdb = ""
    pdb_guid = ""
    if hasattr(pe, "DIRECTORY_ENTRY_DEBUG"):
        for d in pe.DIRECTORY_ENTRY_DEBUG:
            e = getattr(d, "entry", None)
            if e is not None and hasattr(e, "PdbFileName"):
                pdb = e.PdbFileName.rstrip(b"\0").decode("latin1")
                pdb_guid = getattr(e, "Signature_String", "")
    ver = {}
    if hasattr(pe, "FileInfo"):
        for fi in pe.FileInfo:
            for entry in fi:
                if hasattr(entry, "StringTable"):
                    for st in entry.StringTable:
                        for k, v in st.entries.items():
                            ver[k.decode("latin1")] = v.decode("latin1", "replace")
    imports = []
    if hasattr(pe, "DIRECTORY_ENTRY_IMPORT"):
        for imp in pe.DIRECTORY_ENTRY_IMPORT:
            imports.append(imp.dll.decode("latin1"))
    sections = []
    for s in pe.sections:
        sections.append(f"{s.Name.rstrip(b'\0').decode('latin1')}:{s.SizeOfRawData}:{s.get_entropy():.2f}")
    info = {
        "machine": hex(pe.FILE_HEADER.Machine),
        "is_dll": bool(pe.FILE_HEADER.Characteristics & 0x2000),
        "built_utc": built,
        "linker": f"{pe.OPTIONAL_HEADER.MajorLinkerVersion}.{pe.OPTIONAL_HEADER.MinorLinkerVersion}",
        "subsystem": pe.OPTIONAL_HEADER.Subsystem,
        "image_base": hex(pe.OPTIONAL_HEADER.ImageBase),
        "entry_rva": hex(pe.OPTIONAL_HEADER.AddressOfEntryPoint),
        "sections": ";".join(sections),
        "max_entropy": max((s.get_entropy() for s in pe.sections), default=0.0),
        "pdb_path": pdb,
        "pdb_id": pdb_guid,
        "file_version": ver.get("FileVersion", ""),
        "product": ver.get("ProductName", ""),
        "company": ver.get("CompanyName", ""),
        "original_name": ver.get("OriginalFilename", ""),
        "overlay_bytes": max(0, path.stat().st_size - pe.get_overlay_data_start_offset()) if pe.get_overlay_data_start_offset() else 0,
    }
    pe.close()
    return info, imports


def main():
    root, out = Path(sys.argv[1]).resolve(), Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=True)
    files = sorted(p for p in root.rglob("*") if p.is_file())
    net_hits = {}
    pe_rows, imports_out = [], []
    with open(out / "files.csv", "w", newline="") as fh:
        w = csv.writer(fh)
        w.writerow(["path", "size", "sha256", "kind"])
        for p in files:
            rel = str(p.relative_to(root))
            kind = kind_of(p)
            w.writerow([rel, p.stat().st_size, sha256(p), kind])
            if kind == "PE":
                try:
                    info, imports = pe_info(p)
                except pefile.PEFormatError as exc:
                    pe_rows.append({"path": rel, "error": str(exc)})
                    continue
                info["path"] = rel
                pe_rows.append(info)
                imports_out.append((rel, imports))
                data = p.read_bytes()
                found = set()
                for rx in (URL_RE, IP_RE, HOST_RE):
                    for m in rx.finditer(data):
                        s = m.group(0)
                        if s in IGNORE_IP:
                            continue
                        found.add(s.decode("latin1"))
                if found:
                    net_hits[rel] = sorted(found)
    cols = ["path", "machine", "is_dll", "built_utc", "linker", "subsystem", "image_base", "entry_rva",
            "max_entropy", "overlay_bytes", "file_version", "product", "company", "original_name",
            "pdb_path", "pdb_id", "sections", "error"]
    with open(out / "pe.csv", "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=cols, extrasaction="ignore")
        w.writeheader()
        for r in pe_rows:
            w.writerow(r)
    with open(out / "pe_imports.txt", "w") as fh:
        for rel, imps in imports_out:
            fh.write(f"{rel}: {', '.join(imps)}\n")
    with open(out / "network_strings.txt", "w") as fh:
        for rel, hits in sorted(net_hits.items()):
            fh.write(f"## {rel}\n")
            for h in hits:
                fh.write(f"{h}\n")
    print(f"{len(files)} files, {len(pe_rows)} PE files, {len(net_hits)} PE files with network-looking strings")


if __name__ == "__main__":
    main()
