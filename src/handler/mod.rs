mod fs;
mod proxy;

use hyper::Body;
use regex::Regex;

use self::fs::{DirHandler, FileHandler};
use self::proxy::ProxyHandler;
use crate::config;

#[derive(Debug)]
pub struct Handler {
    kind: HandlerKind,
    path_rewriter: Option<PathRewriter>,
}

#[derive(Debug)]
pub enum HandlerKind {
    File(FileHandler),
    Dir(DirHandler),
    Proxy(ProxyHandler),
}

#[derive(Debug)]
pub struct PathRewriter {
    regex: Regex,
    replace: String,
}

impl Handler {
    pub fn new(route: config::Route) -> Self {
        let config::Route {
            rewrite_path,
            route,
            kind,
        } = route;
        let path_rewriter = rewrite_path.map(|replace| {
            let regex = route.to_regex();
            PathRewriter { regex, replace }
        });

        let kind = match kind {
            config::RouteKind::File(file) => HandlerKind::File(FileHandler::new(file)),
            config::RouteKind::Dir(dir) => HandlerKind::Dir(DirHandler::new(dir)),
            config::RouteKind::Proxy(proxy) => HandlerKind::Proxy(ProxyHandler::new(proxy)),
        };

        Handler {
            path_rewriter,
            kind,
        }
    }

    pub async fn handle(
        &self,
        request: http::Request<Body>,
    ) -> Result<http::Response<Body>, (http::Request<Body>, http::Response<Body>)> {
        let path = match &self.path_rewriter {
            Some(path_rewriter) => path_rewriter.rewrite(request.uri().path()),
            None => request.uri().path().to_owned(),
        };

        match &self.kind {
            HandlerKind::File(file) => file.handle(request).await,
            HandlerKind::Dir(dir) => dir.handle(request, &path).await,
            HandlerKind::Proxy(proxy) => proxy.handle(request, &path).await,
        }
    }
}

impl PathRewriter {
    fn rewrite<'a>(&self, path: &'a str) -> String {
        self.regex.replace(path, self.replace.as_str()).into_owned()
    }
}
