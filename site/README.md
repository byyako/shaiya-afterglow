# Public docs site

A MkDocs Material site built from the notes in `../docs`. Only the files listed in `mkdocs.yml`'s `nav` are published (everything else in `docs/` stays private); `content/` holds the home page and CSS.

```bash
python3 -m venv .venv && .venv/bin/pip install -r requirements.txt   # once
python3 build.py                                                      # writes dist/
```

`./deploy.sh` builds and shows what rsync would change; `./deploy.sh --go` uploads. The target (`user@host:/path/to/shaiya/`) comes from `$SHAIYA_DEPLOY` or the first line of `.deploy-target` (git-ignored); the ssh key from `$SHAIYA_KEY` or the second line of `.deploy-target`, else ssh's own default keys. `dist/` is plain static files with relative links, so it can be copied to any path (for example `tylerd.me/shaiya/`): `rsync -a --delete dist/ host:/var/www/.../shaiya/`.

`build.py` also rewrites the GitHub-style Markdown the notes use (a list or table straight after a paragraph line, two-space nested lists, a bare `<p>` in prose) into what Python-Markdown reads, and turns backticked names of published pages (`` `docs/original/shop.md` ``) into links. The source files are not changed.

To publish another page, add it to `nav`. Held back for now: `docs/client/README.md` (the site has its own `content/client.md`), `docs/server/architecture.md`, `docs/original/client-login.md`, `docs/original/database-notes.md`, `docs/original/server-inventory.md`, `docs/original/community-guide-notes.md`, `docs/original/ep6-core-packet-catalogue.md`, `docs/testing/live-test-checklist.md`, and the raw data under `docs/original/db`, `docs/original/inventory`, `docs/original/symbols`.
