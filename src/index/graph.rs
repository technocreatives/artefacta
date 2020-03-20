use super::{Build, Patch, Version};
use crate::{paths, storage::Entry};
use anyhow::{Context, Result};
use petgraph::stable_graph::{DefaultIx, EdgeIndex, NodeIndex, StableGraph as Graph};
use std::{collections::HashMap, convert::TryFrom, fs::ReadDir, io::Error as IoError};

/// Graph of builds and upgrade paths using patches
///
/// Tracks all builds (identified by local and remote files) as well as patches
/// between them. Builds are nodes in the directed graph, patches are edges
/// between them.
#[derive(Debug, Clone, Default)]
pub struct PatchGraph {
    graph: Graph<Build, Patch>,
    /// helper for looking up nodes in the graph
    builds: HashMap<Version, NodeIndex<DefaultIx>>,
    /// helper for looking up edges in the graph
    patches: HashMap<(Version, Version), EdgeIndex<DefaultIx>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Location {
    Local,
    Remote,
}

impl PatchGraph {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn update_from_file_list(&mut self, list: &[Entry], location: Location) -> Result<()> {
        let (patches, builds): (Vec<_>, Vec<_>) =
            list.iter().partition(|entry| entry.path.contains(".patch"));

        log::trace!("Builds: {:?}", builds);
        for entry in builds {
            let version = paths::build_version_from_path(&entry.path)?;
            self.add_build(&version, entry.clone(), location)
                .with_context(|| format!("add build `{}`", entry.path))?;
        }

        log::trace!("Patches: {:?}", patches);
        for entry in patches {
            let (from, to) =
                paths::patch_versions_from_path(&entry.path).context("Patch versions from path")?;
            self.add_patch(&from, &to, entry.clone(), location)
                .with_context(|| format!("add patch `{}`", entry.path))?;
        }

        Ok(())
    }

    pub(crate) fn add_build(
        &mut self,
        version: &Version,
        entry: Entry,
        location: Location,
    ) -> Result<()> {
        use std::collections::hash_map::Entry;

        let build = match self.builds.entry(version.clone()) {
            Entry::Occupied(e) => self
                .graph
                .node_weight_mut(*e.get())
                .context("`builds` points to non-existing NodeIndex")?,
            Entry::Vacant(e) => {
                let build = Build::new(version.clone());
                let idx = self.graph.add_node(build);
                e.insert(idx);
                self.graph
                    .node_weight_mut(idx)
                    .context("`builds` points to existing NodeIndex")?
            }
        };

        match location {
            Location::Local => {
                build.set_local(entry);
            }
            Location::Remote => {
                build.set_remote(entry);
            }
        }

        Ok(())
    }

    pub(crate) fn add_patch(
        &mut self,
        from: &Version,
        to: &Version,
        entry: Entry,
        location: Location,
    ) -> Result<()> {
        use std::collections::hash_map::Entry;

        let patch = match self.patches.entry((from.clone(), to.clone())) {
            Entry::Occupied(e) => self
                .graph
                .edge_weight_mut(*e.get())
                .context("`patches` points to non-existing EdgeIndex")?,
            Entry::Vacant(e) => {
                let patch = Patch::new(from.clone(), to.clone());
                let prev_build = *self.builds.get(from).context("can't find prev build")?;
                let next_build = *self.builds.get(to).context("can't find next build")?;
                let idx = self.graph.add_edge(prev_build, next_build, patch);
                e.insert(idx);
                self.graph
                    .edge_weight_mut(idx)
                    .context("`patches` points to existing EdgeIndex")?
            }
        };

        match location {
            Location::Local => {
                patch.set_local(entry);
            }
            Location::Remote => {
                patch.set_remote(entry);
            }
        }

        Ok(())
    }

    pub(crate) fn has_build(&self, v: Version) -> bool {
        self.builds.contains_key(&v)
    }

    pub(crate) fn has_patch(&self, from: Version, to: Version) -> bool {
        self.patches.contains_key(&(from, to))
    }

    fn patches_needed(&self, from: Version, to: Version) -> Result<(u64, Vec<String>)> {
        let from_idx = *self.builds.get(&from).context("unknown `from` version")?;
        let to_idx = *self.builds.get(&to).context("unknown `to` version")?;

        let (cost, steps) = petgraph::algo::astar(
            &self.graph,
            from_idx,
            |f| f == to_idx,
            |edge| edge.weight().size(),
            |_| 0,
        )
        .with_context(|| format!("no A& solution for patch from `{:?}` to `{:?}`", from, to))?;
        let mut path: Vec<String> = steps
            .windows(2)
            .map(|x| {
                let from = self.graph[x[0]].version.clone();
                let to = self.graph[x[1]].version.clone();
                Patch::new(from, to).to_string()
            })
            .collect();
        path.sort();

        Ok((cost, path))
    }

    #[allow(unused)]
    pub fn find_upgrade_path(&self, from: Version, to: Version) -> Result<UpgradePath> {
        let next_build = *self
            .builds
            .get(&to)
            .with_context(|| format!("unknown build size for `{:?}`", to))?;
        let build_size = self.graph[next_build].size();

        let res = self.patches_needed(from, to.clone()).map_err(|e| {
            log::debug!("{}", e);
            e
        });

        match res {
            Ok((size, path)) if build_size > size => Ok(UpgradePath::ApplyPatches(path)),
            _ => Ok(UpgradePath::InstallBuild(to.as_str().to_string())),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{test_helpers::*, Storage};

    #[test]
    fn this_is_fine() -> Result<()> {
        logger();

        let mut graph = PatchGraph::empty();
        graph.update_from_file_list(
            &[
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
            ],
            Location::Local,
        )?;
        dbg!(&graph);
        let installed_version = Version::try_from("1")?;
        let target_version = Version::try_from("3")?;

        let res = graph.find_upgrade_path(installed_version, target_version)?;

        assert_eq!(
            res,
            UpgradePath::ApplyPatches(vec![String::from("1-2.patch"), String::from("2-3.patch")])
        );

        Ok(())
    }

    #[test]
    fn this_is_also_ok() -> Result<()> {
        logger();

        let mut graph = PatchGraph::empty();
        graph.update_from_file_list(
            &[
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
            ],
            Location::Local,
        )?;
        let installed_version = Version::try_from("1")?;
        let target_version = Version::try_from("3")?;

        let res = graph.find_upgrade_path(installed_version, target_version)?;

        assert_eq!(res, UpgradePath::InstallBuild(String::from("3")));

        Ok(())
    }
}
