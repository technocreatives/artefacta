use crate::{paths, Storage};
use anyhow::{Context, Result};
use std::{fmt, path::Path};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Entry {
    pub storage: Storage,
    pub path: String,
    pub size: u64,
}

impl Entry {
    pub fn from_path(path: impl AsRef<Path>, storage: Storage) -> Result<Self> {
        let path = path.as_ref();
        let path = path
            .canonicalize()
            .with_context(|| format!("cannot canonicalize path `{}`", path.display()))?;

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

impl fmt::Debug for Entry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use humansize::{file_size_opts as options, FileSize};

        f.debug_tuple("Entry")
            .field(&self.storage)
            .field(&self.path)
            .field(&format_args!(
                "{}",
                self.size
                    .file_size(options::BINARY)
                    .expect("never negative")
            ))
            .finish()
    }
}
