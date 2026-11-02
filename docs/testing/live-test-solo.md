# Live tests: what one tester can do

The 478 open items of `docs/testing/live-test-checklist.md` (489 with the 11 ticked; section 0, the smoke list, is drawn from them), sorted by who can test them. The checklist
page (`tools/checklist-server.sh`) reads this file: it filters by group and sitting, and lists any open item missing from it. Item numbers
count the `- [ ]` and `- [x]` lines from the top of each section, ticked ones included (the checklist page shows them in the same order).

| group | meaning | items |
|---|---|---|
| **A. One client** | one character at a time; switching characters or accounts in between is fine | 266 |
| **B. Two clients, by yourself** | two accounts logged in at once, the maintainer switching windows; every step is ask/answer or one side standing still | 193 (2.1 is in A and B: the self heal and healing another) |
| **C. Hard alone** | three clients or more, or two players acting at the same moment | 10 |
| **D. Skip** | needs a modified client or a packet tool, Korean input, or a game state that cannot be forced | 10 |

Group B assumes two clients can run at once, as in the first sittings (the party and the trade between TestPriest and TestRanger). It is slow
by hand; the test bots (shelved, the roadmap's "Frozen") would take the second side of these; for now a second player does. Some A items
end with "a second client sees it": the A part is tested alone and the rest waits for B.

Character moves, gold, items and database settings for a sitting are the server side's to set up (ask; it needs the server stopped).

## Group A in sittings

Grouped by place and setup so each sitting needs one set of changes. Order within a sitting as in the checklist.

1. **Map 1 basics** (setup below). 1 (all; for the 5 % loss set `exp` above the level's start first), 2 (the self heal), 3.1, 3.3,
   3.5, 3.6, 3.8, 4.5, 4.7, 4.8, 6.1, 6.2, 6.4, 6.5, 10 (all but 3), 13.1 to 13.5, 13.9, 14 (all), 15.2, 16.1, 16.2, 16.4, 20 (all), 32
   (all), 42.1, 42.3, 42.4, 42.6, 42.8, 45 (all), 52 (all but 2 and 13; any gambling NPC).
2. **At a blacksmith with the lapis items** (TestRanger or TestPriest, section 19's bag): 19 (all), 26.1, 26.2, 26.4.
3. **Items given in the database:** 21.10 (the kill steps set first), 17 (all; the stones, scrolls and wheels), 18.1 to 18.4 (a mount), 43 (all but 6; ProbeHero and
   PISS), 50.4 (a Life Elixir), 44 (all but 5; the `mall` tool from a shell).
4. **A strong character** (raise one to about level 60, or TestVail as staff): 27 (all), 29 (all but 7; maps 45 and 47), 42.2, 47 (all),
   22.6 to 22.8 (TestVail and six new Goddess Blessings: the first six were used on 10-19).
5. **Guild houses, TestMage:** 34 (all), 35 (all but 6; inside a ranking window, `GAME_RANKING_TIMES` moved to suit), 36.1 to 36.8,
   36.12 to 36.14.
6. **Quest drops, ProbeHero and sefewfe one after the other:** 39.1 to 39.4, 39.8, 39.11.
7. **Auction House, ProbeHero and sefewfe one after the other** (registering, then logging the other in): 24.1 (the shop setup
   window), 40.1 to 40.5, 40.9, 40.11 (an
   auction left 7 hours), 40.12 to 40.14, 40.16, 40.17.
8. **Staff alone (TestVail):** 37.7, 37.14, 38.15, 41.4, 41.7, 41.10, 41.11, 41.13, 49.7, 49.8, 51 (1 to 4, 6, 10 to 13; the "not seen
   by a second client" half of 51.1 and "also on another map" in 51.3 wait for B).
9. **TestVail at the Light guards, staff level set back to 0** (guards ignore staff): 33.2, 33.3, 42.5.
10. **The data editor, TestVail on map 2 (and map 0 for the obelisk):** 53 (all; the page, the `console` tool and the game side by side).
11. **Log checks:** 30.2; 48 (all but 6, 9, 11, 12): ordinary play, the next session reads the rows.
12. **The audit fixes, any character, with a shell on the server beside the client:** 55 (1, 2, 4, 7, 8, 9), 56 (7, 8, 10, 13, 14,
    16; 10 with TestMage, as sitting 5), 57 (all but 8).

## Every section

| § | A, one client | B, two clients | C, hard alone | D, skip | ticked |
|---|---|---|---|---|---|
| 1 | 1–5 | | | | |
| 2 | 1 (self heal) | 1 (heal another) | | | |
| 3 | 1, 3, 5, 6, 8 | 2, 7 | | | 4 |
| 4 | 5, 7, 8 (and casting 1, 3) | 1, 2, 3, 4, 6, 10 | | | 9 |
| 6 | 1, 2, 4, 5 | | | | 3 |
| 7 | | 1–5 | | | |
| 8 | | 1–6 | | | |
| 9 | | 1–9 | | | |
| 10 | 1, 2, 4–9 | 3 | | | |
| 11 | | 1–4 | | | |
| 12 | | | | 1 (modified client) | |
| 13 | 1–5, 9 | 6, 7, 8 | | | |
| 14 | 1–6 (the trade half of 4: B) | | | | |
| 15 | 2 (the trade half: B) | 1 | | | |
| 16 | 1, 2, 4 | | | 3 (modified client) | |
| 17 | 1–5 (others seeing the looks in 3: B) | | | | |
| 18 | 1–4 (others seeing it in 1: B) | 5 (two on one mount; the third viewer is C) | | | |
| 19 | 1–11 | | | | |
| 20 | 1, 2 | | | | |
| 21 | 10 | 1–6, 8, 9 | 7 (three clients) | | |
| 22 | 6, 7, 8 | 1, 2, 4, 5, 9 | 3 (three clients) | | |
| 23 | | 1–13 | | | |
| 24 | 1 | 2–12 | | | |
| 25 | | 1–11 (11: ask sefewfe, not ProbeHero, which shares TestRanger's account) | | | |
| 26 | 1, 2, 4 | 3 | | | |
| 27 | 1–8 (the other country's messages in 2 and 4: B) | | | | |
| 28 | | 1–5, 7, 9–12 | 6, 8 (a third player) | | |
| 29 | 1–6, 8, 9 | | 7 (two players on one monster at once) | | |
| 30 | 2 | | | | 1 |
| 31 | | 1–12 (the second client only has to be in the party; most items are one character inside) | | | |
| 32 | 1–6 | | | | |
| 33 | 2, 3 | | | 4 (no spot exists) | 1, 5 |
| 34 | 1, 2, 4–13 | | | | 3 |
| 35 | 2–5, 7, 8, 9, 11 | 6 | | | 1, 10, 12 |
| 36 | 1–8, 12, 13, 14 | 9, 10, 11 | | | |
| 37 | 7, 14 | 1–6, 8–13 | | | |
| 38 | 15 | 1–5, 7–10, 12, 13, 14 | 6 (three clients) | 11 (Korean input) | |
| 39 | 1–4, 8, 11 | 5, 6, 7 | | 10 (cannot be forced) | 9 |
| 40 | 1–5, 9, 11–14, 16, 17 | 6, 7, 8, 10, 15 | | | |
| 41 | 4, 7, 10, 11, 13 | 1, 2, 3, 5, 6, 8, 9 (a third watcher in 1 and 6 is optional) | | 12 (no item uses it) | |
| 42 | 1–6, 8 | 7 (map 65 needs a party) | | | |
| 43 | 1–5, 7, 8, 9 | 6 | | | |
| 44 | 1–4, 6–13 | 5 | | | |
| 45 | 1–6 | | | | |
| 46 | | 1–8 | | | |
| 47 | 1–12 | | | | |
| 48 | 1–5, 7, 8, 10, 13, 14, 15 | 6, 9, 11, 12 | | | |
| 49 | 7, 8 | 1–6 | | | |
| 50 | 4 | 1, 2, 3, 5 | | | |
| 51 | 1–4, 6, 10–13 | 5, 7, 8, 9 | | 14 (after client data patch 109 is installed) | |
| 52 | 1, 3–12, 14, 15 | 2 | | 13 (modified client) | |
| 53 | 1–10 | | | | |
| 54 | | | | 1 (raw chat: a packet tool or a test client) | |
| 55 | 1, 2, 4, 7, 8, 9 | 5, 10 | 3 (eight clients on one address) | 6 (the real client sends its keepalive) | |
| 56 | 7, 8, 10, 13, 14, 16 | 1–6, 11 | 9, 12, 15 (acting at the same moment; seven players for a guild) | | |
| 57 | 1–7, 9, 10 | 8 (two staff accounts) | | | |

## Sitting 1 setup (applied)

Log in with the accounts of the checklist's "Test setup", pick the character, and go through the sitting's items in any order; the page's
"Sitting 1" filter shows them.

| character | account | where | for |
|---|---|---|---|
| TestPriest (human priest, 30) | the second player account | map 1 (615, 1758), by the human start's guard | most items: heals, buffs, debuffs, Evolution, the human quests, items, repair |
| TestRanger (elf ranger, 30) | the first player account | map 1 (1640, 1768), by the elf start's guard | the ranger skills, the death loss (1.4), quest 3 (10.2), Stealth on a hit (6.4) |
| ProbeHero (human fighter, 60) | the first player account | map 1 (1301, 1650) | Charge (45.4) |
| TestVail (Fury ranger, 60) | the GM account | map 2 (290, 840) | section 6's two map 2 monsters (202 at (174, 828), 2102 at (1577, 1728)) |

- **TestPriest:** experience 1,972,000, 300 short of level 31, so the first kill levels up (1.2). Learnt: Healing 144 (level 2), 123 (2),
  37 (3), Evolution 127 (1), 132 (2), the low-HP passive 28 (2), and for the two-client items later 102 (3), Dispel 103 (2), 116 (2);
  34 skill points left. Bag 1: slot 7 a Light Staff at quality 500 of 1200 (below half: half its attack, 16.1), slot 8 one at 100 (below
  a tenth: none), slot 9 a Small Eternal Endurance (16.4). Bag 2 slot 6: a quest item, Skull of Lupers (27/10), for 14.4 and 15.2.
  Section 13's items are in bag 2.
- **TestRanger:** experience 1,800,000, 121,300 into level 30: a death costs 5 % of the level, 14,680 (1.4). Learnt besides its
  earlier skills: Stop 61 (2), 120 (2), damage over time 135 (2), Scout 100 (1), the low-HP passive 26 (2); 442 points left. Section
  13's items are in bag 2 (Small Increase EXP and both charms for 13.3). Bags 1 and 2 are full: drops go into bag 3.
- **The free skill reset (13.4):** do it last, with TestPriest (bag 3 and up empty): it unlearns the skills above. TestRanger keeps its
  free reset for 41.4.
- **TestVail** is not staff for now (`accounts.status` 0; monsters and guards ignore staff). Sitting 8 makes it staff again.
- **Where things are on map 1:** the gambling NPC (52) at (976, 829); a blacksmith (16.2) at (540, 1816); the warehouse keeper (14) at
  (491, 1785); section 6's monsters, section 10's quest NPCs and section 32's spots are in those sections.
