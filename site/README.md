# Public docs site

A MkDocs Material site built from the notes in `../docs`. Only the files listed in `mkdocs.yml`'s `nav` are published (everything else in `docs/` stays off the site, though it is in the public repository); `content/` holds the pages written for the site (the home page, the landing pages of the player guide, client and server tabs `guide.md`, `client.md` and `server.md`, and `server/run.md`, a short version of the README's setup) and the CSS, and `overrides/main.html` adds the "Buy me a coffee" link in the corner of every page (styled in `content/extra.css`; the footer has it too, with the GitHub link, from `extra.social` in `mkdocs.yml`).

```bash
python3 -m venv .venv && .venv/bin/pip install -r requirements.txt   # once
python3 build.py                                                      # writes dist/
```

`./deploy.sh` builds and shows what rsync would change; `./deploy.sh --go` uploads. The upload mirrors with `--delete`, so the script guards the live site: one deploy at a time, rsync reads a private snapshot of the build (a build running meanwhile cannot empty it), the snapshot must hold every nav page and at least 100 files (`SHAIYA_MIN_FILES`), and a real upload that would delete more than 20 files on the server (`SHAIYA_MAX_DELETES`) stops unless `--allow-deletes` is given. The target (`user@host:/path/to/shaiya/`) comes from `$SHAIYA_DEPLOY` or the first line of `.deploy-target` (git-ignored); the ssh key from `$SHAIYA_KEY` or the second line of `.deploy-target`, else ssh's own default keys. `dist/` is plain static files with relative links, so it can be copied to any path (for example `tylerd.me/shaiya/`): `rsync -a --delete dist/ host:/var/www/.../shaiya/`.

`build.py` also rewrites the GitHub-style Markdown the notes use (a list or table straight after a paragraph line, two-space nested lists, a bare `<p>` in prose) into what Python-Markdown reads, turns backticked names of published pages (`` `docs/original/shop.md` ``) into links, and links backticked paths of the repository's other files and folders (`` `docs/plans/roadmap.md` ``, `` `crates/game-server` ``) to GitHub. The source files are not changed.

To publish another page, add it to `nav`. Held back for now: `docs/client/README.md` (the site has its own `content/client.md`), `docs/guide/README.md` (the site has `content/guide.md`), `docs/server/README.md` (the site has `content/server.md`), `docs/original/community-guide-notes.md`, `docs/original/ep6-core-packet-catalogue.md`, `docs/testing/live-test-checklist.md`, and the raw data under `docs/original/db`, `docs/original/inventory`, `docs/original/symbols`.
