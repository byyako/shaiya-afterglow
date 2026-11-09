# Confirm ledger: every unsettled claim on the public docs site

Built 2026-11-09 from the nav of `site/mkdocs.yml`. Nothing in `docs/` was changed. Not published: `notes/` is outside the site build.

## How to read it

- **Scope.** Every nav page under `docs/original/` and `docs/client/`, plus the Open entries of `docs/testing/client-research-queue.md` (queue 3 and 22 to 32; entries 1 to 21 are under "Answered"). `site/content/` pages in the nav (`index.md`, `client.md`, `guide.md`, `server.md`, `server-patches.md`, `server/run.md`) hold no unsettled claim: their only hits are the words *inferred* and *open* used as legend or in running prose. `docs/original/client-login.md` is in the nav under Server, under `original/`, so its one item is in A1 (it is not in your cluster list).
- **An item** is a statement marked inferred, open, unverified, unknown or not checked (also *unconfirmed* and *not traced*, which the client pages use), or an entry in an "Open" section. Not counted: the legend lines that define the status words, and plain uses of *unknown* or *open* in ordinary sentences ("an unknown id is dropped", "the shop is open"). Marker lines I checked and left out are those two kinds, plus statements of what the original does not check ("packet lengths are not checked", "sex is not checked"), which are facts, not open questions.
- **One id per fact.** `†` = the same fact is marked on several pages (every location is in the file:line column); `‡` = several lines of one page. A fact is filed under the first cluster it appears in; ids that span clusters are listed in the last section.
- **Columns.** *How to settle* is the primary route: ps_game.exe, ps_login.exe, other server exe (ps_session, ps_dbAgent, ps_gamewar, ps_gameLog, ps_passwordEnc, and the EP3 / EP5.4 builds of any of them), PDB, DB procs (docs/original/db and the restored databases), game.exe, live run, not answerable. A second route is named in the text where one exists. *Start from* is the address or function the page itself gives, else the nearest sibling item on the page; "none" means the page gives nothing to start from.
- **Cluster F carries the status of `docs/client/re-open-items.md`** at the front of the *how to settle* column: `[settled]`, `[partly]`, `[live]`, `[server]`, `[open]`, `[skip]`. Rows from its tables and lists carry its own status unchanged, and nothing it settled was re-sorted. That page lists its groups, not each of the ~320 statements, so a status followed by `*` was **assigned here by the page's own rule** (does the answer live in game.exe, in the original server, or only in a running client?) and is the thing to check first. Queue entries have no status in the queue file and are marked `[open]` with the route guessed.
- Some "settle" routes are guesses from the claim's wording; where the answer might sit in two places the first named is the cheaper.

## A1. Protocol and servers, part 1

| id | file:line | claim | how to settle | start from |
|---|---|---|---|---|
| A1-01 | docs/original/protocol-notes.md:79 | Algorithm of the encrypted DB connection strings (hex blobs) in the server configs is unknown; ps_passwordEnc.exe "likely" makes them | other server exe (ps_passwordEnc.exe) | ps_passwordEnc.exe main; the config reader in ps_dbAgent |
| A1-02 | docs/original/protocol-notes.md:265 | The EP4.5 variant of the "start loading the world" packet is unknown (only Gamigo ps0032 layout known) | ps_game.exe | sender of the world-load packet; compare enter-world.md section on the 0x0404/0x0105 burst |
| A1-03 | docs/original/protocol-notes.md:293 | Record structure of NpcQuest.SData is unknown until the ps_game loader is read | ps_game.exe | the NpcQuest.SData loader (the file opened by name in the data loader init) |
| A1-04† | docs/original/protocol-notes.md:383; docs/original/ep5.4.md:91 | Session-to-game message 0x108 (16 random bytes per session run): meaning unknown | other server exe (ps_session.exe) | ps_session handler for 0x108 and ps_game's reader of it |
| A1-05 | docs/original/ps_login.md:99 | Block encrypt FUN_0040cf00 uses the standard AES byte order (inferred, no known-answer test) | ps_login.exe | FUN_0040cf00; check against a real client capture (live run) |
| A1-06 | docs/original/ps_login.md:118 | The MD5 of the password is a 32-character hex string (inferred from FUN_0040a3b0 / FUN_0040b0b0) | ps_login.exe | FUN_0040b0b0 (final), CUser::ChkPasswdMD5 0x404700 |
| A1-07 | docs/original/ps_login.md:188 | 0xFC (-4) means "server saturated": EnableUserCount(list, privilege < 0xB) is the server-full check (inferred meaning) | ps_login.exe | EnableUserCount caller in the select-server path |
| A1-08 | docs/original/ps_login.md:193 | The game server's IP in the select-server reply comes from the login server's own server list, not from ps_session's 0x703 reply (inferred) | ps_login.exe | 0x703 reply builder; ps_session 0x4040e1 (settled absent); the 0xA103 builder |
| A1-09 | docs/original/ps_login.md:206 | FUN_00406e50 (the database call): which variant is used and how Status maps to the 0xA102 result byte | ps_login.exe + DB procs in docs/original/db | FUN_00406e50 |
| A1-10 | docs/original/ps_login.md:207 | Cipher not validated with a known-answer test (section 4) | live run | capture of a real client login; FUN_0040cf00 |
| A1-11 | docs/original/ps_login.md:275 | The ID and password boxes share one max-length value (19), no Tab/click path changes it (unverified by running the client) | live run | game.exe login window 0x4b1f70, 0x4b61b0 |
| A1-12 | docs/original/ps_login.md:339 | Quoting the name/password with "%s" narrows but does not close the login SQL injection (inferred) | DB procs in docs/original/db | the login procedure text, plus the formatter in ps_login |
| A1-13 | docs/original/ps_login.md:341 | MaxConnection=3000 in the shipped ps_login.ini no longer limits accepting (inferred) | ps_login.exe | the ini reader and the accept path |
| A1-14 | docs/original/ps_login.md:348 | Whether the BinToMpz write of 4 or 8 zero bytes before its buffer crashes ps_login (inferred); sections 3 and 13 not re-read by a second reader | ps_login.exe | BinToMpz callers 0x40bc50, 0x40bc57; live run to crash it |
| A1-15‡ | docs/original/ps_game.md:111,415 | The extra u32 at offset 95 of ps_session's 0x801 reply (m_nMovePositionType, from the third login-procedure result column): meaning unknown | other server exe (ps_session.exe) + DB procs in docs/original/db | ps_session 0x403c65, 0x4031ae; the login procedure's third column |
| A1-16 | docs/original/ps_game.md:149 | What the client does with 0xB103 (billingType, remainTime) is unknown | game.exe | the 0xB103 handler in the client packet switch (see client-packets) |
| A1-17 | docs/original/ps_game.md:165 | ps_session stores the 0x701 key bytes unchanged into the 0x801 reply (inferred; CUserMgr::AddUser copy not traced) | other server exe (ps_session.exe) | CUserMgr::AddUser; ps_session 0x403160 |
| A1-18‡ | docs/original/ps_game.md:268,412 | ChkMapByCharLevel's map table (needed only for the map shown on the selection screen) not written up | ps_game.exe | ChkMapByCharLevel |
| A1-19 | docs/original/ps_game.md:272 | Country value 2 means "none chosen yet" (inferred: client sends only 0 or 1) | ps_game.exe | character-list sender and the create handler (section 6.1) |
| A1-20 | docs/original/ps_game.md:284 | Meanings of the non-zero 0x0103-style database failure result values | ps_game.exe + DB procs in docs/original/db | the result switch after the dbAgent reply; the Usp_ procs' return codes |
| A1-21 | docs/original/ps_game.md:294 | Whether ps_dbAgent escapes the character name when it builds Usp_Create_Char_R as text | other server exe (ps_dbAgent.exe) | the ps_dbAgent create-char formatter |
| A1-22 | docs/original/ps_game.md:306 | Collation of the restored database (affects name comparison) not checked | DB procs in docs/original/db | the .bak / sys.databases collation (database-notes.md) |
| A1-23 | docs/original/ps_game.md:383 | Whether the database procedure refuses a create for an account that already has the name/slot state described (item 6) is unknown | DB procs in docs/original/db | Usp_Create_Char_R |
| A1-24 | docs/original/ps_game.md:385 | ps_dbAgent formats the character name into SQL text: injection risk inferred (name only has spaces removed) | other server exe (ps_dbAgent.exe) | same formatter as the escaping item above |
| A1-25 | docs/original/ps_game.md:413 | ps_dbAgent: when byWithCountry is set, and the creation stats, position and base items (CUser::ProcessCharacterCreate) | other server exe (ps_dbAgent.exe) | CUser::ProcessCharacterCreate 0x480e90 and the dbAgent create handler |
| A1-26 | docs/original/ps_game.md:416 | Keepalive timing values (section 9) | ps_game.exe | the keepalive/timeout checks in the connection update |
| A1-27 | docs/original/ps_game.md:417 | CUser PDB record places m_Crypto at +0x5520, code uses +0x5524 (alignment explanation only) | PDB | CUser type record vs the code's m_Crypto offset |
| A1-28 | docs/original/ps_game.md:434 | No reconnect path into a character (0x0112/0x0113 burst, 0x306 request absent) -- inferred from the missing handlers | ps_game.exe | the 0x0306 case in the dispatch jump table |
| A1-29 | docs/original/gamewar.md:174 | The war server's band/level-key check being skipped is inferred (ordinary ps_game reads no such keys) | other server exe (ps_gamewar.exe) | the war-server config reader and its band check |
| A1-30 | docs/original/gamewar.md:218 | The 32 bytes carried to the other server are what lets the session accept the reconnect; their origin and the war server's check not traced | other server exe (ps_gamewar.exe) | CWorld::LoginUser / EnterUserWorld; ps_session reconnect check |
| A1-31 | docs/original/gamewar.md:240 | Whether anything else holds the war server's obelisk state (kept in memory only; empty [DB] section) was not checked | other server exe (ps_gamewar.exe) | obelisk init and save paths in ps_gamewar |
| A1-32 | docs/original/enter-world.md:67 | What starts the client's world-loading step (which received packet) before it sends 0x0201 | game.exe | scene update 0x42e9fa and 0x42fa77; the state change that reaches them |
| A1-33 | docs/original/enter-world.md:97 | Meaning of the 0x0404 world clock to the client is unknown | game.exe | the 0x0404 case in the client switch |
| A1-34‡ | docs/original/enter-world.md:118,248 | 0x021B bytes are m_byResetStatus / m_byResetSkill (inferred); the meaning of 0x0404, 0x0211 and 0x021B to the client is open | game.exe | client reader of 0x021B |
| A1-35‡ | docs/original/enter-world.md:163,248 | The exact attack-speed formula (weapon Speed field and a per-weapon-type table) not written up | ps_game.exe | attack-speed calc; Items.Speed readers |
| A1-36 | docs/original/enter-world.md:194 | The item-read procedure returns bag 0 only (inferred; its text was not read) | DB procs in docs/original/db | usp_Read_Char_Items_Simple_R |
| A1-37 | docs/original/enter-world.md:196 | Statement about the gem bytes/cloak look is community knowledge, not verified here | ps_game.exe | the slot-7 handling in the item-send builder |
| A1-38 | docs/original/enter-world.md:224 | Whether the original refuses a character whose deletion is pending (the new server does) was not checked | ps_game.exe | the enter-world request handler (0x0107-style select) |
| A1-39 | docs/original/enter-world.md:246 | Zone-entry senders (SendShape, SendSpeed, CZone::SendCellInfoView, SendWeather, CMap::SendObeliskList, CUser::EnterZone) and in-world packets not written up | ps_game.exe | CUser::EnterZone |
| A1-40 | docs/original/enter-world.md:249 | Whether the client needs the "if any" packets for a character without them | live run | a character with no items/skills/quests logged in against a patched server |
| A1-41 | docs/original/enter-world.md:250 | Texts of usp_Read_Char_Items_Simple_R and usp_Read_BaseItemDefs_R | DB procs in docs/original/db | those two procs (the MSSQL container has them) |
| A1-42 | docs/original/world.md:103 | An unknown packet group in the in-world dispatch is inferred to end the connection with reason 9 too | ps_game.exe | the dispatch default case (m_nWhere +0x5758 jump table) |
| A1-43 | docs/original/world.md:263 | 0x0209 and the rest of PacketMaininterface; the friends (0x22xx) and points (0x26xx) requests the client sends; 0xB106 | ps_game.exe | PacketMaininterface dispatch |
| A1-44 | docs/original/client-extras.md:115 | Whether 0x0004 on the login connection shows the same box in the client (login server sends nothing) | game.exe | 0x54c860 (0x0004 handler) and the login-state reader |
| A1-45 | docs/original/client-extras.md:116 | The 0x0004 box's text in this client's language file, and that the client quits after it | live run | sysmsg 10070/10071 at 0x54c860 |
| A1-46 | docs/original/move-check.md:92 | A diagonal mounted step can exceed the forward-only table, about 5.22 at index 2 (inferred) | live run | compare a recorded mounted diagonal with the 0x0501 table in ps_game |
| A1-47‡ | docs/original/move-check.md:149,180 | Speed for a rider is 8.0 and for someone being carried "none" is not checked in the original | ps_game.exe | the 0x0501 speed check, mount and passenger branches |
| A1-48 | docs/original/move-check.md:167 | The client's real speeds (section 2) are not verified by a play session | live run | a logged run with positions and timestamps |
| A1-49 | docs/original/move-check.md:210 | About 46 units in 7 s allowed at one step a second at speed 5.0 (inferred from the formula) | live run | the move_check window formula against a recorded run |
| A1-50 | docs/original/client-login.md:22 | Which program reads ShaiyaUS.ini (encrypted, 330 bytes) and whether it matters is unknown | game.exe | string "ShaiyaUS.ini" xref in game.exe |

