use crate::paths::{self, path_as_string};
use anyhow::{Context, Result};
pub use std::{
    convert::{TryFrom, TryInto},
    fs::read_dir,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};
use url::Url;

mod entry;
mod local;
mod s3;

pub use entry::Entry;

/// Storage abstraction
///
/// Cheap to clone, but immutable.
///
/// # Variants
///
/// - Local file system: Some directory on disk
/// - S3: An S3 bucket, identified by a URL
///
///   NOTE: For connecting to S3, the necessary credentials are read from env
///   variables by default. See [this page][1] for more details.
///
/// [1]: https://github.com/rusoto/rusoto/blob/e7ed8eabbb758bda4a857436ca572114de2bf283/AWS-CREDENTIALS.md
///
/// # Examples
///
/// ```rust
/// use std::convert::TryInto;
/// use artefacta::Storage;
///
/// let s3: Storage = "s3://my-bucket.ams3.digitaloceanspaces.com/test".parse().unwrap();
/// assert!(!s3.is_local());
///
/// let local_dir: Storage = std::env::current_dir().unwrap().try_into().unwrap();
/// assert!(local_dir.is_local());
/// assert!(local_dir.local_path().is_some());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Storage {
    inner: Arc<InnerStorage>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum InnerStorage {
    Filesystem(PathBuf),
    S3(s3::Bucket),
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

impl TryFrom<PathBuf> for Storage {
    type Error = anyhow::Error;

    fn try_from(path: PathBuf) -> Result<Self> {
        Storage::try_from(path.as_path())
    }
}

impl FromStr for Storage {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let path = PathBuf::from(s);
        if path.exists() {
            return Ok(InnerStorage::Filesystem(path).into());
        }

        let url = Url::from_str(s).with_context(|| format!("invalid URL `{}`", s))?;
        match url.scheme() {
            "s3" => Ok(InnerStorage::S3(
                s3::Bucket::try_from(&url)
                    .with_context(|| format!("convert `{}` to S3 bucket", url))?,
            )
            .into()),
            scheme => anyhow::bail!("unsupported protocol `{}`", scheme),
        }
    }
}

impl Storage {
    pub async fn list_files(&self) -> Result<Vec<Entry>> {
        match self.inner.as_ref() {
            InnerStorage::Filesystem(path) => read_dir(&path)
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
                .with_context(|| format!("parse directory content of `{}`", path.display())),
            InnerStorage::S3(bucket) => {
                use rusoto_s3::{ListObjectsV2Request, S3Client, S3};

                let list_obj_req = ListObjectsV2Request {
                    bucket: bucket.bucket.to_owned(),
                    ..Default::default()
                };
                let client: S3Client = bucket.try_into().context("build S3 client")?;

                let res = client
                    .list_objects_v2(list_obj_req)
                    .await
                    .context("list files in bucket")?;
                if res.is_truncated.unwrap_or_default() {
                    log::debug!("didn't get all the files -- pagination not implemented!");
                }

                res.contents
                    .context("got no entries when listing files")?
                    .iter()
                    .map(|obj| {
                        Ok(Entry {
                            storage: self.clone(),
                            path: obj.key.clone().context("got an object with no key")?,
                            size: obj
                                .size
                                .map(|s| s as u64)
                                .context("got an object with no size")?,
                        })
                    })
                    .collect::<Result<Vec<_>>>()
                    .context("parsing file list from S3")
            }
        }
    }

    pub async fn get_file(&self, path: &str) -> Result<File> {
        match self.inner.as_ref() {
            InnerStorage::Filesystem(root) => {
                let path = root.join(path);
                anyhow::ensure!(path.exists(), "Path `{}` does not exist", path.display());
                let size = path
                    .metadata()
                    .with_context(|| format!("read metadata of `{}`", path.display()))?
                    .len();

                Ok(File::InFilesystem(Entry {
                    storage: self.clone(),
                    path: path_as_string(path)?,
                    size,
                }))
            }
            InnerStorage::S3(bucket) => {
                use rusoto_s3::{GetObjectRequest, S3Client, S3};
                use tokio::io::AsyncReadExt;

                let key = bucket.key_for(path);
                let get_req = GetObjectRequest {
                    bucket: bucket.bucket.to_owned(),
                    key: key.clone(),
                    ..Default::default()
                };
                let client: S3Client = bucket.try_into().context("build S3 client")?;

                let result = client
                    .get_object(get_req)
                    .await
                    .with_context(|| format!("Couldn't get object with path `{}`", key))?;

                // TODO: Check this. Checksums are in format `{md5}[-{parts}]`.
                let checksum = result.e_tag.context("object has no checksum")?;

                let mut stream = result
                    .body
                    .context("object without body")?
                    .into_async_read();
                let mut body = Vec::new();
                stream
                    .read_to_end(&mut body)
                    .await
                    .context("read object content into buffer")?;

                let entry = Entry {
                    storage: self.clone(),
                    path: bucket.path.to_owned(),
                    size: result
                        .content_length
                        .map(|s| s as u64)
                        .context("got an object with no size")?,
                };

                Ok(File::Inline(entry, body.into_boxed_slice().into()))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum File {
    InFilesystem(Entry),
    Inline(Entry, Arc<[u8]>),
}

impl File {
    pub fn copy_to_local(self, storage: Storage) -> Result<Self> {
        todo!()
    }
}
