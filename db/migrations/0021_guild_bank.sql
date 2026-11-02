-- The guild bank (docs/original/guild-bank.md), 240 slots per guild, after the original GuildStoredItems table. Bag 255 in the
-- packets. An item moved in from a bag keeps its uid (item_uid_seq, migration 0006). The rows of a disbanded guild are kept with its
-- guilds row and never loaded.
CREATE TABLE guild_bank_items (
    guild_id   integer     NOT NULL REFERENCES guilds (id),
    slot       smallint    NOT NULL CHECK (slot BETWEEN 0 AND 239),
    item_id    integer     NOT NULL,
    type       smallint    NOT NULL CHECK (type BETWEEN 0 AND 255),
    type_id    smallint    NOT NULL CHECK (type_id BETWEEN 0 AND 255),
    quality    integer     NOT NULL DEFAULT 0 CHECK (quality BETWEEN 0 AND 65535),
    gems       bytea       NOT NULL DEFAULT '\x000000000000' CHECK (length(gems) = 6),
    count      smallint    NOT NULL DEFAULT 1 CHECK (count BETWEEN 1 AND 255),
    craft_name varchar(20) NOT NULL DEFAULT '',
    make_type  varchar(1)  NOT NULL DEFAULT '',
    made_at    timestamptz NOT NULL DEFAULT now(),
    uid        bigint      NOT NULL DEFAULT nextval('item_uid_seq') UNIQUE,
    -- Deferrable for the same reason as bank_items (migration 0014): a swap is one UPDATE.
    CONSTRAINT guild_bank_items_pkey PRIMARY KEY (guild_id, slot) DEFERRABLE INITIALLY IMMEDIATE
);
