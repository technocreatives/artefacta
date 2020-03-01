use crate::{PatchGraph, Version};
use anyhow::{Context, Result};
use std::{
    fs::{read_dir, File},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

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
                let file_name = path
                    .file_name()
                    .with_context(|| format!("get file name of `{}`", path.display()))?;
                let file_name = file_name
                    .to_str()
                    .with_context(|| format!("file name `{:?}` not valid UTF-8", file_name))?
                    .to_string();
                let size = entry
                    .metadata()
                    .with_context(|| format!("read metadata of `{}`", path.display()))?
                    .size();
                Ok((file_name, size))
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
        todo!()
    }
}
