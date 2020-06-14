use std::path::Path;

use headers::{ContentLength, ContentType, HeaderMapExt};
use hyper::Body;
use tokio::fs;
use tokio::io::ErrorKind;

use crate::config;
use crate::error;

#[derive(Debug)]
pub struct FileHandler {
    config: config::FileRoute,
}

impl FileHandler {
    pub fn new(config: config::FileRoute) -> Self {
        FileHandler { config }
    }

    pub async fn handle(
        &self,
        request: http::Request<Body>,
    ) -> Result<http::Response<Body>, (http::Request<Body>, http::Response<Body>)> {
        if request.method() != http::Method::GET {
            return Err((
                request,
                error::from_status(http::StatusCode::METHOD_NOT_ALLOWED),
            ));
        }

        Ok(file_response(&self.config.path).await)
    }
}

async fn file_response(path: &Path) -> http::Response<Body> {
    let file = match fs::read(path).await {
        Ok(file) => file,
        Err(err) if err.kind() == ErrorKind::NotFound => {
            return error::from_status(http::StatusCode::NOT_FOUND);
        }
        #[cfg(windows)]
        Err(err) if err.raw_os_error() == Some(123) => {
            return error::from_status(http::StatusCode::NOT_FOUND);
        }
        Err(err) => {
            log::error!("Error opening file: {}", err);
            return error::from_status(http::StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let len = file.len();
    let mime = mime_guess::from_path(path).first_or_octet_stream();

    let mut response = http::Response::new(Body::from(file));
    response
        .headers_mut()
        .typed_insert(ContentLength(len as u64));
    response.headers_mut().typed_insert(ContentType::from(mime));
    response
}
