#![allow(unused)]

pub use anyhow::{Context, Result};
pub use rand::prelude::*;
pub use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
};
pub use tempfile::tempdir;

pub fn random_file(path: impl AsRef<Path>) -> Result<Vec<u8>> {
    let path = path.as_ref();
    let mut rng = rand::thread_rng();
    let mut raw_content = vec![0u8; 1024];
    rng.try_fill(&mut raw_content[..])?;
    let content = zstd::stream::encode_all(Cursor::new(&raw_content[..]), 3)?;

    fs::create_dir_all(path.parent().context("parent dir")?).context("mkdir")?;
    fs::write(path, content).context("write file")?;
    Ok(raw_content)
}

pub fn logger() {
    let _ = pretty_env_logger::formatted_builder()
        .filter(None, log::LevelFilter::Debug)
        .target(env_logger::Target::Stderr)
        .is_test(true)
        .try_init();
}
