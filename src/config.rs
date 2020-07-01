use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use anyhow::{bail, Result};
use fn_error_context::context;
use http::uri::Uri;
use serde::Deserialize;
use structopt::StructOpt;

use crate::route;

#[derive(Debug, StructOpt)]
pub struct Options {
    #[structopt(
        value_name = "CONFIG_FILE",
        help = "Path to the config file",
        parse(from_os_str)
    )]
    config: PathBuf,
}

#[context("failed to parse config from `{}`", options.config.display())]
pub fn parse(options: &Options) -> Result<Config> {
    let reader = BufReader::new(File::open(&options.config)?);
    let config: Config = serde_yaml::from_reader(reader)?;
    log::debug!("{:#?}", config);
    config.validate()?;
    Ok(config)
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub routes: Vec<Route>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Route {
    pub route: route::Route,
    pub rewrite_path: Option<String>,
    #[serde(with = "http_serde::header_map", default)]
    pub response_headers: http::HeaderMap,
    #[serde(flatten)]
    pub kind: RouteKind,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RouteKind {
    Dir(DirRoute),
    File(FileRoute),
    Proxy(ProxyRoute),
    Json(JsonRoute),
}

#[derive(Debug, Deserialize)]
pub struct DirRoute {
    pub path: PathBuf,
}

#[derive(Debug, Deserialize)]
pub struct FileRoute {
    pub path: PathBuf,
}

#[derive(Debug, Deserialize)]
pub struct JsonRoute {
    pub path: PathBuf,
    #[serde(default)]
    pub pretty: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ProxyRoute {
    #[serde(rename = "url", with = "http_serde::uri")]
    pub uri: Uri,
}

impl Config {
    fn validate(&self) -> Result<()> {
        for route in &self.routes {
            route.validate()?;
        }
        Ok(())
    }
}

impl Route {
    #[context("error in route `{}`", self.route)]
    fn validate(&self) -> Result<()> {
        match &self.kind {
            RouteKind::Dir(dir) => dir.validate(),
            RouteKind::File(file) => file.validate(),
            RouteKind::Proxy(proxy) => proxy.validate(),
            RouteKind::Json(json) => json.validate(),
        }
    }
}

impl DirRoute {
    fn validate(&self) -> Result<()> {
        if !self.path.is_dir() {
            bail!("`{}` is not a directory", self.path.display());
        }
        Ok(())
    }
}

impl FileRoute {
    fn validate(&self) -> Result<()> {
        if !self.path.is_file() {
            bail!("`{}` is not a file", self.path.display());
        }
        Ok(())
    }
}

impl ProxyRoute {
    fn validate(&self) -> Result<()> {
        if self.uri.scheme().is_none() {
            bail!("url must include scheme");
        }
        if self.uri.authority().is_none() {
            bail!("url must include authority");
        }
        Ok(())
    }
}

impl JsonRoute {
    fn validate(&self) -> Result<()> {
        if !self.path.is_file() {
            bail!("`{}` is not a file", self.path.display());
        }
        Ok(())
    }
}
