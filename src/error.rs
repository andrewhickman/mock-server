use hyper::Body;

pub fn from_status(status: http::StatusCode) -> http::Response<Body> {
    http::Response::builder()
        .status(status)
        .body(Body::empty())
        .unwrap()
}
