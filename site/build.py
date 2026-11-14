#!/usr/bin/env python3
"""Build the public docs site.

Publishes exactly the pages listed in mkdocs.yml's nav, so nothing reaches the site by accident. A nav entry
is taken from content/ when a file of that path exists there (pages written for the site), and from docs/
otherwise. Backticked references to published pages (`docs/original/shop.md`) become links; backticked paths of other files
and folders of the repository (`docs/plans/roadmap.md`, `crates/game-server`) link to them on GitHub.
The server patch kit is zipped into downloads/ and its byte lists are written into the kit page.
Output: `dist/` beside this file (static files, relative links, servable from any path).
"""
import hashlib, json, os, re, shutil, subprocess, sys, time, zipfile
from pathlib import Path

import yaml

HERE = Path(__file__).resolve().parent
DOCS = HERE.parent / "docs"
STAGE = HERE / "build" / "docs"


def pages(node):
    if isinstance(node, str):
        yield node
    elif isinstance(node, list):
        for n in node:
            yield from pages(n)
    elif isinstance(node, dict):
        for v in node.values():
            yield from pages(v)


# the nav is plain YAML, so safe_load is enough
nav = yaml.safe_load((HERE / "mkdocs.yml").read_text(encoding="utf-8"))["nav"]
CONTENT = HERE / "content"
# a nav entry comes from content/ when it has a file of that path (a page written for the site), else from docs/
published = list(pages(nav))
source = {p: CONTENT / p if (CONTENT / p).is_file() else DOCS / p for p in published}
missing = [p for p in published if not source[p].is_file()]
if missing:
    sys.exit(f"nav lists files that are in neither content/ nor docs/: {missing}")

shutil.rmtree(HERE / "build", ignore_errors=True)
STAGE.mkdir(parents=True)
for f in CONTENT.iterdir():  # extra.css and the like; the pages are written below
    if f.is_file() and f.suffix != ".md":
        shutil.copy(f, STAGE / f.name)

fence = re.compile(r"^(```|~~~)")
item = re.compile(r"^(\s*)([-*]|\d+\.) ")
code_span = re.compile(r"(`+[^`]*`+)")
raw_tag = re.compile(r"<(?=[A-Za-z/!?])")


def escape_tags(line):
    """The notes write things like "<p> rejected the invitation" in prose; Markdown would take <p> for HTML and eat the text."""
    parts = code_span.split(line)
    return "".join(x if i % 2 else raw_tag.sub("&lt;", x) for i, x in enumerate(parts))


def spaced(lines):
    """GitHub-style Markdown to what Python-Markdown reads: blank lines before lists and tables that follow a
    paragraph line, nested lists at four spaces per level, raw '<' escaped."""
    out, in_fence, delta, stack, prev_blank = [], False, 0, [], True  # stack: content columns of the open list items
    in_note = False  # inside a top-level "!!! note" block, whose body is indented four spaces and kept as it is
    for line in lines:
        text = line.lstrip()
        indent = len(line) - len(text)
        if in_note and not in_fence and (not text or indent >= 4):
            out.append(line)
            prev_blank = not text
            continue
        in_note = False
        if not in_fence and not indent and text.startswith(("!!! ", "??? ")):
            stack.clear()
            if out and not prev_blank:
                out.append("")
            out.append(text)
            in_note, prev_blank = True, False
            continue
        if fence.match(text):
            if not in_fence:
                delta = (4 * len(stack) if stack and indent else indent) - indent
            in_fence = not in_fence
            out.append(" " * max(indent + delta, 0) + text)
            prev_blank = False
            continue
        if in_fence:
            out.append(" " * max(indent + delta, 0) + text if text else "")
            continue
        if not text:
            out.append("")
            prev_blank = True
            continue
        m = item.match(line)
        if m:
            in_list = bool(stack)  # a lazy continuation line of an item, not a paragraph before the list
            while stack and indent < stack[-1]:
                stack.pop()
            if out and not prev_blank and not in_list and not item.match(out[-1]) and not out[-1].lstrip().startswith(("|", "#", ">")):
                out.append("")
            out.append(" " * (4 * len(stack)) + escape_tags(text))
            stack.append(indent + len(m.group(0)) - len(m.group(1)))
        else:
            if text.startswith("#"):
                stack.clear()
            if prev_blank:
                while stack and indent < stack[-1]:
                    stack.pop()
            if text.startswith("|") and out and not prev_blank and not out[-1].lstrip().startswith("|"):
                out.append("")
            out.append(" " * (4 * len(stack)) + escape_tags(text))
        prev_blank = False
    return out


