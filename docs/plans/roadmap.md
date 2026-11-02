# Roadmap

**The open-source release is done** (`docs/plans/release.md`, decided 2026-10-31; the public repository `byyako/shaiya-afterglow`, tag `v0.1.0`): a public repository that anyone with the
original EP4.5 server files can clone and run, defaulting to the original's behaviour (fixes on, gameplay changes as settings with the
original as default, additions off), with a fresh history.

**There is no player alpha (the maintainer, 2026-11-02).** A hosted alpha (a VPS, open registration, a web API, a tester client) was
planned for after the release; instead the server files are released and the community tries them on their own servers. What people
find comes back as issues (`CONTRIBUTING.md`, "Reporting a bug"); the live tests below are walked by whoever runs a server. The alpha's
work items are kept under "Not scheduled" in case a part of them is wanted later. Balance changes and the extensions stay parked.


## The release (`docs/plans/release.md`): built

| step | state | needs |
|---|---|---|
| R0. README for a fresh clone, `docs/server/settings.md`, `.env.example`, this plan | done | |
| R1. Fidelity pass: `docs/server/deviations.md` (every deviation, classified), defaults back to the original, additions gated | **built** (`docs/server/deviations.md`, Build (R1)) | |
| R2. Setup path: configurable import scripts, `compose.yaml`, `tools/setup.sh`, the install drill from a clean clone | **built** (`docs/plans/release.md` R2: the drill passes from a clean clone; `GAME_MINI_GAME` on by default) | R1 |
| R3. Code health and CI: clippy and fmt clean, the big files split behaviour-neutral, GitHub Actions | **built and reviewed** (`docs/plans/release.md` R3: fmt and clippy clean, the three files split, CI; the review found no behaviour change) | R1 |
| R4. Repository review and organisation: every file checked, `docs/` in folders, links fixed, personal details and workflow notes out | **built** (`docs/plans/release.md` R4: the decisions, the table of every file, the move map, and what was built) | R1 to R3 |
| R5. LICENSE, CONTRIBUTING, SECURITY, changelog, 0.1.0, secret scan | **built** (`docs/plans/release.md` R5) | R4 |
| R6. The release cut: public repository, one commit, tag, push, docs site link and deploy | **built** (`docs/plans/release.md` R6; `byyako/shaiya-afterglow`, tag `v0.1.0`) | R5 |

## Testing

The automated tests cover the rules and the packets; what the real client shows is walked by hand (`docs/testing/README.md`).

- **The smoke list** (`docs/testing/smoke-list.md`, section 0 of the checklist): about a dozen things a new player does in the first
  half hour. An operator walks it on a new server before players join, and again after an update.
- **The long list** (`docs/testing/live-test-checklist.md`, sections 1 to 57): every item built and not yet seen working with the real
  client, with where to try it and what should happen. `docs/testing/live-test-solo.md` sorts the items into one client, two clients,
  hard alone and skip. A result, good or bad, is welcome as an issue naming the section and item; the maintainer folds the results
  into the checklist and fixes what fails (a fix that needs the original binaries read again is analysis work first).
- **On the maintainer's box**, the checklist page (`tools/checklist-server.sh`) keeps the results and `tools/checklist-server.sh report`
  prints them for the next session.

### Work item A3. The smoke walk

**Goal**: `docs/testing/smoke-list.md` (copied as section 0 of the checklist by `tools/smoke-list-sync.sh`) is walked once on the
maintainer's box with the real client, and what fails is fixed, so a new operator meets a server that passes it.

**Inputs**: `docs/testing/smoke-list.md`, `docs/testing/live-test-solo.md` (trade needs two clients), the sections of
`docs/testing/live-test-checklist.md` that cover each item, `docs/server/audit.md` (A2 is built, the fuzz harness included).

**What to do**: about a dozen items covering login, entering the world, moving, fighting, looting, shop, trade, warehouse and a relog
(the twelfth, web registration, is skipped: B1 is not scheduled). Each item says which test setup to use (`docs/testing/live-test-checklist.md`, "Test setup"),
the steps, what should happen, and what to look at in `logs/game-server.log` or `logs/login-server.log` if it does not. After the walk,
read the results with `tools/checklist-server.sh report` and fix what failed; a fix that needs the original binaries read again is
analysis work first.

