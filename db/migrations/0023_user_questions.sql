-- Operator (GM) commands part 2 (docs/original/gm-commands-2.md, deviation 1): the players' questions to the staff (`/request GM`),
-- kept with their answers. taken_by and answered_by are character ids without a foreign key, so a deleted GM character does not
-- lose the record.
CREATE TABLE user_questions (
    id serial PRIMARY KEY,
    server_id smallint NOT NULL,
    character_id integer NOT NULL REFERENCES characters (id) ON DELETE CASCADE,
    character_name varchar(20) NOT NULL,
    question bytea NOT NULL CHECK (length(question) BETWEEN 1 AND 1000),
    asked_at timestamptz NOT NULL DEFAULT now(),
    taken_by integer,
    answered_at timestamptz,
    answered_by integer,
    answer_kind smallint CHECK (answer_kind BETWEEN 1 AND 4),
    answer bytea,
    answer_to varchar(20)
);
CREATE INDEX user_questions_open ON user_questions (server_id, id) WHERE answered_at IS NULL;
