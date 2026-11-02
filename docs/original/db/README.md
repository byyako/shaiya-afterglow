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
SQL Server container (`MSSQL_CONTAINER`, default `newshaiya-mssql`; `tools/dbconn.sh`), once per user database. The container must run
with the ten databases restored.

## Proof

Run against the restored databases, the script writes all 30 files byte for byte the same as the ones here.
