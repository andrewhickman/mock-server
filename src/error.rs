use hyper::Body;

pub fn not_found() -> http::Response<Body> {
    http::Response::builder()
        .status(http::StatusCode::NOT_FOUND)
        .body(Body::empty())
        .unwrap()
}

pub fn internal_server_error() -> http::Response<Body> {
    http::Response::builder()
        .status(http::StatusCode::INTERNAL_SERVER_ERROR)
        .body(Body::empty())
        .unwrap()
}
