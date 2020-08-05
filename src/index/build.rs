use crate::{index::{Version, Checksum}, storage::Entry};

/// Artefact with version
#[derive(Debug, Clone, Eq, PartialOrd, Ord)]
pub struct Build {
    pub(crate) version: Version,
    pub(crate) local: Option<Entry>,
    pub(crate) remote: Option<Entry>,
    pub(crate) checksum: Option<Checksum>,
}

/// Builder
///
/// Sup dawg i herd u liked builds so I put a builder pattern in your build
/// module so u can build while u build
impl Build {
    pub fn new(version: Version) -> Self {
        Self {
            version,
            local: None,
            remote: None,
            checksum: None,
        }
    }

    pub fn set_local(&mut self, local: Entry) {
        self.local = Some(local);
    }

    pub fn set_remote(&mut self, remote: Entry) {
        self.remote = Some(remote);
    }

    pub fn set_checksum(&mut self, checksum: Checksum) {
        self.checksum = Some(checksum);
    }
}

impl Build {
    #[allow(unused)]
    pub fn size(&self) -> u64 {
        if let Some(entry) = self.local.as_ref().or_else(|| self.remote.as_ref()) {
            entry.size
        } else {
            panic!(
                "build `{:?}` has neither local not remote information!",
                self.version
            )
        }
    }
}

impl PartialEq for Build {
    fn eq(&self, other: &Build) -> bool {
        self.version == other.version
    }
}
