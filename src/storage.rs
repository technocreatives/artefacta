use crate::paths::path_as_string;
use anyhow::{Context, Result};
pub use std::{
    fs::read_dir,
    path::{Path, PathBuf},
    str::FromStr,
};
use url::Url;

#[derive(Debug, Clone)]
pub enum Storage {
    Filesystem(PathBuf),
    S3(Url),
}

impl FromStr for Storage {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let path = PathBuf::from(s);
        if path.exists() {
            return Ok(Storage::Filesystem(path));
        }

        let url = Url::from_str(s).context("invalid URL")?;
        match url.scheme() {
            "s3" => Ok(Storage::S3(url)),
            scheme => anyhow::bail!("unsupported protoco `{}`", scheme),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub path: String,
    pub size: u64,
}

impl Storage {
    pub fn list_files(&self) -> Result<Vec<Entry>> {
        let path = if let Storage::Filesystem(p) = self {
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
                    path: path_as_string(path)?,
                    size,
                })
            })
            .collect::<Result<Vec<_>>>()
            .with_context(|| format!("parse directory content of `{}`", path.display()))
    }
}