**Done when**: every item but the web registration is walked, its result is in the checklist page, and each failure is fixed or listed.

## Not scheduled (written for the alpha, dropped 2026-11-02)

The alpha's steps were B1 (the web API and admin hooks), B2 (operations: a VPS, backups, deploys, the wipe, monitoring), B3 (a client
package and updater for testers) and B4 (the launch). B3 and B4 went with the alpha. B1 and B2 are kept below as they were specified,
because an operator running a public server may want parts of them (an API for a registration page, the backup drill); their
"alpha" means such a server. The decisions they rest on (2026-10-22): one HTTP/JSON API with scoped keys as the only disconnect between
the server and any web feature, with static pages that call only it; registration open with rate limits and no email; a user cap
(`.maxusers`) raised as the server holds. Nothing here is started without the maintainer asking.

### Work item B1. Web API and admin hooks

**Goal**: a spec concrete enough to build in parts, for a public server (written for the alpha: 20 to 100 players, a public link, wipeable).

**Inputs**: the decisions above, `README.md`, `docs/server/architecture.md`, `docs/server/deploy.md`,
`docs/plans/done/data-editing.md` (the data editor is an axum service with a page: the model to build from),
`docs/original/operator-console.md` (the `console` tool and the NOTIFY channel), `docs/original/item-mall.md` (points, premium, the
`mall` tool), and the login-server tools (`crates/login-server/src/bin/`: `create_account`, `servers`, `console`, `mall`).

**Decided (the maintainer; not reopened)**:
- One supported disconnect between the server and any web feature: an HTTP/JSON API with scoped API keys. Web pages are separate static
  HTML/JS front-ends that call only that API: a player site and a staff admin, in separate folders, with no build step or framework a
  reader cannot follow, hostable anywhere, replaceable, forkable. Endpoint groups are separable: a community member with their own PHP
  (or any) site can use just the account or ranking calls, with a key scoped to those, without our pages or our other endpoints. The
  community historically shared PHP scripts directly against the database; the API replaces them, so the spec says plainly what is left
  out and why, and the docs give curl examples.
- Registration is open with rate limits and no email; a lost password means staff reset it (or a new account). Alpha: accounts can be
  wiped.
- The API is the only route for web features; the database layout stays private to the server and free to change. The existing tools
  (`create_account`, `console`, `mall`, `servers`, `data-editor`) keep working; the spec decides whether the API calls the same code, goes
  through the NOTIFY channel for live actions, or both, and where the data editor's own API fits (fold in, or keep it and share auth).
- The maintainer once wrote an admin panel in PHP for another server: ask once whether they want to show it, and if so read it for the
  feature list (not for code).

