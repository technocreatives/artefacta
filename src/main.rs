use anyhow::{Context, Result};
use std::path::PathBuf;
use structopt::StructOpt;

use artefacta::{ArtefactIndex, Storage, Version};

#[derive(Debug, StructOpt)]
struct Cli {
    /// Path to local storage directory
    #[structopt(long = "local", env = "ARTEFACTA_LOCAL_STORE")]
    local_store: PathBuf,
    /// Path/URL or remote storage
    #[structopt(long = "remote", env = "ARTEFACTA_REMOTE_STORE")]
    remote_store: Storage,
    #[structopt(subcommand)]
    cmd: Command,
    /// Print more debug output
    #[structopt(short = "v", long = "verbose")]
    verbose: bool,
}

#[derive(Debug, StructOpt)]
enum Command {
    /// Install new build
    Install {
        /// Version of the build to install
        version: Version,
    },
    /// Add a new build to local storage
    // TODO: Add option for calculating patches
    Add {
        /// Version of build
        version: Version,
        /// Upload to remote storage
        #[structopt(long = "upload")]
        upload: bool,
    },
}

fn main() -> Result<()> {
    color_backtrace::install();

    let args = Cli::from_args();

    pretty_env_logger::formatted_timed_builder()
        .filter(None, log::LevelFilter::Info)
        .filter(
            Some("artefacta"),
            if args.verbose {
                log::LevelFilter::Debug
            } else {
                log::LevelFilter::Info
            },
        )
        .target(env_logger::Target::Stderr)
        .init();

    log::debug!("{:?}", args);
    let mut index = ArtefactIndex::new(&args.local_store, args.remote_store.clone())
        .context("open artifact store")?;
    match args.cmd {
        Command::Install { version } => {
            let build = index.get_build(version).context("get build")?;
            dbg!(&build);

            let current = args.local_store.join("current");

            #[cfg(unix)]
            use std::os::unix::fs::symlink;
            #[cfg(windows)]
            use std::os::windows::fs::symlink_file as symlink;

            symlink(&build.path, &current).with_context(|| {
                format!(
                    "create symlink pointing at new build: {} to {}",
                    build.path,
                    current.display()
                )
            })?;
        }
        Command::Add { .. } => todo!("add add"),
    }

    Ok(())
}
