# Client-side findings (from the client RE repo)

Written by the client side (the client repository, a git repository with `docs/` and `tools/`). These files are
copies, refreshed when the client side learns something the server needs. Client = US `game.exe` 2010-04-26, patch level 108
(SHA-256 `5f045bcb…01a6`), the same binary the server docs analyse.

How these pages are written, so that they fit the repository: no machine-specific paths (say "the client repository", "the original files"), no
names of the tools used to write them, no dates except facts about the original files (build dates, the client's 2010-04-26), no commit hashes, and
"the maintainer" for the person who decides. Tools of the client repository are named as such.

| File | What |
|---|---|
| `operator-listen.md` | F502 operator-listen layout, DLL hook/display, offline validation and live acceptance |
| `research-movement-storages-auction.md` | Queue 1/2/4/5 answers: movement speeds/timing, storage opening, B103 date/chat, auction columns/tooltips; existing patch recommendations |
| `client-packets.generated.md` | Every opcode the client handles (425) and sends (245, auction house included), with the field layout the client reads/writes, cross-referenced with the EP4.5 PDB struct sizes (86 exact matches), EP6 names and `shaiya-proto` constants |
| `client-packets.md` | How the client receives/dispatches/sends; the underflow error; built-in packet logging |
| `client-systems.md` | Every game system by packet family, with evidence, and a suggested order for in-world features |
| `client-world-packets.md` | NPCs (`0x0Exx`), mobs (`0x06xx`), combat and skills, HP/MP/SP updates, items and drops: what the client does with each field, from its handlers |
| `client-social-packets.md` | Trade (`0x0Axx`), party and union (`0x0Bxx`), member status (`0x0Cxx`), guild (`0x0Dxx`): fields, result codes with their messages, and the packet order the original `ps_game.exe` uses |
| `CLIENT-FACTS.md` | Corrections to the earlier notes, request/reply facts (item use, stat reset, link/enchant, auction house `1Cxx`), what the client itself filters for blocked players, answers to the server's open questions, region features, SData layouts, the comparison with the server's docs (`0x0802` is 33 bytes for this client, `0x021B` counts, the Teleportation Stone window) and what the client does with its own SData values; the place to keep adding server-relevant client facts |
| `packet-opportunities.md` | The opcode catalogue: every opcode (425 handled, 244 sent) with its fields, what the client does, whether the original `ps_game.exe` sends or handles it and where, whether `shaiya-proto`/`game-server` builds it, and the status (built / original-only / client-only); a `0x0802` size bug in the server. The opportunities moved to `afterglow/opportunities.md`, the region features to `region-features.md` |
| `gm-and-commands.md` | Every GM opcode (`F1xx`–`FAxx`): the chat command (English and Korean) and wire layout, the client's GM-level check (login `0xA102` privilege < 11, then ≤ 5…1 per command), what the client does with each answer (`F501` codes), what the original `ps_game.exe` does (level limits per family; ten commands it disconnects the GM for; `F601` layout mismatch; `F80D`/`F80E` mean heal/cure there); the player chat commands and prefixes; how to add commands with and without a client change; completed: what each privilege 1–10 sends, `F501` values, deferred moves, `/cwatch`, `/autocure`; later: the fixes.dll command fixes (passes 1 and 2) and the answers to the live test (invisible staff, quotes, `/kick`) |
| `scripted-monsters.md` | Scripted monsters: what the client does with `06xx` and `1109`–`110B` (speech is a chat line, voice a positional sound, no client boss logic), and what a script can drive |
| `personal-shop.md` | Personal shop `23xx`: every request and answer, fields, result codes → messages, the client's checks, the original's order |
| `auction-house.md` | Auction house `1Cxx`: requests with client checks, answers, the row layouts (PDB `TP_MARKET_*`), packed dates, what each notice does |
| `item-mall.md` | Item mall `26xx` / `B1xx` and points: product list from `Cash.SData` (+ `0xB106`), buy and gift flows, storages, billing packets, a minimal server plan |
| `duels.md` | Duels `24xx`: request, stake window, countdown, fight, results (corrected `0x2406` codes), original order |
| `guild-wars.md` | Guild wars `25xx`: request rules, countdown, member events, results |
| `obelisks.md` | Obelisks and roads `21xx`: list, states, destruction messages; `0x2104` dead on this client, why, and a 5-byte patch design |
| `union-parties.md` | Union parties `0B0B`–`0B1A`, `0C0A`–`0C14`: short build notes over `client-social-packets.md` |
| `item-sdata-vs-defs.md` | The client's `Item.SData` (decrypted) compared with the server's `defs.items`: counts, what is not a real difference, the differences in items players can get (written on the server side) |
| `npcquest-sdata-vs-world.md` | The client's `NpcQuest.SData` (decrypted; the server's own layout) compared with the server's file and `world.*`: shops identical, NPCs, gatekeepers, quests (written on the server side) |
| `skill-monster-sdata-vs-defs.md` | The client's `Skill`, `NpcSkill`, `Monster`, `Cash`, `GuildHouse`, `KillStatus` and `PriestTalk` SData: layouts, and the differences with `defs.skills`, `defs.mobs`, `defs.product_list` and the server's files (written on the server side) |
| `sdata-generation.md` | Plan and notes for generating the client's SData files from the database (and restoring the originals exactly): what is proven for `Item.SData`, what the database lacks, the other client SData files and their server side, the order of work |
| `glossary.md` | Words used on the client pages, in plain language |
| `region-features.md` | The client's region-feature switches: id, callers, what each does, state for region 6, evidence |
| `eft-tool.md` | The effect tool `eft.exe` and the `.eft` file format |
| `client-patch-kit.md` | The patch kit for `game.exe`: what each patch changes and how to apply it |
| `fixes-dll.md` | The companion DLL: what it hooks and changes |
| `updater.md` | The updater and its patch files |
| `graphics.md`, `re-engine.md`, `re-network.md`, `re-gameplay-state.md`, `re-open-items.md`, `client-actions.md`, `formats.md`, `data-archive.md`, `cleanup.md`, `custom-animations.md`, `english-text.md`, `tools-and-method.md`, `qol-feasibility.md` | Engine, network and game-state internals, open items, actions, file formats and the archive, unused files, custom animations, the English text pass, tools and method, quality-of-life feasibility |
| `research-gambling-kill-boost.md`, `research-extension-features.md`, `research-priest-packets.md`, `research-balance.md` | Research notes (renamed from the earlier `client-research-*` file names) |
| `afterglow/` (`index.md`, `server-checklist.md`, `opportunities.md`, `client-findings-status.md`) | The Shaiya Afterglow tab's pages written by the client side: what a server must do for this client, the opportunities it supports, and the status of client findings |
| `episodes/` (`index.md`, `ep1.md` to `ep8.md`, `later.md`, `us-patches.md`) | The other Shaiya episodes and the US patch history |

