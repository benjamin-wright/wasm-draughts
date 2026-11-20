/// Positive pieces are charcoal (moving up); negative pieces are ivory.
/// A magnitude of two denotes a king. Squares are row-major, starting at a8.
#[derive(Clone, Debug)]
pub struct Game {
    pub board: Vec<i8>,
    pub turn: i8,
    pub forced: Option<usize>,
    pub winner: Option<i8>,
    pub moves: u32,
    pub last_move: Option<Move>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Move {
    pub from: usize,
    pub to: usize,
    pub capture: Option<usize>,
}

impl Default for Game {
    fn default() -> Self {
        let mut board = vec![0; 64];
        for (square, piece) in board.iter_mut().enumerate() {
            let row = square / 8;
            if (row + square % 8) % 2 == 1 {
                *piece = if row < 3 {
                    -1
                } else if row > 4 {
                    1
                } else {
                    0
                };
            }
        }
        Self {
            board,
            turn: 1,
            forced: None,
            winner: None,
            moves: 0,
            last_move: None,
        }
    }
}

impl Game {
    fn moves_from(&self, from: usize, captures_only: bool) -> Vec<Move> {
        let piece = self.board[from];
        if piece == 0 || piece.signum() != self.turn {
            return Vec::new();
        }
        let mut moves = Vec::new();
        for dr in [-1, 1] {
            if piece.abs() == 1 && dr != -piece.signum() {
                continue;
            }
            for dc in [-1, 1] {
                let row = from as i8 / 8 + dr;
                let col = from as i8 % 8 + dc;
                let Some(next) = square(row, col) else {
                    continue;
                };
                if self.board[next] == 0 && !captures_only {
                    moves.push(Move {
                        from,
                        to: next,
                        capture: None,
                    });
                } else if self.board[next].signum() == -self.turn {
                    if let Some(to) = square(row + dr, col + dc) {
                        if self.board[to] == 0 {
                            moves.push(Move {
                                from,
                                to,
                                capture: Some(next),
                            });
                        }
                    }
                }
            }
        }
        moves
    }

    pub fn legal_moves(&self) -> Vec<Move> {
        if self.winner.is_some() {
            return Vec::new();
        }
        if let Some(from) = self.forced {
            return self.moves_from(from, true);
        }
        let all: Vec<_> = (0..64)
            .flat_map(|from| self.moves_from(from, false))
            .collect();
        if all.iter().any(|m| m.capture.is_some()) {
            all.into_iter().filter(|m| m.capture.is_some()).collect()
        } else {
            all
        }
    }

    pub fn play(&mut self, from: usize, to: usize) -> Result<(), &'static str> {
        if self.winner.is_some() {
            return Err("This game has finished. Start a new game to play again.");
        }
        let movement = self.legal_moves().into_iter().find(|m| m.from == from && m.to == to)
            .ok_or("That move is not legal. Choose a highlighted destination; captures are compulsory.")?;
        let piece = self.board[from];
        self.board[from] = 0;
        self.board[to] = piece;
        if let Some(captured) = movement.capture {
            self.board[captured] = 0;
        }
        let promoted =
            piece.abs() == 1 && ((piece > 0 && to / 8 == 0) || (piece < 0 && to / 8 == 7));
        if promoted {
            self.board[to] *= 2;
        }
        self.last_move = Some(movement);
        if movement.capture.is_some() && !promoted && !self.moves_from(to, true).is_empty() {
            self.forced = Some(to);
            return Ok(());
        }
        self.forced = None;
        self.moves += 1;
        self.turn = -self.turn;
        if self.legal_moves().is_empty() {
            self.winner = Some(-self.turn);
        }
        Ok(())
    }
}

fn square(row: i8, col: i8) -> Option<usize> {
    ((0..8).contains(&row) && (0..8).contains(&col)).then_some((row * 8 + col) as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn position(pieces: &[(usize, i8)]) -> Game {
        let mut game = Game {
            board: vec![0; 64],
            ..Game::default()
        };
        for &(square, piece) in pieces {
            game.board[square] = piece;
        }
        game
    }

    #[test]
    fn opening_and_alternating_turns() {
        let mut game = Game::default();
        assert_eq!(game.board.iter().filter(|&&p| p == 1).count(), 12);
        assert_eq!(game.board.iter().filter(|&&p| p == -1).count(), 12);
        assert_eq!(game.legal_moves().len(), 7);
        game.play(40, 33).unwrap();
        assert_eq!(game.turn, -1);
        assert!(game.play(42, 35).is_err());
        game.play(17, 24).unwrap();
        assert_eq!(game.turn, 1);
        assert_eq!(game.moves, 2);
    }

    #[test]
    fn mandatory_capture_and_forced_chain() {
        let mut game = position(&[(42, 1), (46, 1), (35, -1), (21, -1), (7, -1)]);
        assert!(game.play(46, 37).is_err());
        game.play(42, 28).unwrap();
        assert_eq!(game.board[35], 0);
        assert_eq!(game.forced, Some(28));
        assert_eq!(game.turn, 1);
        assert!(game.play(46, 37).is_err());
        game.play(28, 14).unwrap();
        assert_eq!(game.board[21], 0);
        assert_eq!(game.forced, None);
        assert_eq!(game.turn, -1);
        assert_eq!(game.moves, 1);
    }

    #[test]
    fn promotion_ends_capture_turn() {
        let mut game = position(&[(17, 1), (10, -1), (12, -1)]);
        game.play(17, 3).unwrap();
        assert_eq!(game.board[3], 2);
        assert_eq!(game.turn, -1);
        assert_eq!(game.forced, None);
        assert_eq!(game.board[12], -1);
    }

    #[test]
    fn kings_move_and_capture_backwards_but_men_cannot() {
        let mut man = position(&[(28, 1), (37, -1)]);
        assert!(man.play(28, 46).is_err());
        assert!(man.play(28, 35).is_err());
        let mut king = position(&[(28, 2), (37, -1)]);
        king.play(28, 46).unwrap();
        assert_eq!(king.winner, Some(1));
        assert!(king.play(46, 37).is_err());
    }

    #[test]
    fn blocked_opponent_loses_and_bad_indices_are_rejected() {
        let mut game = position(&[(42, 1), (56, -1)]);
        let before = game.board.clone();
        assert!(game.play(usize::MAX, 90).is_err());
        assert_eq!(game.board, before);
        game.play(42, 33).unwrap();
        assert_eq!(game.winner, Some(1));
    }

    #[test]
    fn ivory_promotes_and_edges_do_not_wrap() {
        let mut game = position(&[(55, -1), (40, 1)]);
        game.turn = -1;
        game.play(55, 62).unwrap();
        assert_eq!(game.board[62], -2);
        let edge = position(&[(24, 2), (17, -1)]);
        assert!(edge
            .legal_moves()
            .iter()
            .all(|m| m.from == 24 && m.to == 10));
    }
}
