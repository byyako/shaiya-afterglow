-- Friends and blocked players (docs/original/friends.md section 11), as the original's FriendChars and BanChars: one row per character
-- and friend (or blocked player), with the memo. The names, jobs and deletions come from characters when the lists are loaded. A pair
-- can be stored once only (the original had no key and could store a friend twice).
CREATE TABLE character_friends (
    character_id integer     NOT NULL REFERENCES characters (id) ON DELETE CASCADE,
    friend_id    integer     NOT NULL REFERENCES characters (id) ON DELETE CASCADE CHECK (friend_id <> character_id),
    memo         varchar(50) NOT NULL DEFAULT '',
    created_at   timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (character_id, friend_id)
);

CREATE TABLE character_blocks (
    character_id integer     NOT NULL REFERENCES characters (id) ON DELETE CASCADE,
    blocked_id   integer     NOT NULL REFERENCES characters (id) ON DELETE CASCADE CHECK (blocked_id <> character_id),
    memo         varchar(50) NOT NULL DEFAULT '',
    created_at   timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (character_id, blocked_id)
);
