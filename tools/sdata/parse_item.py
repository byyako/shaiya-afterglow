#!/usr/bin/env python3
"""Turn a decrypted EP4/EP5 Item.SData into TSV, one row per record (docs/client/item-sdata-vs-defs.md).

usage: parse_item.py <decrypted Item.SData> <out.tsv>

Importable: `read_items` and `write_items` (the client import and the patch generator, `tools/sdata/client_patch.py`) read and write the same layout.

Layout: u32 group count, then per group u32 record count and records of u32-length name, u32-length description and 81 fixed bytes
(strings are text + one NUL). After parsing, the file is rebuilt from the parsed values and must come out byte-identical
(docs/client/sdata-generation.md section 1). The description is the last column, with backslash, tab, CR and LF escaped.
"""
import struct, sys
FIX = [('type','B'),('typeid','B'),('model','B'),('icon','B'),('reqlevel','H'),('country','B'),('attackfighter','B'),
 ('defensefighter','B'),('patrolrogue','B'),('shootrogue','B'),('attackmage','B'),('defensemage','B'),('grow','B'),('type2','B'),
 ('type3','B'),('reqstr','H'),('reqdex','H'),('reqrec','H'),('reqint','H'),('reqwis','H'),('reqluc','H'),('reqvg','H'),('reqog','B'),
 ('reqig','B'),('range','B'),('attacktime','B'),('attrib','B'),('special','B'),('slot','B'),('quality','H'),('effect1','H'),
 ('effect2','H'),('effect3','H'),('effect4','H'),('consthp','H'),('constsp','H'),('constmp','H'),('conststr','H'),('constdex','H'),
 ('constrec','H'),('constint','H'),('constwis','H'),('constluc','H'),('speed','B'),('exp','B'),('buy','I'),('sell','I'),('grade','H'),
 ('drop','H'),('server','B'),('count','B')]
FMT = '<' + ''.join(f for _, f in FIX)
assert struct.calcsize(FMT) == 81


def read_items(d):
    """The records of a decrypted Item.SData: a list per group of dicts with `name` and `description` (bytes, without their one NUL)
    and every field of FIX. Every byte must be explained: a string that does not end in exactly one NUL, or bytes left over, raise."""
    o = 0
    def u32():
        nonlocal o
        v = struct.unpack_from('<I', d, o)[0]
        o += 4
        return v
    def text():
        nonlocal o
        n = u32()
        raw = d[o:o + n]
        o += n
        if raw[-1:] != b'\0' or b'\0' in raw[:-1]:
            raise ValueError(f'string at {o - n - 4} is not text and one NUL: {raw!r}')
        return raw[:-1]
    groups = []
    for _ in range(u32()):
        group = []
        for _ in range(u32()):
            rec = {'name': text(), 'description': text()}
            rec.update(zip((n for n, _ in FIX), struct.unpack_from(FMT, d, o)))
            o += 81
            group.append(rec)
        groups.append(group)
    if o != len(d):
        raise ValueError(f'the records end at {o}, the file at {len(d)}')
    return groups


def write_items(groups):
    """The inverse of read_items."""
    text = lambda t: struct.pack('<I', len(t) + 1) + t + b'\0'
    out = bytearray(struct.pack('<I', len(groups)))
    for group in groups:
        out += struct.pack('<I', len(group))
        for rec in group:
            out += text(rec['name']) + text(rec['description']) + struct.pack(FMT, *(rec[n] for n, _ in FIX))
    return bytes(out)


def main():
    d = open(sys.argv[1], 'rb').read()
    groups = read_items(d)
    assert write_items(groups) == d, 'the parsed values do not rebuild the file'
    esc = lambda t: t.replace('\\', '\\\\').replace('\t', '\\t').replace('\r', '\\r').replace('\n', '\\n')
    with open(sys.argv[2], 'w') as f:
        f.write('group\tname\t' + '\t'.join(n for n, _ in FIX) + '\tdescription\n')
        for g, group in enumerate(groups, 1):
            for rec in group:
                f.write(f'{g}\t{esc(rec["name"].decode("cp1252"))}\t' + '\t'.join(str(rec[n]) for n, _ in FIX)
                        + f'\t{esc(rec["description"].decode("cp1252"))}\n')
    print(len(groups), 'groups', sum(map(len, groups)), 'records; rebuilds byte-identical')


if __name__ == '__main__':
    main()
