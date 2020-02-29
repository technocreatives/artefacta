use anyhow::{Context, Result};
use rand::prelude::*;
use std::{
    fs,
    io::{self, Cursor, Read, Write},
    path::Path,
};
use tempfile::tempdir;
use zstd::stream::write::Encoder as ZstdEncoder;

use artefacta::{apply_patch, PatchGraph};

#[test]
fn roundtrip() -> Result<()> {
    let dir = tempdir()?;

    let file1 = dir.path().join("1.tar.zst");
    let content1 = random_file(&file1)?;

    let file2 = dir.path().join("2.tar.zst");
    let content2 = random_file(&file2)?;

    let patch_1_2 = dir.path().join("1-2.patch.zst");

    let mut patch = ZstdEncoder::new(fs::File::create(&patch_1_2)?, 3)?;
    bidiff::simple_diff(&content1, &content2, &mut patch)?;
    patch.finish()?;

    let mut patched = apply_patch(&file1, &patch_1_2)?;
    let mut buffer = Vec::new();
    patched.read_to_end(&mut buffer)?;

    assert_eq!(zstd::stream::decode_all(fs::File::open(&file2)?)?, buffer);

    Ok(())
}

fn random_file(path: &Path) -> Result<Vec<u8>> {
    let mut rng = rand::thread_rng();
    let mut raw_content = vec![0u8; 1024];
    rng.try_fill(&mut raw_content[..])?;
    let content = zstd::stream::encode_all(Cursor::new(&raw_content[..]), 3)?;

    fs::write(path, content).context("write file")?;
    Ok(raw_content)
}
