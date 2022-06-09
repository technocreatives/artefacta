use erreur::{ensure, Context, Report, Result};
use rusoto_core::Region;
use rusoto_s3::S3Client;
use std::convert::TryFrom;
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
        root.push('/');
        root.push_str(path);
        root
    }
}

impl<'a> TryFrom<&'a Url> for Bucket {
    type Error = Report;

    fn try_from(url: &Url) -> Result<Bucket> {
        ensure!(url.scheme() == "s3", "URI scheme has to be `s3`");
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
    type Error = Report;

    fn try_from(bucket: &'a Bucket) -> Result<S3Client> {
        let region = Region::Custom {
            name: "custom-region".to_owned(),
            endpoint: bucket.endpoint.clone(),
        };

        Ok(S3Client::new(region))
    }
}

pub fn validate_checksum(key: &str, body: &[u8], received: &str) -> Result<()> {
    if received.contains('-') {
        log::warn!(
            "S3 checksum for file `{}` is in multipart format, which artefacta doesn't support yet",
            key
        );
        return Ok(());
    }

    // strip quotes
    let received = received.trim_start_matches('"').trim_end_matches('"');

    log::trace!("S3's checksum for file `{}`: {}", key, received);
    let checksum = md5::compute(body);
    let checksum = format!("{:x}", checksum);

    ensure!(
        received == checksum,
        "checksum received from S3 was `{}` but we calculated it `{}`",
        received,
        checksum,
    );

    Ok(())
}
