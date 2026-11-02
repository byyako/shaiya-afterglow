# Pre-launch smoke list

This file is the master of section 0 of `docs/testing/live-test-checklist.md`, which the page on `http://<the box>:30880/` shows at the top.
Everything after the marker line below is copied there (under `## 0. Pre-launch smoke list`) by `tools/smoke-list-sync.sh`; edit it
here and run the script, never in the checklist.

<!-- section 0 starts -->
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
