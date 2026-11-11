# Monster AI 0 to 9: the built-in behaviours

This page covers the monster AI numbers **0 to 9** of the EP4.5 game server `ps_game.exe`: the column `AI` of the monster definitions
(`byAI`), which the server reads in its own code, with no script. It lists every place that reads the number, what each value does, the
fields whose meaning changes with it, and which monsters of the shipped data use each value. It was read with the disassembler and the
PDB names, and from the shipped data (`PS_GameDefs`, `Obelisk.ini`, the map files); nothing was executed. Numbers 10 to 99 (the AI state
machines) and 100 and up (the Lua bosses) are on [Scripted monsters](scripted-mobs.md) and [Party instance zones](instances.md). What
the behaviours do once a fight starts (the attack pass, chasing, going home) is on [Monsters fighting back](mob-fight.md), and walking
about on [Monsters](mobs.md).

## In short

- **The `AI` number picks the kind of brain.** 0 to 9 are built into the server, 10 to 99 run a state machine from `Data/AI/`, and 100
  and up run a Lua script.
- **Only four numbers below 10 do anything special.** **1** attacks players who come near. **2** calls nearby monsters of its own kind
  when a fight starts. **3** does both: it attacks players who come near and calls the aggressive monsters around it. **4** never walks,
  and only fights what it can reach from where it stands. **5** calls every idle monster around it when a fight starts.
- **0, and 6 to 9, are plain passive monsters**: they wander and fight back when hit, and nothing else. No code tests for 0 or for 6 to 9.
- **The shipped data uses 0, 1, 2, 3 and 5.** Nothing uses 4 or 6 to 9. Starter-zone animals and treasure boxes are 0. Most field
  monsters are 1. Dungeon bosses, named monsters and the obelisk towers are 3. Two goblin spirits are 2. Some obelisk guards and the
  monsters of map 58 are 5.
- **A call for help happens only once per fight**: when the monster goes from idle to fighting. The helpers do not call others.
- **Whether a monster walks is not only its AI.** Monsters with no walking or chase step, and the obelisk guards, stand still too.

Status words: **confirmed** = read in the code (address given), **inferred** = follows from confirmed code but is not shown directly.
`CMob` offsets match the PDB.

## 1. Where the number is read

Confirmed, PDB (`stMOBInfo`, 268 bytes; `CMob::m_pInfo` at `+0xa8`).

`byAI` is the byte at **`+0x26`** of the monster definition (`stMOBInfo`), loaded from the column `AI` by `CGameData::LoadConstMob`.
`CGameData::GetMobInfo` (0x4026d0) hands out a pointer into a heap array, so every use of the number is a byte read at `+0x26` through a
pointer. A scan of the whole executable for such reads, keeping those in monster code, gives this complete list:

| function | address | what it tests | what follows |
|---|---|---|---|
| `CMob::Init` | 0x49022b, 0x49026d | `10 ≤ AI < 100`; `AI ≥ 100` | the AI state machine of that number (`CAIFactory::SetFSM`); a Lua script (`CMob::InitLuaFSM`) |
| `CMob::ResetFSM` | 0x492348 | the number | which machine's variables are cleared (`CAIFactory::ClearVariable`) |
| `CMob::Targeting` | 0x49fcf9 | 1 or 3 | looks for a player (`CMob::FindTarget`), section 3.1 |
| `CMob::SetTarget` | 0x49fc39 | 2, 3, 5 | the call for help, section 3.2 |
| `CMob::FindHelpAttack` | 0x4a060b | the **helper's** AI is 1 or 3 | who answers an AI 3 call, section 3.2 |
| `CMob::AttackAI` | 0x4a1be9 | 4 | the attack of a monster that does not walk, section 3.3 |
| `CMob::UpdateIdle` | 0x4a7fe1 | 4 | no idle step, section 3.3 |
| `CMob::SetTargetEx` | 0x4b662a | 2, 3, 5 | the call for help of a Lua monster (never matches: their AI is 100 or more) |
| `CMob::WhileIdle`, `LuaAttackAI`, `AttackAIExp` | 0x4b538a, 0x4b2681, 0x4b6809 | 4 | the Lua monsters' "does not walk" test (never matches in the data) |
| `CMob::LuaRegister` | 0x4b14b2 | the number | the script's file name, `.\Data\Lua\<AI>.lua` |
| `CServerApp::OnConsoleCommand` | 0x4035d3 | | `/aiset` writes the byte (section 6) |
| `CGameData::ReLoadConstMob` | 0x4c2a62 | | `/remob` writes the byte (section 6) |