**What to cover**: player-facing (register, check a name, change password, server status and online count, rankings of the original's
(guild ranking, PvP kills), a character summary for the owner); staff (accounts and characters search, ban and unban, silence, kick,
password reset, mall points and premium, notices, the console's switches and the quit countdown, the user cap, the server list, log
and game-log search, an audit trail of every staff action, the data editor link); API keys (scopes, creation, rotation, revocation,
storage); staff login for the static admin (session or token, TLS, optional 2FA); rate limiting and abuse (per IP and per key, account
creation limits, login brute force at the API and at the login server, captcha or not); the contract (an OpenAPI file as the source of
truth, versioned `/v1`, error format, pagination); CORS for static pages on another host; how the service is deployed beside the two
servers and a reverse proxy (Caddy or nginx; the host choice is B2's, the interfaces are fixed here); a health/metrics endpoint B2 can
scrape; what runs when the game server is down. The crate layout: a new crate in the workspace, tests against the dev database, the same
style as `crates/data-editor`.

**Output**: `docs/plans/web-api.md` with a Build section split into parts (the API core and keys; the player endpoints; the staff
endpoints and audit trail; the player site; the staff site), each part one build session, the tests, the live-test items it adds to
`docs/testing/live-test-checklist.md`, and the OpenAPI file.

**Done when**: the spec is committed and each choice the maintainer had to make is written in it as a decision.

### Work item B2. Operations for a public server

**Goal**: a spec, then a build, of what running the alpha needs: an open alpha with 20 to 100 players, a public link, wipeable, on a
rented VPS (Hetzner is likely), registration open with rate limits and no email, no load test before launch (the alpha is the load
test), a user cap raised as it holds, and the web API of `docs/plans/web-api.md`.

**Inputs**: `docs/server/deploy.md`, `docs/plans/web-api.md` (after B1's spec), the tools.

**What to spec**:
- the host: which plan and region, the OS, the firewall, which ports are open (login, game, the API behind TLS), hardening (ssh keys,
  no root login, unattended upgrades), DDoS basics at that provider, what to do when attacked;
- backups: automatic `pg_dump` of the `public` schema (and `edits`), how many to keep, an off-machine copy, a restore that has been tried
  (write the drill, run it on the dev box);
- deploys: building a version (on the VPS, on the dev box, or in CI), installing it, restarting with players online (the quit countdown
  exists), migrations, a one-command rollback, a release tag for each deploy;
- the alpha's wipe: a script that resets the player data (accounts optional), keeps the game data, announces it;
- monitoring: logs and rotation, the services dying or hanging (systemd restart policy, a watchdog), the numbers to watch (players,
  tick time, memory, database size, connection count) and alerts to the maintainer's phone or chat;
- abuse: connection limits at the login and game ports, banning an address, the account-creation limits of the API;
- the tester's side: where the client download and the updater are served from, and the server address a tester's client gets (a
  question list for the client side, B3).

**Output**: `docs/plans/operations.md` with a Build section (scripts, units, `docs/server/deploy.md` updated, the drills).

**Done when**: the spec is committed with the hosting choices as decisions; after the build, the backup restore drill, a deploy and a
rollback, the wipe script and one login-to-world run on the VPS have each been done.

## Frozen (not scheduled)

- **Phase 4, extensions, rules and pets** (`docs/plans/extensions.md`, unbuilt): 4.1 foundation (spec `docs/plans/ext-1-foundation.md`),
  4.2 rules and hooks, 4.3 item effects and skill abilities, 4.4 pets, 4.5 Lua, 4.6 cross-faction and client-bound rules, 4.7 the
  catalogue waves, 4.8 the author kit, 4.9 per-player loot. The client research entries 9 to 12 (`docs/testing/client-research-queue.md`)
  belong to it.
- **Test bots and a load test** (shelved with the old step 1c; kept because the bot is also the base of a load and soak test). **Goal**: a
  protocol-level bot grown from `crates/game-server/examples/game_probe.rs` (which stops at the character screen) into its own crate or
  binary. It logs in through the login server as the real client does, enters the world and plays a scripted role (walk to a point, send
  or accept trade, party, guild and duel invites, attack a target, use a skill, chat, sit, die and come back), sending only the client's
  packets as documented in `docs/`, so it goes through the same code a player does. **Two uses**: a partner mode driven from the terminal
  or a small script while testing with the real client (a bot that accepts every trade and party invite and follows the tester), and
  scenarios, bot against bot, that check the results without a client (the answer packets, the database rows) for checklist items that
  need no eyes, runnable as a test command. **First**: `docs/testing/live-test-solo.md` sorts the open items into one client, two
  clients, hard alone and skip; decide which two-client and hard-alone items work with a bot partner and which can be bot-only
  scenarios (a table in the spec), and which scenarios to build first. **Later**: the base of the load and soak test (hundreds of bots
  logging in, moving and fighting for hours: connection count, pacing, accounts made by `create_account`, metrics). **Done when**: a
  spec in `docs/plans/test-bots.md` with a Build section, then the bot. Shelved when the alpha was to be the load test and its
  players were to cover the two-player items.
- **The original console's live data reloads**, the second game server (map 200), obelisk portal links: decisions in the README "Not
  built".

## Later, for other operators

Raised by the maintainer (2026-10-22): people already running the original server files should be able to move to Shaiya Afterglow.

**2026-10-26: the maintainer wants this fully discussed before it is specified** (`docs/plans/ideas.md` section 5).

- **Converting an existing original server**: its game definitions (`PS_GameDefs`), its world files (`Data/`: `Map.ini`, the svmaps,
  `Obelisk.ini`, `ItemCreate.ini`, `NpcQuest.SData`, …) and its players (`PS_UserData` accounts, `PS_GameData` characters, items,
  skills, quests, guilds, banks, and the rest the new server keeps) into a Shaiya Afterglow database. Today `tools/defs/` (an MSSQL export and
  `load_into_postgres.py`) and `tools/world/import.py` do the first two as dev tools (Python, the dev container); the conversion needs
  them packaged for an operator, the player data mapped table by table (`docs/original/database-notes.md`), and a dry run that reports what
  cannot be carried over. Their own edits then carry over as data, and later ones go through the data editor
  (`docs/plans/done/data-editing.md`). Needs a spec first.

## Parked ideas

`docs/plans/ideas.md` holds what the maintainer raised (2026-10-26) and nothing is scheduled: the first extensions to design the plugin system against
(fishing, crafting, forging, camping, monster madness), personal loot with the drop numbers and the lean, the balance project (analysis done in three parts; plan
`docs/plans/balance/plan.md`, 2026-10-27; on 2026-10-31 the maintainer parked it as extension work, not core changes; it was to follow the alpha, which was dropped), the deferred or dropped own client, and the discussion of using existing original files with this server.

## Done (the old step numbers, which other documents refer to)

| old step | result |
|---|---|
| 1a. Client research entries 6 and 7 | answered (`docs/client/`) |
| 1b. Client data patches | `docs/plans/done/client-patches.md`; the server side built, patch 109 generated in `data/client-patches/ps0109/`; the client side packed, signed and accepted it (`docs/client/client-patch-0109.md`) |
| 1c. Test bots | shelved |
| 1d, 1e. Live tests and fixes | ongoing, now A3 |
| 1f. Data editor | built, parts A and B (`docs/plans/done/data-editing.md`); the open items are under "Open work items" |
| A1. Robustness audit | `docs/server/audit.md`: 30 findings ranked by harm, the maintainer's decisions, the proptest harness spec and the A2 build in four parts; client queue entry 13 added for two input ranges |
| A2. Audit fixes and the fuzz harness | built in four parts (`docs/server/audit.md` section 6): hostile connections; whole exchanges and saves; login guard, editor, inputs; the proptest harness (`shaiya-proto/tests/decoders.rs`, `shaiya-crypto/tests/fuzz.rs`, `game-server/tests/fuzz_dispatch.rs`, `login-server/tests/fuzz_login.rs`, run in every `cargo test`; deep runs in the README) |

## Open work items

- **Data editor: map pictures.** The page draws the grid alone until `data/map-images/` exists. **Inputs**: the client's answer to queue
  entry 8, `docs/client/research-extension-features.md`, "Map pictures"; `docs/plans/done/data-editing.md` section 8 (the folder's
  layout and `data-editor images import`). **Done when**: the pictures are imported and a map's picture lines up with its spawn areas
  and NPCs (live test 53's last check).
- **Data editor: live test 53** (`docs/testing/live-test-checklist.md`) is open.
- **Data editor, not built** (`docs/plans/done/data-editing.md` section 10): signing in with a GM account (and `EDITOR_ALLOW` so the
  editor can be reached over the internet behind HTTPS), live reload of spawns, portals and obelisks, NPCs, shops and gatekeepers (they
  need the `NpcQuest.SData` client patch), new monsters and moved or added portals and obelisks (they need client data first).
- **Client data patches, not built** (`docs/plans/done/client-patches.md` section 7): serving the patches (the dropped alpha's B3), the other client data files
  (`Monster.SData`, `NpcSkill.SData`, `NpcQuest.SData`, `Cash.SData`) by the same pattern, and growing a table (a new item group, type ids past a
  group's count). They wait for the extension work and the balance plan, which need them.
