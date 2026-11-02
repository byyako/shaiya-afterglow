# symbols: the original server's symbol tables and packet structs

What the original EP4.5 server executables say about themselves, read from their debug symbols (PDB files). Never executed: the
tools only read the files. These do not change (the originals do not), so they are current by nature. Readers: `tools/disasm.py`
(takes a `.tsv` to name the addresses it disassembles) and anyone reading the notes in `docs/original/` who wants a function's
address or a packet struct's layout.

## The `.tsv` files

One per service. Columns, tab-separated, no header:

| column | meaning |
|---|---|
| `va` | virtual address in the executable, `0x` and hex |
| `size` | size in bytes (`proc`: the function's size; `pub` and `data`: 0) |
| `kind` | `proc` = a function (undecorated name, with size), `pub` = a public symbol (the decorated name), `data` = a data symbol |
| `name` | the symbol's name |

Made by `python3 tools/pdb_symbols.py <exe> <pdb> <out.tsv>` (needs `pefile`: `tools/requirements.txt`) from the executable and the PDB
that came with it, both in the server pack under `PSM_Server/ShaiyaServerPatch/Server/` (the `PSM_Client/Bin/` copies of the
executables have no PDB, and `ps_login.exe` there is a different build). SHA-256 from `docs/original/inventory/server/files.csv`:

| file | executable | its SHA-256 | the PDB's SHA-256 |
|---|---|---|---|
| `ps_dbAgent.tsv` | `GameMain/PS_DBAgent/ps_dbAgent.exe` | `893ebe6725cdf3a6976fabee301aeec217702b609383d1f1dee1c368b76018df` | `96dfdef517d77f6d4275e70ccca085c9a7867b11a0bfdfddf88dadbe03dfc357` |
| `ps_game.tsv` | `GameMain/PS_Game/ps_game.exe` | `0fb7c9f13b5bcf186118f8bfe73ab214226b93e8aa502b8e6fded4a8312bc547` | `5ac68bfc243ce3c15a7256bffca7a768bb3cad984bfb9f32675012dd7b2ce13f` |
| `ps_gameLog.tsv` | `GameMain/PS_GameLog/ps_gameLog.exe` | `e9b78c754718bd9f15fe146fe5ccfa9e9a476db3873b31ea013544313cfd9c94` | `12ea138b6a9d0daa0b8c247c6203acaf2362a30161a78e8b581685b70326e02b` |
| `ps_login.tsv` | `ServerMain/PS_Login/ps_login.exe` | `8aa4b377a1d05d97a8ab1def841f0291643a47226bb80ffb8524ba8029c13f07` | `8d466ea8d884d848d9bb4c25096b76f1a41c4e6079204d90f3f05d4a1021ea0a` |
| `ps_session.tsv` | `ServerMain/PS_Session/ps_session.exe` | `5c7d8b02a02fc979948d53f688f7d0ca5403054e46abb49bc0ab4f78c7ccef61` | `c26ebb6922afb90e30eb55505470193874d3dba3d58083f3dc2faacc9bcd7160` |

## `ps_game.packets.txt`

The packet and record structs of `ps_game.pdb` (`TP_*`, `TPC_*`, `SP_*`, `st*`, `DB_*`), with each member's offset, size and type;
`wType` is the opcode. Four header lines, then the output of `python3 tools/pdb_types.py ps_game.pdb '^(TPC?_|SP_|st[A-Z]|DB_)'`.
Exact for this server build.

## Proof

Each of the six files was made again with the commands above from the original files and compared with `cmp`: all six are
byte for byte the same (`ps_game.packets.txt` from its fifth line on).
