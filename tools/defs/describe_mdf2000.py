#!/usr/bin/env python3
"""Describe a SQL Server 2000 data file (.MDF, database version 539) without SQL Server, as `describe_mssql.sh` describes a
restored database: the same three files, in the same format.

usage: describe_mdf2000.py describe <file.MDF> <out dir> [name]   # <name>.columns.txt, .indexes.txt, .procs.txt
       describe_mdf2000.py boot <file.MDF>                        # the boot page: database name, version, create version
       describe_mdf2000.py proc <file.MDF> <object name>          # the text of one procedure, view, trigger or default

SQL Server 2022 refuses version 539 files (Msg 950), and the EP3 and EP5.4 server packs ship their databases that way
(`docs/original/db/README.md`). The file is read as 8 KB pages, never written. In SQL Server 2000 a page header's object id
names the page's owner, and the system tables have fixed ids: sysobjects 1, sysindexes 2, syscolumns 3, systypes 4,
syscomments 6. syscolumns is decoded first with a built-in layout, which is checked against syscolumns' own rows; the other
system tables are decoded with the layouts syscolumns gives. Row counts come from sysindexes.rowcnt (indid 0 or 1), defaults
from the text of the default constraints, primary key and unique from the sysindexes status bits. Standard library only. The
input is untrusted data: run with `python3 -I`.
"""
from __future__ import annotations
import struct, sys, os
from datetime import datetime, timedelta

PAGE = 8192
SYSOBJECTS, SYSINDEXES, SYSCOLUMNS, SYSTYPES, SYSCOMMENTS = 1, 2, 3, 4, 6


class ReaderError(Exception):
    pass


