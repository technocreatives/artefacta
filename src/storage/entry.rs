use crate::{paths, Storage};
use anyhow::{Context, Result};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Entry {
    pub storage: Storage,
    pub path: String,
    pub size: u64,
}

impl Entry {
    pub fn from_path(path: impl AsRef<Path>, storage: Storage) -> Result<Self> {
        let path = path.as_ref();
        let size = path
            .metadata()
            .with_context(|| {
                format!(
                    "can't read metadata for new build file `{}`",
                    path.display()
                )
            })?
            .len();

        Ok(Entry {
            storage,
            path: paths::path_as_string(path)?,
            size,
        })
    }
}
