use anyhow::{Context, Result};
use std::path::PathBuf;
use structopt::StructOpt;

use artefacta::{ArtefactIndex, Storage, Version};

#[derive(Debug, StructOpt)]
struct Cli {
    local_store: PathBuf,
    remote_store: Storage,
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
    let _index = ArtefactIndex::new(&args.local_store, args.remote_store.clone())
        .context("open artifact store")?;
    match args.cmd {
        Command::Install { .. } => todo!(),
    }

    Ok(())
}
