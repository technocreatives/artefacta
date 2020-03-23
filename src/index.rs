use crate::{
    apply_patch, paths,
    storage::{Entry, Storage},
};
use anyhow::{Context, Result};
use std::{
    convert::TryFrom,
    fs::{self, File},
    io::{self, BufReader, Read},
    path::Path,
};

use zstd::stream::write::Encoder as ZstdEncoder;

mod build;
pub use build::Build;
mod patch;
pub use patch::Patch;
mod graph;
pub use graph::{Location, PatchGraph, UpgradePath};
mod version;
pub use version::Version;

/// Artefact index
///
/// Contains local and remote storage as well as graph build from the current
/// contents of the storages.
///
/// This is the main entry point for interacting with any build and patch files.
#[derive(Debug, Clone)]
pub struct Index {
    local: Storage,
    remote: Storage,
    patch_graph: PatchGraph,
}

impl Index {
    /// Build index from directory content
    pub fn new(local: impl AsRef<Path>, remote: Storage) -> Result<Self> {
        let local = Storage::try_from(local.as_ref()).context("invalid local storage path")?;
        let mut patch_graph = PatchGraph::empty();
        patch_graph
            .update_from_file_list(&local.list_files().context("list files")?, Location::Local)
            .with_context(|| format!("build patch graph from `{:?}`", local))?;
        patch_graph
            .update_from_file_list(
                &remote.list_files().context("list files")?,
                Location::Remote,
            )
            .with_context(|| format!("build patch graph from `{:?}`", remote))?;

        Ok(Index {
            local,
            remote,
            patch_graph,
        })
    }

    /// Generate patches from leaf nodes to disconnected nodes
    pub fn generate_missing_patches(&mut self) -> Result<Vec<String>> {
        todo!()
    }

    pub fn calculate_patch(&mut self, from: Version, to: Version) -> Result<()> {
        fn read_file(entry: Entry) -> Result<Vec<u8>> {
            anyhow::ensure!(
                entry.storage.is_local(),
                "only reading from local storage supported"
            );
            let path = entry.path;
            let file =
                File::open(&path).with_context(|| format!("could not open file {}", path))?;
            let mut file = BufReader::new(file);
            let mut bytes = Vec::with_capacity(2 << 20);
            file.read_to_end(&mut bytes).context("read file")?;
            Ok(bytes)
        }

        if self.get_patch(from.clone(), to.clone()).is_ok() {
            log::warn!(
                "asked to calculate patch from `{:?}` to `{:?}` but it's already present",
                from,
                to
            );
            return Ok(());
        }

        let local = self
            .local
            .local_path()
            .context("calculate patch can only write to local storage right now")?;

        let old_build = self.get_build(from.clone()).context("get old build")?;
        let old_build = read_file(old_build).context("read old build")?;
        let new_build = self.get_build(to.clone()).context("get new build")?;
        let new_build = read_file(new_build).context("read new build")?;

        let path_name = Patch::new(from.clone(), to.clone());
        // TODO: Fix that arbitrary "+ zst" here and everywhere else
        let patch_path = local.join(path_name.to_string() + ".zst");
        log::info!("write patch {:?} to `{:?}`", path_name, patch_path);

        let mut patch = ZstdEncoder::new(File::create(&patch_path)?, 3)?;
        bidiff::simple_diff(&old_build, &new_build, &mut patch)?;
        patch.finish()?;

        let patch_size = patch_path
            .metadata()
            .with_context(|| {
                format!(
                    "can't read metadata for new patch file `{}`",
                    patch_path.display()
                )
            })?
            .len();

        let entry = Entry {
            storage: self.local.clone(),
            path: paths::path_as_string(patch_path)?,
            size: patch_size,
        };

        self.patch_graph
            .add_patch(&from, &to, entry, Location::Local)?;

        Ok(())
    }