# every spelling the notes use for a published page -> its path
aliases = {}
for p in published:
    aliases[p] = aliases["docs/" + p] = p
    if p.startswith("client/"):
        aliases[p[len("client/"):]] = p
ref = re.compile(r"`(" + "|".join(re.escape(a) for a in sorted(aliases, key=len, reverse=True)) + r")`")

# Every other tracked file or folder named in backticks links to the public repository. Only paths with a '/', so a bare
# `README.md` or `server.rs` stays code; lines about the client's own repository keep its paths as code (as tools/check_links.py does).
REPO_URL = "https://github.com/byyako/shaiya-afterglow"
tracked = subprocess.run(["git", "ls-files", "-z"], cwd=HERE.parent, capture_output=True, text=True).stdout.split("\0")
tracked_files = {f for f in tracked if f}
tracked_dirs = {p.as_posix() for f in tracked_files for p in Path(f).parents if p.as_posix() != "."}
repo_path = re.compile(r"(?<!\[)`([A-Za-z0-9_.-]+(?:/[A-Za-z0-9_.-]+)+/?)`")


def github(m):
    path = m.group(1).rstrip("/")
    if path in tracked_files:
        return f"[`{m.group(1)}`]({REPO_URL}/blob/main/{path})"
    if path in tracked_dirs:
        return f"[`{m.group(1)}`]({REPO_URL}/tree/main/{path})"
    return m.group(0)


CLIENT_NOTE = (
    '!!! info "Client these notes describe"\n'
    "    US `game.exe` built 2010-04-26, patch level 108 (`ps0108.patch`), EP4.5. "
    "SHA-256 `5f045bcb4a2e29562ac79effc4108fbaa6404ab29258df02c4dfc6ed7e3401a6`. "
    "Other builds may differ.\n\n"
)

# The server patch kit (tools/server-patches/) goes out as a zip: the patcher, fixes.json and site/kit/README.txt. The repo's
# credits name the project and its docs/ files, so the public fixes.json says only where the finding came from; the build
# stops if a project name or path is still in what is shipped.
KIT = HERE.parent / "tools" / "server-patches"
KIT_ZIP = "downloads/ps_game-patch-kit.zip"
KIT_PRIVATE = re.compile(r"NewShaiya|Shaiya Afterglow|Rusty|docs/|reference/|/media/|new server", re.I)
# these five describe the SQL text the quote reaches; the site names the symptom only
KIT_SYMPTOMS = {
    "character-name-filter": "An apostrophe in a character name (create or rename) reaches the database unescaped.",
    "guild-text-filter": "An apostrophe in a guild name or remark reaches the database unescaped.",
    "gift-target-filter": "An apostrophe in a gift's target name reaches the database unescaped.",
    "gm-notice-filter": "An apostrophe in a GM notice reaches the game log's database unescaped.",
    "gm-target-filter": "An apostrophe in a GM command's target name reaches the game log's database unescaped.",
}


def public_kit():
    doc = json.loads((KIT / "fixes.json").read_text(encoding="utf-8"))
    for f in doc["fixes"]:
        f["credit"] = re.sub(r"^(?:NewShaiya|Shaiya Afterglow) analysis \([^)]*\)", "found in disassembly of this build", f["credit"])
        f["symptom"] = KIT_SYMPTOMS.get(f["id"], f["symptom"])
    files = {
        "ps_game-patch-kit/patch_ps_game.py": (KIT / "patch_ps_game.py").read_bytes(),
        "ps_game-patch-kit/fixes.json": (json.dumps(doc, indent=2) + "\n").encode(),
        "ps_game-patch-kit/README.txt": (HERE / "kit" / "README.txt").read_bytes(),
    }
    for name, data in files.items():
        if KIT_PRIVATE.search(data.decode()):
            sys.exit(f"kit: {name} still names the project or a private path: {KIT_PRIVATE.search(data.decode()).group(0)!r}")
    # the same files give the same zip, so rsync only uploads it when the kit changes
    stamp = subprocess.run(["git", "log", "-1", "--format=%ct", "--", str(KIT)], capture_output=True, text=True).stdout.strip()
    when = time.gmtime(int(stamp))[:6] if stamp else (2026, 1, 1, 0, 0, 0)
    (STAGE / KIT_ZIP).parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(STAGE / KIT_ZIP, "w", zipfile.ZIP_DEFLATED) as z:
        for name, data in files.items():
            info = zipfile.ZipInfo(name, when)
            info.external_attr = (0o755 if name.endswith(".py") else 0o644) << 16
            info.compress_type = zipfile.ZIP_DEFLATED
            z.writestr(info, data)
    return doc, hashlib.sha256((STAGE / KIT_ZIP).read_bytes()).hexdigest()


