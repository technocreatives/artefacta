use super::{Build, Patch, Version};
use crate::{paths, storage::Entry};
use erreur::{Context, Help, Result, StdResult};

use petgraph::graph::{DefaultIx, EdgeIndex, Graph, NodeIndex};
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
    pub(crate) builds: HashMap<Version, NodeIndex<DefaultIx>>,
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
        let builds: Vec<_> = list
            .iter()
            .filter(|entry| entry.path.ends_with(".tar.zst"))
            .collect();
        let patches: Vec<_> = list
            .iter()
            .filter(|entry| entry.path.ends_with(".patch.zst"))
            .collect();

        log::trace!("Builds: {:?}", builds);
        for entry in builds {
            if entry.path.ends_with('/') {
                continue;
            }
            let version = paths::build_version_from_path(&entry.path)?;
            self.add_build(&version, entry.clone(), location)
                .with_context(|| format!("add build `{}`", entry.path))?;
        }

        log::trace!("Patches: {:?}", patches);
        for entry in patches {
            if entry.path.ends_with('/') {
                continue;
            }
            let Patch { from, to, .. } = Patch::from_path(&entry.path)?;
            if let Err(e) = self.add_patch(&from, &to, entry.clone(), location) {
                log::error!("failed to add patch `{}`. continuing.", entry.path);
                if log::log_enabled!(log::Level::Debug) {
                    format!("{:?}", e)
                        .lines()
                        .filter(|l| !l.is_empty())
                        .for_each(|l| log::debug!("{}", l));
                }
            }
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
            Entry::Occupied(e) => {
                log::trace!(
                    "graph already has patch {:?}, updating weight only",
                    (from.clone(), to.clone())
                );
                self.graph
                    .edge_weight_mut(*e.get())
                    .context("`patches` points to non-existing EdgeIndex")?
            }
            Entry::Vacant(e) => {
                let patch = Patch::new(from.clone(), to.clone());
                let prev_build = *self
                    .builds
                    .get(from)
                    .with_context(|| format!("can't find prev build `{}` of `{}`", from, to))
                    .note("do your file names follow the pattern artefacta expects?")?;
                let next_build = *self
                    .builds
                    .get(to)
                    .with_context(|| format!("can't find next build `{}` of `{}`", to, from))
                    .note("do your file names follow the pattern artefacta expects?")?;
                let idx = self.graph.add_edge(prev_build, next_build, patch);
                e.insert(idx);
                log::trace!("added new edge/patch {:?}", (from.clone(), to.clone()));
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
        log::trace!("patch updated to be {:?}", patch);

        Ok(())
    }

    pub(crate) fn has_build(&self, v: Version) -> bool {
        self.builds.contains_key(&v)
    }

    pub(crate) fn local_build(&self, v: Version) -> Option<&Entry> {
        let build_idx = self.builds.get(&v)?;
        let build = self.graph.node_weight(*build_idx)?;
        build.local.as_ref()
    }

    pub(crate) fn remote_build(&self, v: Version) -> Option<&Entry> {
        let build_idx = self.builds.get(&v)?;
        let build = self.graph.node_weight(*build_idx)?;
        build.remote.as_ref()
    }

    pub(crate) fn has_local_build(&self, v: Version) -> bool {
        self.local_build(v).is_some()
    }

    pub(crate) fn has_patch(&self, from: Version, to: Version) -> bool {
        self.patches.contains_key(&(from, to))
    }

    fn patches_needed(&self, from: Version, to: Version) -> Result<(u64, Vec<Patch>)> {
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
        let mut path: Vec<_> = steps
            .windows(2)
            .map(|x| {
                let from = self.graph[x[0]].version.clone();
                let to = self.graph[x[1]].version.clone();
                Patch::new(from, to)
            })
            .collect();
        path.sort();

        Ok((cost, path))
    }

    pub fn find_upgrade_path(&self, from: Version, to: Version) -> Result<UpgradePath> {
        let next_build_idx = *self
            .builds
            .get(&to)
            .with_context(|| format!("unknown build size for `{:?}`", to))?;
        let next_build = self.graph[next_build_idx].clone();
        let build_size = next_build.size();

        let res = self.patches_needed(from, to).map_err(|e| {
            log::debug!("{}", e);
            e
        });

        match res {
            Ok((size, path)) if build_size > size => Ok(UpgradePath::ApplyPatches(path)),
            _ => Ok(UpgradePath::InstallBuild(next_build)),
        }
    }

    pub(crate) fn local_only_builds(&self) -> Vec<Build> {
        self.graph
            .raw_nodes()
            .iter()
            .map(|n| &n.weight)
            .filter(|b| b.remote.is_none())
            .cloned()
            .collect()
    }

    pub(crate) fn local_only_patches(&self) -> Vec<Patch> {
        self.graph
            .raw_edges()
            .iter()
            .map(|n| &n.weight)
            .filter(|b| b.remote.is_none())
            .cloned()
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpgradePath {
    ApplyPatches(Vec<Patch>),
    InstallBuild(Build),
}

impl TryFrom<ReadDir> for PatchGraph {
    type Error = IoError;

    fn try_from(_x: ReadDir) -> StdResult<Self, IoError> {
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
            UpgradePath::ApplyPatches(vec![
                Patch::new("1".parse()?, "2".parse()?),
                Patch::new("2".parse()?, "3".parse()?),
            ])
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

        assert_eq!(res, UpgradePath::InstallBuild(Build::new("3".parse()?)));

        Ok(())
    }
}
