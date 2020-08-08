mod fs;
mod json;
mod mock;
mod proxy;

use std::fmt;

use anyhow::Result;
use hyper::Body;

use self::fs::{DirHandler, FileHandler};
use self::json::JsonHandler;
use self::mock::MockHandler;
use self::proxy::ProxyHandler;
use crate::method::MethodFilter;
use crate::path::PathRewriter;
use crate::{config, response};

pub struct Handler {
    kind: HandlerKind,
    path_rewriter: Option<PathRewriter>,
    response_headers: http::HeaderMap,
    method_filter: Box<dyn MethodFilter>,
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
            methods,
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

        let method_filter = match methods {
            Some(methods) => Box::new(methods),
            None => kind.default_method_filter(),
        };

        Ok(Handler {
            path_rewriter,
            kind,
            response_headers,
            method_filter,
        })
    }

    pub async fn handle(
        &self,
        request: http::Request<Body>,
    ) -> Result<http::Response<Body>, (http::Request<Body>, http::Response<Body>)> {
        if !self.method_filter.is_match(request.method()) {
            return Err((
                request,
                response::from_status(http::StatusCode::METHOD_NOT_ALLOWED),
            ));
        }

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

impl HandlerKind {
    fn default_method_filter(&self) -> Box<dyn MethodFilter> {
        match self {
            HandlerKind::File(_) | HandlerKind::Dir(_) => fs::default_method_filter(),
            HandlerKind::Proxy(_) => proxy::default_method_filter(),
            HandlerKind::Json(_) => json::default_method_filter(),
            HandlerKind::Mock(_) => mock::default_method_filter(),
        }
    }
}

impl fmt::Debug for Handler {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Handler")
            .field("kind", &self.kind)
            .field("path_rewriter", &self.path_rewriter)
            .field("response_headers", &self.response_headers)
            .finish()
    }
}
