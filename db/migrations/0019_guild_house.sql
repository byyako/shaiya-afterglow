-- Guild houses (docs/original/guild-house.md section 1), after the original's GuildDetails and GuildNpcLv. ranking 31 = unranked;
-- use_house 0 not open yet, 1 open, 2 closed for want of Etin.
ALTER TABLE guilds
    ADD COLUMN ranking      smallint NOT NULL DEFAULT 31 CHECK (ranking BETWEEN 1 AND 31),
    ADD COLUMN points       bigint   NOT NULL DEFAULT 0 CHECK (points >= 0),
    ADD COLUMN etin         bigint   NOT NULL DEFAULT 0 CHECK (etin BETWEEN 0 AND 4294967295),
    ADD COLUMN etin_returns integer  NOT NULL DEFAULT 0 CHECK (etin_returns >= 0),
    ADD COLUMN keep_etin    bigint   NOT NULL DEFAULT 2000 CHECK (keep_etin >= 0),
    ADD COLUMN use_house    smallint NOT NULL DEFAULT 0 CHECK (use_house IN (0, 1, 2)),
    ADD COLUMN buy_house    boolean  NOT NULL DEFAULT false;

-- The house NPCs' levels: one row per used slot (0 to 7) of a guild; npc_key 2 gatekeeper, 3 blacksmith, 6 warehouse, shop type << 8
-- | 1 a merchant.
CREATE TABLE guild_npc_levels (
    guild_id   integer  NOT NULL REFERENCES guilds (id),
    slot       smallint NOT NULL CHECK (slot BETWEEN 0 AND 7),
    npc_key    integer  NOT NULL,
    level      smallint NOT NULL CHECK (level BETWEEN 1 AND 5),
    PRIMARY KEY (guild_id, slot),
    UNIQUE (guild_id, npc_key)
);
