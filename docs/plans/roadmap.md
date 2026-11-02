# Roadmap

**The open-source release is done** (`docs/plans/release.md`, decided 2026-10-31; the public repository `byyako/shaiya-afterglow`, tag `v0.1.0`): a public repository that anyone with the
original EP4.5 server files can clone and run, defaulting to the original's behaviour (fixes on, gameplay changes as settings with the
original as default, additions off), with a fresh history. The player alpha below moves after the release; balance changes wait until
after the alpha and come as extension work. Bug checks with the real client carry on alongside, not as a release gate.

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

## After the release: the player alpha (planned 2026-10-22, moved after the release 2026-10-31)

### Decisions (2026-10-22)

- **Phase 4 (extensions, rules, Lua, pets, per-player loot) is frozen** until after the alpha, including the chunk 1 spec
  (`docs/plans/ext-1-foundation.md`, unbuilt). The core keeps its current config and database layout for the alpha.
- **The alpha is open-ish: about 20 to 100 players, a public link, wipeable.** Registration is open with rate limits and no email; a
  lost password means staff reset it or a new account. The server starts with a user cap (`.maxusers`) and raises it as it holds.
- **Test bots are shelved, not dropped** (under "Frozen"). There is no load test before the alpha: the alpha is the load test,
  so the operations step must put the numbers (players, tick time, memory, database size) where they can be watched, and the cap is the
  safety.
