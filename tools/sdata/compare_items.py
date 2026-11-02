#!/usr/bin/env python3
"""Compare the client's items (parse_item.py output) with defs.items (docs/client/item-sdata-vs-defs.md).

usage: compare_items.py <client.tsv> <defs.items as TSV with header> <named differences out>

defs.items as TSV: COPY (SELECT * FROM defs.items ORDER BY type, typeid) TO STDOUT WITH (FORMAT csv, HEADER, DELIMITER E'\\t').
Values are compared as the client's 16 bits (defs keeps u16 fields as smallint), buy and sell in full.
"""
import csv, sys, collections
def load(p, q):
    with open(p, newline='', encoding='utf-8', errors='replace') as f:
        return list(csv.DictReader(f, delimiter='\t', quoting=q))
client = [r for r in load(sys.argv[1], csv.QUOTE_NONE) if r['type'] != '0']
server = load(sys.argv[2], csv.QUOTE_MINIMAL)
c = {(int(r['type']), int(r['typeid'])): r for r in client}
s = {(int(r['type']), int(r['typeid'])): r for r in server}
fields = [f for f in client[0] if f in server[0] and f not in ('type', 'typeid')]
wide = {'buy', 'sell'}
norm = lambda f, v: int(v) if f in wide else int(v) & 0xffff
zero = lambda r: all(norm(f, r[f]) == 0 for f in fields)
both = sorted(set(c) & set(s))
client_zero = [k for k in both if zero(c[k])]
cz_named = [k for k in client_zero if not s[k]['itemname'].startswith('???')]
print('both:', len(both), '| client record all zero:', len(client_zero), '(server-named among them:', [(k, s[k]['itemname']) for k in cz_named], ')')
real = [k for k in both if not zero(c[k])]
diffs = collections.defaultdict(list)
for k in real:
    d = [(f, norm(f, s[k][f]), norm(f, c[k][f])) for f in fields if norm(f, s[k][f]) != norm(f, c[k][f])]
    if d: diffs[k] = d
print('items with data on both sides:', len(real), '| differing:', len(diffs))
byfields = collections.defaultdict(list)
for k, d in diffs.items(): byfields[tuple(f for f, _, _ in d)].append(k)
for fs, ks in sorted(byfields.items(), key=lambda x: -len(x[1])):
    print(f'\n[{len(ks)}] fields {fs}')
    for k in ks[:12]:
        print('   ', k, repr(s[k]['itemname']), '/', repr(c[k]['name']), ' '.join(f'{f}: {a}->{b}' for f, a, b in diffs[k]))
    if len(ks) > 12: print('    ...', ks[12:])

print('\n==== SUMMARY ====')
named = {k: d for k, d in diffs.items() if not s[k]['itemname'].startswith('???')}
print('differing named items:', len(named), '| placeholder:', len(diffs) - len(named))
fc = collections.Counter(f for d in named.values() for f, _, _ in d)
print('fields among named:', fc.most_common())
with open(sys.argv[3], 'w') as f:
    for k in sorted(named):
        f.write(f"{k[0]}/{k[1]}\t{s[k]['itemname'].strip()}\t" + ', '.join(f'{fl} {a}->{b}' for fl, a, b in named[k]) + '\n')
