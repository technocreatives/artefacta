use crate::paths::path_as_string;
use anyhow::{Context, Result};
pub use std::{
    convert::TryFrom,
    fs::read_dir,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};
use url::Url;

/// Storage abstraction
///
/// Cheap to clone, but immutable.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Storage {
    inner: Arc<InnerStorage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum InnerStorage {
    Filesystem(PathBuf),
    S3(Url),
}

impl From<InnerStorage> for Storage {
    fn from(inner: InnerStorage) -> Self {
        Storage {
            inner: Arc::new(inner),
        }
    }
}

impl<'p> TryFrom<&'p Path> for Storage {
    type Error = anyhow::Error;

    fn try_from(path: &Path) -> Result<Self> {
        anyhow::ensure!(path.exists(), "Path `{}` does not exist", path.display());
        Ok(InnerStorage::Filesystem(path.to_path_buf()).into())
    }
}

impl FromStr for Storage {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let path = PathBuf::from(s);
        if path.exists() {
            return Ok(InnerStorage::Filesystem(path).into());
        }

        let url = Url::from_str(s).context("invalid URL")?;
        match url.scheme() {
            "s3" => Ok(InnerStorage::S3(url).into()),
            scheme => anyhow::bail!("unsupported protoco `{}`", scheme),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub storage: Storage,
    pub path: String,
    pub size: u64,
}

impl Storage {
    pub fn is_local(&self) -> bool {
        match *self.inner {
            InnerStorage::Filesystem(_) => true,
            _ => false,
        }
    }

    pub fn local_path(&self) -> Option<PathBuf> {
        match *self.inner {
            InnerStorage::Filesystem(ref p) => Some(p.clone()),
            _ => None,
        }
    }

    pub fn list_files(&self) -> Result<Vec<Entry>> {
        let path = if let InnerStorage::Filesystem(ref p) = *self.inner {
            p
        } else {
            unimplemented!("list files only implemented for local fs")
        };

        read_dir(&path)
            .with_context(|| format!("could not read directory `{}`", path.display()))?
            .map(|entry| {
                let entry = entry.context("read file entry")?;
                let path = entry.path();
                let size = entry
                    .metadata()
                    .with_context(|| format!("read metadata of `{}`", path.display()))?
                    .len();
                Ok(Entry {
                    storage: self.clone(),
                    path: path_as_string(path)?,
                    size,
                })
            })
            .collect::<Result<Vec<_>>>()
            .with_context(|| format!("parse directory content of `{}`", path.display()))
    }

    pub fn get_file(&self, path: &str) -> Result<Entry> {
        match self.inner.as_ref() {
            InnerStorage::Filesystem(root) => {
                let path = root.join(path);
                anyhow::ensure!(path.exists(), "Path `{}` does not exist", path.display());
                let size = path
                    .metadata()
                    .with_context(|| format!("read metadata of `{}`", path.display()))?
                    .len();

                Ok(Entry {
                    storage: self.clone(),
                    path: path_as_string(path)?,
                    size,
                })
            }
            InnerStorage::S3(..) => todo!("get_file not implemented for S3 yet"),
        }
    }
}
