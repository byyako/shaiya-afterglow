-- Rules (docs/plans/ext-2-rules.md section 4): an enabled extension's rules (layer 2, stored at install; the key is the rule's name
-- without the extension's prefix) and the data editor's rules (layer 4, written only through the edits overlay, the `rule` target).
-- Guarded (IF NOT EXISTS) as 0031: the tests of other binaries run all migrations again inside throwaway schemas of the same database.
CREATE TABLE IF NOT EXISTS content.ext_rules (
    extension text NOT NULL REFERENCES content.extensions (name) ON DELETE CASCADE,
    name      text NOT NULL,
    rule      jsonb NOT NULL,
    PRIMARY KEY (extension, name)
);

-- The editor's rules: a rule is a row; removing it falls back to the layers below. The name may be `<extension>/<name>` to replace or
-- turn off an extension's rule.
CREATE TABLE IF NOT EXISTS content.rule_values (
    name text PRIMARY KEY,
    rule jsonb NOT NULL
);
