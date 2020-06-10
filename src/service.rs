use std::convert::Infallible;

use hyper::service::{service_fn, Service};
use hyper::Body;

use crate::config::Config;

impl Config {
    pub fn into_service(
        self,
    ) -> impl Service<
        http::Request<Body>,
        Response = http::Response<Body>,
        Error = Infallible,
        Future = impl Send,
    > + Clone {
        service_fn(|_req: http::Request<Body>| async move {
            Ok(http::Response::new(Body::from("Hello World")))
        })
    }
}
