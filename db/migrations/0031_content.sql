-- The database as the source of truth for the game's content (docs/plans/ext-1-foundation.md section 3): which original files were
-- imported, which extensions are installed and the rows each owns, the client-only fields of the rows an extension changes, the ledger
-- of generated client patches, and the data editor's layer of game-server settings. The schema is `content`, apart from `defs` and
-- `world` (the live tables, rebuilt from the frozen originals, the extensions and the edits) and from `edits`; no load replaces it.
-- Every statement is guarded (IF NOT EXISTS) because the tests of other binaries run all migrations again inside throwaway schemas of
-- the same database (as in 0030).
CREATE SCHEMA IF NOT EXISTS content;

-- One per imported part (3.2): the importers write it with the frozen copy (`original_defs`, `original_world`, `client`).
CREATE TABLE IF NOT EXISTS content.imports (
    part        text PRIMARY KEY CHECK (part IN ('defs', 'world', 'client')),
    imported_at timestamptz NOT NULL DEFAULT now(),
    -- the last content rebuild of the part; NULL or older than imported_at: a rebuild is due
    rebuilt_at  timestamptz,
    -- what the caller said it read (--source), or the folder
    source      text NOT NULL,
    -- {file name: sha256}: "the same files" of a re-run
    files       jsonb NOT NULL,
    -- {table: {"rows": n, "md5": "…"}} of the frozen copy (client: of client.items and client.skills)
    tables      jsonb NOT NULL
);

-- The installed extensions (5.7); `install_seq` is the order their rows are written in.
CREATE TABLE IF NOT EXISTS content.extensions (
    name         text PRIMARY KEY CHECK (name ~ '^[a-z][a-z0-9-]{1,39}$'),
    version      text NOT NULL,
    title        text NOT NULL,
    description  text NOT NULL,
    -- extension.toml as read
    manifest     jsonb NOT NULL,
    enabled      boolean NOT NULL DEFAULT true,
    install_seq  integer GENERATED ALWAYS AS IDENTITY UNIQUE,
    installed_at timestamptz NOT NULL DEFAULT now(),
    upgraded_at  timestamptz
);

-- The ids an extension owns (5.6), expanded: items major = type, minor = typeid; skills major = skill id, minor = level.
CREATE TABLE IF NOT EXISTS content.ext_ranges (
    extension  text NOT NULL REFERENCES content.extensions (name) ON DELETE CASCADE,
    kind       text NOT NULL CHECK (kind IN ('item', 'skill')),
    major      integer NOT NULL,
    minor_low  integer NOT NULL,
    minor_high integer NOT NULL CHECK (minor_high >= minor_low),
    PRIMARY KEY (extension, kind, major, minor_low)
);

-- The rows an extension owns (3.3); one owner per row. `row` is the whole row as written, so a rebuild needs no folder.
CREATE TABLE IF NOT EXISTS content.ext_rows (
    extension text NOT NULL REFERENCES content.extensions (name),
    -- 'defs.items', 'defs.skills', 'defs.mob_items'
    tbl       text NOT NULL,
    -- the data editor's key text: '100212', '200/1', '2133/4'
    key       text NOT NULL,
    action    text NOT NULL CHECK (action IN ('add', 'change')),
    row       jsonb NOT NULL,
    -- its place in the rows files
    seq       integer NOT NULL,
    PRIMARY KEY (tbl, key)
);

-- An extension's defaults for game settings (7.1, layer 2); they count while the extension is enabled.
CREATE TABLE IF NOT EXISTS content.ext_settings (
    extension text NOT NULL REFERENCES content.extensions (name) ON DELETE CASCADE,
    name      text NOT NULL,
    value     text NOT NULL,
    PRIMARY KEY (extension, name)
);

-- The migrations an extension ran in its own schema ext_<name> (5.4).
CREATE TABLE IF NOT EXISTS content.ext_migrations (
    extension  text NOT NULL REFERENCES content.extensions (name) ON DELETE CASCADE,
    version    integer NOT NULL,
    name       text NOT NULL,
    sha256     text NOT NULL,
    applied_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (extension, version)
);

