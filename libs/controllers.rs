//! HTTP controllers: cookies, input validation, model calls, and HTML responses.
use super::{
    model::{self, Error},
    views,
};
use crate::{Request, Response};
use serde::Deserialize;

pub const BASE: &str = "/games/draughts";
const COOKIE: &str = "draughts_session";

pub fn html(status: u16, body: String) -> Response {
    Response { status, headers: vec![
        ("content-type".into(), "text/html; charset=utf-8".into()),
        ("cache-control".into(), "no-store".into()),
        ("content-security-policy".into(), "default-src 'none'; style-src 'unsafe-inline'; form-action 'self'; base-uri 'none'; frame-ancestors 'none'".into()),
    ], body: body.into_bytes() }
}

fn parameter<'a>(request: &'a Request, key: &str) -> Option<&'a str> {
    request
        .query
        .iter()
        .find(|(name, _)| name == key)
        .and_then(|(_, values)| values.first())
        .map(String::as_str)
}

fn cookie(request: &Request) -> Option<String> {
    request
        .headers
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("cookie"))
        .flat_map(|(_, value)| value.split(';'))
        .filter_map(|part| part.trim().split_once('='))
        .find(|(name, value)| *name == COOKIE && model::valid_id(value))
        .map(|(_, value)| value.to_ascii_lowercase())
}

pub fn show(request: Request) -> Response {
    let result = (|| {
        let existing = cookie(&request);
        // Old local-storage IDs can be resumed explicitly without client code.
        let resume = parameter(&request, "resume");
        let saved = if let Some(id) = resume {
            if !model::valid_id(id) {
                return Err(Error::new(400, "Invalid saved-game identifier."));
            }
            model::load(&id.to_ascii_lowercase())?
        } else if let Some(ref id) = existing {
            model::load(id)?
        } else {
            model::create()?
        };
        let flipped = parameter(&request, "flipped") == Some("1");
        let requested_revision =
            parameter(&request, "revision").and_then(|v| v.parse::<i64>().ok());
        let selected = if requested_revision == Some(saved.revision) {
            parameter(&request, "selected").and_then(|v| v.parse::<usize>().ok())
        } else {
            None
        };
        let confirm = parameter(&request, "confirm") == Some("1");
        let mut response = views::board(&saved, selected, flipped, confirm, None);
        if existing.as_deref() != Some(saved.id.as_str()) {
            response.headers.push((
                "set-cookie".into(),
                format!(
                    "{COOKIE}={}; Path={BASE}; HttpOnly; SameSite=Strict; Max-Age=31536000",
                    saved.id
                ),
            ));
        }
        Ok(response)
    })();
    result.unwrap_or_else(views::error)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Form {
    action: String,
    token: String,
    revision: i64,
    from: Option<usize>,
    to: Option<usize>,
    flipped: Option<u8>,
}

pub fn play(request: Request) -> Response {
    let Some(id) = cookie(&request) else {
        return views::error(Error::new(
            403,
            "Open the game first and allow cookies to play.",
        ));
    };
    let saved = match model::load(&id) {
        Ok(saved) => saved,
        Err(error) => return views::error(error),
    };
    let content_type = request
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))
        .map(|(_, value)| value.split(';').next().unwrap_or("").trim())
        .unwrap_or("");
    if content_type != "application/x-www-form-urlencoded" {
        return views::board(
            &saved,
            None,
            false,
            false,
            Some(Error::new(415, "Submit moves using the board form.")),
        );
    }
    let form: Form = match serde_urlencoded::from_bytes(&request.body) {
        Ok(form) => form,
        Err(_) => {
            return views::board(
                &saved,
                None,
                false,
                false,
                Some(Error::new(
                    400,
                    "The submitted move was not valid. Please try again.",
                )),
            )
        }
    };
    let flipped = form.flipped == Some(1);
    let result = if form.token != id {
        Err(Error::new(
            403,
            "This form belongs to a different game. Please use the current board.",
        ))
    } else if form.revision != saved.revision {
        Err(Error::conflict())
    } else {
        match (form.action.as_str(), form.from, form.to) {
            ("move", Some(from), Some(to)) => model::play(&saved, from, to),
            ("reset", None, None) => model::reset(&saved),
            _ => Err(Error::new(400, "Choose a move or start a new game.")),
        }
    };
    match result {
        Ok(()) => Response {
            status: 303,
            headers: vec![
                (
                    "location".into(),
                    format!("{BASE}/?flipped={}", u8::from(flipped)),
                ),
                ("cache-control".into(), "no-store".into()),
            ],
            body: Vec::new(),
        },
        Err(error) => {
            // In a racing-tab conflict the first snapshot can already be stale.
            match model::load(&id) {
                Ok(latest) => views::board(&latest, None, flipped, false, Some(error)),
                Err(load_error) => views::error(load_error),
            }
        }
    }
}
