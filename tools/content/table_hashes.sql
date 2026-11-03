-- One line per table: schema.table|rows|md5, the md5 over the rows' text in a stable order. The baseline and the proof of
-- docs/plans/ext-1-foundation.md sections 3.5 and 9: the same lines before and after a change mean no byte of the tables changed.
--   psql -At -v schemas=defs,world,client -f tools/content/table_hashes.sql        (schemas defaults to defs,world,client)
-- The data editor's marker tables (edits_applied, rewritten with a time stamp whenever the edits are written again) are not content
-- and are left out; so are tables that are not ordinary tables. `data-editor content hashes` prints the same lines.
\if :{?schemas}
\else
\set schemas defs,world,client
\endif
SELECT format(
    'SELECT %L || ''|'' || count(*) || ''|'' || coalesce(md5(string_agg(t::text, E''\n'' ORDER BY t::text)), md5('''')) FROM %I.%I t',
    n.nspname || '.' || c.relname, n.nspname, c.relname)
FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
WHERE c.relkind IN ('r', 'p') AND c.relname <> 'edits_applied' AND n.nspname = ANY (string_to_array(:'schemas', ','))
ORDER BY n.nspname, c.relname
\gexec