So below 10 the server knows exactly **1, 2, 3, 4 and 5**. Nothing compares the number with 0 or with 6 to 9.

**`Data/mob_ai.ini`** lists the numbers 0 to 9 as `ai/empty` (lines `0 ai/empty` to `9 ai/empty`). `CAIFactory::LoadMobAI` keeps a list
entry for them and loads no files (`docs/original/scripted-mobs.md` section 2). `CMob::Init` asks for a machine only from 10 on, so these
lines are only placeholders that keep the list positions right.

## 2. What each value does

| `AI` | looks for players (when idle) | calls for help (when a fight starts) | walks | in the shipped data |
|---|---|---|---|---|
| **0** | no | no | yes | 1,344 definitions (155 usable), 120 usable ones placed |
| **1** | **yes** | no | yes | 1,086 definitions, 1,004 placed |
| **2** | no | **its own kind** (same monster id) | yes | 2 definitions, both placed |
| **3** | **yes** | **the aggressive ones** (`AI` 1 or 3) | yes | 199 definitions, 163 placed |
| **4** | no | no | **no** | none |
| **5** | no | **any idle monster** | yes | 23 definitions, all placed |
| **6 to 9** | no | no | yes | none |

"Walks" means the AI allows it. A monster also stands still when its walking step (`NormalStep`) or chase step (`ChaseStep`) is 0, or
when it is an obelisk guard (section 3.3). "Placed" means a spawn area, a boss group or an obelisk names it (section 5).

- **AI 0, passive.** It wanders near its home, and a player who hits it gets a fight. It never starts one.
- **AI 1, aggressive.** Every second, while idle, it looks for a player near it, and attacks the first one it finds (section 3.1).
- **AI 2, calls its own kind.** Passive. When it starts a fight, every idle monster with the **same monster id** near it joins in.
- **AI 3, aggressive, calls the aggressive.** It looks for players as AI 1 does. When it starts a fight, by its own search or because it
  was hit, every idle monster of **AI 1 or 3** near it joins in, whatever kind of monster that is.
- **AI 4, does not walk.** Passive. It does not wander, and in a fight it does not chase. It attacks only a target within reach and drops
  a target that is out of reach (section 3.3).
- **AI 5, calls anyone.** Passive. When it starts a fight, **every idle monster** near it joins in, whatever its kind or AI.
- **AI 6 to 9.** The same as 0.

Everything else about a fight (the attack entries, the skills, aggro, chasing, the 45 limit, going home, recovery) is the same for every
value: [Monsters fighting back](mob-fight.md).

## 3. The behaviours in detail

### 3.1 Looking for players (AI 1 and 3)

Confirmed, `CMob::Targeting` 0x49fcf0, `CMob::FindTarget` 0x49fd20; `docs/original/mob-fight.md` section 2.

The idle pass (every second, `CMob::UpdateIdle`) calls `Targeting` first, before any walking. That call runs `FindTarget` only for AI 1
and 3. It looks at the players in the 3 × 3 cells around the monster. The first one that is alive, has no shape, sees monsters, is
visible and attackable as staff, and is **within the monster's `ChaseRange`** (horizontal distance) becomes the target, with a call for
help (`SetTarget(player, 1)`). The monster's country is not checked here; the attack pass drops a player of its own country.

The search runs once the next new-target time is reached (5 s after the monster is created), and from then on every second while the
monster is idle.

### 3.2 Calls for help (AI 2, 3 and 5)

Confirmed, `CMob::SetTarget` 0x49fad0, `FindHelpNormal` 0x49ff70, `FindHelpAttack` 0x4a0470, `FindHelpNormal2` 0x4a0200.

**When.** `SetTarget(player, help)` makes an idle monster (state 0) fight the player. Then, **only when `help` is set**, it switches on
the monster's AI: 2 → `FindHelpNormal`, 3 → `FindHelpAttack`, 5 → `FindHelpNormal2`. A monster that is already fighting returns from
`SetTarget` at once, so **the call is made once per fight**, when the fight starts. These callers pass `help` = 1:

| caller | the event |
|---|---|
| `CUser::AttackMob` (0x44cfd2) | a player's normal blow that hits and does not kill (`docs/original/combat.md` section 3) |
| `CUser::AttackMob` (skill, 0x44feb8, 0x4502fc, 0x450544) | a skill hit |
| `CZone::UpdateTrab` (0x422bf3) | a trap or field skill (`docs/original/skills-special.md`) |
| `CUser::ChkProtect` (0x486cef, 0x486d69) | Fire Thorns |
| `CUser::TauntMob` (0x487997) | a taunt (`docs/original/skills-buffs.md` section 5.2) |
| `CMob::FindTarget` (0x49ff3f) | the monster's own search (AI 1 and 3, section 3.1) |
| `CMob::AttackAI` (0x4a1d64) | an attack entry with `byOK` 6 picking an attacker (`docs/original/mob-skills.md` section 2) |
| `CMob::DoFunction`, `PAI_AttackCommand` (0x491a67, 0x4afbee) | a script's `SelectUser` and `AttackUser` (`docs/original/scripted-mobs.md` section 5) |

A miss calls nothing (`docs/original/scripted-mobs.md` section 7). The helpers are given the target with `help` = 0, so **they do not call
others in turn**. A script's `AttackUser` order, though, uses `help` = 1, so an idle AI 2, 3 or 5 monster it reaches makes its own call.

**The checks**, the same in all three (confirmed): the zone has players; the player is of **another country than the caller** (country 2,
"both", never matches); the player is alive, sees monsters, is visible and is attackable. Then each monster in the 3 × 3 cells around the
caller (rows of z, then x) that is **idle** (state 0; for a Lua monster the top of its stack state) and **within the caller's
`ChaseRange`** (horizontal, `≤`) gets `SetTarget(player, 0)` when:

| caller's AI | function | the helper must be |
|---|---|---|
| 2 | `FindHelpNormal` | **of the same monster id** as the caller (`m_wMobID`) |
| 3 | `FindHelpAttack` | **of AI 1 or 3** |
| 5 | `FindHelpNormal2` | anything |

The caller itself is no longer idle by then, so it is not its own helper. Scripted monsters (state 100) never answer, and they never
call either, because their `SetTarget` path returns before the switch. **An idle Lua monster can answer an AI 5 call** in the original: it
is idle by its stack, and `SetTarget` hands it to `SetTargetEx` (inferred from the code; no shipped AI 5 monster stands near a Lua one).

**Distance.** The call reaches only the 3 × 3 cells (cells are 40 wide), so a `ChaseRange` above 40 can be cut short by the cells,
depending on where the caller stands in its cell (inferred). The largest `ChaseRange` of an AI 2, 3 or 5 monster in the data is 40.

### 3.3 Standing still (AI 4)

Confirmed, `CMob::UpdateIdle` 0x4a7e60 (test at 0x4a7fe1), `CMob::AttackAI` 0x4a1ab0 (test at 0x4a1be9), `CMob::EnableAttackRange`
0x4a19c0.

- **Idle**: a monster with AI 4, move speed 255, `ChaseStep` 0 or `NormalStep` 0 takes no step; it only sets its next move time to
  `now + 10000 + rand() % 30000` (`docs/original/mobs.md` section 6). An obelisk guard stops even earlier, before the "away from home"
  test.
- **Fighting**: a monster with AI 4, `ChaseStep` 0 or the obelisk flag does not chase. A normal attack needs the player **within
  `Size + Range + 0.3`** (horizontal). Out of reach, the monster **drops the target** (state 0). A skill entry is used as by anyone.

So AI 4 is the same as a definition with both steps at 0, which is how the data makes its towers and immobile bosses instead (section 5).

**An aggressive monster that does not walk** keeps finding and dropping a player who stands within its `ChaseRange` but out of its reach.
It finds the player in the idle pass, drops them in the attack pass, and finds them again a second later. For an AI 3 monster, each
new find is a new fight and so a new call for help (inferred from the two passes; the obelisk towers 69 to 80, AI 3 with chase range 35,
are such monsters).

## 4. Fields that change meaning with the AI

Confirmed, the readers of `byChaseRange` (`+0x69`): `FindTarget`, the three `FindHelp*`, and for scripts `FSM_IsExistUser`,
`LuaAttackOrderSurroundMob`, `FindTargetEx`.

| field | AI 1 | AI 2, 5 | AI 3 | AI 0, 4, 6 to 9 |
|---|---|---|---|---|
| `ChaseRange` | how far it sees a player | how far its call reaches | both | **not read** (unless a script reads it) |

The 45-unit chase limit, the reach of a normal attack and the "10 from home" rule do not depend on the AI ([Monsters fighting
back](mob-fight.md) sections 4 and 5).