## A2. Protocol and servers, part 2: console, GM commands, log, fixes, files, databases, other episodes

| id | file:line | claim | how to settle | start from |
|---|---|---|---|---|
| A2-01 | docs/original/operator-console.md:352 | F106 world notice (u16 len, 1 to 1999 bytes) is unverified in the client | game.exe | client handler of the F106 world-notice opcode (see gm-and-commands) |
| A2-02 | docs/original/operator-console.md:363 | Data reloads other than the one in section 9, and the manager's shared-memory commands, are not built / unverified | other server exe (ps_gameManager / PSM agent) | the manager's shared-memory command table |
| A2-03 | docs/original/operator-console.md:384 | The client's copy of the data patch for the monster-skill reload is unverified | live run | a data-patch run against the client |
| A2-04 | docs/original/operator-console.md:408 | What each value of the unshipped config file changes in play is inferred from the names and the readers' places | ps_game.exe | the config readers named in the page |
| A2-05 | docs/original/gm-commands.md:247 | /server accepting: 0x0204 u8 4 to the session manager is for the login list to show it (inferred) | other server exe (ps_session.exe) | ps_session handler of 0x0204 sub 4; g_pClientToMgr sender |
| A2-06 | docs/original/gm-commands.md:191 | F701 16 exp for /level: the level's start is GetExp(grow, level-1) (inferred) | ps_game.exe | the /level case near 0x48b0e0 |
| A2-07 | docs/original/gm-commands.md:291 | A GM joining a party/guild instance zone via the name argument (inferred; an unknown or -1 choice gives the usual zone pick) | ps_game.exe | the FA0A/FA11 zone pick |
| A2-08 | docs/original/gm-commands.md:410 | F701 stat code written by SendDBGrow/Level/Exp/Money/StatPoint/SkillPoint is the matching column (inferred) | other server exe (ps_dbAgent.exe) | the dbAgent handlers for the six SendDB* messages |
| A2-09 | docs/original/gm-commands.md:411 | 0x0610 maps to F701 stat codes 5 to 10 as the base stats (inferred) | other server exe (ps_dbAgent.exe) | dbAgent 0x0610 handler |
| A2-10 | docs/original/gm-commands.md:412 | 0x0605 kind 2 stands for VG, CG, OG and IG alike (unknown) | other server exe (ps_dbAgent.exe) | dbAgent 0x0605 handler |
| A2-11 | docs/original/gm-commands.md:413 | 0x060C F701 17/18 mean kills/deaths (inferred) | other server exe (ps_dbAgent.exe) | dbAgent 0x060C handler |
| A2-12 | docs/original/gm-commands.md:427 | Client side: whether 0x051D with 13 hides the character for every viewer; whether a same-map move answered with 0x020B only leaves anything undone; whether 0x020B with y 0 grounds the character; what F701 stat 16 and stat 1 show on screen | live run | game.exe 0x051D and 0x020B handlers (client-world-packets) |
| A2-13 | docs/original/gm-commands-2.md:115 | A question from /ask-style F2xx is only delivered in the world, like the other world families (inferred) | ps_game.exe | the F2xx dispatch and its world-state check |
| A2-14 | docs/original/gm-commands-2.md:146 | Meaning of the question's kind (the three buttons' labels) is unknown | game.exe | the question window's button label code (client gm-and-commands) |
| A2-15 | docs/original/gm-commands-2.md:170 | After the skill-reset GM command nothing is sent; the client's own cooldown display keeps refusing the skills until its own timers end (inferred) | live run | game.exe skill-cooldown table; a staged reset |
| A2-16 | docs/original/gm-commands-2.md:173 | Ended applied skills (m_dwEndTime = 0) end at the next pass, with their usual 0x050E (inferred) | ps_game.exe | the applied-skill pass and the 0x050E sender |
| A2-17 | docs/original/gm-commands-2.md:242 | The six u16 stats sent are the totals (CUser::m_wStr ... +0x1220..+0x1234), the copies after stUserInfo (inferred) | ps_game.exe | CUser fields at +0x1220..+0x1234 and the stat-sync sender |
| A2-18 | docs/original/gm-commands-2.md:274 | F907 /cnotice normal range is within 30, the new server's NORMAL_CHAT_RANGE (inferred) | ps_game.exe | CObject::SendViewChat range |
| A2-19 | docs/original/gm-commands-2.md:356 | Client side: labels of the question window's four buttons (kinds 1 to 3); what the window shows of F201's second u32 and n; whether it clears itself after F202/F203; whether F801 changes anything | live run | game.exe question window and its F2xx handlers |
| A2-20 | docs/original/gm-commands-2.md:405 | F202 delivers nothing (as the original): its kinds are unknown, so its text may be a ... | game.exe | client reader of the F202 kind field |
| A2-21 | docs/original/game-log.md:459 | Which fields are unknown in the game-log record spec are to be said so in "Built" (statement of method; an item list lives in game-log section) | ps_game.exe | see the gamelog record builders |
| A2-22‡ | docs/original/game-log.md:585,591 | The game log's records point at PS_GameData (inferred); an EP4.5 ps_game and an EP5.4 ps_gameLog do not fit together (inferred) | other server exe (ps_gameLog.exe) + DB procs in docs/original/db | ps_gameLog record readers vs the ps_game senders |
| A2-23 | docs/original/community-fixes.md:155 | Whether the normal client allows the guild-bank drop onto a bound item's place (0x0204 from bag 255) was not checked | live run | game.exe guild bank drag handler |
| A2-24 | docs/original/community-fixes.md:270 | New-server drop of a monster's items: a design choice when drops are built (marked open) | not answerable | none; a design decision, not a fact |
| A2-25 | docs/original/community-fixes.md:280 | The client's skill animation being cut short when a skill is cancelled by a step (client side, not checked) | live run | game.exe skill cast cancel path |
| A2-26 | docs/original/community-fixes.md:463 | What the client does with the leftover id when a monster runs home and its speed is not updated (not checked) | game.exe | the 0x0603 / mob update handler (client-world-packets) |
| A2-27 | docs/original/community-fixes.md:639 | Forum-reported fixes (Multibuff / DEX / 4x WIS patched ps_game and others) are unconfirmed: forums would not open | not answerable | the forum threads named in the page |
| A2-28† | docs/original/community-fixes.md:660; docs/original/shop.md:112 | npc-shop-price-wrap in EP3: present (inferred, the same 32-bit imul at 0x451e70) | other server exe (EP3 ps_game.exe) | EP3 ps_game 0x451e70 |
| A2-29 | docs/original/community-fixes.md:662 | myshop-visit-cleanup in EP5.4: not checked | other server exe (EP5.4 ps_game.exe) | EP5.4 ps_game personal-shop leave path (EP4.5 0x43668e) |
| A2-30 | docs/original/community-fixes.md:664 | bound-trade in EP5.4: present (inferred) | other server exe (EP5.4 ps_game.exe) | EP5.4 ExchangeItemAdd (EP4.5 0x455bf0) |
| A2-31† | docs/original/community-fixes.md:665; docs/original/duel.md:336 | bound-duel in EP5.4: present (inferred from the stake checks 0x48c565 to 0x48c5c1) | other server exe (EP5.4 ps_game.exe) | EP5.4 0x48c565 |
| A2-32 | docs/original/community-fixes.md:666 | bound-shop in EP5.4: present (inferred from the compares in the place case of PacketMyShop 0x487370) | other server exe (EP5.4 ps_game.exe) | EP5.4 PacketMyShop 0x487370 |
| A2-33† | docs/original/community-fixes.md:667; docs/original/guild-bank.md:362,371 | guild-bank-bound in EP3 (0x42b170) and EP5.4 (0x435bc0): present (inferred) | other server exe (EP3 and EP5.4 ps_game.exe) | EP3 0x42b170, EP5.4 0x435bc0 |
| A2-34 | docs/original/community-fixes.md:673 | blacksmith-range in EP3 and EP5.4: present (inferred, PacketGem 0x452610 in EP3) | other server exe (EP3 and EP5.4 ps_game.exe) | EP3 PacketGem 0x452610 |
| A2-35 | docs/original/community-fixes.md:677 | rune-skill-cut in EP3 and EP5.4: not checked | other server exe (EP3 and EP5.4 ps_game.exe) | the rune-skill cut site (EP4.5 reference in the fix entry) |
| A2-36† | docs/original/community-fixes.md:681,684; docs/original/skills-buffs.md:476 | low-hp-rows in EP5.4: unchanged (inferred, 0x497050); illusion-save present in EP5.4 (inferred, 0x47ad70) | other server exe (EP5.4 ps_game.exe) | EP5.4 0x497050 |
| A2-37 | docs/original/community-fixes.md:684 | illusion-save in EP3: present (inferred, save filter 0x4532d2 has no exception) | other server exe (EP3 and EP5.4 ps_game.exe) | EP3 0x4532d2, EP5.4 0x47ad70 |
| A2-38 | docs/original/community-fixes.md:685 | illusion-newcomers: EP3 differs (no 0x0524, so no observer gets the monster; inferred); EP5.4 not checked | other server exe (EP3 and EP5.4 ps_game.exe) | the 0x0524 sender in EP5.4 |
| A2-39 | docs/original/community-fixes.md:688 | quest-full-bags in EP3 and EP5.4: not checked | other server exe (EP3 and EP5.4 ps_game.exe) | the quest-reward item add (EP4.5 reference in the fix entry) |
| A2-40 | docs/original/community-fixes.md:693 | nan-pos in EP3: present (inferred: CMap::IsSafePos 0x412190 identical to EP4.5) | other server exe (EP3 ps_game.exe) | EP3 0x412190 |
| A2-41 | docs/original/community-fixes.md:694 | chat-markup-filter in EP3 present (inferred: same relaying handlers, no filter seen); EP5.4 not checked | other server exe (EP3 and EP5.4 ps_game.exe) | the chat relay handlers |
| A2-42† | docs/original/community-fixes.md:696; docs/original/trade.md:225 | trade-other in EP3: present (inferred, PacketExchange 0x454fb0) | other server exe (EP3 ps_game.exe) | EP3 PacketExchange 0x454fb0 |
| A2-43 | docs/original/server-patch-notes.md:587 | A fix candidate around CUser::UpdateResetPosition is "inferred and not tried" | live run | CUser::UpdateResetPosition in ps_game; patch and try on a dev box |
| A2-44 | docs/original/server-patch-notes.md:619 | Trade with both players pressing ready at once is a race (inferred; same fact marked in trade.md) | ps_game.exe | the trade ready handler in PacketExchange |
| A2-45 | docs/original/server-patch-notes.md:624 | A zone belongs to one world thread, so the two players are always in one zone (inferred) | ps_game.exe | the world-thread dispatch (CZone owner thread) |
| A2-46 | docs/original/server-inventory.md:82 | Purposes of the databases are inferred from the names, except GameDefs/UserData/GameData | DB procs in docs/original/db | the restored databases' tables and the configs |
| A2-47 | docs/original/server-inventory.md:119 | Restore the backups (needs a SQL Server container) -- listed as open; the dev-box rehearsal in the checklist may have done it | DB procs in docs/original/db | the MSSQL container restore |
| A2-48 | docs/original/server-inventory.md:120 | Settle the client/server version match once game.exe is read | game.exe | the version check in the login window (game.exe version constant) |
| A2-49 | docs/original/server-inventory.md:121 | Run the first Ghidra analysis of ps_login.exe to answer the encryption question (the question is answered in ps_login.md) | ps_login.exe | none; stale item to close |
| A2-50 | docs/original/database-notes.md:51 | The queries ps_game runs at start-up load these tables (inferred from names; not confirmed in the binary) | ps_game.exe | the start-up load list in ps_game (CWorld data loaders) |
| A2-51 | docs/original/database-notes.md:65 | The login server authenticates in this table set and the 16-byte session identity is kept there (inferred from table names) | DB procs in docs/original/db | the login procedures and the table that holds the session id |
| A2-52 | docs/original/database-notes.md:78 | Community findings in that section are not verified against the binaries | ps_game.exe | the readers of the tables named there |
| A2-53 | docs/original/database-notes.md:238 | COMMIT fails unless the caller holds a transaction (inferred) | DB procs in docs/original/db | the proc text around that COMMIT |
| A2-54† | docs/original/database-notes.md:243; docs/original/ep5.4.md:94 | The shipped item mall cannot have worked with its own databases (inferred) | DB procs in docs/original/db | the EP5.4 item-mall procs and the databases in the pack |
| A2-55† | docs/original/ep3.md:61; docs/original/guards.md:266 | EP3 guards lack the 5-second one-guard-per-target lock (inferred) | other server exe (EP3 ps_game.exe) | the EP3 guard AI update; EP4.5 lock site |
| A2-56 | docs/original/ep3.md:96 | EP3 usp_Try_GameLogin_Taiwan accepts any password (confirmed in the file; not checked by a second reader) | DB procs in docs/original/db | usp_Try_GameLogin_Taiwan |
| A2-57 | docs/original/ep5.4.md:55 | EP5.4 item-mall purchase passes with the billing link down via the jne to jmp at 0x487fc7 (inferred); the 19 level bounds of 80 are 70 in ps_gamewar (very likely edits, inferred) | other server exe (EP5.4 ps_game.exe / ps_gamewar.exe) | EP5.4 ps_game 0x487fc7 |
| A2-58 | docs/original/ep5.4.md:70 | Level bounds with thresholds read from the wrong table row: path confirmed, outcome inferred | other server exe (EP5.4 ps_game.exe) | the level threshold lookup |
| A2-59 | docs/original/ep5.4.md:71 | The status table has no rows past 60, so levels 61 to 70 come without HP, SP and MP growth (inferred) | DB procs in docs/original/db | the status table in the EP5.4 data (GameDefs) |
| A2-60† | docs/original/ep5.4.md:91; docs/original/item-mall.md:399 | EP5.4 ps_session without [PAYLETTER] never answers a purchase (0x1B02), so the first purchase leaves the lock set (inferred; 0x108 purpose is the A1 item) | other server exe (EP5.4 ps_session.exe) | ps_session 0x1B02 handler; 0x108 sender |
| A2-61 | docs/original/ep5.4.md:112 | EP5.4 PS_ChatLog has a backdoor trigger on its chat table (confirmed in the file; not checked by a second reader) | DB procs in docs/original/db | the PS_ChatLog trigger text |
| A2-62† | docs/original/ep5.4.md:149; docs/original/skills-buffs.md:465 | Shape 103 (buff 97) in EP5.4 hides the character from every viewer: "hidden" is inferred from the flag (0x4e8cdf) | game.exe | client shape switch 0x4e8b39 / 0x4e8cdf (EP6 client) |

