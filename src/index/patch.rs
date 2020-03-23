use crate::{index::Version, storage::Entry};
use std::fmt;

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
        write!(f, "{}-{}.patch", self.from.as_str(), self.to.as_str())
    }
}

impl PartialEq for Patch {
    fn eq(&self, other: &Patch) -> bool {
        self.from == other.from && self.to == other.to
    }
}
