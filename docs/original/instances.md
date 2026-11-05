# Party instance zones and the Lua monsters (Cailum Sacra)

This page covers party instance zones in the EP4.5 game server `ps_game.exe` — maps with `CreateType` P, where each
party gets a zone of its own — the portal countries of such a zone, and the monsters with `AI` 100 and up (a Lua 5.1 script each, a stack
state machine and a fight path of their own), with the eight shipped scripts. It was read with the decompiler and the disassembler with the
PDB names, and from the files the server reads (`Data/Map.ini`, `Data/Lua/*.lua`, the map files); nothing was executed. The client's side
is in the [client facts](../client/CLIENT-FACTS.md) ("Answers for the server", item 2) and on the client's
[Scripted monsters](../client/scripted-monsters.md) page: the client has no instance logic of its own, and `0x110A` / `0x110B` print and play
what the server sends.

Not covered here: the guild maps (`CreateType` G, R, H; R and H are on [Guild ranking battle](guild-ranking.md) and
[Guild houses](guild-house.md)), the operator's commands that pick a party's zone (`+0x54c8`; see
[GM commands 1](gm-commands.md#83-moves-fa09-fa0a-fa11-fa12)). The boss spawns of `Obelisk.ini` (`[bothmob_…]`) are summed up
[below](#7-boss-spawns-of-obeliskini-bothmob_-catalogue-cmaploadobelisk-0x427300-state-4-czonemapinitbothmob-0x41b470-updatebothmobrespawn-0x41b870-bothmobdestroy-0x41bad0-bothmobrespawn-0x41be50) and analysed in full on [Obelisks](obelisks.md#113-boss-spawns-bothmob_-at-run-time).

## In short

- **Three party instance maps.** Map 64 is a field open Monday to Saturday from 21:00 to 22:00 for parties of 18 or more; maps 65 and 66
  are the first floor of Cailum Sacra (one per faction), always open, for parties of 1 to 7.
- **One copy per party.** Each party that enters gets its own copy of the map, with its own monsters. The copy is created when the first
  member arrives and removed a few minutes after the last one leaves.
- **Leaving the party does not throw you out.** Players leave an instance only by a portal, a recall, dying, or logging out; logging back
  in puts you outside, on map 47.
- **The Cailum Sacra bosses are scripted in Lua.** On the shipped data a party meets an unkillable summoner that speaks, calls two of four
  floor bosses, walks off and vanishes; killing a floor boss opens the portal to "Section 4" for that party. The final boss chain exists in
  the scripts but nothing ever spawns it.
- **Bugs:** a zone whose party came back and left again can vanish at once; leaving the party on the way in disconnects the player.

Status words: **confirmed** = read in the code (address given), **inferred** = follows from confirmed code but is not shown directly.
`CMob` offsets match the PDB; `CUser`, `CZone` and `CMap` offsets are the code's.

## 1. Configuration

Confirmed, `CMap::LoadConfig` 0x426a98, `CMap::CMap`.

`Map.ini`, `[SET_ZONE_n]`, besides the keys of `docs/original/maps-npcs.md`:

| key | `CMap` | value |
|---|---|---|
| `CreateType` | `+0x1a4` | S 1, F 2, **P 3**, G 4, R 5, H 6, else 0 (`docs/original/portals.md` section 1) |
| `CreateTime` | `+0x1a8` | up to 7 triples `(day, hour, hours)`, section 2; missing: first day `0xFFFF` = **always open** |
| `CreateMinUser` | `+0x1d8` | `atoi`, missing: 0 |
| `EnterMaxUser` | `+0x1dc` | `atoi`, missing: **30** (also the constructor's value) |
| `ExpireTime` | `+0x1d4` | minutes; stored as **`minutes × 60000 + 30000` ms**; missing: 0 → 30000 |

Shipped (P maps only):

| map | name | `CreateTime` | min | max | expire | rebirth (`RebirthMapPos…`) | way in |
|---|---|---|---|---|---|---|---|
| 64 | (field, 1024) | Monday to Saturday 21:00–22:00 (`(1, 21, 1)` … `(6, 21, 1)`) | **18** | 30 | 2 min (2.5 min) | map 47, by country | map 47 portals 2 (Light) and 3 (Fury), levels to 60 |
| 65 | Cailum Sacra 1F (Light) | always | 1 | 7 | 5 min (5.5 min) | map 47 (125.58, 9, 454.08) | map 47 portal 6 (country 1) |
| 66 | Cailum Sacra 1F (Fury) | always | 1 | 7 | 5 min (5.5 min) | map 47 (571.37, 7, 194.08) | map 47 portal 4 (country 2) |

Map 67 (Cailum Sacra 2F) has no `CreateType`: an ordinary dungeon shared by everyone.

## 2. Open times

Confirmed, `ParseMapOpenTimeArray` 0x426820, `CMap::IsPartyInsZoneOpenTime` 0x429020.

The value is cut into numbers at any of `` ␠ , ( ) = [ ] { } \t \r \n `` (`strtok`, `atol`); triples fill up to 7 entries of three
`u16` (`day` 0 Sunday … 6 Saturday, `hour`, `hours`); the parse stops at the first missing number. `CMap::CMap` sets **every** entry's day
to `0xFFFF`, so the list ends after the last full triple (a missing key sets the first day to `0xFFFF` again).

Open (`IsPartyInsZoneOpenTime`) when the first day is `0xFFFF`; otherwise for the server's **local** time (`GetLocalTime`), for each of
the 7 entries until a day of `0xFFFF`: `end = hour + hours`; open when **today is `day` and `hour ≤ now's hour < end`**, or, for an `end`
past 24, **today is the next day** (6 → 0) **and now's hour < end − 24**. Minutes do not count.

## 3. Entering: `0x020A` to a P map

Confirmed, `CUser::PacketMaininterface` 0x468be5…0x468ce5, `CWorld::FindPartyInsZone` 0x414430.

After the checks of `docs/original/portals.md` section 2 (distance, country, level, destination loaded), in this order; each refusal is
`0x020A` `u8 portal, u8 reason` (4 bytes) and nothing else:

1. Not open (section 2): **9** ("Now party instance zone closed.", 6600).
2. Not in a party (`+0x173c`): **4** ("only party members allowed", 8042).
3. `FindPartyInsZone(map, party id, the player's current zone)`: **0** (the map index is bad, or the map has a static zone, which a P map
   never has) or **2** (the party's zone exists, is not the player's current zone, and **players in it + players on their way to it ≥
   `EnterMaxUser`**): **11** ("You failed to enter.", 6602). **1** (no zone yet, or the player's own zone, or room): go on.
   (`docs/original/portals.md` section 2 called 11 "no zone for the party": wrong, corrected there.)
4. **The party's member count (`CParty +0x10`, a union's whole count) below `CreateMinUser`**: **10** ("Can't enter because of lack of
   party members.", 6601).
5. Allowed: the move is scheduled as for any portal (gatekeeper move, `m_nMovePositionType` 1). No `0x020A` answer.

## 4. The zone

Confirmed, `CWorld::GetZoneByMapID` 0x413df0, case 3; `CWorld::UpdatePackets` 0x40bf90, `CWorld::ZoneEnterUserMove` 0x40d000.

The zone is found **when the player arrives** (the world thread's queue of waiting players, and the scheduled move's
`ZoneEnterUserMove`), not when the portal is used:

- **Key**: the player's party id (`CParty +0x08`). (An operator with admin level ≤ 10 and `+0x54c8` set uses that instead:
  built in another form, `FA12` moving the operator into the party's zone at once, `docs/original/gm-commands.md` section 8.3 and
  deviation 8.) No party: no zone.
- **Found** in the map's table (`SSyncMap<u32, CZone*>` by key): used.
- **Not found**: a zone is taken from the pool (`SSyncHeap<CZone>::Alloc`), **set up from the map as a static zone is** (`CZone::MapInit`:
  cells, the map's NPCs, the monster spawn entries; `CZone::MapInitObject`), key = the party id, **expiry time 0**, counted (instance
  zones now, the most there have been), filed under the key.
- **No zone** (no party any more, the pool empty): logged ("GetZoneByMapID Error …") and **the connection is closed** (reason 9).

Each zone has **its own** monsters (spawned by its own respawn pass, `CZone::UpdateMobRespawn`, from the map's spawn areas), NPCs, items on
the ground, players and **portal-country table** (section 5). Everything else (the map's terrain, portals, bind areas, rebirth points) is
the map's. The zone's players count (`+0x130`) and the players on their way (`+0x54`) are what section 3 compares with `EnterMaxUser`.

## 5. A zone's portal countries

Confirmed, `CZone::GetInsZonePortalCountry` 0x4179e0, `CZone::UpdateInsZonePortalCountry` 0x417a10.

A portal whose file country is **above 1000** has that number as its id (`docs/original/portals.md` section 1). On a map with any
`CreateType`, the country checked for such a portal is the zone's: a table of **10** `(id, country)` pairs (`CZone +0x218`), empty in a new
zone. **An id not in the table answers 3: nobody passes.** `UpdateInsZonePortalCountry(id, country)` (only for an id above 1000 on a map
with a `CreateType`) changes the pair with that id or fills the first empty one; with all 10 used and the id not among them, nothing.
Countries: 0 everyone, 1 Light, 2 Fury, anything else nobody (as for any portal).

Nothing is sent when a country changes (the client draws every portal and asks with `0x020A` every 15 s while standing in one, item 2 of
the client answers). A refused instance portal gets **no answer**.

Shipped: maps 65 and 66, **portal 1** ("Section 4", country 1001) to (87.49, 33, 180.2) on the same map. Its pair is set by the four
floor bosses' scripts (section 16): `LuaUpdateInsZonePortal(1001, 0)` when one dies, so the portal opens for everyone **in that zone**.
`CZone::BothMobRespawn` / `BothMobDestroy` set pairs too (section 7).

## 6. Leaving and expiry

Confirmed, `CWorldThread::Update` 0x407390 at 0x407a5f, `CUser::PacketUserDBChar` 0x46d1c4, `CUser::PacketUserDBSetInfo` 0x46dc55, `CParty::Leave…`.

- **Nothing moves a player out** of an instance zone: leaving, being expelled from or breaking up the party (`CParty::LeaveNormalParty`,
  `LeaveRaidParty`, `ExpelUser`, `BreakParty`) does not look at the zone; the player stays until they leave by a portal (portal 0 to map
  67), a recall, death and rebirth (the map's `RebirthMapPos`, map 47), or logging out.
- **Logging in** on a map with a `CreateType`: the saved position is replaced by the map's bind position for the character's country
  (`CMap::GetBindPos`, the `RebirthMapPos` of the map: map 47), at the character list and at the character's data load alike.
- **Expiry**, every second, for each zone with a `CreateType` that has **no players and nobody on the way**: with an `ExpireTime` of 0
  it is released at once; otherwise, when its **expiry time is 0, it becomes now + `ExpireTime`** (minutes × 60000 + 30000 ms); once
  that time has passed, the zone is **released**: out of the map's table (the party's zone is gone; a later entry makes a new one), counts
  down, `CZone::MapReleaseObject` (its monsters, NPCs, ground items), back into the pool.
- **The expiry time is never set back to 0** while the zone lives (only a new zone starts at 0): a party that leaves, comes back before
  the time and leaves again after it, loses the zone **at once** on that second leaving (bug, section 17).

## 7. Boss spawns of `Obelisk.ini` (`[bothmob_…]`; catalogue, `CMap::LoadObelisk` 0x427300 state 4, `CZone::MapInitBothMob` 0x41b470, `UpdateBothMobRespawn` 0x41b870, `BothMobDestroy` 0x41bad0, `BothMobRespawn` 0x41be50)

`[bothmob_…` sections ("both" is the code's spelling of boss) define, per zone, a boss group: a position (or three to pick from), a
**chain of boss stages** (one monster each), follower groups per stage (centre, radius, count), a respawn time (seconds, with an
hour's spread, or a random time), and a portal. At zone set-up the first stage spawns (or waits for its random time); when a stage's boss
dies, its followers are removed, the next stage spawns with its followers, and after the last stage the chain starts over and **the
portal opens to everyone** (its country and the zone's pair 0); `BothMobRespawn` of the first stage closes it (**3**, nobody).
(Correction: an earlier version said the shipped `Obelisk.ini` has no `[bothmob_…]` section. It has 25, among them
**2469 on map 67** (`[bothmob_025]`). The full analysis and the catalogue are in `docs/original/obelisks.md` section 11; built,
so 2469 now stands on map 67 and every instance zone of a map with groups (map 64's random 2414, the ranking map's three)
runs its own chains.)

## 8. The shipped Cailum Sacra (data)

Maps 65 and 66 are the same floor for each country (25 spawn groups each: 2460 to 2463, `AI` 1, levels 59 to 61). Group 10 is the only
Lua monster: **2471** ("迪奥斯 艾希尔", level 64, **9999 HP**, `AI` 115) at (220.4–222.4, 28.5, 178.7–180.7). One NPC each. Portals:
0 → map 67 (2F), 1 "Section 4" (1001, section 5), 2 → back to the floor's start (221, 31, 180). Map 67 has 12 royal guards (2468, `AI`
1) and no Lua monster.

| id | level | HP | `AI` | script (section 16) | where |
|---|---|---|---|---|---|
| 2464 | 63 | 197,874 | 111 | floor boss A ("에테르넬") | summoned by 2471 |
| 2465 | 63 | 197,874 | 114 | floor boss D ("휘스타톤") | summoned by 2471 |
| 2466 | 63 | 197,874 | 112 | floor boss B ("가르가델") | summoned by 2471 |
| 2467 | 63 | 197,874 | 113 | floor boss C ("루히엘") | summoned by 2471 |
| 2469 | 64 | 2,059,735 | 118 | final boss, first form | map 67 (315.66, 2.2, 81), `[bothmob_025]` (`docs/original/obelisks.md` section 11.3; built) |
| 2470 | 64 | 2,582,815 | 116 | final boss, second form | 2469's death |
| 2471 | 64 | 9,999 | 115 | the summoner | maps 65, 66 |
| 2472 | 64 | 1,291,407 | 117 | final boss, real one | 2470 at half HP |
| 2473 | 64 | 490,939 | 1 | final boss, copies | 2470 at half HP (2) |

`AI` 104 (1890) names `104.lua`, which is not shipped (section 9: its load fails, the monster has no script).

## 9. Loading a Lua monster

Confirmed, `CMob::InitLuaFSM` 0x4b4c20, `CMob::LuaRegister` 0x4b1310, `CMob::LuaDoFile` 0x4b0f00, `CZone::EnterMob` 0x416860.

`CMob::Init` gives a monster whose definition has **`byAI` ≥ 100** a Lua script instead of an AI machine:

- Its fields: retargeting term (`+0x288`) **5000 ms**; attacked count (`+0x28c`), only-Lua-attack (`+0x290`), hold position (`+0x294`),
  move type (`+0x2a0`), move target (`+0x2a4…`), hook context (`+0x2b4`) all 0; `+0x44` = 1 (it uses the stack machine, section 10).
- **A Lua 5.1.4 state of its own** (`luaL_newstate`) with the **base, math and os** libraries only (no string, table, io); `+0x284` = 1.
- Globals: **`CMob`**, a light user data of the monster, and **`LuaMob`**, a constructor (`LuaDispatcher<LuaMob>::NewObject`) whose object
  has the 23 methods of section 15 (called `Mob:LuaX(…)`). Every script starts with `Mob = LuaMob(CMob)`.
- Then **`.\Data\Lua\<byAI>.lua`** runs (`luaL_loadfile` + `lua_pcall`). A file that does not load or fails is logged
  (`LuaDispatcher<T>::OnError`) and the monster keeps the empty state (its hooks are "not a function", section 14).
- The stack machine of section 10 starts in state 0.
- **`Init()`** is called each time the monster enters a zone (`CZone::EnterMob`, after the `0x0601` to its view), so once per monster.

Each monster has its own state: two copies of one boss share nothing, and a script's globals live as long as that monster.

## 10. The stack machine

Confirmed, `StackStateMachine<CMob,-1>`: `AddState`, `AddPushTransition`, `AddPopTransition`, `AddReplaceTransition`, `STATE::AddTransition`, `DispatchEvent` 0x4929b0, `Poll` 0x4a8350, `SetInitialState` 0x493150; the table set up by `InitLuaFSM`.

**States** (each with an update function, run by `Poll`): **0 idle** (`WhileIdle`), **1 combat** (`WhileCombat`), **2 dead** (nothing),
**3 going home** (`WhileReturnHome`), **4 moving** (`WhileMove`). They form a **stack**; the top is the current state, and **`CMob::GetStatus`
answers the top's number** for a Lua monster (so all the ordinary code that asks a monster's status, e.g. "going home" = 3 in a player's hit
roll, sees it). No depth limit (the template's −1).

**Events**: 0 *attackable* (a player found), 1 *attacked*, 2 *normal reset*, 3 *death*, 4 *return home*, 5 and 7 *move*, 6 *move end*.

**Transitions** (current state, event → kind, target, callback):

| state | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---|---|---|---|---|---|---|---|---|
| 0 idle | push 1, `OnAttackable` | push 1, `OnAttacked` | replace 0, `OnNormalReset` | replace 2, `OnDeath` | push 3, `OnReturnHome` | push 4 | — | push 4 |
| 1 combat | — | replace 1, `OnAttacked` | pop, `OnNormalReset` | replace 2, `OnDeath` | replace 3, `OnReturnHome` | replace 4 | — | pop |
| 2 dead | — | — | — | — | — | — | — | — |
| 3 going home | — | — | pop, `OnNormalReset` | replace 2, `OnDeath` | — | pop | — | — |
| 4 moving | push 1, `OnAttackable` | push 1, `OnAttacked` | pop, `OnNormalReset` | replace 2, `OnDeath` | — | replace 4 | pop (clears the move type) | replace 4 |

(The callback of the move events, `CMob::OnMove`, does nothing.) **Dispatching** an event: with an empty stack, back to state 0 and
nothing else; otherwise the transition of the top state for the event, if any: **its callback runs first** (with the event's argument),
then **push** the target (and run the target's enter function, none here), **pop** (an empty stack: back to state 0), or **replace**
(pop, push). An event the top state has no transition for is logged ("# FSM Error …", `CMob::StackFSMError`) and changes nothing.

**Polling**: **once a second**, in the monster's idle pass (`CMob::UpdateIdle` 0x4a7e60, `CZone::UpdateMobIdle` every 1000 ms): the top
state's update. **A Lua monster has no turn in the 500 ms attack pass** (`CMob::UpdateAttack` skips `+0x44`), so it fights at most once
a second. After the poll, the ordinary idle code runs as well when the status is 0, but a Lua monster's `AI` is never 1 or 3 (it looks for
no one) and its walk timer (`+0xcbc`) has just been set by `WhileIdle`: it does nothing (inferred from the shared timer).

## 11. Where the events come from

Confirmed, the callers of `DispatchEvent`.

| event | from |
|---|---|
| 0 attackable | only `WhileIdle` / `WhileMove` (types 2, 3) finding a player (`FindTargetEx`, section 13), with the player's id |
| 1 attacked | **`CMob::SetTarget(player, …)`** (0x49fb94), which every player hit (normal blow, skill) and other code giving a monster a target calls: for a Lua monster **only in state 0, or in state 4 with a move type other than 1**, and the player alive: **`SetTargetEx`** (section 13), then the event with the target (`+0x12c`). In combat, going home, dead or on a move of type 1 it does nothing |
| 2 normal reset | `WhileReturnHome` arriving home or stuck; `SetReTarget` after 3 s without a target (section 13) |
| 3 death | `CMob::Damage` taking HP to 0 (status 2 written too); `CMob::StatusChange(2)`; `CZone::MobRemove2` (section 15, `LuaDeleteMob`) |
| 4 return home | `WhileIdle` away from home: on a chase path and more than 0.5 from home, or more than **10** |
| 5 / 7 move | `LuaFixedMove` (5 when no move was going on, else 7); `ChaseExp` giving up (5 with move type 3 when none was going on, else 7) |
| 6 move end | `WhileMove` arriving or stuck |

Damage that does not kill (`CMob::Damage`, the hit path; `CMob::Aggro`): instead of `ReTargetByAggro`, **`ReTargetByAggroEx`**: the
attacked count (`+0x28c`) goes up by one, and once the retargeting time has come (`+0xcc4`), unless a vision fix holds the target
(`+0x2b8`), the player with the **most aggro** in the damage record (alive, not of the monster's country, visible, targetable, within
**45** of the monster: `GetMaxAggroEx`) becomes the target; the next time is now + the term (5000 ms, `LuaSetReTargetingTerm`).

## 12. The state updates

Confirmed, `CMob::WhileIdle` 0x4b52c0, `WhileCombat` 0x4b55c0, `WhileMove` 0x4b5790, `WhileReturnHome` 0x4b5c40.

- **`WhileIdle`**: unless paused (`+0xcc8`), `FindTargetEx`; a player found: **event 0** with the player's id. Otherwise, a monster that
  walks (not an obelisk, `+0x7c`) whose walk time (`+0xcbc`) has come: away from home (as in section 11) → **event 4**; else, **unless
  holding position**, the ordinary idle step (`docs/original/mobs.md` section 6: `AI` 4, move speed 255, no `byNormalStep`, hold
  position: no step).
- **`WhileCombat`**: nothing while asleep, stunned or stopped (`+0x24c`, `+0x248`; the abnormal-state check of `+0x240` is always true).
  **Only-Lua-attack set** (`+0x290`): only the hook `WhileCombat(now, HP percent, attacked count)`. Otherwise: **before the next attack
  time** (`+0xcb4`): the hook; **at or after it**: the target (`+0x12c`) looked up in the zone; gone or dead: the vision fix removed
  (`RemApplySkillVisionFix`) and `SetReTarget`; else **`AttackAIExp`** (section 13), `SetReTarget` when it fails. **The hook is not
  called on a turn that attacks.** HP percent = `round(HP / max HP × 100)`.
- **`WhileMove`** (to the move target `+0x2a4`, y ignored): move types **2 and 3** first look for a player (`FindTargetEx`): found →
  event 0. Before the next step time (`+0xcb8`): nothing. Arrived (**less than `5 + rand() % 5`** away, or on a path and less than 0.5):
  event 6, the home becomes the current spot (`+0xc88`), the chase path cleared; then for move type **3**: **`CMob::ResetAttack`**
  (HP, SP and MP full, attack times and modifiers cleared, `docs/original/mob-fight.md` section 5) and the damage record cleared;
  **otherwise the hook `OnMoveEnd(now)`**. Not arrived: a
  **walking** step (motion 0) of `g_ConstMobMoveSpeed[speed] + byNormalStep` towards it on a free line (blocked: back along the chase path, else
  an A* search, `docs/original/pathing.md` section 5), next step after `dwNormalTime`, scaled for a short last step; no way at all:
  event 6 as if arrived.
- **`WhileReturnHome`** (to the home `+0xc88`): as `WhileMove` with **running** steps (motion 1) of `g_ConstMobMoveSpeed[speed] +
  byChaseStep` every `dwChaseTime`; within **10** of home (or 0.5 on a path): **`CMob::ResetAttack`**, the record and chase path
  cleared and **event 2**; stuck: **event 2** (the home becoming the current spot when not even the first step is free).

## 13. The fight

Confirmed, `CMob::FindTargetEx` 0x4b6330, `SetTargetEx` 0x4b65e0, `AttackAIExp` 0x4b66f0, `ChaseExp` 0x4b6be0, `SetReTarget` 0x4b62c0, `GetMaxAggroEx` 0x4b74f0.

Close to the ordinary fight (`docs/original/mob-fight.md` sections 3 and 4), with these differences:

- **`FindTargetEx`**: only when the zone has players. The 5 × 5 cells around the monster's cell, in order; the **first** player who is
  alive, not in a shop, not hidden (`+0x1354`, `+0x1366`), visible and targetable (`+0x5774`, `+0x5775`), **within the definition's
  `byChaseRange` (`+0x69`) of the monster** and **within 45 of its home** gets `SetTargetEx`. **No country check** (a Lua monster may
  take a player of its own country here; `AttackAIExp` then drops it, below).
- **`SetTargetEx(player)`**: nothing while a vision fix holds the target (`+0x2b8`). Otherwise: in battle (`+0x128` = 1), the target,
  the no-target timer (`+0x280`) cleared; and, unless the status is **2** (dead), each attack entry with a skill gets its ready time
  **`now + rand() % (wResetTime × 1000)`**, entry 0 and entries without a reuse time **0**. (The calls for help of `byAI` 2, 3, 5 cannot
  happen here.)
- **`AttackAIExp(now, player)`**: the target of the monster's country, dead, hidden, invisible or not targetable: the vision fix removed,
  fail. Otherwise the entries **2 down to 0** with an attack type (`+0x82`), ready, and allowed by their HP rule (`byOK`/attack type 3:
  HP between 20 % and 40 %, 4: at most 5 %, 5: below 20 %); none: **next attack = now + `g_ConstMobAttackSpeed[speed]` + 2500**, success.
  For the first usable entry: **a monster that does not walk** (`AI` 4, no `byChaseStep`, an obelisk, **or holding position**): a heal
  (`byTypeEffect` 2) is used at once; any other entry needs the target in reach (`EnableAttackRange`), else, for a skill, the entry waits its
  reuse time and **fail**, for a normal attack fail; **a walking monster**: `ChaseExp` towards the target first (in reach → go on; not →
  success, try later). Then a normal attack (`CMob::Attack`, the ordinary blow, next attack after entry 0's `dwTime`) or **the skill**
  (`CMob::UseSkill`), the entry's ready time = now + its reuse time (+ the attack speed for a normal attack).
- **`ChaseExp`** = the ordinary chase (`docs/original/mob-fight.md` section 4 and `docs/original/pathing.md` section 3: in reach, steps, the
  path kept, A* when blocked, partial waits; **full steps wait `dwChaseTime − 400`**), except
  how it gives up: unless a vision fix holds, **the monster more than 45 from home** or **the target more than 45 from the monster**,
  and the blocked cases: out of battle, the chase path cleared, **move target = home, move type 3**, event 5 (7 when a move was already
  going on, the type unchanged). The monster then walks home as a *move* (section 12), not as *going home*.
- **`SetReTarget(now)`**: with no no-target time yet: `FindTargetEx`; nobody → the no-target time = now + **3000**. With one: once past,
  **give up**: the record and chase path cleared, out of battle, **event 2**; before it, `FindTargetEx` again.

## 14. The hooks

Confirmed, `CMob::LuaCall` 0x4b1010…0x4b1210, `CMob::OnAttacked` 0x4b6120, `OnAttackable` 0x4b6170, `OnNormalReset` 0x4b61c0, `OnDeath` 0x4b6220, `OnReturnHome` 0x4b6270, `OnMoveEnd` 0x4b60f0, `WhileCombat`.

Called by name (`lua_getglobal`); a name that is not a function is logged and skipped; a run-time error is logged (`LuaPrintError`,
which also clears only-Lua-attack). Times are `GetTickCount` milliseconds (a `u32`, as a Lua number). Every call needs `+0x284` (a
loaded script).

| hook | called | arguments | context (`+0x2b4`) |
|---|---|---|---|
| `Init()` | entering the zone (section 9) | — | 0 |
| `OnAttackable(time, char id)` | transition, event 0 | the player found | 2 |
| `OnAttacked(time, char id)` | transition, event 1 | the attacker | 1 |
| `OnNormalReset(time)` | transition, event 2; first only-Lua-attack, the no-target time and the current entry are cleared | — | 3 |
| `OnDeath(time, attacked count)` | transition, event 3 | `+0x28c` | 4 |
| `OnReturnHome(time, attacked count)` | transition, event 4 | `+0x28c` | 5 |
| `OnMoveEnd(time)` | `WhileMove` arriving (move types other than 3) | — | 0 |
| `WhileCombat(time, HP %, attacked count)` | `WhileCombat` (section 12) | | 0 |

The attacked count is never cleared while the monster lives.

## 15. The functions

Confirmed, `LuaMob::*` 0x4b4570…0x4b4ba0 for the arguments, `CMob::Lua*` 0x4b1570…0x4b3520.

Arguments by `lua_tointeger` / `lua_tonumber` (a missing one reads 0). "The view by radius" = every player in the zone cells from
`trunc((pos − radius) / 40)` to `trunc((pos + radius) / 40)` (clamped) whose horizontal distance to the monster is **≤ radius**.

| function | does | returns |
|---|---|---|
| `LuaSay(text, radius)` | `0x1109` **kind 1**, the monster's id, `len = strlen(text)` (only 1 to 127), the text with its NUL in a 128-byte buffer: **136 bytes**, to the view by radius | — |
| `LuaSayByIndex(index, radius)` | **`0x110A`**: `u16 op, u32 monster id, u16 index` (**8 bytes**), to the view by radius; the client prints sysmsg `index` | — |
| `LuaSayByVoice(file, radius)` | **`0x110B`**: `u32 monster id`, the name in 32 bytes (`strncpy`, last byte 0): **38 bytes**, to the view by radius | — |
| `LuaGetName(char id)` | the player of the zone's name, `""` when not there | string |
| `LuaCreateMob(id, count, x, z)` | definition must exist (id, HP, level); count below 1 = 1; `(x, z)` both 0 = the monster's own spot, else `(x, own y, z)`; **`CZone::MobGenEx`**: up to `10 × count` tries, each at the spot **± `rand() % 20 − 10`** in x and z; a try off the map or blocked uses the exact spot instead, and when that is blocked too it costs a try without counting; each new monster (`CMob::Init` with no spawn entry: **never respawns**) enters the zone | the new monster's id when exactly one was made, else 0 |
| `LuaDeleteMob(id, count, x, z)` | definition must exist; `(x, z)` both 0 = the monster's own spot and cell, else `(x, own y, z)` and its cell; **`CZone::MobRemove2`**: **every** monster of that id (count ignored) not dead, in the cells **± 5** around, within **100** of the spot: status 2, Lua ones get **event 3** (`OnDeath` runs), removal time now: the next body pass (≤ 1 s) sends `0x0602`; **no `0x0606`, no experience, no drops** | — |
| `LuaAttackAI(skill, char id)` | target = the player (0: the current target), in the zone, not of the monster's country, alive, visible, targetable; else the vision fix removed and `SetReTarget`. Skill 0: only when entry 0 has no skill: the **normal attack** with entry 0; else the skill (`CGameData::GetMobSkillInfo(skill)`, no entry, **no reuse time, no HP rule**). A monster that does not walk (or holds position): a heal (`byTypeEffect` 2) at once, else in reach → the attack, out of reach → `SetReTarget`; a walking one: `ChaseExp`, then the attack when in reach | — |
| `LuaSetTarget(char id)` | the target, unless a vision fix holds it (no check at all) | — |
| `LuaSetReTargetingTerm(seconds)` | term = seconds × 1000 when it differs and is at least 1000 ms; the next retargeting time moves by the difference | — |
| `LuaSetOnlyLuaAttack(v)` / `LuaSetHoldPosion(v)` / `LuaSetUnBeatable(v)` | `+0x290` / `+0x294` / `+0x298` = v. **Unbeatable**: every player blow and skill **misses** (`CUser::AttackSuccess` 0x44d017, as for a monster going home), and damage over time does nothing (`CUser::ChkProtect`) | — |
| `LuaResetAggro()` | every damage-record entry's aggro = 0 | — |
| `LuaGetMinAggro()` | the player with the **least** aggro among the record's players in the zone, not of its country, alive, visible, targetable, within 45 | char id or 0 |
| `LuaGetNumberAggro(n)` | n = 0: 0; n = 1: the one with the most aggro (as `GetMaxAggroEx`); n ≥ 2: the qualifying players (as above) sorted by aggro, and, when there are at least n, **always the second** (bug) | char id or 0 |
| `LuaGetMobCountByType(id, x, z)` | the monsters other than itself, not dead, of that id (0: any), in the cells ± 5 around its own cell or `trunc(x / 40), trunc(z / 40)` | count |
| `LuaGetMobHP(id)` | 0: its own HP %, else that monster's in the zone (0 when not there), `HP / max × 100` (a float) | number |
| `LuaGetMobPos(id)` | that monster's x and z (0, 0 when not there) | two numbers |
| `LuaRecover(kind, amount)` | only amount > 0, and HP below max: kind 0 adds `min(amount, max)`; else adds `round(max × min(amount, 100) / 100)`; capped; an increase sends **`0x0610`** (`u32 id`, the amount gained) within 60 | — |
| `LuaRecallUser(byte, radius, map, x, y, z)` | the destination map must exist and `(x, z)` be on it and free; every player in the view by radius, alive, visible, targetable, not hidden, whose byte `+0x134` equals the first argument (**job**, inferred from its uses), with no move pending: a move in 100 ms to that map at the spot ± `rand() % 4 − 2` (x, z; the exact spot when that is off or blocked); `(x, y, z)` all 0 = the monster's spot | — |
| `LuaAttackOrderSurroundMob(char id, x, z)` | only with players in the zone; the player (0: … the current target, both branches look the same id up) alive: every other monster of status 0 or 1 in the cells ± 5 around its own cell or `(x, z)`'s, **within its `byChaseRange` of the player**: `SetTargetEx(player)` and, unless dead, **status 1** with its walk time cleared | — |
| `LuaUpdateInsZonePortal(id, country)` | section 5 | — |

## 16. The eight scripts (`Data/Lua/111.lua` … `118.lua`, code page 949, comments Korean)

- **111, 112, 113, 114** (floor bosses A to D, 2464, 2466, 2467, 2465): `OnDeath` → **`LuaUpdateInsZonePortal(1001, 0)`** (opens "Section
  4" in the zone). 114 also: in combat, at HP ≤ 80 % (and > 60) the skill **334**, ≤ 60 % **335**, ≤ 40 % 334, ≤ 20 % 335, each once
  (`b` counts, reset by `OnNormalReset`).
- **115** (2471, the summoner): `Init` → **unbeatable**; `OnAttacked`/`OnAttackable`, first time: `LuaSayByIndex(6401, 500)` ("Do you
  think you can leave this place while you still breathe?"), `bMove = 1`. `WhileCombat`: once `dwNextSayTime ≥ 5000` (always true after
  the first line: it is a time, not a delay; bug) and fewer than 2 lines said: `LuaSayByIndex(6402, 500)` ("Guards! Kill them!"); then,
  once: **two of the four floor bosses** at random (`math.random(6)` picks a pair) at **(220, 180)**, and with `bMove` 1:
  **`LuaFixedMove(1, 173, 179)`**. `OnMoveEnd` → hold position and **`LuaDeleteMob(2471, 1, 0, 0)`**: it removes itself (and any other
  2471 within 100). `math.randomseed(os.time())` at load.
- **116** (2470): from HP ≤ 50 %: once `LuaSayByIndex(6403, 200)` ("You can't win this fight.") and a time 1 s later; from then,
  once: **`LuaCreateMob(2472, 1)`** and **`LuaCreateMob(2473, 2)`** at its spot; from then on each combat turn **`LuaDeleteMob(2470, 1)`**
  (itself).
- **117** (2472): from HP ≤ 30 %: once `LuaSayByIndex(6404, 200)` ("How dare you... I will kill all of you !!"), then every 3 s
  **`LuaAttackAI(337)`**; `OnNormalReset` restarts both; `OnDeath` → **`LuaDeleteMob(2473, 2)`** (its copies within 100).
- **118** (2469): `OnDeath` → **`LuaCreateMob(2470, 1)`** at its spot.

So on the shipped data a party entering map 65 or 66 meets 2471; the first hit (it cannot be hurt) makes it speak, call two floor bosses
at (220, 180) on its next combat turn, walk to (173, 179) and vanish; killing either boss opens "Section 4". The final boss chain never
starts (section 8).

## 17. Original bugs and oddities

- **The expiry time is set once** per zone (section 6): coming back and leaving again after it releases the zone at once.
- **A party gone on arrival closes the connection** (section 4): leaving the party between `0x020A` and the arrival disconnects.
- **`LuaGetNumberAggro(n)`** answers the second-highest for every n ≥ 2.
- **`LuaDeleteMob` ignores its count** (removes every such monster within 100).
- **115's second line** fires on the first combat turn after the first (`dwNextSayTime ≥ 5000` compares a time).
- **`LuaFixedMove` is ignored inside the transition hooks** (`+0x2b4` ≠ 0: `OnAttacked`, `OnAttackable`, …); the scripts call it from
  `WhileCombat`, which has no context.
- **`FindTargetEx` takes players of the monster's own country**, whom `AttackAIExp` then drops (one wasted turn, then `SetReTarget`).
- **`104.lua` is missing** (1890 has no script).
- `LuaRecallUser` compares a job byte, an odd filter (inferred; no script uses it).

## 18. The new server

**Instance zones** (`CreateType` 3; guild houses, H, are built on them, `docs/original/guild-house.md` section 11, and so is the guild
ranking map, R, `docs/original/guild-ranking.md` section 10; G stays refused; `crate::instance`, `zone/instance.rs`):

- **Zone keys**: every zone structure (players, monsters, ground items, fields, scripts) is filed under a **zone key** (`u16`): a static
  map's key is its map id, an instance zone's is **`map | n << 8`** with `n` 1 to 255 (the lowest free for that map), so map ids stay
  below 256 (checked at start). The player's state keeps the zone key; every look-up of map data (`World`: size, blocked cells, portals,
  `CreateType`, rebirth and bind points, spawn areas, NPCs) takes the key's map (`world::map_of`), and the key's map is what reaches the
  client (`0x020B`, the party's member and position packets) and the database. Each instance zone gets **its own NPCs** from
  its map's list, with new object ids, and they go with the zone (`docs/original/guards.md` section 10).
- **No static zone** for a map with a `CreateType` (`CWorld::InitZones`): no monsters spawn there until a party's zone is made.
- **Portal use** (`0x020A` to a P map): section 3 in order (9, 4, 11, 10), the server's local time for section 2. Allowed: the party's
  zone is **found or made now** (with the map's spawn entries; its monsters spawn at the next respawn pass) and the player is **booked**
  into it, counted with its players against `EnterMaxUser`; the move is the ordinary map change (the client gets the map); the client's
  `0x0201` places the player into the booked zone. A booking ends on arrival or when the player leaves the world. A portal on the zone's
  own map (as 65's portals 1 and 2) checks the same and moves within the zone.
- **Instance portals** (country above 1000, the player's map with a `CreateType`): the zone's table of section 5 (10 pairs, unknown =
  3); refused ones get no answer.
- **Expiry**: the one-second pass releases an instance zone with no players and no bookings once its time is up (deviation 1); release
  drops its monsters (with their Lua states), ground items and fields and frees the key.
- **Logging in** on a map with a `CreateType` puts the character at the map's bind position for its country (`CMap::GetBindPos`,
  `docs/original/mob-fight.md` section 8: the rebirth point, map 47 for maps 64 to 66) at the select, before the enter-world burst.
- `world.maps` gains `create_time` (the triples), `create_min_user`, `enter_max_user`, `expire_time` (`tools/world/import.py`).

**Lua monsters** (`AI` ≥ 100; `crate::mob_lua`, `zone/lua.rs`):

- **Lua 5.1** through the `mlua` crate (`lua51`, `vendored`, `send`): the scripts run as shipped. `tools/world/import.py` stores
  `Data/Lua/*.lua` in **`world.mob_lua`** (`ai`, `source` as bytes). The base library without `dofile` and `loadfile`, the math library
  and an `os` table holding `time` alone (all the shipped scripts need is `math.randomseed(os.time())`; audit R28); a script's first
  run and each hook may take **1,000,000 Lua instructions** (an instruction hook every 10,000, counted per call), past which the call
  ends with an error naming the script, logged at error, as any Lua error ends it (the hook's `only_lua_attack` is cleared); `CMob` (the monster's id) and
  `LuaMob(CMob)`, whose methods reach the server's functions only while one of the monster's hooks runs. A monster with no script
  (1890, `104.lua`) or one that does not load keeps the stack machine and the fight **without hooks**, as in the original.
- **Apart from the extensions' scripts** (`docs/server/scripts.md`, `crate::script`): those run in Lua states of their own, one per
  extension, with another API (`game`), other limits and a thread of their own; `mob_lua.rs` and `zone/lua.rs` share nothing with them and
  are unchanged by them. An extension can spawn monsters (`game.spawn`) but never gives one a script.
- **Per monster** (`MobMap::lua`): its Lua state, its stack (section 10) and fields (section 9), made at its first Lua pass (`Init`
  then); `MobState::Lua(target)`. The status the rest of the code asks is the stack's top: a hit misses a Lua monster going home (3) or
  unbeatable; it recovers as an idle monster in state 0.
- **The pass** (`Zones::lua`, every second after the scripted monsters'): per zone, the new monsters' scripts and the waiting events,
  then each living Lua monster's top state (section 12) with the fight of section 13, built on the ordinary fight helpers
  (`fight::chase`, `entry_usable`, `mob_strike`, `mob_skills::use_skill`). Not in the 500 ms attack pass. Events a hook raises wait
  until it ends, so hooks never run inside each other.
- **Events**: a player's hit (`set_target`: section 11's conditions, `SetTargetEx`, event 1) and a death (event 3) are queued for the
  pass (deviation 4); damage that does not kill counts the attack and may change the target by aggro at once (`ReTargetByAggroEx`).
- **The functions** of section 15 with their packets (`0x1109` 136 bytes, `0x110A`, `0x110B`, `0x0610`), `MobGenEx` and `MobRemove2`;
  `LuaRecallUser` moves the players at once (the original after 100 ms). (**Built since**: the path search and the chase path for
  blocked moves, `docs/original/pathing.md` section 5; before it a blocked move ended there.)

**Deviations** (all approved):

1. **The expiry time starts afresh each time the zone empties** (the original sets it once, section 17).
2. **The zone is booked when the portal is used** (section 3's count then includes the player), so a party that breaks up on the way
   does not disconnect anyone: the player arrives in the booked zone (the original looks the party up again on arrival and closes the
   connection without one).
3. **`LuaGetNumberAggro(n)` answers the n-th highest** (the original: the second for any n ≥ 2). No shipped script calls it.
4. **A hit's `OnAttacked` and a death's `OnDeath` run at the next Lua pass** (within a second), not inside the hit: the boss's first
   line, the portal opening and the deaths' summons come up to a second later than in the original.
