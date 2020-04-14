#![allow(unused)]

pub use anyhow::{Context, Result};
pub use assert_cmd::Command;
pub use assert_fs::{prelude::*, TempDir};
pub use predicates::prelude::*;
pub use rand::prelude::*;
pub use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
};

pub fn tempdir() -> Result<TempDir> {
    assert_fs::TempDir::new().context("can't create temp dir")
}

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

pub fn ls(path: impl AsRef<Path>) {
    let path = path.as_ref();
    let res = Command::new("ls")
        .arg("-lah")
        .current_dir(path)
        .output()
        .unwrap();
    println!(
        "> ls {}\n{}---",
        path.display(),
        String::from_utf8_lossy(&res.stdout)
    );
}

pub fn untar(archive_path: impl AsRef<Path>, target_dir: impl AsRef<Path>) {
    assert!(predicate::path::is_dir().eval(target_dir.as_ref()));

    let res = Command::new("tar")
        .arg("-Izstd")
        .arg("-xvf")
        .arg(archive_path.as_ref())
        .current_dir(target_dir.as_ref())
        .output()
        .expect("tar");

    println!(
        "> tar {}\n{}---",
        archive_path.as_ref().display(),
        String::from_utf8_lossy(&res.stdout)
    );
    assert!(res.status.success());
}
