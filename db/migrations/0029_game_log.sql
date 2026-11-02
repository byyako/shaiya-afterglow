-- The game log (docs/original/game-log.md deviation 1): what the original's ps_game sent to ps_gameLog, written here by the game server
-- itself. game_log holds PS_GameLog.dbo.ActionLog (one row per record, the ActionType numbers and the placement of values and texts as
-- the original's writers make them), chat_log PS_ChatLog.dbo.ChatLog. Both carry the game server's id: the original kept one set of log
-- databases per game server. No foreign keys: the log outlives deleted characters and accounts. Nothing deletes rows unless the
-- operator sets GAME_LOG_DAYS (deviation 5).
CREATE TABLE game_log (
    id             bigint       GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    server_id      smallint     NOT NULL,
    -- The account's name and id, the character's id, name, level and experience, its map and position: the header of section 2. The
    -- records without a header leave the user columns empty (0 and ''); see section 3.
    user_name      varchar(32)  NOT NULL DEFAULT '',
    user_id        integer      NOT NULL DEFAULT 0,
    character_id   integer      NOT NULL DEFAULT 0,
    character_name varchar(50)  NOT NULL DEFAULT '',
    level          smallint     NOT NULL DEFAULT 0,
    exp            bigint       NOT NULL DEFAULT 0,
    map            smallint     NOT NULL DEFAULT 0,
    x              real         NOT NULL DEFAULT 0,
    y              real         NOT NULL DEFAULT 0,
    z              real         NOT NULL DEFAULT 0,
    at             timestamptz  NOT NULL,
    -- The original's ActionType (section 3), decimal.
    action         smallint     NOT NULL,
    -- Value1 .. Value10 and Text1 .. Text4, NULL where a record does not use them. For the item records value1 is the item's unique id
    -- (character_items.uid) and value2 the item id.
    value1         bigint,
    value2         bigint,
    value3         bigint,
    value4         bigint,
    value5         bigint,
    value6         bigint,
    value7         bigint,
    value8         bigint,
    value9         bigint,
    value10        bigint,
    text1          varchar(100),
    text2          varchar(100),
    text3          varchar(100),
    text4          varchar(100)
);
CREATE INDEX game_log_character_idx ON game_log (character_id, at);
CREATE INDEX game_log_action_idx ON game_log (action, at);
CREATE INDEX game_log_at_idx ON game_log (at);
-- To follow an item by its unique id (docs/original/database-notes.md, "following an item").
CREATE INDEX game_log_value1_idx ON game_log (value1) WHERE value1 IS NOT NULL;

-- Every chat line of types 1 to 7 and the staff's chat (section 3.5), after ChatLog; target_name is the whisper's target.
CREATE TABLE chat_log (
    id           bigint       GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    server_id    smallint     NOT NULL,
    user_id      integer      NOT NULL,
    character_id integer      NOT NULL,
    chat_type    smallint     NOT NULL,
    target_name  varchar(50),
    text         varchar(200) NOT NULL,
    map          smallint     NOT NULL,
    at           timestamptz  NOT NULL
);
CREATE INDEX chat_log_character_idx ON chat_log (character_id, at);
CREATE INDEX chat_log_at_idx ON chat_log (at);
