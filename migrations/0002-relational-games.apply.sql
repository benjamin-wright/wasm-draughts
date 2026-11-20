-- Keep the original games table as a read-only legacy archive. Existing
-- deployments can overlap while this additive migration imports their saves.
CREATE TABLE matches (
    id TEXT PRIMARY KEY NOT NULL,
    revision INTEGER NOT NULL DEFAULT 0 CHECK (revision >= 0),
    turn INTEGER NOT NULL CHECK (turn IN (-1, 1)),
    forced_square INTEGER CHECK (forced_square BETWEEN 0 AND 63),
    winner INTEGER CHECK (winner IN (-1, 1)),
    turns INTEGER NOT NULL DEFAULT 0 CHECK (turns >= 0)
);

CREATE TABLE pieces (
    match_id TEXT NOT NULL REFERENCES matches(id),
    square INTEGER NOT NULL CHECK (square BETWEEN 0 AND 63 AND (square / 8 + square % 8) % 2 = 1),
    player INTEGER NOT NULL CHECK (player IN (-1, 1)),
    king INTEGER NOT NULL CHECK (king IN (0, 1)),
    PRIMARY KEY (match_id, square)
);

-- Every move and reset reserves a new revision in the same atomic batch as
-- its board changes. History is kept across resets so revisions never repeat.
CREATE TABLE actions (
    match_id TEXT NOT NULL REFERENCES matches(id),
    revision INTEGER NOT NULL CHECK (revision >= 0),
    kind TEXT NOT NULL CHECK (kind IN ('move', 'reset')),
    player INTEGER CHECK (player IN (-1, 1)),
    from_square INTEGER CHECK (from_square BETWEEN 0 AND 63),
    to_square INTEGER CHECK (to_square BETWEEN 0 AND 63),
    captured_square INTEGER CHECK (captured_square BETWEEN 0 AND 63),
    promoted INTEGER NOT NULL DEFAULT 0 CHECK (promoted IN (0, 1)),
    PRIMARY KEY (match_id, revision),
    CHECK (kind != 'move' OR (player IS NOT NULL AND from_square IS NOT NULL AND to_square IS NOT NULL))
);

INSERT INTO matches (id, revision, turn, forced_square, winner, turns)
SELECT id, revision, json_extract(state, '$.turn'), json_extract(state, '$.forced'),
       json_extract(state, '$.winner'), json_extract(state, '$.moves')
FROM games;

INSERT INTO pieces (match_id, square, player, king)
SELECT games.id, CAST(board.key AS INTEGER),
       CASE WHEN board.value > 0 THEN 1 ELSE -1 END,
       CASE WHEN abs(board.value) = 2 THEN 1 ELSE 0 END
FROM games, json_each(games.state, '$.board') AS board
WHERE board.value != 0;

-- Old saves contain only their most recent move, not a full move history.
INSERT INTO actions (match_id, revision, kind, player, from_square, to_square, captured_square)
SELECT id, revision, 'move',
       CASE WHEN json_extract(state, '$.forced') IS NOT NULL
            THEN json_extract(state, '$.turn') ELSE -json_extract(state, '$.turn') END,
       json_extract(state, '$.last_move.from'), json_extract(state, '$.last_move.to'),
       json_extract(state, '$.last_move.capture')
FROM games WHERE json_extract(state, '$.last_move.from') IS NOT NULL;
