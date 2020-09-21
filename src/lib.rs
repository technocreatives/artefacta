use std::{convert::TryFrom, fs, path::Path};

use cli::AddBuild;
use erreur::{ensure, Context, Help, Result};

pub mod paths;

mod apply_patch;
pub use apply_patch::apply_patch;

mod index;
pub use index::{Index as ArtefactIndex, Version};

mod packaging;
pub use packaging::package;

mod storage;
pub use storage::Storage;

mod compression;
pub use compression::{compress, decompress};

mod partial_file;
pub use partial_file::PartialFile;

pub mod git;

pub mod cli;

#[cfg(test)]
pub(crate) mod test_helpers;

pub async fn sync(index: &ArtefactIndex) -> Result<()> {
    index.push().await.context("sync new local files to remote")
}

pub async fn install(
    index: &mut ArtefactIndex,
    target_version: Version,
    current: &Path,
) -> Result<()> {
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
    Ok(())
}

pub async fn add(index: &mut ArtefactIndex, build: cli::AddBuild) -> Result<()> {
    build.add_to(index).await.context("could not add new build")
}

pub async fn add_package(
    index: &mut ArtefactIndex,
    version: Version,
    build: cli::AddBuild,
) -> Result<()> {
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

    let mut archive_file = PartialFile::create(&archive_path)
        .with_context(|| format!("cannot create file `{}`", archive_path.display()))?;
    let mut archive = compress(&mut archive_file)
        .with_context(|| format!("cannot create zstd file `{}`", archive_path.display()))?;
    package(&build_path, &mut archive)
        .with_context(|| format!("package archive `{}`", archive_path.display()))?;
    archive
        .finish()
        .with_context(|| format!("write zstd archive `{}`", archive_path.display()))?;
    archive_file
        .finish()
        .context("faild to finish moving archive file into place")?;

    let add = AddBuild {
        path: archive_path,
        ..build
    };
    add.add_to(index).await.context("could not add new build")?;

    tmp.close()
        .context("could not clean up temporary directory")?;
    Ok(())
}

pub async fn create_patch(index: &mut ArtefactIndex, from: Version, to: Version) -> Result<()> {
    ensure!(
        from != to,
        "Rejecting to create patch between same versions ({}->{})",
        from,
        to
    );
    index.get_build(from.clone()).await?;
    index.get_build(to.clone()).await?;
    index.calculate_patch(from.clone(), to.clone()).await?;
    Ok(())
}

pub async fn auto_patch(
    index: &mut ArtefactIndex,
    repo_root: &Path,
    current: Version,
    prefix: &str,
) -> Result<()> {
    let current_build =
        Version::try_from(&format!("{}{}", prefix, current)).with_context(|| {
            format!(
                "given current version name is not valid with given prefix `{}`",
                prefix
            )
        })?;
    log::debug!("current version incl. given prefix is {}", current_build);
    index.get_build(current_build.clone()).await?;

    let repo = git2::Repository::discover(&repo_root)
        .with_context(|| format!("can't open repository at `{}`", repo_root.display()))
        .suggestion("If this path looks wrong, you can overwrite it with `--repo-root=<PATH>`")?;
    log::debug!("opened git repo {}", repo_root.display());
    let tags = git::get_tags(&repo).context("can't get tags from repo")?;
    let tag_names = tags
        .iter()
        .map(|tag| tag.name.clone())
        .collect::<Vec<String>>();
    log::trace!("found these tags in repo: {:?}", tag_names);

    let to_patch = git::find_tags_to_patch(current.as_str(), &tag_names)
        .context("can't find version to create patches for")?;
    log::info!("will create patches from these versions: {:?}", to_patch);

    let mut failed = false;
    for tag in &to_patch {
        let tag = format!("{}{}", prefix, tag);
        if let Err(e) = get_and_patch(index, &tag, current_build.clone()).await {
            log::error!("could not create patch from tag {}: {:?}", tag, e);
            failed = true;
        } else {
            log::info!("patch `{}` -> `{}`", tag, current_build);
        }
    }
    if failed {
        log::error!("failed to create patches");
        std::process::exit(1);
    }
    Ok(())
}

async fn get_and_patch(index: &mut ArtefactIndex, tag: &str, to: Version) -> Result<()> {
    let version = index.get_build_for_tag(tag)?;
    log::debug!("source version: picked {} from tag {}", version, tag);
    index.get_build(version.clone()).await?;
    index.calculate_patch(version.clone(), to.clone()).await?;
    Ok(())
}
