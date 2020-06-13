use hyper::Body;

use crate::config;

#[derive(Debug)]
pub struct Handler {}

impl Handler {
    pub fn new(route_kind: config::RouteKind) -> Self {
        Handler {}
    }

    pub fn handle(&self, request: http::Request<Body>) -> Result<http::Response<Body>, http::Request<Body>> {
        Err(request)
    }
}
