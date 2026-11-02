#!/bin/bash
# Copies the body of docs/testing/smoke-list.md (everything after its "section 0 starts" marker) into section 0 of docs/testing/live-test-checklist.md.
#   tools/smoke-list-sync.sh
# Section 0 sits between the checklist's header and "## 1."; it is replaced if it is there and added if not.
set -eu
root="$(cd "$(dirname "$0")/.." && pwd)"
python3 - "$root/docs/testing/smoke-list.md" "$root/docs/testing/live-test-checklist.md" <<'PY'
import re, sys
src, dst = sys.argv[1:3]
marker = "<!-- section 0 starts -->\n"
body = open(src, encoding="utf-8").read().split(marker, 1)[1].strip("\n") + "\n"
text = open(dst, encoding="utf-8").read()
section = "## 0. Pre-launch smoke list\n\n" + body + "\n"
if re.search(r"^## 0\. ", text, re.M):
    text = re.sub(r"^## 0\. .*?(?=^## 1\. )", lambda m: section, text, count=1, flags=re.M | re.S)
else:
    text = re.sub(r"^## 1\. ", lambda m: section + "## 1. ", text, count=1, flags=re.M)
open(dst, "w", encoding="utf-8").write(text)
PY
