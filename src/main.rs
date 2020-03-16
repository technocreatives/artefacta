use anyhow::{Context, Result};
use std::path::PathBuf;
use structopt::StructOpt;

use artefacta::{ArtefactIndex, Storage, Version};

#[derive(Debug, StructOpt)]
struct Cli {
    #[structopt(long = "local", env = "ARTEFACTA_LOCAL_STORE")]
    local_store: PathBuf,
    #[structopt(long = "remote", env = "ARTEFACTA_REMOTE_STORE")]
    remote_store: Storage,
    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
    Install {
        version: Version,
    },
    Add {
        version: Version,
        #[structopt(long = "upload")]
        upload: bool,
    },
}

fn main() -> Result<()> {
    color_backtrace::install();
    pretty_env_logger::formatted_timed_builder()
        .filter(None, log::LevelFilter::Info)
        .filter(Some("artefacta"), log::LevelFilter::Debug)
        .target(env_logger::Target::Stderr)
        .init();

    let args = Cli::from_args();
    log::debug!("{:?}", args);
    let mut index = ArtefactIndex::new(&args.local_store, args.remote_store.clone())
        .context("open artifact store")?;
    match args.cmd {
        Command::Install { version } => {
            let build = index.get_build(version).context("get build")?;
            dbg!(build);
        }
        Command::Add { .. } => todo!("add add"),
    }

    Ok(())
}
