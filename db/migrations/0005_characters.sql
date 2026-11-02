-- The game server's handshake uses a session once (the original refused a second game connection for the same selection).
-- A newer login replaces the session row and clears this again.
ALTER TABLE sessions ADD COLUMN game_connected_at timestamptz;

-- Faction per account and game server (the original's UserMaxGrow). No row means no faction chosen yet,
-- which the client is told as country 2. The original always reported max grow 3 (ultimate allowed);
-- the column keeps the stored value so that rule can change later.
CREATE TABLE account_factions (
    user_id   integer  NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    server_id smallint NOT NULL CHECK (server_id BETWEEN 0 AND 255),
    country   smallint NOT NULL CHECK (country IN (0, 1)),
    max_grow  smallint NOT NULL DEFAULT 3 CHECK (max_grow BETWEEN 0 AND 3),
    PRIMARY KEY (user_id, server_id)
);

-- Characters, after the original Chars table (docs/original/ps_game.md section 6). server_id is deliberately not a
-- foreign key to game_servers: removing a server entry must never remove characters.
CREATE TABLE characters (
    id            integer     GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id       integer     NOT NULL REFERENCES accounts (id),
    server_id     smallint    NOT NULL CHECK (server_id BETWEEN 0 AND 255),
    slot          smallint    NOT NULL CHECK (slot BETWEEN 0 AND 4),
    name          varchar(20) NOT NULL CHECK (length(name) BETWEEN 2 AND 20),
    family        smallint    NOT NULL CHECK (family BETWEEN 0 AND 3),
    grow          smallint    NOT NULL CHECK (grow BETWEEN 0 AND 3),
    hair          smallint    NOT NULL CHECK (hair BETWEEN 0 AND 255),
    face          smallint    NOT NULL CHECK (face BETWEEN 0 AND 255),
    size          smallint    NOT NULL CHECK (size BETWEEN 0 AND 255),
    job           smallint    NOT NULL CHECK (job BETWEEN 0 AND 5),
    sex           smallint    NOT NULL CHECK (sex BETWEEN 0 AND 1),
    level         smallint    NOT NULL DEFAULT 1 CHECK (level >= 0),
    stat_point    smallint    NOT NULL DEFAULT 0 CHECK (stat_point >= 0),
    skill_point   smallint    NOT NULL DEFAULT 0 CHECK (skill_point >= 0),
    str           smallint    NOT NULL CHECK (str >= 0),
    dex           smallint    NOT NULL CHECK (dex >= 0),
    rec           smallint    NOT NULL CHECK (rec >= 0),
    int           smallint    NOT NULL CHECK (int >= 0),
    wis           smallint    NOT NULL CHECK (wis >= 0),
    luc           smallint    NOT NULL CHECK (luc >= 0),
    hp            smallint    NOT NULL CHECK (hp >= 0),
    mp            smallint    NOT NULL CHECK (mp >= 0),
    sp            smallint    NOT NULL CHECK (sp >= 0),
    map           smallint    NOT NULL CHECK (map >= 0),
    dir           smallint    NOT NULL DEFAULT 0,
    exp           bigint      NOT NULL DEFAULT 0 CHECK (exp BETWEEN 0 AND 4294967295),
    money         bigint      NOT NULL DEFAULT 0 CHECK (money BETWEEN 0 AND 4294967295),
    pos_x         real        NOT NULL,
    pos_y         real        NOT NULL,
    pos_z         real        NOT NULL,
    -- the original's Hg, Vg, Cg, Og, Ig (meaning not yet known; stored as the original did)
    hg            smallint    NOT NULL DEFAULT 0,
    vg            smallint    NOT NULL DEFAULT 0,
    cg            smallint    NOT NULL DEFAULT 0,
    og            smallint    NOT NULL DEFAULT 0,
    ig            smallint    NOT NULL DEFAULT 0,
    kills         integer     NOT NULL DEFAULT 0,
    deaths        integer     NOT NULL DEFAULT 0,
    victories     integer     NOT NULL DEFAULT 0,
    defeats       integer     NOT NULL DEFAULT 0,
    rename_allowed boolean    NOT NULL DEFAULT false,
    created_at    timestamptz NOT NULL DEFAULT now(),
    -- deletion requested; the character is removed two hours later unless restored (the original's RemainTime)
    delete_requested_at timestamptz,
    -- removed; the row is kept (the original's Del = 1 and DeleteDate)
    deleted_at    timestamptz
);

-- One live character per slot, and live names unique across all servers ignoring case (the original checked every server).
CREATE UNIQUE INDEX characters_slot_key ON characters (user_id, server_id, slot) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX characters_name_key ON characters (lower(name)) WHERE deleted_at IS NULL;
CREATE INDEX characters_deleted_name_idx ON characters (lower(name), deleted_at) WHERE deleted_at IS NOT NULL;