    pub fn get_patch(&mut self, from: Version, to: Version) -> Result<Entry> {
        anyhow::ensure!(
            self.patch_graph.has_patch(from.clone(), to.clone()),
            "patch `{:?}` unknown",
        );

        let patch = Patch::new(from.clone(), to.clone());
        let patch_name = patch.to_string() + ".zst";
        match self.local.get_file(&patch_name) {
            Ok(local_file) => return Ok(local_file),
            Err(e) => log::debug!("could not get local patch {:?}: {}", patch, e),
        }

        let remote_entry = self
            .remote
            .get_file(&patch_name)
            .with_context(|| format!("can't find `{}` either locally or remotely", patch))?;

        self.add_patch(&remote_entry.path)
            .context("copy remote entry to local storage")?;
        let newly_local_build = self
            .local
            .get_file(&patch_name)
            .context("fetch newly added local build")?;
        Ok(newly_local_build)
    }

    /// Upgrade from one version to the next
    pub fn upgrade_to_build(&mut self, from: Version, to: Version) -> Result<Entry> {
        anyhow::ensure!(
            self.patch_graph.has_build(from.clone()),
            "build `{:?}` unknown",
            from
        );
        anyhow::ensure!(
            self.patch_graph.has_build(to.clone()),
            "build `{:?}` unknown",
            to
        );

        match self
            .patch_graph
            .find_upgrade_path(from.clone(), to.clone())
            .with_context(|| format!("can't find upgrade path from `{:?}` to `{:?}", from, to))?
        {
            UpgradePath::ApplyPatches(patches) => {
                let needed_patches = patches
                    .into_iter()
                    .skip_while(|patch| self.patch_graph.has_build(patch.to.clone()))
                    .collect::<Vec<Patch>>();

                for patch in needed_patches {
                    self.add_build_from_patch(&patch)
                        .with_context(|| format!("add build from patch `{:?}`", patch))?;
                }

                let local_build = self.get_build(to).context("fetch just added build")?;
                Ok(local_build)
            }
            UpgradePath::InstallBuild(_build) => {
                let local_build = self.get_build(to).context("install fresh build")?;
                Ok(local_build)
            }
        }
    }

    fn add_build_from_patch(&mut self, patch: &Patch) -> Result<Entry> {
        let patch_file = self
            .get_patch(patch.from.clone(), patch.to.clone())
            .context("fetch patch")?;
        let source_build = self
            .get_build(patch.from.clone())
            .context("fetch source build")?;

        let build_temp_name = format!("tmp.{}.tar.zst", patch.to);
        let build_real_name = format!("{}.tar.zst", patch.to);

        let build_root = self.local.local_path().context("local storage not local")?;
        let build_temp_path = build_root.join(&build_temp_name);
        let build_real_path = build_root.join(&build_real_name);

        let mut build_file = ZstdEncoder::new(
            File::create(&build_temp_path).with_context(|| {
                format!("create new build file `{}`", build_temp_path.display())
            })?,
            3,
        )
        .context("zstd writer for new build")?;
        let mut patch_data =
            apply_patch(&source_build.path, &patch_file.path).context("apply patch")?;

        io::copy(&mut patch_data, &mut build_file).context("write patch")?;
        build_file.finish().context("finish zstd writer")?;

        fs::rename(&build_temp_path, &build_real_path).context("rename tmp build file")?;

        let entry = Entry::from_path(&build_real_path, self.local.clone())
            .context("create entry for new build file")?;

        self.patch_graph
            .add_build(&patch.to, entry.clone(), Location::Local)
            .with_context(|| {
                format!(
                    "add newly created build `{}` to index",
                    build_real_path.display()
                )
            })?;
        Ok(entry)
    }

    /// Get build (adds to local cache if not present)
    pub fn get_build(&mut self, version: Version) -> Result<Entry> {
        anyhow::ensure!(
            self.patch_graph.has_build(version.clone()),
            "build `{:?}` unknown",
            version
        );

        let build_path = paths::build_path_from_version(version.clone())?;
        match self.local.get_file(&build_path) {
            Ok(local_file) => return Ok(local_file),
            Err(e) => log::debug!("could not get local build for {}: {}", version.as_str(), e),
        }

        let remote_entry = self.remote.get_file(&build_path).with_context(|| {
            format!(
                "can't find `{}` either locally or remotely",
                version.as_str()
            )
        })?;

        self.add_build(&remote_entry.path)
            .context("copy remote entry to local storage")?;
        let newly_local_build = self
            .local
            .get_file(&build_path)
            .context("fetch newly added local build")?;
        Ok(newly_local_build)
    }

