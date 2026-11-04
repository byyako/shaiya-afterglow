# Live test checklist

Everything built but not yet tried with the real client, with where to try it and what should happen. Tick items off (or note what went
wrong) and the server log (`logs/game-server.log`) fills in the rest. Remove sections once they pass.

**Section 0 is the pre-launch smoke list** (`docs/testing/smoke-list.md` is its master, copied here by `tools/smoke-list-sync.sh`): twelve items,
walked on a new server before players join. Sections 1 and up are the long list.

Which items one tester can do with one client, which need two clients, and which to skip: `docs/testing/live-test-solo.md` (with the
one-client items grouped into sittings).

The same list as a page to tick off while playing: **http://<the box>:30880/**, served by `tools/checklist-server.sh start` (`tools/checklist_server.py`). It is built from this file at every
load, so an edit shows at the next reload with nothing to publish. Results and notes go to `logs/live-test-results.json` on the box
running the page, and `tools/checklist-server.sh report` prints the marked items as text (results of reworded items are listed apart).
An item keeps its result as long as its text is unchanged. `tools/checklist_page.py <out.html>` builds a stand-alone copy of the page;
it does not save results (it has "Copy results").

**Log review:** the game and login logs (10-13 to 10-20 UTC) and the database were read against this list. Windows, messages
and icons leave no trace in them, so an item is ticked only where the log or the database shows the whole server-side outcome; what the
logs show for the others is in a "Log review" paragraph at the top of each section. The logs hold client sessions on 10-13, 10-14, 10-16
(04:30 to 04:55 UTC), 10-19 and 10-20 only. Sections with no trace at all: 11, 12, 14 to 21, 23 to 26, 28, 32, 36 to 38 and 40 (37 and 38
have their setup applied; no command was run). The logs show no failure of any item. The only errors are database
connection resets on 10-19 at 04:32 UTC (a Docker upgrade, no client online) and the known slow-statement warnings.

## Test setup

Three accounts (make them with `target/release/create_account`; any password), then the characters below on them (create each in
the game, then set its level, skills and position with the GM commands, or in the database with the server stopped; the items and
sittings say where they need a character somewhere else). Items and sections say **the GM account**, **the first player account**
and **the second player account**.

- **The GM account**: `accounts.status` 16 (staff level 1). Where a section raises or lowers a status, it goes back to 16 afterwards.
- **The first player account** and **the second player account**: `accounts.status` 0.

All the characters are level 30 with 500 skill points and nothing learnt yet (learn the skills below first), unless the table says
otherwise.

| Account | Character | Race, class | Level | Where, and role |
|---|---|---|---|---|
| first player | TestRanger | elf, Light, ranger | 30 | map 47 (138.6, 8.6, 459.6), for section 31 |
| first player | TestMage | elf, Light, mage | 30 | map 35 (380, 26, 285), master of the guild Builders, for sections 34 to 36 |
| second player | TestPriest | human, Light, priest | 30 | map 53 (at TestMage's position), member of Builders at rank 9, for section 36 (section 31 needs it on map 47) |
| the GM | TestVail | vail, Fury, ranger | 30 | map 1 (1205, 12.5, 1617), 30 from three Light guards, for section 33 |
| first player | ProbeHero | human, Light, fighter | 10 | map 1 (736, 1796) |
| second player | sefewfe | elf, Light, archer | 10 | map 1 (647, 1745) |

(Sections that moved a character say so. Section 21's TestVail/TestRanger pairing needs TestRanger back on map 1.)

Skill numbers are the ids in `defs.skills` (the client shows the names).

## 0. Pre-launch smoke list

Twelve things a new player does in the first half hour, walked on a new server before players join, and again after an update
(`docs/plans/roadmap.md`, "Testing"). It is short on purpose: if one of these fails, nothing else is worth testing yet. The long list
(sections 1 to 57 below) is for anyone testing with the real client, and each item here is drawn from the section named in it.

Walk the items in order; later ones use what the earlier ones left (the gold, the Apples, the position). Items 8 and 9 need two game
clients at once (`docs/testing/live-test-solo.md`, group B). Mark each Passed, Failed or Can't test, and when it fails write what you saw in the
note: the next session reads the notes with `tools/checklist-server.sh report` and looks up the logs itself. The "If not" half of each
item is for that session, and for you if you want to look.

The accounts and characters are those of the checklist's "Test setup" (positions move when someone plays):

| Account | Character | Where | Carries |
|---|---|---|---|
| the second player account | sefewfe (elf archer, level 10, Light), the main character | map 1 (565, 1818), among the human town's merchants | Short Bow in hand, 1,000,000 gold, 10 Apples (bag 1 slot 0), 10 Small Ressurrection Runes (bag 1 slot 1), empty warehouse |
| the first player account | PISS (human priest, level 1, Light), the trade partner | map 1 (521, 1792), beside the Light bank teller | a staff in hand, 0 gold, empty bags |

On map 1: the weapons merchant (NPC 1/5) at (535, 1812); the potion and fruit merchant (1/8) at (511, 1788); the warehouse keeper at
(491, 1785); small thief monkeys (monsters 2008 and 2009, levels 1 and 2) in small spots between x 690 and 800, z 1800 and 1880, the
nearest around (740, 1815).

Logs: `logs/login-server.log` and `logs/game-server.log` (debug level, so the refusals show), newest at the bottom; `grep` them for the
quoted text. The game also writes what the original's `ps_gameLog` wrote into the database: `U=$(. ./.env; echo "$DATABASE_URL" | sed -E
's#@[^/]+/#@localhost:5432/#'); docker exec -i shaiya-afterglow-postgres psql "$U" -c "SELECT at, character_name, action, value1, value2, value3,
text1 FROM game_log WHERE character_name IN ('sefewfe','PISS') ORDER BY id DESC LIMIT 40"` (actions: 232 enter and leave, 111 pick up, 161
gold picked up, 113 buy, 114 sell, 121 and 122 warehouse, 116 and 164 trade giver, 115 and 163 trade receiver).

- [ ] **1. Login (the second player account)** (sections 30 and 57). Steps: on the login screen type the second player account's name with a wrong password,
      once: the client says so and stays on the login screen. Then the right password: the server list shows; pick the server; the character
      screen lists sefewfe and TestPriest. Try only one wrong password (five make that address wait 15 minutes for the account).
      Should: no error box and no stall of more than a few seconds. If not, `logs/login-server.log`: "login refused … user="<second player account>"
      status=BadCredentials" for the wrong try and "login accepted … user="<second player account>"" for the right one (neither: the client never
      reached port 30800, so check `tools/login-server.sh status` and the client's login address; accepted but no "server selected":
      the server list, see the `servers` tool); then `logs/game-server.log`: "entered the character screen … user_id=6" (missing: the
      game server did not take the session; read the WARN or ERROR lines just before).
- [ ] **2. Enter the world (sefewfe)** (section 1). Steps: select sefewfe and start. Should: map 1 loads and sefewfe stands in the human
      town at about (565, 1818) among the stalls, bow in hand, the HP, MP and SP bars full, the Apples and runes in bag 1, 1,000,000
      gold; NPCs and other characters are in sight; the client stays connected for a minute of standing still. If not,
      `logs/game-server.log`: "entering the world … id=2 name=sefewfe" then "in the world … id=2 map=1" (the first without the second:
      the load never finished, read the lines between; "dropped … error=" or "connection ended in the world" soon after: the client
      let go or the server ended it), and game_log action 232 ('Enter').
- [ ] **3. Move (sefewfe)** (section 45). Steps: walk and run east out of the town to the grass at about (740, 1815), around the
      stalls and the guards, then half way back; click far points and near ones. Should: the character follows the clicks, never snaps
      back to an earlier spot, nothing freezes, and monsters and NPCs appear as you come near. If not, `logs/game-server.log`: "move
      refused: too fast" (the server put you back; the line gives `walked`, `allowed` and `speed`) or "move refused: step too long";
      the debug "move window … average=" lines give the real speed.
- [ ] **4. Fight (sefewfe)** (sections 1 and 2). Steps: at about (740, 1815) click a small thief monkey and shoot it with the bow until
      it dies; kill five. Let one monkey reach you and hit you once, then stand still. Should: damage shows on the monkey, it falls,
      the experience bar moves with every kill, the monkey's hit lowers your HP, and the HP comes back over a few seconds out of combat.
      If not, `logs/game-server.log`: "attack refused: not attackable" or "attack refused: not an enemy here" (the server turned the
      click down); "not handled in the world yet … opcode=" (a client packet the server does not know); nothing at all with no monkeys
      in sight: "monsters spawned spawned=… total=…" at start-up says whether the map was filled.
- [ ] **5. Loot (sefewfe)** (sections 15 and 48). Steps: keep killing monkeys until something lies where one died (gold or an item;
      expect one within about ten kills), walk to it and click it. Should: gold goes straight into the gold count; an item goes into
      the first free place of the bags; nothing says "This doesn't belong to you" (the kill was yours). If not, `logs/game-server.log`:
      "picked up id=2 item_id=… loot=Item {…}" (debug); game_log action 111 per item and 161 for gold (value1 the amount, value2 the
      gold after). An item that will not be picked up with free bag places: no "picked up" line at that second means the server refused it silently (it is out of reach, or already gone).
- [ ] **6. Shop (sefewfe)** (section 48). Steps: back in town, click the weapons merchant at (535, 1812) (it must be on
      screen) and buy one Long Sword (100 gold); sell it straight back (10 gold). At the fruit merchant at (511, 1788) buy 2 Pomegranates
      (2,500 each). Write down the gold before and after each step. Should: the gold falls by exactly the price the window shows and
      rises by 10 for the sale; the sword and the fruit land in the first free bag places. If not, `logs/game-server.log`: "bought
      character_id=2 item_type=1 type_id=1 count=1 price=100 money=…" and "sold character_id=2 … gain=10 money=…" (no line at all: the
      request never got through, the merchant was out of range or out of view); game_log 113 and 114; the gold after a relog
      (`SELECT money FROM characters WHERE id = 2`) must match the last "money=".
- [ ] **7. Warehouse (sefewfe)** (section 14). Steps: at the keeper (491, 1785) open the warehouse. Drag the 10 Apples (bag 1 slot 0)
      into it: they move and no gold is taken. Close the window, open it again: still there. Take them out into an empty bag place:
      the gold drops by 12 (5 % of the stack's sell price, 10 × 25, rounded down; 11 or 13 is the original's float rounding, still
      fine). Then drag the 10 Small Ressurrection Runes in and leave them there for item 10. Should: the window and the bags agree
      after each move, and a taken-out stack is whole. If not, `logs/game-server.log`: "warehouse move character_id=2 from=(1, 0)
      to=(100, …) fee=0 money=…" going in and "fee=12" coming out (a move that shows nothing: "warehouse move ignored");
      game_log 121 (in) and 122 (out, the fee in value8).
- [ ] **8. Trade, two clients (sefewfe and PISS)** (section 8, `docs/testing/live-test-solo.md`). Steps: the second client logs in as
      `<first player account>` and enters the world as PISS (it stands beside the bank teller at (521, 1792)). Walk sefewfe to within 10 of PISS.
      On sefewfe click PISS and ask for a transaction; PISS accepts. sefewfe puts in the 10 Apples and 1,000 gold; PISS puts in nothing;
      both confirm. Should: the Apples and 1,000 gold leave sefewfe and arrive in PISS's bag and gold; both windows close. Then a
      second request, and sefewfe closes the window: both see "Transaction has been canceled" and nothing moves. If not,
      `logs/game-server.log`: "traded id=2 partner=7 …" and "traded id=7 partner=2 …" (one per side; "trade refused" or "trade
      cancelled: …" explain a failure; "trade offer refused: no such item, too many, a quest item or not tradeable" for an item the
      window let in but the server did not), game_log 116 and 164 for sefewfe, 115 and 163 for PISS.
- [ ] **9. The same account twice (the second player account on both clients)** (section 30). Steps: log PISS out to the login screen on the second
      client (sit, menu, wait 10 s, then back to the login screen). With sefewfe still in the world on the first client, log in as
      `<second player account>` on the second. Should: the second client picks the server, waits a few seconds, then shows the character screen,
      with no error and no second try; the first client's game ends. Enter the world as sefewfe on the second client: same place, same
      gold. If not, `logs/game-server.log`: "the account logged in again: its connection here ends", then "connection ended in the
      world; the character stays" (linger 10 s) and "left …", then "entered the character screen … user_id=6" for the new one;
      `logs/login-server.log` must hold exactly one "login accepted" for the second login.
- [ ] **10. Relog and what stayed (sefewfe, then PISS)** (sections 1, 14 and 30). Steps: sit, choose the character screen (10 s), enter
      again; then sit, quit the game, and log in as the second player account once more. Check the place, the gold, the bags (the Pomegranates, no
      sword, no Apples), the experience bar, and in the warehouse the 10 Small Ressurrection Runes. Take the runes out. Last, log in as
      `<first player account>` and enter as PISS: the Apples and the 1,000 gold are still there. Should: every number is what it was before the
      logout. If not, `logs/game-server.log`: "logout pending … delay=10s" and "back to the character screen" or "quit" (a request with
      only "logout request ignored: not sitting or already pending": the character was not sitting), then "left"; a missing
      value: compare with the database (`SELECT money, exp, map, pos_x, pos_z FROM characters WHERE id = 2`, `bank_items` for the
      warehouse, `character_items` for the bags) and whether "left" came before the next "entering the world".
- [ ] **11. The logs are clean.** Steps: after the walk, read both logs from the last start-up on. Should: no ERROR line and no panic in
      either; WARN lines only the known ones ("slow statement", "time to acquire", "heartbeat failed" when the database was slow).
      The check: `tail -n +$(grep -n "game server listening" logs/game-server.log | tail -1 | cut -d: -f1) logs/game-server.log | grep -E
      "ERROR|WARN|panic" | grep -v "slow statement\|time to acquire"`, and the same with `login server listening` on
      `logs/login-server.log`. If not: copy the lines into the note.
- [ ] **12. Web registration (not built: the web API, `docs/plans/roadmap.md` B1, is not scheduled).** Mark Can't test unless your
      server has a registration page of its own. Then: register a new account
      on the site (an unused name, a password of 6 to 16 characters), log in with it in the client, create a character, enter the
      world and walk a few steps. Should: the account works at once, the character screen is empty, creation and entering work as for
      any account. If not: the login server's log for "login refused" with the name, the registration page's own log.

## 1. Basics not yet seen live

**Log review:** Seen: 13 deaths (11 TestVail to guards, TestRanger and TestMage to monsters), 10 "back to life" lines at the
bind point or rebirth spot and 3 "left dead" revives. Every one says `loss=0`, because all four level-30 test characters sit at exactly
1,678,700 exp, the start of level 30, so the 5 % loss can't show: set `exp` to about 1,700,000 first to test it. Skills (TestMage's seven,
TestRanger's 192 after a relog) survive relogs; quick slots, passives, equipment numbers and kill experience are not in the log.

- [ ] Max HP/MP/SP and speeds change when equipping or removing gear.
- [ ] Experience from a kill; level-up effect and new stat/skill points (kill something your level).
- [ ] A monster attacks back, HP drops, HP/MP/SP recover over time when out of combat.
- [ ] Death: rebirth window, rebirth at the bind point, 5 % exp loss above level 10.
- [ ] Skill list and quick slots survive a relog; passive skills raise the numbers.

## 2. Using skills (part 2)

**Log review:** Ticked: Slasher 55 killed monsters on 10-14 and 10-20 ("killed=true"). Magic Ball 29 casts finish and kill
(TestMage 10-19 02:07 to 02:10); a step cancelling a cast isn't logged. Refusals in the log: result 6 (not enough SP or MP) for skills 30,
55 and 77, result 7 (wrong weapon) for 38, result 10 for 112 on a monster. A cooldown refusal (result 9) never appears. Hail 53 and
Healing 144 were never used.

- [ ] Self heal: priest Healing 144. Heal another player: priest 102 or 148 (on a second client).

## 3. Buffs and debuffs (part 3)

**Log review:** Buff casts accepted with no refusal: 132, 117 and 44 on oneself, Stealth 63 (three times). TestMage entered the
world on 10-19 15:15 with `applied=2`, at 15:25 with 1 and afterwards with 0: saved buffs come back after a relog and run out. Nothing in
the log for buffs on other players, debuffs, damage over time or dispel.

