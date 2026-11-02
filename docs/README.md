# docs: the documentation

Everything the project knows about the original EP4.5 server, how this server runs, what is planned and how it is tested. The notes
about the original are the reference; the code's comments cite them by file name and section number.

| folder | what it holds |
|---|---|
| [`original/`](original/README.md) | what the original server does, one note per system (60 notes), reference notes, and the raw data read from the original files (`symbols/`, `db/`, `inventory/`) |
| [`server/`](server/README.md) | running this server: architecture, settings, deployment, the list of deviations from the original, the robustness audit |
| [`plans/`](plans/README.md) | the roadmap, the plans for what is not built yet, the plans of what is (`done/`), the balance work (`balance/`) |
| [`testing/`](testing/README.md) | the live tests to walk with a real client, and the queue of questions only the client can answer |
| [`client/`](client/README.md) | findings about the US client `game.exe` (maintained on the client side; the server's notes cite them) |

## Where to start

- To understand a system of the game: its note in `original/` (its **In short** section first). The login server and the game server,
  [`original/ps_login.md`](original/ps_login.md) and [`original/ps_game.md`](original/ps_game.md), come first, then
  [`original/protocol-notes.md`](original/protocol-notes.md).
- To run the server: the root `README.md`, then [`server/settings.md`](server/settings.md) and [`server/deploy.md`](server/deploy.md).
- To see how the server differs from the original: [`server/deviations.md`](server/deviations.md).
- To see what comes next: [`plans/roadmap.md`](plans/roadmap.md).

## Status words

The notes say how sure they are. **confirmed** (or **verified**): read in the executable or in the server's code, with the address or
the function given. **inferred**: follows from what was confirmed but was not seen directly. **PDB**: read from the debug symbols, so the
names and struct layouts are exact for the build. Letters in square brackets in the protocol notes (`[B]`, `[W]`, ...) name the source
of a fact; the table is in [`original/protocol-notes.md`](original/protocol-notes.md). A note says where a claim comes from.

## The documentation site

`site/` builds a public site from these files with MkDocs (`site/README.md`). It publishes exactly the pages listed in the nav of
`site/mkdocs.yml`; a page comes from `site/content/` when a file of that path is there (pages written for the site) and from `docs/`
otherwise. The plans and the testing pages are not on it; a backticked path to them, or to any other file of the repository, links to
it on GitHub.

`python3 tools/check_links.py` checks every link and every path and section reference in the repository; CI runs it.
