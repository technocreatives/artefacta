use crate::{index::Version, storage::Entry};

/// Artefact with version
#[derive(Debug, Clone, PartialEq)]
pub struct Build {
    pub(crate) version: Version,
    local: Option<Entry>,
    remote: Option<Entry>,
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
        }
    }

    pub fn set_local(&mut self, local: Entry) {
        self.local = Some(local);
    }

    pub fn set_remote(&mut self, remote: Entry) {
        self.remote = Some(remote);
    }
}

impl Build {
    pub fn size(&self) -> u64 {
        if let Some(entry) = self.local.as_ref().or(self.remote.as_ref()) {
            entry.size
        } else {
            panic!(
                "build `{:?}` has neither local not remote information!",
                self.version
            )
        }
    }
}
