# F502 operator chat listener installed

Installed with the maintainer's explicit approval for this client installation and this new report. Client root: the client repository.

## Installed files and verification

- Copied the existing i686 release build from `extracted/client-research-20261021/build/i686-pc-windows-gnullvm/release/fixes.dll` to root `fixes.dll` (430,080 bytes). This is the full working-tree build documented in [operator-listen.md](operator-listen.md), containing `rust/fixes/src/listen.rs`; it was not rebuilt during installation.
- Installed DLL SHA-256: `e1910d289438f65de49845982efed8ac78720d84cb4455852fe70991863e638c`. Source and installed hashes match the documented build hash.
- Added only `listen_chat=1` immediately below `[fixes]` in root `fixes.ini`. Removing that inserted line restores the exact original file bytes. All other settings, comments, encoding and line endings were retained. In particular, the existing feature list `0,5,8,23,26,43,45,46,62,21,35`, `anti_freeze=1`, `road_messages=1`, `log=0`, and all graphics settings remain as found. Other default-on behavior in this build remains at its existing defaults.
- Installed config SHA-256: `9a6bfd68f365c89a69640f3080c06b082beddc00dce403b59bede2e15527f4e2`.
- `game.exe` was not written. Before/after SHA-256: `5f045bcb4a2e29562ac79effc4108fbaa6404ab29258df02c4dfc6ed7e3401a6`.
- No client process was running at the install check. No process was closed or launched by this session. Server code, configuration, binaries and existing documentation were not edited; the only Shaiya Afterglow write was this new report.

## Verified backup

Before replacing either installed file, copied and hash-verified both in:

the client repository's `extracted\client-research-20261021\install-backups\20261021T170332Z\`

| Backup file | SHA-256 |
|---|---|
| `fixes.dll` | `2c6a8cad149c452e872631f2d3077351f373a2ef4a722a1d8a3079f1ac664e12` |
| `fixes.ini` | `750a902850cd0c90c5a32f24b35b80ebe0f61d3a17512341d71437f1dba17c08` |

`installation.json` beside those backups records the source, destinations, hashes and install time.

## Rollback

Close the client before rollback. Restore BOTH backed-up files to the client root, then confirm their hashes match the table above. This restores the previous DLL and exact previous configuration; `game.exe` needs no rollback. PowerShell, while the client is closed:

```powershell
$ErrorActionPreference = 'Stop'
$clientRoot = '<the client repository>'
$backupDir = Join-Path $clientRoot 'extracted\client-research-20261021\install-backups\20261021T170332Z'
$expected = @{
    'fixes.dll' = '2c6a8cad149c452e872631f2d3077351f373a2ef4a722a1d8a3079f1ac664e12'
    'fixes.ini' = '750a902850cd0c90c5a32f24b35b80ebe0f61d3a17512341d71437f1dba17c08'
}
foreach ($name in $expected.Keys) {
    if ((Get-FileHash -LiteralPath (Join-Path $backupDir $name)).Hash -ne $expected[$name]) {
        throw "Backup hash mismatch: $name"
    }
}
foreach ($name in $expected.Keys) {
    Copy-Item -LiteralPath (Join-Path $backupDir $name) -Destination (Join-Path $clientRoot $name) -Force
    if ((Get-FileHash -LiteralPath (Join-Path $clientRoot $name)).Hash -ne $expected[$name]) {
        throw "Restored hash mismatch: $name"
    }
}
Get-FileHash -LiteralPath (Join-Path $clientRoot 'fixes.dll'), (Join-Path $clientRoot 'fixes.ini')
```

For a temporary listener-only disable, set `[fixes] listen_chat=0` before the next launch. This does not restore the old DLL and is distinct from full rollback.

## Live test hand-over

Installation and file verification are complete. In-game behavior has NOT been tested by this installation session. The server side and user own [live-test-checklist.md section 49](../testing/live-test-checklist.md#49-operator-chat-listening-cwatch); no checklist item was marked complete here. Its introductory statement that the DLL is not installed predates this report and is superseded for this client only.

Use the installed client for TestVail (the GM account), with a second client for the watched player (e.g. ProbeHero). Check start/unknown-name confirmations, each chat channel, GM-log placement with no centre-screen notice, literal markup handling, `/cwatch off`, watch cleanup on logout, and auctions/mail/server time through the preserved fallback. The handler displays the server-supplied prefix verbatim (including the section 49 channel labels); this install does not change the wire contract. Server authorization, logs and live acceptance remain server-side responsibilities.

Prior offline evidence in `operator-listen.md`: six parser tests; i686 release build success; 27 tests passed, 2 existing tests ignored; disassembly validation. Those results were recorded before this install and were not rerun. They are not live acceptance evidence.
