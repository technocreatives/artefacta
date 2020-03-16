use crate::{
    graph::FileSize,
    paths,
    storage::{Entry, Storage},
    PatchGraph, PatchName, Version,
};
use anyhow::{Context, Result};
use std::{
    convert::TryFrom,
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
        let local = Storage::try_from(local.as_ref()).context("invalid local storage path")?;
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

        if self.get_patch(from, to).is_ok() {
            log::warn!(
                "asked to calculate patch from `{:?}` to `{:?}` but it's already present",
                from,
                to
            );
            return Ok(());
        }

        let local = if let Storage::Filesystem(p) = self.local.clone() {
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

    /// Get build (adds to local cache if not present)
    ///
    /// TODO: Use `Index::find_upgrade_path` if build isn't available locally
    pub fn get_build(&mut self, version: Version) -> Result<Entry> {
        anyhow::ensure!(
            self.patch_graph.graph.contains_node(version),
            "build `{:?}` unknown",
            version
        );

        let build_path = paths::build_path_from_version(version)?;
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
    pub(crate) fn add_build(&mut self, path: impl AsRef<Path>) -> Result<()> {
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

    #[test]
    fn generate_patches() -> Result<()> {
        let dir = test_dir(&["1.tar.zst", "2.tar.zst", "1-2.patch.zst"])?;
        let remote_dir = test_dir(&["3.tar.zst"])?;

        let mut index = dbg!(Index::new(
            &dir,
            Storage::Filesystem(remote_dir.path().into()),
        )?);
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
