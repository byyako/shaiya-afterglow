-- Scripts (docs/plans/ext-5-lua.md section 3.2): the Lua files of an extension, stored at install after compiling them, its option
-- specs, and the values scripts store (per character and the extension's own). Guarded (IF NOT EXISTS) as 0031 and 0032: the tests of
-- other binaries run all migrations again inside throwaway schemas of the same database.
CREATE TABLE IF NOT EXISTS content.ext_scripts (
    extension text NOT NULL REFERENCES content.extensions (name) ON DELETE CASCADE,
    -- the order of the manifest's `scripts`
    seq       smallint NOT NULL,
    path      text NOT NULL,
    source    text NOT NULL,
    sha256    text NOT NULL,
    PRIMARY KEY (extension, path),
    UNIQUE (extension, seq)
);

CREATE TABLE IF NOT EXISTS content.ext_options (
    extension text NOT NULL REFERENCES content.extensions (name) ON DELETE CASCADE,
    name      text NOT NULL,
    -- type, default, doc, min, max as in the manifest
    spec      jsonb NOT NULL,
    PRIMARY KEY (extension, name)
);

-- Values a script stores (section 7.5): per character, and the extension's own.
CREATE TABLE IF NOT EXISTS content.script_values (
    extension    text NOT NULL REFERENCES content.extensions (name) ON DELETE CASCADE,
    character_id integer NOT NULL REFERENCES characters (id) ON DELETE CASCADE,
    key          text NOT NULL,
    value        jsonb NOT NULL,
    PRIMARY KEY (extension, character_id, key)
);

CREATE TABLE IF NOT EXISTS content.script_globals (
    extension text NOT NULL REFERENCES content.extensions (name) ON DELETE CASCADE,
    key       text NOT NULL,
    value     jsonb NOT NULL,
    PRIMARY KEY (extension, key)
);

-- `ext scripts` is logged (section 3.3): the log's action list gains it.
ALTER TABLE content.ext_log DROP CONSTRAINT IF EXISTS ext_log_action_check;
ALTER TABLE content.ext_log ADD CONSTRAINT ext_log_action_check
    CHECK (action IN ('install', 'upgrade', 'remove', 'enable', 'disable', 'force-delete', 'scripts'));
