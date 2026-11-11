# Server files inventory (the original files' `Server/`)

Static analysis only: nothing was executed. Source: `server/ep4.5.zip` from the
OpenShaiya archive. Raw data: `docs/original/inventory/server/` (`files.csv` with SHA-256 for all 748 files,
`pe.csv`, `pe_imports.txt`, `network_strings.txt`, `pdb_identity.txt`). Tools: `tools/inventory_server.py`,
`tools/pdb_check.py`.

The EP3 and EP5.4 server packs are listed the same way in `docs/original/inventory/server-ep3/` and `server-ep5.4/`. They are
compared with this pack in [`ep3.md`](ep3.md) and [`ep5.4.md`](ep5.4.md); the binary tables of all three episodes are there.

What each config and data file holds, every key the programs read and how to change it: [The server's files](server-files.md).

## Summary

| | |
|---|---|
| Files | 748, of which 392 unique (583 are duplicates of another file; the tree stores most binaries twice) |
| Size | 122 MB |
| Executables / DLLs | 40 PE files, all 32-bit x86. The 30 Shaiya executables use the MSVC 7.1 (Visual Studio 2003) linker; the other 10 are Microsoft debug DLLs and GameGuard DLLs |
| Debug symbols | 7 PDB files (MSF 7.00) for the seven game services |
| Database backups | 10 SQL Server backups in `PSM_Data/` |
| Game data | 154 SVMap files, 58 mob AI script sets, 16 Lua files, 5 SData files, 50 ini files |

## Layout

| path | contents |
|---|---|
| `PSM_Client/Bin/` | the running server set: `ps_login`, `ps_session`, `ps_game`, `ps_gamewar`, `ps_dbAgent`, `ps_gameLog`, `ps_userLog`, plus `ggauth.dll` / `ggsrvdll.dll` (nProtect GameGuard server side) and Microsoft `dbghelp` / `dbgeng` |
| `PSM_Client/Bin/Config/` | one ini per service (ports blank, DB strings encrypted) |
| `PSM_Client/Bin/Data/` | `AI/` (180 files), `Lua/` (8), `Map/` (77 SVMap), `KillStatus.SData`, `NpcQuest.SData`, `*.ini` tables, `kr_string.DB` |
| `PSM_Client/PSM_*.exe`, `PSM_Server/PSMServer_*.exe` | the PSM management/patch/monitor layer (agent, service manager, log gather) |
| `PSM_Server/ShaiyaServerPatch/Server/{GameMain,ServerMain}` | a second copy of the service binaries **with PDBs**, plus `Game01`, `Game02`, `GameWar` data folders |
| `PSM_Server/ShaiyaServerPatch/Agent/Update_ForGMTool/` | `PSGMTool.exe` and updated `NpcQuest.SData` |
| `PSM_Server/ps_passwordEnc.exe` | small tool, most likely the one that produces the encrypted DB strings in the ini files |
| `PSM_Data/` | ten SQL Server backups (see below) |
| `*/Log/` | 101 tiny PSM agent logs, Oct 2009 to Mar 2010 |

## Binaries

The Shaiya executables were built with Visual Studio 2003 (linker 7.10). Build timestamps are plausible and not obviously altered:

| binary | built (UTC) |
|---|---|
| `ps_game.exe` | 2009-07-15 |
| `ps_dbAgent.exe`, `ps_gameLog.exe` | 2009-06-24 |
| `ps_login.exe`, `ps_session.exe`, `ps_userLog.exe`, `PSGMTool.exe` | 2009-06-10 |
| `ps_gamewar.exe` | 2009-04-01 |
| `ps_passwordEnc.exe` | 2006-12-07 |
| PSM tools | 2007-02 to 2008-04 |

- Original source path baked into the PDB references: `d:\SonoV\Bin.net\` (Sonov, the developer). Build times and the backup dates (2010-03-12) put this pack at about March 2010.
- **`ps_login.exe` exists in two versions:** `PSM_Client/Bin/` is older (2007-12-07, 319,488 bytes); `ShaiyaServerPatch/Server/ServerMain/PS_Login/` is newer (2009-06-10, 327,680 bytes). All other services are byte-identical between the two trees. The newer one is the one that has a matching PDB. `PSM_Agent.exe` also differs between copies, same size.
- No packers: the highest section entropy is about 6.5 to 6.6 across all executables, so they are not compressed or encrypted.
- Imports are what you would expect: `WS2_32`, `MSWSOCK`, `ODBC32` (database), `dbghelp` / `dbgeng` (crash dumps). `ps_session` has no ODBC import. Nothing imports `wininet`, `urlmon` or other outbound HTTP APIs.

## Debug symbols

All seven PDBs are present for the newer binaries (`ps_game.pdb` is 10.9 MB). The GUID of each PDB matches the CodeView id in its executable for **six of seven**:

| service | match |
|---|---|
| `ps_game`, `ps_login`, `ps_session`, `ps_userLog`, `ps_dbAgent`, `ps_gameLog` | exact (GUID and age) |
| `ps_gamewar` | GUID matches, age differs (exe 42, PDB 66): the PDB was produced by a later link of the same code, so names and types are reliable but addresses may not line up exactly |

This is the single most valuable find for the reverse engineering: full function names, class layouts
and struct definitions, instead of guessing from raw disassembly. The same names appear in the
Shaiya-Core headers (`TP_*`, `TPC_*`), which gives a cross-check.

## Hard-coded addresses and strings

Searched every PE for URLs, IPv4 addresses and hostnames (ASCII only):
- `192.168.0.2` in `ps_gamewar.exe`, `10.10.10.231` and `192.168.10.231` in `PSGMTool.exe`: private ranges, apparent developer leftovers.
- No external URLs or public IPs. `Lua.org` and `command.com` in `ps_game.exe` are Lua banner/runtime strings.

This is a surface check, not a clearance: it does not cover UTF-16 strings, built-up strings or
logic. It does mean nothing obviously phones home. The files stay untrusted and are never run on
this machine.

## Database backups (`PSM_Data/`)

Ten SQL Server backups in MTF format ("TAPE" header), all taken 2010-03-12 on a host named `SHAIYA`
with original file paths `D:\SHAIYA_SERVER\DATABASE\*.mdf`:

| database | size | purpose (confirmed by the server configs for GameDefs, UserData, GameData, ChatLog and GameLog: `ps_game.ini`, `ps_dbAgent.ini`, `ps_login.ini`, `ps_userlog.ini`, `ps_gameLog.ini`; the others inferred from the name: no server config of the pack names them) |
|---|---|---|
| `PS_GameDefs` | 7.5 MB | game definitions (items, mobs, skills, maps, NPCs, quests): the key one for the rewrite |
| `PS_UserData` | 6.4 MB | accounts and sessions |
| `PS_GameData` | 3.2 MB | characters, inventories, guilds |
| `PS_GMTool` | 3.2 MB | GM tool tables |
| `PS_Statics` / `PS_StatData` | 2.4 / 1.8 MB | statistics |
| `PS_ChatLog`, `PS_GameLog` | 2.3 / 2.2 MB | logs |
| `OMG_GameWEB` | 2.2 MB | web-side database (forum/website integration) |
| `PS_Billing` | 1.7 MB | billing |

These cannot be read without restoring them. SQL Server version is not yet determined. Caution: `PS_UserData`
and `PS_GameData` may contain real account or character rows. Anything pulled from them stays out of the
repo (the `original/` and `reference/` folders are git-ignored, and any exported data goes under a
git-ignored path).

## Client folder (not yet examined)

Only text files were read, to settle the version question:
- `Version.ini`: `CurrentVersion=108`, `CheckVersion=3` (the client patch number).
- `ShaiyaUS.ini`: encrypted binary, not printed.
- `game.exe` dated 2010-04-26, `data.saf` 2.3 GB / `data.sah` 889 KB dated 2011-01-21.
- Everything else (SAF/SAH extraction, `game.exe` analysis) is still to do.

## Findings that change the plan

1. **Symbols**: the reverse engineering can run from real names (section above).
2. **The database schema is recoverable** by restoring the backups into a throwaway SQL Server
   (Docker image on this machine), then exporting the schema and the definitions tables to
   PostgreSQL. No need to infer the schema from queries.
3. **Server binaries are from mid-2009 and the pack from March 2010, but the data files do not match
   the client in the archive**. The `NpcQuest.SData` shipped with the server is 2,637,480 bytes; client
   `NpcQuest.SData` files at US patches near 108 are around 3.1 MB. The server and client in these folders
   may be different episodes. Which one is "EP4.5" still needs confirmation.

## Open items

All three are settled:

- Restoring the backups: done (`tools/restore_backups.sh`; the structure of all ten is in [`db/`](db/README.md)).
- The client/server version match: the client sends `0x10042600` in its server select (`game.exe` 0x4dcce6, built at 0x5868ee), which
  reads as the date 2010-04-26, the file date of `game.exe` (the reading as a date is inferred). Every shipped `ps_game.ini` has
  `GameVersion=0x00000000` (one leftover `PSMServer_Config.ini` copy has `0x09072200`), and the login server's version check starts switched
  on, so the original as shipped refuses this client's select with `0xFD` until an operator types `/vchkoff` or sets the version
  ([`ps_login.md`](ps_login.md) section 16; confirmed).
- The encryption question: answered, AES-128 in counter mode with a standard `AES_encrypt` (ps_login 0x40cf00, `Te0` at 0x447f78;
  [`ps_login.md`](ps_login.md) section 4).

## The "Omega" pack (`Server/New Files/OmegaFiles`)

766 files, inventoried with the same tool (`docs/original/inventory/omega/`). **All 748 files of the EP4.5 pack are present with identical contents** (same paths and SHA-256),
so the executables, the PDBs, the SData and ini files, and all ten database backups are the same. Omega adds 18 files, none of them binaries or data:

- `Install_Server.bat` and `batch/*.bat`: restore the databases with `osql`, register the services (`ps_dbAgent`, `ps_gameLog`, `ps_game`, `ps_gamewar`, `ps_login`,
  `ps_session`, `ps_userLog`, plus the PSM agents) with `sc create`, start MSDTC.
- `batch/config.sql` and `LinkedServers.txt`: create a SQL login with the `sysadmin` role and **linked servers** named `game`, `PS_NCASH`, `PS_USERDB`, `PS_USERDB01`,
  `PS_DEFINEDB`, `PS_GAMEDB01`, with RPC enabled. This is how the original stored procedures reach other databases (see `docs/original/database-notes.md`).
- `batch/usp_Read_User_CashPoint_NCash.txt`, `usp_Save_User_BuyPointItems_NCash.txt`, `Read_User_CashPoint_NCash.sql`: the cash-point (item mall) procedures.
- `Config/PSMServer_Config.ini` (encrypted database user), `Omega.jpg`, `Important_Links.html`.

The installer ships weak default credentials (the SQL login and a default game admin account). They are not reproduced here and must never be reused. So Omega is the
same server and database as the EP4.5 pack with an installer on top, not a different variant.
