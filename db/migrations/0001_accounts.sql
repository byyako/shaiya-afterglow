CREATE TABLE accounts (
    id            integer GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    username      varchar(12) NOT NULL,
    password_hash text        NOT NULL,
    -- 0 normal; non-zero values are sent to the client as the privilege byte (GM level)
    privilege     smallint    NOT NULL DEFAULT 0 CHECK (privilege BETWEEN 0 AND 255),
    blocked_until timestamptz,
    deleted       boolean     NOT NULL DEFAULT false,
    created_at    timestamptz NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX accounts_username_key ON accounts (lower(username));

CREATE TABLE login_log (
    id         bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    account_id integer     REFERENCES accounts (id),
    username   varchar(32) NOT NULL,
    ip         inet        NOT NULL,
    outcome    text        NOT NULL,
    at         timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX login_log_account_idx ON login_log (account_id, at DESC);
