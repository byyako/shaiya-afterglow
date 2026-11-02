#!/usr/bin/env python3
"""Builds the interactive live-test checklist page from docs/testing/live-test-checklist.md.

usage: checklist_page.py <output.html>

tools/checklist_server.py serves the same page live from the dev box (results in a file); this script builds a stand-alone copy.
Each `- [ ]` item becomes a row with an id made from its section and its text (so a ticked result survives edits elsewhere in the
file, but not a rewording of that item). A stand-alone copy does not save results (its "Copy results" button gives the marked items as text); the served page keeps them
through tools/checklist_server.py. Items already ticked in the file (`- [x]`)
show as passed when there is no result for them.

The groups (one client, two clients, hard alone, skip) and the one-client sittings come from docs/testing/live-test-solo.md; the page filters
by them, shows a sitting's setup, and lists open items the file leaves out.
"""
import hashlib
import html
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "docs" / "testing" / "live-test-checklist.md"
SOLO = ROOT / "docs" / "testing" / "live-test-solo.md"
TEMPLATE = Path(__file__).resolve().parent / "checklist_page.html"
GROUPS = {"A": "One client", "B": "Two clients", "C": "Hard alone", "D": "Skip"}


def inline(text):
    """Markdown inline: `code` and **bold**, everything else escaped."""
    out = []
    for i, part in enumerate(re.split(r"(`[^`]*`)", text)):
        if i % 2:
            out.append(f"<code>{html.escape(part[1:-1])}</code>")
        else:
            out.append(re.sub(r"\*\*(.+?)\*\*", r"<strong>\1</strong>", html.escape(part)))
    return "".join(out)


def blocks(lines):
    """Paragraphs, plain lists and tables of a section's own text."""
    out, para, items, rows = [], [], [], []

    def flush():
        if para:
            out.append(f"<p>{inline(' '.join(para))}</p>")
            para.clear()
        if items:
            out.append("<ul>" + "".join(f"<li>{inline(i)}</li>" for i in items) + "</ul>")
            items.clear()
        if rows:
            cells = [[c.strip() for c in r.strip().strip("|").split("|")] for r in rows if not re.match(r"^\|[\s|:-]+\|$", r.strip())]
            head, body = cells[0], cells[1:]
            t = "<thead><tr>" + "".join(f"<th>{inline(c)}</th>" for c in head) + "</tr></thead>"
            t += "<tbody>" + "".join("<tr>" + "".join(f"<td>{inline(c)}</td>" for c in r) + "</tr>" for r in body) + "</tbody>"
            out.append(f'<div class="table"><table>{t}</table></div>')
            rows.clear()

    for line in lines:
        s = line.strip()
        if not s:
            flush()
        elif s.startswith("|"):
            if para or items:
                flush()
            rows.append(s)
        elif s.startswith("- "):
            if para or rows:
                flush()
            items.append(s[2:])
        elif items and line.startswith(" "):
            items[-1] += " " + s
        else:
            if rows:
                flush()
            para.append(s)
    flush()
    return "".join(out)


def item_id(section, text):
    norm = re.sub(r"\s+", " ", text).strip().lower()
    return f"s{section}-{hashlib.sha1(norm.encode()).hexdigest()[:8]}"


def numbers(text):
    """`1–5, 7, 9 to 11` → {1, 2, 3, 4, 5, 7, 9, 10, 11}; parenthesised remarks are left out."""
    out = set()
    for a, b in re.findall(r"(\d+)(?:\s*(?:–|to)\s*(\d+))?", re.sub(r"\([^)]*\)", "", text)):
        out.update(range(int(a), int(b or a) + 1))
    return out


