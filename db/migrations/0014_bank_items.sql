-- The warehouse (docs/original/warehouse.md): 240 slots per account and game server, after the original UserStoredItems table.
-- Bag 100 in the packets. An item moved in from a bag keeps its uid (item_uid_seq, migration 0006). Warehouse money is not stored:
-- nothing in EP4.5 can change it, so it is always 0.
CREATE TABLE bank_items (
    user_id    integer     NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    server_id  smallint    NOT NULL CHECK (server_id BETWEEN 0 AND 255),
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
    -- Deferrable for the same reason as character_items (migration 0007): a swap is one UPDATE.
    CONSTRAINT bank_items_pkey PRIMARY KEY (user_id, server_id, slot) DEFERRABLE INITIALLY IMMEDIATE
);
