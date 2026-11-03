# The operator console

This page covers what the EP4.5 game server `ps_game.exe` (built 2009-07-15) let its operator do outside the game: the console commands
typed in its console window or sent by the PSM manager through shared memory, the switches they turn (the monster EXP event, the item
mall, the faction kill counts), and `ps_game.ini`'s `[NEW_SERVER]` setting. Then it describes what the new server builds instead: one
command set reachable from the game (level-1 staff typing `.command`) and from an operator tool on the server machine, a kill rate in
place of `NEW_SERVER`, and a kill-boost buff.

The original was read from `ps_game.exe` (SHA-256 `0fb7c9f1…`, from `PSM_Client/Bin`) in the Ghidra decompiler and the disassembly
(`tools/disasm.py`) with the PDB names (`docs/original/symbols/ps_game.tsv`). Nothing was executed.

The in-game staff commands are on [GM commands 1](gm-commands.md) and [GM commands 2](gm-commands-2.md), and the
client's side on [GM opcodes and commands](../client/gm-and-commands.md). Not covered: `SShareMemTCommandServer` (how the PSM manager writes
a command into shared memory; the command reaches `OnConsoleCommand` as a typed one does) and the manager's side (`PSM_Agent`).

## In short

- **The original console has 34 commands**: server and user counts, notices to everyone, kicking by account or character, closing and
  opening the server to new connections, the user cap, a shutdown countdown, disconnecting everyone, and reloading monster, item and
  skill rows from the database while running.
- **The EXP event** (`/exp2xenable <factor>`) multiplies the experience every monster gives, by any factor of 0 or more. Quest and
  player-kill experience are not changed.
- **The item mall can be switched off**, with or without an answer to the player. **The faction kill counts can be stopped.**
- **`/quitt` counts down** with notices to everyone (in Korean) and then ends the process; the countdown cannot be cancelled.
- **`[NEW_SERVER] Check`**, a hidden setting, doubles every counted player kill, halves the price of enchanting with a Lapisia and
  refuses moves to map 200. Switches set by a command last until the server restarts.
- **The new server has one command set with two ways in**: level-1 staff type `.command` in the game (the line is taken by the server,
  nobody sees it, the answer is a whisper from `Server`), and the `console` tool on the server machine sends the same commands through
  PostgreSQL `NOTIFY`.
- **`NEW_SERVER` is not built.** A kill rate (`GAME_KILL_RATE`, `.killrate`) replaces it, and a kill-boost buff can multiply counted
  kills (an addition, off by default).
- **`reload`** puts changed monster and drop tables in force while running; **`restart`** is a shutdown countdown that ends with exit
  status 75 so that a supervisor starts the server again.

Status words: **confirmed** = read in the code (address given).

## 1. The console

Confirmed, `CServerApp::InitConsoleCommand` 0x402a60, `CServerApp::OnConsoleCommand` 0x402ce0.

`InitConsoleCommand` registers each word with `SConsole::AddCommand(id, word, argument count, …)`; `OnConsoleCommand` switches on the
id (jump table 0x4038ec, 48 entries) and writes its answer text into the reply buffer (`sprintf`, the `cmd …` strings). Ids 0x10 and 0x13
have a case but **no registered word** (unreachable). Ids 0x1b to 0x21, 0x23, 0x24, 0x26, 0x27 do nothing.

