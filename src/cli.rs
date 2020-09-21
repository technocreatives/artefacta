use crate::{paths, Storage, Version};
use erreur::{ensure, Context, Result, StdResult};
use std::{
    convert::Infallible,
    fmt,
    path::{Path, PathBuf},
    str::FromStr,
};
use structopt::StructOpt;

/// Manage software builds in different versions across local and remote storage
#[derive(Debug, StructOpt)]
pub struct Cli {
    /// Path to local storage directory
    #[structopt(long = "local", env = "ARTEFACTA_LOCAL_STORE")]
    pub local_store: PathBuf,
    /// Path/URL or remote storage
    #[structopt(long = "remote", env = "ARTEFACTA_REMOTE_STORE")]
    pub remote_store: Storage,
    #[structopt(subcommand)]
    pub cmd: Command,
    /// Print more debug output
    #[structopt(short = "v", long = "verbose")]
    pub verbose: bool,
}

#[derive(Debug, StructOpt)]
pub enum Command {
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
        /// Prefix for finding builds, used like "$prefix$tag". When setting
        /// this, omit the prefix from the current flag.
        #[structopt(long, default_value)]
        prefix: String,
    },
    /// Sync all new local files to remote store
    Sync,
    /// Build index (from local and remote data) and print it
    Debug,
}

#[derive(Debug, StructOpt)]
pub struct AddBuild {
    /// Path to the build
    pub path: PathBuf,
    /// Upload to remote storage
    #[structopt(long = "upload")]
    pub upload: bool,
    /// Calculate path from this build version
    #[structopt(long = "calc-patch-from")]
    pub calculate_patch_from: Option<Version>,
}

impl AddBuild {
    pub async fn add_to(&self, index: &mut crate::ArtefactIndex) -> Result<()> {
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

#[derive(Debug, Clone)]
pub struct WorkingDir(PathBuf);

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

impl AsRef<Path> for WorkingDir {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}
