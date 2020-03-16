pub use anyhow::{Context, Result};
pub use rand::prelude::*;
pub use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
};
pub use tempfile::tempdir;

pub fn random_file(path: impl AsRef<Path>) -> Result<Vec<u8>> {
    let mut rng = rand::thread_rng();
    let mut raw_content = vec![0u8; 1024];
    rng.try_fill(&mut raw_content[..])?;
    let content = zstd::stream::encode_all(Cursor::new(&raw_content[..]), 3)?;

    fs::write(path, content).context("write file")?;
    Ok(raw_content)
}
