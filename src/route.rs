use std::any::Any;
use std::borrow::Cow;
use std::convert::Infallible;
use std::fmt::{self, Display};
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;

use futures::future::{FutureExt, TryFutureExt};
use hyper::service::{service_fn, Service};
use hyper::Body;
use once_cell::sync::Lazy;
use regex::{Regex, RegexSet};
use serde::de::{self, Deserialize, Deserializer};

use crate::config::Config;
use crate::error;
use crate::handler::Handler;

#[derive(Debug)]
pub struct Router {
    regex_set: RegexSet,
    handlers: Vec<Handler>,
}

#[derive(Debug)]
pub struct Route {
    precedence: Precedence,
    path: String,
    regex: String,
}

#[derive(Copy, Clone, Debug, Default, PartialOrd, Ord, Eq, PartialEq)]
struct Precedence {
    multi_wildcards: u32,
    wildcards: u32,
}

impl Router {
    pub fn new(mut config: Config) -> Self {
        config.routes.sort_by_key(|route| route.route.precedence);
        let regex_set = RegexSet::new(config.routes.iter().map(|route| &route.route.regex))
            .expect("error in generated regex");
        let handlers = config
            .routes
            .into_iter()
            .map(|route| Handler::new(route))
            .collect();

        Router {
            regex_set,
            handlers,
        }
    }
}

impl Router {
    pub async fn try_handle(
        self: Arc<Self>,
        mut request: http::Request<Body>,
    ) -> http::Response<Body> {
        let mut response = error::from_status(http::StatusCode::NOT_FOUND);

        let matches = self.regex_set.matches(request.uri().path());
        if matches.matched_any() {
            for match_index in matches {
                match self.handlers[match_index].handle(request).await {
                    Ok(response) => return response,
                    Err(parts) => {
                        request = parts.0;
                        response = parts.1;
                    }
                }
            }
        } else {
            log::info!("Path `{}` did not match any route", request.uri().path());
        }

        response
    }

    pub fn handle(
        self: Arc<Self>,
        request: http::Request<Body>,
    ) -> impl Future<Output = http::Response<Body>> {
        AssertUnwindSafe(self.try_handle(request))
            .catch_unwind()
            .unwrap_or_else(|payload| {
                log::error!(
                    "Panic while handling request: {}",
                    fmt_panic_payload(payload)
                );
                error::from_status(http::StatusCode::INTERNAL_SERVER_ERROR)
            })
    }

    pub fn into_service(
        self,
    ) -> impl Service<
        http::Request<Body>,
        Response = http::Response<Body>,
        Error = Infallible,
        Future = impl Send,
    > + Clone {
        let this = Arc::new(self);
        service_fn(move |request: http::Request<Body>| this.clone().handle(request).never_error())
    }
}

impl Route {
    pub fn new(path: String) -> Result<Self, impl Display> {
        macro_rules! chars {
            () => {
                r"[\w\-\.~%!$&'()*+,;=:@]*"
            };
        }

        const PATH_SEGMENT_PATTERN: &str = concat!(r"/(", chars!(), ")");
        const MULTI_PATH_SEGMENT_PATTERN: &str = concat!(r"/(", chars!(), "(?:/", chars!(), ")*)");

        static PATH_CHARS_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(chars!()).unwrap());

        let mut regex = String::with_capacity(path.len() + 5);
        let mut precedence = Precedence::default();

        regex.push('^');
        for segment in path.split('/') {
            if segment.is_empty() {
                continue;
            }

            if !PATH_CHARS_REGEX.is_match(segment) {
                return Err("invalid character in path");
            }

            match segment {
                "*" => {
                    precedence.wildcards += 1;
                    regex.push_str(PATH_SEGMENT_PATTERN);
                }
                "**" => {
                    precedence.multi_wildcards += 1;
                    regex.push_str(MULTI_PATH_SEGMENT_PATTERN);
                }
                _ => {
                    regex.push('/');
                    regex_syntax::escape_into(segment, &mut regex);
                }
            }
        }
        regex.push_str(r"/?$");

        Ok(Route {
            precedence,
            regex,
            path,
        })
    }

    pub fn to_regex(&self) -> Regex {
        Regex::new(&self.regex).expect("error in generated regex")
    }
}

impl Display for Route {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.path.fmt(f)
    }
}

impl<'de> Deserialize<'de> for Route {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let path = String::deserialize(deserializer)?;
        Route::new(path).map_err(de::Error::custom)
    }
}

fn fmt_panic_payload(payload: Box<dyn Any + Send + 'static>) -> impl Display {
    if let Some(string) = payload.downcast_ref::<&'static str>() {
        Cow::Borrowed(*string)
    } else if let Ok(string) = payload.downcast::<String>() {
        Cow::Owned(*string)
    } else {
        Cow::Borrowed("Box<Any>")
    }
}

#[test]
fn test_precedence() {
    assert!(
        Precedence {
            multi_wildcards: 0,
            wildcards: 0,
        } < Precedence {
            multi_wildcards: 0,
            wildcards: 1,
        }
    );

    assert!(
        Precedence {
            multi_wildcards: 0,
            wildcards: 1,
        } < Precedence {
            multi_wildcards: 1,
            wildcards: 0,
        }
    );

    assert!(
        Precedence {
            multi_wildcards: 1,
            wildcards: 0,
        } < Precedence {
            multi_wildcards: 1,
            wildcards: 1,
        }
    );
}