    /// Add build to graph and copy it into index's root directory
    pub(crate) fn add_build(&mut self, path: impl AsRef<Path>) -> Result<Entry> {
        let path = path.as_ref();
        let path = path
            .canonicalize()
            .with_context(|| format!("canonicalize {}", path.display()))?;

        let local = self
            .local
            .local_path()
            .context("add_build can only write to local storage right now")?;

        anyhow::ensure!(
            !path.starts_with(&local),
            "asked to add build from same directory it would be written to"
        );

        let file_name = paths::file_name(&path)?;
        let version: Version = file_name.parse()?;
        let new_path = local.join(format!("{}.tar.zst", version.as_str()));
        fs::copy(&path, &new_path)
            .with_context(|| format!("copy `{}` to `{}`", path.display(), new_path.display()))?;

        let entry = Entry::from_path(&new_path, self.local.clone())
            .context("create entry for new build file")?;

        self.patch_graph
            .add_build(&version, entry.clone(), Location::Local)
            .with_context(|| format!("add build `{}`", path.display()))?;
        Ok(entry)
    }

    /// Add build to graph and copy it into index's root directory
    ///
    /// TODO: Refactor this and add_build to be the same generic method
    pub(crate) fn add_patch(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let path = path
            .canonicalize()
            .with_context(|| format!("canonicalize {}", path.display()))?;

        let local = self
            .local
            .local_path()
            .context("add_patch can only write to local storage right now")?;

        anyhow::ensure!(
            !path.starts_with(&local),
            "asked to add patch from same directory it would be written to"
        );

        let (from, to) = paths::patch_versions_from_path(&path)?;
        let patch = Patch::new(from.clone(), to.clone());
        let new_path = local.join(patch.to_string());
        fs::copy(&path, &new_path)
            .with_context(|| format!("copy `{}` to `{}`", path.display(), new_path.display()))?;

        let entry = Entry::from_path(&new_path, self.local.clone())
            .context("create entry for new build file")?;

        self.patch_graph
            .add_patch(&from, &to, entry, Location::Local)
            .with_context(|| format!("add patch `{}`", path.display()))?;
        Ok(())
    }

    // Fetch current state from S3 and upload all missing files (i.e. new builds
    // and patches)
    pub fn push(&self) -> Result<()> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use std::convert::TryInto;

    // TODO: Add same but with one the builds only available on remote
    #[test]
    fn create_patch() -> Result<()> {
        let local_dir = tempdir()?;
        let remote_dir = tempdir()?;

        // Add some builds
        let _build1 = random_file(local_dir.path().join("build1.tar.zst"))?;
        let _build2 = random_file(local_dir.path().join("build2.tar.zst"))?;
        let _build3 = random_file(local_dir.path().join("build3.tar.zst"))?;

        let mut index = Index::new(local_dir.path(), remote_dir.path().try_into()?)?;

        index.calculate_patch("build2".parse()?, "build3".parse()?)?;

        index.get_patch("build2".parse()?, "build3".parse()?)?;

        Ok(())
    }

    #[test]
    fn generate_patches() -> Result<()> {
        let dir = test_dir(&["1.tar.zst", "2.tar.zst", "1-2.patch.zst"])?;
        let remote_dir = test_dir(&["3.tar.zst"])?;

        let mut index = dbg!(Index::new(&dir, remote_dir.path().try_into()?,)?);
        index.add_build(&remote_dir.path().join("3.tar.zst"))?;

        assert!(
            index.get_build("3".parse()?).is_ok(),
            "didn't add build to index {:?}",
            index
        );

        index
            .calculate_patch("2".parse()?, "3".parse()?)
            .context("calc patches")?;

        dbg!(&index);

        index.get_patch("2".parse()?, "3".parse()?).unwrap();

        Ok(())
    }

    fn test_dir(files: &[&str]) -> Result<tempfile::TempDir> {
        let dir = tempdir()?;
        let mut rng = rand::thread_rng();

        for file in files {
            let mut raw_content = vec![0u8; 1024];
            rng.try_fill(&mut raw_content[..])?;
            let content = zstd::stream::encode_all(Cursor::new(&raw_content[..]), 3)?;

            fs::write(dir.path().join(file), content).context("write file")?;
        }

        Ok(dir)
    }
}