## B. Characters and items

| id | file:line | claim | how to settle | start from |
|---|---|---|---|---|
| B-01† | docs/original/stats.md:148; docs/original/combat.md:283 | With the 6x71 status table and StatusDefs ending at level 60, levels 61 to 70 get 0 and max HP, SP and MP drop at the step to 61 (inferred) | other server exe (EP5.4 ps_game.exe) + DB procs in docs/original/db | EP5.4 ps_game 0x4d83b2 (table) and the level-up stat calc; the StatusDefs rows |
| B-02 | docs/original/items.md:106 | The purpose of the 0x0204 freeze (CUser+0x5880, set by special 59 for 5 s) is the move to the war server (inferred) | other server exe (EP5.4 ps_game.exe) | EP5.4 ps_game 0x4760ed and the special-59 handler |
| B-03† | docs/original/item-drop.md:123; docs/original/combat.md:267 | EP3 monster drops roll with rand() only: no SRandom call, no 1,000,000 roll (inferred from the calls) | other server exe (EP3 ps_game.exe) | EP3 CMob::ItemDrop 0x487050 |
| B-04† | docs/original/item-flags.md:238,241; docs/original/pvp.md:333 | EP3 death drops have no honour rule (inferred from the constants); EP3 resurrection runes, gold drops and ultimate-mode rules not compared | other server exe (EP3 ps_game.exe) | EP3 ItemDropByUser 0x443df0, KillSoul 0x443650, FindItemRebirth 0x448e90 |
| B-05† | docs/original/item-use.md:396; docs/original/auction.md:513 | EP5.4 specials 61, 76 to 78, 85 to 98 (including the remote auction house scroll, special 61) are on items but no code compares them: inferred unused by this build | other server exe (EP5.4 ps_game.exe) | EP5.4 special-use switch around the tables 0x56e608, 0x56e628, 0x56e648 |
| B-06 | docs/original/item-windows.md:63 | The client's Teleportation Stone window sends 0x0557 for the saved-position move (inferred) | game.exe | the teleport stone window's send call (client-packets 0x0557) |
| B-07 | docs/original/item-origins.md:197 | EP5.4 box generator is reseeded with time(0) xor character id at every use, so two boxes opened in the same second roll the same (inferred) | other server exe (EP5.4 ps_game.exe) | EP5.4 ps_game 0x473b2e |
| B-08 | docs/original/item-origins.md:201 | Two box name sets absent from the file (1028, 1031) always fail (inferred) | other server exe (EP5.4 ps_game.exe) | EP5.4 ps_game box open handler 0x473aaa to 0x473b6e |
| B-09 | docs/original/lapis.md:385 | EP5.4 link request 0x0801 order is item then hammer (inferred for EP5.4; confirmed for EP6 client 0x4a9b8d) | other server exe (EP5.4 ps_game.exe) | EP5.4 0x0801 handler argument order |
| B-10 | docs/original/lapis.md:407 | How the reload of the lapis table is reached in EP5.4 (the /reitemenh console word is gone) was not traced | other server exe (EP5.4 ps_game.exe) | EP5.4 0x4dc297 and 0x4dc2e6 callers |
| B-11 | docs/original/repair.md:138 | Whether game.exe lowers the qualities it shows by itself when an item wears (and so asks the right repair price) is not known | game.exe | the wear/quality code in the item view and the repair request price |
| B-12 | docs/original/shop.md:101 | The +0x142c state that ignores shop packets (likely dying or dead) | ps_game.exe | writers of CUser +0x142c |
| B-13 | docs/original/shop.md:102 | What the original does with items in bags above m_nMaxBag | ps_game.exe | the bag loops in the shop/bag handlers (m_nMaxBag users) |
| B-14 | docs/original/item-mall.md:317 | The item-mall windows' opening (store button; Light's/Fury's Bank Teller) is known from code, to be confirmed live | live run | ins_market_store_button click handler |
| B-15 | docs/original/item-mall.md:318 | A gift taken while the item-mall storage window is open, and the billing storage delivered while in the world, behave as described | live run | an item-mall gift run against a client |
| B-16 | docs/original/item-mall.md:320 | What message 816 (premium type 3) says; type 2 has no date by design | game.exe | the sysmsg table entry 816 in the English text and the premium window code |
| B-17 | docs/original/item-mall.md:321 | Whether the client shows 0x2604 n = 0 notices correctly right after the burst | live run | game.exe 0x2604 handler |
| B-18 | docs/original/item-mall.md:395 | EP5.4 usp_Rollback_User_GiftPointItems save order (inferred) | other server exe (EP5.4 ps_dbAgent.exe) | ps_dbAgent 0x407d3d, 0x407f8d, 0x423b90 |
| B-19 | docs/original/personal-shop.md:293 | Whether a shop title goes through the chat text drawing in the client (asked in the client research queue, entry 13 b) | game.exe | the shop sign drawing at 0x0523 handler |
| B-20 | docs/original/personal-shop.md:307 | Whether the EP3 personal shop has the visitor clean-up and the bound-item check was not checked | other server exe (EP3 ps_game.exe) | EP3 0x45e4dc neighbourhood, the leave-zone path |
| B-21 | docs/original/personal-shop.md:314 | EP5.4 placing (0x2305): missing ReqOg read inferred | other server exe (EP5.4 ps_game.exe) | EP5.4 0x2305 handler (PacketMyShop 0x487370) |
| B-22 | docs/original/auction.md:118 | Which auction-row fields the client reads for display (client auction-house.md lists it Open) | game.exe | auction list row reader (see F cluster auction-house) |
| B-23 | docs/original/auction.md:510 | EP5.4 auction packets keep their wire size (inferred) | other server exe (EP5.4 ps_game.exe) | EP5.4 market send functions |
| B-24 | docs/original/warehouse.md:162 | Whether the EP3 client shows 40 free slots was not checked | live run | EP3 client (not in hand?) or EP3 server 0x44f326 |
| B-25 | docs/original/trade.md:208 | That a zone belongs to one world thread is inferred (same fact as server-patch-notes.md:624) | ps_game.exe | world thread dispatch |
| B-26 | docs/original/trade.md:231 | EP5.4 duel flag (CUser+0x558c) and personal shop flag (CUser+0x1638) meanings are inferred from their writers | other server exe (EP5.4 ps_game.exe) | writers of +0x558c and +0x1638 |
| B-27 | docs/original/trade.md:238 | The 0x0A0A fix of section 6 targets an EP5.4-type binary, which has no 0x0A0A of its own (inferred) | other server exe (EP5.4 ps_game.exe) | EP5.4 send call 0x4921a8, SConnection::Send 0x4ED0E0 |
| B-28 | docs/original/vehicles.md:145 | 48 mounts (shape up to 38): the server passes the number unchecked, and whether the client draws it was not checked (inferred) | game.exe | the mount shape reader in the client (client-world-packets 0x0516-style) and live run |
| B-29 | docs/original/small-items.md:85 | The kick notice goes to the chat window like 339 (inferred from the handler table; not traced) | game.exe | the F501 handler table in game.exe (gm-and-commands) |
| B-30 | docs/original/small-items.md:237 | Client side: how the client shows an 0x1108 whose name is its own character, and whether it keeps a "message to server" state after a ... | live run | game.exe 0x1108 handler |
| B-31 | docs/original/small-items.md:343 | The answer on the name-change path (0x40591f) was not traced | ps_game.exe | 0x40591f and the usp_Save_Char_Name_E result mapping |
| B-32 | docs/original/small-items.md:429 | Which NPC's menu sends 0x0F01/0x0F02 is not known on the server side (open for the client) | game.exe | the NPC menu type 14 handler (see research-priest-packets) |
| B-33 | docs/original/small-items.md:520 | COMMIT in a small-items procedure holds a transaction (inferred) | DB procs in docs/original/db | the proc text (same fact family as database-notes.md:238) |

## C. Combat and skills

