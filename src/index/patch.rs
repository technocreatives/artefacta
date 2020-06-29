use crate::{index::Version, paths::file_name, storage::Entry};
use erreur::{bail, Context, Result};
use std::{convert::TryFrom, fmt, path::Path};

/// Patch from old to new build
#[derive(Debug, Clone, Eq, PartialOrd, Ord)]
pub struct Patch {
    pub(crate) from: Version,
    pub(crate) to: Version,
    pub(crate) local: Option<Entry>,
    pub(crate) remote: Option<Entry>,
}

/// Builder
impl Patch {
    pub fn new(from: Version, to: Version) -> Self {
        Self {
            from,
            to,
            local: None,
            remote: None,
        }
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let (from, to) = patch_versions_from_path(path).context("constructing patch from path")?;
        Ok(Self::new(from, to))
    }

    pub fn set_local(&mut self, local: Entry) {
        self.local = Some(local);
    }

    pub fn set_remote(&mut self, remote: Entry) {
        self.remote = Some(remote);
    }
}

impl Patch {
    pub fn size(&self) -> u64 {
        if let Some(entry) = self.local.as_ref().or_else(|| self.remote.as_ref()) {
            entry.size
        } else {
            panic!("patch `{}` has neither local not remote information!", self)
        }
    }
}

impl fmt::Display for Patch {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.from.as_str().contains('-') || self.to.as_str().contains('-') {
            write!(f, "{}---{}.patch", self.from.as_str(), self.to.as_str())
        } else {
            write!(f, "{}-{}.patch", self.from.as_str(), self.to.as_str())
        }
    }
}

impl PartialEq for Patch {
    fn eq(&self, other: &Patch) -> bool {
        self.from == other.from && self.to == other.to
    }
}

fn patch_versions_from_path(path: impl AsRef<Path>) -> Result<(Version, Version)> {
    let path = path.as_ref();
    let name = file_name(path).with_context(|| format!("get name of `{:?}`", path))?;

    let parts: Vec<&str> = name.split('-').collect();
    if parts.len() == 2 {
        // patch file name pattern is assumed to be `<hash>-<hash>`
        return Version::try_from(parts[0])
            .into_iter()
            .zip(Version::try_from(parts[1]))
            .next()
            .with_context(|| format!("parse name `{}` from path `{:?}` as version", name, path));
    }

    let parts: Vec<&str> = name.split("---").collect();
    if parts.len() == 2 {
        // patch file name pattern is assumed to be `<complex-name>---<complex-name>`
        return Version::try_from(parts[0])
            .into_iter()
            .zip(Version::try_from(parts[1]))
            .next()
            .with_context(|| format!("parse name `{}` from path `{:?}` as version", name, path));
    }

    bail!(
        "path `{}` cannot be parsed as patch file name pattern with 2 version",
        path.display()
    );
}

#[test]
fn parsing_weird_patch_names() {
    assert_patch_names("foo/bar/build1-build2.tar.zst", "build1", "build2");
    assert_patch_names(
        "foo/bar/module-v1.2.3---module-v1.2.4.tar.zst",
        "module-v1.2.3",
        "module-v1.2.4",
    );

    fn assert_patch_names(path: &str, from: &str, to: &str) {
        let from = Version::try_from(from).unwrap();
        let to = Version::try_from(to).unwrap();
        let parsed = patch_versions_from_path(path).unwrap();
        assert_eq!(parsed, (from, to));
    }
}
