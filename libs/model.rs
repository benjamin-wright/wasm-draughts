//! Relational persistence. Each write is an atomic, revision-checked SQL batch.
use super::game::{Game, Move};
use crate::sql::{self, Statement, Value};

#[derive(Debug)]
pub struct Error {
    pub status: u16,
    pub message: &'static str,
}

impl Error {
    pub fn new(status: u16, message: &'static str) -> Self {
        Self { status, message }
    }
    pub fn conflict() -> Self {
        Self::new(
            409,
            "The board changed in another tab. Review the current position and try again.",
        )
    }
}

pub struct SavedGame {
    pub id: String,
    pub revision: i64,
    pub game: Game,
}

fn storage(_: sql::SqlError) -> Error {
    Error::new(
        503,
        "The saved game is temporarily unavailable. Please try again.",
    )
}

fn statement(sql: &str, parameters: Vec<Value>) -> Statement {
    Statement {
        sql: sql.into(),
        parameters,
    }
}

fn text(value: &str) -> Value {
    Value::Text(value.into())
}
fn number(value: impl Into<i64>) -> Value {
    Value::Integer(value.into())
}
fn square(value: Option<usize>) -> Value {
    value.map(|v| number(v as i64)).unwrap_or(Value::Null)
}

pub fn valid_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|c| c.is_ascii_hexdigit())
}

fn initial_pieces(id: &str, game: &Game) -> Vec<Statement> {
    game.board
        .iter()
        .enumerate()
        .filter(|(_, piece)| **piece != 0)
        .map(|(at, piece)| {
            statement(
                "INSERT INTO pieces (match_id, square, player, king) VALUES (?, ?, ?, ?)",
                vec![
                    text(id),
                    number(at as i64),
                    number(piece.signum()),
                    number(i64::from(piece.abs() == 2)),
                ],
            )
        })
        .collect()
}

pub fn create() -> Result<SavedGame, Error> {
    // SQL supplies the session identifier; the WASM guest needs no random API.
    let cursor =
        sql::query(&statement("SELECT lower(hex(randomblob(16)))", vec![])).map_err(storage)?;
    let row = cursor
        .next()
        .map_err(storage)?
        .ok_or_else(|| Error::new(503, "Could not create a game."))?;
    let [Value::Text(id)] = row.as_slice() else {
        return Err(Error::new(503, "Could not create a game."));
    };
    let id = id.clone();
    drop(cursor);
    let game = Game::default();
    let mut statements = vec![statement(
        "INSERT INTO matches (id, turn) VALUES (?, 1)",
        vec![text(&id)],
    )];
    statements.extend(initial_pieces(&id, &game));
    sql::batch(&statements).map_err(storage)?;
    Ok(SavedGame {
        id,
        revision: 0,
        game,
    })
}

pub fn load(id: &str) -> Result<SavedGame, Error> {
    // One cursor provides a consistent snapshot of metadata, pieces, and the
    // latest action, even if another tab commits a move while rows are read.
    let cursor = sql::query(&statement(
        "SELECT m.revision, m.turn, m.forced_square, m.winner, m.turns,
                a.from_square, a.to_square, a.captured_square,
                p.square, p.player, p.king
         FROM matches m
         LEFT JOIN actions a ON a.match_id = m.id AND a.revision = m.revision AND a.kind = 'move'
         LEFT JOIN pieces p ON p.match_id = m.id
         WHERE m.id = ? ORDER BY p.square",
        vec![text(id)],
    ))
    .map_err(storage)?;
    let mut saved = None;
    while let Some(row) = cursor.next().map_err(storage)? {
        let integer = |index: usize| -> Option<i64> {
            match row.get(index) {
                Some(Value::Integer(value)) => Some(*value),
                _ => None,
            }
        };
        let current = saved.get_or_insert_with(|| SavedGame {
            id: id.into(),
            revision: integer(0).unwrap_or(0),
            game: Game {
                board: vec![0; 64],
                turn: integer(1).unwrap_or(1) as i8,
                forced: integer(2).map(|v| v as usize),
                winner: integer(3).map(|v| v as i8),
                moves: integer(4).unwrap_or(0) as u32,
                last_move: integer(5).zip(integer(6)).map(|(from, to)| Move {
                    from: from as usize,
                    to: to as usize,
                    capture: integer(7).map(|v| v as usize),
                }),
            },
        });
        if let (Some(at), Some(player), Some(king)) = (integer(8), integer(9), integer(10)) {
            current.game.board[at as usize] = (player * (1 + king)) as i8;
        }
    }
    saved.ok_or_else(|| Error::new(404, "This saved game was not found."))
}

/// Reserving the next action revision is the first mutation in the batch.
/// A stale revision inserts NULL into a NOT NULL column and aborts the entire
/// batch. This also protects resets without retaining any serialized board.
fn action(saved: &SavedGame, kind: &str, movement: Option<Move>, promoted: bool) -> Statement {
    statement(
        "INSERT INTO actions (match_id, revision, kind, player, from_square, to_square, captured_square, promoted)
         SELECT id, CASE WHEN revision = ? THEN revision + 1 ELSE NULL END, ?, ?, ?, ?, ?, ?
         FROM matches WHERE id = ?",
        vec![number(saved.revision), text(kind), number(saved.game.turn),
            square(movement.map(|m| m.from)), square(movement.map(|m| m.to)),
            square(movement.and_then(|m| m.capture)), number(i64::from(promoted)), text(&saved.id)])
}

fn metadata(saved: &SavedGame, game: &Game) -> Statement {
    statement(
        "UPDATE matches SET revision = revision + 1, turn = ?, forced_square = ?, winner = ?, turns = ? WHERE id = ?",
        vec![number(game.turn), square(game.forced), game.winner.map(number).unwrap_or(Value::Null),
             number(game.moves), text(&saved.id)])
}

fn commit(statements: Vec<Statement>) -> Result<(), Error> {
    sql::batch(&statements).map_err(|error| match error {
        sql::SqlError::Constraint => Error::conflict(),
        other => storage(other),
    })?;
    Ok(())
}

pub fn play(saved: &SavedGame, from: usize, to: usize) -> Result<(), Error> {
    let mut game = saved.game.clone();
    game.play(from, to)
        .map_err(|message| Error::new(422, message))?;
    let movement = game.last_move.expect("A legal move records its squares");
    let promoted = saved.game.board[from].abs() == 1 && game.board[to].abs() == 2;
    let mut statements = vec![action(saved, "move", Some(movement), promoted)];
    if let Some(captured) = movement.capture {
        statements.push(statement(
            "DELETE FROM pieces WHERE match_id = ? AND square = ?",
            vec![text(&saved.id), number(captured as i64)],
        ));
    }
    statements.push(statement(
        "UPDATE pieces SET square = ?, king = ? WHERE match_id = ? AND square = ?",
        vec![
            number(to as i64),
            number(i64::from(game.board[to].abs() == 2)),
            text(&saved.id),
            number(from as i64),
        ],
    ));
    statements.push(metadata(saved, &game));
    commit(statements)
}

pub fn reset(saved: &SavedGame) -> Result<(), Error> {
    let game = Game::default();
    let mut statements = vec![
        action(saved, "reset", None, false),
        statement(
            "DELETE FROM pieces WHERE match_id = ?",
            vec![text(&saved.id)],
        ),
    ];
    statements.extend(initial_pieces(&saved.id, &game));
    statements.push(metadata(saved, &game));
    commit(statements)
}
