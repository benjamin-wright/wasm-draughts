//! Presentation-only view models. Rules and persistence remain in the model.
use super::{
    controllers::{self, BASE},
    model::{Error, SavedGame},
};
use crate::Response;
use askama::Template;

pub struct Square {
    pub index: usize,
    pub classes: String,
    pub label: String,
    pub piece_class: &'static str,
    pub occupied: bool,
    pub king: bool,
    pub selected: bool,
    pub selectable: bool,
    pub destination: bool,
    pub href: String,
}

pub struct Player {
    pub name: &'static str,
    pub class: &'static str,
    pub count: usize,
    pub kings: usize,
    pub captured: usize,
    pub active: bool,
}

#[derive(Template)]
#[template(path = "index.html")]
struct Board<'a> {
    base: &'static str,
    token: &'a str,
    revision: i64,
    squares: Vec<Square>,
    players: Vec<Player>,
    files: Vec<char>,
    ranks: Vec<char>,
    flipped: u8,
    flip_href: String,
    select_from: usize,
    has_selection: bool,
    forced: bool,
    capture_available: bool,
    finished: bool,
    player: &'static str,
    turns: u32,
    last_move: String,
    confirm_reset: bool,
    error: &'a str,
}

fn name(player: i8) -> &'static str {
    if player > 0 {
        "Charcoal"
    } else {
        "Ivory"
    }
}
fn coordinate(index: usize) -> String {
    format!("{}{}", (b'a' + (index % 8) as u8) as char, 8 - index / 8)
}

pub fn board(
    saved: &SavedGame,
    selected: Option<usize>,
    flipped: bool,
    confirm_reset: bool,
    error: Option<Error>,
) -> Response {
    let game = &saved.game;
    let legal = game.legal_moves();
    let selected = game
        .forced
        .or(selected)
        .filter(|at| legal.iter().any(|m| m.from == *at));
    let flip = u8::from(flipped);
    let squares = (0..64)
        .map(|display| {
            let index = if flipped { 63 - display } else { display };
            let piece = game.board[index];
            let is_selected = selected == Some(index);
            let selectable = legal.iter().any(|m| m.from == index);
            let destination = legal
                .iter()
                .any(|m| Some(m.from) == selected && m.to == index);
            let mut classes = "square".to_string();
            if (index / 8 + index % 8) % 2 == 1 {
                classes.push_str(" dark");
            }
            if is_selected {
                classes.push_str(" selected");
            }
            if selectable {
                classes.push_str(" playable");
            }
            if destination {
                classes.push_str(" destination");
            }
            if game
                .last_move
                .is_some_and(|m| m.from == index || m.to == index)
            {
                classes.push_str(" last");
            }
            let description = if piece == 0 {
                "empty".into()
            } else {
                format!(
                    "{} {}",
                    name(piece),
                    if piece.abs() == 2 { "king" } else { "piece" }
                )
            };
            Square {
                index,
                classes,
                piece_class: if piece > 0 { "charcoal" } else { "ivory" },
                occupied: piece != 0,
                king: piece.abs() == 2,
                selected: is_selected,
                selectable,
                destination,
                label: format!(
                    "{}, {}{}",
                    coordinate(index),
                    description,
                    if destination {
                        ", legal destination"
                    } else if selectable {
                        ", can move"
                    } else {
                        ""
                    }
                ),
                href: if is_selected {
                    format!("{BASE}/?flipped={flip}")
                } else {
                    format!(
                        "{BASE}/?selected={index}&revision={}&flipped={flip}",
                        saved.revision
                    )
                },
            }
        })
        .collect();
    let players = [1, -1]
        .into_iter()
        .map(|side| {
            let count = game.board.iter().filter(|p| p.signum() == side).count();
            Player {
                name: name(side),
                class: if side > 0 { "charcoal" } else { "ivory" },
                count,
                kings: game.board.iter().filter(|&&p| p == side * 2).count(),
                captured: 12 - count,
                active: game.winner.is_none() && game.turn == side,
            }
        })
        .collect();
    let last_move = game
        .last_move
        .map(|m| {
            format!(
                "{} {} {}",
                coordinate(m.from),
                if m.capture.is_some() { "×" } else { "–" },
                coordinate(m.to)
            )
        })
        .unwrap_or_else(|| "—".into());
    let view = Board {
        base: BASE,
        token: &saved.id,
        revision: saved.revision,
        squares,
        players,
        files: if flipped { "hgfedcba" } else { "abcdefgh" }
            .chars()
            .collect(),
        ranks: if flipped { "12345678" } else { "87654321" }
            .chars()
            .collect(),
        flipped: flip,
        flip_href: format!(
            "{BASE}/?flipped={}&revision={}{}",
            1 - flip,
            saved.revision,
            selected
                .map(|s| format!("&selected={s}"))
                .unwrap_or_default()
        ),
        select_from: selected.unwrap_or(0),
        has_selection: selected.is_some(),
        forced: game.forced.is_some(),
        capture_available: legal.iter().any(|m| m.capture.is_some()),
        finished: game.winner.is_some(),
        player: name(game.winner.unwrap_or(game.turn)),
        turns: game.moves,
        last_move,
        confirm_reset,
        error: error.as_ref().map(|e| e.message).unwrap_or(""),
    };
    match view.render() {
        Ok(body) => controllers::html(error.as_ref().map(|e| e.status).unwrap_or(200), body),
        Err(_) => self::error(Error::new(500, "The board could not be rendered.")),
    }
}

#[derive(Template)]
#[template(path = "error.html")]
struct ErrorPage<'a> {
    base: &'static str,
    message: &'a str,
}

pub fn error(error: Error) -> Response {
    let body = ErrorPage {
        base: BASE,
        message: error.message,
    }
    .render()
    .unwrap_or_else(|_| {
        "<!doctype html><title>Draughts</title><p>The game is temporarily unavailable.</p>".into()
    });
    controllers::html(error.status, body)
}
