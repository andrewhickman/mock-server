mod fs;
mod json;
mod mock;
mod proxy;

use anyhow::Result;
use hyper::Body;

use self::fs::{DirHandler, FileHandler};
use self::json::JsonHandler;
use self::mock::MockHandler;
use self::proxy::ProxyHandler;
use crate::config;
use crate::path::PathRewriter;

#[derive(Debug)]
pub struct Handler {
    kind: HandlerKind,
    path_rewriter: Option<PathRewriter>,
    response_headers: http::HeaderMap,
}

#[derive(Debug)]
pub enum HandlerKind {
    File(FileHandler),
    Dir(DirHandler),
    Proxy(ProxyHandler),
    Json(JsonHandler),
    Mock(MockHandler),
}

impl Handler {
    pub async fn new(route: config::Route) -> Result<Self> {
        let config::Route {
            rewrite_path,
            route,
            kind,
            response_headers,
        } = route;
        let path_rewriter = rewrite_path.map(|replace| {
            let regex = route.to_regex();
            PathRewriter::new(regex, replace)
        });

        let kind = match kind {
            config::RouteKind::File(file) => HandlerKind::File(FileHandler::new(file)),
            config::RouteKind::Dir(dir) => HandlerKind::Dir(DirHandler::new(dir)),
            config::RouteKind::Proxy(proxy) => HandlerKind::Proxy(ProxyHandler::new(proxy)),
            config::RouteKind::Json(json) => HandlerKind::Json(JsonHandler::new(json).await?),
            config::RouteKind::Mock(mock) => HandlerKind::Mock(MockHandler::new(mock)),
        };

        Ok(Handler {
            path_rewriter,
            kind,
            response_headers,
        })
    }

    pub async fn handle(
        &self,
        request: http::Request<Body>,
    ) -> Result<http::Response<Body>, (http::Request<Body>, http::Response<Body>)> {
        let path = match &self.path_rewriter {
            Some(path_rewriter) => path_rewriter.rewrite(request.uri().path()),
            None => request.uri().path().to_owned(),
        };

        let mut result = match &self.kind {
            HandlerKind::File(file) => file.handle(request).await,
            HandlerKind::Dir(dir) => dir.handle(request, &path).await,
            HandlerKind::Proxy(proxy) => proxy.handle(request, &path).await,
            HandlerKind::Json(json) => json.handle(request, &path).await,
            HandlerKind::Mock(mock) => mock.handle(request).await,
        };

        if let Ok(response) = &mut result {
            response.headers_mut().extend(self.response_headers.clone());
        }

        result
    }
}