-- What an extension needs of the client: fixes.dll features and art (5.2, 4.3). `name` is the feature name, or the archive member path.
CREATE TABLE IF NOT EXISTS content.ext_client (
    extension text NOT NULL REFERENCES content.extensions (name) ON DELETE CASCADE,
    kind      text NOT NULL CHECK (kind IN ('feature', 'art')),
    name      text NOT NULL,
    -- art: the file in the folder and its hash
    file      text,
    sha256    text,
    PRIMARY KEY (extension, kind, name)
);

-- What was done to the content and when (install, upgrade, remove, enable, disable, the held rows a forced remove deleted).
CREATE TABLE IF NOT EXISTS content.ext_log (
    id        integer GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    at        timestamptz NOT NULL DEFAULT now(),
    extension text NOT NULL,
    action    text NOT NULL CHECK (action IN ('install', 'upgrade', 'remove', 'enable', 'disable', 'force-delete')),
    detail    jsonb NOT NULL
);

-- The client rows (4.1): the owner, the text (or keep = the client's own), the look-alike as a reference and the fields given by name
-- (NULL: not given); client_patch.py resolves them against the client schema. since_patch is stamped by `generate`.
CREATE TABLE IF NOT EXISTS content.client_items (
    type smallint NOT NULL, typeid smallint NOT NULL,
    extension text NOT NULL REFERENCES content.extensions (name),
    keep boolean NOT NULL DEFAULT false,
    name text, description text,
    look_type smallint, look_typeid smallint,
    model smallint, icon smallint, type2 smallint, type3 smallint,
    since_patch integer, note text NOT NULL,
    PRIMARY KEY (type, typeid),
    CHECK (keep OR (name IS NOT NULL AND description IS NOT NULL)),
    CHECK ((look_type IS NULL) = (look_typeid IS NULL))
);
CREATE TABLE IF NOT EXISTS content.client_skills (
    skill_id smallint NOT NULL, level smallint NOT NULL,
    extension text NOT NULL REFERENCES content.extensions (name),
    keep boolean NOT NULL DEFAULT false,
    name text, description text,
    look_skill smallint, look_level smallint,
    icon integer, animation integer, toggle_type smallint, sound integer,
    since_patch integer, note text NOT NULL,
    PRIMARY KEY (skill_id, level),
    CHECK (keep OR (name IS NOT NULL AND description IS NOT NULL)),
    CHECK ((look_skill IS NULL) = (look_level IS NULL))
);
-- Item and skill changes the client needs nothing of (the reason is the extension's).
CREATE TABLE IF NOT EXISTS content.server_only (
    kind text NOT NULL CHECK (kind IN ('item', 'skill')), id integer NOT NULL, level smallint NOT NULL DEFAULT 0,
    extension text NOT NULL REFERENCES content.extensions (name),
    reason text NOT NULL,
    PRIMARY KEY (kind, id, level)
);

-- The patch ledger (4.2): one row per generated client patch, and one per member with its encrypted bytes. `commit` is the commit the
-- manifest names (patch 109: the old private tree's); `extensions` is {name: version}; the client side's results come with `record`.
CREATE TABLE IF NOT EXISTS content.client_patches (
    patch            integer PRIMARY KEY,
    previous         integer NOT NULL,
    generated_at     timestamptz NOT NULL DEFAULT now(),
    commit           text NOT NULL,
    extensions       jsonb NOT NULL,
    dirty            boolean NOT NULL,
    patch_sha256     text,
    sig_sha256       text,
    status           text NOT NULL DEFAULT 'generated' CHECK (status IN ('generated', 'staged', 'live', 'withdrawn'))
);
CREATE TABLE IF NOT EXISTS content.client_patch_files (
    patch           integer NOT NULL REFERENCES content.client_patches (patch),
    member          text NOT NULL,
    us_path         text NOT NULL,
    size            integer NOT NULL,
    sha256          text NOT NULL,
    plain_sha256    text NOT NULL,
    plain_size      integer NOT NULL,
    replaces_sha256 text NOT NULL,
    changed         text[] NOT NULL,
    encrypted       bytea NOT NULL,
    PRIMARY KEY (patch, member)
);

-- The data editor's settings layer (7.4): an override is a row; removing it falls back to the layers below.
CREATE TABLE IF NOT EXISTS content.setting_values (
    name  text PRIMARY KEY,
    value text NOT NULL
);
