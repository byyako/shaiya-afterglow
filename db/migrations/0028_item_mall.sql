-- The item mall and premium accounts (docs/original/item-mall.md section 10). Points belong to the account across every server, as
-- the original's Users_Master.Point; premium is the original's billing type (1 to 3) with an end (deviation 4), NULL = no end.
ALTER TABLE accounts
    ADD COLUMN points        integer     NOT NULL DEFAULT 0 CHECK (points >= 0),
    ADD COLUMN premium       smallint    NOT NULL DEFAULT 0 CHECK (premium BETWEEN 0 AND 3),
    ADD COLUMN premium_until timestamptz NULL;

-- Purchases (kind 1) and gifts sent (kind 2), after the original's PointLog: a character sees its own last 10 at world entry.
CREATE TABLE point_log (
    id           bigint      GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id      integer     NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    character_id integer     NOT NULL REFERENCES characters (id) ON DELETE CASCADE,
    kind         smallint    NOT NULL CHECK (kind IN (1, 2)),
    product_code varchar(20) NOT NULL,
    points       integer     NOT NULL CHECK (points >= 0),
    target_name  varchar(20) NOT NULL DEFAULT '',
    at           timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX point_log_character_idx ON point_log (character_id, at DESC);

-- The item-mall storage (gifts received), 240 slots per account and game server, after UserStoredPointItems.
CREATE TABLE point_items (
    user_id   integer     NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    server_id smallint    NOT NULL CHECK (server_id BETWEEN 0 AND 255),
    slot      smallint    NOT NULL CHECK (slot BETWEEN 0 AND 239),
    item_id   integer     NOT NULL,
    count     smallint    NOT NULL CHECK (count BETWEEN 1 AND 255),
    at        timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, server_id, slot)
);

-- Gifts that came while the account was not in the world, shown (and deleted) at its next world entry, after PointGiftNotify.
CREATE TABLE point_gift_notices (
    id           bigint      GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id      integer     NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    server_id    smallint    NOT NULL CHECK (server_id BETWEEN 0 AND 255),
    product_code varchar(20) NOT NULL,
    sender_name  varchar(20) NOT NULL,
    at           timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX point_gift_notices_user_idx ON point_gift_notices (user_id, server_id, at);

-- The billing storage, one per account for every server as the original's PS_Billing Users_Product, filled by the operator tool
-- (deviation 8).
CREATE TABLE billing_items (
    user_id      integer     NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    slot         smallint    NOT NULL CHECK (slot BETWEEN 0 AND 239),
    item_id      integer     NOT NULL,
    count        smallint    NOT NULL CHECK (count BETWEEN 1 AND 255),
    product_code varchar(20) NOT NULL DEFAULT '',
    at           timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, slot)
);