| id | word | args | what it does (verified) |
|---|---|---|---|
| 0x00 | `/mem` | 0 | `SServerTBase::HeapMemInfoGet`: heap statistics into the reply |
| 0x01 | `/si` | 0 | `SAgent::GetStatistics` → "cmd server info …" plus "connection is start" or "connection is stop" (the acceptor's flag 0xfded4c) |
| 0x02 | `/quitt [minutes]` | 1 | the shutdown countdown, section 1.1; ends the process |
| 0x03 | `/allout` | 0 | `SAgent::DisconnectAll`: every connection closed (each saved as on a lost connection); the server keeps running |
| 0x04 | `/uc` | 0 | "cmd User count : %d (%d/%d)": the players in the world (`g_pWorld+0x88`), the connections (0xfdeb80) and the maximum (0xfdeb74) |
| 0x05 | `/nt <text>` | 1 | `CWorld::SendNotice(text)`: **`F106 u16 len, text`** (len 2 to 2000, the last byte forced to 0) to everyone (`CWorld::SendAll`); "cmd send notice ok" |
| 0x06 | `/cstop` | 0 | `SAcceptor::StopAcception`; tells the manager `0x0204` (server id, 4, 0/1) |
| 0x07 | `/cstart` | 0 | `SAcceptor::StartAcception`; the same `0x0204` |
| 0x08 | `/setmaxuser <n>` | 1 | 0 ≤ n < 30000: the maximum (0xfdeb74) = n, `0x0806` to the manager; else "failed" |
| 0x09 | `/kickun <account>` | 1 | `CWorld::FindUserBill(name)` → `Close(9)`; "cmd kick by user name(str) ok! user:%s(%d)-char:%s(%d)" |
| 0x0a | `/kickuid <n>` | 1 | the same by account id (n > 0) |
| 0x0b | `/kickcn <name>` | 1 | `CWorld::FindUser(name)` (a character) → `Close(9)` |
| 0x0c | `/kickcid <n>` | 1 | the same by character id |
| 0x0d, 0x0e | `/nprotecton`, `/nprotectoff` | 0 | `g_bUseNProtect` = 1 / 0 (GameGuard) |
| 0x0f | `/chktimeout` | 0 | prints `g_nSTimeover`, `g_nDTimeover`, `g_nWTimeover` (send, database, world timeouts counted) |
| 0x10 | (none) | | toggles `g_bChkItemPick` |
| 0x11 | `/exp2xdisable` | 0 | `g_nExp2XbyAdmin` = 0 |
| 0x12 | `/exp2xenable <f>` | 1 | `f = atof(arg)`; **f < 0 → "failed"**; else `g_nExp2XbyAdmin` = 1, `g_fExp2XValue` = f (a double; 1.0 at start) |
| 0x13 | (none) | | `g_nExp2XbyAdmin` = 2 ("auto") |
| 0x14 | `/enableshop` | 0 | `g_bDisableShop` = 0 |
| 0x15 | `/disableshop` | 0 | `g_bDisableShop` = 1 ("with msg") |
| 0x16 | `/disablenshop` | 0 | `g_bDisableShop` = 2 ("no msg") |
| 0x17 | `/enablekill` | 0 | `g_bDisableKillCount` = 0 |
| 0x18 | `/disablekill` | 0 | `g_bDisableKillCount` = 1 |
| 0x19 | `/ticktime` | 0 | the tick count and its days |
| 0x1a | `/inszonecnt` | 0 | "cmd CurAll(%d), guildhouse(cur=%d,max=%d), guildrank(cur=%d,max=%d), party(cur=%d,max=%d)": the instance zones |
| 0x22 | `/viewmap <map>` | 1 | `CWorld::GetMapStatus(map)`: `PrintZoneStatus` of each of the map's zones into the log file; "ok"/"fail" |
| 0x25 | `/aiset <mob> <ai>` | 2 | the monster definition's AI byte (`+0x26`) = ai, live |
| 0x28 | `/remob <mob>` | 1 | `CGameData::ReLoadConstMob`: that monster's row read again from the database |
| 0x29 | `/remobitem <mob>` | 1 | `ReLoadConstMobItem`: its drops |
| 0x2a | `/reitem <id>` | 1 | `ReLoadConstItem` (type = id / 1000, type id = id % 1000) |
| 0x2b | `/reskill <id> <level>` | 2 | `ReLoadConstSkill` |
| 0x2c | `/remobskill <id>` | 1 | `ReLoadConstMobSkill` |
| 0x2d | `/reseqitem` | 0 | `CItemDrop::ReSequenceItemCreate` |
| 0x2e | `/reitemenh` | 0 | `CGameData::ReItemEnchant` |
| 0x2f | `/reitemcre` | 0 | `CItemDrop::ReItemCreate` |

### 1.1 `/quitt`

Confirmed, 0x402df4.

`SAcceptor::StopAcception`; `t = atoi(arg) × 60000` ms, at least **30000**; then the notice "잠시 후에 서버가 종료됩니다. 모두 접속
종료해주십시오." ("The server will shut down shortly. Please log off, everyone.") through `CWorld::SendNotice`, and until `t` has passed:
the user count printed, the notice "%d초 후에 서버가 종료됩니다. 모두 접속 종료해주십시오." ("The server will shut down in %d seconds.
…") with the seconds left, then a sleep of **1 s while 10 s or less are left, 5 s while 30 s or less, else 10 s**. Then a 1 s sleep,
"server shutdown start...", `SAgent::DisconnectAll`, and the console returns false: the process ends. The countdown blocks the console
thread (nothing cancels it).

## 2. The switches and who reads them

Confirmed.

- **The monster EXP event** (`CMob::GetExp` 0x491690): the base `exp × ((mob level − level + 1) × 0.2 + 1)` (0 when the monster is more than
  9 levels above, `combat::mob_exp`), truncated; then **with `g_nExp2XbyAdmin` 1: × `g_fExp2XValue`**, truncated again (`fistp` with the
  chop mode); with 2 ("auto", unreachable) the same only while `g_bExp2X` is 1, and **nothing writes `g_bExp2X`**; negative → 0. Every
  user of `GetExp` gets it: the killer (`× 1.2` after), the party and union shares (`CMob::Death`, `docs/original/party.md`). Quest and
  player-kill experience do not.
- **`g_bDisableShop`** (`CUser::PacketPoint`, the item mall, 0x479811 and 0x479f5c): **1**: a purchase (`0x2602`) answers its failure, a
  gift (`0x2604`) answers 6; **2**: **no answer at all** to either; 0: as usual. The new server's `GAME_ITEM_MALL=false` is the 1.
- **`g_bDisableKillCount`** (`CWorld::AddKillCountGrade` 0x413413, `AddKillCount` 0x413603, `SubKillCount` 0x413751): the faction kill
  counts stop (`GAME_KILL_COUNT`, `kc::KillCounts.disabled`). The players' own kill counters are not affected.
- **`[NEW_SERVER] Check`** (`InitLogFile` 0x403dbc, `GetPrivateProfileInt("NEW_SERVER", "Check", 0, "./Config/ps_game.ini")` →
  `g_dwNewServerCheck`; the shipped files have no such section, so 0). Read at start only. Where it is **non-zero**:
  - `CUser::KillPCTo` (0x45af76): a counted kill adds **2** instead of 1 (at most `g_nMaxKillValue` 200000);
  - `CUser::ItemLapisianAdd` (0x45fc45): the price of enchanting with a Lapisia (`GetGemMoneyByGrade`, `lapis::gem_price`) is **halved**;
  - `CUser::PacketMaininterface` (0x468ae9, 0x469002) and `PacketAdminCmdE` (0x474dbb, 0x47544f): a move to **map 200** (`g_dwUnitedWarServer`,
    the second server) answers `0x0110 2` (refused); the new server refuses map 200 always (README "Not built").
  - **Oddity**: `ChkTimeExp2X` (0x401230, from `CWorldDBThread::OnUpdate`) flips a non-zero value between 1 and 2 by the local hour (2 at
    0:00, 13:00 and 19:00 to 23:59; 1 otherwise), but every reader only tests non-zero: the hour changes nothing.

## 3. The new server now

- **Built as staff commands** (`docs/original/gm-commands.md`): `/server off|on` (`F706`/`F707`, deviations 12, 19, 20: `ctx.accepting`, the
  heartbeat lists the server as closed), `/akick` (`F709`, an account), `/kick` (`F804`, a character), the notices (`F906` to `F90B`;
  `ctx.zones.send_world` for everyone), `/set kills` (`F701` 17).
- **Settings, fixed at start**: `GAME_ITEM_MALL` (the shop), `GAME_KILL_COUNT` (the faction kill counts), `GAME_MAX_CONNECTIONS`.
- **Shutdown**: Ctrl+C / SIGTERM (`tools/game-server.sh stop`): every connection saved and closed (`server::shut_down`), at once.
- **Operator notifications** already go from the operator tools to the game server through PostgreSQL `NOTIFY` (`main.rs`
  `account_notifications`: `pg::ACCOUNT_LOGIN_CHANNEL`, `pg::ACCOUNT_POINTS_CHANNEL`).
- **What reaches the server from the client** (`docs/client/gm-and-commands.md`, "What the US client sends at each privilege"): the client knows only its own `/` words, but a
  line starting with any other character, e.g. `.`, is sent whole as normal chat: `F101 u8 len, text` from staff. A whisper from a fixed
  name (`0x1102`, `chat::whisper`) is the simplest reply.

## 4. Design decisions

1. **Both ways in**: level-1 staff type the commands in the game with a dot (`.exp 2`); the server takes the line (nobody sees it, it is
   not chat) and whispers the answer from `Server`. And an operator tool on the server machine (`console`, beside `mall` and `servers`)
   sends the same commands through `NOTIFY` and prints the answer.
2. **`NEW_SERVER` is rejected** (not built, by decision). Instead a **kill rate**: a setting `GAME_KILL_RATE` (default 1) and the command `.killrate <n>`: every
   counted player kill adds n to the killer's kills (at most 200000). The Lapisia price stays full; map 200 stays refused.
3. **A kill-boost buff**: while a player has an applied skill listed in `GAME_KILL_BOOST_SKILLS`, each of its counted kills adds
   the entry's multiplier times the kill rate (three strengths, see the end of this decision). **The item** (after research entry 7): a **new** item and skill rather than a reused
   one (no existing item is both unobtainable and the only one with its buff: the Mini Sun EXP Stone 100/108 → 250/1 is sold in the
   mall and gives EXP; 100/172 → 223/1 shares its skill with 100/43). **Item 100/212** (the empty "???177" in both copies) becomes the
   **Kill Boost Stone**, giving **skill 200 level 1**, "Kill Boost": a copy of 250/1 (kind 4, detail 82) with **no ability**, lasting **one
   hour** and **kept on death** (`fixrange` 1: the hour runs on through deaths and logouts). Skill 200's levels 1 to 3 are empty in both
   copies and no item names it; an id of 255 or less keeps the client's one-byte item link equal to the server's number (327, the
   client's next id, would read 71 in the client's item record). The setting takes **`id/level`** (or `id` for any level), default
   **`200/1`**: the monsters' skills share the ids at level 100 (200/100 is their Weakening Curse, kept in the client's separate
   `NpcSkill.SData`), and a player hit by one must not count double. The server's rows are the extension `extensions/kill-boost` (before 0.3.0: `additions.sql`); the client's copy
   (`Item.SData` 100/212, `Skill.SData` 200 levels 1 to 3, same names and values) ships in the first client data patch
   (`docs/plans/roadmap.md`, client data patches).
   **Three strengths (`docs/plans/done/client-patches.md` decision 1)**: the Kill Boost Stone (100/212, skill 200/1)
   counts each player kill **2×**, **Kill Boost Stone Lv2** (100/213, skill 200/2 "Kill Boost Lv2") **5×**, **Kill Boost Stone Lv3**
   (100/214, 200/3 "Kill Boost Lv3") **10×**, one hour each, kept through death and logout, all looking like the Mini Sun EXP Stone
   (buff icon 225). The stones' setting is `200/1:2,200/2:5,200/3:10` (entries `id[/level][:times]`; the highest matching entry
   counts), the former default; the default is now empty and the stones' rows load only with
   `data-editor ext install extensions/kill-boost` (before 0.3.0: `additions.sh --with-additions`; an addition: `docs/server/deviations.md` section 1); a stronger stone used during a weaker one ends it and starts a fresh hour, a weaker one is refused while a stronger runs
   (`buffs.rs`). The client's copy is client data patch 109.