| id | file:line | claim | how to settle | start from |
|---|---|---|---|---|
| C-01 | docs/original/combat.md:268 | The EP3 helper that puts a dropped item on the ground returns for real types 16 and 24 (0x48763a); the surrounding roll claim is inferred | other server exe (EP3 ps_game.exe) | EP3 0x48763a |
| C-02 | docs/original/combat.md:288 | Meaning of the 0x0527 packet (no payload, sent from PacketPC and the attack tick; EP5.4 client reads nothing) is open | game.exe | the client 0x0527 handler (EP6 client) and EP5.4 ps_game 0x4920d0 senders |
| C-03 | docs/original/skills.md:184 | A request the original leaves without a reply (unknown skill at level 2 or 3, a race that may not learn it) gets result 3 in the new server (statement of design, no marker); the original's silence is confirmed | not answerable | none; a deliberate deviation |
| C-04 | docs/original/skills.md:209 | Who reads the new TypeDetail/1000 field (EP5.4 0x4d914e to 0x4d916b) was not checked | other server exe (EP5.4 ps_game.exe) | EP5.4 ps_game xrefs to the new CSkill field |
| C-05 | docs/original/skills.md:215 | EP5.4 skill data types 50 and 51 fall outside the switch and do nothing (inferred) | other server exe (EP5.4 ps_game.exe) | EP5.4 ps_game 0x495952 switch bounds |
| C-06 | docs/original/skills-use.md:208 | The skill's wAggro (column arrow) is added to the aggro with the hit (inferred; CDamageMob::Add not read again) | ps_game.exe | CDamageMob::Add and its caller in the skill hit path |
| C-07 | docs/original/skills-use.md:388 | A combo skill needs no previous skill when buff 96 skips the combo check (skip confirmed, meaning inferred) | ps_game.exe | combo check at the 0x49462d buff-96 flag readers |
| C-08 | docs/original/skills-buffs.md:71 | m_dwID of the skill object is the key the client names a buff by in 0x050E and in a cancel (inferred from its use) | ps_game.exe | CObjectMgr::CreateSkill and the 0x050E sender |
| C-09 | docs/original/skills-buffs.md:278 | Damage-over-time debuff current-value arithmetic (at least 1) is by analogy with damage types 4 and 5 (inferred; arithmetic hidden in _ftol2 calls) | ps_game.exe | the apply-skill damage code, the _ftol2 call sites |
| C-10 | docs/original/skills-buffs.md:438 | The client's icons for the kept buffs after a duel death (live test 50); a guild war death has no test of its own | live run | live test 50 in docs/testing/live-test-checklist.md |
| C-11 | docs/original/skills-buffs.md:464 | Meaning of buff 96 (skips the combo prerequisite) is inferred; the flag byte is confirmed | ps_game.exe | EP4.5 flag byte 0x49462d / 0x494b4d readers |
| C-12 | docs/original/skills-buffs.md:473 | In EP5.4, when the stripping check for an applied skill runs is inferred | other server exe (EP5.4 ps_game.exe) | EP5.4 ps_game 0x495d48 to 0x495e12 caller |
| C-13 | docs/original/skills-buffs.md:481 | The meaning of the new EP5.4 flags is confirmed client side and inferred for the server | other server exe (EP5.4 ps_game.exe) | the server immediates named in the section |
| C-14 | docs/original/skills-special.md:183 | On death the client clears hiding/transformation by itself, so the server sends no 0x051D or 0x050E (inferred) | game.exe | the client death handler that clears applied skills |
| C-15 | docs/original/skills-special.md:359 | EP5.4 special skill triggers match those of hiding (constants confirmed, triggers inferred) | other server exe (EP5.4 ps_game.exe) | the trigger sites around the confirmed constants |
| C-16 | docs/original/thorns-fields-pvp.md:219 | What the client draws for 0x0513 from a buff or a field (client research queue entry 3; live test 46) | live run | game.exe 0x0513 handler and live test section 46 |
| C-17 | docs/original/pvp.md:353 | EP5.4 reward panels make the same calls as in EP4.5 (inferred, offsets moved, 0x4670a0) | other server exe (EP5.4 ps_game.exe) | EP5.4 ps_game 0x4670a0 |
| C-18 | docs/original/pvp-2.md:375 | EP8 item-grade bonuses (costs by 10 %, item cast times at those grades) are inferred from a tool's names only; no EP8 code read | not answerable | no EP8 binary in hand |
| C-19 | docs/original/guild-war.md:178 | Whether guild war "ready" is checked when both sides have the other's guild as enemy (ready not checked) | ps_game.exe | the guild-war start/ready handlers |
| C-20 | docs/original/guild-war.md:257 | EP5.4 guild-war lock releases on more exits and PvPReset, PvPUserClear, PvPUserLeave, SendPvPJoinUser, PvPChkLeaveBy* match EP4.5 (inferred) | other server exe (EP5.4 ps_game.exe) | the EP5.4 functions of those names |
| C-21 | docs/original/obelisks.md:260 | Room for 64 portal links (stPortalLink[64]) and 64 maps of 64 groups: a 65th overruns (not checked) | ps_game.exe | LoadObelisk / Obelisk.ini parser array bounds |
| C-22 | docs/original/obelisks.md:305 | Road links are mutually exclusive (taking one road closes the other faction's); the intent is inferred, the code verified | ps_game.exe | the pairing code in the obelisk change handler |
| C-23 | docs/original/obelisks.md:528 | EP3's obelisk reader reads four roads from the shared parser, and the roads most likely never worked in EP3 (inferred); where EP3 sets them up not located | other server exe (EP3 ps_game.exe) | EP3 LoadObelisk, portal 1049 handling |
| C-24 | docs/original/obelisks.md:534 | EP5.4 obelisk code is the same except the confirmed SendObeliskList 0x4316e0 and SendObeliskChange 0x431860; the rest inferred from matched constants | other server exe (EP5.4 ps_game.exe) | EP5.4 0x4316e0, 0x431860 |
| C-25 | docs/original/obelisks.md:542 | In EP5.4 the section-1 value becomes 12288 / 12 + 1 = 1025 (inferred); one guard line per relic lacks its ")" and the tokenizer ignores it (inferred) | other server exe (EP5.4 ps_game.exe) | EP5.4 obelisk loader tokenizer |

## D. Social

| id | file:line | claim | how to settle | start from |
|---|---|---|---|---|
| D-01 | docs/original/party.md:341 | The party-mark type byte relayed as 0x0520 is not read by the server's own party state (inferred) | ps_game.exe | the 0x0520 relay in PacketParty (EP5.4 0x475fa4) and CParty readers |
| D-02 | docs/original/guild.md:351 | EP4.5's five extra guild messages (0xB13 to 0xB17: guild house, NPC upgrades) are named from the message names only (inferred) | other server exe (ps_dbAgent.exe) | EP4.5 ps_dbAgent jump table entries 0xB13 to 0xB17 |
| D-03 | docs/original/guild-house.md:323 | EP5.4 guild house rate functions keep their constants (inferred) | other server exe (EP5.4 ps_game.exe) | EP5.4 ps_game 0x437960 and the rate functions |
| D-04 | docs/original/guild-ranking.md:204 | Guild ranking list is ordered by guild id first (inferred from the container) | ps_game.exe | the ranking list build in UpdateGuildPointRank |
| D-05 | docs/original/guild-ranking.md:291 | EP5.4 two-beat timing that can skip a notice is unchanged (inferred) | other server exe (EP5.4 ps_game.exe) | EP5.4 CheckGuildRankNotices (EP4.5 0x41a780) |
| D-06 | docs/original/guild-ranking.md:292 | EP5.4 UpdateGuildPointRank and SendRankingZone keep their constants (inferred); GetGuildRankJoinCount has no match (inlined or removed, not settled) | other server exe (EP5.4 ps_game.exe) | EP5.4 functions of those names |
| D-07 | docs/original/guild-bank.md:234 | That the second byte of the guild-bank error is not shown by the client is inferred (client notes give only the texts) | game.exe | the guild-bank result handler and its message table |
| D-08 | docs/original/guild-bank.md:253 | Guild bank item rows (Usp_Save_Guild_StoredItem_Add/Mod/Del) run on the save timer like other GuildMgrSave writes (inferred) | other server exe (ps_dbAgent.exe) | GuildMgrSave::DBSaveItem |
| D-09 | docs/original/guild-bank.md:267 | Whether usp_Delete_Guild_E deletes or keeps the GuildStoredItems rows is unknown (body not in the pack) | DB procs in docs/original/db | usp_Delete_Guild_E in the restored database |
| D-10 | docs/original/guild-bank.md:279 | What the client shows for the guild bank's (bag 255) item-move answer is inferred (client notes do not say) | game.exe | the 0x0204 answer handler for bag 255 |
| D-11 | docs/original/guild-bank.md:282 | Client side: what window 0x72 shows and which moves it sends; whether 0x0D29 clears slots it does not list; whether leaving a guild clears the bank | live run | game.exe 0x0D29 handler and DAT_021e5be0 writers |
| D-12 | docs/original/quest.md:288 | dwTimeMin, unused by the data, is not checked (not built) | not answerable | none; a build choice |
| D-13 | docs/original/quest.md:311 | EP3 quest fault 3 (full bags) was not checked | other server exe (EP3 ps_game.exe) | EP3 quest reward add (EP4.5 reference in community-fixes quest-full-bags) |
| D-14 | docs/original/mini-game.md:349 | Everything of section 11 (the gambling window, the reels, what happens on standing up) is unverified in the client | live run | game.exe gambling window (see research-gambling-kill-boost) |
| D-15 | docs/original/mini-game.md:407 | The play of the gambling house with eight gambling NPCs in EP5.4 is inferred; nothing was run | live run | EP5.4 CMap::InitNPC 0x42fb50 (placement is confirmed); run on a dev box |

## E. World and monsters

| id | file:line | claim | how to settle | start from |
|---|---|---|---|---|
| E-01 | docs/original/maps-npcs.md:33 | Map files 100 to 108 not in Map.ini are likely for ps_gamewar.exe (not checked) | other server exe (ps_gamewar.exe) | the war server's map list loader (Map.ini or MapWar.ini) |
| E-02 | docs/original/maps-npcs.md:74 | The NPC type names (1 merchant ... 13 combat commander) are community names, not verified here | ps_game.exe | CMap::InitNPC type switch |
| E-03 | docs/original/maps-npcs.md:117 | Regions and weather timing are still open | ps_game.exe | CZone::SendWeather and the region loader |
| E-04† | docs/original/maps-npcs.md:138; docs/original/instances.md:459 | Map files 80, 98 and 99 lie past MapCount and are not loaded (inferred); 98 and 99 are copies of a Cailum Sacra section | other server exe (EP5.4 ps_game.exe) | the EP5.4 map loader MapCount loop |
| E-05 | docs/original/portals.md:77 | Open items: 0x0227, instance zones, guild houses and the guild ranking battle portals (other pages cover them) | ps_game.exe | the portal handler 0x020A / 0x0227 case |
| E-06 | docs/original/portals.md:78 | Bind areas and respawning (CMap::GetBindPos, m_nMovePositionType 0) | ps_game.exe | CMap::GetBindPos |
| E-07 | docs/original/portals.md:104 | Maps 62 and 63 are unreachable except by rebirth or a GM move in EP5.4 (inferred) | other server exe (EP5.4 ps_game.exe) | EP5.4 portal and gatekeeper tables for maps 59 to 63 |
| E-08 | docs/original/gatekeeper.md:75 | The other m_nMovePositionType values: 0 return to the bind position, 2 to 6 (town portal scrolls, recall, 0x0222 cancel) and their delays | ps_game.exe | the m_nMovePositionType switch in the move-delay handler |
| E-09 | docs/original/gatekeeper.md:77 | Portals (0x020A, CUser and CMap portal data in the svmaps) | ps_game.exe | the 0x020A handler |
| E-10 | docs/original/mobs.md:165 | Maps 73 and 75 to 79 spawn ids above 3087, which section 1 never spawns (inferred) | other server exe (EP5.4 ps_game.exe) | the EP5.4 spawn loop and the svmap spawn data |
| E-11 | docs/original/mobs.md:168 | EP5.4 operator command 0xF70F uses the same monster sets (inferred from the strings); the shipped sets name monsters without HP or missing sets so nothing spawns (inferred); how the summon places the monsters not followed | other server exe (EP5.4 ps_game.exe) | EP5.4 handler of 0xF70F |
| E-12‡ | docs/original/chase-smoothing.md:235,241 | EP3 and EP5.4 CMob::Chase step times are most likely unchanged (inferred: bodies not compared step by step; EP3 0x487d80, EP5.4 0x4bb8f0) | other server exe (EP3 and EP5.4 ps_game.exe) | EP3 0x487d80 and 0x4949a0, EP5.4 0x4bb8f0 |
| E-13 | docs/original/scripted-mobs.md:447 | EP5.4 scripted-monster case bodies match the catalogue of sections 4 to 6 (inferred: case bodies not compared) | other server exe (EP5.4 ps_game.exe) | EP5.4 TravelCondition 0x4ac2f0, CalculateParam 0x4b0f50, ConnectToFSM 0x4a29b0 |
| E-14† | docs/original/scripted-mobs.md:455; docs/original/instances.md:462 | EP5.4 LuaDeleteMob's fifth argument is the radius that EP4.5 fixes at 100 (inferred; the read is confirmed; instances.md says "was not checked") | other server exe (EP5.4 ps_game.exe) | EP5.4 LuaDeleteMob 0x4cb790 |
| E-15 | docs/original/guards.md:273 | Map 62's blocked-cell bitmap is empty so guards there are not kept out of walls (inferred from maps-npcs.md) | ps_game.exe | the map 62 svmap blocked cells and the guard walk check |
| E-16 | docs/original/instances.md:218 | A zone's walk timer being set by WhileIdle means a function does nothing (inferred from the shared timer) | ps_game.exe | the function around +0xcbc in the guard/mob idle code |
| E-17‡ | docs/original/instances.md:341,378 | LuaRecallUser's first argument is the job byte +0x134 (inferred from its uses); an odd filter, no script uses it (inferred) | ps_game.exe | LuaRecallUser and +0x134 readers |
| E-18 | docs/original/instances.md:450 | EP3: the instance party term inferred; Lua absent (confirmed); maps 45 and up do not exist | other server exe (EP3 ps_game.exe) | EP3 CMap instance code |
| E-19 | docs/original/instances.md:457 | EP5.4 new EnterMaxUser and ExpireTime values on maps 64 to 66 do nothing (inferred) | other server exe (EP5.4 ps_game.exe) | EP5.4 readers of the instance keys |
| E-20 | docs/original/instances.md:467 | EP5.4 LuaGetMobUID mode/type/radius reading of the loop (inferred) | other server exe (EP5.4 ps_game.exe) | EP5.4 LuaGetMobUID 0x4cba70 |

## F. Client pages and the client research queue

| id | file:line | claim | how to settle | start from |
|---|---|---|---|---|
| F-01† | docs/client/re-open-items.md:21; docs/client/packet-opportunities.md:68 | Feature 46 (auto-dismount): who replays the queued action (0x4c49c0 via 0x416ba0; queue globals 0x21e8988 to 0x21e8994) | [settled] game.exe; the server answer producing the state change stays live (see the live row) | 0x416ba0, 0x4c17a0, 0x4c49c0 |
| F-02† | docs/client/re-open-items.md:22; docs/client/re-engine.md:603 | [VIDEO] TEXTURE: a reader of 0x70196c (it is a texture size limit) | [settled] game.exe | 0x4444d0, 0x52abd0 |
| F-03† | docs/client/re-open-items.md:23; docs/client/re-gameplay-state.md:399 | Ground item count (0x0401) kept by the client? (no, 0x55acb0 never reads it) | [settled] game.exe | 0x55acb0 |
| F-04† | docs/client/re-open-items.md:24; docs/client/re-network.md:473 | Opcode 0x0002 behaviour beyond its message (sysmsg 10069, main state 0, WM_CLOSE) | [settled] game.exe | 0x54c7e0 |
| F-05† | docs/client/re-open-items.md:25; docs/client/re-network.md:473 | Opcode 0x0004 behaviour (sysmsg 10070/10071, reset, WM_CLOSE) | [settled] game.exe | 0x578ed0, 0x54c860 |
| F-06† | docs/client/re-open-items.md:26; docs/client/re-network.md:358,460; docs/client/packet-opportunities.md:229 | Opcode 0x0111: the handler is read (new network object, state 6); its purpose, a hop between game servers, stays inferred | [partly] ps_game.exe or ps_gamewar.exe (which server sends it) | client 0x54c910; server side: the war-server reconnect in gamewar.md |
| F-07† | docs/client/re-open-items.md:27; docs/client/client-social-packets.md:321; docs/client/packet-opportunities.md:556 | Opcode 0x0D0C: which guild button sends 0 and which 1 is read (0x4e35e0); which label is demote and which promote stays inferred | [partly] game.exe | button objects at window +0x2d8c and +0x3260; label draw code |
| F-08† | docs/client/re-open-items.md:28; docs/client/client-actions.md:74; docs/client/auction-house.md:47; docs/client/CLIENT-FACTS.md:72 | Opcode 0x1C03 direction byte is read (0x459f60: action 7 sends 1, action 8 sends 2); that they are the previous and next buttons stays inferred | [partly] game.exe | 0x459f60 and the caller that maps the clicked button to the action |
| F-09† | docs/client/re-open-items.md:29; docs/client/research-priest-packets.md:17,88,131 | A 0x0F02 u8 1 reply crashes the client: the null table read is certain in code (0x479390); the crash itself is inferred, strong | [partly] live run (code confirmed, behaviour live) | 0x479390 and the Priest window 0x4d0770 |
| F-10† | docs/client/re-open-items.md:30; docs/client/obelisks.md:23 | Opcode 0x2104 (road change): the client never sends it and prints nothing | [settled] game.exe | immediate scan for 0x2104 |
| F-11† | docs/client/re-open-items.md:38; docs/client/research-priest-packets.md:140,141 | A 0x0F02 u8 1 reply crashes the client; what the four Korean priest buttons look like in the US client | [live] live run | the Priest window run with a debug hook opening it with 0x0F01 u8 1 |
| F-12† | docs/client/re-open-items.md:39; docs/client/research-priest-packets.md:142,112 | Whether a type-14 NPC is clickable and sends 0x0F01 (teleport inferred) | [live] live run | a type-14 NPC next to a test character |
| F-13† | docs/client/re-open-items.md:40; docs/client/research-priest-packets.md:113,143 | State +0x279 = 2 is death (death closes the priest window); Escape closes it | [live] live run | the Priest window open run |
| F-14† | docs/client/re-open-items.md:41; docs/client/re-engine.md:608 | Resolutions outside the list use layout 1 and aspect 1.25: how it looks | [live] live run | a run at an unlisted resolution |
| F-15† | docs/client/re-open-items.md:42; docs/client/packet-opportunities.md:988,989 | Region features whose effect is only read in code (8, 23, 24, 41, 43, 45, 62; also 5, 46): every effect line is a code reading, none was run | [live] live run | packet-opportunities.md region feature table |
| F-16† | docs/client/re-open-items.md:43; docs/client/packet-opportunities.md:979 | Feature 46: the server's 0x0216 answer produces the state change that replays the queue | [live] live run | mount, skill and server together |
| F-17† | docs/client/re-open-items.md:44; docs/client/packet-opportunities.md:979 | 0x0004's message box; 0xB106 changing a mall entry; feature 0's request and answer (8-byte request, 34-byte answer, gem choice) | [live] live run | a sender for each (patched server) |
| F-18† | docs/client/re-open-items.md:45; docs/client/client-actions.md:245,246; docs/client/CLIENT-FACTS.md:181 | Gruesome Attack and Enhance Wind Spin weapon checks (the client sends, the server decides: inferred); Healing, Healing Prayer, Recovery cast bars | [live] live run | the weapon-check paths 0x4f12c0, 0x4d4e10, 0x43c000 |
| F-19 | docs/client/re-open-items.md:46 | 0x0F02 exactly 10 gold: client sends, server refuses | [live] live run | the Priest window run with 10 gold |
| F-20† | docs/client/re-open-items.md:47; docs/client/research-movement-storages-auction.md:47 | Combined forward and strafe speed can exceed the forward-only table (about 5.22 at index 2) (inferred); camera-button double buffer (shipping.md) | [live] live run | the 0x0501 reporter; a recorded run |
| F-21† | docs/client/re-open-items.md:48; docs/client/graphics.md:87,88,89; docs/client/qol-feasibility.md:105 | Graphics claims: unproven performance benefit of the DLL controls; title/costume hiding paths; map preloading, 16x anisotropy look, texture-reader cost | [live] live run | a timed run with the option off and on |
| F-22† | docs/client/re-open-items.md:52; docs/client/personal-shop.md:30; docs/client/client-actions.md:78 | Personal-shop commission 5 % (ROUND(price x 0.05), factor inferred from message 5357) and the auction commission rounding round(minBid x 0.000002) x hours x 5 | [server] ps_game.exe | the shop price handler (EP4.5 0x478450 family) and the auction commission calc |
| F-23† | docs/client/re-open-items.md:52; docs/client/client-social-packets.md:244,356; docs/client/guild-wars.md:26 | Guild rank meaning: rank < 5 or < 3, master = 1 (inferred from the original's name) | [server] ps_game.exe | the guild rank checks in PacketGuild (kick/invite rank < 3) |
| F-24† | docs/client/re-open-items.md:52; docs/client/packet-opportunities.md:124,991 | Guild house cost field +8 is Etin (inferred) | [server] ps_game.exe | the 0x0D25/0x0D34 guild-house buy path |
| F-25† | docs/client/re-open-items.md:52; docs/client/client-social-packets.md:123 | The unit of the guild-war time field (milliseconds vs the original's seconds; inferred) | [server] ps_game.exe | the guild-war request sender in ps_game |
| F-26† | docs/client/re-open-items.md:52; docs/client/duels.md:64 | ItemDropPvP pays the stakes to the winner (inferred from the name) | [server] ps_game.exe | CUser::ItemDropPvP |
| F-27† | docs/client/re-open-items.md:52; docs/client/client-world-packets.md:61,78; docs/client/packet-opportunities.md:394,598; docs/client/scripted-monsters.md:51 | byMotion values mean walk/run on the server's side (0x0603 mob move, 0x0E03 NPC move) (inferred) | [server] ps_game.exe | the mob and NPC move senders (CMob::SendMove, CNpc::SendMove) |
| F-28† | docs/client/re-open-items.md:52; docs/client/packet-opportunities.md:260,263; docs/client/CLIENT-FACTS.md:119 | Faction kill-count packets 0x020F and 0x0211 and the "blessing of the goddess" event (inferred) | [server] ps_game.exe | CWorldThread::SendAllKillCount, CUser::SendKCStatus |
| F-29† | docs/client/re-open-items.md:52; docs/client/packet-opportunities.md:493,526,527,528,529,530,531,532 | Union packets 0x0B0E and 0x0C0B to 0x0C12 are inferred from their sharing a handler with the party versions | [server] ps_game.exe | the union cases in CUser::PacketParty and the senders named in the table |
| F-30† | docs/client/re-open-items.md:52; docs/client/packet-opportunities.md:827,828,829,981 | 0xA104 (login notice / server versions) and 0xA105 (OTP card login) on the login server (inferred, not traced) | [server] ps_login.exe | the 0xA104 and 0xA105 cases in the ps_login dispatch |
| F-31† | docs/client/re-open-items.md:52; docs/client/packet-opportunities.md:62,199,981 | Who sends 0x0004 first (the session server's duplicate-login kick is inferred) | [server] other server exe (ps_session.exe) | the duplicate-login path in ps_session |
| F-32† | docs/client/re-open-items.md:64; docs/client/re-gameplay-state.md:397 | Stat bonuses 0x84d6a4 to 0x84d6b8: no instruction writes these absolute addresses; writers reach them through the object at 0x84d668 (+0x3c to +0x50) | [open] game.exe | writers via 0x84d668; reader 0x4f3f80; loop 0x46b7e0 |
| F-33† | docs/client/re-open-items.md:65; docs/client/client-world-packets.md:216 | Mob byMotion (0x0603) and NPC motion flag (0x0E03): 0x54e810 stores it at mob +0x84; whether anything reads it | [open] game.exe | the mob per-frame update, readers of +0x84 |
| F-34 | docs/client/re-open-items.md:66 | Which guild button is demote and which promote (0x0D0C) (the label draw code) | [open] game.exe | window +0x2d8c and +0x3260 draw/label code |
| F-35 | docs/client/re-open-items.md:67 | Auction page buttons: actions 7 and 8 of 0x459f60 (the caller that maps the clicked button) | [open] game.exe | callers of 0x459f60 |
| F-36† | docs/client/re-open-items.md:68; docs/client/re-gameplay-state.md:144,395 | Warehouse (0x0711) and other storages in memory: the arrays and their handlers | [open] game.exe | 0x0711 handler and the arrays it fills |
| F-37† | docs/client/re-open-items.md:69; docs/client/re-gameplay-state.md:324,387,394,396 | Selected target, guild member list and ranks: where the click stores the target; the list storage; guild points | [open] game.exe | 0x43c000 target kinds 1 to 4; the guild member list storage |
| F-38† | docs/client/re-open-items.md:70; docs/client/re-gameplay-state.md:190; docs/client/item-sdata-vs-defs.md:49; docs/client/skill-monster-sdata-vs-defs.md:8 | Item and skill fields the client ignores (a scan of every use of the three record pointers); what the client does with its own value is inferred unless marked | [open] game.exe | every use of the item, skill and monster record pointers |
| F-39† | docs/client/re-open-items.md:71; docs/client/re-engine.md:301,454,593,602,605,606,607 | Scene states 8 and 9 (which window returns 4 for state 8); UI slot 5 meaning; FPS overlay flag (renderer +0x410) writer; error reporter 0x40c4c0 sink | [open] game.exe | 0x40c4c0, 0x40a2a0, 0x5a41b0; scene function around state 8 |
| F-40† | docs/client/re-open-items.md:72; docs/client/re-engine.md:167,604 | data/Config.cfg keys (opened with the same parser as gsConfig.cfg, 0x5a8cd0; the US build ships no such file) | [open] game.exe | the key names inside the parser 0x5a8cd0; callers 0x40a774, 0x5a9343, 0x4bc406 |
| F-41† | docs/client/re-open-items.md:73; docs/client/re-engine.md:609,510 | Per-pass drawing inside 0x447d30 (draw order of terrain, models and effects) | [open] game.exe | 0x447d30 |
| F-42† | docs/client/re-open-items.md:74; docs/client/re-network.md:386,422,423,462,475,476 | Network: accumulation buffer capacity at 0x222aa40 and its overflow (no check found); the click and drawing steps of the worked examples (ground item pickup, HP bars) | [open] game.exe | the receive buffer size at 0x222aa40; the ground item click handler; the HP bar drawing |
| F-43† | docs/client/re-open-items.md:75; docs/client/formats.md:94 | FUN_00430280: the optional Data/World/<id>.svmap read and its feature flag ("not verified") | [open] game.exe | FUN_00430280, the flag's origin |
| F-44† | docs/client/re-open-items.md:79; docs/client/packet-opportunities.md:801,802,803,805,807,809,810,811,813,815,817,818; docs/client/client-systems.md:41,52,53 | The 31xx mini-game packets (0x3101 to 0x3109) and the 27xx item-list and 0Fxx / 12xx families: purpose only inferred from names and layouts | [skip] not answerable | none; settled enough by research-gambling-kill-boost.md |
| F-45† | docs/client/re-open-items.md:81; docs/client/packet-opportunities.md:846,982 | Japan-only exit window, web inventory opener (region 4), anti-hack packet 0xA303, OTP login packet 0xA105: nothing in the US client reaches them | [skip] not answerable | none; 0x405460 for 0xA303 |
| F-46† | docs/client/re-open-items.md:83; docs/client/re-network.md:230,454 | The alternative stream cipher (mode != 0): in code, never selected by a known server; use on the wire unknown | [skip] not answerable | none |
| F-47† | docs/client/re-open-items.md:84; docs/client/re-engine.md:421,422 | Names guessed from a string alone (stat and item-type names in sysmsg tags 0x563e00, Korean particle markers 0x41b900 to 0x41ba90, ef_* effect names) | [skip] not answerable | none |
| F-48 | docs/client/re-open-items.md:86 | Emote rig notes (EMOTE_RIG_REFERENCE.md): inferred means an animator's reading of a pose, not a binary question | [skip] not answerable | none |
| F-49 | docs/client/CLIENT-FACTS.md:53 | 0xF701 GM set status: attribute 4 is skill points (inferred) | [server]* ps_game.exe | F701 attribute switch (gm-commands.md) |
| F-50 | docs/client/CLIENT-FACTS.md:150 | Field names after slot in the item answer are the server's (inferred from the same order) | [server]* ps_game.exe | the 0x0802 answer builder (CUser::ItemGemRemoveAll) |
| F-51 | docs/client/CLIENT-FACTS.md:179 | The not-yet-learnt (grey) skill entries' source was not traced; an Archer who has Trap Lv1 cannot raise it | [live]* live run | the skill list 0x021e47dc fill from 0x0108 answers (0x56c1d0) |
| F-52 | docs/client/CLIENT-FACTS.md:227 | A newline in a client string: nothing visible (inferred from the draw flags) | [live]* live run | the notice/chat draw flags |
| F-53 | docs/client/CLIENT-FACTS.md:354 | Live checks not done: 0x0802 with the 33-byte answer, the NPC-13 "chances left" counts, F501 failure codes, the word filter on region 6, Trap Lv1 for an Archer, Gruesome Attack with a sword | [live] live run | the CLIENT-FACTS open-questions list |
| F-54 | docs/client/client-packets.md:92 | Whether the text in a packet is counted to the first NUL or to the packet's length was not checked | [open]* game.exe | the string copy in the chat/guild handlers (count to the first NUL to be safe) |
| F-55 | docs/client/client-packets.md:136 | Handlers whose reads are hidden in callees (0xFA0E to 0x5537e0 "no reads"): follow the callee | [open] game.exe | 0x5537e0 |
| F-56 | docs/client/client-packets.md:137 | Name the handlers (what each does in the UI and world), starting with the 0x05xx, 0x02xx, 0x0Dxx and 0x0Bxx families | [open] game.exe | the client packet switch handlers by family |
| F-57 | docs/client/client-packets.md:139 | Login-connection packets (0xA1xx / 0xA2xx) also go through the dispatcher; compare with ps_login | [server] ps_login.exe | the ps_login dispatch vs the client dispatcher |
| F-58 | docs/client/client-packets.md:140 | Try the packet log live and document the exact line format of RECV>> (where in the receive path it is called) | [live] live run | the packet log call in the receive path |
| F-59 | docs/client/client-systems.md:43 | 12xx family is unknown: one client request with a variable length | [open]* game.exe | the 0x12xx case in the packet jump table |
| F-60 | docs/client/re-network.md:180 | The big-number routine (0x5718e0) is modular exponentiation (inferred from its shape) | [open]* game.exe | 0x5714e0, 0x571720, 0x5718e0; matches ps_login SRsa |
| F-61 | docs/client/re-network.md:300 | The accumulation buffer: it is the server's job to send well-formed lengths (a length under 4 should never be sent) (inferred) | [open]* game.exe | the receive loop's length check |
| F-62 | docs/client/re-network.md:340 | The 16 KiB send queue flush ignores how many bytes send() accepted (inferred; queue-ordering risk) | [open]* game.exe | the send queue flush on FD_WRITE |
| F-63 | docs/client/re-network.md:346 | Which of the two disconnect texts is used depends on a value returned by 0x429e20, not traced | [open] game.exe | 0x429e20 |
| F-64 | docs/client/re-network.md:366 | The ping loop at 0x4b1260 looks like a worker thread's (inferred) | [open]* game.exe | 0x4b1260 and its CreateThread caller |
| F-65 | docs/client/client-world-packets.md:42 | Special skill result 4: plays an extra caster animation; with skill ability 0x17 on a mob it sets the mob's HP to 0 (inferred) | [open]* game.exe | the skill result handler for result 4 |
| F-66 | docs/client/client-world-packets.md:131 | Player actions 2, 3, 10, 11 in FUN_0043c000 not identified (2 inferred jump) | [open]* game.exe | FUN_0043c000 action cases |
| F-67 | docs/client/client-world-packets.md:145 | 0x0522 stores a value on the player (+0x2EC); use not traced | [open] game.exe | readers of player +0x2EC |
| F-68 | docs/client/client-world-packets.md:200 | A quest's u16 value starts as NpcQuest.SData field +0x30 (inferred time limit) | [server]* ps_game.exe | NpcQuest field +0x30 readers (dwTimeMin) |
| F-69 | docs/client/client-world-packets.md:216 | Result value 4, 0x0522's value and 0x0520 / 0x0524 are not traced | [open] game.exe | the handlers for 0x0520, 0x0522, 0x0524 |
| F-70 | docs/client/client-social-packets.md:44 | Party mark byte (0x0520, player +0x2c0): values from CUser::GetPartyType unknown; what the client does with them | [server]* ps_game.exe | CUser::GetPartyType |
| F-71 | docs/client/client-social-packets.md:43 | The two party options are inferred to be "refuse party/trade" | [open]* game.exe | the options window flags at the 0x0A02 / 0x0B02 senders |
| F-72‡ | docs/client/client-social-packets.md:109,253,255,256 | Member/guild-unit job byte (u8 inferred job); the member unit's second name (szMasterName inferred) | [server]* ps_game.exe | GUILDUSER_UNIT / GUILD_UNIT builders in CGuild |
| F-73† | docs/client/client-social-packets.md:275,282; docs/client/packet-opportunities.md:562,563,569 | Guild member status 1/2/3 means online / offline / in the guild war zone (inferred) | [server]* ps_game.exe | CGuild::SendGuildUserList |
| F-74† | docs/client/client-social-packets.md:293; docs/client/packet-opportunities.md:582 | 0x0D27 fields unknown (the PDB's TP_GUILD_RANKING_GETPOINT is 8 bytes; CGuild::SendRankingZone sends 14) | [open] game.exe | 0x555b40; CGuild::SendRankingZone |
| F-75 | docs/client/client-social-packets.md:344 | The "no" vote of a guild disband is 0x0D03 3 (CGuildCreate::SendGuildFireFailed; inferred) | [server]* ps_game.exe | the 0x0D03 reply senders |
| F-76 | docs/client/client-social-packets.md:355 | Whether the original sends 0x0B16 before a new 0x0B0B list was not traced | [server] ps_game.exe | the 0x0B0B / 0x0B16 senders in CUser::PacketParty |
| F-77 | docs/client/union-parties.md:63 | Whether the original sends 0x0B16 before a new 0x0B0B list (same fact as client-social-packets.md:355) | [server] ps_game.exe | see above |
| F-78 | docs/client/packet-opportunities.md:59 | Item-mall product record: the u32 at entry +0xc is the price (inferred) | [open]* game.exe | the product-list updater 0x55a000 (entries at 0x84cea0) |
| F-79 | docs/client/packet-opportunities.md:93 | Weapon-effect render pass without the video option (inferred) | [live]* live run | 0x428860 at 0x429118 and 0x429212 |
| F-80 | docs/client/packet-opportunities.md:111 | 0x84d684 is the experience value (inferred); whether feature 43's default quick slots reach the server | [open]* game.exe | 0x84d684 writers; the quick-bar save/restore 0x4d4b30 |
| F-81 | docs/client/packet-opportunities.md:119 | Feature 51: a skill tree button for one job (window contents unknown; skilltree_.tga missing) | [live]* live run | 0x4f0ff0, 0x4f1630, 0x4f2700, 0x424100 |
| F-82 | docs/client/packet-opportunities.md:990 | Not settled: which of feature 12's buttons sits where (one sends sit, 0x0506 motion 1); the two cost fields of feature 58 | [live] live run | the feature 12 and 58 buttons |
| F-83 | docs/client/client-actions.md:163 | Item-to-collect (41) and gold-to-collect (53) record fields: contents not traced field by field | [open] game.exe | the auction row reader (client/auction-house.md) |
| F-84 | docs/client/client-actions.md:209 | 0x0556: chat commands /return and /town and two buttons are inferred to be the death/return window | [open]* game.exe | 0x4d52a7, 0x4db66c |
| F-85 | docs/client/client-actions.md:211 | 0x0F01/0x0F02 is a paid character "reading" by the Priest (inferred) | [live]* live run | the Priest window (research-priest-packets.md) |
| F-86 | docs/client/client-actions.md:218 | In Japan the exit window was an exit-time window (a survey or notice) (inferred) | [skip]* not answerable | none; region 4 only |
| F-87 | docs/client/client-actions.md:243 | How not-yet-learnt (grey, number 0xFF) entries get into the skill list was not traced (same fact as CLIENT-FACTS.md:179) | [live]* live run | 0x021e47dc, 0x56c1d0 |
| F-88 | docs/client/gm-and-commands.md:143 | F709 /akick finds the user by account name (inferred; CWorld::FindUserBill name-keyed map) | [server]* ps_game.exe | CWorld::FindUserBill string overload |
| F-89 | docs/client/gm-and-commands.md:155 | F801 /cure clears type-3 effects (inferred debuffs) | [server]* ps_game.exe | the F801 case in ps_game |
| F-90 | docs/client/gm-and-commands.md:189 | A client rule for staff (inferred) | [open]* game.exe | the privilege check near the chat command table |
| F-91 | docs/client/gm-and-commands.md:290 | /sign up for guild: the current target must be a player (0x84fa18 == 2, inferred) | [open]* game.exe | 0x84fa18 writers |
| F-92 | docs/client/gm-and-commands.md:293 | /raid only when 0x21e485c is 0 or 1 (inferred: union leader or sub-leader) | [open]* game.exe | 0x21e485c |
| F-93‡ | docs/client/gm-and-commands.md:320,409 | The chat box's own input limit was not traced (bounded: 0x475e00, 0x46d200 only) | [open] game.exe | 0x475e00, 0x46d200 |
| F-94 | docs/client/gm-and-commands.md:405 | Live: most of the page was not run against a server; deferred-move timing and F904/F905 height handling; the banned-word filter on region 6 (0x7019f4); CZone::UpdateAllCall minimum distance (0x53a45c, not read) | [live] live run | 0x7019f4; 0x53a45c |
| F-95 | docs/client/auction-house.md:98 | byResultType values (ENDITEM / ENDMONEY) are not read by any display code found; the "N min" format string (0x6a97e8) not read | [open] game.exe | 0x6a97e8 and the auction row display |
| F-96 | docs/client/personal-shop.md:25 | Personal-shop seat: CUser +0x1434 == 1 means the player sits in the shop motion (inferred) | [server]* ps_game.exe | writers of +0x1434 |
| F-97 | docs/client/personal-shop.md:64 | On map 42 the client adds a value of its own to the amount shown (inferred: the -5 %) | [open]* game.exe | the shop price display code |
| F-98 | docs/client/personal-shop.md:73 | 0x2309 result 2 means shop full of visitors (inferred; stMyShop::Use result 2) | [server]* ps_game.exe | stMyShop::Use 0x478450 |
| F-99 | docs/client/personal-shop.md:85 | Live: the sign 0x0523 text placement; what stMyShop::Use result 2 means; the shop motion the original's +0x1434 tests (client queue entry 13b, see B personal-shop.md:293) | [live] live run | 0x0523 handler; stMyShop::Use |
| F-100 | docs/client/duels.md:30 | The win/loss counters (0x21e4f70, 0x21e4f74) the client adds itself (inferred) | [live]* live run | 0x21e4f70, 0x21e4f74 |
| F-101 | docs/client/duels.md:38 | 0x2401 sender 0x589370 is the player menu (inferred); the +0x1460 flag is exchange (inferred) | [open]* game.exe | 0x589370 |
| F-102 | docs/client/duels.md:39 | The 0x2401 is ignored if either player has +0x250 set (inferred: dead or mounted) | [open]* game.exe | writers of +0x250 |
| F-103 | docs/client/duels.md:73 | At the duel end the client drops its type-3 effects on the players (inferred debuffs) | [live]* live run | the 0x2408 handler |
| F-104 | docs/client/duels.md:84 | The server checks the area at 35 units around the centre, inferred from the client's warning radius | [server]* ps_game.exe | the 0x2406 area check |
| F-105 | docs/client/duels.md:90 | The original's death / move / attack cancel paths (PvPChkLeave) and the stake payout; the counters live | [server] ps_game.exe | PvPChkLeave* (covered in docs/original/duel.md section 6) |
| F-106 | docs/client/guild-wars.md:48 | A dead member is out of the war (inferred) | [server]* ps_game.exe | the guild-war death handler |
| F-107 | docs/client/guild-wars.md:64 | Live: the guild-war result delay | [live] live run | the result packet handler |
| F-108 | docs/client/obelisks.md:23 | Obelisk states 0, 1, 2 are neutral / light / dark (drawn on the minimap; inferred) | [server]* ps_game.exe | the obelisk change sender (SendObeliskChange) |
| F-109‡ | docs/client/obelisks.md:42,48 | Obelisk list on maps 68 to 70: the status icon from states 101 to 103 (inferred icon); SendObeliskList content (inferred) | [open]* game.exe | 0x553040 |
| F-110 | docs/client/item-mall.md:21 | A fifth tab (ins_market_past.tga) shows the purchase history (inferred) | [live]* live run | the item-mall window tab 5 |
| F-111 | docs/client/item-mall.md:28 | 0x2605 is also sent when the game window becomes active again (0x40977b; inferred purpose) | [live]* live run | 0x40977b |
| F-112 | docs/client/item-mall.md:65 | buyTime in purchase-history rows is a packed date (inferred) | [open]* game.exe | the TP_POINTLOG_UNIT display |
| F-113 | docs/client/item-mall.md:79 | Live: what the fifth tab shows exactly; whether 0x2605 is sent on window activation; the gift flag 0x21e5b9d's icon | [live] live run | 0x21e5b9d readers |
| F-114 | docs/client/item-mall.md:80 | How the billing-storage and mall-storage windows are opened (NPC or button) was not traced | [open] game.exe | the window openers (store button, bank teller) |
| F-115 | docs/client/scripted-monsters.md:32 | An empty second line in a scripted-monster speech is invisible (inferred) | [live]* live run | CMob::PAI_Say2 |
| F-116 | docs/client/scripted-monsters.md:75 | Live only: how a kind-1 line looks in the centre list next to other notices | [live] live run | the kind-1 notice path |
| F-117 | docs/client/formats.md:37 | A map box pair (box2min/box2max) purpose unknown | [open] game.exe | the .wld loader |
| F-118 | docs/client/formats.md:94 | EP6's format number for the model file is not checked here | [open] game.exe | the model loader's format tag |
| F-119 | docs/client/formats.md:104 | A model variant without a bone block: its loader is not traced | [open] game.exe | the loader for str, nVert, vertices, faces |
| F-120 | docs/client/formats.md:201 | Animation sequence delay is the start offset in seconds (inferred) | [open]* game.exe | the sequence table reader |
| F-121 | docs/client/formats.md:219 | Emitter field i32 unk [+dc]: 0, or 3.0 as a float in 111 emitters (not traced) | [open] game.exe | the particle loader |
| F-122‡ | docs/client/formats.md:253,261 | Animation hit frames and unknown ints (not traced) | [open]* game.exe | the motion record reader |
| F-123 | docs/client/formats.md:300 | Map group records ({k; {box, f32 rotY, i32, i32}[k]}): Parsec spawns, not traced | [open] game.exe | the map group loader |
| F-124 | docs/client/formats.md:309 | A fog/light block with feature 0x26 on: exact roles not traced | [open] game.exe | the feature 0x26 readers |
| F-125 | docs/client/formats.md:314 | (inferred) a field of the map format | [open]* game.exe | near formats.md:314 |
| F-126 | docs/client/data-archive.md:26 | data.saf trailer 4 bytes 60 49 6b 0b (unknown) | [open] game.exe | the archive reader/writer in the updater |
| F-127 | docs/client/data-archive.md:31 | How the updater sizes free blocks in data.saf is open | [open] game.exe | the updater's pack code (updater.md) |
| F-128 | docs/client/item-sdata-vs-defs.md:65 | Item 32/1 ReqVg 0 / 1: "unknown use" | [open] game.exe | ReqVg readers |
| F-129 | docs/client/skill-monster-sdata-vs-defs.md:101 | 13 quest monsters' quest-item field is the server's own, the client does not need it (inferred) | [open]* game.exe | the monster record reader |
| F-130 | docs/client/sdata-generation.md:95 | Which values a shipped file should take, difference by difference (see the comparison pages) | [open] not answerable | none; a decision |
| F-131 | docs/client/fixes-dll.md:93 | Camera button double buffer: inferred from the symptom (shipping.md) | [live] live run | the camera button handler |
| F-132‡ | docs/client/operator-listen.md:34,71 | The F502 chat copy: the client shows the prefix verbatim (inferred); a watch session across ordinary, whisper, party and guild chat, literal prefix, name and line; placement in the GM window | [live] live run | the 0xF502 handler |
| F-133 | docs/client/research-movement-storages-auction.md:156 | 0x0513 buff and field hits: not examined; look only if a live test shows a wrong display | [live] live run | 0x0513 handler (same fact as queue entry 3) |
| F-134 | docs/client/research-gambling-kill-boost.md:143 | The login snapshot 0x010a carries the remaining buff duration (inferred from earlier protocol notes) | [open]* game.exe | the kill-boost buff icon duration code |
| F-135 | docs/client/research-gambling-kill-boost.md:179 | All live checks: gambling scene behaviour, chat in scene 9, the wide item-link option | [live] live run | gambling scene 9 |
| F-136 | docs/client/research-gambling-kill-boost.md:180 | Whether an existing item can be proven unobtainable (needs the server's data, not the client's) | [server] other server exe (data) | the server item and drop tables |
| F-137‡ | docs/client/research-extension-features.md:12,37,51 | Race above 3, sex above 1 and colour index are not checked and index tables out of bounds: wrong model or crash (inferred) | [live]* live run | 0x415ec0, 0x415310; tables 0x21f4cac, 0x21f4eac, 0x21f5aa8 |
| F-138 | docs/client/research-extension-features.md:58 | A format token is not parsed (plain draw, inferred) | [open]* game.exe | the chat markup parser |
| F-139 | docs/client/research-extension-features.md:82 | 0x0204 is the silent way to fill a slot (inferred) | [live]* live run | the 0x0204 handler |
| F-140 | docs/client/research-extension-features.md:84 | Every item of a type looks the same on the ground; a different model per type is a data change in its .ITM list (inferred) | [live]* live run | the ground item model lookup |
| F-141 | docs/client/research-extension-features.md:117 | Max HP of 0 divides by zero in the target bar (0x4f8e50, FIDIV at 0x4f93b5): a garbage value (inferred) | [live] live run | 0x4f8e50, 0x4f93b5 |
| F-142‡ | docs/client/research-extension-features.md:126,308 | Cheapest campfire look: a new Monster.SData row with model 422 (inferred) | [live] live run | the "Flame Relicpole" row |
| F-143‡ | docs/client/research-extension-features.md:178,187,191,193 | Button disabling (inferred); the condition at 0x4fffad was not traced; fields inferred unused on this path | [open] game.exe | 0x4fffad, 0x4ffc50 around 0x4fff93 (condition of message 231) |
| F-144 | docs/client/research-extension-features.md:201 | A per-monster item record or a tooltip line from the craft name is small (inferred) | [live]* live run | the item tooltip builder 0x49fa00 |
| F-145 | docs/client/research-extension-features.md:218 | Quick-bar count inferred from the window; map-id ceiling not traced further | [open] game.exe | the map tables indexed by 0x84e6e0; 0x4d4e10 |
| F-146 | docs/client/research-extension-features.md:226 | New icon files follow the existing naming (the index from the item record +0x1a at 0x51d370, inferred) | [open]* game.exe | 0x51d370 |
| F-147‡ | docs/client/research-extension-features.md:235,239,246 | Ride together (0x021D) and the party-member summon in the menu; remake as the cheapest "put items in, server answers" window (inferred) | [live]* live run | the player menu code |
| F-148 | docs/client/research-extension-features.md:260 | A motion set is for riding or transformed (inferred), except motion 2 | [live]* live run | the 0x0501 motion reader |
| F-149 | docs/client/research-extension-features.md:274 | An event position can be marked on the minimap (inferred; not on maps 68, 69, 105, 108, 0x4ba850) | [live]* live run | 0x4ba850 |
| F-150 | docs/client/research-extension-features.md:281 | The .wtr water file starts with f32 64.0 (inferred meaning) | [open]* game.exe | the .wtr reader after 0x59a7e0 |
| F-151‡ | docs/client/research-extension-features.md:292,295 | A DLL overlay must subclass the window procedure and also stop DirectInput (inferred); one new client-to-server opcode after 0x0201 is the cheapest way to announce a modified client (inferred) | [live]* live run | the window procedure and the input object 0x21ea0b8 |
| F-152‡ | docs/client/research-extension-features.md:212,301,312,313 | Open points at the end of the page: live look of a scaled monster (Day 6), monster with max HP 0, quit-dialog timer (366; kind 8 callers 0x516770), condition of 231, ground model per item, in-water test (Cwater.EFT), merchant kind 15/16 NPCs, map-id ceiling, quick-bar count, 0x1108 centre look | [live] live run | the "cheapest way" column on research-extension-features.md:301 to 313 |
| F-153 | docs/client/research-priest-packets.md:71 | The path variable 0x7013e0 also gets [USER] SHOW_INTERFACE (inferred) | [open]* game.exe | 0x7013e0 writers |
| F-154 | docs/client/research-priest-packets.md:125 | The next time the Priest window is open, 0x4d0770 runs the text lookup and crashes (inferred) | [live] live run | 0x4d0770 |
| F-155 | docs/client/research-balance.md:19 | Skill costs are not checked before sending; a cost of exactly 1 is printed as "spend all" | [live]* live run | the skill window cost print |
| F-156‡ | docs/client/research-balance.md:68,253 | Other writers of the add_ fields (mastery passives): the window leaves them out (confirmed by absence in 0x4f3f80); that nothing else writes them is inferred | [open] game.exe | references to 0x84d668 + 0x54..0x68 through other base registers |
| F-157 | docs/client/research-balance.md:83 | A formula rebalance needs the five attack floats patched (inferred) | [live]* live run | the character window attack formula |
| F-158 | docs/client/research-balance.md:141 | The vail caster rigs have no one-hand attack set, so a vail caster with a short sword uses another set for normal blows (inferred; live) | [live] live run | the creation screen mode draw |
| F-159 | docs/client/research-balance.md:174 | A skill level offered although ultimate-only; the server must refuse it (inferred; keep the server's Grow check); a server refusing level 2 of 180-185 in hard mode | [server] ps_game.exe | the server skill learn check (docs/original/skills.md) |
| F-160 | docs/client/research-balance.md:189 | A feature test passes; the condition was not traced | [open] game.exe | the feature test near the line |
| F-161 | docs/client/research-balance.md:228 | A box closes (0x4ef1a7 to 0x4ef218) without sending anything (inferred: no cancel packet exists) | [open]* game.exe | 0x4ef1a7 to 0x4ef218 |
| F-162 | docs/client/research-balance.md:257 | Who opens the 365/366 "sit down first" boxes; the +0xe4 flag | [open] game.exe | callers passing kind 6 or 8 to 0x516770 through a register |
| F-163 | docs/client/qol-feasibility.md:105 | Separate cosmetic-system existence, safe detour ABIs, complete new-widget registration and all-scene UI scaling are only partly answered; no live performance, visual or multiplayer claim is made | [live] live run | the focused trace named for each feature |
| F-164 | docs/client/tools-and-method.md:33 | Stop rule: a question taking about 30 queries without a confirmed function is written up as open (method statement, not a claim) | [skip] not answerable | none |
| F-165 | site/content/client.md:57 | site/content/client.md: what "inferred" and "live" mean (legend only; the page's open pointer points to re-open-items.md) | [skip] not answerable | none |
| F-166† | docs/testing/client-research-queue.md:12; docs/original/thorns-fields-pvp.md:219 | Queue 3: what the client shows for 0x0513 when the skill is Fire Thorns (buff, kind 4) or a field (Trap, Fire Wall: kind 5) and the target is a player | [open] live run | 0x0513 handler; live test section 46 (see C thorns-fields-pvp.md:219) |
| F-167 | docs/testing/client-research-queue.md:16 | Queue 22: how server texts from scripts read in the client (F106, the to-one notice, country and zone notices, 0x1108): longest line, wrapping, sound, duration, empty sender name, blocked sender, burst after entering the world | [open] live run | the F106 and 0x1108 handlers; live test section 60 |
| F-168 | docs/testing/client-research-queue.md:23 | Queue 23: the opcode and format a fixes.dll would send after 0x0201 naming its features (a design to agree with the DLL work); what this server does with an unknown opcode | [open] not answerable | a design decision; the DLL and the server's unknown-opcode path |
| F-169 | docs/testing/client-research-queue.md:29 | Queue 24: the enchant lines and item blocks read for the player guide (0x4c4920, 0x4a0cb0, 0x4a2830, 0x5559b0): offered for folding in; the client uses the file's values | [open] game.exe | 0x4c4920 (factor 0x6ac568), 0x4a2830; research-balance.md "The character window" |
| F-170 | docs/testing/client-research-queue.md:39 | Queue 25: a usable item with special 0, no skill link and no HP/SP/MP (100/215 to 217): does the client send 0x050A or refuse with message 227; does a level jump of more than one show right | [open] live run | the item-use sender; live test section 61 item 2 |
| F-171 | docs/testing/client-research-queue.md:45 | Queue 26: a Lapisia in an empty record of group 95 (95/3, 95/8): does the enchant window take an item by its type alone | [open] game.exe | the enchant window item check; live test section 61 item 3 |
| F-172 | docs/testing/client-research-queue.md:48 | Queue 27: a Recreation Rune in an empty record of group 100 (100/221 to 226, special 62): does the client open the recreation window from the special, and show the new craft name | [open] game.exe | the special-62 branch of the item-use sender; live test section 61 item 4 |
| F-173 | docs/testing/client-research-queue.md:51 | Queue 28: the Pet Egg's tooltip line (fixes.dll pet-egg-tooltip): where the line is added and what the craft-name pointer holds at that point for type 100 | [open] game.exe | the tooltip 0x49fa00 and Monster.SData lookup 0x461210 |
| F-174 | docs/testing/client-research-queue.md:58 | Queue 29: appended Monster.SData records with patch 111 (a filler record behaves as missing; records 3001 to 5473 load, scale, are unclickable, walk 3.7, run 9.7) | [open] live run | 0x461210, count read at 0x460e20; live test section 63 items 1, 4, 6 |
| F-175 | docs/testing/client-research-queue.md:63 | Queue 30: the 0x050A success answer for an item that stays (the Pet Egg): does the client keep the item, its count and reuse time right | [open] live run | the 0x050A handler; live test section 63 item 4 |
| F-176 | docs/testing/client-research-queue.md:66 | Queue 31: does the client send 0x0304 or 0x0305 when the player selects a monster, or only when it enters view | [open] game.exe | the target select code in 0x43c000 and the 0x0304 / 0x0305 senders |
| F-177 | docs/testing/client-research-queue.md:70 | Queue 32: patch 111: the archive member of Monster.SData (data/monster/monster.sdata vs Monster/Monster.SData in the generator manifest) | [open] game.exe | the archive's member names (data.sah) for Monster.SData |
| F-178 | docs/client/re-engine.md:90 | The single-instance mutex name switch (Global\Shaiya vs Shaiya) is the C runtime's Windows major version at 0x22330ec (inferred) | [open]* game.exe | 0x22330ec and the CreateMutexA call |
| F-179 | docs/client/re-engine.md:101 | The random seed call 0x5b73f4(timeGetTime()) is srand (inferred) | [open]* game.exe | 0x5b73f4 |
| F-180 | docs/client/re-engine.md:104 | 0x5a9350 is the logger shut-down (inferred) | [open]* game.exe | 0x5a9350 |
| F-181 | docs/client/re-engine.md:161 | CONFIG.INI [INTERFACE] section and [LOGIN] ID: their readers were not traced | [open] game.exe | the CONFIG.INI parser callers |
| F-182 | docs/client/re-engine.md:233 | Font slot +0x40 is OnLostDevice (inferred) | [open]* game.exe | the device-lost path after Present (0x52fe20), 0x701b0c |
| F-183 | docs/client/re-engine.md:239 | What sets a device state after the lost-device path was not traced | [open] game.exe | near re-engine.md:239 |
| F-184 | docs/client/re-engine.md:295 | The loading screen 0x4b13f0 at state 5 (inferred) | [open]* game.exe | 0x4b13f0, 0x42e632 |
| F-185 | docs/client/re-engine.md:307 | The scene-change "fade" object (vtable 0x6a80fc) is a fade (inferred) | [open]* game.exe | 0x41f980, 0x41fa00 |
| F-186 | docs/client/re-engine.md:363 | The "selected entry" global makes the archive helpers unsafe from two threads (inferred; the client never does) | [open]* game.exe | the safopen helpers around 0x5964e0 |
| F-187 | docs/client/re-engine.md:366 | 0x5cd4ab is D3DXCreateTextureFromFileInMemoryEx (inferred from its 14 arguments) | [open]* game.exe | 0x5cd4ab |
| F-188 | docs/client/re-engine.md:425 | 0x41e8f0(type, id) is a chat or notice print function (inferred from its arguments) | [open]* game.exe | 0x41e8f0 |
| F-189 | docs/client/re-engine.md:450 | UI window vtable stub 0x5b804d is _purecall (inferred) | [open]* game.exe | 0x5b804d |
| F-190 | docs/client/re-engine.md:484 | The UI windows' other fields are filled when each window is first needed (inferred) | [open]* game.exe | 0x424100 constructors |
| F-191 | docs/client/re-engine.md:487 | The input result goes to the world update 0x43c000 so a window-taken click does not also move the character (inferred) | [open]* game.exe | 0x48c120 and 0x43c000 |
| F-192 | docs/client/re-engine.md:501 | Saved window positions live in CONFIG.INI (confirmed for the status window; inferred for the rest) | [open]* game.exe | the window position writers |
| F-193 | docs/client/re-engine.md:510 | Per-frame preparation 0x53cec0, 0x596fd0, 0x596ed0 not traced | [open] game.exe | those three functions |
| F-194 | docs/client/re-engine.md:514 | 0x539c50 with fog off, then on again, draws the sky (inferred) | [open]* game.exe | 0x539c50 |
| F-195 | docs/client/re-engine.md:515 | 0x52e7c0(aspect, near, far) is the projection; far is the RANGE distance (inferred) | [open]* game.exe | 0x52e7c0 |
| F-196 | docs/client/re-engine.md:532 | The mouse's DirectInput cooperative level, built from (0, 1), is the same mode as the keyboard's (inferred) | [open]* game.exe | 0x594680, 0x594647 |
| F-197 | docs/client/re-engine.md:538 | The input object at 0x21ea0b8 is inferred from the +0x18 button field at 0x21ea0d0 | [open]* game.exe | 0x51e720 |
| F-198 | docs/client/re-engine.md:549 | The symptom of other programs reaching the game (a mechanism confirmed; the symptom inferred) | [live]* live run | the input/device handling around re-engine.md:549 |
| F-199 | docs/client/re-engine.md:557 | 0x595120 and 0x595160 set up the primary buffer; which listener constants go to which slot (inferred) | [open]* game.exe | 0x595120, 0x595160, 0x5951f0 |
| F-200 | docs/client/re-engine.md:561 | The sound effect distances 5 and 30 are minimum and maximum (inferred) | [open]* game.exe | 0x527830 |
| F-201 | docs/client/re-engine.md:566 | The music value 3000 is a 3-second fade-out and the second argument is looping (inferred) | [open]* game.exe | the music play function using SMusic::CreateStream |
| F-202 | docs/client/re-gameplay-state.md:76 | Reset struct: opcode 0x0214, reader 0x580680, same shape as the server's (inferred) | [open]* game.exe | 0x580680 |
| F-203 | docs/client/re-gameplay-state.md:102 | Level and stats written by 0x46a4e0 are inferred from the chosen (character) | [open]* game.exe | 0x46a4e0, 0x4c8bf0 |
| F-204 | docs/client/re-gameplay-state.md:117 | Item byte +0x00 (free lapis slots, computed at 0x54cf90) is inferred | [open]* game.exe | 0x54cf90 |
| F-205 | docs/client/re-gameplay-state.md:136 | Item +0x19a enchant level per slot (inferred from the server's ITEM_TYPE) | [open]* game.exe | 0x54d370 |
| F-206 | docs/client/re-gameplay-state.md:137 | Item +0x384/+0x388 are the cape's emblem (inferred) | [open]* game.exe | 0x54d370 |
| F-207 | docs/client/re-gameplay-state.md:142 | Equipment slot names beyond 5 (weapon) and 7: server order, no client-side proof (inferred) | [open] game.exe | the equipment slot readers in the character window |
| F-208 | docs/client/re-gameplay-state.md:216 | Entity +0xdc action state (2 attacking; 8 reset by a move) is inferred as action state | [open]* game.exe | 0x54de20, 0x551650 |
| F-209 | docs/client/re-gameplay-state.md:223 | Entity +0x168 is the move motion, walk/run (inferred) | [open]* game.exe | 0x551650 |
| F-210 | docs/client/re-gameplay-state.md:233 | Entity +0x298 means moving (inferred) | [open]* game.exe | 0x551650 |
| F-211 | docs/client/re-gameplay-state.md:257 | Entity +0x88 is the size used in range checks (inferred) | [open]* game.exe | 0x551650 |
| F-212 | docs/client/re-gameplay-state.md:339 | Skill record +0x10 (the per-skill value x 1000) is the remaining recharge (inferred) | [open]* game.exe | the 0x0108 / learn handlers (0x56c1d0) |
| F-213 | docs/client/re-gameplay-state.md:341 | Skills 3, 4 and 5 set the bytes 0x21e4e1d to 0x21e4e1f (inferred passive flags) | [open]* game.exe | 0x21e4e1d to 0x21e4e1f writers |
| F-214 | docs/client/re-gameplay-state.md:354 | Buff record +0x10 is the duration again, for the icon's countdown (inferred) | [open]* game.exe | the 0x050E / buff-add handler |
| F-215‡ | docs/client/re-gameplay-state.md:371,377 | Party member +0x1a level, +0x1c class, +0x2c/+0x30/+0x34 position (inferred); their meaning on screen | [open]* game.exe | the party member copy from the unit (+0x15, +0x17, +0x26..+0x2e) |
| F-216 | docs/client/re-gameplay-state.md:398 | Equipment slot names beyond 5 (weapon) and 7 need a client-side proof for each slot (same fact as re-gameplay-state.md:142) | [open] game.exe | see above |
| F-217 | docs/client/formats.md:314 | The height scale (i16)h x 0.2 - 200.0 reported by another member for the map height format is inferred | [open]* game.exe | the .wld height reader (FUN_00430280 neighbourhood) |
| F-218 | docs/client/research-balance.md:215 | Rate arithmetic from the static values is far below the patch file's (inferred side note) | [live]* live run | the character window rate calc and a measured run |

## Ids that span clusters

| id | clusters | pages |
|---|---|---|
| A1-04 | A1, A2 | docs/original/ep5.4.md; docs/original/protocol-notes.md |
| A2-28 | A2, B | docs/original/community-fixes.md; docs/original/shop.md |
| A2-31 | A2, C | docs/original/community-fixes.md; docs/original/duel.md |
| A2-33 | A2, D | docs/original/community-fixes.md; docs/original/guild-bank.md |
| A2-36 | A2, C | docs/original/community-fixes.md; docs/original/skills-buffs.md |
| A2-42 | A2, B | docs/original/community-fixes.md; docs/original/trade.md |
| A2-55 | A2, E | docs/original/ep3.md; docs/original/guards.md |
| A2-60 | A2, B | docs/original/ep5.4.md; docs/original/item-mall.md |
| A2-62 | A2, C | docs/original/ep5.4.md; docs/original/skills-buffs.md |
| B-01 | B, C | docs/original/combat.md; docs/original/stats.md |
| B-03 | B, C | docs/original/combat.md; docs/original/item-drop.md |
| B-04 | B, C | docs/original/item-flags.md; docs/original/pvp.md |
| F-166 | C, F | docs/original/thorns-fields-pvp.md; docs/testing/client-research-queue.md |

## Counts

Per cluster (one count per id):

| cluster | items |
|---|---|
| A1 | 50 |
| A2 | 62 |
| B | 33 |
| C | 25 |
| D | 15 |
| E | 20 |
| F | 218 |
| **all** | **423** |

Per settle route (primary route only):

| route | A1 | A2 | B | C | D | E | F | all |
|---|---|---|---|---|---|---|---|---|
| ps_game.exe | 14 | 12 | 4 | 8 | 2 | 9 | 27 | 76 |
| ps_login.exe | 7 | 1 | 0 | 0 | 0 | 0 | 2 | 10 |
| other server exe | 10 | 26 | 15 | 11 | 6 | 11 | 2 | 81 |
| PDB | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 1 |
| DB procs | 5 | 8 | 1 | 0 | 1 | 0 | 0 | 15 |
| game.exe | 6 | 6 | 8 | 2 | 2 | 0 | 119 | 143 |
| live run | 7 | 7 | 5 | 2 | 3 | 0 | 58 | 82 |
| not answerable | 0 | 2 | 0 | 2 | 1 | 0 | 10 | 15 |
| **all** | 50 | 62 | 33 | 25 | 15 | 20 | 218 | **423** |

Cluster F by the status carried from `re-open-items.md` (`*` = assigned here by its rule):

| status | items | of which assigned here (`*`) |
|---|---|---|
| settled | 6 | 0 |
| partly | 4 | 0 |
| live | 52 | 23 |
| server | 30 | 14 |
| open | 106 | 64 |
| skip | 8 | 1 |
| queue | 12 | 0 |
| **all** | **218** | **102** |

