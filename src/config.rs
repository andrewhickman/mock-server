use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use anyhow::{bail, Result};
use fn_error_context::context;
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
    config.validate()?;
    Ok(config)
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub routes: Vec<Route>,
}

#[derive(Debug, Deserialize)]
pub struct Route {
    pub route: route::Route,
    #[serde(flatten)]
    pub kind: RouteKind,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RouteKind {
    Dir(DirRoute),
    File(FileRoute),
}

#[derive(Debug, Deserialize)]
pub struct DirRoute {
    pub path: PathBuf,
}

#[derive(Debug, Deserialize)]
pub struct FileRoute {
    pub path: PathBuf,
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
