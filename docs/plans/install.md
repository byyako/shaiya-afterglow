# Plan: an install anyone on Linux can try in minutes

Decided 2026-11-02 with the maintainer. The audience first is people comfortable with a terminal, Docker and git. The aim is that
someone who has the original server files can have a working server and a connected client in well under an hour, with the least
reading and the fewest places to go wrong.

## Where it hurts today

1. **Rust.** Building needs rustup, Rust 1.94 or newer and about 2 GB of memory, and takes minutes.
2. **SQL Server.** One database (`PS_GameDefs`, 7 MB) is read by restoring all ten backups into a 2 GB SQL Server container
   (about 20 minutes, a long start-up wait, memory limits, an EULA flag).
3. **Many steps.** `.env` with three passwords that must agree, then `setup.sh`, then an account, a server entry, two start commands, and a
   patched client.
4. **Names.** The setup used the project's old name (`newshaiya-*`, a `newshaiya` database, `NEWSHAIYA_*` variables).
5. **The README** is the whole manual (327 lines); a newcomer has to find the short path in it.

## Steps

| # | Step | Decision |
|---|---|---|
| I1 | **Rename** containers, volumes, database and variables to the project's name, with the old variable names and edits files still read (no fallback for container names: it could pick a different setup's containers) | built 2026-11-02 |
| I2 | **Export path:** `tools/setup.sh` accepts a folder of already-exported definition tables instead of the backups (no SQL Server) | built 2026-11-02 |
| I3 | **Read the backup directly:** a Python reader for the SQL Server 2005 backup (`tools/defs/read_bak.py`) that writes the same export folder `export_from_mssql.sh` writes; checked against the SQL Server export table by table. Then `setup.sh` needs no SQL Server for `PS_GameDefs` | built 2026-11-02: all eight tables identical to the SQL Server export; a full setup in 40 seconds |
| I4 | **Prebuilt release:** CI builds the Linux binaries on a tagged release and attaches them; the quick start downloads them; building from source stays for developers | built 2026-11-02; v0.2.0 released by the workflow, and `tools/fetch-release.sh` downloaded and ran its binaries |
| I5 | **One command:** `tools/setup.sh` writes `.env` with random passwords when there is none, checks Docker access and ports, and ends by creating the account and the server entry (asking for the name, password and address) and starting the servers | built 2026-11-02 (driven through a terminal in a drill) |
| I6 | **A short README:** a ten-line quick start at the top, the long manual below it; the site's Run page the same | built 2026-11-02 |
| I7 | **The client:** a one-line client patch with a clearer result message, and a check that the patched `game.exe` is the right build | |

Tested 2026-11-02 in drills with their own container names and ports: the no-Rust path (the release binaries downloaded by `setup.sh`), the direct reader, and `--sqlserver` (19 minutes 38 seconds, verified field by field against SQL Server; its export is identical to the direct reader's).

I2 and I3 produce the same folder (`data/defs-export/`) from different sources, so `setup.sh` can take any of three inputs: the
backups (SQL Server container), the backups (direct reader, once it passes its check) or an export folder.

## Out of scope

Windows hosts (WSL2 works in principle but is untested), a published Docker image of the servers, a graphical installer.
