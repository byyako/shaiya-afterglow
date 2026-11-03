#!/usr/bin/env python3
"""Read the PS_GameDefs tables straight out of the original SQL Server backup, without SQL Server.

usage: read_bak.py <PS_GameDefs backup> <out dir> [export_queries.tsv]

Writes the same files `tools/defs/export_from_mssql.sh` writes (data/defs-export/*.psv: pipe-separated, text columns as hex,
NULL as an empty field), so `load_into_postgres.py` takes either. What is exported, and in which column order, comes from
`export_queries.tsv` (the SQL Server export's own queries); this reads the SELECT lists and `ORDER BY`.

How: the backup is a Microsoft Tape Format file holding the database's 8 KB pages (SQL Server 2005, uncompressed). The pages
are located by their headers, the table names, ids and columns are read from the database's own catalogue tables (`sysschobjs`,
`syscolpars`), and each table's data pages are decoded record by record. Supported column types are the ones the game
definitions use (integers, bit, char/varchar/nchar/nvarchar, datetime); anything else stops with a message. Standard library only.
"""
from __future__ import annotations

import re
import struct
import sys
from datetime import datetime, timedelta
from pathlib import Path

PAGE = 8192
HEADER = 96
SYSSCHOBJS = 34
SYSCOLPARS = 41
SYSROWSETS = 5
SYSHOBTCOLUMNS = 13

# syscolpars.xtype -> (name, fixed width or None for variable)
TYPES = {
    48: ("tinyint", 1), 52: ("smallint", 2), 56: ("int", 4), 127: ("bigint", 8), 104: ("bit", 1),
    61: ("datetime", 8), 58: ("smalldatetime", 4), 62: ("float", 8), 59: ("real", 4),
    175: ("char", None), 167: ("varchar", None), 239: ("nchar", None), 231: ("nvarchar", None),
}
FIXED_CHAR = {175, 239}      # char/nchar: fixed width, `length` bytes
VARIABLE = {167, 231}


class BackupError(Exception):
    pass


def find_pages(data: bytes) -> dict[int, bytes]:
    """{page id: the 8 KB page, torn-page bits restored}. The data pages follow the tape header in 8 KB steps; extents the
    backup skipped are zero pages, and the ids come from the page headers."""
    if data[:4] != b"TAPE":
        raise BackupError("not a SQL Server backup (it does not start with a tape header)")
    first = None
    # the first page is the file header page (type 15, page id 0 of file 1)
    for offset in range(0, min(len(data) - PAGE, 64 * 1024), 512):
        if data[offset] == 1 and data[offset + 1] == 15 and struct.unpack_from("<IH", data, offset + 32) == (0, 1):
            first = offset
            break
    if first is None:
        raise BackupError("no database pages found in the backup (compressed or encrypted, or not a SQL Server backup?)")
    pages: dict[int, bytes] = {}
    for offset in range(first, len(data) - PAGE + 1, PAGE):
        if data[offset] != 1:
            continue
        page = bytearray(data[offset:offset + PAGE])
        page_id, file_id = struct.unpack_from("<IH", page, 32)
        if file_id != 1:
            continue
        flags = struct.unpack_from("<H", page, 4)[0]
        if flags & 0x100:                       # torn page detection: the low two bits of each sector's first byte are parked
            torn = struct.unpack_from("<I", page, 60)[0]
            for sector in range(1, 16):          # the last byte of sectors 1 to 15 (sector 0 holds the header)
                position = sector * 512 + 511
                page[position] = (page[position] & ~3) | ((torn >> (2 * sector)) & 3)
        pages[page_id] = bytes(page)
    return pages


def records(page: bytes):
    """The primary records of a data page (forwarding stubs, ghosts and deleted slots skipped)."""
    count = struct.unpack_from("<H", page, 22)[0]
    for slot in range(count):
        offset = struct.unpack_from("<H", page, PAGE - 2 * (slot + 1))[0]
        if offset == 0:
            continue
        kind = (page[offset] >> 1) & 7
        if kind not in (0, 1):                  # 0 primary, 1 forwarded; 2 stub, 3 index, 4 blob, 5 ghost, ...
            continue
        yield offset


def split_record(page: bytes, offset: int, fixed_columns: int):
    """(fixed bytes, null bitmap bytes, [variable column bytes]) of one record."""
    fixed_end = struct.unpack_from("<H", page, offset + 2)[0]
    fixed = page[offset + 4:offset + fixed_end]
    ncols = struct.unpack_from("<H", page, offset + fixed_end)[0]
    bitmap_len = (ncols + 7) // 8
    bitmap = page[offset + fixed_end + 2:offset + fixed_end + 2 + bitmap_len]
    position = offset + fixed_end + 2 + bitmap_len
    variable: list[bytes] = []
    if page[offset] & 0x20:                     # has variable-length columns
        nvar = struct.unpack_from("<H", page, position)[0]
        ends = [struct.unpack_from("<H", page, position + 2 + 2 * i)[0] for i in range(nvar)]
        start = position + 2 + 2 * nvar - offset
        for end in ends:
            variable.append(page[offset + start:offset + end])
            start = end
    return fixed, bitmap, variable, ncols


