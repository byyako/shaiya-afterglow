# inventory: a static list of the original server files

What is in the two packs of original server files, listed without running anything: every file with its size and SHA-256, every
executable's build facts and imports, and the network addresses found in the executables. Current by nature (the originals do not
change). `docs/original/server-inventory.md` is the write-up; these are its raw data. Readers: that note, `docs/original/symbols/`'s
README (which executable and PDB each symbol file comes from), and anyone checking a file against its hash.

| folder | the pack | root given to the tool |
|---|---|---|
| `server/` | the EP4.5 server pack (748 files, 40 PE files) | its `Server/` folder holding `PSM_Client/`, `PSM_Data/` and `PSM_Server/` (paths start at `PSM_Client/...`), without the `New Files/` folder |
| `omega/` | the Omega repack (766 files, 40 PE files) | `Server/New Files` (paths start at `OmegaFiles/...`) |

Each folder holds:

| file | contents |
|---|---|
| `files.csv` | `path,size,sha256,kind`, one row per file |
| `pe.csv` | one row per PE file: machine, build time, linker, subsystem, image base, entry point, entropy, version resource, the PDB path and id, the sections |
| `pe_imports.txt` | the DLLs each PE file imports |
| `network_strings.txt` | URLs, IPv4 addresses and host names found in each PE file's bytes |
| `pdb_identity.txt` (`server/` only) | each PDB's stream count, GUID and age, to match against an executable's CodeView id |

## How they are made

```
python3 tools/inventory_server.py <root> <out dir>      # files.csv, pe.csv, pe_imports.txt, network_strings.txt
cd <the pack>/PSM_Server/ShaiyaServerPatch/Server && python3 <repo>/tools/pdb_check.py GameMain/*/*.pdb ServerMain/*/*.pdb > pdb_identity.txt
```

`pdb_identity.txt` lists the files in the order the shell sorts the glob in an English UTF-8 locale; another locale orders the lines
differently. Both tools need `pefile` (`pip install -r tools/requirements.txt`) and never execute what they read.

## Proof

Run again from the original files, all nine files came out byte for byte the same as the ones here.
