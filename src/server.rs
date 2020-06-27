use std::convert::Infallible;
use std::fs::File;
use std::future::Future;
use std::io::{BufReader, Seek, SeekFrom};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use anyhow::{format_err, Context, Result};
use fn_error_context::context;
use futures::{future, FutureExt, TryFutureExt};
use hyper::server::{conn::AddrIncoming, conn::AddrStream, Server};
use hyper::service::{make_service_fn, Service};
use hyper::Body;
use rustls::internal::pemfile;
use structopt::StructOpt;

use crate::tls::{TlsAcceptor, TlsStream};

#[derive(Debug, StructOpt)]
pub struct Options {
    #[structopt(
        long,
        short = "n",
        value_name = "HOST",
        default_value = "localhost",
        help = "Host to listen on"
    )]
    host: String,
    #[structopt(
        long,
        short = "p",
        value_name = "PORT",
        help = "Port to listen on [default: an OS-assigned port]"
    )]
    port: Option<u16>,
    #[structopt(
        name = "tls-cert",
        long,
        value_name = "CERT_FILE",
        help = "Path to the certificate to use for TLS",
        requires = "tls-key",
        parse(from_os_str)
    )]
    tls_cert: Option<PathBuf>,
    #[structopt(
        name = "tls-key",
        long,
        value_name = "KEY_FILE",
        help = "Path to the private key to use for TLS",
        requires = "tls-cert",
        parse(from_os_str)
    )]
    tls_key: Option<PathBuf>,
}

pub async fn run<S>(options: &Options, service: S) -> Result<()>
where
    S: Service<http::Request<Body>, Response = http::Response<Body>, Error = Infallible>
        + Clone
        + Send
        + 'static,
    S::Future: Send,
{
    let incoming = AddrIncoming::bind(&options.resolve_addr().await?)?;
    let addr = incoming.local_addr();

    let result = if let Some(tls_config) = options.tls_config()? {
        let incoming = TlsAcceptor::new(incoming, tls_config);
        let server = Server::builder(incoming);
        log::info!("Listening on https://{}", addr);
        server
            .serve(make_service_fn(move |_: &TlsStream| {
                future::ready(service.clone()).never_error()
            }))
            .with_graceful_shutdown(ctrl_c())
            .await
    } else {
        let server = Server::builder(incoming);
        log::info!("Listening on http://{}", addr);
        server
            .serve(make_service_fn(move |_: &AddrStream| {
                future::ready(service.clone()).never_error()
            }))
            .with_graceful_shutdown(ctrl_c())
            .await
    };

    result.context("server execution failed")
}

impl Options {
    async fn resolve_addr(&self) -> Result<SocketAddr> {
        let error_message = || format!("failed to resolve host `{}`", self.host);
        Ok(
            tokio::net::lookup_host((self.host.as_ref(), self.port.unwrap_or(0)))
                .await
                .with_context(error_message)?
                .next()
                .with_context(error_message)?,
        )
    }

    fn tls_config(&self) -> Result<Option<rustls::ServerConfig>> {
        if let (Some(cert_path), Some(key_path)) = (&self.tls_cert, &self.tls_key) {
            let certs = self.tls_certs(cert_path)?;
            let key = self.tls_key(key_path)?;
            let mut config = rustls::ServerConfig::new(rustls::NoClientAuth::new());
            config.set_single_cert(certs, key)?;
            Ok(Some(config))
        } else {
            Ok(None)
        }
    }

    #[context("failed to load TLS certificates from `{}`", path.display())]
    fn tls_certs(&self, path: &Path) -> Result<Vec<rustls::Certificate>> {
        let mut reader = BufReader::new(File::open(path)?);
        pemfile::certs(&mut reader).map_err(|()| format_err!("invalid certificate"))
    }

    #[context("failed to load TLS key from `{}`", path.display())]
    fn tls_key(&self, path: &Path) -> Result<rustls::PrivateKey> {
        let mut reader = BufReader::new(File::open(path)?);

        let pkcs8_keys = pemfile::pkcs8_private_keys(&mut reader).map_err(|()| {
            format_err!(
                "file contains invalid pkcs8 private key (encrypted keys are not supported)"
            )
        })?;
        if let Some(key) = pkcs8_keys.into_iter().next() {
            return Ok(key);
        }

        reader.seek(SeekFrom::Start(0))?;

        let rsa_keys = pemfile::rsa_private_keys(&mut reader)
            .map_err(|()| format_err!("file contains invalid rsa private key"))?;
        if let Some(key) = rsa_keys.into_iter().next() {
            return Ok(key);
        }

        Err(format_err!("no pkcs8 or rsa private keys found"))
    }
}

fn ctrl_c() -> impl Future<Output = ()> {
    tokio::signal::ctrl_c()
        .or_else(|err| {
            log::warn!("Error listening for SIGINT: {:#?}", err);
            future::pending::<Result<(), Infallible>>()
        })
        .map(|_| {
            log::info!("Received SIGINT, shutting down server");
        })
}
