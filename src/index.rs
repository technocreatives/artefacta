use crate::{graph::FileSize, paths, storage::Storage, PatchGraph, PatchName, Version};
use anyhow::{Context, Result};
use std::{
    fs::{self, File},
    io::{BufReader, Read},
    path::Path,
};

use zstd::stream::write::Encoder as ZstdEncoder;

#[derive(Debug)]
pub struct Index {
    local: Storage,
    remote: Storage,
    patch_graph: PatchGraph,
}

impl Index {
    /// Build index from directory content
    pub fn new(local: impl AsRef<Path>, remote: Storage) -> Result<Self> {
        let local = Storage::Filesystem(local.as_ref().into());
        let patch_graph = PatchGraph::from_file_list(&local.list_files().context("list files")?)
            .with_context(|| format!("build patch graph from `{:?}`", local))?;

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
        fn read_file(file: File) -> Result<Vec<u8>> {
            let mut file = BufReader::new(file);
            let mut bytes = Vec::with_capacity(2 << 20);
            file.read_to_end(&mut bytes).context("read file")?;
            Ok(bytes)
        }

        if self.get_patch(from, to).is_ok() {
            log::warn!(
                "asked to calculate patch from `{:?}` to `{:?}` but it's already present",
                from,
                to
            );
            return Ok(());
        }

        let local = if let Storage::Filesystem(p) = &self.local {
            p
        } else {
            anyhow::bail!("calculate patch can only write to local storage right now");
        };

        let old_build = self.get_build(from).context("get old build")?;
        let old_build = read_file(old_build).context("read old build")?;
        let new_build = self.get_build(to).context("get new build")?;
        let new_build = read_file(new_build).context("read new build")?;

        let path_name = PatchName { from, to };
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

        self.patch_graph
            .graph
            .add_edge(from, to, FileSize(patch_size));

        Ok(())
    }

    pub fn get_patch(&self, from: Version, to: Version) -> Result<File> {
        anyhow::ensure!(
            self.patch_graph.graph.contains_edge(from, to),
            "patch `{:?}` unknown",
            (from, to)
        );
        let local = if let Storage::Filesystem(p) = &self.local {
            p
        } else {
            anyhow::bail!("calculate patch can only write to local storage right now");
        };

        let path = local
            .join(format!("{}-{}", from.as_str(), to.as_str()))
            .with_extension("patch.zst");
        let file = File::open(&path).with_context(|| {
            format!(
                "could not open file `{:?}` for patch `{:?}`",
                path.display(),
                (from, to)
            )
        })?;
        Ok(file)
    }

    pub fn get_build(&self, version: Version) -> Result<File> {
        anyhow::ensure!(
            self.patch_graph.graph.contains_node(version),
            "build `{:?}` unknown",
            version
        );
        let local = if let Storage::Filesystem(p) = &self.local {
            p
        } else {
            anyhow::bail!("get_build can read from local storage right now");
        };

        let path = local.join(version.as_str()).with_extension("tar.zst");
        let file = File::open(&path).with_context(|| {
            format!(
                "could not open file `{:?}` for build `{:?}`",
                path.display(),
                version
            )
        })?;
        Ok(file)
    }

    /// Add build to graph and copy it into index's root directory
    pub fn add_build(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let path = path
            .canonicalize()
            .with_context(|| format!("canonicalize {}", path.display()))?;

        let local = if let Storage::Filesystem(p) = &self.local {
            p
        } else {
            anyhow::bail!("add_build can only write to local storage right now");
        };

        anyhow::ensure!(
            !path.starts_with(local),
            "asked to add build from index directory"
        );

        let file_name = paths::file_name(&path)?;
        let version: Version = file_name.parse()?;
        let new_path = local.join(format!("{}.tar.zst", version.as_str()));
        fs::copy(&path, &new_path)
            .with_context(|| format!("copy `{}` to `{}`", path.display(), new_path.display()))?;

        let size = path
            .metadata()
            .with_context(|| {
                format!(
                    "can't read metadata for new build file `{}`",
                    path.display()
                )
            })?
            .len();
        self.patch_graph
            .add_build(&file_name, size)
            .with_context(|| format!("add build `{}`", path.display()))?;
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

    #[test]
    fn construct_index() -> Result<()> {
        let dir = tempdir()?;

        let _index = Index::new(dir.path(), "s3://my-bucket/".parse()?)?;

        let remote_dir = tempdir()?;
        let _index = Index::new(dir.path(), remote_dir.path().try_into()?)?;

        Ok(())
    }

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
}
