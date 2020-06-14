mod fs;

use hyper::Body;

use self::fs::FileHandler;
use crate::config;

#[derive(Debug)]
pub enum Handler {
    File(FileHandler),
}

impl Handler {
    pub fn new(route_kind: config::RouteKind) -> Self {
        match route_kind {
            config::RouteKind::File(file) => Handler::File(FileHandler::new(file)),
            _ => todo!(),
        }
    }

    pub async fn handle(
        &self,
        request: http::Request<Body>,
    ) -> Result<http::Response<Body>, (http::Request<Body>, http::Response<Body>)> {
        match self {
            Handler::File(file) => file.handle(request).await,
        }
    }
}
