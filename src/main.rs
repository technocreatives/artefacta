use anyhow::{Context, Result};
use std::{fs, path::PathBuf};
use structopt::StructOpt;

use artefacta::{paths, ArtefactIndex, Storage, Version};

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
        path: PathBuf,
        /// Upload to remote storage
        #[structopt(long = "upload")]
        upload: bool,
    },
    Debug,
}

#[tokio::main]
async fn main() -> Result<()> {
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
        .await
        .context("open artifact store")?;
    match args.cmd {
        Command::Debug => {
            dbg!(index);
        }
        Command::Install {
            version: target_version,
        } => {
            let current = args.local_store.join("current");
            let target_build = match fs::read_link(&current) {
                Ok(curent_path) => {
                    let current_version = paths::build_version_from_path(&curent_path)?;
                    index
                        .upgrade_to_build(current_version, target_version)
                        .await
                        .context("get build")?
                }
                Err(e) => {
                    log::debug!("could not read `current` symlink: {}", e);
                    index.get_build(target_version).await.context("get build")?
                }
            };

            dbg!(&target_build);

            #[cfg(unix)]
            use std::os::unix::fs::symlink;
            #[cfg(windows)]
            use std::os::windows::fs::symlink_file as symlink;

            if current.exists() {
                fs::remove_file(&current).context("clear old `current` symlink")?;
            }

            symlink(&target_build.path, &current).with_context(|| {
                format!(
                    "create symlink pointing at new build: {} to {}",
                    target_build.path,
                    current.display()
                )
            })?;
        }
        Command::Add { path, upload } => {
            index
                .add_local_build(&path)
                .with_context(|| format!("add `{}` as new build", path.display()))?;

            if upload {
                index.push().await.context("sync local changes to remote")?;
            }
        }
    }

    Ok(())
}
