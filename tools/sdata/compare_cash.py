#!/usr/bin/env python3
"""Compare the client's Cash.SData products (client_sdata.py cash TSV) with defs.product_list, by product code
(docs/client/skill-monster-sdata-vs-defs.md section 5).

usage: compare_cash.py <client Cash TSV> <defs.product_list as TSV with header>

defs.product_list as TSV: COPY (SELECT * FROM defs.product_list ORDER BY rowid) TO STDOUT WITH (FORMAT csv, HEADER, DELIMITER E'\\t').
Item ids in both are type * 1000 + type id; the client's items are written as type/typeid x count.
"""
import csv
import sys

with open(sys.argv[1], newline="", encoding="utf-8") as f:
    client = {r["code"]: r for r in csv.DictReader(f, delimiter="\t", quoting=csv.QUOTE_NONE)}
with open(sys.argv[2], newline="", encoding="utf-8") as f:
    server = {r["productcode"]: r for r in csv.DictReader(f, delimiter="\t")}


def server_items(r):
    out = []
    for i in range(1, 25):
        item, count = int(r[f"itemid{i}"] or 0), int(r[f"itemcount{i}"] or 0)
        if item:
            out.append(f"{item // 1000}/{item % 1000}x{count}")
    return " ".join(out)


print(f"client {len(client)} products, defs {len(server)}; only in the client {sorted(set(client) - set(server))}; "
      f"only in defs {sorted(set(server) - set(client))}")
for code in sorted(set(client) & set(server)):
    c, s = client[code], server[code]
    diffs = []
    if int(c["cost"]) != int(s["buycost"]):
        diffs.append(f"cost {s['buycost']} -> {c['cost']}")
    if c["items"] != server_items(s):
        diffs.append(f"items {server_items(s)} -> {c['items']}")
    if diffs:
        print(f"{code} {c['name']}: " + "; ".join(diffs))
