#!/usr/bin/env python3
"""Check the repository's links and path references. Standard library only; run from anywhere (CI runs it).

    python3 tools/check_links.py

Four checks over the tracked files, each problem printed as `file:line: message`; the exit status is 1 if there is any.
Paths that a file under docs/plans/ names but that do not exist yet are printed as `planned:` notes (plans name files they
will create) and do not fail the run.

1. Markdown links `[text](target)` in every tracked .md file, outside code spans and fences: the target (without its `#anchor`,
   and only if it has a `.` or `/` in it) must be a tracked file or folder, relative to the file's folder. The documentation
   site lays site/content/ over docs/, so a file under docs/ may also resolve against that view, and a file under
   site/content/ resolves from the docs/ root, where the site stages it. Anchors are left to the site build.
2. Path references in every tracked text file (not archives, Cargo.lock, .gitignore (it names ignored paths), or the generated data in docs/original/{symbols,db,
   inventory}/): a token `(docs|tools|crates|db|site)/...` that ends in a file extension or `/` must be a tracked file or
   folder. Skipped: tokens with `<`, `*`, `{` or `NNNN` (patterns), the target of a Markdown link (check 1 has it, relative to
   its file), and lines that say "client repository" or "client repo" (the client's own tools).
3. In .md files, a backticked docs path followed by "section N" (also "sections N and M", "section N.M"): the file must exist and N (or N.M, else its
   first number) must be a numbered heading of it.
4. site/mkdocs.yml's nav: every page is in site/content/ or docs/.
"""
import posixpath
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
GENERATED = ("docs/original/symbols/", "docs/original/db/", "docs/original/inventory/")

files = subprocess.run(["git", "ls-files", "-z"], cwd=ROOT, capture_output=True, check=True).stdout.decode().split("\0")
files = sorted(f for f in files if f)
fileset = set(files)
folders = set()
for f in files:  # every parent of every file is a folder
    d = posixpath.dirname(f)
    while d:
        folders.add(d)
        d = posixpath.dirname(d)

errors = []
planned = []


def report(path, lineno, message, plan_ok=False):
    (planned if plan_ok and path.startswith("docs/plans/") else errors).append((path, lineno, message))


def exists(path):
    return path in fileset or path in folders


def text_of(path):
    try:
        return (ROOT / path).read_bytes().decode("utf-8")
    except (UnicodeDecodeError, FileNotFoundError):
        return None


# --- 1. Markdown links --------------------------------------------------------------------------------------------------
link = re.compile(r"\[[^\]]*\]\(\s*<?([^)\s>]+)>?(?:\s+\"[^\"]*\")?\s*\)")
code_span = re.compile(r"`+[^`]*`+")
fence = re.compile(r"^\s*(```|~~~)")


def link_resolves(md, target):
    """True if `target` (from the file `md`) names a tracked file or folder, directly or through the site's view."""
    here = posixpath.dirname(md)
    if md.startswith("site/content/"):
        # staged at the docs/ root: the file's place there is its path under site/content/
        rel = posixpath.normpath(posixpath.join(posixpath.dirname(md[len("site/content/"):]), target))
        return not rel.startswith("..") and (exists("docs/" + rel) or exists("site/content/" + rel))
    direct = posixpath.normpath(posixpath.join(here, target))
    if exists(direct):
        return True
    if md.startswith("docs/"):
        rel = posixpath.normpath(posixpath.join(posixpath.dirname(md[len("docs/"):]), target))
        return not rel.startswith("..") and exists("site/content/" + rel)
    return False


def check_links(md, lines):
    in_fence = False
    for n, line in enumerate(lines, 1):
        if fence.match(line):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        for m in link.finditer(code_span.sub(lambda s: " " * len(s.group(0)), line)):
            target = m.group(1)
            if re.match(r"^(https?:|mailto:)", target, re.I):
                continue
            target = target.split("#", 1)[0].split("?", 1)[0]
            if not target or not ("." in target or "/" in target):
                continue
            target = target.replace("%20", " ")
            if not link_resolves(md, target):
                report(md, n, f"link to {m.group(1)!r}: no such file or folder")


