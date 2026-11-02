-- Guilds (docs/original/guild.md), after the original's Guilds and GuildChars. A disbanded guild keeps its row with disbanded_at
-- set, and its name is free again. Names are unique without case among the live guilds of a server (CGuildMgr's name map).
CREATE TABLE guilds (
    id           integer     GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    server_id    smallint    NOT NULL CHECK (server_id BETWEEN 0 AND 255),
    name         varchar(24) NOT NULL CHECK (length(name) BETWEEN 2 AND 24),
    country      smallint    NOT NULL CHECK (country IN (0, 1)),
    remark       varchar(64) NOT NULL DEFAULT '',
    created_at   timestamptz NOT NULL DEFAULT now(),
    disbanded_at timestamptz
);
CREATE UNIQUE INDEX guilds_name_key ON guilds (server_id, lower(name)) WHERE disbanded_at IS NULL;

-- One row per character, as usp_Create_GuildChar_E allows: a member (rank 1 the master to 9) or a join request (rank 0). A row
-- leaves when its character leaves the guild, is kicked, is deleted, or its request is withdrawn or answered no.
CREATE TABLE guild_members (
    character_id integer     PRIMARY KEY REFERENCES characters (id) ON DELETE CASCADE,
    guild_id     integer     NOT NULL REFERENCES guilds (id),
    rank         smallint    NOT NULL CHECK (rank BETWEEN 0 AND 9),
    joined_at    timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX guild_members_guild_idx ON guild_members (guild_id);

-- When the character last joined a guild: the 72-hour join penalty runs from there (usp_Read_Char_GuildJoinDate_R).
ALTER TABLE characters ADD COLUMN guild_joined_at timestamptz;
