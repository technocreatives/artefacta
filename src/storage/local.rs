use super::{InnerStorage, Storage};
use std::path::PathBuf;

impl Storage {
    pub fn is_local(&self) -> bool {
        matches!(*self.inner, InnerStorage::Filesystem(_))
    }

    pub fn local_path(&self) -> Option<PathBuf> {
        match *self.inner {
            InnerStorage::Filesystem(ref p) => Some(p.clone()),
            _ => None,
        }
    }
}