class Table:
    def __init__(self, name: str, object_id: int, columns: list[dict]):
        self.name = name
        self.object_id = object_id
        self.columns = sorted(columns, key=lambda c: c["colid"])
        for number, column in enumerate(self.columns):
            column["index"] = number
        self.fixed = [c for c in self.columns if c["xtype"] not in VARIABLE]
        self.variable = [c for c in self.columns if c["xtype"] in VARIABLE]

    def check(self) -> None:
        for column in self.columns:
            if column["xtype"] not in TYPES:
                raise BackupError(f"{self.name}.{column['name']}: column type {column['xtype']} is not supported by this reader")

    def width(self, column: dict) -> int:
        fixed = TYPES[column["xtype"]][1]
        if fixed is not None:
            return fixed
        return column["length"] * (2 if column["xtype"] == 239 else 1)   # nchar: length is in bytes already

    def decode(self, page: bytes, offset: int) -> dict:
        fixed, bitmap, variable, ncols = split_record(page, offset, len(self.fixed))
        row: dict = {}
        bit_run = 0
        for column in self.fixed:
            kind = column["xtype"]
            width = self.width(column)
            position = column["offset"] - 4          # the leaf offset counts from the start of the record
            if kind == 104:                          # bit: up to eight columns share a byte
                row[column["name"]] = (fixed[position] >> column["bitpos"]) & 1
                continue
            raw = fixed[position:position + width]
            if len(raw) != width:
                raise BackupError(f"{self.name}: a record is shorter than the table's fixed columns")
            if kind == 48:
                value = raw[0]
            elif kind == 52:
                value = struct.unpack("<h", raw)[0]
            elif kind == 56:
                value = struct.unpack("<i", raw)[0]
            elif kind == 127:
                value = struct.unpack("<q", raw)[0]
            elif kind == 62:
                value = struct.unpack("<d", raw)[0]
            elif kind == 59:
                value = struct.unpack("<f", raw)[0]
            elif kind == 61:                         # datetime: days since 1900-01-01 and 1/300 s ticks, written as bcp does
                ticks, days = struct.unpack("<Ii", raw)   # the time of day comes first
                millis = (ticks * 1000 + 150) // 300
                moment = datetime(1900, 1, 1) + timedelta(days=days, milliseconds=millis)
                value = moment.strftime("%Y-%m-%d %H:%M:%S.") + f"{moment.microsecond // 1000:03d}"
            else:                                    # smalldatetime, char, nchar: kept as bytes
                value = raw
            row[column["name"]] = value
        for column in self.variable:                 # leaf offset -1, -2, ...: the variable slot (dropped columns keep theirs)
            slot = -column["offset"] - 1
            row[column["name"]] = variable[slot] if slot < len(variable) else None
        for column in self.columns:                  # the null bitmap covers every column, by its null bit
            nullbit = column["nullbit"] - 1
            if 0 <= nullbit and nullbit // 8 < len(bitmap) and bitmap[nullbit // 8] >> (nullbit % 8) & 1:
                row[column["name"]] = None
        return row


