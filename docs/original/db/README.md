# db: the structure of the original databases

What the ten SQL Server databases of the original server contain, as plain text: tables and columns, indexes, stored procedures.
Structure only, no player data (a row count per table is the only number taken from the data). The databases are restored from
the ten backups in the server pack (`PSM_Data/`; `docs/original/database-notes.md` describes the backups and `tools/restore_backups.sh`
restores them). Current by nature. Readers: `tools/defs/generate.py` and `verify.py` (both read `PS_GameDefs.columns.txt`) and
anyone looking for a table or a stored procedure of the original.

The ten databases: `OMG_GameWEB`, `PS_Billing`, `PS_ChatLog`, `PS_GameData`, `PS_GameDefs`, `PS_GameLog`, `PS_GMTool`, `PS_StatData`,
`PS_Statics`, `PS_UserData`. Three files each:

| file | one line per | fields, `|` between |
|---|---|---|
| `<db>.columns.txt` | column | table, ordinal, column, type (`varchar(12)`, `nvarchar(255)`, `int`, ...), `NOT NULL` or `NULL`, `IDENTITY` or empty, the default's text (`(0)`, `(getdate())`) or empty, the table's row count |
| `<db>.indexes.txt` | index (heaps are not listed) | index name, table, primary key (1/0), unique (1/0), `CLUSTERED` or `NONCLUSTERED` |
| `<db>.procs.txt` | stored procedure | its name |

Tables and indexes are in name order, columns in ordinal order, procedures by name.

## How they are made

```
tools/defs/describe_mssql.sh [out dir]        # default: this folder
```

It runs read-only catalogue queries (`sys.tables`, `sys.columns`, `sys.indexes`, `sys.procedures`, `sys.partitions`) with `sqlcmd` inside the
SQL Server container (`MSSQL_CONTAINER`, default `shaiya-afterglow-mssql`; `tools/dbconn.sh`), once per user database. The container must run
with the ten databases restored.

## Proof

Run against the restored databases, the script writes all 30 files byte for byte the same as the ones here.

## Other episodes: `ep3/` and `ep5.4/`

The same three files per database for the EP3 and EP5.4 server packs, in the same format. Each pack's ten databases are in its
`PSM_Data/` as `.MDF`/`.LDF` data files, not as backups. EP3 has `PS_Monitor` and `PS_Statistics` where EP4.5 has
`PS_StatData` and `PS_Statics`. Each file is named the way EP4.5 spells the database, for example `PS_GMTool` for EP3's
`PS_GMTOOL`. [`../ep3.md`](../ep3.md) and [`../ep5.4.md`](../ep5.4.md) describe the differences.

How they were made:

- **EP5.4 `PS_GameDefs`** is database version 661 (SQL Server 2008 R2). A copy of its files was attached to the SQL Server 2022
  container and described with `tools/defs/describe_mssql.sh`; the copy was detached again afterwards.
- **The other 19** (all ten of EP3, nine of EP5.4) are database version 539, SQL Server 2000. SQL Server 2022 refuses them (Msg 950,
  "non-release version (539) is not supported"). They were read without a database server by `tools/defs/describe_mdf2000.py`, a
  read-only static reader of the SQL Server 2000 page format:
  1. It reads the system tables by their fixed object ids: `sysobjects` 1, `sysindexes` 2, `syscolumns` 3, `systypes` 4 and
     `syscomments` 6. It decodes `syscolumns` with a built-in layout first and checks that layout against `syscolumns`' own rows.
  2. It takes the bit meanings from the computed columns' definitions stored in `syscomments`.
  3. It writes `describe_mssql.sh`'s format: the row count from `sysindexes.rowcnt`, defaults from the text of the default
     constraints, primary key and unique from the `sysindexes` status bits, and names in the same order as the files above.

```
python3 -I tools/defs/describe_mdf2000.py describe <the pack>/PSM_Data/<db>_Data.MDF docs/original/db/ep3 <name>
```

The name argument gives each file EP4.5's spelling of the database (the boot page of EP3's `PS_GMTool`, for example, says
`PS_GMTOOL`); without it the boot page's name is used. `boot` prints a file's database name and version, `proc` the text of one
procedure.

Proof for the reader: run again from the pack files, it writes all 57 files byte for byte the same as the ones here. Seven EP5.4 databases have the same schema as EP4.5's. For each of them, all three files came out byte for
byte the same as the SQL Server 2022 files for EP4.5 in this folder. Every remaining difference was checked against the raw pages.
The row counts match the records on each table's allocated data pages, except EP5.4 `PS_GameLog.ActionLog`. There, SQL Server
2000's own stored count is two rows stale.
