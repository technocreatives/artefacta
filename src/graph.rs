use anyhow::{Context, Result};
use std::{collections::HashMap, convert::TryFrom, fmt, fs::ReadDir, io::Error as IoError};

#[derive(Debug, Clone)]
pub struct PatchGraph {
    graph: petgraph::graphmap::UnGraphMap<Version, FileSize>,
    build_sizes: HashMap<Version, FileSize>,
}

impl PatchGraph {
    pub fn from_file_list(list: Vec<(String, usize)>) -> Result<Self> {
        let mut graph = petgraph::graphmap::GraphMap::new();
        let mut build_sizes = HashMap::new();

        let (patches, builds): (Vec<(String, usize)>, Vec<(String, usize)>) = list
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

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Version {
    len: u8,
    data: [u8; 23],
}

impl Version {
    fn as_str(&self) -> &str {
        // Data is a valid subset of an UTF-8 string by construction
        unsafe { std::str::from_utf8_unchecked(&self.data[..self.len as usize]) }
    }
}

impl fmt::Debug for Version {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_tuple("Version")
            .field(&String::from_utf8_lossy(&self.data[..self.len as usize]))
            .finish()
    }
}

impl<'a> TryFrom<&'a str> for Version {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<Self> {
        let mut v = [0u8; 23];
        let len = std::cmp::min(s.len(), 23);

        anyhow::ensure!(
            s.is_char_boundary(len),
            "version string too long and cut-off point not at char boundary"
        );

        s.as_bytes()[..len]
            .iter()
            .enumerate()
            .for_each(|(i, c)| v[i] = *c);

        Ok(Version {
            len: len as u8,
            data: v,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
struct FileSize(usize);

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