4. **The data reloads** (`/remob`, `/remobitem`, `/reitem`, `/reskill`, `/remobskill`, `/reseqitem`, `/reitemenh`, `/reitemcre`, and
   `/aiset`, which changes the data live too): **not built, except `reload` for the monsters and the drop tables** (section 10; section 9 has the estimate for the rest). Data the client also has needs a
   client patch anyway; restarting the server takes seconds and saves everyone.
5. **Not asked, as the original**: the switches set by commands last until the server restarts (then the settings apply again).

**Additions (not in the original):** `quit cancel` (the original's countdown cannot be stopped); `info` gathers `/si`,
`/uc`, `/mem` and `/ticktime` into one line of the new server's numbers; `GAME_EXP_RATE` gives the EXP event's value at start, as
`GAME_KILL_RATE` does for the kill rate; every command is written to the game log as a staff command (`0x0C03`, action 180) and to the
server log.

**Left out**: `/nprotecton`/`/nprotectoff` (no GameGuard), `/chktimeout` (no such counters), the unregistered 0x10 and 0x13, the manager
messages `0x0204`/`0x0806` (the heartbeat does that job).

## 5. The commands (the new server)

Words are case-insensitive; arguments are separated by spaces. Each answer is one or more short lines. In the game the dot is required
(`.info`); in the tool it is optional.

| command | the original | effect |
|---|---|---|
| `help`, `help <command>` | — | `help`: every command with its arguments, one line each (`notice <text>`, `kick char <name>`, …); `help <command>`: that command's usage, what it does in one sentence and an example (`help exp` → `exp <factor> | exp off: multiplies the experience monsters give; 0 or more, 1 = normal. e.g. .exp 1.5`) |
| `info` | `/si`, `/uc`, `/mem`, `/ticktime` | `players <in world> (<connections>/<max>), accepting yes/no, up <d>d <h>h <m>m, exp x<f>, killrate <n>, shop on/off/silent, killcount on/off` |
| `notice <text>` | `/nt` | `F106` (section 1, len 2 to 2000) to every player in the world (`zones.send_world`) |
| `quit [minutes]` | `/quitt` | the countdown of 1.1 (minimum 30 s; the notices in English: "The server will shut down shortly. Please log out." and "The server will shut down in N seconds. Please log out."), then the shutdown Ctrl+C does |
| `quit cancel` | — | stops a countdown; accepting connections again if it was before; notice "The server shutdown was cancelled." |
| `restart [minutes]` | — | as `quit` (same countdown, notices and `quit cancel`), then the shutdown ends the process with exit status **75** instead of 0, so that `tools/game-server.sh` and a systemd `Restart=on-failure` start it again; answer `restarting in N seconds` |
| `reload` | `/remob`, `/remobitem`, `/reseqitem`, `/reitemcre` (in part) | reads `defs.mobs`, `defs.items` (the drop grades only), the drop tables, box sets, quest links and sequence groups again and puts them in force together; answer `reloaded: <n> monsters, <n> drop grades, <n> box sets (<ms> ms)`, or `reload failed, nothing changed: <error>` |
| `allout` | `/allout` | every connection closed as a lost connection (saved); the server keeps running |
| `close`, `open` | `/cstop`, `/cstart` | as `/server off`, `/server on` (`ctx.accepting` and the heartbeat at once) |
| `maxusers <n>` | `/setmaxuser` | 0 ≤ n < 30000: new connections are refused while the open connections are n or more (those open stay); at most `GAME_MAX_CONNECTIONS` takes effect (the semaphore's size) |
| `kick user <account>`, `kick userid <n>`, `kick char <name>`, `kick charid <n>` | `/kickun`, `/kickuid`, `/kickcn`, `/kickcid` | that connection closed as `/akick` does (saved, no packet); answer `kicked user:<account>(<id>) char:<name>(<id>)` or `not found` |
| `exp <f>`, `exp off` | `/exp2xenable`, `/exp2xdisable` | f ≥ 0 (as the original; 0 gives no monster experience): every monster's experience × f (section 2); `off` = × 1; answer `exp x<f, 3 decimals>` |
| `killrate <n>` | (replaces `NEW_SERVER`) | 1 ≤ n ≤ 100: each counted player kill adds n (× the kill-boost buff's multiplier: 2, 5 or 10) |
| `shop on`, `shop off`, `shop silent` | `/enableshop`, `/disableshop`, `/disablenshop` | the item mall: as usual / purchases and gifts answer their failure (as `GAME_ITEM_MALL=false`) / no answer at all |
| `killcount on`, `killcount off` | `/enablekill`, `/disablekill` | the faction kill counts (`kc::KillCounts.disabled`) |
| `zones` | `/inszonecnt` | the instance zones now open, by kind (party, guild, guild house, …: what `instance.rs` keeps), and their total |
| `map <id>` | `/viewmap` | each zone of that map (the static one and its instances): players and monsters |

A wrong word or wrong arguments: `unknown command <word>; try help` or, for a known word, the same text as `help <command>`. Nothing is changed then.

## 6. Build plan

The plan the code was written from. Section 10 lists where the code differs from it, and the code is authoritative. All in
`crates/game-server` unless said.

1. **Settings** (`config.rs`, README settings table): `GAME_EXP_RATE` (f64 ≥ 0, default 1), `GAME_KILL_RATE` (u32 1 to 100, default 1),
   `GAME_KILL_BOOST_SKILLS` (comma-separated skill ids, parse as `maps()` does; each entry `id[/level][:times]`, default empty; first planned as `id` or `id/level` with the default
   `200/1`), `GAME_CONSOLE_LEVEL` (the staff levels
   allowed to type commands, 1 to this; default 1). Refuse out-of-range values at start with the variable's name.
2. **The switches** (`server.rs` `Context`, or a small `switches.rs`): `pub struct Switches { exp_rate: AtomicU64 /* f64 bits */, kill_rate:
   AtomicU32, shop: AtomicU8 /* 0 on, 1 off, 2 silent */, max_users: AtomicU32 }` with typed getters/setters, filled from the config
   (`shop` from `GAME_ITEM_MALL`, `max_users` from `GAME_MAX_CONNECTIONS`); `pub switches: Switches` in `Context`. Then use them:
   - **EXP**: `combat::mob_exp(def, level, rate: f64)`: after the base (truncated as now), `(base as f64 * rate) as u32` (truncation, ≥ 0)
     — rate 1.0 must give exactly today's numbers (the existing tests unchanged apart from the argument); `kill_exp` passes it on; the two
     callers in `server.rs` (~1591, ~1600) pass `ctx.switches.exp_rate()`. Check `party.rs`'s shares get their base from the same call
     (they take `CMob::GetExp` at each member's level); if a share computes its own base, pass the rate there too.
   - **Kill rate and boost**: `pvp::counted(count: u32, by: u32) -> Option<u32>`: `None` at the cap (200000), else `min(count + by,
     cap)`; deaths call it with 1. `server/pvp.rs` `kill_counted`: `by = kill_rate × (2 if the killer has an applied skill listed in
     GAME_KILL_BOOST_SKILLS)`. The applied skills live in the zone (`Occupant.buffs`): add `Zones::has_skill(map, id, ids: &[u16]) ->
     bool` (or use an existing lookup) and ask it. The faction kill count (`kill_counts.add(country, 1, …)`) and the quest's player-kill
     count stay 1.
   - **Shop**: `server/mall.rs` ~116 (purchase) and ~163 (gift): replace `ctx.config.item_mall` by the switch; **silent** returns before any
     answer (the original's 2), **off** answers as today's `item_mall == false`.
   - **Max users**: where connections are accepted (`server.rs` / `main.rs` `serve`, the `connections` semaphore), also refuse (close at
     once, logged as the full server is) when `max_connections − connections.available_permits() >= switches.max_users()`.
   - **Kill counts**: `kill_counts.lock().disabled` is already the switch.
3. **`console.rs`** (new, pure; `pub mod console;`): `pub enum Command { Help(Option<String>), Info, Notice(String), Quit(Option<u32>), QuitCancel,
   AllOut, Close, Open, MaxUsers(u32), Kick(KickBy), Exp(f64), ExpOff, KillRate(u32), Shop(u8), KillCount(bool), Zones, Map(u16) }`,
   `pub enum KickBy { User(String), UserId(u32), Char(String), CharId(u32) }`, `pub fn parse(line: &str) -> Result<Command, String>`
   (a leading `.` is dropped; the error is the answer of section 5), `pub fn help() -> Vec<&'static str>` (one line per command) and `pub fn help_for(word) -> Option<&'static str>` (usage, one sentence, an example; one table of `(word, usage, sentence, example)` feeds both and the parse errors), and the countdown's timing
   `pub fn next_notice(left: Duration) -> Duration` (1 s at ≤ 10 s, 5 s at ≤ 30 s, else 10 s) and its texts. Unit tests: every command and
   its errors, `exp -1` refused and `exp 0` accepted, `killrate 0` and `101` refused, `maxusers 30000` refused, the countdown steps.
4. **`server/console.rs`** (new): `pub async fn run<S: Backend>(ctx: &Arc<Context<S>>, command: Command, by: &str) -> Vec<String>` (the
   answer lines; `by` names who sent it for the logs):
   - `Info`: the numbers of section 5 (`ctx.players`, the semaphore, `ctx.accepting`, `ctx.started`, the switches).
   - `Notice`: a new `shaiya_proto::admin::world_notice(text) -> Vec<u8>` (`F106 u16 len, text, 0`; len counts the zero; refuse an empty
     text or one above 1999 bytes) through `ctx.zones.send_world`.
   - `Quit`: `Context` gets `quit: Mutex<Option<tokio::task::JoinHandle<()>>>` and `quit_now: tokio::sync::Notify`. A countdown already
     running → "a shutdown is already counting down". Else stop accepting (remember whether it was), spawn the task of section 1.1
     (notices through `world_notice` and `send_world`), and at the end `quit_now.notify_one()`. `main.rs`: the `select!` waits on
     `ctx.quit_now.notified()` as on `stop_requested()`, so the end is the same save-and-close shutdown. `QuitCancel` aborts the task,
     restores accepting and sends the cancel notice; with none running → "no shutdown is counting down".
   - `AllOut`: `OnlineUsers::kick_all(Kick::Operator { op: 0 })` (new; every account with a connection here), answer the count.
   - `Close`/`Open`: the code of `F706`/`F707` (`server/admin.rs` ~163) moved into a shared `set_accepting(ctx, on)`.
   - `Kick`: by account name/id → `OnlineUsers::kick(user_id, Kick::Operator { op: 0 })` (the account id from `ctx.store.account_id`
     for a name); by character name/id → its account (`zones.find_by_name` / `zones.locate`, then the character's account) and the same
     kick. Answer as section 5.
   - `Exp`, `ExpOff`, `KillRate`, `Shop`, `KillCount`, `MaxUsers`: set the switch; answer the new value.
   - `Zones`, `Map`: read what `instance.rs` and the zones keep (counts only; take each lock briefly, in the documented lock order).
   - Every command: `tracing::info!(by, command = line, "console command")` and a game log row `game_log::staff_command(header, level,
     ".<word>", None, Some(args))` (the in-game sender's header and level; the tool's: `Header::none()`, level 0).
5. **In the game** (`server/admin.rs`, the `F101` (`OP_CHAT_NORMAL`) branch): when the GM's level is 1 to `GAME_CONSOLE_LEVEL` and the
   text, after a leading chat colour marker (`<<` and 6 more bytes, `docs/client/gm-and-commands.md` 5a; skip it when present), starts
   with `.`: parse and run it, and send each answer line as a whisper **from `Server`** to the GM (`chat::whisper`, the receiving side's
   form; cut lines at 120 bytes). The line is not chat: not shown, not copied to listeners, not in the chat log. Other staff levels and
   players (`0x1101`) keep today's behaviour for `.` lines (normal chat).
6. **The operator tool**: `pg.rs` constants `CONSOLE_CHANNEL = "game_console"`, `CONSOLE_REPLY_CHANNEL = "game_console_reply"`; the
   payload `<server id> <request id> <command line>` and the reply `<request id> <lines joined by \n>` (keep a reply under 7900 bytes:
   `NOTIFY`'s limit is 8000). `main.rs` `account_notifications` listens on `CONSOLE_CHANNEL` too: a payload for another server id is
   ignored; else parse, `run`, and `SELECT pg_notify($1, $2)` the reply (a parse error is the reply). New
   `crates/login-server/src/bin/console.rs` (beside `mall.rs`, same style and `.env` handling): `console <server id> <command…>`:
   `LISTEN game_console_reply`, then `pg_notify('game_console', …)` with a random request id, print the reply with that id, give up after
   5 s with "no answer (is game server <id> running?)". Usage in the file's doc comment and in the README.
7. **Tests**: `console.rs` units (step 3); `combat.rs`: `mob_exp` with rates 1.0, 2.0, 0.0 and 1.5 (truncation); `pvp.rs`: `counted(199_999,
   2)` = 200000, `counted(200_000, 1)` = None. `tests/game_flow.rs` (memory backend): a level-1 GM sends `F101 ".info"` and gets a
   whisper from `Server` starting with `players`; `.killrate 3`, then the GM's kill of a player in a PvP map adds 3 (or, if a full kill is
   heavy there, test `kill_counted`'s amount through a smaller unit); a level-5 GM's `.info` is ordinary chat; `.quit` then `.quit
   cancel` answers both and leaves the server accepting. The NOTIFY path: a PostgreSQL test in `tests/postgres.rs` (skips without
   `DATABASE_URL`) if a short one is possible, else note it untested.
8. **Docs**: a "Built" section 10 here (commits, what changed from this plan); README: the settings, the `console` tool usage, "Not built" (the
   data reloads: maybe, to discuss later; `NEW_SERVER` rejected, the kill rate instead); `docs/original/pvp.md` (the top summary, sections 7 and 9: the
   `NEW_SERVER` notes: "not built by decision; the kill rate and kill boost of `docs/original/operator-console.md` instead") and
   `docs/original/lapis.md` (sections 5 and 11, the `NEW_SERVER` half price: not built by decision). Live test: **section 51** of
   `docs/testing/live-test-checklist.md`.

## 7. Play test list

Also `docs/testing/live-test-checklist.md` section 51. A level-1 test character in the game; a shell on the server machine for the tool.

- [ ] `.help` → whispers from `Server`, one per command; `.help kick` → its usage, sentence and example; `.kick` alone → the same text; `.foo` → "unknown command foo; try help". `.info` → `players 1 (1/…)`, `accepting yes`, the uptime, `exp x1.000`,
      `killrate 1`, `shop on`, `killcount on`. The line itself is not seen by a second client standing next to the staff character.
- [ ] `.notice Hello` → every player sees "[Notice] Hello" (also on another map).
- [ ] `.exp 2` → `exp x2.000`; a monster gives twice the experience it gave before (compare one kill before and after); `.exp off` → back.
- [ ] `.killrate 2`, a PvP kill (duel-free, on a PvP map) raises the killer's kills by 2; `.killrate 1` → by 1.
- [ ] `.shop off` → an item mall purchase fails with the usual message; `.shop silent` → the purchase gets no answer (the client waits);
      `.shop on` → it works.
- [ ] `.close` → the login server lists the server as closed, a new login is refused; `.open` → open again.
- [ ] `.kick char <name>` → that character's client is disconnected, the character saved (relog: where it was).
- [ ] `.quit 1` → "shortly" then "in N seconds" notices every 10 s, 5 s near 30 s, each second at the end; `.quit cancel` before the end
      → the cancel notice, logins work. (Optionally let one run to the end: the server stops as with `tools/game-server.sh stop`; start it
      again.)
- [ ] The tool: `cargo run -p login-server --bin console -- 1 info` prints the same line; `… -- 9 info` (no such server) → "no answer".
- [ ] A level-5 GM's `.info` is ordinary chat. The game log has a 180 row per command (T1 `.info` …).
- [ ] Kill boost (decision 3): see `docs/testing/live-test-checklist.md` section 51 (the Kill Boost Stone 100/212, after the client data patch).

## 8. Client

Nothing for the commands (they ride on chat and whispers the client already has). The kill-boost item is new in the client's data
(decision 3; research entry 7 answered in `docs/client/research-gambling-kill-boost.md`, "Entry 7"): `Item.SData` 100/212
renamed with skill byte 200, level 1, Special 0, a usable stack and an icon as 100/108's; `Skill.SData` 200 level 1 as 250/1 with
KeepTime 1 and the duration flag 1 (hours), the three abilities (0, 0), and the name and description of both. The client sends only bag
and slot to use it (`0x050A`), and the buff comes back as `0x050D` with the full id, so nothing else changes. Levels 2 and 3 of 200
carry the two stronger stones (100/213 and 100/214); the patch is built by `tools/sdata/client_patch.py`
(`docs/plans/done/client-patches.md`).

## 9. The data reloads: what a full reload would take

- **What the new server does**: `Context.data: GameData` (items, monsters, skills, quests, drops, world, the mall's products) is loaded once
  at start and read in place: **178 reads (`ctx.data.…`) in 12 files**. Some definitions are also **copied** into live objects: an
  applied skill keeps its `SkillDef` (`buffs.rs` `Applied.def`), a spawned monster its monster numbers, the zones a few derived tables
  (`Zones.item_skills`).
- **What a reload needs**: (1) `data` behind a swappable pointer (`arc_swap::ArcSwap<GameData>` or `RwLock<Arc<GameData>>`) and every
  read through it (mechanical); (2) a loader for one row of each table (`pg.rs` has the whole-table loaders); (3) a new `GameData` built
  with that row replaced, and the derived tables rebuilt (`DropTables` from the items); (4) for the live copies, an order to the zones to
  refresh the monsters of that id (or accept that they change at their next spawn, and buffs at their next use). The original changes its
  one shared copy in place, so live monsters change at once.
- **Size**: a modest change for (1) to (3) with the commands, plus a smaller one for (4); low risk with `ArcSwap` (no new lock
  order). 
- **Worth it?** The parts the client also has (names, prices, stats shown, monster HP in `0x0601`, skill numbers) need a client patch
  anyway. **Server-only** data are where a live reload would really help: the **monster drop lists** (`/remobitem`,
  `world`/`defs` drop rows), the item drop grades (`/reseqitem`, `/reitemcre`), monster AI and attack numbers the client does not show
  (`/aiset`, parts of `/remob`), and the mall's products. A smaller first step could be "reload the drops" only.

## 10. As built

Steps 1 to 7 of section 6 as planned, with these points:

- **Settings and switches** as in steps 1 and 2: `config.rs` (`GAME_EXP_RATE`, `GAME_KILL_RATE`, `GAME_KILL_BOOST_SKILLS`,
  `GAME_CONSOLE_LEVEL`, refused at start when out of range), `switches.rs` (`Switches` in `Context`). `combat::mob_exp(def, level, rate)` and
  `kill_exp` take the factor (the base is cut first, then `(base × rate)` is cut; rate 1.0 skips the multiplication). Party and union
  shares take their base from the same `mob_exp` at each member's level, so the factor reaches them with no change to `party.rs`.
  `pvp::counted(count, by)`; `kill_counted` uses `kill_rate × Zones::kill_boost(map, id, GAME_KILL_BOOST_SKILLS)` (2, 5 or 10 with a Kill Boost Stone's buff, 1 without; first built as a doubling with `has_applied_skill`); the
  faction count and the quest stay 1. The shop switch is read in `server/mall.rs` (silent: a purchase or gift gets no answer and is
  logged); the max-users check is in `serve` (open connections = `max_connections − available permits`).
- **`console.rs`** (pure): `parse`, `typed_command` (skips a `<<NNNcCC` colour marker and the zero), `help`, `help_for`, the countdown's
  timing and texts. **Difference:** `help_for` returns `Option<Vec<String>>` (three lines: `usage: …`, `<word> <sentence>`, `e.g. .<example>`),
  not one string, so a whisper line stays under 120 bytes; wrong arguments answer these lines. `quit` takes 1 to 1440 minutes (`quit 0`
  or more is a usage error); without minutes it is the original's 30 s minimum.
- **`server/console.rs`**: `line(ctx, text, by, header, level)` parses and runs (the in-game path and the tool's path are the same
  function); `run` writes the server log line and the game log row (action 180, T1 `.word`, T3 the arguments). `Context` is not an `Arc` on the
  connections' side, so the **countdown is a task started by `main`** (`console::countdown`) that waits on `Context::quit_start`; the command
  only records the `Quit` state (end time, whether logins were open) and wakes it. `quit cancel` clears the state and wakes the sleep
  (`quit_cancel`); at the end the task wakes `quit_now`, which `main` selects on beside Ctrl+C, so the end is the same save-and-close
  shutdown. In the test server no such task runs, so `.quit` there only sets the state.
- **Differences in the answers:** `kick …` answers `kicked user <account id>` (or `not found`), not the original's longer text; `kick charid`
  finds the character's name in the zones and then its account in the database; a kick reaches an account that has a connection here
  (`OnlineUsers::kick`, as `/akick` does for one on the character screen); `.notice` answers `notice sent`; `.close`/`.open` answer a
  sentence; `maxusers` above `GAME_MAX_CONNECTIONS` says that the setting is the limit. `OnlineUsers::kick_all` is new for `allout`;
  `Zones::instance_counts` (party and guild-house zones) and `Zones::map_report` (each zone of a map with its players and living
  monsters) for `zones` and `map`.
- **In the game** (`server/admin.rs` `gm_chat`): the check comes first, before the chat-ban and the Message To Server branches, so a dot line is never
  chat for a level in `1..=GAME_CONSOLE_LEVEL`; answers are `0x1102` whispers from `Server`, cut at 120 bytes.
- **The tool** (`crates/login-server/src/bin/console.rs`) and `main.rs`'s listener: as step 6. The reply is cut to 7900 bytes with a
  final `...` line. The tool needs two pool connections (the listener keeps one).
- **`F106`**: `shaiya_proto::admin::world_notice` (`u16 len` counting the zero, 1 to 1999 bytes of text). **Unverified in the client**
  (live test 51).
- **Tests:** 7 in `console.rs`, `config` (range checks), `switches`, `combat` (rates 1, 2, 0 and 1.5), `pvp::counted`, `zone`
  (`has_applied_skill`), `pg` (the NOTIFY payloads), the tool (reply matching), `shaiya-proto` (`world_notice`), `game_flow`
  (`a_level_one_gm_types_console_commands_with_a_dot`, `a_higher_level_gms_dot_line_is_ordinary_chat`, the shop switch inside
  `buying_in_the_item_mall`). **No test drives a PvP kill through the kill rate** (the player-kill path needs a heavy setup): the pieces are
  tested (`counted`, `has_applied_skill`), the whole is for live test 51. **No PostgreSQL test of the NOTIFY path**: it was run against a
  running test server instead (info, help, a wrong argument, zones, map, quit and quit cancel, a countdown to its end, a server id nobody
  listens for).
- **Docs:** README (settings, the tool, test count), `docs/original/pvp.md` and `docs/original/lapis.md` (`NEW_SERVER` not built by decision),
  `docs/testing/live-test-checklist.md` section 51.
- **Not built / unverified:** the data reloads other than the one below (section 9); the manager's shared-memory commands.
- **`reload` and `restart` (`docs/plans/done/data-editing.md` section 7)**: `GameData.live` is a
  `RwLock<Arc<Live>>` holding the monsters (`MobDefs`) and the drop tables (`DropTables`); `GameData::live()` hands out a snapshot that
  the caller keeps for the whole operation (a hit, a kill, a pass of `mob_updater`), `set_live` swaps. `pg::load_live(pool, &items, &skills)`
  builds a `Live` (used by `main` and, through `AdminStore::load_live`, by the console's `reload`, which loads `defs.items` again for the
  grades; `ItemDefs` itself is not swapped). A failed load swaps nothing. Living monsters follow the table in `docs/plans/done/data-editing.md`
  section 2 (most fields at once, HP/SP/MP and AI at the next spawn); spawn areas, obelisks, boss groups and portals need a restart.
  `restart` is `quit` with `Quit.restart`: when the countdown ends `Context.restart` is set and `main` exits with `console::RESTART_STATUS`
  (75) after saving everyone; `quit cancel` cancels either. Tests: `console` (parse, help), `game_flow` (`reload_puts_new_drops_in_force_and_a_failed_one_changes_nothing`:
  a failed load, then a monster killed after the reload drops its new grade; `reload_puts_a_new_attack_in_force`;
  `restart_counts_down_like_quit_and_cancel_ends_either`). The exit status itself and `tools/game-server.sh`'s loop were run on a test server.
- **Three strengths**: `skills::KillBoost { skill: SkillMatch, times }`; `GAME_KILL_BOOST_SKILLS` parses `id[/level][:times]`
  (times 1 to 100, 2 when left out), default `200/1:2,200/2:5,200/3:10`; `Zones::kill_boost(map, id, &[KillBoost]) -> u32` is the highest
  `times` of the entries matching an applied skill, 1 when none (replacing `has_applied_skill`); `pvp::kill_counted` uses
  `kill_rate × kill_boost`. The extension `extensions/kill-boost` (once `additions.sql`) also makes 100/213, 100/214 and 200/2, 200/3. Tests: `config` (the default,
  `200`, `200/2:5`, `310:3`, errors naming the entry), `zone` (levels 1 to 3 give 2, 5, 10; none gives 1; two matching entries give the
  higher; the monster's level 100 never counts).
- **The kill-boost item** (decision 3, after research entry 7; now the first of three): `skills::SkillMatch` (an id, and a level or any);
  `GAME_KILL_BOOST_SKILLS` parses `id` and `id/level` (ids and levels from 1), default `200/1`; `Zones::has_applied_skill` takes the
  matches. The extension `extensions/kill-boost` (before 0.3.0 `additions.sql`, run by `additions.sh` after every load of `defs`) turns 100/212 and 200/1 into the
  Kill Boost Stone and its buff; the PostgreSQL test of the definitions checks both rows. Tests: `config` (the forms and refusals), `zone`
  (by id, by id and level, a monster's level-100 skill of the same id not counted). Unverified: the client's copy (the data patch) and
  the whole in a live PvP kill (live test 51).
