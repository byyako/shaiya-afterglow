-- Game servers known to the login server. Rows are created by an operator (tools `servers add`);
-- a game server only reports its player count and that it is alive (`servers heartbeat`, later the game server itself).
-- The client sends port 30810 and the select reply carries only an IPv4 address, so every game server needs its own IP.
CREATE TABLE game_servers (
    id         smallint    PRIMARY KEY CHECK (id BETWEEN 0 AND 255),
    name       varchar(31) NOT NULL,
    address    inet        NOT NULL CHECK (family(address) = 4 AND masklen(address) = 32),
    capacity   integer     NOT NULL DEFAULT 1000 CHECK (capacity BETWEEN 1 AND 32766),
    -- client version this server requires when version checking is on; NULL accepts any client
    version    bigint      CHECK (version BETWEEN 0 AND 4294967295),
    -- operator switch: false keeps the server closed whatever its heartbeat says
    enabled    boolean     NOT NULL DEFAULT true,
    players    integer     NOT NULL DEFAULT 0 CHECK (players BETWEEN 0 AND 65535),
    last_seen  timestamptz
);

CREATE UNIQUE INDEX game_servers_address_key ON game_servers (address);

-- One active session per account, written by the login server and read by the game server's handshake.
-- It carries the AES key and IV negotiated on the login connection, which the game server needs to derive its own keys.
CREATE TABLE sessions (
    user_id    integer     PRIMARY KEY REFERENCES accounts (id) ON DELETE CASCADE,
    identity   bytea       NOT NULL CHECK (octet_length(identity) = 16),
    aes_key    bytea       NOT NULL CHECK (octet_length(aes_key) = 16),
    aes_iv     bytea       NOT NULL CHECK (octet_length(aes_iv) = 16),
    login_ip   inet        NOT NULL,
    server_id  smallint    REFERENCES game_servers (id) ON DELETE SET NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL
);

CREATE INDEX sessions_expires_idx ON sessions (expires_at);
