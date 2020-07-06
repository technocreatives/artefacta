use erreur::{ensure, Context, Help, Result, StdResult};
use std::{
    convert::{Infallible, TryFrom},
    fmt, fs,
    path::PathBuf,
    str::FromStr,
};
use structopt::StructOpt;

use artefacta::{compress, package, paths, ArtefactIndex, Storage, Version};
pub(crate) mod git;

/// Manage software builds in different versions across local and remote storage
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
    CreatePatch { from: Version, to: Version },
    /// Create patches by looking at the git repo
    AutoPatch {
        /// Git repository in which to look for tags
        #[structopt(long, default_value)]
        repo_root: WorkingDir,
        /// Version to created patches to
        #[structopt(env = "CI_COMMIT_REF_NAME")]
        current: Version,
    },
    /// Sync all new local files to remote store
    Sync,
    /// Build index (from local and remote data) and print it
    Debug,
}

#[derive(Debug, StructOpt)]
struct AddBuild {
    /// Path to the build
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
    erreur::install_panic_handler()?;

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
        Command::Sync => {
            index
                .push()
                .await
                .context("sync new local files to remote")?;
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
            let mut archive = compress(archive)
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
        Command::AutoPatch {
            repo_root: WorkingDir(repo_root),
            current,
        } => {
            index.get_build(current.clone()).await?;

            let repo = git2::Repository::discover(&repo_root)
                .with_context(|| format!("can't open repository at `{}`", repo_root.display()))
                .suggestion(
                    "If this path looks wrong, you can overwrite it with `--repo-root=<PATH>`",
                )?;
            log::debug!("opened git repo {}", repo_root.display());
            let tags = git::get_tags(&repo).context("can't get tags from repo")?;
            let tag_names = tags
                .iter()
                .map(|tag| tag.name.clone())
                .collect::<Vec<String>>();
            log::trace!("found these tags in repo: {:?}", tag_names);
            ensure!(
                tag_names.iter().any(|tag| tag.as_str() == current.as_str()),
                "given version `{}` is not a tag in the repository (`{}`)",
                current,
                repo_root.display()
            );

            let to_patch = git::find_tags_to_patch(current.as_str(), &tag_names)
                .context("can't find version to create patches for")?;
            log::info!("will create patches from these versions: {:?}", to_patch);

            let mut failed = false;
            for tag in &to_patch {
                if let Err(e) = get_and_patch(&mut index, tag, current.clone()).await {
                    log::error!("could not create patch from tag {}: {:?}", tag, e);
                    failed = true;
                } else {
                    log::info!("create patch `{}` -> `{}`", tag, current);
                }
            }
            if failed {
                log::error!("failed to create patches");
                std::process::exit(1);
            }

            async fn get_and_patch(
                index: &mut ArtefactIndex,
                tag: &str,
                to: Version,
            ) -> Result<()> {
                let version = Version::try_from(tag)
                    .with_context(|| format!("cant' parse tag `{}` as version", tag))?;
                index.get_build(version.clone()).await?;
                index.calculate_patch(version.clone(), to.clone()).await?;
                Ok(())
            }
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

#[derive(Debug, Clone)]
struct WorkingDir(PathBuf);

impl Default for WorkingDir {
    fn default() -> Self {
        WorkingDir(std::env::current_dir().expect("cannot access current working directory"))
    }
}

impl FromStr for WorkingDir {
    type Err = Infallible;

    fn from_str(s: &str) -> StdResult<Self, Infallible> {
        Ok(WorkingDir(s.into()))
    }
}

impl fmt::Display for WorkingDir {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0.display())
    }
}