- [ ] Self buff: mage/priest 132 or ranger 44 (300 s). Icon shows, numbers change, it ends on time.
- [ ] Buff on another player: priest 116 (target another client's character).
- [ ] "Party" buff with no party: priest 123 / mage 117 / ranger 120 buff only the caster.
- [x] Cancel a buff by right-clicking its icon. (Tested: works for Stealth 63; the client offers no cancel for stat buffs such
      as 132 and 117 (skill type 82): it sends the cancel only for hiding, transformation and types 94 and 95, as the original client.)
- [ ] Debuff on a monster: mage 35 (stun), priest 37, ranger 61 (stop): the monster stops moving/attacking for the duration.
- [ ] Damage over time on a monster: mage 65/66 or ranger 135: damage every 3 s; it can kill (exp and drops still come).
- [ ] Dispel: priest 103 on a player who has a debuff from a monster (see section 6) removes it.
- [ ] Low-HP passive (if one is learnt): kicks in under the HP threshold.

## 4. Hiding, transformations, fields, one-off skills (part 4)

**Log review:** Only casts show: Trap 113 (TestRanger 10-16), Illusion 77 (accepted 10-16 04:37:06 after a refusal for MP at
04:32), Fire Wall 48 (TestMage 10-19 02:10:25), Stealth 63 (three times), Disguise 107 refused "wrong target" 12 times (monsters and no
target, as it should). What the shapes and fields did to monsters and a second client isn't in the log. Scout 100 was learnt by TestVail
(05:57) and never used; Detect, Resurrection, Evolution and Fire Thorns were never learnt.

- [ ] Stealth 63 (ranger, level 8): you turn invisible to a second client; monsters ignore you; attacking ends it. (Tested: casts with
  one client; still needs a second client to see it.)
- [ ] Detect 43 (priest, level 8) near a hidden enemy-faction player reveals them (needs TestVail hidden near TestPriest).
- [ ] Illusion 77 (ranger, level 21) on a monster: you look like that monster to others; attacking ends it. (Tested: refused with
  "insufficient energy" after Trap and skill 44 had used up the ranger's MP; Illusion costs 80 MP. Retry with full MP.)
- [ ] Disguise 107 (ranger, level 23) on an enemy player: you look like them. (Tested: on no target and on monsters it is refused, as it
  should be; it needs an enemy-faction player such as TestVail.)
- [ ] Evolution 127 (priest, level 15): monster shape that does not end on attacking.
- [ ] A second client coming into view sees the shape (hidden, illusion, disguise, evolution).
- [ ] Fire Wall 48 (mage, level 8) / Trap 113 (sefewfe or ranger, level 10): a field on the ground that hurts monsters walking in.
- [ ] Scout 100 (ranger, level 23) on a monster: shows its info.
- [x] Return 192 (everyone, level 1): 10 s cast, then you are moved to the bind point. (TestRanger)
- [ ] Resurrection 126 (priest, level 10) on a dead player: they come back at the priest's spot and can walk normally.

## 6. Monster skills and debuffs on players

| Map | Spot | Monster | Level | What it should do |
|---|---|---|---|---|
| 1 | (194,1762) | 2031 | 10 | single big blow skill |
| 1 | (1612,1310) | 2026 | 8 | multi-hit skill |
| 1 | (241,1816) | 118 | 12 | lowers your abilities (debuff icon, numbers drop, then come back) |
| 1 | (1280,718) | 2051 | 15 | stun: you cannot move or act for the duration |
| 2 | (174,828) | 202 | 6 | damage over time on you, every 3 s |
| 2 | (1577,1728) | 2102 | 17 | stop: you cannot move (can still act) |

**Log review:** Ticked: TestMage's Magic Ball was cut by a monster's hit twice ("cast broken by a hit" 10-19 16:15:01 and
16:15:05, monster 19512 in the ranking zone). Not decidable from the log: the six monster skills, the debuff icons, and Stealth ending on
a hit (TestRanger cast Stealth at 02:13:14 and a monster killed it at 02:13:27, but not when the Stealth ended).

- [ ] Each monster above uses its skill and the effect matches the table.
- [ ] Debuff icons show and go away when the debuff ends.
- [x] A monster's hit (normal or skill) breaks a cast in progress (start Magic Ball 29 while a monster is hitting you).
- [ ] A monster's hit ends Stealth 63.
- [ ] Dying to damage over time works like any other death.

## 7. Parties

Two members of the same faction on different accounts: TestPriest (the second player account) with TestMage or TestRanger (the first player account). A full party (7,
double experience) cannot be tried with the accounts we have.

**Log review:** One party formed on 10-16: TestPriest invited TestRanger twice (04:42:43 and 04:42:55) and "joined a party
party=1 inviter=5 member=3" followed at 04:42:57. Both logged out at 04:45:06 and TestRanger "left the party" at 04:45:16 (logging out
leaves it). No kill, loot, buff, leader or kick line, so only the invite and the logout part of the list is seen.

- [ ] Killing together within 100 of each other: both get experience (each at its own level, no ×1.2); items go straight into the bags
      in turn with "<p> attained <i>" for the other; gold is split, straight into the money; nothing of that is left on the ground.
- [ ] A member too far (over 100) gets nothing of the kill.
- [ ] Party buff (priest 123, level 12, or mage 117) reaches the other member nearby.
- [ ] Dead, choose to wait for the party: 30 s later back to life next to the leader (leader alive on the same map), else at the bind
      point.
- [ ] Grant leadership, kick, leave; a party of two ends when one leaves or logs out; logging out leaves the party.

## 8. Trade

Two characters near each other (within 10), same faction: TestPriest (the second player account) with TestMage or TestRanger (the first player account). For trade
between factions: TestVail (the GM account, Fury) with a Light character, both in the Auction House (map 42).

**Log review:** One trade completed: "traded id=5 partner=3 items=2 money_gained=0" at 10-16 04:44:52 (TestPriest and
TestRanger, two items, no gold), 8 minutes after they formed a party. Neither character entered the world again afterwards, so the relog
check, the refusals, the cancellations and the faction case are untried.

- [ ] Closing the window cancels it for both ("Transaction has been canceled" / "trade has been cancelled by the other player").
- [ ] Walking more than 10 away, moving an item in the bag, picking something up, buying/selling at an NPC or dying ends the trade.
- [ ] Not enough room in the bags: "Insufficient inventory for a transaction" on the one short of room, nothing changes hands.
- [ ] Quest items, equipped items and untradeable (bound, `reqog` 1) items cannot be put in.
- [ ] A request nobody answers lapses after 30 s: both can trade again.
- [ ] Other faction: refused outside the Auction House ("Transaction in process with another user"); inside it, the trade works.
      (Does the client offer "request transaction" on a player of the other faction at all?)

## 9. Guilds

The dev server runs with `guild_party_size = 2` (in `config/game.toml`; the original needs a party of exactly 7; remove the line for that), so
TestPriest (the second player account) and TestMage (the first player account), both Light and level 30, can create one in a party of two. TestPriest, TestMage and
TestRanger have 100,000 gold (creating costs 1,000). Joining a guild starts a 72-hour penalty against creating and join requests (not
invitations): try join requests with a character that has not joined one (TestRanger).

**Log review:** No guild was created or changed through the client in the log (Rangers, Priests and Builders were made in the
database). The remark was changed on Rangers ("Test message") and on Builders, so the master's remark change reached the database; whether
TestMage's attempt was refused isn't visible. Priests (2) was closed in the database on 10-19 16:42 when TestPriest was moved into
Builders.

**Tested (the maintainer's report; the logs match the sessions):** Passed but the relog item. The server sends the online/offline state (`0x0D0C` 104/105) to the other members, as the original; this client only changes the member's state in the list (`docs/client/client-social-packets.md`) and prints no text, so the item is reworded to the list.

- [ ] Create: TestPriest and TestMage in a party, on the same map; TestPriest fills in the guild-creation window (name and remark). Both
      get the agreement dialog (the 3-day warning), the requester too; both yes → "Formed <name> guild" for both, 1,000 gold off
      TestPriest, the guild window lists both (TestPriest the master), the name shows over both heads, also for a second client nearby.
- [ ] One of them says no → "rejection of party members" for both; nobody answers for 30 s → "Failed to form <name> guild".
- [ ] A name already taken (any case) or with `'` in it → the banned-words message; not in a party of two → "shortage of members".
- [ ] Relog: the guild name in the character window and over the head, the member list with online and offline members; the
      other member's entry in the list changes between online and offline as one logs in or out (the client prints no message for it).
- [ ] Kick: the master kicks TestMage → "Kicked from guild" for her, "Kicked <b> from guild" for the master; her name over the head goes.
- [ ] Invite: the master targets TestMage and uses `/sign up for guild` → the dialog; yes → "… guild accepted join-request" for her,
      "Guild accepted join-request" for the master, she is in the member list with rank 9; no → "Guild rejected join-request" for the
      master. A rank-9 member inviting is refused.
- [ ] Join request: TestRanger (no guild yet) asks from the guild list → "Requested to join"; the master sees the request and accepts
      → TestRanger joins (or refuses → "… guild rejected join-request").
- [ ] Leave: a member leaves → "Quit guild"; the master cannot leave while others remain ("Impossible to quit guild"), can as the last one
      (the guild then closes).
- [ ] Disband: with two members or fewer the master disbands at once → "Guild has been disbanded" for the members, the guild leaves the
      guild list. (A bigger guild needs the master and two officers in a party; not testable with two Light accounts.)

## 10. Quests

Quest ids are the server's (`world.quests`); the client shows their names. Every quest gives **twice** the experience its data lists
unless it is for ultimate mode only (the original does this; the window may show the single amount). The NPC must be on screen.

**Log review:** ProbeHero (human fighter) took quests 160, 13 and 155 at 10-20 01:14 to 01:18 UTC and finished 155 ("quest
done" 01:21:17: result 0, 200 experience, 990 gold, no item); quest 156 was taken straight after, so the chain works. In the database 13,
156 and 160 are in progress with no kills counted and 155 is finished. The quests named below (1, 3, 1239, 9, 161, 166) have not been
taken, nobody gave a quest up, and no party kill is logged.

- [ ] Kill quest, human start (TestPriest, map 1): the guard at (622,1756) gives quest 1 (kill 5 small thief monkeys, monster 2008) →
      quest accepted sound, the quest in the quest window. Each kill shows "… battle : n/5". Hand it in at the guard captain beside it
      (622,1751): "Completed the quest", 100 experience (50 doubled), 330 gold. Quest 2 (5 foxes) is offered next by the captain.
- [ ] The same from the elf start (TestRanger or TestMage): quest 3 at the guard at (1649,1774), handed in at (1623,1696).
- [ ] In a party: a kill by one member counts for the other's quest when within 100 of the killer.
- [ ] Talk quest: quest 1239 at the NPC at (461,1786), handed back to the same NPC: 10 experience doubled to 20, 100 gold.
- [ ] Reward choice: TestPriest (human only) takes quest 160 at (631,1753) (kill 5 of monster 2019), hands it in, then quest 161 at the
      same NPC: handing 161 in opens the reward choice (three items); the one picked arrives in the bags with "Completed the quest".
- [ ] Bring items with a choice: quest 9 at the merchant at (559,1815) (bring 5 of quest item 27/2, from monsters nearby): the items go
      when the reward is picked (two to choose from).
- [ ] Time limit: quest 166 at the merchant at (570,1711) (60 minutes, kill 5 of monster 2009, more kills a bigger reward): the window
      shows the time left; after a relog the time left is where it was at logout. Kill 10 or 15 before handing in → the bigger result.
- [ ] Give up a quest from the quest window → "Failed the quest"; the same NPC no longer offers it (as the original: a failed quest is
      never offered again, except repeatable ones).
- [ ] Full bags (all usable slots taken): a quest that gives an item at the start, or a reward item, is refused with "Insufficient
      inventory"; with room again it works.

## 11. Friends, the block list, the party search

Two clients of the same faction on the **same map** (for example TestRanger on the first player account and TestPriest on the second player account, both light). The
friend window and the block list are in the client's community window.

- [ ] Add a friend by name: the other client gets the question; yes → both lists show the other, online. No → the asker gets the refusal
      message. No answer for 30 s → nothing, and a new request works afterwards.
- [ ] A friend logging out shows offline in the other's list, and online again when it comes back. Relog: the list is still there,
      online states right.
- [ ] Remove a friend: gone from your list; you show offline in theirs (they keep you listed, as in the original).
- [ ] Party search (community window): registered; two players of the same faction within 5 levels on the same map, both registered and
      not in a party, find each other. Joining a party takes you out of the search.

## 12. Spending stat points

Any of the level-30 test characters (145 stat points each). The stat window is in the character window.

- [ ] What does the client do if it is somehow refused (the server answers with zeros)? Only reachable with a modified client; skip unless
      convenient.

## 13. Using items

TestRanger (the first player account) and TestPriest (the second player account), both Light, level 30, have test items in **bag 2** (inventory page 2): apples, Mini
Healing and Mini Stamina Potions, Stun Cure and Perfect Cure Scrolls, Scroll Protected Faint, a Stat Reset Stone, a Skill Reset Stone,
Capital/Bootleggery/Arena Recall Runes, Movement Runes, Party Member Summon Stones, Target Resurrection Runes, a Repair Hammer, Increase
Fame, Victory Firecrackers, Health Remedies, a Small Increase EXP stone, a Blue Dragon and a Red Phoenix Charm.

**Log review:** Only three kinds of item use are logged: an apple (25001, TestVail 10-19 05:57), item 25018 (ProbeHero 10-20
01:19:36) and the six Goddess Blessings of section 22. Refusals aren't logged, so the reuse-group, cure, protection, rune, reset and charm
items have no trace.

**Tested (the maintainer's report; the logs match the sessions):** Passed. Increase Fame: the server saved the honor (TestPriest has `hg` 200 after two uses) and sent `0x0230`; this client shows no honor in the character window.

- [ ] A Stun Cure Scroll while stunned by a monster (map 1 (1280,718), mob 2051): the stun ends. Perfect Cure: every debuff ends.
- [ ] Scroll Protected Faint, then let a stunning monster skill hit: the first such skill misses (once).
- [ ] Health Remedy: max HP +500 for 3 h (buff icon). Small Increase EXP: applied; a second one is refused while the first lasts. Red
      Phoenix Charm, then the Blue Dragon Charm: the blue one is refused (weaker).
- [ ] The client's free skill reset (NPC type 13 menu, option 3 "1 chances left"; bags III and up must be empty): works once, then is
      refused. **Right after it the menu should say "0 chances left"** (the 6-byte `0x0215`, deviation 5); a Skill Reset Stone leaves the
      count alone. The free stat reset is always refused.
- [ ] Capital Recall Rune: a 5 s cast is shown; then the move to the Light capital (map 35). Step during the cast: it stops and the rune
      stays. Bootleggery and Arena runes likewise (maps 42 and 40).
- [ ] Movement Rune with two clients in a party: used on the other member (the client asks for a target), 5 s cast, then you arrive where
      the member stood when you used it, also on another map.
- [ ] Party Member Summon Stone: 5 s cast; the other member gets a question; yes → moves to the summoner; no → the summoner gets the
      refusal; no answer for 35 s → the summoner gets the refusal.
- [ ] Target Resurrection Rune on a dead party member / same-faction player in the same zone: it comes back to life where you stand.
- [ ] A quest that starts by using an item (start type 4), if one is found in the data, starts after using that item.

## 14. The warehouse

Any test character (TestRanger and TestPriest also have 3 Warehouse Recall Runes and a Double Warehouse in **bag 2**, slots 21 and 22). The
warehouse is the account's: TestRanger and TestMage (both the first player account) share one. The human start's keeper is on map 1 at (491,1785); others
at (1560,1811) and (1046,821).

- [ ] Drag a stack and a sword from the inventory into the warehouse: they move, no money taken. Relog (or switch to TestMage): still there.
- [ ] Take them out into empty inventory slots: the money drops by 5 % of the stack's sell price (rounded down; the window's money should
      follow at once). Into a taken slot, or straight onto the equipment: nothing happens.
- [ ] Inside the warehouse: move to an empty slot; two stacks of the same potion combine (overflow stays in the source); two other items
      swap.
- [ ] A quest item (types 27 to 29) cannot go in. An item offered in an open trade: the trade is cancelled by the move.
- [ ] Walk away from the keeper (the window closes) and use the Warehouse Recall Rune: the window opens anywhere, moves work for the 5
      minutes of the buff; after it ends the window closes and moves are refused (deviation 1: the server checks for a keeper in view or
      the buff).
- [ ] The second page (slots 120 and up): refused without the Double Warehouse buff; with it, items go in. After the buff ends, items there
      can still be taken out.

## 15. Dropping items

Any test character; a second client (same faction, standing next to the first) for the ownership check.

- [ ] The second client tries to pick up what the first dropped: "This doesn't belong to you" for the first 30 s, then it can; after 60 s
      the item disappears for both.
- [ ] Equipped items and quest items (types 27 to 29) cannot be dropped (nothing happens). Dropping during an open trade cancels the trade.

## 16. Equipment wear and repair

Any test character with a weapon and some armour equipped. The human start's blacksmiths are on map 1 (three of them; NPC type 3).

- [ ] An item worn below half its quality gives half its attack or defence (character window), below a tenth nothing.
- [ ] At a blacksmith, repair one item and "repair all": money goes down by the price the client shows (one item: missing quality / 400 ×
      buy price / 30, rounded down, at least 1), the item is back at full. "Repair all" with nothing worn costs 1 gold.
- [ ] Away from any blacksmith (a modified client, or if the client lets you): nothing happens (deviation 1).
- [ ] With Eternal Endurance (ability 36) active, nothing wears.

## 17. Saved positions, body change, experience wheels

Items: Teleportation Stone 100/65, Appearance Change Scroll 100/70, Sex Change Scroll 100/71, experience wheels 44/213 and 44/214 (give
them in the database while the server is stopped).

- [ ] Save a position from the Teleportation Stone's window (slots 1 to 4) in a field map: accepted. In a dungeon: refused. Relog: the
      saved positions show again in the window (`0x0225` in the burst).
- [ ] Use the stone on a saved slot: 5 s casting (`0x0221`), a step cancels it; when it ends you stand at the saved spot and the stone is
      used up. An empty slot: the item error.
- [ ] Appearance Change Scroll: pick a new hair, face and size: the scroll is used, you and a second client see the new looks at once
      (`0x0226`); relog: kept, also on the character screen.
- [ ] Sex Change Scroll: the same with the sex; while wearing sex-bound equipment (items of special 64 or 65) it is refused (`0x0228`).
- [ ] Experience wheel on map 18 or 30 with the experience past half the level: its buff applies and the experience bar goes back to the
      level's start. Below half, or on another map: the item error.

## 18. Mounts

A character with a mount (item type 42, e.g. 42/1) equipped in the mount slot; for two on one mount, two characters of the same faction in
hard or ultimate mode and a mount of mode 2 or more (e.g. 42/17 to 42/24).

- [ ] Summon: the summoning animation shows to you and a second client (`0x0217`); after 5 s (3 s for mode-3 mounts) you sit on the mount
      and the second client sees it. Summon again within the mount's 10 or 20 s: refused ("too soon").
- [ ] A step or a monster's hit during the summoning stops it. Sitting, or without a mount: refused.
- [ ] Riding: attacks, skills and item skills do nothing; potions still work. A monster's hit throws you off. Taking the mount out of its
      slot throws you off. Using the summon key again gets you off.
- [ ] A gatekeeper or portal to another field map: still riding there; into a dungeon: on foot.
- [ ] Two on one mount: ask the second character (`0x021D`); it accepts: it sits behind you, both clients and a third see it. It gets off
      on its own (`0x021C`), or both go down when either is hit. Asking a player of the other faction, one already asked, or with a
      one-seat mount: refused. No answer for 30 s: the invitation lapses.

## 19. Lapis, enchanting, craft stats, cloak badges

Given to TestRanger (3) and TestPriest (5), bag 1 slots 10 to 22, and 10000000 gold each: a weapon with lapis room (9/44, 4 rooms, for
TestRanger; 12/54 for TestPriest), three Craft Lapis Lv1 (30/1, weapons), a Max Craft Lapis (30/7, dangerous), 5 Linking Hammers Lv1
(100/195), 5 Operator's Exclusive (100/82, slot 23), 3 Lucky Charms (100/139), two Power Lapisia (95/2) and a Weapon Lapisia (95/1, dangerous), 100 of each weapon's enchant
material (25/137, 25/157: the weapon's `reqluc`), 3 Recreation Runes (100/171) and a cloak (24/1). Starter weapons have no lapis room.
Stand beside a blacksmith.

- [ ] Link a lapis into a weapon at the blacksmith: the price (4095 for grade 31, less a tenth of the character's VG) is taken, one lapis
      is used, success about 41 % + level/20 (with a Linking Hammer × its percentage, at most 50 %). On success the lapis shows in the
      item and its stats count (character window); relog: still there.
- [ ] With the Operator's Exclusive (100/82) as the hammer: every link succeeds, even the Max Craft Lapis (deviation 4).
- [ ] A lapis that does not fit the item (e.g. a weapon lapis into armour), the same lapis twice, or no room left: nothing happens.
- [ ] Away from a blacksmith: nothing happens (deviation 3).
- [ ] A dangerous lapis (`reqvg` ≠ 0, the Max lapis) failing without a Lucky Charm: the item is destroyed (`0x0206`); with a charm one
      charm is used instead (and also on success).
- [ ] Extract (the client's "remove lapis", 4 bytes): all gems at once, the price of each; success at the first gem's rate puts each
      lapis in a free bag slot; failure loses them (and a dangerous one destroys the item). Without enough free slots on success: nothing.
      **The gold shown after it must be right** (the 33-byte `0x0802`; the old 34-byte answer showed money × 256) and the lapis must land
      in the slots the log names.
- [ ] Enchant a weapon with Lapisia 95/1 and its material: the price 2500000; step 0 → 1 at 70 % (weapons) or 99 % (armour); on success
      the item's name shows +1 and the attack (or absorption for armour) rises; others see the new level on equipped gear. Without the
      material: nothing. The dangerous Lapisia destroys the item on failure.
- [ ] Armour with absorption: a monster's blow shows "<n> absorption damage" (`0x0525`) and takes that much less HP (at least 1).
- [ ] Recreation Rune on a craftable item: new random craft stats, the enchant level kept, the rune used. On a quest reward: refused.
- [ ] Cloak badge: putting a badge costs 450000 and shows on the equipped cloak to others; removing it is free. With less than 450000:
      refused.
- [ ] Remake (if the client offers it): three of the same craftable item become one with new stats; the same item named three times
      does nothing (deviation 2).


## 20. Craft stats on new items, boxes

Kill monsters that drop equipment, buy craftable gear from a merchant, and open the boxes given to TestRanger (bag 1 slots 3 to 6) and
TestPriest (bag 1 slots 2 to 5): two Normal Item Pockets (100/78), ten Fortune Bags (25/224) and a Mystery Lapis Pocket (100/68) each.

**Tested (the maintainer's report; the logs match the sessions):** Passed but the Fortune Bag, which gave nothing ten times of ten ("box gave nothing", set 19). Not a bug: in the shipped `ItemCreate.ini` set 19 names only grades 1011 to 1015, which have no items, so the original gives nothing every time too (`docs/original/item-origins.md` section 5.1). The item is removed.

- [ ] A weapon, armour or accessory dropped by a monster shows random extra stats (STR, DEX, …; armour also HP/MP/SP) when picked up, and
      equipping it adds them (character window). Relog: still there. Most such items have some stats; their enchant level is 0.
- [ ] Gear bought from a merchant has no extra stats. The Recreation Rune still works on it; "identify" (`0x0808`) and remake refuse it.

## 21. Player kills (two countries), the reward panels

Set up in the database (server stopped): **TestVail** (the GM account, Fury, level 30) was moved to map 1 next to
**TestRanger** (the first player account, Light, level 30) at about (1004, 1616), with 100,000 gold. TestRanger has **49 kills**, TestVail **49 deaths**,
so one kill reaches the second reward step. To set them again (server stopped):
`UPDATE characters SET kills = 49, kill_level = 0 WHERE id = 3; UPDATE characters SET deaths = 49, dead_level = 0 WHERE id = 6;`

Two clients: the first player account → TestRanger, the GM account → TestVail. **The victim (above level 10, VG 0) always loses one weapon, armour or accessory
from its bags** (equipped items are safe) and 30 % of the time some gold: keep nothing valuable in the bags.

**Log review:** No trace: no player kill (TestRanger still has 49 kills, TestVail 49 deaths and 0 victories).

- [ ] TestRanger attacks TestVail: blows land, both clients show the damage, HP drops. TestRanger cannot attack TestPriest (same country):
      nothing happens. Walk out of reach while attacking: the attack stops.
- [ ] TestRanger kills TestVail: TestVail falls; TestRanger gets experience (2,160 for a level-30 victim at level 30), the kill count goes
      to 50 (character window); TestVail's deaths go to 50. Players nearby see TestRanger's new kill count.
- [ ] TestVail lost one bag item: it lies at TestVail's feet, TestRanger can pick it up (TestVail cannot). Sometimes gold too (1 to 150),
      on the ground as gold, TestRanger's. Relog TestVail: the item is gone from its bag.
- [ ] TestVail comes back to life with the usual 5 % experience loss.
- [ ] A monster that did most of the damage gets the kill: let a monster hurt TestVail badly, then TestRanger finishes it: the death names
      the monster, no kill or death is counted, nothing is lost. The other way round (TestRanger did most, a monster finishes): TestRanger
      gets the kill.
- [ ] Full HP clears that record: TestRanger hurts TestVail, TestVail heals to full (sitting or potion), then a monster kills it: the
      monster's kill.
- [ ] With three clients: TestRanger and TestPriest in a party, both near; TestRanger kills TestVail: both get experience and a kill; the
      lost item goes straight into one member's bag (turns), gold is split between the members near the killer.
- [ ] On map 40 or 42 the two countries cannot attack each other.
- [ ] Reward panels (level 16 and up): the kill/death reward window shows the next steps (TestRanger kill step 1). Take the kill reward:
      1 stat point (step 1); again: 2 stat points (step 2, 50 kills); again: nothing. TestVail takes the death reward: 100 gold, then 200,
      then nothing. Relog: the panel remembers the steps.
- [ ] A Stat Reset Stone on TestRanger puts the kill steps back to the first (the panel shows step 1 again); the death steps stay.

## 22. Skills on players, the faction kill count (player kills part 2)

Same two clients as section 21 (the first player account → TestRanger, Light; the GM account → TestVail, Fury; both on map 1 near (1004, 1616)). Learn the skills
first (500 skill points each). **TestVail has 6 Goddess Blessings** in bag 1 slot 1 (item 100/41). Watch `logs/game-server.log` for
"skill used", "goddess blessing" and "kill counts".

**Log review:** From the log (TestVail, 10-19 05:55:11 to 05:55:13): six Goddess Blessings used within 2.5 s, "goddess blessing
country=1" six times, and the Fury counts went 750, 1500, 2250, 3000, 7200, 12288 exactly as the item says, the sixth with `top: true` and
"obelisks reset". After a restart at 05:58:49 and another at 06:04:03 the server loaded "light=0 fury=12288 counting=true" (the count
survives a restart), and Fury went back to 0 at 06:14:04, ten minutes after the 06:04:03 load rather than after the top at 05:55:13: a
restart starts the ten minutes again. Because all six were used in seconds, a blessing being refused at the top, the client's countdown,
the +6 % HP/MP/SP and the relog check can't be read from the log. No player kill is in the log (`kills` 49 and `deaths` 49 are unchanged),
so the skills-on-players items and the per-kill count have not been tried.

- [ ] TestRanger uses an attack skill (134) on TestVail: the hit shows on both clients, TestVail's HP drops. On TestPriest (same country)
      the skill is refused ("Wrong target").
- [ ] A debuff (ranger 61, stop) on TestVail: TestVail is held for its time and the icon shows; a damage over time (135) ticks every 3 s.
- [ ] An area skill (TestMage's around-oneself or around-the-target skills, or a ranger area skill) near TestVail and a monster: both are
      hit (`0x0513` for the player); TestPriest standing in it is not.
- [ ] Kill TestVail with a skill: the kill counts as in section 21 (experience, kill count, loss), TestRanger stops attacking.
- [ ] After a skill on an enemy with normal attacks on (attack the enemy first, then use a skill): the normal blows go on afterwards.
- [ ] On map 42 no skill through `0x0511` works at all (buffs on oneself included); skills on monsters still do.
- [ ] Goddess Blessing (TestVail, Fury): the item is used up; every Fury player gets the new kill count (`0x020F`, 750) and TestVail's max
      HP, MP and SP rise by 6 % (Fury grade 5: 1:6, 2:6, 3:6, recovery 10/11/12). Relog: the bonuses are still there (`0x0211` shows 750).
- [ ] Use the other five: 1500 (grade 10: experience loss at death halved), 2250, 3000, 7200 … the sixth reaches the top (12288): the
      Light count goes back to 0, a 10-minute countdown runs on Fury's client, further blessings and kills do not count (the blessing is
      refused, kept). After 10 minutes Fury goes back to 0 and the bonuses go (max HP back down).
- [ ] Each player kill adds 1 to the killer's faction (log "kill counts"); a restart keeps the counts (saved every 10 minutes and at the
      top's resets; `SELECT * FROM faction_kill_counts`).

## 23. Duels

Two Light characters on two clients: **the first player account → TestRanger** and **the second player account → TestPriest** (both on map 1 near (1001, 1616), 10M gold
each, items in their bags). Request from the player menu (right-click the other player, "battle"). Watch `logs/game-server.log` for
"duel stakes taken", "duel result" and "duel stakes back in the bags".

- [ ] TestRanger asks TestPriest: both see "<p> requested battle to <t>." (5325). TestPriest refuses: TestRanger sees "rejected" (5326).
- [ ] Ask again and do not answer for 10 s: both see "…the opponent didn't respond." (5328).
- [ ] Ask and accept: the stakes window opens on both (the trade window). Stake money and an item on one side: the other sees them; a
      change after "ready" clears ready on both.
- [ ] Close the window (or move an item, or walk more than 40 away): it closes on both.
- [ ] Both ready: the window closes, the staked gold and items leave the bags (gold shown lower), "the battle starts after 5 seconds"
      with the area shown at the midpoint; after 5 s "start battle!!" (5311).
- [ ] Before the start the two cannot hit each other; after it they can (same faction).
- [ ] TestRanger kills TestPriest: TestRanger wins (5310 or the win message), TestPriest loses; **both stakes lie on the ground at the
      two players' feet, all TestRanger's** (TestPriest cannot pick them up for 30 s). No kill count, no item or gold loss from the death,
      and **no experience lost** at TestPriest's rebirth. `SELECT victories, defeats FROM characters WHERE id IN (3, 5)`: 1 / 1.
- [ ] `/defeat` while fighting: the duel ends ("<p> admitted the failure." 5315 and "<p> won at the battle." 5310 on the other side),
      each gets its own stake back on the ground at its feet. The client counts a win/loss itself; the server does not (relog shows 1/1
      from the kill above, not more).
- [ ] Walk more than 40 from the centre while fighting: the duel ends ("…out of allowed battle area"), stakes back.
- [ ] A monster hits one of them while fighting: the duel ends ("…you've been attacked before it finished." 5329?), stakes back.
- [ ] While fighting, TestPriest's party heal or buff does not reach TestRanger (even in one party), and neither can be healed by others.
- [ ] Leave the map (portal or logout) while fighting: the partner sees the duel end; **the leaver's own stake is back in its bags**
      after the map change or relog (deviation 3).
- [ ] A player with a debuff on cannot be asked: "impossible to battle as Debuff skill is on <p>." (5334).

## 24. Personal shops

The same two characters: **the first player account → TestRanger** and **the second player account → TestPriest** (Light, 10M gold each). A shop may only be set up
on a market map (`market_maps`, default 35, 36, 42): both use a **Bootleggery Recall Rune** (100/132, bag 2 slot 9, two each) to
reach map 42, where the seller pays 5 %; Apulune (map 35) works too, without the commission. Watch `logs/game-server.log` for
"personal shop opened", "sold from a personal shop" and "purchase refused".

- [ ] On map 42, TestRanger sits and opens the market setup: the setup window opens. Place the 10 Apples (25/1, bag 2 slot 0) at 100 each
      and a bound or quest item: the Apples show in the window, the bound/quest item does not (nothing happens).
- [ ] Enter a title, open: "Individual market has been opened"; **the sign with the title shows over TestRanger** on both clients.
- [ ] TestPriest walks up (or relogs nearby): the sign is there too (sent with the looks).
- [ ] TestPriest views the market: the visit window lists the Apples with price 100 and count 10.
- [ ] TestPriest buys 4: "Purchased … with 400G." on TestPriest, the Apples in its first empty slot, gold 400 lower. On TestRanger:
      "… is sold with 400G." and "5% of item cost has been paid for commission." (5357); gold **380** higher; the Apple stack in its bag
      shows 6. Both windows show 6 left. Relog both: the counts and gold are the same (one transaction).
- [ ] TestPriest buys the remaining 6: TestRanger's shop slot and bag slot empty, TestPriest's window shows the slot empty.
- [ ] TestPriest tries to buy more than is left (or with too little gold, e.g. after dropping gold below the price): "Can't purchase the
      item." / "Insufficient gold".
- [ ] TestRanger closes the market (back to setup): "Individual market has been closed.", the sign goes, TestPriest's window closes with
      "Seller closed the market."
- [ ] Reopen; TestRanger stands up (or is attacked, or moves an item, or uses an item): the shop ends, the sign goes, TestPriest's window
      closes.
- [ ] While TestRanger's shop is open, TestPriest's party invitation to TestRanger fails ("cannot be invited", code 1), and the other way
      round.
- [ ] A trade request from either ends the shop first.
- [ ] TestPriest visits, then leaves the map (recall rune, portal) and comes back: it can visit TestRanger's shop again (deviation 4).

## 25. Guild wars

The same two characters, each now the master of its own guild (database): **TestRanger** of **Rangers** (guild 1) and
**TestPriest** of **Priests** (guild 2), both Light, on map 1 at about (1001, 1616). Both guilds are Light: outside a war they cannot hurt
each other, so every hit below shows the war at work. Stay within 80 of each other when the war is accepted (deviation 1). Watch
`logs/game-server.log` for "guild war accepted" and "guild war over". The request is the client's GvG request on the targeted player.

- [ ] Before any war: TestRanger cannot attack TestPriest (same country).
- [ ] TestRanger asks TestPriest: TestPriest's dialog "<p> requested the battel to <t>." (5340). No answer for 10 s: TestRanger "GvG is
      canceled as the opponents didn't respond." (5345); TestPriest's dialog closes without a message.
- [ ] Ask again, TestPriest refuses: TestRanger "<p> rejected GvG." (5341).
- [ ] Ask again, TestPriest accepts: on both, the 6-second countdown (5308, `bgm_vs.wav`), then "Start GvG!!" (5316) about 5 s after the
      answer. During the countdown they are already enemies on the server.
- [ ] Now TestRanger attacks TestPriest with normal blows and skills; TestPriest can hit back.
- [ ] A duel request between them is ignored while the war lasts.
- [ ] TestRanger walks more than 40 from the point between where the two stood: "out of allowed battle area." (5332); TestRanger is
      Rangers' officer, so **Rangers lose**: TestPriest "Priests guild won." (5317 with its own guild), TestRanger "Priests guild won."
      (the enemy's name: both guilds are Light, so the name is in the client's list).
- [ ] New war; TestRanger kills TestPriest: TestPriest "have been defeated at the battle" (5337), TestRanger "<p> has been defeated at
      the battle." (5338), then "Rangers guild won." on both. **No experience lost** at TestPriest's rebirth, no kill or death count
      (`SELECT kills, deaths FROM characters WHERE id IN (3, 5)` unchanged), no item or gold dropped.
- [ ] New war; one of them logs out (or changes map) during the fight: the other sees "<p> disconnected." (logout, deviation 5) or
      "<p> is out of allowed battle area." (map change), then its own guild's win.
- [ ] In one party during a war, TestPriest's heals and buffs do not reach TestRanger.
- [ ] Asking ProbeHero or another player without a guild: "can't request GvG to <p>." (5343).

## 26. Lapis skills on hits (item option skills)

Given to TestRanger (3) and TestPriest (5) in bag 1 (database): a second lapis-room weapon with two lapis already linked,
a stun crystal (30/151, skill 205: lands 8 % of hits, 3 s stun) and a Sleep Lapis (30/110, skill 1: sleep 30 s, a magic roll),
TestRanger's Dread Unicorn Blade (9/44) in slot 8, TestPriest's Dread Sapphire Rose (12/54) in slot 7; and an upper armour with the
upper-armour sleep lapis (30/111, skill 1, also blocks sleep on the wearer), TestRanger's Dread Terra Armor (17/99) in slot 7, TestPriest's
Dread Tathar Armor (17/184) in slot 6. Equip both. The skills show with level 100; the client may name the lapis "???".

- [ ] Hit a monster with normal blows: after a hit (not a miss, not the killing blow), often a sleep (skill 1) and now and then a stun
      (205) on it, shown on the monster as when a skill lands (`0x0517`). A sleeping monster wakes when hit again.
- [ ] Use an attack skill on a monster: the lapis skills follow the skill's hit the same way, before the SP/MP cost shows.
- [ ] In a duel or a guild war between them (sections 23, 25): each blow that lands puts the attacker's weapon lapis on the target and
      the target's armour sleep back on the attacker; when that lands, the attacker's armour sleep back on the target once more, no further. Works
      without a shield or helmet (deviation 1).
- [ ] `logs/game-server.log` shows no errors; nothing loops (one set of lapis skills per hit).

## 27. Obelisks

`docs/original/obelisks.md`. Obelisks stand on map 18 at (560, 540) (neutral, level 16, ~10,000 HP, eight guards of level 10 around it),
map 30 at (900, 180) (neutral, level 32) and map 0 (seven: 1 to 5 neutral, 6 light at (622, 1721), 7 fury at (1759, 798), with many
level 56 to 58 guards). The two clients of section 22 (TestRanger, Light; TestVail, Fury). Watch `logs/game-server.log` for "obelisk
fell", "obelisks reset" and "kill counts".

**Log review:** "obelisks reset" appeared at 05:55:13 when Fury reached the top count (section 22), and `obelisk_owners` shows
every obelisk back at its base country afterwards (map 0: five neutral, number 6 Light, number 7 Fury; maps 18 and 30 neutral). Not seen:
the 10 minutes with nothing standing, the base countries' obelisks coming back, any obelisk being destroyed.

- [ ] Enter map 18: the obelisk shows on the minimap (`0x2101` after the weather); the obelisk and its guards stand there, they do not walk.
- [ ] TestRanger kills it: its guards vanish at once; Light players see "Destoyed …" (message 463, the client's spelling), Fury
      players nothing; the minimap mark changes for everyone on the map (`0x2102`). Every Light player gets the new kill count (`0x020F`,
      + 1366) and its grade's bonuses.
- [ ] About 3 min 10 s later Light's obelisk rises in its place with Light's guards. TestRanger cannot attack it (own faction).
- [ ] TestVail kills it: Fury + 1366, Light − 1366 (its grade goes down as soon as the count is below its grade's kills, deviation 2);
      Fury's obelisk and guards rise 3 min later. Light players see "… is destroyed!" (462), Fury "Destoyed …" (463).
- [ ] Kill a guard while its obelisk stands: it comes back after its respawn time (3 min after its body went), one at a time.
- [ ] Restart the server: the obelisk rises as its last owner's (`SELECT * FROM obelisk_owners`).
- [ ] A faction reaching the top grade (captures or Goddess Blessings, section 22): every obelisk and its guards vanish, the minimap
      marks go back to the base country, and nothing rises for 10 minutes; then the base countries' obelisks come back.
- [ ] `logs/game-server.log` shows no errors.

## 28. Union parties

`docs/original/union.md`. Two Light characters on the two clients (TestRanger and TestPriest); a third player, if there is one, for
`/join`. Watch `logs/game-server.log` for "union set up", "joined a union", "union leader", "union dismantled".

- [ ] TestRanger invites TestPriest to a party, then "Create Union" (loot rule "leader", auto-join on): both see "Union Party
      created." (6532) and the union window; TestPriest is sub-leader (with 2 members, the original's pick).
- [ ] The union window shows HP/SP/MP, buffs, deaths and positions of the other member (the union's `0x0C0A`–`0x0C13`); level up:
      the other sees it.
- [ ] Kill monsters near each other: experience is lower than in a party of 2 (each gets 73 %, the union table, no ×1.2); gold is
      shared; gear (ReqDex not 0) goes straight into the leader's bags with "<p> attained <i>" for the other (`0x0C14`); potions and
      lapis (ReqDex 0) fall on the ground as the killer's.
- [ ] "Config Union" with "random" (the client sends `0x0B0F`): the loot-rule message (6517) for both; the sub-leader and the
      groups stay; loot now goes to a random member near.
- [ ] Move TestPriest to another group ("Move Group", drop on a slot of group 2): both windows show it there. TestPriest's party
      buffs and its party heal still reach TestRanger (deviation 6); a party heal without the caster does not.
- [ ] A third Light player types `/join TestRanger`: it joins, sees "Joined the party" (207) and the union window with everyone
      (its own `0x0B0C`, deviation 9); its `/p` chat reaches the union. With auto-join off: "Leader don't select auto-join mode."
      (6536).
- [ ] "Delegate sub-leader" and "Delegate leader": the messages 6514/6515 and 307/308; the party marks over the heads change.
- [ ] Union invitation by the menu and by `/party <name>`: the dialog, yes joins, no shows nothing.
- [ ] Kick (union "Kick"): "Party leader kicked you." (6533) for the kicked, "<p> kicked from party." (6534) for the others; with
      only two, the union ends for both.
- [ ] "Leave Union" and "Dismantle Union": "Left the party for good" (305) / "Union Party dismantled." (6531); nobody is left in a
      party afterwards.
- [ ] Union chat: the leader or sub-leader types `/raid hello` (or `/alert`): every member sees "[Union] hello" (deviation 7), in the
      chat window and pink in the centre of the screen. A plain member's `/raid` shows nothing. Party chat (`/p`) reaches every union member.
- [ ] `logs/game-server.log` shows no errors.

## 29. Scripted monsters

`docs/original/scripted-mobs.md`. The scripted monsters live on maps 45, 46 and 47 (levels 54 to 58: use a strong character, or expect
to die). Light reaches map 47 by the portal at about (223, 1873) on map 0 (arriving at (120, 453)), map 45 by the one at (187, 1833);
Fury by (1921, 332) and (1941, 367). Or put the character on map 47 at (636, 586) in the database while the game server is stopped.
The start-up log line "monster AIs loaded" should say `machines=27 lines=31`.

**Log review:** No scripted-monster line exists to look for. TestRanger was killed by a map 47 monster (id 7592) at 02:13:27;
the log doesn't say which monster it was.

**Tested (the maintainer's report; the logs match the sessions):** Passed but the first item: the crocodiles of `AI` 11 "move, pause, move, pause" on the way to you. That is the original's chase: a step, then `dwChaseTime` of waiting (400 ms to 1 s and more), with the monsters updated every 500 ms (`fight::chase`, `docs/original/pathing.md`). Changed since (database): steps are now paced to the client, `docs/original/chase-smoothing.md`; retest it in section 42.

- [ ] Map 47 around (636, 586): the red and green crocodiles (`AI` 11) stand still until you come within 24; then the nearest one
      attacks (its script runs once a second). At half HP it gets stronger (its blows hurt about 1.5 times as much) and calls the monsters within 10 to
      you.
- [ ] Map 47 around (746, 1529): the Helltooth fighters (`AI` 13). At 15 % HP one doubles its melee defence (your blows do much less).
- [ ] Map 45 around (316, 837): the named scorpions (`AI` 17). Starting to fight it says "Smite the heretics!" or "Li laka Amon-Ra".
      At 25 % HP one casts, then may run off about 23 in a random direction (deviation 3; only when that leaves it less than 20 from
      where it spawned, so often it stays) and casts again.
- [ ] Map 45 around (305, 850): the scorpion guards (`AI` 16). At 20 % HP one runs to a monster nearby that is not fighting, and the
      monsters within 7 of it attack you.
- [ ] Map 45 around (243, 881): the female scorpions (`AI` 19). At 10 % HP one runs about at random, the monsters within 10 attack you,
      and once it is 20 from where it started running it fights again.
- [ ] Run more than 45 away from a scripted monster: it goes home and is back to full HP; attack it again: its script starts over (a
      named one says a line again).
- [ ] With two players on one named monster, let the second do much more damage: the monster turns to the second and keeps its
      script (it still casts at its HP marks; deviation 4). Kill one of the two players with it: it goes on against the other without
      saying its opening line again (deviation 6).
- [ ] A named scorpion (`AI` 17) at 50 % HP: for about 5 s every blow and skill misses (deviation 5), then hits land again.
- [ ] `logs/game-server.log` shows no errors.

## 30. Monster HP bars and a second login

`docs/original/client-extras.md`. Any character for the bars; for the second login, the account the first player account on both clients (the
second one logs in while the first plays). Watch `logs/game-server.log` for "the account logged in again: its connection here ends"
and "listening for logins" at start-up.

**Log review:** Ticked: the second login. TestRanger's connection was ended by the same account logging in ("the account logged
in again: its connection here ends" 02:14:27), its character stayed 10 s ("linger=10s", left at 02:14:37), and the new connection reached
the character screen at 02:14:37, entered at 02:14:49, with a single login in the login log (no second try). A third player seeing the
character stay, the character-screen case and the pending-logout case were not tried. "listening for logins" appears at every start-up.

- [x] B picks the server right away: it waits a few seconds (until A's character is saved) and then shows the character screen, with
      no error and no second try (deviation 3).
- [ ] `logs/game-server.log` shows no errors.

## 31. Party instance zones and the Lua bosses (Cailum Sacra)

`docs/original/instances.md`. TestRanger (the first player account) and TestPriest (the second player account), both Light, level 30, start on map 47 next to
portal 6 (about (1145, 101, 1080)); put them in one party first. Watch `logs/game-server.log` for "instance zone made",
"instance zone released" and any "Lua error".

**Log review:** No trace of the instance: no portal into map 65, no "instance zone made". TestRanger logged in on map 47 (10-19
01:25 and 02:12) and was killed by monster 7592 at the arrival spot (02:13:27), reviving at (125.6, 9.3, 454.1) on map 47.

- [ ] TestRanger alone (no party) steps into portal 6: "only party members allowed" (8042), no move.
- [ ] In a party: portal 6 takes each of them to map 65 (313, 29, 341), and both are in the same zone (they see each other); the
      floor's monsters (2460 to 2463) are there. The log shows "instance zone made" once.
- [ ] Walk to the summoner 2471 at about (221, 29, 180) and come within 6: "Do you think you can leave this place while you still
      breathe?" in chat and pink in the middle of the screen. Hit it: every blow misses.
- [ ] A second or two later: "Guards! Kill them!", two of the four floor bosses (2464 to 2467) appear at (220, 180), and 2471 walks
      to about (173, 179) and vanishes there (no death animation, no experience).
- [ ] "Section 4" (portal 1, about (342, 22, 149)): nothing happens while no floor boss has died (the client asks every 15 s). The
      floor bosses have 197,874 HP; if killing one is out of reach at level 30, skip to the next items. After one dies, portal 1
      takes you to (87.5, 33, 180) on the same floor (deviation 4: the portal may open up to a second after the death).
- [ ] Portal 2 (about (95, 32, 184)) takes you back to the floor's start (221, 31, 180); portal 0 (about (132, 37, 3)) to map 67.
- [ ] Leave the party while on map 65: you stay. Portal 2 then answers "only party members allowed".
- [ ] Log out on map 65 and back in: you start on map 47 at about (125.6, 10, 454.1).
- [ ] Die on map 65 and come back to life: you are on map 47 at that same spot.
- [ ] Both leave map 65: about 5.5 minutes later the log shows "instance zone released". Go in again: 2471 stands at its place again
      and "Section 4" is closed again. Coming back before the 5.5 minutes keeps the old zone, and leaving again gives it a fresh 5.5
      minutes (deviation 1).
- [ ] Map 64 (map 47 portal 2, about (535, -9, 1526), Light, levels up to 60): outside Monday to Saturday 21:00–22:00 (server time)
      "Now party instance zone closed." (6600); inside those hours with a party of two, "Can't enter because of lack of party members."
      (6601; it needs 18).
- [ ] `logs/game-server.log` shows no errors.

## 32. Monster paths (A*)

`docs/original/pathing.md`. Any character above level 7 on map 1.

- [ ] Map 1, the dwarf bandits (2023, level 7) around (497, 1137): hit one, then run to about (502, 1128), where the ground between is
      blocked. The monster comes round in short straight runs and reaches you (before, it gave up and walked off). Also try the
      Kloth dwarf warriors (118, level 12) around (47, 1802), standing at about (43, 1812).
- [ ] Hit a monster in the open and run off sideways: it runs to where you were when it set out, then turns towards you, so it
      zig-zags after you rather than bending smoothly (the original's way; before, it aimed at you every step).
- [ ] Hit one, then stand where it cannot reach you at all (inside a ring of rocks, on a ledge): it stops and goes back to idling,
      no lag on the server (`monster steps in the last minute` keeps coming every minute).
- [ ] Lure a monster round a corner, then run more than 45 from its home: with its straight line home blocked, it walks back the way
      it came.
- [ ] Map 65 (section 31): the summoner 2471 still walks to about (173, 179) and vanishes; the floor bosses chase as before, a
      little quicker on long runs (their full steps wait 400 ms less).
- [ ] `logs/game-server.log` shows no errors.

## 33. Walking NPCs and guards

`docs/original/guards.md`. Guards fight only the other faction. Set up in the database (server stopped): **TestVail**
(the GM account, Fury) stands on map 1 at (1205, 12.5, 1617), 30 west of three Light guards at about (1235, 1618), (1247, 1618), (1253, 1613),
just outside their reach. In bag 1 slot 2 TestVail has an upper armour (17/99) with an absorption lapis (30/240, absorption 20) for
the 1-HP item. Any Light character for the walking items.

**Log review:** Ticked: TestVail died to guards 11 times on 10-19 (guards 327, 332, 378, all `killer_kind=3`; four between
02:00 and 02:20, seven between 05:51 and 06:11). After the 0x0E05 fix every death in the 05:50 to 06:26 session was followed by a normal
revive 3 to 5 s later (7 times), unlike the frozen client before. The Stealth item is not decided: TestVail cast Stealth 63 level 2 (20 s)
at 02:19:49 and died at 02:20:03; the maintainer reported cancelling the buff by right-click and the guard coming at about the time it ended, so
the log can't say whether the guard ignored it. The absorption-armour, 5-second lock, run-away, body-left and walking-NPC items have no
trace in the log. No errors in the log for those sessions.

**Tested (the maintainer's report; the logs match the sessions):** Passed except two. The animal stops on click and moves on 1 to 2 s later without you moving: the server holds it while the click counts (the original's `m_nNumUsing`, released only by walking more than 10 away); what you see is the client finishing the walk step already sent. Kept as the original. The absorption armour (17/99, Dread Terra Armor, flags for both rogue classes, any faction): you got an Archer/Ranger tooltip; if the client would not let TestVail wear it, say what it said and another armour can be placed.

- [x] TestVail logs in where it stands: nothing happens. Walk east within 20 of a guard: it runs at you (16 every 2 s) and kills you
      with one blow, whatever your HP: you see the guard's blow, your HP goes to 0 and the death window opens; `logs/game-server.log`
      shows "died" with killer_kind 3. (Retest: the blow is now `0x0E05`, as the original; with `0x0E06` the client stayed
      at full HP and froze: no moving, no logout.)
- [ ] Put on the armour from bag 1 slot 2 (absorption 20), come back to full HP, walk in again: the first blow leaves 1 HP and shows
      an absorption message (20); the next one, half a second later, kills.
- [ ] Run away from a guard that is chasing you: more than 30 from its post it gives up and walks back. (A chasing guard keeps the
      height it started at, as in the original, so on slopes and bridges it may look sunk or floating. If one seems to jump, note
      where: the log now has a "guard run step" line for each of its moves.)
- [ ] (Guards killing monsters, `0x0E05` and `0x0606` kind 3: no shipped spot brings a monster within a guard's reach; covered by
      the unit tests only.)
- [x] `logs/game-server.log` shows no errors.

## 34. Guild houses (part 1)

`docs/original/guild-house.md`. Set up in the database (server stopped): **TestMage** (the first player account, Light) is the master of
the new guild **Builders**, ranking 5 with the house opening state already set (as a ranking would leave it), house not bought, no
Etin; TestMage has 60,000,000 gold, stands on map 35 at (380, 26, 285) next to the guild clerk (NPC 11/4 at (384, 26, 291)), and
carries 5 × [1000] Etin (bag 1 slot 9), 5 × [10] Etin (bag 2 slot 23) and 2 Guild House Teleport Stones (bag 1 slot 1). The house
portal is map 35 portal 2 at about (347, 26, 213). Watch `logs/game-server.log` for "guild house purchase", "Etin returned", "guild
NPC upgraded" and "guild house zone made".

**Log review:** Ticked: Etin return ("Etin returned guild=4 sum=5050 etin=5050" 15:47:15; `guilds.etin` is 5050). The house
purchase went through too ("guild house purchase price=50000000 money=10000000" 15:47:19, `buy_house` true) but the item also asks for the
second purchase to be refused (8029), which the log can't show. No "guild house zone made" and no portal into map 51 were logged, so
nothing from the house portal on (items 4 to 12) has been tried.

- [ ] Walk into portal 2 before buying: a guild-house message (reason 6, the house is not bought), no move.
- [ ] At the clerk, buy the guild house: "purchased guild house." and the gold drops by 50,000,000 (to 10,000,000). Buying again:
      "already" (8029).
- [x] Return Etin at the clerk: both Etin stacks leave the bags and "You return total 5050Etin." The guild window's Etin shows 5050.
- [ ] Portal 2 now takes you to map 51 (491, 42, 328), the guild house, with its NPCs (merchants, gatekeepers, the blacksmith, the
      warehouse, Isaac Contrium (11/5) at about (491, 51, 426)). The log shows "guild house zone made".
- [ ] Upgrade the gatekeeper to level 1 (Isaac's NPC upgrade window, if that is where it is): done; the Etin becomes 4050. Upgrade it
      to level 3 straight away: "upgrade is done one by one level." (8004). Log in again: the window still shows gatekeeper level 1.
- [ ] Upgrade the weapons merchant to level 1 (4050 − keep 2200 leaves 1850, it costs 100): done, Etin 3950.
- [ ] Buy from the level-0 weapons merchant (195): 95 % of the normal price (90 % after the weapons upgrade). The level-2 one (198):
      "level is not high enough" (8027 kind) and nothing bought. After the weapons upgrade, the level-1 one (196) sells.
- [ ] The potion merchant (203): 90 % of the price (the guild's ranking 5 is within 6).
- [ ] A gatekeeper of level 0 (36): its moves cost nothing. The level-2 one (38): "level is not high enough".
- [ ] The blacksmith (35): repairs cost 99 %; linking a lapis costs 90 % and extraction too.
- [ ] Log out in the house and back in: you start on map 53 (50, 7, 87). Use a Guild House Teleport Stone there: after 5 s you are
      back in the house (one stone gone).
- [ ] Everyone leaves the house: about 2.5 minutes later the log shows "instance zone released" for it; going in again makes a new
      one with the same levels.
- [ ] `logs/game-server.log` shows no errors.

## 35. Guild ranking battle (guild houses part 2)

`docs/original/guild-ranking.md`. The server runs with a test window: **Monday 11:00 to 12:00** (`ranking_times = "(1,11,1)"`) and the
ranking update **2 minutes** after the close (`ranking_update_delay_secs = 120`), both in `config/game.toml`; the log's first lines show
"the ranking battle's open times are set by ranking_times". To test at another time, edit the window in `config/game.toml` and restart
(`tools/game-server.sh restart`; `target/release/game-server config` shows what is in force; the notices come at hour−1:30, hour−1:50, the start, end−1:50 and end−1:59). **TestMage** (the first player account, Light) is the
master of **Builders**, ranking 5; the page below needs only him, a second Builders member shows the entry bonus (Rangers and Priests are
unranked guilds of one member each). Map 53's portal 0 (about (89, 115)) leads to map 50 (508, 3, 98), map 54's portal 1 for Fury. Watch
`logs/game-server.log` for "ranking battle open", "ranking notice", "ranking zone made", "ranking battle entry", "ranking battle points"
(debug), "ranking battle closed" and "ranking update".

**Log review:** The whole clock is in the log (times UTC, local = CDT, 5 hours earlier; the window is Monday 11:00 to 12:00
local): notices "kind=11" at 15:30 and 15:50 (10:30 and 10:50), "ranking battle open" and kind 100 at 16:00:00, kinds 101 and 102 at 16:50
and 16:59, "ranking battle closed moved=0" at 17:00:00, "ranking update placed=0" at 17:02:00. TestMage was online for the 15:50 and 16:00
notices only (online 15:45 to 16:37), so whether the client showed any message is unknown; nobody was online for the closing notices.
Ticked: the entry ("ranking zone made" 16:11:23, portal 0 from map 53 to zone 306, "ranking battle entry guild=4 character=4 entries=1"
16:11:34) and the unranked guild (Builders now has ranking 31, 0 points and `use_house` 0; it had ranking 5 and the house open before; its
Etin stayed 5050). Map 50's portals are level-gated in `world.map_portals`: portal 0 is levels 16 to 30, portal 1 levels 1 to 15 and
portal 2 levels 31 to 60, so level-30 TestMage could only use portal 0 ("portal use ignored index=1" at 16:11:49 is most likely that
gate, which the server drops silently; it also drops a use out of range); the item that says portals 0 to 2 would need three characters of different levels. TestMage died to a monster (16:15:05, two Magic
Ball casts broken by its hits), came back at map 53 (16:15:11) and the zone was released 2 min 31 s after he left (16:17:42). No kill and
no "ranking battle points" line, so the points, drop and two-member items are untried; no attempt at the "already participated" portal is
logged. A restart at 16:44 inside the window reopened the battle ("ranking battle open" at start-up); the restart-between-close-and-update
case was not tried.

- [x] After the start, portal 0: the question "You can only participate once in the zone. Do you want to participate?" (re-asked about every
      15 s while you stand in the portal). No: nothing happens. Yes: you arrive on map 50, in Builders' own zone (log "ranking zone made",
      then "ranking battle entry").
- [ ] A player without a guild at the portal: the "need a guild" message (8038), no question.
- [ ] On map 50 the portals 0 to 2 move you inside the zone (no question); the monsters are there.
- [ ] Kill map-50 monsters: no gold on the ground; items drop as usual; the debug log shows the guild's points (2 or 3 a kill in the low
      areas, 1 in the high ones). Nothing visible is expected from the points packet (the client only stores it).
- [ ] Die, or log out and in again, on map 50: you are back at map 53. The portal now says "already participated in Ranking battle."
      (8040) and no question.
- [ ] Two members of Builders: both can enter (once each); after the update the guild's score is higher than its kill points alone.
- [ ] At 11:50 "will be finished in 10 minutes…", at 11:59 "After 1 minute…"; just after 12:00 everyone left inside is moved to map 53
      (log "ranking battle closed", moved = how many).
- [ ] Two minutes after the close: "Each guild's gained point has been settled so guild ranking is renewed…"; the guild window shows
      Builders' new ranking (1 if it is the only guild with points) and points (open the guild list to see them; log in again if it
      does not show). Guilds without points are unranked (31).
- [ ] With a bought house and Etin above the keep Etin (section 34): the keep Etin is taken (the guild window's Etin drops by 2000 plus
      the NPC upkeep) and the house stays open. Without enough Etin: the house is closed (portal 2 on map 35 says it is closed).
- [x] A guild without points (do not enter, or kill nothing): after the update it is unranked and its house closed (portal 2: reason 3,
      the ranking).
- [ ] Stop and start the server between the close and the update: the log says "a ranking update was left due" and the update runs at
      start-up.
- [x] `logs/game-server.log` shows no errors.

## 36. Guild bank (guild houses part 3)

`docs/original/guild-bank.md`. **TestMage** (the first player account, Light) is the master of **Builders** (ranking 5, house bought and open, Etin
enough for the warehouse upgrade), with in bag 1: slots 2 and 3 five Blood Berries each (25/3, sell 110, stack 10), slot 4 a Ventus
Alear Guild Copperbar (94/5), slot 5 three bound items (44/42); the Green Apple is in slot 0. **TestPriest** (the second player account) is a member of
Builders at **rank 9**, also on map 53. **TestRanger**'s guild Rangers has one Apple in its bank, slot 0. Map 53's portal 2 (about (50,
12, 128)) leads into the Light house (map 51, (492, 42, 325)); the house's warehouse keeper stands at about (436, 43, 363). The bank
works only there (a house map with the keeper in view). Watch `logs/game-server.log` for "guild bank move", "guild bank move ignored"
(debug) and "disband refused".

**Log review:** Nothing run: no "guild bank move" line. The database has TestPriest in Builders at rank 4, not the rank 9 the
intro above says (`guild_members`); with rank 4 the six promotions in the item below come out as five, so check it before starting.
`guild_bank_items` holds Rangers' one apple.

- [ ] TestMage at the house's warehouse keeper: its menu opens a window. Note what it shows (the guild warehouse is expected: an empty
      bank and your bags); nothing is in it yet.
- [ ] Drag the first Blood Berries (bag 1 slot 2) into the bank's first slot and the second into the next: both move, no gold taken.
- [ ] In the bank, drag the second stack onto the first: one stack of 10 is left.
- [ ] Drag the bound food (bag 1 slot 5) into the bank: nothing happens.
- [ ] Put the guild copperbar in and take it out again: no gold taken either way.
- [ ] Take the 10 Blood Berries out: 55 gold taken (5 % of 10 × 110).
- [ ] If the window shows slots past the 40th: dropping into one says "level is not high enough to have service upgraded." Upgrade the
      warehouse to level 1 (the NPC upgrade window, 200 Etin): slots up to the 80th take items.
- [ ] Put an item back in, log out and in again: the bank still shows it.
- [ ] TestPriest (the second player account, rank 9) comes in (map 53 portal 2) with TestMage still online: at the keeper the window shows the bank's
      items; dragging one of its own items in says "Not authorized".
- [ ] TestMage promotes TestPriest to rank 3 (six promotions in the member list): TestPriest's item now goes in, and TestMage's open
      window shows it at once. TestPriest taking an item out, dropping onto a taken bank slot, or moving inside the bank: "Not
      authorized".
- [ ] TestMage takes TestPriest's item out: it leaves TestPriest's window too. TestMage moves an item inside the bank: TestPriest's
      window follows.
- [ ] TestMage drags a bank item onto the bound food in the bags: nothing happens. Onto the Green Apple (bag 1 slot 0): they swap (the
      apple is in the bank), and the fee for the item that came out is taken.
- [ ] TestRanger (the first player account) disbands Rangers (whose bank holds an apple): "Failed to disband guild." and the guild stays.
- [ ] `logs/game-server.log` shows no errors.

## 37. Operator (GM) commands, part 1

`docs/original/gm-commands.md` (sections 4 to 9 and deviations 1 to 18). **TestVail** (the GM account, Fury) is made a staff member of level 1
for this section; **TestRanger** (the first player account, Light) is the player it works on. Watch `logs/game-server.log` for "gm command" lines
(one per command, with the target and the values), "gm command refused" (debug) and "account blocked by a GM".

**Setup** (server stopped; check the ids first: the GM account is account 4 with TestVail 6; the first player account 5 with TestRanger 3 and TestMage 4;
the second player account 6 with TestPriest 5):

- `UPDATE accounts SET status = 16 WHERE username = '<GM account>'` (staff level 1). **After this section set it back to 0**, or section 33
  (guards) sees TestVail as staff (invisible and untouchable).
- TestVail stays on map 1 at (1205, 12.5, 1617), near the Light guards; TestRanger (the first player account) moved there too, a few steps away (`map 1`,
  `pos_x 1210, pos_y 12.5, pos_z 1617`), so both see each other. TestRanger needs a free slot in bag 1 and one Apple (25/1, item 25001).
- Note TestRanger's level, money and STR before starting (`SELECT level, money, str, exp FROM characters WHERE id = 3`). If section 31 is
  still open, move TestRanger back to map 47 (138.6, 8.6, 459.6) afterwards.

Commands are typed in TestVail's chat box.

**Log review:** Nothing run: no "gm command" line in the log, `character_events` and `closed_quests` are empty. the GM account has
status 16 now (the setup); set it back to 0 after sections 37 and 38.

**Tested (the maintainer's report; the logs match the sessions):** Passed: 15 of 20. **Invisible on login / `/char`**: the server does what the original does (shape 13 to the view, untouchable for guards, monsters and players) and the guards left it alone, but this client does not hide a shape-13 player on its own; the same shape is Stealth's, so check the Stealth item of section 4 too. Open. **`/warning`, `/eventadd`, the notices**: found with the new "gm packet" log and the client's code: this client only takes a text argument inside double quotes (`0x46d5e0`), so `/warning TestVail hi` sends an empty text (the log showed length 1) and the server correctly refuses it; type `/warning name "text"`. The items are reworded with the quotes. **`/server off`**: the client compares only 9 characters of "/server on", so it sends `F706` for both words; `F706` now flips the switch (deviation 19). **`/apower` then `/apowercan`**: fixed: going back to a player now switches `/char` and `/attack` back to a player's (visible, attackable). Ideas, not built: players whispering a GM across the factions, and a GM seeing both sides' chat.

- [ ] TestVail logs in: TestRanger does not see it (invisible); the guards next to it do not attack it.
- [ ] `/char off`: TestRanger sees TestVail now. `/char on`: it disappears again for TestRanger.
- [ ] `/recallitem TestRanger <Apple's item name>`: the Apple leaves TestRanger's bag.
- [ ] `/warning TestRanger "behave"` (text in double quotes): TestRanger sees "[Warning] behave". `/watch TestRanger`: TestVail sees TestRanger's map and position.
- [ ] `/stopon 1 TestRanger`: TestRanger cannot move; it can after a minute (or `/stopoff TestRanger`).
- [ ] `/amove TestRanger`: TestVail stands at TestRanger.
- [ ] `/guildrank Builders 5`, then `/gmove 492 325 51 Builders`: TestVail enters the Builders house (map 51; the Light house, so check
      `buy_house` and `use_house` for Builders first: `use_house` becomes 1 by the command). `/guildpoint Builders 100`: confirmation; the
      `guilds` row shows 100 points.
- [ ] `/server off` (with fixes.dll the client sends `F707`; `F706` no longer flips, `docs/original/small-items.md` section 4), then the second player account tries to log in to the game: refused; `/server on`: it can (TestPriest, for the next two).
- [ ] `/apower 2 TestPriest` (TestPriest online): the `accounts.status` of the second player account becomes 32; TestPriest's client says it got level 2.
      `/apowercan TestPriest`: status 0 again. (Revert by SQL if anything is left.)
- [ ] `/aban 2 <second player account>` with TestPriest online: TestPriest's client shows "Ban connecting for 2 minutes by operator" and is
      disconnected; the second player account's login is refused ("account cannot log into the game"); `accounts.status` is 2 with `blocked_until` set.
      `/abancan <second player account>`: status 0, it can log in. (Or wait the two minutes: the login lifts it.)
- [ ] `/pvpoff` with `/char off` and `/attack off`: TestRanger cannot hit TestVail, the guards still attack it. `/pvpon`: TestRanger's hits
      land.
- [ ] `/questoff <a quest TestRanger can take>`: TestRanger cannot start it at its NPC (the log shows "closed by a GM"); restart the server:
      still closed (`closed_quests` has the row); `/queston <quest>`: TestRanger can start it.
- [ ] `/eventadd TestRanger "winners"`: confirmation; `character_events` has the row. Again: the failure message. `/eventrem TestRanger
      "winners"`: removed. Again: the failure message.
- [ ] `logs/game-server.log` shows no errors. Then `UPDATE accounts SET status = 0 WHERE username = '<GM account>'` (server stopped or not:
      it counts at the next login).

## 38. Operator (GM) commands, part 2

`docs/original/gm-commands-2.md` (sections 3 to 5 and deviations 1 to 15): the players' questions to the staff (`/request GM`,
`/UQGET`, `/UQCOUNT`), the player commands `/cure`, `/event on|off`, `/iclear`, `/eclear`, `/kick`, `/questadd`, `/questrem`, the
character sheet `/quiry`, the summons `/asummon`, `/bsummon`, `/partysummon`, and the notices `/gmnotice`, `/cnotice`, `/wnotice`,
`/znotice`, `/bnotice`, `/notice`. **TestVail** (the GM account, Fury) is staff level 1 as in section 37; **TestRanger** (the first player account) and
**TestPriest** (the second player account) are the players. Watch `logs/game-server.log` for "gm command" lines, "user question" and "summoned by a GM".

**Setup** (server stopped; ids on the dev box: the GM account is account 4 with TestVail 6; the first player account 5 with TestRanger 3; the second player account 6 with
TestPriest 5 and sefewfe 2). Already done (the server runs it), repeat if the characters moved:

- `accounts.status` of the GM account stays **16** (staff level 1; section 37 set it). **After sections 37 and 38 set it back to 0**, or section 33
  (guards) sees TestVail as staff.
- TestVail on map 1 at (1205, 12.5, 1617), TestRanger at (1210, 12.5, 1617) (both already there for section 37).
- TestPriest on map 1 at (1260, 12.5, 1640): `UPDATE characters SET map = 1, pos_x = 1260, pos_y = 12.5, pos_z = 1640 WHERE id = 5`. If
  section 36 is still open, put it back afterwards (`map 53, 50.4, 8.0, 86.6`).
- sefewfe's items are destroyed by the clearing commands: they are backed up in the table `s38_sefewfe_items`. Restore them afterwards
  (server stopped): `DELETE FROM character_items WHERE character_id = 2; INSERT INTO character_items SELECT * FROM s38_sefewfe_items;
  DROP TABLE s38_sefewfe_items;`.
- Quest 50 (선물(1), start item 27/29) is the test quest: TestRanger has no row for it (`SELECT * FROM character_quests WHERE
  character_id = 3`).

Commands are typed in TestVail's chat box.

**Log review:** Nothing run: no "gm command" or "user question" line, `user_questions` is empty. The setup is applied (the GM account
has status 16; `s38_sefewfe_items` holds sefewfe's two items and sefewfe still has them, so nothing was cleared).

**Tested (the maintainer's report; the logs match the sessions):** Passed: 7 of 16 (4 can't test). **`/request GM` / `/UQGET`**: the question window opens; the stored text is plain ASCII (`where the  are you` + a zero, in `user_questions`) and goes out as the notes say, so the garbling is the client's font or code page (your DLL idea). **`/event on|off` and the four notices**: the same quoting: the log showed `F80D` with an empty name; type `/event on "TestRanger"`, `/gmnotice "text"`, `/notice 0 "text"`, and so on. Items reworded. **`/kick`**: the server sends `F804` before closing (deviation 6 of part 2), but the client showed its plain disconnect box; if it keeps doing so the text is not reachable on this client, left as it is.

- [ ] TestRanger: `/request GM where is the blacksmith`: "Successfully applied". TestVail: `/UQCOUNT` shows 1; `/UQGET` opens the question
      window with TestRanger's text; `/UQGET` again shows the same question.
- [ ] TestVail answers with the fourth button, naming TestRanger: TestRanger gets a whisper from TestVail with the answer; `/UQCOUNT` shows 0;
      the `user_questions` row has the answer (kind 4, `answer_to` TestRanger).
- [ ] TestRanger asks four questions: `/UQCOUNT` shows 3. TestVail takes one with `/UQGET`, closes it with one of the first three buttons
      (note which label): TestRanger gets nothing; the row has kind 1, 2 or 3.
- [ ] TestVail takes another with `/UQGET` and relogs: `/UQGET` gives the same question again. Restart the server: the two still open
      questions are there (`/UQCOUNT` 2).
- [ ] `/event on "TestRanger"`: TestRanger's HP fills; TestVail gets a whisper from "<GM Cmd>": "cure ok ->TestRanger". With a buff on
      TestRanger (a skill or a buff item), `/event off TestRanger`: the buff ends (an item buff that survives death stays); the whisper says
      "cure, buff,debuff clear ok ->TestRanger".
- [ ] TestRanger invites TestPriest to a party; TestPriest walks off. `/partysummon TestRanger 1230 1630 1`: both arrive there.
- [ ] `/gmnotice "staff only"`: TestVail sees "[Notice] staff only", TestRanger does not. `/notice 0 "hello everyone"`: both see it.
- [ ] `/cnotice 10 "near you"`: TestRanger (next to TestVail) sees it now and again every 10 seconds; `/cnotice 0 "stop"`: it shows once more and
      stops.
- [ ] `/wnotice TestRanger 5 "hello"`: TestRanger sees it every 5 seconds wherever it walks; `/wnotice TestRanger 0 "bye"` stops it.
      `/znotice 0 "map notice"`: everyone on map 1 sees it.
- [ ] `/znotice 15 "repeat"`, then TestVail logs out: the repeats stop.
- [ ] `/bnotice` needs the Korean faction word (빛 or 어둠): mark "can't test" unless the client takes Korean input.
- [ ] Log TestPriest out and sefewfe in (map 1, 647, 1745; walk or `/asummon sefewfe`). `/iclear sefewfe`: its bags are empty ("…" 337);
      after a relog still empty. `/eclear sefewfe`: its equipment is gone, its looks change for TestVail, its HP maximum drops. (Restore its
      items afterwards, setup above.)
- [ ] Staff protection: with everyone logged out, `UPDATE accounts SET status = 48 WHERE username = '<GM account>'` (level 3) and `UPDATE accounts
      SET status = 16 WHERE username = 'the second player account'` (level 1); log TestVail and TestPriest in: `/kick TestPriest` and `/iclear TestPriest`
      fail. Then set the second player account back to 0 and the GM account to 16.
- [ ] `/akick` is a level 2 command: with the GM account at status 32 (level 2) and the second player account at 16 (level 1), both logged in, `/akick
      the second player account` gives the failure message; with the second player account at 0, TestPriest is told "Log out by operator" and disconnected. (Then set
      the second player account to 0 and the GM account to 16.)
- [ ] `logs/game-server.log` shows no errors. Then restore sefewfe's items and, after sections 37 and 38, `UPDATE accounts SET status = 0
      WHERE username = 'the GM account'`.

## 39. Monster drops part 2: quest items and sequence items

`docs/original/quest-drops.md`: monsters now carry quest items (types 27 to 29), a quest item drops only for a killer (or a party member in the
killer's zone) who has one of its quests in progress, it always lies on the ground as the killer's, and only a player with the quest (or for a
quest-starting item, anyone) can pick it up; every 16,001st monster made (and rarer ids) also drops a sequence item; the monsters' id counter is
kept across restarts. Players: **ProbeHero** (the first player account, human, level 10) and **sefewfe** (the second player account, elf, level 10), both Light, placed on map 1
next to the merchant NPC 1/6 at (559, 1815); neither has quest 9. Watch `logs/game-server.log`.

Quest **9** (케이의 방어구, level 3 to 60, Light, no previous quest) is taken from NPC 1/6 and ended with item 27/2 ×5 (a reward choice of two
items). Its monster is **2015** (level 4, quality 60 for 27/2), spawning on map 1 around (650 to 850, 1440 to 1500), e.g. (785, 1465).

**Log review:** Ticked: "monster id counter loaded" said mob_id=1 at the first start with the feature (10-20 00:04:59 UTC) and
mob_id=19565 after the next restart (02:34:09). Nobody has taken quest 9 (no row in `character_quests`), so the rest is untried. ProbeHero
and sefewfe are still on map 1 at the merchant.

- [ ] Before taking the quest, ProbeHero kills 10 or more monsters 2015: item 27/2 never drops (other drops and gold may).
- [ ] ProbeHero takes quest 9 at NPC 1/6, then kills 2015s: a little more than half the kills leave a 27/2 on the ground (a roll of 60 %).
      ProbeHero picks up five of them (each works).
- [ ] ProbeHero hands quest 9 in with the five items: the quest ends, the items are taken, the reward choice works. After that, more 2015s
      drop no 27/2 any more (the quest is finished, not in progress).
- [ ] sefewfe takes quest 9, kills a few 2015s (27/2 drops), then gives the quest up in the quest window: further 2015s drop no 27/2
      (do this before the party items, which need sefewfe without the quest).
- [ ] Pick-up refusal: ProbeHero and sefewfe in a party, only ProbeHero with quest 9. ProbeHero kills a 2015 and the 27/2 drops: sefewfe
      tries to pick it up and gets "Unavailable quest item" and the item stays on the ground; ProbeHero then picks it up.
- [ ] Party kill: sefewfe (no quest) kills a 2015 with ProbeHero (quest 9 in progress) standing next to it: the 27/2 can drop, lands on the
      ground (not in anyone's bags), and ProbeHero can take it.
- [ ] Party member on another map: ProbeHero leaves map 1 (any portal) and stays in the party; sefewfe (no quest) kills 10 or more 2015s on
      map 1: no 27/2 drops (only members in the killer's zone count).
- [ ] A quest-starting item: monster **108** (level 10) around (950 to 1000, 1270 to 1340) on map 1 carries item 27/248 (a quest start
      item, 30 %): it drops for anyone with no quest at all, any player may pick it up. (Quest 821 itself needs level 11.)
- [x] After the next server restart the log's "monster id counter loaded" line shows a value above 1 (the counter is saved every minute and
      at shutdown), not 1 again.
- [ ] Sequence items cannot be forced (every 16,001st monster made): if the log ever shows a debug "sequence item" line (needs
      `RUST_LOG=debug`), the item lay on the ground and could be picked up. Mark "can't test" otherwise.
- [ ] `logs/game-server.log` shows no errors.

## 40. The auction house

`docs/original/auction.md`: the Auction House (map 42 only) with search and paging, registering (with the fee as a deposit), cancelling,
buying now, bids at 5 % steps, collecting items and gold (14 days to collect), the end of an auction (checked every minute), the interest
list, and the GM's `/auctionsearch` and `/auctionrecall`. Deviations: gold past its 14 days can't be collected and old rows are deleted (1),
the name search ignores case (2), a recall gives the seller its deposit back (3). Watch `logs/game-server.log` for "market register",
"market bid", "market buy", "market cancel", "market collect", "market end", "market purge" and "market recall" lines.

**Setup** (server stopped; after section 39, which needs these two on map 1): **ProbeHero** (the first player account, id 1) sells and **sefewfe**
(the second player account, id 2) buys, both Light; **TestVail** (the GM account, staff level 1, Fury) is the GM.

- `UPDATE characters SET map = 42, pos_x = 51, pos_y = 1, pos_z = 50 WHERE id IN (1, 2, 6)` (the Auction House).
- A bound item for ProbeHero: `INSERT INTO character_items (character_id, bag, slot, item_id, type, type_id, count) VALUES (1, 1, 6, 100090,
  100, 90, 1)` (a Stat Reset Stone, `reqog` 1). ProbeHero carries 10 Hubbin Fruit (bag 1 slot 2) and 10 Fireshroom (slot 3) for the stacks.
- Note both characters' gold before starting (ProbeHero 812,419, sefewfe 1,000,000).

**Log review:** Nothing yet. The setup is not applied (ProbeHero, sefewfe and TestVail are on map 1, `market_auctions` is
empty). Start-up shows "auction house loaded auctions=0 item_rows=0 gold_rows=0".

- [ ] Outside map 42 nothing answers: before the setup (or after moving away), opening the auction window, if the client allows it,
      gives no results.
- [ ] ProbeHero registers 3 of the 10 Hubbin Fruit for 24 h at minimum 1,000, buy-now 5,000: message 7116, the gold drops by **120**
      (24 × 1 × 5), the stack shows 7, and the auction is in "my auctions".
- [ ] Registering the Stat Reset Stone (bound) is refused (7117); so is a minimum of 400 (7117).
- [ ] sefewfe searches: the fruit is found by part of its name, also typed in another letter case ("hubbin"), by its category, and by
      "search this item" on the row.
- [ ] Paging: with more than 17 matching auctions (ProbeHero registers single fruits until there are 18 or more), the first page shows 17
      newest first, "next" shows the rest, "previous" goes back to the first 17.
- [ ] sefewfe bids 1,000 on the fruit: refused (7104) and the row's price shows **1,050**; a bid of 1,050: 7106, sefewfe's gold drops by
      1,050, and ProbeHero (online) gets 7132 ("bid with 1050 golds").
- [ ] sefewfe bids again on the same auction: 7109 ("bid already"). A bid of 5,000 or more: nothing happens at all.
- [ ] sefewfe buys it now: 7111, the gold drops by 5,000, the fruit is in sefewfe's items to collect, the 1,050 bid is back in sefewfe's
      gold to collect (7134), and ProbeHero gets 7130 with **5,020** in gold to collect (120 − 100 + 5,000).
- [ ] Collecting: sefewfe's fruit goes into the first free bag place (7121); sefewfe's and ProbeHero's gold are paid (7123, a chat line
      with the gain). Collecting again shows nothing left.
- [ ] Cancel: ProbeHero registers another auction, sefewfe bids on it, ProbeHero cancels it: the item is in ProbeHero's items to collect
      (7119), the 120 deposit is not returned; sefewfe gets 7134 and its bid back in gold to collect.
- [ ] The end: an auction of 7 h left alone ends within a minute after its end time (ProbeHero gets 7131, the item back to collect, the
      deposit lost). One of 7 h with a bid from sefewfe: sefewfe gets 7133 (the item to collect), ProbeHero 7130 (the gold). Mark "can't
      test" if nobody waits 7 hours.
- [ ] Interest list (sefewfe): add auctions (7125) up to 10; the same one twice (7127); an 11th (7128); one that has been sold (7126). Remove
      one, then clear the list (after the confirmation 7129).
- [ ] After a server restart (ask first): the auctions, the items and gold to collect and the interest list are all still there.
- [ ] GM, on map 42: TestVail types `/auctionsearch ProbeHero`: the auction window shows ProbeHero's auctions. Off map 42 the command
      fails.
- [ ] GM: `/auctionrecall ProbeHero <market id>` (an id from the search) with sefewfe's bid on it: ProbeHero is disconnected, the item is
      in TestVail's items to collect, sefewfe gets 7134 and its bid back, and ProbeHero (after logging in again, on map 42) has the
      **120** deposit in gold to collect (deviation 3). `/auctionrecall ProbeHero 0` recalls all of ProbeHero's auctions.
- [ ] Open from the client: search with a level range (e.g. 1 to 10). Items of those levels should show; if only level 0 and 1 items
      show, the client sends tenths of the level (note what you see).
- [ ] `logs/game-server.log` shows no errors.

## 41. Small items: kick notice, `/server`, reset counts, all-server chat, `/town`, equipment, VG, name change, Priest

`docs/original/small-items.md`: the `/kick` and ban notices held before the close (deviation 1), `/server on`/`off` as the original
(`F706` opens, `F707` closes), the reset counters, a debug line for the invisible staff member, Message To Server (`0x1108`), `/town`,
looking at another player's equipment (`0x0307`), the VG for the lapis link window (`0x0555`), the name change (Name Change Stone and
`0x010E`, deviations 3 to 6), `0x0559` kinds 1 and 2 (deviation 7) and the Priest (`0x0F01`/`0x0F02`). **TestVail** (the GM account, staff
level 1, Fury), **TestRanger** (the first player account, Light) and **TestPriest** (the second player account, Light), on map 1 near (1205, 12.5, 1617) as for
sections 37 and 38. Watch `logs/game-server.log` for "kicked by a GM", "all-server chat", "town", "priest reading", "rename allowed",
"renamed", "rename refused" and "staff hidden from viewer" (debug).

**Setup** (server stopped, after rebuilding **both** servers: the first start applies migration 0026): TestRanger (id 3) gets 3 Message
To Server (`INSERT INTO character_items (character_id, bag, slot, item_id, type, type_id, count) VALUES (3, 1, <slot>, 100064, 100, 64,
1)`) and 2 Name Change Stones (the same with `100136, 100, 136`), each in a free bag 1 slot (`SELECT slot FROM character_items WHERE
character_id = 3 AND bag = 1` lists the taken ones). Note TestRanger's gold.

**Setup applied**: the 3 Message To Server as one stack in bag 1 slot 0 (bag 1 had only two free
slots), the Name Change Stones in bag 1 slot 9 and bag 2 slot 23. TestRanger's gold: 10,000,000.

- [ ] `/kick TestRanger` (TestVail): TestRanger's chat shows "Log out by operator", about 3 s later the disconnect box; TestPriest saw
      TestRanger leave at once. TestRanger can log in again at once.
- [ ] `/aban 2 <second player account>` with TestPriest online: "Ban connecting for 2 minutes by operator" in TestPriest's chat before the box (then
      `/abancan <second player account>`).
- [ ] `/server off` (client with fixes.dll) closes the server to new logins; a second `/server off` keeps it closed; `/server on` opens it; a
      second `/server on` keeps it open (the second player account's game login refused only while closed).
- [ ] At the combat instructor (NPC type 13): the stat reset question says "0 chances left", the skill reset "1 chances left" (TestRanger
      has not used its free skill reset); after using it "0", also after a relog.
- [ ] Invisible staff, with **TestRanger's packet log on** (`gsConfig.cfg`, `Log.txt`): TestVail logs in near TestRanger: TestVail is not
      drawn; the log has `01 02 06 00 00 00 02 …` then `1D 05 06 00 00 00 0D`, and no `1D 05 06 00 00 00 00` after it. `/char off`: drawn,
      `1D 05 06 00 00 00 00`; `/char on`: gone, `… 0D`. If TestVail is drawn although the lines are there in that order, keep the log lines
      for the client side.
- [ ] TestRanger uses a Message To Server: used up; a second at once is refused and stays. TestRanger types a normal line: TestPriest (walk
      it to another map) and TestVail see "[chatting all members]TestRanger : …"; TestRanger's own chat shows the same all-members line, not a
      normal one (note how it looks); the next line is normal chat. Arm another, go to the character screen and back: the next line is
      normal chat.
- [ ] `/town` (or `/return`) on TestRanger, standing still: 10 s later at the map's bind point. Again, then a step within 10 s: nothing
      happens, no message.
- [ ] TestRanger views TestPriest's equipment (player menu): the window lists its gear with gems; on TestVail (Fury, staff) nothing opens.
- [ ] Lapis link window: the price shown matches what the server takes; with `/set` VG 1000 on TestRanger (relog not needed) it is 100 lower.
- [ ] TestRanger uses a Name Change Stone (a second in the same session is refused). Character screen: TestRanger offers the rename;
      "TestPriest" (taken) fails; "Ranger2" succeeds and shows; after another logout no rename is offered; TestMage's guild list (Builders,
      if TestRanger is a member, else skip) shows the new name; TestPriest's friend list (if friends) after its relog; `SELECT * FROM
      character_renames` has the row. Rename it back to TestRanger with the second stone afterwards.
- [ ] Priest: if an NPC menu offers the paid reading, it shows five numbers and takes 10 gold (note whether the gold shown drops: the
      server sends no `0x0213`, as the original); with 10 gold or less, "Insufficient gold". Mark "can't test" if no NPC offers it.
- [ ] `0x0559` kinds 1 and 2: no US item uses them (the snowball is a placeholder): mark "can't test" unless the client side gives a way.
- [ ] `logs/game-server.log` shows no errors.

## 42. Monsters, guards and walking NPCs moving without stops

`docs/original/chase-smoothing.md` (deviations 1 to 5, `smooth_chase`, on by default): running steps wait for the client's own
walking time, with a step taken every 100 ms between the passes, so a chasing monster never stands still until it is in reach. Any
character; a monster that chases you, standing and then walking away. Expect the monsters to catch up about 10 % sooner than before.

- [ ] An ordinary monster (map 1, e.g. near ProbeHero): hit it from 15 to 20 away and stand still: it runs to you without a stop and
      strikes on arrival. Walk away slowly: it follows without stopping (before: a short stop every step, the run animation starting over).
- [ ] The crocodiles of `AI` 11 on map 47 around (636, 586) (section 29): the same, no "move, pause".
- [ ] Run away beyond its reach (about 45 from where it stood): it still gives up and runs home, without stops, and is shown home where
      the server has it (no jump at the end).
- [ ] A monster slowed by a debuff (move speed down, e.g. a ranger's or mage's slow on it): it chases slower, still without stops.
- [ ] Guards (section 33, TestVail): a guard's run at you looks the same as before but without a stand between its steps; its last,
      short step comes at once.
- [ ] The walking animal of section 33 (or any walking NPC): it walks on between its steps without the 1 s stand (it still stops at
      each end of its walk, 18 to 28 s, as before).
- [ ] A Lua boss (section 31, Cailum Sacra) chasing or walking: no stops, no jump.
- [ ] `logs/game-server.log` shows no errors.

## 43. Item-skill flags: EXP stones, protections, gold, charms, resurrection runes, ultimate-mode deaths

`docs/original/item-flags.md`. **ProbeHero** (the first player account, Light, level 60) uses the items; **PISS** (the first player account, ultimate mode, level 1)
tests the ultimate death. **Careful: sefewfe, PISS and PIZZA are ultimate mode**: from this build on, dying without a resurrection rune
or a Continuous Res Rune makes them fall (gone from the list, kept in the database). To bring one back (server stopped):
`UPDATE characters SET deleted_at = NULL, fallen_at = NULL WHERE name = '<name>'`.

**Setup** (server stopped): ProbeHero (id 1) gets in bag 1: slot 6 Large Increase EXP (100043), 7 Large Increase Gold Drop (100076),
8 Blue Dragon Charm (100072), 9 White Tiger Charm (100073), 10 Red Phoenix Charm (100074), 11 Small Prevent Of EXP Drop (100057),
12 two Small Ressurrection Runes (100081), 13 two Ressurrection Runes (100044), 14 3Hr Continous Res Rune (100129).
**Setup applied** (migration 27). PISS has one Small Ressurrection Rune (bag 1 slot 1); sefewfe has 10 of each rune (bag 1 slots 1 and 2), so it cannot fall by accident.

- [ ] Kill a monster without the stone and note the experience (`0x0207`, the client's message); use the EXP stone: the same kind of
      monster gives twice as much (deviation 1). With a charm too: 2.2 times.
- [ ] Gold Drop: a monster's gold picked up comes in at twice the amount; gold you drop yourself and pick up again stays the same. With
      the White Tiger Charm + 20 % more, the Red Phoenix + 50 %.
- [ ] Charms: a weaker or equal charm is refused; the Red Phoenix opens the warehouse's second half (slots 120 and up) like Double Warehouse.
- [ ] Die to a monster above level 10 with no protection: 5 % of the level lost at rebirth (note the experience bar). With the White
      Tiger or Blue Dragon: 1 %; with the Red Phoenix or Prevent Of EXP Drop: nothing.
- [ ] Death window: the small-rune button (kind 4): one Small Ressurrection Rune goes, back at the bind point at half HP, SP and MP,
      nothing lost. The rune button (kind 5): one Ressurrection Rune goes, back where you died at half. With none left the button
      works as the plain return (full HP, bind point).
- [ ] After a duel or a death to another player, the rune buttons use no rune (the plain return, full HP).
- [ ] PISS (ultimate): give it a Small Ressurrection Rune first, die: the plain return button uses it (half, bind point, nothing lost).
      Then die with no rune: back at the character screen at once, PISS no longer listed; `SELECT name, deleted_at, fallen_at FROM
      characters WHERE name = 'PISS'` shows both set. Restore it with the SQL above (server stopped) and it is listed again.
- [ ] The 3Hr Continous Res Rune on an ultimate character: dying brings it back at half, nothing lost, nothing used up.
- [ ] `logs/game-server.log` shows "back to life by a resurrection item" and "fallen" lines and no errors.

## 44. The item mall, gifts, the two storages and premium accounts

`docs/original/item-mall.md`. Accounts: **the first player account** (ProbeHero, TestRanger, …) and **the second player account** (TestPriest, sefewfe). Points and
premium are set with `target/release/mall` (`mall show <user>`, `mall points <user> add|take|set <n>`, `mall premium <user> <type>
[<days>]`, `mall deliver <user> <code>`).

**Setup applied** (migration 28): the first player account 5000 points and premium 2 for 30 days; the second player account 100
points and premium 1 for 30 days; the second player account's billing storage holds the Big Firecracker product (`B1_Firec0001`: three firecrackers,
item 100115, slot 0).

- [ ] Logging in with the first player account: the premium line shows (message 815, the 1.5× experience one, with the end date (30 days after the setup)); with
      the second player account the "individual package" line (814) with the same date. Note what the client shows exactly (the client shows only the date, YYYY-MM-DD, once in chat; type 2 has no date; type 3 shows one).
- [ ] The mall window shows 5000 points (the first player account). Buy a cheap product (e.g. Big Firecracker, 25): "Purchased …", the items appear in
      the first empty bag places, the points drop to 4975, and the purchase is in the history tab.
- [ ] Buy something with a full bag (or with fewer free places than the product has items): "Inventory is full", no points taken.
- [ ] With the second player account (100 points) buy something above 100: "Insufficient points."
- [ ] Gift from ProbeHero to TestPriest (logged in, any map): "Sent …" on ProbeHero, "Received … from ProbeHero." on TestPriest, and the
      item in TestPriest's item-mall storage window (open the mall and click the store button at its lower left; the mall closes and the storage opens
      beside the inventory). Drag the item into the inventory: it goes to the first empty bag place. Opening sends nothing.
- [ ] Gift to a character of the same account (e.g. TestRanger from ProbeHero): "Can't send the present." To a name that does not
      exist: "The character doesn't exist."
- [ ] Gift to sefewfe while the second player account is logged out; log in with sefewfe: "You got a new present." right after entering, and the item
      in the storage window.
- [ ] Points added while in the world: `target/release/mall points <first player account> add 1000` while ProbeHero is in the mall window: the
      points shown change at once (the log says "the account's points changed").
- [ ] The billing storage (the second player account, TestPriest or sefewfe): click the faction's bank teller (Light's or Fury's Bank Teller, a
      merchant) and choose its main menu entry; the window lists the three firecrackers. Drag them into a bag. Opening sends nothing.
      Placed tellers (world.map_npcs, x/z): Light's Bank Teller (1/179) on map 0 (278, 1863), map 1 (517, 1793), (1041, 827),
      (1526, 1865), map 19 (573, 1859), (1395, 136); Fury's Bank Teller (1/180) on map 0 (1895, 363), map 2 (1876, 286), (1028, 1031),
      (350, 217). If the teller is not there or its menu has no storage entry, say which one you clicked.
- [ ] Premium 2 (the first player account): experience from a monster is 1.5 times the usual (2 times with an EXP stone); the equipment does not wear;
      the warehouse's slots 120 and up take items; lapis links succeed more often (rate × 1.5, hard to see).
- [ ] Premium 1 (the second player account): the warehouse's slots 120 and up take items; experience as usual.
- [ ] `mall premium <first player account> 0`, log in again: no premium line, experience as usual, the warehouse's second half refused.
- [ ] `logs/game-server.log` shows "item mall purchase", "item mall gift", "storage item taken" lines and no errors.

## 45. The move speed check

`docs/original/move-check.md`. Any character; ProbeHero (fighter) for Charge. Nothing to set up. The check refuses only walking about
three times too fast, so normal play should never be put back: the point of this section is that it is **not** triggered.

- [ ] Run around normally for a few minutes, on foot and on a mount, through cell edges and across a map: never put back.
      `logs/game-server.log` has no "move refused: too fast" line.
- [ ] Use a portal, a gatekeeper, `/town` or a Return scroll, then run on at once: never put back.
- [ ] Die, come back to life at the bind position (and, in a party, next to the leader), then run: never put back.
- [ ] ProbeHero: learn Charge (fighter skill 56, level 1), use it on a monster about 20 away, then keep walking: never put back.
- [ ] Lag: run while the connection stutters (for example while something else uses the network): still never put back.
- [ ] For the record: after the session, send the "move window" lines (debug level: `RUST_LOG=game_server=debug`) or note the
      `average` values they show; they give the client's real running speeds, needed before any tighter check (spec deviation 4).

## 46. Fire Thorns and fields hitting players

`docs/original/thorns-fields-pvp.md`. Two clients: TestVail (the GM account, Fury, level 60) against TestMage (the first player account, Light mage, needs
Fire Thorns 47 at level 1 and Fire Wall 48) and TestRanger (the first player account, knows Trap 113). Everyone on map 1, TestVail 30 or less from the
caster. Stop the server and insert the missing skills into `character_skills` first if they are not learnt.

- [ ] TestMage casts Fire Thorns (47) and stands next to TestVail: every 3 s TestVail loses HP and sees the hit; a Light character next
      to TestMage loses nothing.
- [ ] Fire Thorns kills TestVail: it dies, the kill counts for TestMage (kill count up, the usual player-kill rewards).
- [ ] TestMage lays Fire Wall (48) and TestVail stands in it: HP drops every second; TestMage's own country takes nothing; TestVail out of
      the wall is left alone.
- [ ] A kill by Fire Wall counts as TestMage's kill while TestMage stands near.
- [ ] TestRanger lays a Trap (113) and TestVail steps in: 15 % of its current HP once, and the trap is gone.
- [ ] A hidden TestVail (Stealth) is revealed by Fire Thorns or a field, then hit.
- [ ] Nothing of this on map 42 (or 40): no hit from either.
- [ ] What the client draws for the hit (number, animation) from Fire Thorns and from a field: note it (`docs/testing/client-research-queue.md`).

## 47. Obelisk.ini boss groups and `/mera` on obelisks and bosses

`docs/original/obelisks.md` sections 11.3 and 11.5 (`docs/plans/done/obelisk-bosses.md`). Setup: the world import with the boss groups (done)
and a restart; the start lines show "boss groups loaded" (25 groups, 21 with a portal). An operator character for `/move`, `/mera` and
the fights (section 37's TestVail as staff, or TestRanger raised to about level 65: raising the test characters is authorized). Watch
`logs/game-server.log` for "boss rose", "boss fell" and "boss due" (zone, group number, stage).

- [ ] `/mera` next to a standing obelisk (map 18 (560, 540) is the easiest): it and its guards vanish, the minimap mark stays its
      owner's; about 3 minutes later it rises again with its guards (was: never until a reset).
- [ ] `/mera t` on a captured obelisk: back to its base country (the minimap mark changes), it rises 3 minutes later as the base's.
- [ ] Map 14 (Cloron's lair): Cloron 871 stands at (203.6, 619.96); kill it → portal 1 lets players into the inner hall; the log's "boss
      due" says about 12 h ± 1 h, and when it stands again the door shuts. Map 17 (Fantasma): the same with 901 at (123.5, 606.7).
- [ ] Map 14 (494.97, 600.08): Cloron 872 with its 8 followers (873, 874, 875); killing it removes them and leaves 10 chests (1970).
      Map 17 (388.49, 610): 902 the same (11 chests, 1971). Neither opens a door (their portal 1005 does not exist).
- [ ] Map 19: the dragon 1974 alone at (229.4, 1031.1); about 5 s after its death the second stage 1977 with its 14 followers; kill
      1977 → the followers vanish, 16 chests (1972) appear, portal 8 now leads to map 48 (Cryptic One, Light). Map 20 the same for
      Fury (1975 → 1978 at (454.1, 191.8), portal 7 to map 49).
- [ ] Map 48: the seven bosses stand by their doors; each kill opens its door (portals 1001 to 1007); 1714 and 1700 are followed by a
      second stage (1703, 1716) about 5 s later. Map 49 the same with the 18xx bosses.
- [ ] Map 67 (Cailum Sacra): 2469 stands at (315.7, 2.2, 81); its death makes 2470 (its script).
- [ ] Map 64 (Oblivio, party zone): 2414 does not stand at first; the log's first "boss due" for it is 45 minutes to 60 hours away (a
      random group). Not worth waiting for; only check that nothing errors.
- [ ] The guild ranking map (section 35's setup): in the guild's zone the three bosses stand (1855 at (507, 340.6), 1853, 1851); killing
      1855 opens portal 3 of that zone only (another guild's zone keeps it shut).
- [ ] `/mera` on a standing boss: its followers vanish and the next stage comes after the boss's own respawn time (the log's "boss
      due"); on a last stage the door opens and the chests come too.
- [ ] Restart: every chain starts afresh (the bosses stand again at once, opened doors shut).
- [ ] `logs/game-server.log` shows no errors.

## 48. The game log (`game_log`, `chat_log`)

`docs/original/game-log.md`. The game server writes what the original's `ps_game` sent to `ps_gameLog` straight into PostgreSQL (`game_log`
for `ActionLog`, `chat_log` for `ChatLog`; migration 0029, runs at start). The client has nothing to show: you do ordinary things, and the
next session reads the rows (`docker exec shaiya-afterglow-postgres psql … "SELECT action, value1, value2, value3, text1 … FROM game_log
ORDER BY id DESC LIMIT 50"`) and checks them against this list. Use two clients where it says so (any two characters of one country; one of them staff for the staff item). Every row
has the character's name and the time, so the rows are found from those. Nothing else is needed: the log is on by default (`log`,
`chat_log`).

- [ ] Log in and enter the world with one character, then log out to the character screen: one 107 (T1 your address, T2 money and bank and
      cash, T3 level/skill/stat points, T4 the stats) and one 232 ('Enter') at entering; one 108 (T2 with `firstmoney`) and one 232
      ('Leave') at the end.
- [ ] Pick up a monster's drop and its gold: a 111 per item (V2 the item id, V3 the count; the unique id in V1) and a 161 for the gold
      (V1 the amount, V2 your money after). A stack topped up shows the stack's own unique id.
- [ ] Buy a few potions and a weapon at a merchant, sell some: 113 (V3 the NPC, V4 the count, V5 the price, V6 the money after) and 114
      (no NPC); the weapon's unique id is the same in the buy and the sell.
- [ ] Put an item into the warehouse and take it out: 121 then 122 (the fee in V8) with the same unique id.
- [ ] Drop part of a stack, pick it up again: 112 (T4 `bag=…,slot=…,Count=…`) and 111; the stack's unique id is the same all through.
- [ ] Trade with the second client: potions (part of a stack) and gold. Each side has its own rows: 116 and 164 for the giver, 115 and 163
      for the receiver; the receiver's row has a new unique id, the giver's keeps its own; a whole item keeps one unique id on both sides.
- [ ] Learn a skill, raise two stats, gain a level: 141 (the skill and the points left), a 151 to 156 per stat raised, a 146 per level.
- [ ] Die to a monster and come back (the button): a 102 (V1 the monster, T1 its name), a 105 and a 106 (V2 1 for the base button); die
      again and let the 200 s timer do it: 106 with V2 5 (`timeover`).
- [ ] Say something normal, whisper the second character, say something in the party and the guild, shout: rows in `chat_log` (type 1
      normal, 2 whisper with the target's name, 3 guild, 4 party, 6 shout), the text as typed (an apostrophe stays one).
- [ ] Take a quest and hand it in: 131, 133 (V2 1) and the reward item's 111 with T2 `ItemCreate`; give another one up: 133 with V2 0.
- [ ] Form a party and leave it: 210 and 211 with the party's number. Make a guild, invite the second character, expel it: 175, 178 for
      both, 179 with V2 1 for the expulsion.
- [ ] Buy something in the item mall: 113 with T2 `PointItem`. List an item at the Auction House, buy it with the second client: 216,
      218, and the seller's 222 (the unique id is the same in all of them).
- [ ] As staff, give yourself an item with the make-item command and use a command or two: 111 `ItemCreate` for the item, a 180 per
      command (V1 your staff level, T1 the command's opcode in hex, T3 what it did).
- [ ] The rows' times are the time of the action (not the log's), and a character name with an apostrophe or a Korean/Chinese
      character is stored as typed.
- [ ] `logs/game-server.log` shows no "game log could not be written" and no "game log queue is full".

## 49. Operator chat listening (`/cwatch`)

`docs/original/gm-commands.md` section 8.4 (deviations 21 to 24), client side `docs/client/operator-listen.md`. **Needs the new `fixes.dll`
installed** in the GM's client (`listen_chat=1`): installed in this client (`docs/client/operator-listen-install.md`,
with its rollback); a stock client, or an older DLL, drops every copy without a trace. The staff account is the GM account (TestVail, level 1); the watched player is any
character of the second client (e.g. ProbeHero, Light). Both may be on different maps.

- [ ] TestVail: `/cwatch ProbeHero` → the command's confirmation (272). `/cwatch Nobody` (no such character online) → 2017 "Observation
      function failed."
- [ ] ProbeHero says a normal line: TestVail's chat shows `[Listen] ProbeHero: [NORMAL]<line>` among the GM lines (not in the centre of
      the screen), even when far away.
- [ ] ProbeHero whispers someone of its country, chats in its party and its guild, shouts, uses the trade channel (`$`, level 10 or more)
      and the area channel: `[TO,<target>]`, `[PARTY]`, `[GUILD]`, `[SHOUT]`, `[ALL]`, `[ZONE]` lines. Lines ProbeHero receives are not
      copied. A refused line (a whisper to nobody, guild chat without a guild) gives no copy.
- [ ] A line with `<` in it, or a colour code, shows literally (the DLL turns `<` into `[`); nothing crashes.
- [ ] `/cwatch off` → 273; ProbeHero's next lines give no copy. A second `/cwatch off` → 2018 "Quitting observation function failed."
- [ ] `/cwatch ProbeHero`, then ProbeHero logs out to the character screen and comes back: no more copies (the link ended), TestVail's
      `/cwatch off` → 2018.
- [ ] With the DLL installed: auctions, mail and the server time still work (they reach the client through the handler the DLL wraps).
- [ ] `logs/game-server.log` shows "gm command" lines for `FA0B`/`FA0C` and "gm listens to a player's chat"; the game log has a 180 row for
      each (T1 `FA0B` or `FA0C`).

## 50. Duel and guild-war deaths keep the item buffs

`docs/original/skills-buffs.md` section 16. A death that costs no experience (a duel's, a guild war's, the flagged death) keeps the applied
skills with an id above 200 at a level other than 100 (the elixirs, the stat, speed and critical potions, the goddess blessings, the
invincibility potions, the newer player skills 302 to 316); every other applied skill not marked "keep through death" goes, as before.
TestVail and ProbeHero are of different countries (a duel needs that); ProbeHero is the one who dies.

- [ ] ProbeHero drinks a Life Elixir (skill 207) and a stat potion, and casts a class buff of its own (a skill id up to 200). TestVail
      and ProbeHero duel; ProbeHero loses by death. After the rebirth the elixir's and the potion's effects still work (max HP and the
      stat still raised); the class buff is gone.
- [ ] **Watch the client:** the original sends no `0x050E` at death because the client clears its own list. Do the icons of the kept
      buffs stay in the client after the duel death? If they vanish while the effect still works, note it: the fix would be a `0x050D`
      (applied) per kept buff after the rebirth (a small follow-up, not built).
- [ ] ProbeHero logs out and back in: the kept buffs are listed again, with the time left (saved applied skills).
- [ ] ProbeHero dies to a monster with the elixir active: the elixir goes (experience is lost as always).
- [ ] A guild war (if one can be started): a death in it keeps the elixir too; `logs/game-server.log` shows no errors.

## 51. The operator console (`.commands`, the `console` tool)

`docs/original/operator-console.md` sections 5 to 7 (built, section 10). TestVail (the GM account, level 1) in the game; a shell on the server
machine for the tool (`target/release/console 1 …` or `cargo run -p login-server --bin console -- 1 …`). The answers come as whispers from
`Server` (the chat shows "Server: …" as a whisper). The tool, the switches, the countdown and the NOTIFY path were run against the live
dev server without a client; what is left is what only the client shows.

- [ ] `.help` → whispers from `Server`, one per command; `.help kick` → usage, one sentence and an example; `.kick` alone → the same text;
      `.foo` → "unknown command foo; try help". `.info` → `players 1 (1/…)`, `accepting yes`, the uptime, `exp x1.000`, `killrate 1`,
      `shop on`, `killcount on`. The typed line itself is not seen by a second client standing next to TestVail (not chat).
- [ ] A line after a chat colour (`/chatcolor`, if the client allows it for staff): `.info` still works.
- [ ] `.notice Hello` → every player sees "[Notice] Hello" (also on another map), as a new `F106` (**watch the client**: the packet is
      `u16 len, text`; if nothing shows or the client misbehaves, note it).
- [ ] `.exp 2` → `exp x2.000`; a monster gives twice the experience it gave before (compare one kill before and after, same monster and
      level); `.exp off` → back. `.exp 0` → monsters give none.
- [ ] `.killrate 2`, a PvP kill (no duel, on a PvP map) raises the killer's kills by 2 (the faction count by 1); `.killrate 1` → by 1.
- [ ] `.shop off` → an item mall purchase fails with the usual message; `.shop silent` → the purchase gets no answer (the client waits);
      `.shop on` → it works.
- [ ] `.killcount off` → the faction counts stop moving with kills; `.killcount on` → they move again.
- [ ] `.close` → the login server lists the server as closed, a new login is refused; `.open` → open again. `.maxusers 1` → a second
      login is refused while one connection is open; `.maxusers 5000` → it works.
- [ ] `.kick char ProbeHero` → ProbeHero's client is disconnected, the character saved (relog: where it was). `.kick user <account>`,
      `.kick userid <n>`, `.kick charid <n>` the same by their names; an unknown one → "not found".
- [ ] `.zones` and `.map 1` answer with counts; `.allout` → every client is disconnected, the server keeps running.
- [ ] `.quit 1` → "The server will shut down shortly…" then "in N seconds" notices every 10 s, every 5 s from 30 s and every second from 10 s;
      new logins refused meanwhile; `.quit cancel` before the end → the cancel notice, logins work again. (Optionally let one run to the
      end: the server stops as with `tools/game-server.sh stop`; start it again. Done from the tool: it does.)
- [ ] The tool: `console 1 info` prints the same line; `console 9 info` (no such server) → "no answer"; `console 1 exp -1` → the usage text.
- [ ] A level-5 GM's `.info` is ordinary chat. The game log has a 180 row per command (T1 `.info`, T3 the arguments); the server log a
      "console command" line.
- [ ] Kill boost (`kill_boost_skills` is `200/1:2,200/2:5,200/3:10`, set by the kill-boost extension; the three stones from `/imake 100 212 1`, `100 213 1`,
      `100 214 1`). **After client data patch 109 is installed** (`docs/plans/done/client-patches.md`): the stones show their names (Kill Boost
      Stone, Lv2, Lv3), the Mini Sun EXP Stone's icon and their text; using one shows its buff ("Kill Boost", "Kill Boost Lv2", "Kill
      Boost Lv3", icon 225) for one hour; with them a player kill adds 2×, 5× and 10× the kill rate (`.killrate 1`: +2, +5, +10), without
      one +1; the buff stays through a death and a relog; a stronger stone used during a weaker one replaces it with a fresh hour, a
      weaker one is refused while a stronger runs, the same one again restarts the hour. Before the patch the client's copy is the empty
      "???177": note whether it can be used at all.

## 52. The gambling house (`mini_game`)

`docs/original/mini-game.md` sections 2 to 5 and 9 (built, section 12; the client's side read statically, section 13). A character with
some gold (e.g. 50 000) next to a gambling NPC: map 1's (type id 1, Asran Epebos), map 2's (4, Quibos Raksion), or the others (maps 19,
20, 28, 29). Research entry 6 read the client without running it: every answer's fields agree with the server's, and standing up should
reload the map and send `0x0201`. This test is the first run. If the client misbehaves, set `mini_game = false` in `config/game.toml`, restart, and report
what happened.

- [ ] The NPC is drawn with its model and name (Asran Epebos on map 1), and clicking it from a normal distance works. Clicking opens the
      table selector (seven rows, bets 100 to 3000 per line, a gamble button and a cancel button); the labels and buttons read right.
- [ ] Sitting at a table: the character leaves the view of a second client standing by (it sees the character vanish).
- [ ] Bet one line, spin: the reels stop on nine tarot faces (never the card back); a win highlights its line and shows its amount. Bet up
      to eight lines: the ninth bet is refused; with too little gold the bet is refused.
- [ ] After a loss the next spin is already paid (the gold shown went down by the bet, the lines stay selected); taking it back (`bet`
      again) returns it. With too little gold for the re-bet, the lines and bet clear instead (the `short` byte).
- [ ] After a win: "again" adds the win and pays the next spin (or clears the bet when the gold cannot pay it); or double up: the dealer's
      card shows; guess high or low; a win doubles, then ×4 …; stop takes the stake; a loss gives back a fifth.
- [ ] After the card game ends (a loss, stop, or the fifth win), back at the slots: **no bet is selected** and the gold shown is the
      server's (the server clears the bet; whether the window does is not read). Note it if the old lines are still selected.
- [ ] Card round five (if reached, or with `card_win_chances = [1000, 1000, 1000, 1000, 1000]` in `config/game.toml` for the test): the fifth win pays the stake and
      returns to the slots.
- [ ] The gold shown at the table agrees with the server's (the window works most of it out itself and takes the server's number only
      after a bet and on standing up): the gold shown just before standing up equals the gold after it and after a relog; the 192 rows
      of `game_log` follow the same numbers. No 30-second timeout box appears.
- [ ] While seated: the chat input cannot be opened (expected: the scene has no chat editor); note whether chat from others shows; the
      character cannot be moved (the camera turns and zooms).
- [ ] Standing up: the gold shown is right, the map reloads and the character is back in the zone (the client sends `0x0201`; the second
      client sees it again). If the character stays gone after standing up, note it.
- [ ] Logging out at the table, back in: the bet or winnings on the table were handed back (the gold).
- [ ] The odds feel fair: over about 50 spins of one line at table 0, the gold ends near where it began (90 % back on average).
- [ ] Sitting down while dead, trading, with a personal shop open, in a duel or a guild war, or far from the NPC (a modified client only;
      else skip) is refused (`0x3101 0`).
- [ ] `game_log` has rows 190 to 194 for the session (190 with the map, 192 the money changes, 193 each spin, 194 each card guess, 191 on
      standing up); level and experience are 0 in them.
- [ ] `logs/game-server.log` shows "sat at a gambling table" and "stood up from a gambling table" lines and no errors.

## 53. The data editor (`tools/data-editor.sh`, `docs/plans/done/data-editing.md`)

The page is at `http://<the box>:30890/` (`tools/data-editor.sh start`; loopback, private and Tailscale addresses only). Use TestVail
or any character on map 2 near monster 200 (the small grubs at (174, 828)); the page's History and Undo put everything back afterwards.
The editor and the game server share the database; the `console` tool does what the page's Servers window does.

- [ ] The page opens, map 2 is drawn with its spawn areas (green boxes), NPCs, portals and the blocked-cell layer; clicking a monster in
      the list opens its nine drop entries; the numbers (a percent for entries 1 to 6, per million for 7 to 9) match what `docs/original/combat.md` section 7 says.
- [ ] An edited drop seen in game: raise monster 200's entry 1 chance (or give it a grade of valuable items) on the page, Apply with a
      message, press "Reload drops and monsters" in the Servers window (no restart), kill a few of those monsters in game: the new drop
      appears, and with the chance back at its old value (History, Undo, reload) it goes away again.
- [ ] A monster's stat edited live: change monster 200's `defense` or `attack1` on the page, Apply, reload; a monster already standing
      there follows at once (the damage you deal or take changes); its HP, if edited, follows only for monsters that spawn afterwards.
- [ ] A moved spawn area after a restart: drag one of map 2's spawn areas (select it on the map, then drag it) to an empty spot, Apply,
      "Restart in 1 minute" in the Servers window (the countdown notices reach the game, the server comes back by itself); after the log-in the
      monsters of that area stand in the new place (before the restart they were still in the old one).
- [ ] A new spawn area: draw one with "New area", give it a monster, Apply, restart; the monsters appear there. Removing the area on
      the page, Apply and restart takes them away.
- [ ] A portal's new destination: change a portal's `to_map` and spot on its tab, Apply, restart; walking into the portal arrives at
      the new place (change it back with Undo afterwards).
- [ ] An obelisk guard count: on map 0 (the obelisks) change a guard group's `count`, Apply, restart; the obelisk's guards number what the
      page says when the obelisk is neutral.
- [ ] The drop-this-one-item helper: pick an item on the "Items & grades" tab, "drop this one item", warning read, a monster and an
      empty entry chosen, Apply and reload; only that monster drops the item (kill it a few times), and the other monsters of its old grade
      no longer do.
- [ ] A map's background picture: "Set picture…" with any PNG and the four edge numbers shows it under the spawn areas; the opacity
      slider works; "Remove picture" takes it away. (With the real map pictures of research entry 8: spawn areas and NPCs stand on the
      right places of the picture.)
- [ ] A refused value says why (a drop chance of 150, a monster id that cannot spawn, an area outside the map) and leaves the draft as
      it was; History lists every Apply with its message and who; Undo of the newest change set works, an older one is refused when a later
      change touched the same field.

## 54. The chat markup check measures to the first zero (`markup_is_safe`)

`docs/client/client-packets.md`, "Chat markup crash"; client research entry 14. The server now ends the text at its first zero byte
before it checks the colour markers (as the patch kit does), so a marker whose length is covered only by bytes after a zero is
refused. Needs a client that can send raw chat (a packet tool or a test client); the normal client cannot type a zero.

- [ ] Normal chat `<<005c01` + zero byte + `hello` is refused (nobody sees it, no client crashes); `<<005c01hello` still shows the
      word in colour 01; plain text and the other chat kinds (whisper, party, guild, memo remark) still work.


## 55. Audit fixes, part 1: surviving hostile connections (`docs/server/audit.md`)

The server and the login server now bound what one connection can cost. Everyday play must feel the same; the rest needs a second
machine, a packet tool or a test client.

- [ ] Everyday play feels the same: walking in a crowded town, chat in every channel, the auction house's "collect all", entering and
      leaving the world, a relog within 15 s of leaving (the old session is waited for, then the new login gets in).
- [ ] Chat at human speed is never dropped: type lines as fast as you can in normal, zone, party and guild chat; they all arrive. A
      macro or a tool sending more than about 3 lines a second to one of these channels gets 5 through and then only 3 a second (the
      rest vanish with no message); shout and trade chat keep their own cooldowns.
- [ ] A second client on the same address, up to 8 (several characters in the world on one machine); the ninth connection from the address
      is refused (the client shows its connection-failed box).
- [ ] Pulling the network cable (or switching Wi-Fi off) in a busy place: the character leaves the world within about 30 s (the write
      deadline of 20 s, then the 10 s stay) and a login with the same account works at once afterwards.
- [ ] A client that is frozen (suspend the process, e.g. `kill -STOP`) in a busy place is dropped after about 20 s and the others in the
      place are not held up; `/kick <name>` of such a client works at once.
- [ ] A silent client is idled out at `idle_secs` (600) even in a busy area: leave a client at the character screen with nothing
      typed; it is closed after ten minutes without the client's keepalive. (The real client sends one, so normal play never meets this.)
- [ ] Leaving the client at the login screen for more than 10 minutes without logging in disconnects it; the key exchange itself is
      unchanged (the client does it at once).
- [ ] The log under `tools/game-server.sh` stays quiet while a player walks into a wall or spams chat (no line per refused step or per
      unsafe chat line; at most one every ten seconds per kind).
- [ ] `tools/game-server.sh restart` after a `kill -9` of the server: the loop starts it again; five kills within a minute stop the loop
      (the log says it gives up). The console's `restart` still brings the server back.
- [ ] Auction house, guild bank and personal shops work as before while another player stands stuck in the same place (a frozen client in
      the auction house does not stop other players' bids and collects).

## 56. Audit fixes, part 2: whole exchanges and saves (`docs/server/audit.md`)

Trades, duels, rewards and guild writes now happen whole or not at all. Everyday trading and dueling must feel the same; the rest
needs two or three clients on one machine (several characters on one address are fine).

- [ ] A normal trade still works: items and gold both ways, a split stack, a cancel, a step more than 10 away. The partner sees nothing new.
- [ ] A trade and a duel cannot be open together: with a stakes window open, a trade request to either player is answered busy; while
      trading, a duel request is refused (the client's "not possible"); a player with a personal shop open cannot be asked to duel and
      cannot ask. Closing the window (or the trade) frees both again.
- [ ] Staking an item and then moving, selling, dropping or using it closes the duel stakes window on both screens (and a trade offer of
      the item still ends the trade, as before). Pressing ready afterwards takes nothing from either bag.
- [ ] A dead character cannot ask for a trade or answer one (nothing happens on the screen); after coming back it can.
- [ ] A duel with stakes still works end to end: stakes out of the bags at the countdown, the winner takes both, a give-up or a step out of
      the area puts each side's stakes on the ground at its own feet.
- [ ] Leaving the world during a duel's countdown or fight (log out, or pull the cable): the leaver's gold and items are back in its bags
      at the next login, in one piece.
- [ ] Taking a PvP kill reward or a death reward (the panels, level 16 and up, normal mode) gives the points or gold once; opening the
      panel again gives the next step only. A stat reset item puts the kill steps back to the first.
- [ ] A buff item (an EXP stone, the gold drop item) and the Goddess Blessing still work: the item is used up and the effect appears. A
      recall rune, a teleportation stone and a movement rune still move the player after the 5 s cast, with the item gone at the arrival.
- [ ] A kill in a party with the loot rule "in turn": the item goes into a member's bags as before; a member who logs out at that moment
      leaves the drop on the ground where the monster died, and the party's gold share of a member who is gone lies on the ground at its
      feet.
- [ ] Guild: Etin items in the bags are all handed in at the house NPC and the guild's Etin rises; an Etin item worth nothing (special 0)
      stays in the bags. A player without a guild loses nothing at the NPC. Buying the house still works for the master with the gold,
      and not twice.
- [ ] Disbanding a guild whose bank holds an item is refused ("failed to disband"), also when the item was deposited after the vote began.
- [ ] Creating a guild: agree, but leave the founder's client frozen (or log out the founder) at the last yes; the others can start another
      guild action within 30 s (before: never until a restart).
- [ ] An auction past its end time refuses a bid and a buy-now (the client's "closed" answer) even in the minute before the end pass.
- [ ] A quest with a reward choice still gives the chosen reward.
- [ ] Two accounts gifting each other from the item mall at the same moment both succeed (hard to try by hand; the database test covers it).
- [ ] With the database stopped for a few seconds while players chat and change friends or guild ranks, the changes made are written when
      it is back (the log says "a queued database write failed; trying again"); a clean shutdown (`tools/game-server.sh stop`) writes the
      queue before it exits.

## 57. Audit fixes, part 3: login guard, editor, inputs and latent items (`docs/server/audit.md`)

The login server counts wrong passwords per account, the data editor answers only requests addressed to localhost, and a few inputs are
stricter. Everyday login and play must feel the same.

- [ ] Login guard (login screen, a spare account `spare` and the GM account): type a wrong password for `spare` five times (they
      may be on one connection or several), then the right one: still "wrong password". On the same machine the GM account logs in at
      once, and logging in with it does not lift the wait for `spare`. After 15 minutes `spare` works again (or restart the login server, which
      forgets the counts).
- [ ] Four wrong passwords then the right one on the same account logs in as before.
- [ ] `create_account` (`nsh create_account name` or `cargo run -p login-server --bin create_account -- name`) refuses a password of 5
      characters ("password must be 6-16 printable ASCII characters") and takes 6 and 16. A 16-character password logs in from the real client (type 16; a 17th to 19th typed character is not sent, so a longer one logs in as its first 16). Existing accounts with short passwords still log in.
- [ ] The data editor: `tools/data-editor.sh restart`, then `ss -ltn | grep 30890` shows `127.0.0.1:30890`. From another machine,
      `ssh -L 30890:127.0.0.1:30890 <the box>` and `http://localhost:30890/` opens the page. `curl -H 'Host: evil.example'
      http://127.0.0.1:30890/api/status` on the box answers 403 ("addressed to localhost"), and `curl http://127.0.0.1:30890/api/status`
      answers the status. Direct `http://<the box's LAN address>:30890/` no longer connects.
- [ ] The checklist page (this one) still opens by the box's LAN address; `curl -H 'Host: evil.example' http://<LAN address>:30880/`
      answers 403.
- [ ] A personal shop title with `<<` markup (`<<999c01`, or `<<` alone) typed into the title box: the shop does not open (no sign, no
      "opened" message); a plain title opens it. Control characters cannot be typed, so only a tool tests those.
- [ ] Walking, the saved positions (the save-position item) and the GM's `/go` and `/summon` still work, on the ground and on bridges and
      high places (the height limit is 10,000 either way).
- [ ] Console demotion (two staff accounts, level 1): as the first, the staff-level command (`F601`, `docs/original/gm-commands.md` section 6) sets the second to 3. The second types
      `.info` in normal chat: it is now ordinary chat (no "Server" whisper). Moved to 11 (a player), it stops receiving players' `/request GM`
      questions at once (a staff level keeps them).
- [ ] The Lua monsters still run their scripts: the instance bosses of `docs/original/instances.md` section 9 (the floor bosses of 115's
      dungeon speak and summon as before); the log has no "Lua script did not load" and no "ran over its instruction limit".
- [ ] Restart the game server (`tools/game-server.sh restart`) with the guild bank loaded: the log has no "guild bank count fix" error,
      and the bank still opens with the same items.

## 58. Extensions foundation (the Kill Boost Stones as an extension; `docs/plans/ext-1-foundation.md`)

The dev box now runs from a worktree of the clone, and the Kill Boost Stones come from the extension `extensions/kill-boost` instead of
`additions.sql`. Nothing in play should change. Part B moved the game server's settings out of the environment: the dev box's six
(`guild_party_size = 2`, `mini_game`, `slot_payout_percent = 220`, `card_win_chances`, `cross_faction_trade_maps`, `market_maps`) are
keys of `config/game.toml` in the live worktree, and `kill_boost_skills` comes from the extension (`target/release/game-server config`
shows each value and its layer).

- [ ] After the move (staff account, a spare character): `/imake 100 212 1`, `/imake 100 213 1` and `/imake 100 214 1` give the three stones; using each
      shows its buff for one hour (with client patch 109 installed: the names, icons and text as before); a player kill with each counts 2×,
      5× and 10× (section 51 unchanged).
- [ ] (part B) The stones with the extension off and on. On the box: `target/release/data-editor ext disable kill-boost`, then
      `tools/game-server.sh restart` (`game-server config` shows `kill_boost_skills = "" # default`). A stone from `/imake 100 212 1` still
      uses and shows its buff, and a player kill adds 1×, not 2×. Then `ext enable kill-boost`, restart: the kill adds 2× again.
- [ ] (part B) Settings from the file: a guild founded by a party of two (`guild_party_size = 2` now comes from `config/game.toml`; section
      9 as before); the gambling house's slot payouts and card double-up as in section 52 (`slot_payout_percent = 220`, `card_win_chances =
      "420,200,130,100,80"`), and a personal shop set up on map 35, 36 or 42 and refused on another (`market_maps`) and a trade between the
      factions on map 42 and refused elsewhere (`cross_faction_trade_maps`), all as before the move.
- [ ] (part B) The data editor's layer: `target/release/data-editor draft add set setting guild_party_size value 3`, `apply-draft -m
      "party of three"`, `tools/game-server.sh restart`: `game-server config` shows `guild_party_size = 3 # data editor (change set N)`
      and a party of two can no longer found a guild (a party of three can). `data-editor undo N`, restart: a party of two can again.

## 59. Rules (extensions chunk 2; `docs/plans/ext-2-rules.md`, `docs/server/rules.md`)

Rules are off until written: the live dev box's `config/game.toml` has the examples at its end, commented out. To turn a rule on,
uncomment it, run `target/release/game-server rules --check`, then `target/release/console 1 reload`; to turn it off, comment it again and
reload. `target/release/game-server rules` shows what is in force, and `.rules` in the game (staff) the hits of each rule.

- [ ] Without any `[rules]` entry nothing changed: trade chat below level 10 says nothing (as before); a personal shop outside maps 35, 36
      and 42 does not open (as before); players of both factions trade on map 42 and not elsewhere (as before). `.rules` lists the three
      `setting.…` rules and no others.
- [ ] `level-gap` (set `gap` to fit two test characters of different factions, e.g. 10 and 40 with `gap = 20`; reload): neither can attack
      the other; the attacker is told "The level difference is too great to fight." by a whisper from `Server` once in 10 seconds however
      often the attack is tried; a skill and an area skill that would hit the protected player do not; a duel between two players of one
      faction across the gap still works.
- [ ] A `deny attack` rule in the style of `safe-evenings` on a map (give it the hours of now, reload): attacks on players of the other
      faction there are refused with the rule's message; area skills, Fire Thorns and fire walls there do not hit players; outside the hours,
      or on another map, they do.
- [ ] `market-no-pvp-gain` (map 42 or the map you test on): a player kill there gives no experience, the kill counter and the faction's
      count do not move, no player-kill quest step is counted, the victim's death counter does not move; on another map they all do.
- [ ] `enchant-jumps` with the weights changed to `[[3, 1]]` for the test (reload): a successful enchant at the blacksmith goes up three
      steps; the item's tooltip, the blacksmith window and another player's view of the item (the craft name in `0x0507`) show the new
      step, for a weapon and for armour; a failure is as before. (A jump of more than one step is not verified in the client.)
- [ ] A rule through the data editor: `target/release/data-editor draft add add rule level-gap '{"rule": {"type": "level_gap", "gap": 15}}'`,
      `apply-draft -m "gap"`, `console 1 reload`: it is in force and `game-server rules` names `data editor (change set N)`; `data-editor undo
      N`, `console 1 reload`: it is gone.
- [ ] `.reload` typed in the game by a level-1 staff member after editing a rule puts it in force without a restart and answers with the
      rule count; a rule with a typo (`target/release/game-server rules --check` shows the line) makes `.reload` answer "reload failed,
      nothing changed" and the old rules stay.
