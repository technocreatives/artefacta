use crate::{paths, Version};
use anyhow::{Context, Result};
use std::{
    collections::HashMap, convert::TryFrom, fmt, fs::ReadDir, io::Error as IoError, path::PathBuf,
};

#[derive(Debug, Clone, Default)]
pub struct PatchGraph {
    pub(crate) graph: petgraph::graphmap::UnGraphMap<Version, FileSize>,
    build_sizes: HashMap<Version, FileSize>,
}

impl PatchGraph {
    pub fn from_file_list(list: impl IntoIterator<Item = (PathBuf, u64)>) -> Result<Self> {
        let mut res = PatchGraph::default();

        let (patches, builds): (Vec<_>, Vec<_>) = list
            .into_iter()
            .map(|(path, size)| Ok((paths::path_as_string(path)?, size)))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .partition(|(path, _size)| path.contains(".patch"));

        for (path, size) in dbg!(builds) {
            let file_name = paths::file_name(path)?;
            res.add_build(&file_name, size)
                .with_context(|| format!("add build `{}`", file_name))?;
        }

        for (path, size) in dbg!(patches) {
            let file_name = paths::file_name(path)?;
            res.add_patch(&file_name, size)
                .with_context(|| format!("add patch `{}`", file_name))?;
        }

        Ok(res)
    }

    pub(crate) fn add_build(&mut self, name: &str, size: u64) -> Result<()> {
        let build = paths::build_version_from_path(name).context("Build version from path")?;

        self.graph.add_node(build.clone());
        self.build_sizes.insert(build, FileSize(size));

        Ok(())
    }

    pub(crate) fn add_patch(&mut self, name: &str, size: u64) -> Result<()> {
        let (from, to) =
            paths::patch_versions_from_path(name).context("Patch versions from path")?;

        self.graph.add_edge(from, to, FileSize(size));

        Ok(())
    }

    fn patches_needed(&self, from: Version, to: Version) -> Option<(FileSize, Vec<String>)> {
        let (cost, steps) = petgraph::algo::astar(
            &self.graph,
            to,
            |f| f == from,
            |(_from, _to, size)| *size,
            |_| FileSize(0),
        )?;
        let path = steps
            .windows(2)
            .map(|x| {
                PatchName {
                    from: x[1],
                    to: x[0],
                }
                .to_string()
            })
            .collect();

        Some((cost, path))
    }

    pub fn find_upgrade_path(&self, from: Version, to: Version) -> Result<UpgradePath> {
        let build_size = *self
            .build_sizes
            .get(&to)
            .with_context(|| format!("unknown build size for `{:?}`", to))?;

        match self.patches_needed(from, to) {
            Some((size, path)) if build_size > size => Ok(UpgradePath::ApplyPatches(path)),
            _ => Ok(UpgradePath::InstallBuild(format!("{}", to.as_str()))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpgradePath {
    ApplyPatches(Vec<String>),
    InstallBuild(String),
}

impl TryFrom<ReadDir> for PatchGraph {
    type Error = IoError;

    fn try_from(_x: ReadDir) -> Result<Self, IoError> {
        todo!()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub(crate) struct FileSize(pub(crate) u64);

impl std::ops::Add<FileSize> for FileSize {
    type Output = FileSize;

    fn add(self, other: Self) -> Self::Output {
        FileSize(self.0 + other.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PatchName {
    pub(crate) from: Version,
    pub(crate) to: Version,
}

impl fmt::Display for PatchName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}-{}.patch", self.from.as_str(), self.to.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use std::{convert::TryFrom, path::PathBuf};

    #[test]
    fn this_is_fine() -> Result<()> {
        let graph = PatchGraph::from_file_list(vec![
            (PathBuf::from("1.tar.zst"), 42),
            (PathBuf::from("1-2.patch.zst"), 2),
            (PathBuf::from("2-3.patch.zst"), 30),
            (PathBuf::from("2.tar.zst"), 64),
            (PathBuf::from("3.tar.zst"), 72),
        ])?;
        let installed_version = Version::try_from("1")?;
        let target_version = Version::try_from("3")?;

        let res = graph.find_upgrade_path(installed_version, target_version)?;

        assert_eq!(
            res,
            UpgradePath::ApplyPatches(vec![String::from("2-3.patch"), String::from("1-2.patch")])
        );

        Ok(())
    }

    #[test]
    fn this_is_also_ok() -> Result<()> {
        let graph = PatchGraph::from_file_list(vec![
            (PathBuf::from("1.tar.zst"), 42),
            (PathBuf::from("1-2.patch.zst"), 2),
            (PathBuf::from("2-3.patch.zst"), 70),
            (PathBuf::from("2.tar.zst"), 64),
            (PathBuf::from("3.tar.zst"), 72),
        ])?;
        let installed_version = Version::try_from("1")?;
        let target_version = Version::try_from("3")?;

        let res = graph.find_upgrade_path(installed_version, target_version)?;

        assert_eq!(res, UpgradePath::InstallBuild(String::from("3")));

        Ok(())
    }
}
