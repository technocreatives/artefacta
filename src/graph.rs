use crate::{paths, storage::Entry, Version};
use anyhow::{Context, Result};
use std::{collections::HashMap, convert::TryFrom, fmt, fs::ReadDir, io::Error as IoError};

/// Graph of builds and upgrade paths using patches
//
// NOTE: Builds are represented by their version alone right now. This is
// allows us to use the GraphMap type which has a simpler API. In the future we
// might want to introduce another structure that maps between graph indices
// and associated information, so we can also keep track of which files exists
// locally and which only exist remotely. This will allow quick syncing of
// files to remote, as well as optimizing for _download_ size instead of file
// size when searching for the optimal upgrade path.
#[derive(Debug, Clone, Default)]
pub struct PatchGraph {
    pub(crate) graph: petgraph::graphmap::UnGraphMap<Version, FileSize>,
    build_sizes: HashMap<Version, FileSize>,
}

impl PatchGraph {
    pub fn from_file_list(list: &[Entry]) -> Result<Self> {
        let mut res = PatchGraph::default();

        let (patches, builds): (Vec<_>, Vec<_>) = list
            .into_iter()
            .partition(|entry| entry.path.contains(".patch"));

        for Entry { path, size, .. } in dbg!(builds) {
            let file_name = paths::file_name(path)?;
            res.add_build(&file_name, *size)
                .with_context(|| format!("add build `{}`", file_name))?;
        }

        for Entry { path, size, .. } in dbg!(patches) {
            let file_name = paths::file_name(path)?;
            res.add_patch(&file_name, *size)
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
    use crate::Storage;
    use anyhow::Result;
    use std::{convert::TryFrom, path::Path};

    #[test]
    fn this_is_fine() -> Result<()> {
        let graph = PatchGraph::from_file_list(&[
            Entry {
                storage: Storage::try_from(Path::new("/tmp"))?,
                path: "1.tar.zst".into(),
                size: 42,
            },
            Entry {
                storage: Storage::try_from(Path::new("/tmp"))?,
                path: "1-2.patch.zst".into(),
                size: 2,
            },
            Entry {
                storage: Storage::try_from(Path::new("/tmp"))?,
                path: "2-3.patch.zst".into(),
                size: 20,
            },
            Entry {
                storage: Storage::try_from(Path::new("/tmp"))?,
                path: "2.tar.zst".into(),
                size: 64,
            },
            Entry {
                storage: Storage::try_from(Path::new("/tmp"))?,
                path: "3.tar.zst".into(),
                size: 72,
            },
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
        let graph = PatchGraph::from_file_list(&[
            Entry {
                storage: Storage::try_from(Path::new("/tmp"))?,
                path: "1.tar.zst".into(),
                size: 42,
            },
            Entry {
                storage: Storage::try_from(Path::new("/tmp"))?,
                path: "1-2.patch.zst".into(),
                size: 2,
            },
            Entry {
                storage: Storage::try_from(Path::new("/tmp"))?,
                path: "2-3.patch.zst".into(),
                size: 70, // <- large now!
            },
            Entry {
                storage: Storage::try_from(Path::new("/tmp"))?,
                path: "2.tar.zst".into(),
                size: 64,
            },
            Entry {
                storage: Storage::try_from(Path::new("/tmp"))?,
                path: "3.tar.zst".into(),
                size: 72,
            },
        ])?;
        let installed_version = Version::try_from("1")?;
        let target_version = Version::try_from("3")?;

        let res = graph.find_upgrade_path(installed_version, target_version)?;

        assert_eq!(res, UpgradePath::InstallBuild(String::from("3")));

        Ok(())
    }
}
