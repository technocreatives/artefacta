use crate::paths::path_as_string;
use anyhow::{Context, Result};
pub use std::{
    convert::{TryFrom, TryInto},
    fmt,
    fs::{self, read_dir},
    io::{BufWriter, Write},
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
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Storage {
    inner: Arc<InnerStorage>,
}

impl fmt::Display for Storage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.inner.as_ref() {
            InnerStorage::Filesystem(root) => write!(f, "filesystem (`{}`)", root.display()),
            InnerStorage::S3(b) => write!(f, "S3 ({})", b.bucket),
        }
    }
}

impl fmt::Debug for Storage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.inner.as_ref() {
            InnerStorage::Filesystem(root) => {
                f.debug_tuple("Filesystem").field(root).finish()?;
            }
            InnerStorage::S3(b) => {
                f.debug_tuple("S3")
                    .field(&b.endpoint)
                    .field(&b.path)
                    .finish()?;
            }
        }
        Ok(())
    }
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
        let path = path
            .canonicalize()
            .with_context(|| format!("cannot canonicalize path `{}`", path.display()))?;
        Ok(InnerStorage::Filesystem(path).into())
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
            InnerStorage::Filesystem(path) => Ok(read_dir(&path)
                .with_context(|| format!("could not read directory `{}`", path.display()))?
                .map(|entry| -> Result<_> {
                    let entry = entry.context("could not read file entry")?;
                    let path = entry.path();
                    let path = path.canonicalize().with_context(|| {
                        format!("cannot canonicalize path `{}`", path.display())
                    })?;
                    let metadata = entry.metadata().with_context(|| {
                        format!("could not read metadata of `{}`", path.display())
                    })?;

                    Ok((metadata, path_as_string(path)?))
                })
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .filter(|(metadata, _)| !metadata.file_type().is_symlink())
                .map(|(metadata, path)| Entry {
                    storage: self.clone(),
                    path,
                    size: metadata.len(),
                })
                .collect::<Vec<_>>()),
            InnerStorage::S3(bucket) => {
                use rusoto_s3::{ListObjectsV2Request, S3Client, S3};

                let client: S3Client = bucket.try_into().context("build S3 client")?;

                let res = client
                    .list_objects_v2(ListObjectsV2Request {
                        bucket: bucket.bucket.to_owned(),
                        prefix: Some(bucket.path.trim_start_matches('/').to_string()),
                        ..Default::default()
                    })
                    .await
                    .context("list files in bucket")?;
                if res.is_truncated.unwrap_or_default() {
                    log::debug!("didn't get all the files -- pagination not implemented!");
                }

                res.contents
                    .unwrap_or_default()
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
                let client: S3Client = bucket.try_into().context("build S3 client")?;

                let result = client
                    .get_object(GetObjectRequest {
                        bucket: bucket.bucket.to_owned(),
                        key: key.clone(),
                        ..Default::default()
                    })
                    .await
                    .with_context(|| format!("Couldn't get object with path `{}`", key))?;

                // TODO: Check this. Checksums are in format `{md5}[-{parts}]`.
                let _checksum = result.e_tag.context("object has no checksum")?;

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
                    path: key.to_owned(),
                    size: result
                        .content_length
                        .map(|s| s as u64)
                        .context("got an object with no size")?,
                };

                Ok(File::Inline(entry, body.into_boxed_slice().into()))
            }
        }
    }

    pub async fn add_file(&self, file: &File, target: impl AsRef<Path>) -> Result<()> {
        log::debug!("adding file {:?} to `{}`", file, self);
        let target = target.as_ref();

        match self.inner.as_ref() {
            InnerStorage::Filesystem(root) => {
                let new_path = if target.is_absolute() {
                    anyhow::ensure!(
                        target.starts_with(&root),
                        "build target path is absolute but not in storage directory"
                    );

                    target.to_path_buf()
                } else {
                    root.join(target)
                };

                match file {
                    File::InFilesystem(entry) => {
                        fs::copy(&entry.path, &new_path).with_context(|| {
                            format!("copy `{}` to `{}`", entry.path, new_path.display())
                        })?;
                    }
                    File::Inline(_, content) => {
                        let new = fs::File::create(&new_path)
                            .with_context(|| format!("create `{}`", new_path.display()))?;
                        let mut new = BufWriter::new(new);
                        new.write_all(&content).context("write content of file")?;
                    }
                };
            }

            InnerStorage::S3(bucket) => {
                use rusoto_s3::{PutObjectRequest, S3Client, S3};

                let client: S3Client = bucket.try_into().context("build S3 client")?;

                let content = match file {
                    File::InFilesystem(entry) => fs::read(&entry.path)
                        .with_context(|| format!("could not read `{}`", entry.path))?,
                    File::Inline(_, content) => content.to_vec(),
                };

                let key = bucket.key_for(&path_as_string(target)?);
                log::debug!("adding file as `{}`", key);
                let checksum = md5::compute(&content);
                client
                    .put_object(PutObjectRequest {
                        bucket: bucket.bucket.to_owned(),
                        key: key.clone(),
                        content_md5: Some(base64::encode(&*checksum)),
                        body: Some(content.into()),
                        ..Default::default()
                    })
                    .await
                    .with_context(|| format!("Failed to upload object `{}` to S3", key))?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum File {
    InFilesystem(Entry),
    Inline(Entry, Arc<[u8]>),
}

impl File {
    pub fn copy_to_local(self, _storage: Storage) -> Result<Self> {
        todo!()
    }
}

impl fmt::Debug for File {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            File::InFilesystem(e) => f.debug_tuple("File").field(e).finish(),
            File::Inline(e, _) => f
                .debug_tuple("InlineFile")
                .field(e)
                .field(&format_args!("[bytes]"))
                .finish(),
        }
    }
}
