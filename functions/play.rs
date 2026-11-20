use crate::{libs, Request, Response};

pub fn handle(request: Request) -> Response {
    libs::controllers::play(request)
}
