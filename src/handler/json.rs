use std::io::SeekFrom;
use std::sync::Arc;

use anyhow::{Context, Result};
use bytes::buf::BufExt;
use headers::{ContentType, HeaderMapExt};
use hyper::body::{self, Body};
use json_patch::{Patch, PatchError};
use mime::Mime;
use serde::de::DeserializeOwned;
use tokio::fs::{self, File};
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::sync::{Notify, RwLock};
use urlencoding::decode;

use crate::method::MethodFilter;
use crate::{config, response};

#[derive(Debug)]
pub struct JsonHandler {
    state: Arc<State>,
}

#[derive(Debug)]
struct State {
    value: RwLock<serde_json::Value>,
    dirty: Notify,
}

#[derive(Debug)]
struct Sync {
    state: Arc<State>,
    config: config::JsonRoute,
    file: File,
    buf: Vec<u8>,
}

pub fn default_method_filter() -> Box<dyn MethodFilter> {
    Box::new(|method: &http::Method| method == http::Method::GET || method == http::Method::PATCH)
}

impl JsonHandler {
    pub async fn new(config: config::JsonRoute) -> Result<Self> {
        let mut file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&config.path)
            .await
            .with_context(|| format!("failed to open file `{}`", config.path.display()))?;

        let mut buf = Vec::new();
        file.read_to_end(&mut buf)
            .await
            .with_context(|| format!("failed to read from file `{}`", config.path.display()))?;
        let value = serde_json::from_slice(&buf).with_context(|| {
            format!("failed to read JSON from file `{}`", config.path.display())
        })?;
        buf.clear();

        let state = Arc::new(State {
            value: RwLock::new(value),
            dirty: Notify::new(),
        });

        let sync = Sync {
            state: state.clone(),
            config,
            file,
            buf,
        };
        tokio::spawn(sync.run());

        Ok(JsonHandler { state })
    }

    pub async fn handle(
        &self,
        request: http::Request<Body>,
        path: &str,
    ) -> Result<http::Response<Body>, (http::Request<Body>, http::Response<Body>)> {
        let path = match decode(path) {
            Ok(path) => path,
            Err(err) => {
                log::info!("Invalid path `{}`: {}", path, err);
                return Ok(response::from_status(http::StatusCode::BAD_REQUEST));
            }
        };

        match request.method() {
            &http::Method::GET => Ok(self.handle_get(request, &path).await),
            &http::Method::PATCH => Ok(self.handle_patch(request, &path).await),
            _ => Err((
                request,
                response::from_status(http::StatusCode::METHOD_NOT_ALLOWED),
            )),
        }
    }

    pub async fn handle_get(&self, _: http::Request<Body>, path: &str) -> http::Response<Body> {
        let value = self.state.value.read().await;
        match value.pointer(path) {
            Some(subvalue) => response::json(subvalue),
            None => {
                log::info!("Pointer `{}` did not match JSON", path);
                return response::from_status(http::StatusCode::NOT_FOUND);
            }
        }
    }

    pub async fn handle_patch(
        &self,
        request: http::Request<Body>,
        path: &str,
    ) -> http::Response<Body> {
        let patch = match json_request::<Patch>(request).await {
            Ok(patch) => patch,
            Err(response) => return response,
        };

        let response = {
            let mut value = self.state.value.write().await;
            let subvalue = match value.pointer_mut(path) {
                Some(subvalue) => subvalue,
                None => {
                    log::info!("Pointer `{}` did not match JSON", path);
                    return response::from_status(http::StatusCode::NOT_FOUND);
                }
            };

            if let Err(err) = json_patch::patch(subvalue, &patch) {
                log::info!("Failed to apply patch: {}", err);
                return match err {
                    PatchError::TestFailed => response::from_status(http::StatusCode::CONFLICT),
                    PatchError::InvalidPointer => {
                        response::from_status(http::StatusCode::NOT_FOUND)
                    }
                };
            }

            response::json(subvalue)
        };

        self.state.dirty.notify();
        response
    }
}

impl Sync {
    async fn run(mut self) {
        loop {
            self.state.dirty.notified().await;
            log::trace!("Syncing JSON data to file `{}`", self.config.path.display());

            if let Err(err) = self.write().await {
                log::error!(
                    "Failed to write JSON data to file `{}`: {}",
                    self.config.path.display(),
                    err
                );
            }
        }
    }

    async fn write(&mut self) -> io::Result<()> {
        self.fill_buf().await;

        self.file.seek(SeekFrom::Start(0)).await?;
        self.file.set_len(0).await?;
        self.file.write_all(&self.buf).await?;

        self.buf.clear();
        Ok(())
    }

    async fn fill_buf(&mut self) {
        let value = self.state.value.read().await;
        if self.config.pretty {
            serde_json::to_writer_pretty(&mut self.buf, &*value)
        } else {
            serde_json::to_writer(&mut self.buf, &*value)
        }
        .expect("writing value to a string should not fail");
    }
}

async fn json_request<T: DeserializeOwned>(
    request: http::Request<Body>,
) -> Result<T, http::Response<Body>> {
    match request.headers().typed_try_get::<ContentType>() {
        Err(err) => {
            log::info!("Error parsing content-type header: {}", err);
            return Err(response::from_status(http::StatusCode::BAD_REQUEST));
        }
        Ok(Some(content_type)) => {
            let mime: Mime = content_type.into();
            if mime.subtype() != mime::JSON && mime.suffix() != Some(mime::JSON) {
                return Err(response::from_status(
                    http::StatusCode::UNSUPPORTED_MEDIA_TYPE,
                ));
            }
        }
        Ok(None) => (),
    }

    let buf = match body::aggregate(request.into_body()).await {
        Ok(buf) => buf,
        Err(err) => {
            log::error!("Error reading request body: {}", err);
            return Err(response::from_status(
                http::StatusCode::INTERNAL_SERVER_ERROR,
            ));
        }
    };

    match serde_json::from_reader(buf.reader()) {
        Ok(value) => Ok(value),
        Err(err) => {
            log::info!("Error deserializing request body: {}", err);
            Err(response::from_status(http::StatusCode::BAD_REQUEST))
        }
    }
}