## Things the server should know

- **Client packet log.** Put this next to `game.exe` as `gsConfig.cfg`: `[gsCommon]` / `LogDefaultListener=true` / `LogLevel=67`.
  The client then writes `Log.txt` with hex dumps of every decrypted packet both ways (`SEND>>`/`RECV>>`), plus missing-file
  errors. The login packet (`0xA102`) appears with the password in clear. The client repo's `tools/clientlog.py` decodes the log
  against the catalogue. First live log: login → world → move/sit/chat → logout, **every packet size matched the client**.
- **Short packets.** If a server packet is shorter than the client reads, the client logs
  `Packet Underflow. Protocol : 0x%04X Size : %d Read %d` (`CNetWork::PacketError`) — visible in `Log.txt`.
- **Chat markup crash: filter chat text.** `<<NNNcCC` is the client's colour/item-link marker for the next NNN bytes. A marker
  whose NNN runs past the text (the classic `<<999c01`) crashes every client that displays the line, including the sender's
  own client when the server echoes it (tested; the server itself survived). Before relaying any chat (`11xx`,
  `F1xx`), reject or drop text containing `<<` unless every marker is `<<` + 3 digits + `c` + 2 digits with
  1 <= NNN <= bytes left after the 8-byte marker. Details: `client-packets.md` (Findings).
- **Unknown opcodes** go to a fallback (`FUN_00551030`) that handles `0x0114` (server time) and the `0x1Cxx` (auction) / `0x1Dxx`
  (mail) families; anything else is silently ignored. `0x0509` and `0x1112` (party-leader chat) are explicitly ignored when received.
- **`0x020A` portal use**: the client sends 3 bytes (`u8 portal`), the PDB struct is 4 (shared with the reply). Accept 3.
- **`0x0105` tail**: the last field is read-up-to 25 bytes (likely guild name); sending it empty (1 byte NUL) is fine.
- **Market Anywhere** (client patch at `0x41F11`) lets the client send the personal-shop motion anywhere; the original only allowed
  it in the market maps (map 42 "Bootleggery" and maps 35/36). The server must enforce the map rule.
- **Archive**: the client's `data.saf` was repacked (−270 MB, duplicates removed as the client resolves them, 8 missing UI files
  restored from later US patches), then cleaned (476 files the client never opens removed, tested in game).
  No server impact.