## 5. In the shipped data

From `original_defs.mobs` (2,747 definitions) and the files that place monsters: spawn areas (`world.mob_area_mobs`), the boss groups
and the obelisk guards of `Obelisk.ini` (`docs/original/obelisks.md`). Names are the US client's (`Monster.SData`).

**Unusable rows.** `CGameData::GetMobInfo` returns nothing for an id with `wID` 0, HP 0 or level 0 (confirmed, 0x4026d0), so such a
definition can never be a monster. 1,189 of the AI 0 rows ("Error Monster" placeholders and the like) and 2 of the AI 3 rows (158, 159
"Mine Mouse", HP 0) are such rows. Id 0 fills the empty slots of the obelisk and boss-group lists.

| `AI` | who |
|---|---|
| **0** | the **starter-zone animals** of maps 1 and 2 (Wild Antelope 100, Wild Boar 103, the monkeys and foxes 2008 to 2018 …), the rats 156 and 157 (nine maps), the bison and boars of maps 0, 19, 20, the animals of maps 43 and 44 (Skulleron, Astenes, 1300 to 1406), and the **chests and boxes** (Treasure Box 1250 to 1252 on maps 55 to 57, the boxes 582, 769, 2132 to 2134, 2220, 2303, 2304, 2371, 2418, 2419, and the Tightly Sealed Dragon's Treasure 1970 to 1972 of the boss groups). Not placed: the goddesses 196 to 199, the "Cuty pig" 60 to 62, the event boxes 1967, 2384 to 2392 and 2397 to 2399, "Skill Test Monster" 37 |
| **1** | nearly every field and dungeon monster (982 kinds in spawn areas; `ChaseRange` 9 for 822 of them, 24 for 94), the boss-group bosses (the dragons 872 and 902; Freezing Mirage 1974 and Haruhion 1975 and their second stages 1977, 1978; Cryptic the Immortal 1716, 1816, the second stage of Cryptic One; the guild-ranking bosses 1851, 1853, 1855), the guards 874 and 904, 9 obelisk guards of map 0 (160 to 163, 260 to 263, and 68 Relic Guardian Soldier), and the Greendieta 1253 to 1256 (maps 55 to 57) and Dark Fear Troll Spirit 1642 (map 47), which do not walk |
| **2** | **2369, 2370 Silverpick Goblin Spirit** (map 20, level 30, `ChaseRange` 30): the only two |
| **3** | the **named monsters and bosses of the dungeons** (Cloron 871, Fantasma 901, Vampire Queen Mago 1022, Astaroth 964, Kimuraku 835 …: maps 3 to 17, 22, 24, 25, 27 to 29, 31, 32, 60), the whole populations of maps 37 and 61, 38 and 63, 48, 49 and 64, a few orc and goblin fighters of map 1 (118, 119, 2034, 2035, 2038, 2039), the Cloron and Fantasma followers, the Freezing and Haruhion followers, the **obelisk towers** (Idespole, Altarpole, Relicpole 69 to 80, map 0; `NormalStep` and `ChaseStep` 0), the garrisons and protectors (713 to 715, 719 to 721), the Ides and Altar stones of maps 18 and 30 (706, 707, 1056, 1057, which do not walk), and the guardian soldiers 65 and 66 |
| **5** | 16 **obelisk guards of map 0** (164 to 171, 264 to 271: the Medius and Ereon undines, bears, pixies, leopards, succubi and nightmares, `ChaseRange` 14), and the **seven Greendieta of map 58** (1259 to 1265; 1262 to 1265 do not walk) |

**Placed, by source** (distinct definitions): AI 0: 124 in spawn areas, 3 chest kinds in boss groups. AI 1: 982 in spawn areas, 11 boss
stages, 2 follower kinds, 9 obelisk guards. AI 2: 2 in spawn areas. AI 3: 125 in spawn areas, 3 boss stages, 15 follower kinds, 20
obelisk guards. AI 5: 7 in spawn areas, 16 obelisk guards.

**The client's copy.** The US client's `Monster.SData` has an AI field too (`docs/client/research-extension-features.md`). It agrees with
the server for every definition both have, except for 33: 27 that are 1 on the server are 0 in the client, 15 (one of two) and 104 are 0,
and **four bosses are 35 or 36**: 1978 Haruhion (35), 1716 and 1816 Cryptic the Immortal and 1977 Freezing Mirage (36). Those are the
`mob_ai.ini` lines `data/ai/Haruhion` and `data/ai/Freezing`, scripts no server definition uses. The client does not check the AI when the
player attacks (same page), so these differences show nothing.

## 6. Changing the AI while the server runs

Confirmed, `CServerApp::OnConsoleCommand` (0x4035a1 to 0x4035eb), `CGameData::ReLoadConstMob`, `CMob::Init` 0x490040.

`/aiset <mob> <ai>` (`docs/original/operator-console.md`) prints `cmd aiset[<mob>:<old>-><new>] ok` and writes the byte into the
definition. `/remob <mob>` reads the row again from the database, the AI with it. Every test of sections 3.1 to 3.3 reads the byte when it
runs, so **for AI 0 to 9 the change applies at once** to every monster of that definition. The split into the built-in code, a state
machine and Lua is made by `CMob::Init`, which runs at every spawn and respawn (it sets state 0, clears `m_bUsePAIS`, then asks for a
machine from 10 on). So **moving a definition into or out of 10 to 99 applies from each monster's next spawn**.

## 7. The new server

- `byAI` is `MobDef::ai` (`crates/game-server/src/mobs.rs`). `mobs::AI_SCRIPTED` (10) and `mob_ai::AI_LUA` (100) split off the scripted
  monsters.
- **Looking for players**: AI 1 and 3 in the idle pass (`Zones::idle`, `crates/game-server/src/zone.rs`), `zone::monsters::find_target`.
- **Calls for help**: `zone::monsters::set_target`, AI 2 (same monster id), 3 (helpers of AI 1 or 3), 5 (any), idle helpers within the
  caller's chase range in the 3 × 3 cells, no chain.
- **Standing still**: `MobDef::steps` (no idle step for AI 4, `NormalStep` 0 or `ChaseStep` 0) and `fight::walks` (no chase for AI 4,
  `ChaseStep` 0 or an obelisk guard), with `fight::stationary_reach` (`Size + Range + 0.3`).
- **Deviations**:
  1. **Lua monsters never answer a call for help.** The original lets an idle Lua monster answer an AI 5 call (section 3.2). No shipped AI
     5 monster stands where a Lua one does (maps 0 and 58 against the instance maps 65 to 67), so nothing changes in play.
- The aggressive search passes over players of the monster's own country, where the original takes them and drops them on its next
  attack turn (`docs/server/deviations.md`, mob-fight).

## 8. Other episodes

This section compares the EP3 build of `ps_game` (2008-01-17) and the EP5.4 build (2011-09-14) with this one, read statically; the
overview is in [`ep3.md`](ep3.md) and [`ep5.4.md`](ep5.4.md). Function names in those builds are carried over from EP4.5 (matched by the
same code shape and the calls around it).

### EP3

- **The same values** (confirmed): `Targeting` tests 1 and 3 (EP3 ps_game 0x4812a6, then `FindTarget` 0x4812d0); `SetTarget` switches on
  2, 3, 5 (0x4811e7); `FindHelpAttack` accepts helpers of AI 1 or 3 (0x481acc); AI 4 stands still in `AttackAI` (0x482de0) and
  `UpdateIdle` (0x488e76). The definition's AI is at `+0x26` here too.
- **`CMob::Init` (EP3 ps_game 0x474192)**: from AI 10 on it sets the state to **4** and the scripted flag, then asks for the machine,
  whether or not that works. There is no Lua split (EP3 has no Lua). State 4 is EP3's scripted state: `ReturnHome` sets it and then
  resets the machine (0x4888fd, 0x475ea0). That explains the state 4 that EP4.5's `SetTarget` still accepts, which nothing in EP4.5 sets
  (`docs/original/mob-fight.md` section 2; inferred).

### EP5.4

- **The same** (confirmed): `Init` splits at 10 and 100 (EP5.4 ps_game 0x4a0ce3, 0x4a0d2b), `SetTarget` switches on 2, 3, 5 (0x4b423c),
  `Targeting` tests 1 and 3 (0x4b4309), `FindHelpAttack` 1 or 3 (0x4b4b6f), AI 4 in `AttackAI` (0x4b66cc) and `UpdateIdle` (0x4bcaba),
  and the Lua paths (0x4c8929, 0x4cc28a, 0x4cd561, 0x4cd739).
- **Data**: EP5.4's `Mobs` (3,088 definitions) uses AI 0 (1,253), 1 (1,447), 2 (5), 3 (240) and 5 (23) below 10; again no 4 and none
  of 6 to 9.
