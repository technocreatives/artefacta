use erreur::{ensure, Context, Help, Result};

use std::{fs, path::PathBuf};
use structopt::StructOpt;

use artefacta::{package, paths, ArtefactIndex, Storage, Version};

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
    /// Add a new build
    // TODO: Add option for calculating patches
    Add(AddBuild),
    /// Package a new build and add it
    AddPackage {
        /// Version of the build
        version: Version,
        #[structopt(flatten)]
        build: AddBuild,
    },
    /// Create a patch from one version to another
    CreatePatch {
        from: Version,
        to: Version,
    },
    Debug,
}

#[derive(Debug, StructOpt)]
struct AddBuild {
    /// Version of build
    path: PathBuf,
    /// Upload to remote storage
    #[structopt(long = "upload")]
    upload: bool,
    /// Calculate path from this build version
    #[structopt(long = "calc-patch-from")]
    calculate_patch_from: Option<Version>,
}

#[tokio::main]
async fn main() -> Result<()> {
    color_backtrace::install();

    let args = Cli::from_args();
    setup_logging(args.verbose);

    log::debug!("{:?}", args);
    let mut index = ArtefactIndex::new(&args.local_store, args.remote_store.clone())
        .await
        .context("open artifact store")
        .note("Always use absolute paths. This is serious business, there is no room for doubt.")?;
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
                    log::debug!(
                        "identified version `{}` from path `{}`",
                        current_version,
                        curent_path.display()
                    );

                    if current_version == target_version {
                        log::info!("version `{}` already installed", target_version);
                        return Ok(());
                    }

                    index
                        .upgrade_to_build(current_version, target_version.clone())
                        .await
                        .context("get build")?
                }
                Err(e) => {
                    log::debug!("could not read `current` symlink: {}", e);
                    index
                        .get_build(target_version.clone())
                        .await
                        .context("get build")?
                }
            };

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
            log::info!(
                "successfully installed `{}` as `{}`",
                target_version,
                current.display()
            );
        }
        Command::AddPackage { version, build } => {
            use tempfile::tempdir;
            use zstd::stream::write::Encoder as ZstdEncoder;

            let build_path = build
                .path
                .canonicalize()
                .with_context(|| format!("cannot canonicalize path `{}`", build.path.display()))?;

            let archive_name = format!("{}.tar.zst", version);
            let tmp = tempdir()
                .context("could not create temporary directory")
                .note("that is really strange: are you running this as weird dynamic user in systemd or something?")?;
            let archive_path = tmp.path().join(&archive_name);

            log::info!(
                "packaging `{}` into `{}`",
                build_path.display(),
                archive_path.display()
            );

            let archive = fs::File::create(&archive_path)
                .with_context(|| format!("cannot create file `{}`", archive_path.display()))?;
            let mut archive = ZstdEncoder::new(archive, 3)
                .with_context(|| format!("cannot create zstd file `{}`", archive_path.display()))?;
            package(&build_path, &mut archive)
                .with_context(|| format!("package archive `{}`", archive_path.display()))?;
            archive
                .finish()
                .with_context(|| format!("write zstd archive `{}`", archive_path.display()))
                .note(
                    "archive file was created but not written successfully -- clean it up yourself",
                )?;

            let add = AddBuild {
                path: archive_path,
                ..build
            };
            add.add_to(&mut index)
                .await
                .context("could not add new build")?;

            tmp.close()
                .context("could not clean up temporary directory")?;
        }
        Command::CreatePatch { from, to } => {
            index.get_build(from.clone()).await?;
            index.get_build(to.clone()).await?;
            index.calculate_patch(from.clone(), to.clone()).await?;
        }
        Command::Add(add) => add
            .add_to(&mut index)
            .await
            .context("could not add new build")?,
    }

    Ok(())
}

impl AddBuild {
    async fn add_to(&self, index: &mut ArtefactIndex) -> Result<()> {
        // TODO: Also set exitcode::NOINPUT in this case
        ensure!(
            self.path.exists(),
            "Tried to add `{}` as new build, but file does not exist",
            self.path.display()
        );

        let entry = index
            .add_local_build(&self.path)
            .await
            .with_context(|| format!("add `{}` as new build", self.path.display()))?;
        log::info!(
            "successfully added `{}` as `{:?}` to local index",
            self.path.display(),
            entry
        );

        if let Some(old_build) = self.calculate_patch_from.as_ref() {
            let new_build: Version = paths::file_name(&entry.path)?.parse()?;
            index
                .calculate_patch(old_build.clone(), new_build)
                .await
                .context("create patch for new build")?;
        }

        if self.upload {
            log::debug!("uploading new local artefacts to remote");
            index
                .push()
                .await
                .context("could not sync local changes to remote")?;
        }

        Ok(())
    }
}

fn setup_logging(verbose: bool) {
    let mut log = pretty_env_logger::formatted_timed_builder();
    log.target(env_logger::Target::Stderr);

    if verbose {
        log.filter(None, log::LevelFilter::Info)
            .filter(Some("artefacta"), log::LevelFilter::Debug);
    };

    if let Ok(s) = std::env::var("RUST_LOG") {
        log.parse_filters(&s);
    }

    log.init();
}
