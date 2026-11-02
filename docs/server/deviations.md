# Deviations from the original EP4.5 server

Every place where this server knowingly behaves differently from the original EP4.5 `ps_game.exe` / `ps_login.exe`, collected from
the feature notes (`docs/original/*.md`, whose numbering the `#` column keeps) and the settings. Each one is classified, and
the project owner's decisions on them are recorded.

**The rule for the release:** the original's behaviour by default. Fixes of the original's bugs stay on.
Gameplay changes approved during the build stay as they are, listed here. Defaults that turn on something the original does not
have (cross-faction trade, the market-map limit, the Kill Boost Stones) go back to the original; the setting stays for operators who
want them. The one exception is the gambling house: shown by default, with the original's odds.

**Kinds**

| kind | meaning |
|---|---|
| fix | a bug of the original: a crash, a duplication or exploit, a hole a modified client can use, a lost item or lost money, a stuck state, a rule its own data contradicts |
| client | a packet changed so that this client (US, 2010-04-26, patch 108) reads or shows it correctly |
| robust | limits and safety the original lacks: rate limits, timeouts, connection caps, whole saves, retries |
| tooling | the operator's side: GM commands, the console, the logs, the tools that replace the original's separate programs |
| gameplay | a rule changed on purpose and kept; the setting is named when there is one |
| addition | not in the original at all; **off by default** from the release (one exception: the gambling house, section 1) |
| internal | different underneath, the same in play (timing to the second, the order of packets in a burst) |

A setting written as `NAME` (`value`) gives its release default. `docs/server/settings.md` lists them all.

## 1. Defaults changed for the release

| setting | before the release | release default | the original | kind |
|---|---|---|---|---|
| `GAME_MINI_GAME` | `true` | `true` | never creates the type-5 NPCs, so the gambling house cannot be reached (`docs/original/mini-game.md`) | addition, on by default |
| `GAME_SLOT_PAYOUT_PERCENT` | `220` | `100` | the table's payout: about 41 % of the bets come back | addition (used only with the gambling house on) |
| `GAME_CARD_WIN_CHANCES` | `420,200,130,100,80` | `400,200,100,50,10` | the double-up's chances | addition (same) |
| `GAME_CROSS_FACTION_TRADE_MAPS` | `42` | empty | no trade between factions anywhere (`docs/original/trade.md` deviation 1) | addition |
| `GAME_MARKET_MAPS` | `35,36,42` | empty | the server allows a personal shop on any map; the unpatched client offers it only where its own map table allows (`docs/original/personal-shop.md` deviation 1) | addition |
| `GAME_KILL_BOOST_SKILLS` | `200/1:2,200/2:5,200/3:10` | empty | no kill-boost item (`docs/original/operator-console.md`) | addition |
| `tools/defs/additions.sql` (the Kill Boost Stone rows, items 100/212-214 and skill 200/1-3) | loaded by `additions.sh` | loaded only with `additions.sh --with-additions` | items 100/212-214 and skill 200/1-3 are empty `???` rows | addition |

Unchanged on purpose, though they differ from the original: `GAME_SMOOTH_CHASE` (`true`, kept as a display fix, section 9),
`GAME_CHECK_IP` (`true`, a security fix), `GAME_KICK_NOTICE_SECS` (`3`, so the client shows the notice), the trade chat limits (`10`,
`5`: what the US client enforces itself), the robustness limits (section 2).

## 2. Connections, login and robustness

From the robustness audit (`docs/server/audit.md`, findings R1 to R30, every one built). The original has none of these limits.

