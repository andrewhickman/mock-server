use structopt::StructOpt;

mod config;
mod server;
mod service;
mod tls;

const ABOUT: &str = "A simple proxy server.";
const VERSION: &str = concat!(
    structopt::clap::crate_version!(),
    " (",
    env!("VERGEN_SHA"),
    ")"
);

#[derive(Debug, StructOpt)]
#[structopt(about = ABOUT, version = VERSION)]
#[structopt(setting = structopt::clap::AppSettings::UnifiedHelpMessage)]
pub struct Options {
    #[structopt(flatten)]
    config: config::Options,
    #[structopt(flatten)]
    server: server::Options,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().filter_or("PROXY_SERVER_LOG", "info"));
    let options = Options::from_args();
    log::debug!("{:#?}", options);

    let config = config::parse(&options.config)?;

    server::run(&options.server, config.into_service()).await
}