def solo(md, sections):
    """Groups and sittings from docs/testing/live-test-solo.md, added to the items: `groups` (letters of GROUPS) and `sitting` (a number or
    None). Returns the sittings ({num, title, html, setup}) and the problems found (an A item in no sitting or in two, a sitting item
    that is not A), which the page lists."""
    groups = {}  # (section, item) -> set of letters
    sittings, setups, problems = [], {}, []
    part, current_setup = None, None
    for line in md.splitlines():
        if line.startswith("## "):
            part = line[3:].strip()
            m = re.match(r"Sitting (\d+)\b", part)
            current_setup = int(m.group(1)) if m else None
            if current_setup is not None:
                setups[current_setup] = [line[3:].strip()]
            continue
        if current_setup is not None:
            setups[current_setup].append(line)
        elif part == "Every section":
            m = re.match(r"^\| (\d+) \|(.*)\|\s*$", line)
            if m:
                for letter, cell in zip("ABCD", m.group(2).split("|")):
                    for n in numbers(cell):
                        groups.setdefault((int(m.group(1)), n), set()).add(letter)
        elif part == "Group A in sittings":
            m = re.match(r"^(\d+)\. \*\*(.+?)\*\*(.*)$", line)
            if m:
                sittings.append({"num": int(m.group(1)), "title": m.group(2).rstrip(".:"), "lines": [m.group(3).strip()]})
            elif sittings and line.startswith(" ") and line.strip():
                sittings[-1]["lines"].append(line.strip())
    in_a = lambda sec: sorted(n for (s, n), g in groups.items() if s == sec and "A" in g)
    where = {}
    for st in sittings:
        text = " ".join(st["lines"])
        picked = set()
        # `13.1 to 13.5`, `36.12`: single items; taken out first so `18.4 (a mount)` is not read as section 4.
        for a, i, b, j in re.findall(r"(\d+)\.(\d+)(?: to (\d+)\.(\d+))?", text):
            picked.update((int(a), n) for n in range(int(i), int(j or i) + 1))
        text = re.sub(r"\d+\.\d+(?: to \d+\.\d+)?", "", text)
        # `10 (all but 3)`, `51 (1 to 4, 6; …)`, `2 (the self heal)`: a section's A items, all or some.
        for sec, inner in re.findall(r"\b(\d+) \(([^)]*)\)", text):
            sec, head = int(sec), inner.split(";")[0].split(":")[0]
            if head.startswith("all but"):
                picked.update((sec, n) for n in in_a(sec) if n not in numbers(head[7:]))
            elif re.match(r"^\d", head):
                picked.update((sec, n) for n in numbers(head))
            else:
                picked.update((sec, n) for n in in_a(sec))
        for key in sorted(picked):
            if "A" not in groups.get(key, set()):
                problems.append(f"sitting {st['num']} names {key[0]}.{key[1]}, not a one-client item")
            elif key in where:
                problems.append(f"{key[0]}.{key[1]} is in sittings {where[key]} and {st['num']}")
            else:
                where[key] = st["num"]
    for s in sections:
        for n, it in enumerate(s["items"], 1):
            it["groups"] = sorted(groups.get((s["num"], n), set()))
            it["sitting"] = where.get((s["num"], n))
            if "A" in it["groups"] and it["sitting"] is None and not it["done"]:
                problems.append(f"{s['num']}.{n} is a one-client item in no sitting")
    for st in sittings:
        st["html"] = blocks([" ".join(st.pop("lines"))])
        st["setup"] = blocks(setups[st["num"]][1:]) if st["num"] in setups else ""
        if st["num"] in setups:
            st["setup_title"] = setups[st["num"]][0]
    return sittings, problems


def parse(md, solo_md=None):
    lines = md.splitlines()
    intro, sections, current = [], [], None
    i = 0
    while i < len(lines):
        line = lines[i]
        head = re.match(r"^## (\d+)\. (.+)$", line)
        box = re.match(r"^- \[( |x)\] (.*)$", line)
        if head:
            current = {"num": int(head.group(1)), "title": head.group(2).strip(), "intro": [], "items": []}
            sections.append(current)
        elif box and current:
            text = [box.group(2)]
            while i + 1 < len(lines) and lines[i + 1].startswith("  ") and lines[i + 1].strip():
                i += 1
                text.append(lines[i].strip())
            joined = " ".join(text)
            current["items"].append({"id": item_id(current["num"], joined), "text": joined, "html": inline(joined), "done": box.group(1) == "x"})
        elif current is None:
            if not line.startswith("# "):
                intro.append(line)
        else:
            current["intro"].append(line)
        i += 1
    for s in sections:
        s["intro"] = blocks(s["intro"])
    seen = set()
    for s in sections:
        for it in s["items"]:
            if it["id"] in seen:
                raise SystemExit(f"two items share an id in section {s['num']}: {it['text'][:60]}")
            seen.add(it["id"])
    if solo_md is None and SOLO.exists():
        solo_md = SOLO.read_text(encoding="utf-8")
    sittings, problems = solo(solo_md, sections) if solo_md else ([], [])
    return {"intro": blocks(intro), "sections": sections, "groups": GROUPS, "sittings": sittings, "problems": problems}


def main():
    out = Path(sys.argv[1])
    data = parse(SOURCE.read_text(encoding="utf-8"))
    payload = json.dumps(data, ensure_ascii=False).replace("</", "<\\/")
    page = TEMPLATE.read_text(encoding="utf-8").replace("/*DATA*/null", payload)
    out.write_text(page, encoding="utf-8")
    n = sum(len(s["items"]) for s in data["sections"])
    print(f"{out}: {len(data['sections'])} sections, {n} items")


if __name__ == "__main__":
    main()
