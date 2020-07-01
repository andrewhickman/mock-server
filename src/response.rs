use headers::{ContentType, HeaderMapExt};
use hyper::Body;
use serde::Serialize;

pub fn from_status(status: http::StatusCode) -> http::Response<Body> {
    http::Response::builder()
        .status(status)
        .body(Body::empty())
        .unwrap()
}

pub fn json<T: Serialize>(value: &T) -> http::Response<Body> {
    let body = serde_json::to_string(value).expect("writing value to string should not fail");
    let mut response = http::Response::new(body.into());
    response.headers_mut().typed_insert(ContentType::json());
    response
}