| # | the original → this server | kind | setting |
|---|---|---|---|
| R1, R19 | a client that stops reading kept its connection and an unbounded queue → a write deadline, a bounded queue, the idle clock from the client's last bytes | robust | `GAME_WRITE_TIMEOUT_SECS` (`20`), `GAME_OUTBOX_LIMIT`, `GAME_IDLE_SECS` (`600`) |
| R2, R10 | any number of connections per address; pings keep a socket for ever → per-address caps and connects a minute, handshake and key-exchange deadlines | robust | `GAME_MAX_PER_IP`, `GAME_CONNECTS_PER_MINUTE`, `GAME_HANDSHAKE_SECS`; `LOGIN_MAX_PER_IP`, `LOGIN_CONNECTS_PER_MINUTE`, `LOGIN_KEY_TIMEOUT_SECS`, `LOGIN_IDLE_SECS` |
| R4 | no chat or packet limit → per-connection chat and packet rates | robust | `GAME_CHAT_PER_SEC`, `GAME_CHAT_BURST`, `GAME_PACKETS_PER_SEC` |
| R5, R6 | a crash in one connection or an accept error ends the server → survived, with the shutdown save | robust | |
| R7 | wrong passwords never locked anyone out → lockouts per address, per account and address, per account; passwords 6 to 16 characters | robust | `LOGIN_FAILURE_*`, `LOGIN_PAIR_*`, `LOGIN_ACCOUNT_*` |
| ps_login | passwords stored as the original's database holds them → Argon2 hashes in `accounts` | robust | |
| ps_game 3 | the game connection's address is never compared with the login's → it must match | fix | `GAME_CHECK_IP` (`true`) |
| R8, R9, R11, R18, R22, R26, R28, R29 | the auction and guild-bank turns held across writes; the handshake order; eight database connections; player-triggered log lines; oversize packets; the console's staff level; the Lua `os` library; the bank count fix-ups → each made safe | robust | `GAME_DB_POOL`, `GAME_DB_ACQUIRE_SECS` |
| R21 | a NaN or huge height stored and relayed; shop titles unchecked; hair, face and size unchecked (the last still stored as sent: entry 13 of `docs/testing/client-research-queue.md` gave the ranges, the clamp is not built) | fix | |
| R3, R12-R16, R20, R23-R25, R27, N6 | exchanges, rewards and saves in several writes → each one transaction, or retried; listed under their systems below | fix | |
| client-extras 2 | a kicked connection closes silently → `0x0004 00` first, so the client says why and quits | client | |
| client-extras 3 | a new login while the old connection is still leaving is refused → it waits for it | robust | `GAME_HANDSHAKE_WAIT_SECS` (`15`) |
| world | a player dead when the connection ends stays 10 s → leaves at once (nothing could happen to it) | internal | |

## 3. Characters, stats and items

| note | # | the original → this server | kind | setting |
|---|---|---|---|---|
| small-items | 3 | a new name needs only two non-zero bytes → the creation rule: 2 to 18 ASCII letters or digits, unique ignoring case | fix | |
| small-items | 4, 5, 6 | renames refused for a character not this account's, deleted or still lingering; the guild list and auction names follow the rename; a stone with 255 renames left is not used up | fix | |
| stats | 1 | a base stat could wrap at 65536 → refused above 32767 | fix | |
| item-use | 1, 2 | the movement rune with a logout pending shows a cast that never moves → the error answer; after a reset only the changed maxima are sent | fix, internal | |
| item-windows | 1, 2 | the body change takes any sex byte → 0 or 1 only; a saved position is the client's (up to 50 away) → the server's | fix | |
| item-flags | 1 | an EXP stone alone gives ×1.5 (×2 needs premium) → ×2 alone | gameplay | |
| item-flags | 2 | an ultimate-mode death without a rune deletes the character → it **falls**: gone from the list as a deletion, kept so staff can restore it | fix (data kept; the player sees the original) | |
| item-flags | 3, 4, 5 | the original kept (safe deaths, Prevent Drop Equipped Item, the Red Phoenix Charm) | none | |
| repair | 1 | a repair anywhere → a blacksmith in view (the Repair Hammer needs none) | fix | |
| warehouse | 1 | the warehouse anywhere → a keeper in view or the Warehouse Recall buff | fix | |
| lapis | 1 | anything links or enchants into a cloak for free → refused | fix | |
| lapis | 2 | remake accepts one place three times (one item freed three times) → three different places | fix | |
| lapis | 3 | linking, extraction and enchanting anywhere → a blacksmith in view | fix | |
| lapis | 4 | the Operator's Exclusive (2000 %) capped at 50 % → always links | gameplay | |
| lapis | 5 | extraction always answered with 34 bytes (read one byte off by this client) → 33 to a 4-byte request | client | |
| item-options | 1 | an empty shield or helmet slot ends the lapis scan, so body-armour lapis rarely strike back at players → empty slots skipped | fix | |
| item-options | 2 | a gem skill still rolls on a target an earlier gem killed → nothing | fix | |
| item-mall | 1-3 | points checked against memory and taken after the items; the gift lock left set; gifts through a billing server → one transaction each, points from the account | fix, tooling | |
| item-mall | 4, 5, 8 | premium, points and the billing storage from outside services → the `mall` tool | tooling | |
| item-mall | 6, 7 | gift notices a recipient cannot use; counts above a stack size → a notice after entering; split rows | fix | |
| client-extras | 1 | monster health bars guessed by the client → `0x0608` with the real max HP after every `0x0601` | client | |

