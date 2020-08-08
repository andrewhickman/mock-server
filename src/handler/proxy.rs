use std::sync::Arc;

use http::uri::{PathAndQuery, Uri};
use hyper::client::{Client, HttpConnector};
use hyper::Body;
use hyper_rustls::HttpsConnector;
use once_cell::sync::Lazy;

use crate::method::{self, MethodFilter};
use crate::{config, response};

#[derive(Debug)]
pub struct ProxyHandler {
    config: config::ProxyRoute,
    client: Arc<Client<HttpsConnector<HttpConnector>>>,
}

pub fn default_method_filter() -> Box<dyn MethodFilter> {
    method::any()
}

impl ProxyHandler {
    pub fn new(config: config::ProxyRoute) -> Self {
        static CLIENT: Lazy<Arc<Client<HttpsConnector<HttpConnector>>>> =
            Lazy::new(|| Arc::new(Client::builder().build(HttpsConnector::new())));

        ProxyHandler {
            config,
            client: CLIENT.clone(),
        }
    }

    pub async fn handle(
        &self,
        mut request: http::Request<Body>,
        path: &str,
    ) -> Result<http::Response<Body>, (http::Request<Body>, http::Response<Body>)> {
        let uri = match self.get_uri(path) {
            Ok(uri) => uri,
            Err(err) => {
                log::info!("path `{}` produced invalid uri: {}", path, err);
                return Ok(response::from_status(http::StatusCode::NOT_FOUND));
            }
        };

        log::debug!(
            "Path `{}` matched proxy `{}`",
            request.uri().path(),
            self.config.uri
        );
        request.headers_mut().insert(
            http::header::HOST,
            http::HeaderValue::from_str(uri.authority().unwrap().as_str())
                .expect("authority is valid header value"),
        );
        *request.uri_mut() = uri;
        log::debug!("Forwarding request to `{}`", request.uri());

        match self.client.request(request).await {
            Ok(response) => Ok(response),
            Err(err) => {
                log::error!("Error making request: {}", err);
                Ok(response::from_status(
                    http::StatusCode::INTERNAL_SERVER_ERROR,
                ))
            }
        }
    }

    fn get_uri(&self, path: &str) -> http::Result<Uri> {
        let path_and_query = append_path(self.config.uri.path_and_query(), path)?;
        let uri = Uri::builder()
            .scheme(self.config.uri.scheme().unwrap().as_str())
            .authority(self.config.uri.authority().unwrap().as_str())
            .path_and_query(path_and_query)
            .build()
            .unwrap();
        Ok(uri)
    }
}

fn append_path(
    base: Option<&PathAndQuery>,
    path: &str,
) -> Result<PathAndQuery, http::uri::InvalidUri> {
    match base {
        Some(base) => {
            let mut result = String::with_capacity(base.as_str().len() + 1 + path.len());
            result.push_str(base.path());
            if result.ends_with('/') && path.starts_with('/') {
                result.pop();
            } else if !result.ends_with('/') && !path.starts_with('/') {
                result.push('/');
            }
            result.push_str(path);
            if let Some(query) = base.query() {
                result.push('?');
                result.push_str(query);
            }
            result.parse()
        }
        None => path.parse(),
    }
}
