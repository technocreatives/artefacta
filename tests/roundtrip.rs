use anyhow::{Context, Result};
use rand::prelude::*;
use std::convert::TryInto;
use std::{
    fs,
    io::{Cursor, Read},
    path::Path,
};
use tempfile::{tempdir, TempDir};
use zstd::stream::write::Encoder as ZstdEncoder;

use artefacta::{apply_patch, ArtefactIndex};

#[test]
#[ignore]
fn generate_patches() -> Result<()> {
    let dir = test_dir(&["1.tar.zst", "2.tar.zst", "1-2.patch.zst"])?;

    let mut index = ArtefactIndex::from_dir(&dir)?;
    index.add_build("3.tar.zst")?;
    index.generate_missing_patches()?;

    assert!(
        index.get_patch("2".try_into()?, "3".try_into()?).is_ok(),
        "didn't create patch"
    );

    Ok(())
}

fn test_dir(files: &[&str]) -> Result<TempDir> {
    let dir = tempdir()?;
    let mut rng = rand::thread_rng();

    for file in files {
        let mut raw_content = vec![0u8; 1024];
        rng.try_fill(&mut raw_content[..])?;
        let content = zstd::stream::encode_all(Cursor::new(&raw_content[..]), 3)?;

        fs::write(dir.path().join(file), content).context("write file")?;
    }

    Ok(dir)
}
