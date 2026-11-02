-- The auction house (docs/original/auction.md section 15), after the original's Market and MarketItems (an auction and its item),
-- MarketCharResultItems and MarketCharResultMoney (what waits to be collected) and MarketCharConcern (the interest list). The
-- original's database agent seeds its market and money id counters from the highest ids it loads; here two sequences make them.
-- Money columns hold the original's u32 amounts. Every row is changed in one transaction with the gold and the bags it touches.
CREATE SEQUENCE market_id_seq;
CREATE SEQUENCE market_money_id_seq;

-- An auction (CMarket): the seller, the end time, the prices, the period (0 to 3 = 7, 24, 72, 168 hours), the deposit (the
-- registration fee, paid back only on a sale), the top bidder, the current price (the minimum until the first bid, then the top
-- bid), and the item with every column of its character_items row (its uid kept when the whole stack was registered).
CREATE TABLE market_auctions (
    id           integer     PRIMARY KEY DEFAULT nextval('market_id_seq'),
    server_id    smallint    NOT NULL,
    seller_id    integer     NOT NULL REFERENCES characters (id),
    seller_name  varchar(20) NOT NULL,
    ends_at      timestamptz NOT NULL,
    min_money    bigint      NOT NULL CHECK (min_money BETWEEN 0 AND 4294967295),
    direct_money bigint      NOT NULL CHECK (direct_money BETWEEN 0 AND 4294967295),
    period       smallint    NOT NULL CHECK (period BETWEEN 0 AND 3),
    deposit      bigint      NOT NULL CHECK (deposit BETWEEN 0 AND 4294967295),
    bidder_id    integer     NULL REFERENCES characters (id),
    bidder_name  varchar(20) NOT NULL DEFAULT '',
    price        bigint      NOT NULL CHECK (price BETWEEN 0 AND 4294967295),
    item_id      integer     NOT NULL,
    type         smallint    NOT NULL CHECK (type BETWEEN 0 AND 255),
    type_id      smallint    NOT NULL CHECK (type_id BETWEEN 0 AND 255),
    quality      integer     NOT NULL DEFAULT 0 CHECK (quality BETWEEN 0 AND 65535),
    gems         bytea       NOT NULL DEFAULT '\x000000000000' CHECK (length(gems) = 6),
    count        smallint    NOT NULL DEFAULT 1 CHECK (count BETWEEN 1 AND 255),
    craft_name   varchar(20) NOT NULL DEFAULT '',
    make_type    varchar(1)  NOT NULL DEFAULT '',
    made_at      timestamptz NOT NULL DEFAULT now(),
    uid          bigint      NOT NULL DEFAULT nextval('item_uid_seq') UNIQUE
);
CREATE INDEX market_auctions_server ON market_auctions (server_id);

-- An item to collect (ResultItem), one per ended auction: result 0 won or bought, 1 back to the seller (cancelled or unsold), 2 taken
-- by a GM's /auctionrecall. The deadline is 14 days after the row was made; the end pass deletes rows past it (deviation 1).
CREATE TABLE market_result_items (
    market_id    integer     PRIMARY KEY,
    character_id integer     NOT NULL REFERENCES characters (id),
    result       smallint    NOT NULL CHECK (result BETWEEN 0 AND 2),
    deadline     timestamptz NOT NULL,
    item_id      integer     NOT NULL,
    type         smallint    NOT NULL CHECK (type BETWEEN 0 AND 255),
    type_id      smallint    NOT NULL CHECK (type_id BETWEEN 0 AND 255),
    quality      integer     NOT NULL DEFAULT 0 CHECK (quality BETWEEN 0 AND 65535),
    gems         bytea       NOT NULL DEFAULT '\x000000000000' CHECK (length(gems) = 6),
    count        smallint    NOT NULL DEFAULT 1 CHECK (count BETWEEN 1 AND 255),
    craft_name   varchar(20) NOT NULL DEFAULT '',
    make_type    varchar(1)  NOT NULL DEFAULT '',
    made_at      timestamptz NOT NULL DEFAULT now(),
    uid          bigint      NOT NULL DEFAULT nextval('item_uid_seq') UNIQUE
);
CREATE INDEX market_result_items_character ON market_result_items (character_id);

-- Gold to collect (ResultMoney): result 0 a sale (money the price, return = deposit - 2 % of the price + the price), 1 a refund (an
-- outbid or cancelled bid, or a recalled auction's deposit). Collecting pays return_money. The item's columns are for display only.
CREATE TABLE market_result_money (
    id           integer     PRIMARY KEY DEFAULT nextval('market_money_id_seq'),
    market_id    integer     NOT NULL,
    character_id integer     NOT NULL REFERENCES characters (id),
    result       smallint    NOT NULL CHECK (result BETWEEN 0 AND 1),
    money        bigint      NOT NULL CHECK (money BETWEEN 0 AND 4294967295),
    deposit      bigint      NOT NULL CHECK (deposit BETWEEN 0 AND 4294967295),
    return_money bigint      NOT NULL CHECK (return_money BETWEEN 0 AND 4294967295),
    deadline     timestamptz NOT NULL,
    type         smallint    NOT NULL CHECK (type BETWEEN 0 AND 255),
    type_id      smallint    NOT NULL CHECK (type_id BETWEEN 0 AND 255),
    quality      integer     NOT NULL DEFAULT 0 CHECK (quality BETWEEN 0 AND 65535),
    gems         bytea       NOT NULL DEFAULT '\x000000000000' CHECK (length(gems) = 6),
    count        smallint    NOT NULL DEFAULT 1 CHECK (count BETWEEN 1 AND 255),
    craft_name   varchar(20) NOT NULL DEFAULT ''
);
CREATE INDEX market_result_money_character ON market_result_money (character_id);

-- The interest list (CUser +0x5d14), ten auctions at most per character, written whole after each change. Ids of auctions that
-- have ended stay until the next add removes them, as in the original.
CREATE TABLE market_interest (
    character_id integer  NOT NULL REFERENCES characters (id) ON DELETE CASCADE,
    position     smallint NOT NULL CHECK (position BETWEEN 0 AND 9),
    market_id    integer  NOT NULL,
    PRIMARY KEY (character_id, position)
);