- **In-world packets** (`client-world-packets.md`), the points most likely to bite:
  - Mobs are `0x06xx`, not `0x0Exx`. `0x0601` has no HP: the client's current HP starts at **0** until `0x0608`
    (`u32 id, u32 maxHP, u32 HP`, 14 bytes) arrives; the client never asks for it.
  - Damage triples in attack/skill packets are **HP, SP, MP**; recovery packets (`0x0505`, `0x0521`, `0x051F`) are **HP, MP, SP**.
    `0x050B` type: 0 HP, 1 SP, 2 MP. `0x0505` values are signed deltas, `0x0521` absolute.
  - Attack result byte: 0 hit, 1 critical, 2/3 failed, 5–11 refusals with a message (5 = out of range, as the original server's
    move-refusal `0x0502` uses).
  - Pickup request is `0x0205` with `u32 groundItemId` (6 bytes). Gold in `0x0205` is item type 26 with the amount in
    quality + gem[0..1]. Error reply = `u8 0, u8 code` (4 bytes).
  - `0x0E04` from the client = NPC clicked (`u32 npcId`); the client ignores a `0x0E04` reply.
  - Attack a player = client `0x0502` (6 bytes, `u32 playerId`). The same number is the 17-byte attack result the server
    sends (`OP_MOVE_REFUSED` in shaiya-proto is that packet with result 5), so accept the 6-byte request under `0x0502` too.
  - **Level-up `0x0508` vs `0x051E`**: both update level/points/exp; `0x051E` plays the effect and `ch_levup001.wav` at once,
    `0x0508` defers them until the next hit on that player's target is applied (the killing blow) or a quest ends. The original
    server sends `0x0508` for a level-up from the player's own kill: send it **before** the attack/skill result that kills.
  - **Shops and quests** (`07xx`, `09xx`) are in `client-world-packets.md`: the shop window fills itself from `NpcQuest.SData`;
    buy/sell replies carry the new total money; `0x0903` (quest end) does not apply `dwExp`, so send the exp change separately.
  - **Emotes are built into the client**: `/victory` … `/insult` send `0x0506` with motion 116–125; the client plays them for any
    player when the server relays `0x0506` (`u32 id, u8 motion`). Accepting those values is all that is needed.

## Updater (client side, for when patches are served from this box)

The client repo has a new updater (`rust/`, `shaiya-updater.exe`) using the original protocol: the client's `UpdaterURL.txt`
names the host, the updater fetches `http(s)://<host>/Shaiya/UpdateVersion.ini` (`[Version] CheckVersion=3`,
`PatchFileVersion=N`) and `/Shaiya/patch/psNNNN.patch` for each version above the client's `Version.ini CurrentVersion` (108 now).
Each patch must have a matching `psNNNN.patch.sig` (Ed25519, made with `shaiya-patch sign`); unsigned patches are refused.
Serving = any static HTTP server over a folder with that layout. The updater can also replace itself: `UpdaterVersion=N` in `UpdateVersion.ini` plus `/Shaiya/new_updater.exe` and its `.sig` (made with `shaiya-patch sign-updater`); leave `UpdaterVersion` out until then. The signing key stays on the dev PC, never on the server.

## Social systems (`client-social-packets.md`)

From the client's handlers and send sites, with the packet order read from the original `ps_game.exe` (same build as the PDB).
The points most likely to bite:

- **Yes/no answers are not uniform**: party (`0x0B03`), union (`0x0B15`) and trade (`0x0A02`) send **0 = yes, 1 = no**; guild
  answers (`0x0D04`, `0x0D06`, `0x0D08`, `0x0D22`) send **1 = yes, 0 = no**. The party/union answer carries the **answering
  player's own id**, not the inviter's: keep the pending inviter on the server side (the original does, 30 s).
- **Trade**: the client moves no items itself. On success the original server sends, per traded item, `0x0206` to the giver
  (bag/slot and what is left) and `0x0205` (36 bytes) to the receiver, then `0x0A04 0` to both; the client then adds
  (partner's money − own money) by itself, so do not also send a money update. `0x0A05` from the client: 0 ready, 1 unready,
  2 cancel trade (window closed). `0x0A05` to the client: `byWho` 1 you / 2 partner / 3 both, then 0 = ready, 1 = not ready.
  Any change to the offer clears both ready flags (`0x0A05 3,1`).
- **Party list**: `0x0B01` lists the *other* members only; the client appends the local player last, and the leader byte is
  an index into that order. Member units are 55 + 7×buffs bytes, packed. Party max 7 (original). The joiner gets `0x0B01`, the
  others `0x0B04`. Leaving sends `0x0B05` to everyone including the leaver; the last member left is removed too.
- **Member bars** (`0x0Cxx`) only change when the server pushes them: types are **0 HP, 1 SP, 2 MP** (the HP, SP, MP order
  again), `0x0C03` sets all three, `0x0C08` all maxima. `0x0C0A`–`0x0C14` are the same for union members (no `0x0C0F`).
- **Results that name a player** (`0x0B03` codes 0/1, `0x0D0A`) show nothing unless the id is a player the client knows (in
  view, or in its guild member list for `0x0D0A`).
- **Guild**: creation needs a full party of 7 (original rule), each member agreeing via `0x0D22`; the result the client wants
  is `0x0D21` (98 bytes, with guild id and remark; `0x0D01` is the old 33-byte form, which the original still uses for
  failures). Member events are `0x0D0C` with a state byte: < 100 level, 101 disbanded, 102 left, 103 kicked, 104 online,
  105 offline, 106 war zone, 200 + rank = rank changed. The client ignores `0x0D0B`, `0x0D0E` and `0x0D33`; the original
  server ignores the client's `0x0D01` (old create request) — handle `0x0D21`.