def catalogue(pages: dict[int, bytes]) -> dict[str, Table]:
    """The user tables with their columns, from sysschobjs and syscolpars."""
    def rows_of(object_id: int):
        for page_id in sorted(pages):
            page = pages[page_id]
            if page[1] == 1 and struct.unpack_from("<I", page, 24)[0] == object_id:
                for offset in records(page):
                    yield page, offset

    objects: dict[int, str] = {}
    for page, offset in rows_of(SYSSCHOBJS):
        if page[offset] & 0x30 != 0x30:
            continue
        fixed, _, variable, _ = split_record(page, offset, 0)
        # id int, nsid int, nsclass tinyint, status int, type char(2), pid int, pclass tinyint, intprop int, created, modified
        object_id = struct.unpack_from("<i", fixed, 0)[0]
        kind = fixed[13:15]
        if kind == b"U " and variable:
            objects[object_id] = variable[0].decode("utf-16le")

    columns: dict[int, list[dict]] = {}
    for page, offset in rows_of(SYSCOLPARS):
        if page[offset] & 0x30 != 0x30:
            continue
        fixed, _, variable, _ = split_record(page, offset, 0)
        # id int, number smallint, colid int, xtype tinyint, utype int, length smallint, prec tinyint, scale tinyint, ...
        object_id, number, colid = struct.unpack_from("<ihi", fixed, 0)
        xtype = fixed[10]
        length = struct.unpack_from("<h", fixed, 15)[0]
        if object_id in objects and number == 0 and variable:
            columns.setdefault(object_id, []).append(
                {"name": variable[0].decode("utf-16le"), "colid": colid, "xtype": xtype, "length": length})
    # where each column sits in the record: sysrowsets maps a rowset to its table, syshobtcolumns holds the leaf offsets
    rowsets: dict[int, tuple[int, int]] = {}
    for page, offset in rows_of(SYSROWSETS):
        fixed, _, _, _ = split_record(page, offset, 0)
        rowset_id, _, object_id, index_id = struct.unpack_from("<qBii", fixed, 0)
        if object_id in objects and index_id <= 1:
            rowsets[rowset_id] = (object_id, index_id)
    physical: dict[tuple[int, int], tuple[int, int, int]] = {}
    for page, offset in rows_of(SYSHOBTCOLUMNS):
        fixed, _, _, _ = split_record(page, offset, 0)
        if len(fixed) < 37:
            continue
        rowset_id, colid = struct.unpack_from("<qi", fixed, 0)
        if rowset_id in rowsets and colid > 0:
            leaf = struct.unpack_from("<h", fixed, 27)[0]       # the leaf offset: from the record start, or -1, -2, ... for variable columns
            physical[(rowsets[rowset_id][0], colid)] = (leaf, 0, colid)
    tables = {}
    for object_id, name in objects.items():
        if object_id not in columns:
            continue
        for column in columns[object_id]:
            leaf, bitpos, _ = physical.get((object_id, column["colid"]), (0, 0, 0))
            column["offset"] = leaf
            column["bitpos"] = bitpos
            column["nullbit"] = column["colid"]
        tables[name] = Table(name, object_id, columns[object_id])
    return tables


def table_rows(pages: dict[int, bytes], table: Table):
    for page_id in sorted(pages):
        page = pages[page_id]
        if page[1] == 1 and struct.unpack_from("<I", page, 24)[0] == table.object_id:
            for offset in records(page):
                yield table.decode(page, offset)


# --- the SQL Server export's queries -------------------------------------------------------------------------------------

def parse_query(query: str):
    """([(column, hex?)], table, order column) from one line of export_queries.tsv."""
    match = re.match(r"SELECT (.*) FROM \[(\w+)\] ORDER BY \[(\w+)\]\s*$", query)
    if not match:
        raise BackupError(f"cannot read the export query: {query[:80]}")
    columns = []
    for item in re.findall(r"CONVERT\(varchar\(max\), CAST\(\[(\w+)\] AS varbinary\(max\)\), 2\)|\[(\w+)\]", match.group(1)):
        hexed, plain = item
        columns.append((hexed or plain, bool(hexed)))
    return columns, match.group(2), match.group(3)


def field(value, hexed: bool) -> str:
    if value is None:
        return ""
    if hexed:
        return value.hex().upper()
    return str(value)


def main() -> int:
    if len(sys.argv) not in (3, 4):
        print(__doc__, file=sys.stderr)
        return 2
    backup, out_dir = Path(sys.argv[1]), Path(sys.argv[2])
    queries = Path(sys.argv[3]) if len(sys.argv) == 4 else Path(__file__).with_name("export_queries.tsv")
    try:
        pages = find_pages(backup.read_bytes())
        tables = catalogue(pages)
        if not tables:
            raise BackupError("the table catalogue could not be read: the file is cut short, damaged or another SQL Server version")
        out_dir.mkdir(parents=True, exist_ok=True)
        for line in queries.read_text(encoding="utf-8").splitlines():
            if not line.strip():
                continue
            name, query = line.split("\t", 1)
            columns, table_name, order = parse_query(query)
            table = tables.get(table_name)
            if table is None:
                raise BackupError(f"the backup has no table {table_name} (found: {', '.join(sorted(tables))})")
            table.check()
            known = {c["name"] for c in table.columns}
            missing = [c for c, _ in columns if c not in known]
            if missing:
                raise BackupError(f"{table_name} has no column {', '.join(missing)}")
            rows = sorted(table_rows(pages, table), key=lambda r: r[order])
            with open(out_dir / f"{name}.psv", "w", encoding="ascii", newline="") as handle:
                for row in rows:
                    handle.write("|".join(field(row[c], hexed) for c, hexed in columns) + "\n")
            print(f"{name}: {len(rows)} rows")
    except BackupError as error:
        print(f"read_bak.py: {error}", file=sys.stderr)
        return 1
    except (struct.error, IndexError, KeyError, ValueError, UnicodeDecodeError, OSError) as error:
        print(f"read_bak.py: the backup is not laid out as this reader expects ({type(error).__name__}: {error}); "
              "a damaged or different backup. Use tools/setup.sh --sqlserver instead.", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
