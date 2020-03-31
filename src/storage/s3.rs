use anyhow::{Context, Result};
use rusoto_core::{Region, RusotoError};
use rusoto_s3::S3Client;
use std::{collections::HashMap, convert::TryFrom};
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Bucket {
    pub endpoint: String,
    pub bucket: String,
    pub path: String,
}

impl Bucket {
    /// Get S3 key for file path.
    ///
    /// Takes into account the root path in the bucket as well as normalizes the
    /// path.
    pub fn key_for(&self, path: &str) -> String {
        let mut root = self
            .path
            .trim_start_matches('/')
            .trim_end_matches('/')
            .to_owned();
        root.push_str("/");
        root.push_str(path);
        root
    }
}

impl<'a> TryFrom<&'a Url> for Bucket {
    type Error = anyhow::Error;

    fn try_from(url: &Url) -> Result<Bucket> {
        anyhow::ensure!(url.scheme() == "s3", "URI scheme has to be `s3`");
        let host = url
            .host_str()
            .context("S3 URI needs to contain a full host name")?;
        let mut host_parts = host.splitn(2, '.');
        let (bucket, endpoint) = (
            host_parts.next().context("read bucket name")?.to_owned(),
            host_parts.next().context("read endpoint")?.to_owned(),
        );

        let path = url.path().to_owned();

        Ok(Bucket {
            endpoint,
            bucket,
            path,
        })
    }
}

#[test]
fn bucket_config_from_url() {
    let url = Url::parse("s3://nevs-artefacts.ams3.digitaloceanspaces.com/test").unwrap();
    let bucket = Bucket::try_from(&url).unwrap();
    assert_eq!(
        bucket,
        Bucket {
            endpoint: "ams3.digitaloceanspaces.com".into(),
            bucket: "nevs-artefacts".into(),
            path: "/test".into(),
        }
    );
}

impl<'a> TryFrom<&'a Bucket> for S3Client {
    type Error = anyhow::Error;

    fn try_from(bucket: &'a Bucket) -> Result<S3Client> {
        let region = Region::Custom {
            name: "custom-region".to_owned(),
            endpoint: bucket.endpoint.clone(),
        };

        Ok(S3Client::new(region))
    }
}
