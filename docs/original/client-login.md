# How the client reaches the login server (`game.exe`, US build)

Static analysis of the original files' `Client/game.exe` (SHA-256 `5f045bcb4a2e29562ac79effc4108fbaa6404ab29258df02c4dfc6ed7e3401a6`,
built 2010-04-26, MSVC 7.1, not packed, no PDB). Nothing was executed. The client folder reports `CurrentVersion=108` in `Version.ini`.

## Verified in the code

- **Region table.** `FUN_00408750` (start-up configuration) reads `CONFIG.INI` `[LOGIN] VERSION=` and `[LOGIN] TEST_IP=`; the value is a region
  name (`KOREA`, `CHINA`, `VIETNAM`, ... 17 cases). Each region copies a 16-byte address string into the global `0x701970`. There is no free-form
  address setting. `TEST_IP` also takes a region name and selects internal test addresses (`172.17.x.x`).
- **Default region.** The region variable (`0x6f1558`) starts as **6**. This build's `CONFIG.INI` has no `VERSION`, so region 6 is used: the string
  **`174.142.118.105`**, stored in `.rdata` at file offset `0x2a5b34` (16-byte slot, VA `0x6a5b34`).
- **Connection.** `FUN_0047bb10` calls `FUN_00578b10(&0x701970, 0x7850)`: TCP to that address, **port 0x7850 = 30800**. On failure it shows
  `LoginServer Connet Error!!!`. On success it waits up to 30 seconds for the key packet and otherwise shows `key Data Recv!!!`.
- **Launch.** The string `start game` is in the binary; the original launch path is `Updater.exe`, which then starts `game.exe`.

## Not verified

- Whether `game.exe` runs with `start game` as its only argument and without the updater. (The string exists; the check that uses it was not read.)
- GameGuard: only the stray string `nProtect GameGuard` exists. There is no `GameMon` reference and no GameGuard module import, so it is probably
  not wired in, but nothing here proves the client starts without GameGuard files.
- `ShaiyaUS.ini` in the client folder is encrypted (330 bytes). Which program reads it, and whether it matters, is unknown.
- The cipher: all of `docs/original/ps_login.md` section 4 is from the server binary; the client side has not been read.

## Pointing the client at our server

Two options. Neither modifies the original files.

1. **Patched copy (needs no network setup).**
   `python3 tools/patch_client_ip.py <Client>/game.exe <Client>/game_local.exe <server ip>` writes a copy whose region-6 address is your server. The tool checks the
   input hash and the original bytes first and changes 10 bytes. Run the copy with the same arguments and files as the original.
2. **Network redirect.** Make `174.142.118.105:30800` reach the server (router/firewall DNAT, or a port proxy on the client machine). No client change.

## Reading the result on screen

| what you see | what it means |
|---|---|
| `LoginServer Connet Error!!!` | TCP connection to the server failed: wrong IP, firewall, or the server is not running |
| `key Data Recv!!!` after about 30 s | connected, but no `0xA101` key packet arrived |
| the login screen appears, login hangs or is refused | the key packet arrived; see the server log below |

Server log lines (with `RUST_LOG=info,login_server=debug`), in order for a good attempt:
`encryption established` -> `login accepted` or `login refused` -> `server selected`.
- `dropped ... unexpected opcode 0x....` right after `encryption established`: the first packet did not decrypt to `0xA102`, so the key exchange or
  keystream does not match the real client. The opcode value printed is the garbage that came out; this is the signal that the cipher needs work.
- `dropped ... malformed key reply` / `key reply not below modulus`: the client's reply to `0xA101` did not look as documented.

## Server select and the game connection (verified in `FUN_004dca00`)

- The version sent with the server choice is the literal `0x10042600` in the code (not from `Version.ini`).
- The client refuses to select a server whose list status is not 0.
- After a successful select it connects to the returned IP on **port 30810** (`0x785A`) and waits up to 30 s.
- Three places in the client connect to the login server (port 30800) and three to the game server (30810).
- First live session: key exchange, cipher, login and server list worked against the real client. The quit message `0x010B`
  is sent when the client closes.
