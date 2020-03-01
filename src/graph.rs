use crate::Version;
use anyhow::{Context, Result};
use std::{collections::HashMap, convert::TryFrom, fs::ReadDir, io::Error as IoError};

#[derive(Debug, Clone)]
pub struct PatchGraph {
    pub(crate) graph: petgraph::graphmap::UnGraphMap<Version, FileSize>,
    build_sizes: HashMap<Version, FileSize>,
}

impl PatchGraph {
    pub fn from_file_list(list: impl IntoIterator<Item = (String, u64)>) -> Result<Self> {
        let mut graph = petgraph::graphmap::GraphMap::new();
        let mut build_sizes = HashMap::new();

        let (patches, builds): (Vec<_>, Vec<_>) = list
            .into_iter()
            .partition(|(filename, _size)| filename.ends_with(".patch"));

        for (filename, size) in builds {
            let build = Version::try_from(filename.as_str())?;
            graph.add_node(build.clone());
            build_sizes.insert(build, FileSize(size));
        }

        for (filename, size) in patches {
            let name = filename.trim_end_matches(".patch");
            let parts: Vec<&str> = name.splitn(2, '-').collect();
            anyhow::ensure!(
                parts.len() == 2,
                "patch file name pattern is not `<hash>-<hash>`: `{}`",
                name,
            );

            graph.add_edge(
                Version::try_from(parts[0])?,
                Version::try_from(parts[1])?,
                FileSize(size),
            );
        }

        Ok(PatchGraph { graph, build_sizes })
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
            .map(|x| format!("{}-{}.patch", x[1].as_str(), x[0].as_str()))
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
pub(crate) struct FileSize(u64);

impl std::ops::Add<FileSize> for FileSize {
    type Output = FileSize;

    fn add(self, other: Self) -> Self::Output {
        FileSize(self.0 + other.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Patch {
    filename: String,
    size: FileSize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use std::convert::TryFrom;

    #[test]
    fn this_is_fine() -> Result<()> {
        let graph = PatchGraph::from_file_list(vec![
            (String::from("1"), 42),
            (String::from("1-2.patch"), 2),
            (String::from("2-3.patch"), 30),
            (String::from("2"), 64),
            (String::from("3"), 72),
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
            (String::from("1"), 42),
            (String::from("1-2.patch"), 2),
            (String::from("2-3.patch"), 70),
            (String::from("2"), 64),
            (String::from("3"), 72),
        ])?;
        let installed_version = Version::try_from("1")?;
        let target_version = Version::try_from("3")?;

        let res = graph.find_upgrade_path(installed_version, target_version)?;

        assert_eq!(res, UpgradePath::InstallBuild(String::from("3")));

        Ok(())
    }
}
