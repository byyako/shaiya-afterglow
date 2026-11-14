# What the client supports: systems by packet family

This page maps every game system the client `game.exe` (US build 2010-04-26, patch 108, Episode 4.5) implements on the network side, one
row per packet family. It was built from the [packet catalogue](client-packets.generated.md), the messages, sounds and files each handler
uses (a Ghidra script over every handler and the network-object methods it calls, resolved through vtable `0x6A5C20`), the system-message
table `sysmsg-uni.txt` (ID → text, looked up by `FUN_0041c180`), the original server's handler names (from `ps_game.pdb`), and EP6 packet
names. *inferred* marks families whose purpose comes from indirect evidence only.

## In short

- **Packets come in families.** The first byte of an [opcode](glossary.md#opcode) (the 2-byte number that names a packet) names the system: `05xx` is player actions and combat, `0Bxx` parties,
  `0Dxx` guilds, `23xx` personal shops, and so on.
- **The client covers the whole EP4.5 game**: combat, items, quests, trade, parties and unions, guilds, friends, personal shops, duels,
  guild battles, the item mall, the auction house, a mini-game, and the GM tools.
- **One family is still unclear**: the item list view in `27xx`. The Priest reading (`0Fxx`), the GM request (`12xx`) and the gambling house (`31xx`) now have their own notes.
- **The client asks for files it does not have**: some icons, effects and UI pieces are missing from its archive, which explains blank
  icons and missing effects in game.

### Using this page

- **If you write a server:** find the family of the opcode you are implementing, then follow the link to the page that lists its fields.
  [What a server must do for this client](afterglow/server-checklist.md#packet-handling) says what to check first.
- **If you patch or mod the client:** the second table lists files the client asks for and does not have; the family rows show which
  system a patch touches.
- **To check a claim yourself:** start from the [packet catalogue](client-packets.generated.md) row for the opcode and the method on
  [Tools and method](tools-and-method.md).

## The families

Counts: S→C = opcodes the client handles, C→S = opcodes the client sends. "Server ps_game" is the original server's handler for the
family (from its PDB).

| Family | System | S→C | C→S | Server ps_game | Evidence |
|---|---|---|---|---|---|
| `00xx` | connection control: disconnect `0002` ("connection with server has been disconnected"), kicked `0004` (duplicate login / system error) | 2 | 1 | — | sysmsg 10069–10071 |
| `01xx` | character screen, world entry, quick slots, logout, server time | 19 | 9 | inline / `PacketCharacter` | catalogue |
| `02xx` | main interface: bag moves (armor sounds), item get, appearance changes, skill learn, portals and gate-keeper costs, stat/skill reset, mount on/off, party summon, dungeon restrictions, map names | 38 | 23 | `PacketMaininterface` | sysmsg 5238–5245, 441, 819 |
| `03xx` | info about other players (`0301` info, `0303` shape/equipment) | 6 | 6 | `PacketGetInfo` | server pages |
| `04xx` | world state: day/time `0404`, weather `0451` (rain), drops on the ground `0401`/`0402` | 7 | 0 | (pushed) | `Rain.wav`, item DDS |
| `05xx` | player actions and combat: move `0501`, motions/emotes `0506` (victory pose, beg…), attacks/skill results `0502`–`0518` (death sounds, obstacle messages), firecrackers `050A`, repair `0554`, speed `051C`, HP/MP/SP updates `0521`/`0522` | 41 | 11 | `PacketPC` | sysmsg 457/458, 5228/5229, 835 |
| `06xx` | monsters: appear `0601`, leave `0602`, move `0603`, attacks and skills `0605`/`060B`–`060F`, death `0606`, HP `0608`/`0610` (see [World packets](client-world-packets.md)) | 15 | 0 | (pushed) | handlers |
| `07xx` | NPC shop buy/sell | 4 | 2 | `PacketShop` | `ui_pay001.wav` |
| `08xx` | lapis linking/extraction, enchanting | 8 | 8 | `PacketGem` | `ui_rapis*.wav` |
| `09xx` | quests | 6 | 4 | `PacketQuest` | `bgm_quest02.wav` |
| `0Axx` | player trade | 8 | 6 | `PacketExchange` | server pages |
| `0Bxx` | party and union (raid) party | 24 | 19 | `PacketParty` | `ui_party001.wav`, `UNION_WINODW_VISIBLE` |
| `0Cxx` | party member status (HP/SP/MP, maxima, buffs, death, position, level); `0C01`–`0C09` party, `0C0A`–`0C14` union (see [Social packets](client-social-packets.md)) | 19 | 0 | (pushed) | handlers; PDB `TP_PARTY_SET_*` |
| `0Dxx` | guild: create, disband, join, quit, kick, ranks, guild list, guild points | 44 | 17 | `PacketGuild` | sysmsg 372–398, 8006–8054 |
| `0Exx` | NPCs in view: appear `0E01`, leave `0E02`, move `0E03`, NPCs attacking players/mobs `0E05`/`0E06`; client sends `0E04` when an NPC is clicked | 6 | 1 | (pushed) | handlers |
| `0Fxx` | the Priest's paid "reading" (10 gold); the client has no NPC type that sends it, see [The Priest packets](research-priest-packets.md) | 2 | 2 | `CUser::PacketPriest` | sysmsg 303, 5296 |
| `11xx` | chat: normal, whisper, all/trade, guild, party, shout, zone, union | 13 | 2 | `PacketChatE` | catalogue |
| `12xx` | the `/request GM` chat command: `0x1201` carries the text, the client prints "Successfully applied" (252) (**confirmed (disasm)**, [client actions](client-actions.md)) | 0 | 1 | ? | sysmsg 5077, 252 |
| `1Cxx` | auction house (via fallback sub-dispatcher) | 23 | 0 | `PacketMarket` | `auction_objectsearch_button` |
| `1Dxx` | mail ("paper") | 8 | 0 | ? | `ui_paper001.wav`, sysmsg 7166 |
| `21xx` | world events: kills/destructions, obelisks, "a road to Jungle of Fury will be opened" | 4 | 0 | (pushed) | sysmsg 463, 5372 |
| `22xx` | friends, block list, party search | 14 | 8 | `PacketComm` / `PacketUserDBFriend` | sysmsg 399–405 |
| `23xx` | personal shop ("market"); the Market Anywhere patch bypasses its map check | 14 | 9 | `PacketMyShop` | server names |
| `24xx` | duels | 13 | 7 | `PacketPVP` | sysmsg 5325–5336, `Duel.EFT` |
| `25xx` | guild vs guild battles | 6 | 2 | `PacketPVPGuild` | sysmsg 5340–5343, `Guild.EFT` |
| `26xx` | item mall: points, buy, send/receive gifts | 8 | 4 | `PacketPoint` | sysmsg 824–832 |
| `27xx` | *inferred:* item list view (records with craft names; warehouse-like) | 4 | 2 | ? | field layout |
| `31xx` | gambling house (NPC type 5): table choice, slot and card scenes; the 9-byte result is a 3×3 reel plus the winnings; details on [Gambling house](research-gambling-kill-boost.md) | 9 | 9 | `PacketMiniGame` | handler state machine `0x84CEDC` |
| `A1xx`–`A3xx` | login key exchange, login, server list/select, game handshake | 9 | 4 | ps_login | working |
| `B1xx` | billing / time-limited services ("3 hour-free service", EXP penalty) | 6 | 2 | `PacketBilling` | sysmsg 813, 834, 840 |
| `F1xx`, `F2xx` | GM chat (mirrors `11xx`) and GM notices | 11 | 5 | `PacketAdminChat` | |
| `F5xx`–`FAxx` | GM commands (move to map, item info, char appearance, warnings/news) | 34 | 61 | `PacketAdminCmdA`–`E` | `[Map:%d, %d, %d]`, item tooltips |

Total: 425 S→C, 228 C→S. Field details: [World packets](client-world-packets.md) (`04`–`09`, `0E` families) and
[Social packets](client-social-packets.md) (`0A`–`0D`). Each family's exact field layout (client view) is in the
[packet catalogue](client-packets.generated.md); 86 of them are confirmed by the EP4.5 server's own struct sizes.

## Assets the client asks for that are not in data.saf

Seen in a client log.

| Requested | Notes |
|---|---|
| `data/interface/icon/101–141.dds`, `25–30.dds`, `41.dds`, `icon_skill3–10.dds` | item/skill icons referenced by definitions but never shipped (shown as blank icons) |
| `data/interface/invasionUI_*.tga` | "invasion" UI (later content, unused in EP4) |
| `data/interface/passpod.tga`, `ch_cardnum.tga` | Korean OTP / security card login (unused) |
| `data/interface/window_button.tga` (×5), `talk1_close_button_3.tga` | missing UI pieces (possibly visible as blank buttons) |
| `data/world/dungeon/Login_eternity_territory_01/*_L0–L3.dds` | login-screen dungeon lightmaps |
| `data/Effect/RC_stamina.EFT`, `RC_health.EFT`, `IN_sc_cowardice.EFT`, `DA_self_crag.EFT` | skill/potion effects referenced by data but missing |
| `data/Config.cfg`, `Data/World/1.svmap` | optional dev files: with feature flag `0x10` set, `FUN_00430280` reads the server map's monster areas (likely a debug overlay); missing is fine |

These are harmless (the client logs and continues), but they explain blank icons and missing effects in game.
