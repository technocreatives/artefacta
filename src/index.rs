use crate::{graph::FileSize, paths, PatchGraph, PatchName, Version};
use anyhow::{Context, Result};
use std::{
    fs::{self, read_dir, File},
    io::{BufReader, Read},
    path::{Path, PathBuf},
};
use zstd::stream::write::Encoder as ZstdEncoder;

#[derive(Debug)]
pub struct Index {
    root: PathBuf,
    patch_graph: PatchGraph,
}

impl Index {
    /// Build index from directory content
    pub fn from_dir(p: impl AsRef<Path>) -> Result<Self> {
        let path = p.as_ref();
        let dir = read_dir(path)
            .with_context(|| format!("could not read directory `{}`", path.display()))?
            .map(|entry| {
                let entry = entry.context("read file entry")?;
                let path = entry.path();
                let size = entry
                    .metadata()
                    .with_context(|| format!("read metadata of `{}`", path.display()))?
                    .len();
                Ok((path, size))
            })
            .collect::<Result<Vec<_>>>()
            .context("parse directory content")?;

        let patch_graph = PatchGraph::from_file_list(dir)
            .with_context(|| format!("build patch graph from `{}`", path.display()))?;

        Ok(Index {
            patch_graph,
            root: path.to_owned(),
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

        let old_build = self.get_build(from).context("get old build")?;
        let old_build = read_file(old_build).context("read old build")?;
        let new_build = self.get_build(to).context("get new build")?;
        let new_build = read_file(new_build).context("read new build")?;

        let path_name = PatchName { from, to };
        let patch_path = self.root.join(path_name.to_string() + ".zst");
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
        let path = self
            .root
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
        let path = self.root.join(version.as_str()).with_extension("tar.zst");
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

        anyhow::ensure!(
            !path.starts_with(&self.root),
            "asked to add build from index directory"
        );

        let file_name = paths::file_name(&path)?;
        let version: Version = file_name.parse()?;
        let new_path = self.root.join(format!("{}.tar.zst", version.as_str()));
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
}