## 4. Combat, skills and monsters

| note | # | the original → this server | kind | setting |
|---|---|---|---|---|
| combat | | attacks on a 100 ms grid, positions interpolated, experience before drops, a partly stacked pickup | internal | |
| skills | | a level can be bought without the one below (totals go wrong); some requests never answered → in order only; result 3 | fix | |
| skills-special | | Resurrection leaves the server position at the body → the caster's (where the client puts it); Illusion shown with `0x0524` and not saved | fix, client | |
| skills-buffs | | the map-18 rule as code; the low-HP effect written and never deleted → not written; the aggro of Fire Thorns / Mass Taunt on the caster's target (blocked by a status check in the original) → works as the data gives | fix | |
| skills-buffs 16 | | duel and guild-war deaths keep the skills above id 200 | none (as the original) | |
| thorns-fields-pvp | 1, 2 | `0x0513` reach and the order of victims inside one pass | internal | |
| thorns-fields-pvp | 3 | a Fire Thorns kill stops whatever the caster was doing → it does not | fix | |
| mob-fight | | aggressive monsters target players of their own country first and drop them → passed over; `0x0609` names the monster; `0x0521` after entering; a lower maximum lowers the current value | fix, client | |
| mob-skills | | damage over time on players added → multiplied, as on monsters; a healed monster's HP also as `0x0305`; drain hits as a blow; area hits stop when a reflection kills; DoT end within 60 | fix, client | |
| pathing | 1 | a monster at a turn of its way set to height 0 → keeps its height | fix | |
| chase-smoothing | 1-5 | monsters, guards and walking NPCs step to the server's clock, so the client shows them stopping and starting → paced to the client's walking, with a 100 ms move tick | fix (display, kept on) | `GAME_SMOOTH_CHASE` (`true`) |
| guards | 1, 2 | an NPC's cell lags a move behind; a zero-length move sent every turn → fixed | fix | |
| guards | 4, 6 | a death by a guard leaves trades and shops open; a guard keeps hitting a dead target → fixed | fix | |
| guards | 7 | a guard walking home ignores everyone → it looks for targets on the way | gameplay | |
| scripted-mobs | 1 | Korean `kr_string.DB` lines, unreadable in this client → English | client | |
| scripted-mobs | 2 | `0x1109` with the line end and no NUL → as the client reads it | client | |
| scripted-mobs | 3-6 | `SetDrMove` absolute; a changed target drops the boss out of its script; the attack-kind switches written and never read (shields, immunities); a lost target ends the script → as the scripts were written | fix | |
| instances | 1 | an instance's expiry set once → restarts each time it empties | gameplay | |
| instances | 2-4 | the zone booked at the portal, `LuaGetNumberAggro(n)` the n-th, Lua events at the next pass | fix, internal | |
| move-check | 1-3 | a refused step leaves client and server apart → the player placed back; a setting; logging | fix, tooling | `GAME_MOVE_CHECK` (`true`, the original's check) |
| quest-drops | 1 | every party member wherever it is counts for a quest drop → only members in the killer's zone | gameplay | |
| quest-drops | 2 | the monster id counter saved hourly → every minute | robust | |

## 5. PvP, duels, guild wars, obelisks

| note | # | the original → this server | kind | setting |
|---|---|---|---|---|
| pvp-2 | 1 | the kill-count grade bonuses worked out inconsistently (type 8 to the wrong pool, type 20 not magic defence, 18/19 at the wrong time) → the same way every time | fix | |
| duel | 1-8 | self duels; the stakes window's area; a leaver's stake on an unreachable ground; bound items as stakes; vanished stakes paid anyway; trades, shops and duels mixing; stakes back piece by piece → each fixed | fix | |
| guild-war | 1-7 | wars that never end (officers apart, a defeated officer gone); any officer answering; deaths before the countdown; everyone cleared at the end; leaving events → each fixed | fix | |
| obelisks | 1, 2, 7 | guard and boss-follower counts that ignore blocked spots; the grade lowered one step late → fixed | fix | |
| obelisks | 3-6 | road portal links (11.6): **not built**, left as shipped by decision | | |
| guild-ranking | 1, 3, 4 | notices skipped; a second entry by changing guilds; the update lost to a restart → fixed | fix, robust | |
| guild-ranking | 2 | the ranking portal goes straight in → asks first (the client's question 8043) | gameplay | |

## 6. Trade, shops, auction, warehouse

| note | # | the original → this server | kind | setting |
|---|---|---|---|---|
| trade | 1 | no trade between factions → on the chosen maps | addition | `GAME_CROSS_FACTION_TRADE_MAPS` (empty) |
| trade | 2-11 | the exchange in pieces; split items lose their stats; a requester left busy; self trades; bound items; vanished offers paid anyway; trades with duels; trading while dead; item actions during a trade → each fixed | fix | |
| personal-shop | 1 | a shop on any map → only on the listed maps | addition | `GAME_MARKET_MAPS` (empty = any map) |
| personal-shop | 2-9 | empty slots listed; the price wraps (two for 0); visitors not cleared; self visits; the sign left up; the purchase in pieces → each fixed | fix | |
| auction | 1, 4, 5 | gold rows paid past the deadline; bids up to a minute after the end; refreshes larger than a packet → fixed | fix | |
| auction | 2 | the name search is case-sensitive → ignores case | gameplay | |
| auction | 3 | `/auctionrecall` keeps the seller's deposit → returns it | tooling | |
| guild-bank | 1-4 | the bank anywhere; bound items in by a swap; the wrong refusal text; a guild with items in its bank disbanded → each fixed | fix, client | |
| mini-game | | the gambling house unreachable (type-5 NPCs never made) → shown, with the original's odds (fairer odds are settings) | addition | `GAME_MINI_GAME` (`true`), `GAME_SLOT_PAYOUT_PERCENT` (`100`), `GAME_CARD_WIN_CHANCES` (`400,200,100,50,10`) |

## 7. Parties, unions, guilds, friends, quests

| note | # | the original → this server | kind | setting |
|---|---|---|---|---|
| party | | unknown `0x0Bxx` closes the connection; failure codes without an id; kicking breaks the leader slot; the full-party bonus skipped on a level-up gain; member status every 500 ms; party drops lost on a failed save → each fixed | fix, client | |
| union | 1-3, 5, 8, 9 | odd `bRaidType` and loot values; group moves out of range; the party's kick and leader change in a union; the config reset; a joiner not counted by this client → fixed | fix, client | |
| union | 6 | party buffs reach the caster's group of 6 → the whole union | gameplay | |
| union | 7 | the leader's union line as `0x1112`, which this client ignores → `0xF10A` | client | |
| guild | 1 | guild creation needs a party of exactly 7 | none (default `7`) | `GAME_GUILD_PARTY_SIZE` (`7`) |
| guild | 2, 4-13 | agreements that never lapse; the join penalty; a deleted master; refused remarks; unchecked names; looks not resent; double joins; lost writes → each fixed | fix, robust | |
| guild | 3 | a disband goes through at the third yes → needs every officer asked | gameplay | |
| guild-house | 2-7 | `0x0D34`/`0x0D35` without the NPC; the Etin to the requester only; a full slot table silent; a disconnect for a house that stopped qualifying; the purchase in pieces → each fixed | fix | |
| friends | 1-7 | full lists accept; deleted friends listed; memos unchecked and one byte too long; duplicate rows; blocking does nothing to whispers and requests → each fixed | fix | |
| quest | 1-5, 7 | any NPC in the zone; NPC id 0 skips the end NPC; rewards lost for want of room; end items of a reward choice; "bring more for more" counts; the money shown → each fixed | fix | |
| quest | 6 | time limits checked every 60 s → to the second | internal | |

## 8. Operator side

| note | # | the original → this server | kind | setting |
|---|---|---|---|---|
| game-log | 1 | a separate log server (`ps_gameLog.exe`) → the game server writes `game_log` and `chat_log` itself, same action types and columns | tooling | `GAME_LOG`, `GAME_CHAT_LOG` (`true`), `GAME_LOG_DAYS` (`0`) |
| game-log | 3, 4 | two guild records the log server drops by a typo → stored; item ids from the database writes | fix | |
| operator-console | | the console window of `ps_game.exe` → the `console` tool through PostgreSQL `NOTIFY`, and `.commands` in the game for staff | tooling | `GAME_CONSOLE_LEVEL` (`1`) |
| operator-console | | `reload` (monsters and drop tables) and `restart` with a countdown | tooling | |
| operator-console | | `[NEW_SERVER]` (double kills, half-price Lapisia): **rejected**; the kill rate instead | tooling | `GAME_KILL_RATE` (`1`), `GAME_EXP_RATE` (`1`) |
| operator-console | | the Kill Boost Stones (2×, 5×, 10× kills for an hour) | addition | `GAME_KILL_BOOST_SKILLS` (empty), `additions.sh --with-additions` |
| gm-commands | 1-13, 18-24 | unknown commands close the staff connection; `/apower` unusable with this client; `/set` on the wrong character; moves through a deferred pass; bans lost at relog; summons, guild and NPC commands; `/server off` still listed; `/cwatch` links → each fixed for this client | tooling, fix, client | |
| gm-commands | 14-17 | `/aban`, `/ban` and their cancels; `/pvpon`/`/pvpoff`; `/queston`/`/questoff`; `/eventadd`/`/eventrem`, in the client but unhandled → built | tooling | |
| gm-commands-2 | 1-15 | user questions saved (migration 0023), answers delivered, at most 3 open each; names not found refused; kicks announced; staff protected from lower staff; safe quest and strip commands; summons → each fixed or built | tooling, fix | |
| small-items | 1 | kick and ban notices cut off by the close → held a moment | client | `GAME_KICK_NOTICE_SECS` (`3`) |
| small-items | 7 | `0x0559` kinds 1 and 2 → built with the attack checks | fix | |
| small-items | 8 | `F706` / `F707` as the original (deviation 19 of gm-commands withdrawn) | none | |

Not built, by decision: the second game server for map 200 (`ps_gamewar.exe`), the original console's other live reloads, the global
chat without its item (small-items 2), the obelisk road links (obelisks 3-6).

## 9. Notes on the classification

- **Smooth chasing** stays on: the original's stop-start movement is treated as a display bug.
- **The gameplay rows** (the EXP stone, the Operator's Exclusive, union-wide buffs, guards hunting on the way home, the ranking portal's
  question, same-zone quest drops, the instance expiry, the guild disband, the auction search) stay as they are, without settings. If one is ever wanted the original way, it becomes a setting then.
- **The fixes that change numbers in play** (the lapis scan, the grade bonuses, damage over time, Fire Thorns, the obelisk grade, the
  full-party bonus, "bring more for more") and **the boss scripts** stay fixed.