- **Web: one HTTP/JSON API with scoped keys is the only supported disconnect between the server and any web feature.** Pages are
  separate static HTML/JS front-ends that call only the API (a player site and a staff admin), replaceable and forkable; endpoint groups
  can be shared on their own, so someone with a PHP site can use the registration or ranking calls without our pages. No reference PHP
  is built now (the maintainer's old PHP admin panel may be shown to the spec as a reference).
- Public documents (the internal docs pass, the public site, the patch kit for the original files) wait for the alpha's fixes, except
  the parts that do not depend on the server, which are done.

### Alpha steps

| step | what | needs |
|---|---|---|
| A3. Live tests and the smoke walk, deferred to the alpha (decided 2026-10-26): a short pre-launch smoke list is walked once before B4, the rest of `docs/testing/live-test-checklist.md` by alpha players and staff, and fixes follow | work item A3 below | A2 (built); the web registration item needs B1 |
| B1. Web API and admin hooks: spec, then build (API, player site, staff site) | work item B1 below | none |
| B2. Operations for the alpha: spec, then build | work item B2 below | B1's service list, so it follows B1's spec |
| B3. Client for testers: package, updater served from the VPS, the server address; patch 109's acceptance is done (`docs/client/client-patch-0109.md`) | the client side | B2 (the host); the maintainer decides how the client is distributed |
| B4. Go/no-go and launch: provision the VPS, deploy, soft launch to the maintainer and a few friends, then the public link | the maintainer | A2, B1, B2, B3 |

Order of work: B1's spec and A3's smoke walk are independent. A2 and B1's build do not touch the same files, but if they run together, A2
goes first on `crates/game-server`. B2 starts when B1's spec is committed. B1's spec reads `docs/server/audit.md` section 3.5 (the
6-to-12 password rule, the NOTIFY channel, the data editor's access); A2 part 3 changes the login server's limits and
`validate_credentials`, which B1's registration should reuse.

### During the alpha

C0. The checklist is the alpha's test plan: staff and willing players work through it (the checklist page can be shown to them, or its
items turned into a tester-facing list by the staff site), and results go back into the checklist and the README status lists. C1. A weekly cadence: collect reports (a thread or the staff site's notes), fix, deploy with the restart countdown, announce. C2. Wipe
when a change needs it: the wipe procedure is part of B2 (`public` schema reset, accounts optional). C3. Read the numbers; if the cap
is raised past what the first hundred players show, build the shelved bot as a load test first.

### After the alpha

Run the public documents (internal docs pass, public site, bug page and patch kit), decide what phase 4 chunk to restart from what
testers asked for, then the items under "Frozen" and "Later". Re-run the audit on the web API and anything built during the alpha.

### Work item A3. The pre-launch smoke list

**Goal**: `docs/testing/smoke-list.md` (copied as section 0 of the checklist by `tools/smoke-list-sync.sh`) is walked once, before the
launch, and what fails is fixed. The walk is optional until then: the smoke list stays the first thing alpha staff walk after each deploy.

**Inputs**: `docs/testing/smoke-list.md`, `docs/testing/live-test-solo.md` (trade needs two clients), the sections of
`docs/testing/live-test-checklist.md` that cover each item, `docs/server/audit.md` (A2 is built, the fuzz harness included).

**What to do**: about a dozen items covering login, entering the world, moving, fighting, looting, shop, trade, warehouse, a relog and
the web registration (which waits for B1). Each item says which test setup to use (`docs/testing/live-test-checklist.md`, "Test setup"),
the steps, what should happen, and what to look at in `logs/game-server.log` or `logs/login-server.log` if it does not. After the walk,
read the results with `tools/checklist-server.sh report` and fix what failed; a fix that needs the original binaries read again is
analysis work first.

**Done when**: every item that can run before B1 is walked, its result is in the checklist page, and each failure is fixed or listed.

### Work item B1. Web API and admin hooks

**Goal**: a spec concrete enough to build in parts, for the open alpha (20 to 100 players, a public link, wipeable).

**Inputs**: this roadmap's Decisions, `README.md`, `docs/server/architecture.md`, `docs/server/deploy.md`,
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

### Work item B2. Operations for the alpha

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
  spec in `docs/plans/test-bots.md` with a Build section, then the bot. Shelved because the alpha is the load test and the alpha's
  players cover the two-player items.
- **The original console's live data reloads**, the second game server (map 200), obelisk portal links: decisions in the README "Not
  built".

## Later, for other operators

Raised by the maintainer (2026-10-22): people already running the original server files should be able to move to NewShaiya.

**2026-10-26: the maintainer wants this fully discussed before it is specified** (`docs/plans/ideas.md` section 5).

- **Converting an existing original server**: its game definitions (`PS_GameDefs`), its world files (`Data/`: `Map.ini`, the svmaps,
  `Obelisk.ini`, `ItemCreate.ini`, `NpcQuest.SData`, …) and its players (`PS_UserData` accounts, `PS_GameData` characters, items,
  skills, quests, guilds, banks, and the rest the new server keeps) into a NewShaiya database. Today `tools/defs/` (an MSSQL export and
  `load_into_postgres.py`) and `tools/world/import.py` do the first two as dev tools (Python, the dev container); the conversion needs
  them packaged for an operator, the player data mapped table by table (`docs/original/database-notes.md`), and a dry run that reports what
  cannot be carried over. Their own edits then carry over as data, and later ones go through the data editor
  (`docs/plans/done/data-editing.md`). Needs a spec first.

## Parked ideas

`docs/plans/ideas.md` holds what the maintainer raised (2026-10-26) and nothing is scheduled: the first extensions to design the plugin system against
(fishing, crafting, forging, camping, monster madness), personal loot with the drop numbers and the lean, the balance project (analysis done in three parts; plan
`docs/plans/balance/plan.md`, 2026-10-27; on 2026-10-31 the maintainer moved it to after the alpha, as extension work, not core changes), the deferred or dropped own client, and the discussion of using existing original files with this server.

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
- **Client data patches, not built** (`docs/plans/done/client-patches.md` section 7): serving the patches (the alpha's B3), the other client data files
  (`Monster.SData`, `NpcSkill.SData`, `NpcQuest.SData`, `Cash.SData`) by the same pattern, and growing a table (a new item group, type ids past a
  group's count). They wait for the extension work and the balance plan, which need them.
