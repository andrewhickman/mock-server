use std::path::{self, Path};

use headers::{ContentLength, ContentType, HeaderMapExt};
use hyper::Body;
use tokio::fs;
use tokio::io::ErrorKind;
use urlencoding::decode;

use crate::{config, error};

#[derive(Debug)]
pub struct FileHandler {
    config: config::FileRoute,
}

#[derive(Debug)]
pub struct DirHandler {
    config: config::DirRoute,
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

        log::debug!(
            "Path `{}` matched file `{}`",
            request.uri().path(),
            self.config.path.display()
        );

        Ok(file_response(&self.config.path).await)
    }
}

impl DirHandler {
    pub fn new(config: config::DirRoute) -> Self {
        DirHandler { config }
    }

    pub async fn handle(
        &self,
        request: http::Request<Body>,
        path: &str,
    ) -> Result<http::Response<Body>, (http::Request<Body>, http::Response<Body>)> {
        if request.method() != http::Method::GET {
            return Err((
                request,
                error::from_status(http::StatusCode::METHOD_NOT_ALLOWED),
            ));
        }

        log::debug!(
            "Path `{}` matched directory `{}`",
            request.uri().path(),
            self.config.path.display()
        );

        let path = match decode(path) {
            Ok(path) => path,
            Err(err) => {
                log::info!("Invalid path `{}`: {}", path, err);
                return Ok(error::from_status(http::StatusCode::BAD_REQUEST));
            }
        };

        match sanitize_path(&path) {
            Some(components) => {
                let mut path = self.config.path.clone();
                path.extend(components);
                Ok(file_response(&path).await)
            }
            None => Ok(error::from_status(http::StatusCode::NOT_FOUND)),
        }
    }
}

fn sanitize_path<'a>(path: &'a str) -> Option<Vec<path::Component<'a>>> {
    let mut result = Vec::new();

    for component in Path::new(path).components() {
        match component {
            path::Component::Prefix(_) => return None,
            path::Component::ParentDir => {
                if result.pop().is_none() {
                    return None;
                }
            }
            path::Component::RootDir => (),
            path::Component::CurDir => (),
            path::Component::Normal(_) => result.push(component),
        }
    }

    Some(result)
}

async fn file_response(path: &Path) -> http::Response<Body> {
    log::debug!("Returning file from `{}`", path.display());

    let file = match fs::read(path).await {
        Ok(file) => file,
        Err(err) if err.kind() == ErrorKind::NotFound => {
            log::info!("File not found: `{}`", path.display());
            return error::from_status(http::StatusCode::NOT_FOUND);
        }
        #[cfg(windows)]
        Err(err) if err.raw_os_error() == Some(123) => {
            log::info!("Invalid file name: `{}`", path.display());
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