# --- 2. path references -------------------------------------------------------------------------------------------------
token = re.compile(r"(?<!\]\()(?<![A-Za-z0-9_./~-])((?:docs|tools|crates|db|site)/[A-Za-z0-9_./-]+)")
skip_line = re.compile(r"client repo(?:sitory)?", re.I)


def check_paths(path, lines):
    for n, line in enumerate(lines, 1):
        if skip_line.search(line):
            continue
        for m in token.finditer(line):
            tok = m.group(1)
            nxt = line[m.end():m.end() + 1]
            if nxt in ("<", "*", "{") or "NNNN" in tok:
                continue
            tok = tok.rstrip(".,")  # sentence punctuation
            if tok.endswith("-") or tok.endswith("/..") or "/./" in tok or "//" in tok:
                continue
            if not (tok.endswith("/") or re.search(r"\.[A-Za-z0-9]+$", tok)):
                continue
            if not exists(tok.rstrip("/")):
                report(path, n, f"{tok}: no such file or folder", plan_ok=True)


# --- 3. section references ----------------------------------------------------------------------------------------------
section_ref = re.compile(r"`(docs/[A-Za-z0-9_./-]+\.md)` sections? (\d+(?:\.\d+)*(?:(?:,| and|, and) \d+(?:\.\d+)*)*)")
numbered = re.compile(r"^#{1,6}\s+(?:Part\s+)?(\d+(?:\.\d+)*)(?=[.\s:)]|$)")
_headings = {}


def headings_of(path):
    if path not in _headings:
        text = text_of(path) or ""
        found, in_fence = set(), False
        for line in text.splitlines():
            if fence.match(line):
                in_fence = not in_fence
            elif not in_fence:
                m = numbered.match(line)
                if m:
                    found.add(m.group(1))
        _headings[path] = found
    return _headings[path]


def check_sections(md, lines):
    in_fence = False
    for n, line in enumerate(lines, 1):
        if fence.match(line):
            in_fence = not in_fence
        if in_fence:
            continue
        for m in section_ref.finditer(line):
            target = m.group(1)
            if target not in fileset:
                report(md, n, f"{target}: no such file (section reference)", plan_ok=True)
                continue
            have = headings_of(target)
            for num in re.findall(r"\d+(?:\.\d+)*", m.group(2)):
                if num not in have and num.split(".")[0] not in have:
                    report(md, n, f"{target} has no numbered heading {num}")


# --- 4. the site's nav --------------------------------------------------------------------------------------------------
def check_nav():
    nav_file = "site/mkdocs.yml"
    text = text_of(nav_file)
    if text is None:
        return
    in_nav = False
    for n, line in enumerate(text.splitlines(), 1):
        if re.match(r"^nav:\s*$", line):
            in_nav = True
            continue
        if in_nav and line and not line.startswith((" ", "#")):
            in_nav = False
        if in_nav:
            m = re.search(r"(?:^|\s)\"?([^\s\"]+\.md)\"?\s*$", line)
            if m and not (exists("site/content/" + m.group(1)) or exists("docs/" + m.group(1))):
                report(nav_file, n, f"nav page {m.group(1)}: in neither site/content/ nor docs/")


def main():
    for path in files:
        if path.endswith(".zip") or path in ("Cargo.lock", ".gitignore") or path.startswith(GENERATED):
            continue
        text = text_of(path)
        if text is None:
            continue
        lines = text.splitlines()
        if path.endswith(".md"):
            check_links(path, lines)
            check_sections(path, lines)
        check_paths(path, lines)
    check_nav()
    for path, n, msg in sorted(planned):
        print(f"{path}:{n}: planned: {msg}")
    for path, n, msg in sorted(errors):
        print(f"{path}:{n}: {msg}")
    print(f"check_links: {len(errors)} errors, {len(planned)} planned notes", file=sys.stderr)
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
