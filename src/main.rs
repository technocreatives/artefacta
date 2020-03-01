use anyhow::{Context, Result};
use std::path::PathBuf;
use structopt::StructOpt;
use url::Url;

use artefacta::{ArtefactIndex, Version};

#[derive(Debug, StructOpt)]
struct Cli {
    local_store: PathBuf,
    remote_store: Url,
    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
    Install { version: Version },
}

fn main() -> Result<()> {
    color_backtrace::install();
    pretty_env_logger::formatted_timed_builder()
        .filter(None, log::LevelFilter::Info)
        .filter(Some("artefacta"), log::LevelFilter::Debug)
        .init();

    let args = Cli::from_args();
    log::debug!("{:?}", args);
    let index = ArtefactIndex::from_dir(&args.local_store).context("open artifact store")?;
    match args.cmd {
        Command::Install { version } => todo!(),
    }

    Ok(())
}
