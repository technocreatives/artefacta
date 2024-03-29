use crate::{
    apply_patch, paths,
    storage::{Entry, File as FileEntry, Storage},
    PartialFile,
};
use erreur::{bail, ensure, Context, Help, LogAndDiscardResult, Report, Result};
use std::{
    convert::TryFrom,
    fs::File,
    io::{self, BufReader, Cursor, Read},
    path::Path,
};

mod build;
pub use build::Build;
mod patch;
pub use patch::Patch;
mod graph;
pub use graph::{Location, PatchGraph, UpgradePath};
mod version;
pub use version::Version;

/// Artefact index
///
/// Contains local and remote storage as well as graph built from the current
/// contents of the storages.
///
/// This is the main entry point for interacting with any build and patch files.
#[derive(Debug, Clone)]
pub struct Index {
    local: Storage,
    remote: Storage,
    patch_graph: PatchGraph,
}

impl Index {
    /// Build index from directory content
    pub async fn new(local: impl AsRef<Path>, remote: Storage) -> Result<Self> {
        let local = Storage::try_from(local.as_ref())
            .context("invalid local storage path")
            .note("`mkdir -pv` is your friend")?;
        let mut patch_graph = PatchGraph::empty();
        patch_graph
            .update_from_file_list(
                &remote.list_files().await.context("list files")?,
                Location::Remote,
            )
            .with_context(|| format!("build patch graph from `{:?}`", remote))?;
        patch_graph
            .update_from_file_list(
                &local.list_files().await.context("list files")?,
                Location::Local,
            )
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

    pub async fn calculate_patch(&mut self, from: Version, to: Version) -> Result<()> {
        fn read_file(entry: Entry) -> Result<Vec<u8>> {
            ensure!(
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

        fn file_size(size: u64) -> String {
            use humansize::{file_size_opts as options, FileSize};
            size.file_size(options::BINARY).expect("never negative")
        }

        if self.get_patch(from.clone(), to.clone()).await.is_ok() {
            log::warn!(
                "asked to calculate patch from `{:?}` to `{:?}` but it's already present",
                from,
                to
            );
            return Ok(());
        }

        log::debug!("calculate path from `{}` to `{}`", from, to);

        let local = self
            .local
            .local_path()
            .context("calculate patch can only write to local storage right now")?;

        let old_build = self
            .get_build(from.clone())
            .await
            .context("get old build")?;
        let old_build = read_file(old_build).context("read old build")?;
        let old_build = crate::decompress(Cursor::new(old_build))?;

        let new_build = self.get_build(to.clone()).await.context("get new build")?;
        let new_build_size = new_build.size;
        let new_build = read_file(new_build).context("read new build")?;
        let new_build = crate::decompress(Cursor::new(new_build))?;

        let path_name = Patch::new(from.clone(), to.clone());
        // TODO: Fix that arbitrary "+ zst" here and everywhere else
        let patch_path = local.join(path_name.to_string() + ".zst");
        log::debug!("write patch {:?} to `{:?}`", path_name, patch_path);

        let mut patch_file =
            PartialFile::create(&patch_path).context("creating file to write patch to")?;
        let mut patch = crate::compress(&mut patch_file)?;
        bidiff::simple_diff_with_params(&old_build, &new_build, &mut patch, &{
            const MB: u64 = 1_000_000;
            bidiff::DiffParams::new(
                {
                    if new_build_size > (100 * MB) {
                        4
                    } else {
                        1
                    }
                },
                Some(100 * MB as usize),
            )
            .map_err(|e| Report::msg(e.to_string()))
            .context("valid diff params")
            .note("this is a programming error, please open an issue")?
        })
        .context("calculating binary diff between builds")?;
        patch.finish().context("finishing zstd file")?;
        patch_file
            .finish()
            .context("finishing writing patch file")?;

        let patch_size = patch_path
            .metadata()
            .with_context(|| {
                format!(
                    "can't read metadata for new patch file `{}`",
                    patch_path.display()
                )
            })?
            .len();

        let entry = Entry {
            storage: self.local.clone(),
            path: paths::path_as_string(patch_path)?,
            size: patch_size,
        };

        log::info!(
            "Calculated new patch from {} to {} of size {} -- that's {:.1}% of the new build's {}",
            from,
            to,
            file_size(patch_size),
            (patch_size as f64) / (new_build_size as f64) * 100_f64,
            file_size(new_build_size),
        );

        self.patch_graph
            .add_patch(&from, &to, entry, Location::Local)?;

        Ok(())
    }

    async fn get_local_file(&self, path: &str) -> Result<Entry> {
        let file = self.local.get_file(path).await.context("fetch local file");

        match file {
            Ok(FileEntry::InFilesystem(local)) => Ok(local),
            Ok(_) => unreachable!("local storage always returns local files"),
            Err(e) => Err(e).context("get local build"),
        }
    }

    pub async fn get_patch(&mut self, from: Version, to: Version) -> Result<Entry> {
        ensure!(
            self.patch_graph.has_patch(from.clone(), to.clone()),
            "patch `{:?}` unknown",
            (from, to)
        );

        let patch = Patch::new(from, to);
        let patch_name = patch.file_name();
        match self.get_local_file(&patch_name).await {
            Ok(local) => return Ok(local),
            Err(e) => log::debug!("could not get patch {:?} locally: {}", patch, e),
        }

        let remote_entry = self
            .remote
            .get_file(&patch_name)
            .await
            .with_context(|| format!("can't find `{}` either locally or remotely", patch))?;

        self.add_patch(&remote_entry)
            .await
            .context("copy remote entry to local storage")?;
        log::debug!("fetched patch `{}` from remote ({:?})", patch, remote_entry);

        self.get_local_file(&patch_name)
            .await
            .context("fetch newly added local path")
    }

    /// Upgrade from one version to the next
    pub async fn upgrade_to_build(&mut self, from: Version, to: Version) -> Result<Entry> {
        log::debug!("searching for upgrade path from `{}` to `{}`", from, to);
        ensure!(
            self.patch_graph.has_build(from.clone()),
            "build `{:?}` unknown",
            from
        );
        ensure!(
            self.patch_graph.has_build(to.clone()),
            "build `{:?}` unknown",
            to
        );

        match self
            .patch_graph
            .find_upgrade_path(from.clone(), to.clone())
            .with_context(|| format!("can't find upgrade path from `{:?}` to `{:?}", from, to))?
        {
            UpgradePath::ApplyPatches(patches) => {
                log::debug!("found upgrade path via patches: {:?}", patches);
                let needed_patches = patches
                    .into_iter()
                    .skip_while(|patch| self.patch_graph.has_local_build(patch.to.clone()))
                    .collect::<Vec<Patch>>();
                log::debug!(
                    "using already existing local builds, we need to fetch: {:?}",
                    needed_patches
                );

                async fn apply_patches(index: &mut Index, needed_patches: &[Patch]) -> Result<()> {
                    for patch in needed_patches {
                        index
                            .add_build_from_patch(patch)
                            .await
                            .with_context(|| format!("add build from patch `{:?}`", patch))?;
                    }
                    Ok(())
                }

                match apply_patches(self, &needed_patches).await {
                    Ok(_) => log::debug!("successfully applied all patches to get to final build."),
                    e => {
                        log::warn!("failed to get build using patches, will use direct build.");
                        e.note("one of the patches might be corrupt.")
                            .log_and_discard();
                    }
                }

                let local_build = self.get_build(to).await.context("fetch target build")?;
                log::debug!("arrived at final build: {:?}", local_build);

                Ok(local_build)
            }
            UpgradePath::InstallBuild(build) => {
                log::debug!("found upgrade path installing build `{:?}`", build);
                let local_build = self.get_build(to).await.context("install fresh build")?;
                Ok(local_build)
            }
        }
    }

    async fn add_build_from_patch(&mut self, patch: &Patch) -> Result<Entry> {
        let patch_file = self
            .get_patch(patch.from.clone(), patch.to.clone())
            .await
            .context("fetch patch")?;
        let source_build = self
            .get_build(patch.from.clone())
            .await
            .context("fetch source build")?;

        let build_name = format!("{}.tar.zst", patch.to);
        let build_root = self.local.local_path().context("local storage not local")?;
        let build_path = build_root.join(&build_name);

        let mut build_file = PartialFile::create(&build_path)
            .with_context(|| format!("create new build file `{}`", build_path.display()))?;
        let mut build_writer =
            crate::compress(&mut build_file).context("zstd writer for new build")?;
        let mut patch_data =
            apply_patch(&source_build.path, &patch_file.path).context("apply patch")?;

        io::copy(&mut patch_data, &mut build_writer).context("write patch")?;
        build_writer.finish().context("finish zstd writer")?;
        build_file.finish().context("finish build file")?;

        let entry = Entry::from_path(&build_path, self.local.clone())
            .context("create entry for new build file")?;
        log::debug!(
            "created new build `{:?}` from patch `{:?}`",
            entry,
            patch_file
        );

        self.patch_graph
            .add_build(&patch.to, entry.clone(), Location::Local)
            .with_context(|| {
                format!(
                    "add newly created build `{}` to index",
                    build_path.display()
                )
            })?;
        Ok(entry)
    }

    /// Get build (adds to local cache if not present)
    pub async fn get_build(&mut self, version: Version) -> Result<Entry> {
        ensure!(
            self.patch_graph.has_build(version.clone()),
            "build `{:?}` unknown",
            version
        );

        let build_path = paths::build_path_from_version(version.clone())?;
        match self.get_local_file(&build_path).await {
            Ok(local) => {
                log::debug!("using local file for build `{:?}`", local);

                // quick sanity check
                if let Some(remote) = self.patch_graph.remote_build(version.clone()) {
                    if local.size != remote.size {
                        log::warn!(
                            "Using locally cached file for `{}` but size on remote differs",
                            version
                        );
                    }
                }

                return Ok(local);
            }
            Err(e) => log::debug!(
                "could not get local patch {:?} ({}), trying remote next",
                build_path,
                e
            ),
        }

        let remote_entry = self.remote.get_file(&build_path).await.with_context(|| {
            format!(
                "can't find `{}` either locally or remotely",
                version.as_str()
            )
        })?;

        self.add_build(&remote_entry)
            .await
            .context("copy remote entry to local storage")?;
        self.get_local_file(&build_path)
            .await
            .context("fetch newly added local build")
    }

    pub fn get_build_for_tag(&self, tag: &str) -> Result<Version> {
        let parsed_tag = crate::git::tag_to_slice(tag);
        self.patch_graph
            .builds
            .keys()
            .find(|build| crate::git::tag_to_slice(build.as_str()) == parsed_tag)
            .cloned()
            .with_context(|| format!("no build found matching tag `{}`", tag))
    }

    pub async fn add_local_build(&mut self, path: impl AsRef<Path>) -> Result<Entry> {
        let entry = Entry::from_path(path.as_ref(), self.local.clone())
            .context("local build file as entry")?;
        self.add_build(&FileEntry::InFilesystem(entry))
            .await
            .context("add local build file")
    }

    /// Add build to graph and copy it into index's root directory
    pub(crate) async fn add_build(&mut self, file: &FileEntry) -> Result<Entry> {
        let local = self
            .local
            .local_path()
            .context("add_build can only write to local storage right now")?;

        let path = match file {
            FileEntry::InFilesystem(entry) => {
                let path = Path::new(&entry.path);
                ensure!(
                    !path.starts_with(&local),
                    "asked to add patch from same directory it would be written to"
                );
                path.canonicalize()
                    .with_context(|| format!("canonicalize {}", path.display()))?
            }
            FileEntry::Inline(entry, ..) => Path::new(&entry.path).to_path_buf(),
        };

        let file_name = paths::file_name(&path)?;
        let version: Version = file_name.parse()?;
        let new_path = local.join(format!("{}.tar.zst", version.as_str()));

        self.local
            .add_file(file, &new_path)
            .await
            .context("write build file to local storage")?;

        let entry = Entry::from_path(&new_path, self.local.clone())
            .context("create entry for new build file")?;

        ensure!(
            entry.size > 0,
            "Just added `{}` but it's empty (size 0). That's not gonna be useful.",
            entry.path
        );

        self.patch_graph
            .add_build(&version, entry.clone(), Location::Local)
            .with_context(|| format!("add build `{}`", path.display()))?;
        Ok(entry)
    }

    /// Add build to graph and copy it into index's root directory
    ///
    /// TODO: Refactor this and add_build to be the same generic method
    pub(crate) async fn add_patch(&mut self, file: &FileEntry) -> Result<()> {
        let local = self
            .local
            .local_path()
            .context("add_patch can only write to local storage right now")?;
        let path = match file {
            FileEntry::InFilesystem(entry) => {
                let path = Path::new(&entry.path);
                ensure!(
                    !path.starts_with(&local),
                    "asked to add patch from same directory it would be written to"
                );
                path.canonicalize()
                    .with_context(|| format!("canonicalize {}", path.display()))?
            }
            FileEntry::Inline(entry, ..) => Path::new(&entry.path).to_path_buf(),
        };

        let patch = Patch::from_path(&path)?;
        let new_path = local.join(patch.file_name());

        self.local
            .add_file(file, &new_path)
            .await
            .context("write patch file to local storage")?;
        log::trace!("added file `{}` to local storage", new_path.display());

        let entry = Entry::from_path(&new_path, self.local.clone())
            .context("create entry for new build file")?;

        self.patch_graph
            .add_patch(&patch.from, &patch.to, entry, Location::Local)
            .with_context(|| format!("add patch `{}`", path.display()))?;
        log::debug!("added patch `{}`: {:?}", path.display(), patch);
        Ok(())
    }

    // Fetch current state from S3 and upload all missing files (i.e. new builds
    // and patches)
    pub async fn push(&self) -> Result<()> {
        use futures::stream::{self, StreamExt, TryStreamExt};

        let builds = self
            .patch_graph
            .local_only_builds()
            .into_iter()
            .map(|b| {
                if let Some(local) = b.local {
                    Ok(local)
                } else {
                    bail!("no local entry in `{:?}`", b)
                }
            })
            .collect::<Result<Vec<Entry>>>()
            .context("collecting builds to upload")?;
        log::debug!(
            "found {} builds locally that are not on remote",
            builds.len()
        );
        let builds = stream::iter(builds);

        let patches = self
            .patch_graph
            .local_only_patches()
            .into_iter()
            .map(|b| {
                if let Some(local) = b.local {
                    Ok(local)
                } else {
                    bail!("no local entry in `{:?}`", b)
                }
            })
            .collect::<Result<Vec<Entry>>>()
            .context("collecting patches to upload")?;
        log::debug!(
            "found {} patches locally that are not on remote",
            patches.len()
        );
        let patches = stream::iter(patches);

        builds
            .chain(patches)
            .map(|x| -> Result<Entry> { Ok(x) }) // necessary for fallible method and type inference
            .try_for_each_concurrent(3, |entry| async {
                let s3_key = entry
                    .path
                    .rsplit('/')
                    .next()
                    .expect("always one item in split")
                    .to_owned();
                self.remote
                    .add_file(&FileEntry::InFilesystem(entry), &s3_key)
                    .await
                    .with_context(|| format!("adding `{}`", s3_key))?;
                log::info!("uploaded `{}`", s3_key);
                Ok(())
            })
            .await
            .context("uploading missing files to remote")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use std::convert::TryInto;

    // TODO: Add same but with one the builds only available on remote
    #[tokio::test]
    async fn create_patch() -> Result<()> {
        let local_dir = tempdir()?;
        let remote_dir = tempdir()?;

        // Add some builds
        let _build1 = random_zstd_file(local_dir.path().join("build1.tar.zst"))?;
        let _build2 = random_zstd_file(local_dir.path().join("build2.tar.zst"))?;
        let _build3 = random_zstd_file(local_dir.path().join("build3.tar.zst"))?;

        let mut index = Index::new(local_dir.path(), remote_dir.path().try_into()?).await?;

        index
            .calculate_patch("build2".parse()?, "build3".parse()?)
            .await?;

        index
            .get_patch("build2".parse()?, "build3".parse()?)
            .await?;

        Ok(())
    }

    #[tokio::test]
    async fn generate_patches() -> Result<()> {
        let dir = test_dir(&["1.tar.zst", "2.tar.zst", "1-2.patch.zst"])?;
        let remote_dir = test_dir(&["3.tar.zst"])?;

        let mut index = Index::new(&dir.path(), remote_dir.path().try_into()?).await?;
        let build1 = FileEntry::InFilesystem(Entry::from_path(
            remote_dir.path().join("3.tar.zst"),
            index.local.clone(),
        )?);
        index.add_build(&build1).await?;

        assert!(
            index.get_build("3".parse()?).await.is_ok(),
            "didn't add build to index {:?}",
            index
        );

        index
            .calculate_patch("2".parse()?, "3".parse()?)
            .await
            .context("calc patches")?;

        dbg!(&index);

        index.get_patch("2".parse()?, "3".parse()?).await?;

        Ok(())
    }

    fn test_dir(files: &[&str]) -> Result<TempDir> {
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
