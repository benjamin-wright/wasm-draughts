use crate::{Request, Response};

pub fn handle(request: Request) -> Response {
    crate::libs::controllers::show(request)
}