def hexlines(h, prefix):
    b = bytes.fromhex(h)
    rows = [b[i:i + 16].hex(" ") for i in range(0, len(b), 16)] or [""]
    return [prefix + rows[0]] + [" " * len(prefix) + r for r in rows[1:]]


def kit_tables(doc):
    """Every fix of the kit with its full before/after bytes, for the kit page."""
    out = []
    for group, title in (("clear", "Clear-bug fixes (applied by default)"), ("design", "Design-choice fixes (applied only when named)")):
        out += [f"### {title}", ""]
        for f in (f for f in doc["fixes"] if f["category"] == group):
            out += [f"#### `{f['id']}`", "", f"**{f['title']}.** {f['symptom']}", "",
                    f"Status: {f['status']}. Credit: {f['credit']}. The bug: [{f['id']}](original/community-fixes.md#{f['id']}).", "", "```text"]
            for p in f["inplace"]:
                where = "code cave" if "fill" in p else "in place"
                out.append(f"VA 0x{p['va']:06x}  file offset 0x{p['off']:06x}  {len(bytes.fromhex(p['patched']))} bytes, {where}")
                out += hexlines(p["original"], "  before ") + hexlines(p["patched"], "  after  ")
            out += ["```", ""]
    h = doc["header"]
    out += ["### The header field", "", "Changed while any code-cave fix is applied, restored when the last one is reverted:", "", "```text",
            f"VA 0x{h['va']:06x}  file offset 0x{h['off']:06x}  4 bytes, {h['note']}",
            *hexlines(h["original"], "  before "), *hexlines(h["patched"], "  after  "), "```"]
    return "\n".join(out)


kit_doc, kit_sha = public_kit()
kit_tokens = {
    "<!-- kit:fixes -->": kit_tables(kit_doc),
    "KIT_SHA256": kit_sha,
    "KIT_ZIP": KIT_ZIP,
    "KIT_COUNT": str(len(kit_doc["fixes"])),
}

for p in published:
    out, in_fence = [], False
    text = source[p].read_text(encoding="utf-8")
    for old, new in kit_tokens.items():
        text = text.replace(old, new)
    here = Path(p).parent
    link = lambda m: f"[`{m.group(1)}`]({Path(os.path.relpath(aliases[m.group(1)], here)).as_posix()})"
    for line in spaced(text.splitlines()):
        if fence.match(line.lstrip()):
            in_fence = not in_fence
        elif not in_fence and not line.startswith("#"):
            line = ref.sub(link, line)
            if not re.search(r"client repo", line, re.I):
                line = repo_path.sub(github, line)
        out.append(line)
    # the title comes first, then the note saying which client the page is about
    if p.startswith("client/") and not p.startswith(("client/afterglow/", "client/episodes/")) and out and out[0].startswith("# "):
        out[1:1] = ["", *CLIENT_NOTE.rstrip("\n").split("\n"), ""]
    (STAGE / p).parent.mkdir(parents=True, exist_ok=True)
    (STAGE / p).write_text("\n".join(out) + "\n", encoding="utf-8", newline="\n")

mkdocs = HERE / ".venv" / ("Scripts" if os.name == "nt" else "bin") / ("mkdocs.exe" if os.name == "nt" else "mkdocs")
cmd = [str(mkdocs) if mkdocs.exists() else sys.executable, *([] if mkdocs.exists() else ["-m", "mkdocs"]), "build", "--clean", "-f", str(HERE / "mkdocs.yml")]
# the site stays on MkDocs 1.6 on purpose, so Material's banner about MkDocs 2.0 is noise
sys.exit(subprocess.call(cmd + sys.argv[1:], env={**os.environ, "NO_MKDOCS_2_WARNING": "true"}))