def load_pages(path):
    with open(path, 'rb') as handle:
        data = handle.read()
    pages = {}
    for number in range(len(data) // PAGE):
        page = bytearray(data[number * PAGE:(number + 1) * PAGE])
        if page[0] != 1:
            continue
        page_id, file_id = struct.unpack_from('<IH', page, 32)
        if page_id != number or file_id != 1:
            continue                                   # unformatted or stale page
        flags = struct.unpack_from('<H', page, 4)[0]
        if flags & 0x100:                              # torn page detection: restore the parked bits
            torn = struct.unpack_from('<I', page, 60)[0]
            for sector in range(1, 16):
                position = sector * 512 + 511
                page[position] = (page[position] & ~3) | ((torn >> (2 * sector)) & 3)
        pages[number] = bytes(page)
    return pages


def slots(page):
    count = struct.unpack_from('<H', page, 22)[0]
    for slot in range(count):
        offset = struct.unpack_from('<H', page, PAGE - 2 * (slot + 1))[0]
        if offset < 96 or offset >= PAGE - 2 * count:
            continue
        yield slot, offset


def split_record(page, offset):
    """(status A, fixed bytes (from record offset 0), null bitmap, column count, [variable bytes]) or None."""
    status = page[offset]
    kind = (status >> 1) & 7
    if kind not in (0, 1) or not status & 0x10:         # primary/forwarded records with a null bitmap only
        return None
    fixed_end = struct.unpack_from('<H', page, offset + 2)[0]
    if fixed_end < 4 or offset + fixed_end + 2 > PAGE:
        return None
    ncols = struct.unpack_from('<H', page, offset + fixed_end)[0]
    bitmap_len = (ncols + 7) // 8
    position = offset + fixed_end + 2 + bitmap_len
    if position > PAGE:
        return None
    variable = []
    if status & 0x20:
        nvar = struct.unpack_from('<H', page, position)[0]
        start = position + 2 + 2 * nvar - offset
        for i in range(nvar):
            end = struct.unpack_from('<H', page, position + 2 + 2 * i)[0] & 0x7fff   # high bit: a LOB pointer
            if offset + end > PAGE or end < start:
                return None
            variable.append(page[offset + start:offset + end])
            start = end
    record = page[offset:offset + fixed_end]
    bitmap = page[offset + fixed_end + 2:offset + fixed_end + 2 + bitmap_len]
    return status, record, bitmap, ncols, variable


def owner_pages(pages, object_id, page_type=1):
    for number in sorted(pages):
        page = pages[number]
        if page[1] == page_type and struct.unpack_from('<I', page, 24)[0] == object_id:
            yield number, page


def datetime_of(raw):
    ticks, days = struct.unpack('<Ii', raw)
    moment = datetime(1900, 1, 1) + timedelta(days=days, milliseconds=(ticks * 1000 + 150) // 300)
    return moment.strftime('%Y-%m-%d %H:%M:%S.') + f'{moment.microsecond // 1000:03d}'


def decode_value(xtype, raw, length, scale=0):
    if xtype == 48: return raw[0]
    if xtype == 52: return struct.unpack('<h', raw)[0]
    if xtype == 56: return struct.unpack('<i', raw)[0]
    if xtype == 127: return struct.unpack('<q', raw)[0]
    if xtype == 104: return raw
    if xtype == 61: return datetime_of(raw)
    if xtype == 58:
        minutes, days = struct.unpack('<HH', raw)
        return (datetime(1900, 1, 1) + timedelta(days=days, minutes=minutes)).strftime('%Y-%m-%d %H:%M:%S')
    if xtype == 62: return struct.unpack('<d', raw)[0]
    if xtype == 59: return struct.unpack('<f', raw)[0]
    if xtype in (60, 122):
        value = struct.unpack('<q', raw)[0] if xtype == 60 else struct.unpack('<i', raw)[0]
        return value / 10000
    return raw                                          # character and binary data, decimals: bytes


class Layout:
    """The physical layout of one table, from its syscolumns rows."""
    def __init__(self, columns):
        # columns: dicts with name, colid, xtype, length, xoffset, bitpos, colstat, xscale
        self.columns = sorted(columns, key=lambda c: c['colid'])
        self.physical = [c for c in self.columns if not c['colstat'] & 4]   # colstat 4: computed

    def decode(self, page, offset):
        parts = split_record(page, offset)
        if parts is None:
            return None
        status, record, bitmap, ncols, variable = parts
        row = {}
        for column in self.physical:
            xoffset = column['xoffset']
            if xoffset > 0:
                if column['xtype'] == 104:
                    raw = record[xoffset:xoffset + 1]
                    row[column['name']] = (raw[0] >> column['bitpos']) & 1 if raw else None
                    continue
                width = column['length']
                raw = record[xoffset:xoffset + width]
                if len(raw) != width:
                    row[column['name']] = None          # a column added after this record was written
                    continue
                row[column['name']] = decode_value(column['xtype'], raw, width, column.get('xscale', 0))
            elif xoffset < 0:
                slot = -xoffset - 1
                row[column['name']] = variable[slot] if slot < len(variable) else None
            else:
                row[column['name']] = None
            bit = column['colid'] - 1
            if bit // 8 < len(bitmap) and bitmap[bit // 8] >> (bit % 8) & 1:
                row[column['name']] = None
        return row


# syscolumns describes itself; its own layout is fixed (read from a SQL 2000 file and checked against its own rows below)
SYSCOLUMNS_BOOT = [
    dict(name='name', colid=1, xtype=231, length=256, xoffset=-1),
    dict(name='id', colid=2, xtype=56, length=4, xoffset=4),
    dict(name='xtype', colid=3, xtype=48, length=1, xoffset=8),
    dict(name='typestat', colid=4, xtype=48, length=1, xoffset=9),
    dict(name='xusertype', colid=5, xtype=52, length=2, xoffset=10),
    dict(name='length', colid=6, xtype=52, length=2, xoffset=12),
    dict(name='xprec', colid=7, xtype=48, length=1, xoffset=14),
    dict(name='xscale', colid=8, xtype=48, length=1, xoffset=15),
    dict(name='colid', colid=9, xtype=52, length=2, xoffset=16),
    dict(name='xoffset', colid=10, xtype=52, length=2, xoffset=18),
    dict(name='bitpos', colid=11, xtype=48, length=1, xoffset=20),
    dict(name='reserved', colid=12, xtype=48, length=1, xoffset=21),
    dict(name='colstat', colid=13, xtype=52, length=2, xoffset=22),
    dict(name='cdefault', colid=14, xtype=56, length=4, xoffset=24),
    dict(name='domain', colid=15, xtype=56, length=4, xoffset=28),
    dict(name='number', colid=16, xtype=52, length=2, xoffset=32),
    dict(name='colorder', colid=17, xtype=52, length=2, xoffset=34),
    dict(name='autoval', colid=18, xtype=165, length=8000, xoffset=-2),
    dict(name='offset', colid=19, xtype=52, length=2, xoffset=36),
]
for _c in SYSCOLUMNS_BOOT:
    _c.update(bitpos=0, colstat=0)


class Database:
    def __init__(self, path):
        self.path = path
        self.pages = load_pages(path)
        if 9 not in self.pages:
            raise ReaderError('no boot page (page 9 of file 1)')
        self.boot = self.read_boot()
        boot_layout = Layout(SYSCOLUMNS_BOOT)
        self.syscolumns = []
        for _, page in owner_pages(self.pages, SYSCOLUMNS):
            for _, offset in slots(page):
                row = boot_layout.decode(page, offset)
                if row and row['name'] is not None:
                    row['name'] = row['name'].decode('utf-16le')
                    self.syscolumns.append(row)
        self.layouts = {}
        for object_id in (SYSOBJECTS, SYSINDEXES, SYSCOLUMNS, SYSTYPES, SYSCOMMENTS):
            cols = [c for c in self.syscolumns if c['id'] == object_id and c['number'] == 0]
            if not cols:
                raise ReaderError(f'syscolumns has no rows for system table {object_id}')
            self.layouts[object_id] = Layout(cols)
        # self-check: the syscolumns layout read from syscolumns must equal the boot layout
        own = {c['name']: c['xoffset'] for c in self.layouts[SYSCOLUMNS].physical}
        for c in SYSCOLUMNS_BOOT:
            if own.get(c['name']) != c['xoffset']:
                raise ReaderError(f"syscolumns.{c['name']}: offset {own.get(c['name'])} in the file, {c['xoffset']} expected")
        self.objects = self.table_rows(SYSOBJECTS)
        for row in self.objects:
            row['name'] = row['name'].decode('utf-16le')
            row['xtype'] = row['xtype'].decode('ascii').strip()
        self.by_id = {o['id']: o for o in self.objects}
        self.indexes = self.table_rows(SYSINDEXES)
        for row in self.indexes:
            row['name'] = row['name'].decode('utf-16le') if row['name'] is not None else None
        self.types = self.table_rows(SYSTYPES)
        for row in self.types:
            row['name'] = row['name'].decode('utf-16le')
        self.type_name = {t['xusertype']: t['name'] for t in self.types}
        self.comments = self.table_rows(SYSCOMMENTS)

    def read_boot(self):
        page = self.pages[9]
        offset = next(slots(page))[1]
        version, create = struct.unpack_from('<HH', page, offset + 4)
        raw = page[offset + 0x34:offset + 0x34 + 256]
        # the name field is padded with single-byte spaces, which decode in pairs as U+2020 ('†'): cut there
        name = raw.decode('utf-16le', 'replace').split('\u2020')[0].rstrip(' \x00')
        return {'version': version, 'create_version': create, 'name': name}

    def table_rows(self, object_id, layout=None):
        layout = layout or self.layouts[object_id]
        out = []
        for _, page in owner_pages(self.pages, object_id):
            for _, offset in slots(page):
                row = layout.decode(page, offset)
                if row is not None:
                    out.append(row)
        return out

    def text_of(self, object_id):
        parts = sorted((c for c in self.comments if c['id'] == object_id), key=lambda c: (c['number'], c['colid']))
        text = []
        for c in parts:
            if c['status'] & 1:                         # status 1: encrypted (WITH ENCRYPTION)
                return None
            raw = c['ctext'] or b''
            # status 2: "compressed", i.e. stored as 8-bit text in the database code page (the text column's definition
            # calls uncompress()); these databases hold Korean, so cp949. Otherwise UTF-16.
            text.append(raw.decode('cp949', 'replace') if c['status'] & 2 else raw.decode('utf-16le', 'replace'))
        return ''.join(text) if parts else None


def sort_key(name):
    """The order SQL Server's case-insensitive Windows collations give these names: case ignored, punctuation before
    digits before letters (checked against every existing description file)."""
    out = []
    for ch in name:
        c = ch.lower()
        out.append((3 if c.isalpha() else 2 if c.isdigit() else 1, c))
    return out


BASE_LENGTH = {'char', 'varchar', 'binary', 'varbinary'}
BASE_NLENGTH = {'nchar', 'nvarchar'}
BASE_DECIMAL = {'decimal', 'numeric'}


def type_text(db, column):
    name = db.type_name.get(column['xusertype'])
    if name is None:
        raise ReaderError(f"column {column['name']}: unknown type {column['xusertype']}")
    if name in BASE_LENGTH:
        return f"{name}({column['length']})"
    if name in BASE_NLENGTH:
        return f"{name}({column['length'] // 2})"
    if name in BASE_DECIMAL:
        return f"{name}({column['xprec']},{column['xscale']})"
    return name


def describe(db):
    """(columns lines, indexes lines, procs lines, notes)"""
    notes = []
    tables = sorted((o for o in db.objects if o['xtype'] == 'U'), key=lambda o: sort_key(o['name']))
    rowcount = {}
    for index in db.indexes:
        if index['indid'] in (0, 1):
            rowcount[index['id']] = rowcount.get(index['id'], 0) + index['rowcnt']
    columns_out = []
    for table in tables:
        cols = sorted((c for c in db.syscolumns if c['id'] == table['id'] and c['number'] == 0), key=lambda c: c['colid'])
        if not cols:
            notes.append(f"table {table['name']} has no columns")
        for c in cols:
            default = ''
            if c['cdefault']:
                obj = db.by_id.get(c['cdefault'])
                if obj is None:
                    notes.append(f"{table['name']}.{c['name']}: default object {c['cdefault']} missing")
                elif obj['parent_obj'] != table['id']:
                    notes.append(f"{table['name']}.{c['name']}: bound default {obj['name']} (not a constraint; 2022 shows none)")
                else:
                    text = db.text_of(obj['id'])
                    default = text if text is not None else ''
            columns_out.append('|'.join([
                table['name'], str(c['colid']), c['name'], type_text(db, c),
                'NOT NULL' if c['typestat'] & 1 else 'NULL',
                'IDENTITY' if c['colstat'] & 1 else '',
                default, str(rowcount.get(table['id'], 0))]))
    table_ids = {t['id']: t for t in tables}
    constraint_names = {o['name']: o['xtype'] for o in db.objects if o['xtype'] in ('PK', 'UQ')}
    index_rows = []
    for index in db.indexes:
        if index['id'] not in table_ids or not 1 <= index['indid'] <= 250:
            continue
        status = index['status']
        if status & 64:                                 # statistics (auto-created _WA_Sys_ or CREATE STATISTICS)
            continue
        if status & 32:
            notes.append(f"hypothetical index {index['name']} on {table_ids[index['id']]['name']}")
        pk = 1 if status & 0x800 else 0
        if pk != (constraint_names.get(index['name']) == 'PK'):
            notes.append(f"index {index['name']}: status says pk={pk}, sysobjects says {constraint_names.get(index['name'])}")
        unique = 1 if status & 2 else 0
        kind = 'CLUSTERED' if index['indid'] == 1 else 'NONCLUSTERED'
        index_rows.append((sort_key(table_ids[index['id']]['name']), sort_key(index['name']),
                           '|'.join([index['name'], table_ids[index['id']]['name'], str(pk), str(unique), kind])))
    indexes_out = [r[2] for r in sorted(index_rows, key=lambda r: (r[0], r[1]))]
    procs_out = [o['name'] for o in sorted((o for o in db.objects if o['xtype'] == 'P'), key=lambda o: sort_key(o['name']))]
    return columns_out, indexes_out, procs_out, notes


USAGE = __doc__.split('usage: ', 1)[1].split('\n\n', 1)[0]


def main(argv):
    if len(argv) < 3 or argv[1] not in ('describe', 'boot', 'proc') \
            or (argv[1] == 'describe' and len(argv) not in (4, 5)) or (argv[1] == 'proc' and len(argv) != 4):
        print('usage: ' + USAGE, file=sys.stderr)
        return 2
    try:
        db = Database(argv[2])
    except (OSError, ReaderError) as error:
        print(f'{argv[2]}: {error}', file=sys.stderr)
        return 1
    if argv[1] == 'boot':
        print(db.boot)
    elif argv[1] == 'describe':
        out = argv[3]
        name = argv[4] if len(argv) > 4 else db.boot['name']
        os.makedirs(out, exist_ok=True)
        cols, idx, procs, notes = describe(db)
        for suffix, lines in (('columns', cols), ('indexes', idx), ('procs', procs)):
            with open(os.path.join(out, f'{name}.{suffix}.txt'), 'w', encoding='utf-8', newline='\n') as handle:
                handle.write(''.join(line + '\n' for line in lines))
        print(f"{name}: {len(cols)} columns, {len(idx)} indexes, {len(procs)} procedures")
        for note in notes:
            print('  note:', note)
    else:
        obj = next((o for o in db.objects if o['name'] == argv[3]), None)
        if obj is None:
            print(f'{argv[3]}: no such object', file=sys.stderr)
            return 1
        text = db.text_of(obj['id'])
        print(text if text is not None else '(encrypted or no text)')
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv))
