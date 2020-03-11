use anyhow::{Context, Result};
use rand::prelude::*;
use std::{fs, io::Cursor};
use tempfile::{tempdir, TempDir};

use artefacta::{ArtefactIndex, Storage};

#[test]
// #[ignore]
fn generate_patches() -> Result<()> {
    let dir = test_dir(&["1.tar.zst", "2.tar.zst", "1-2.patch.zst"])?;
    let remote_dir = test_dir(&["3.tar.zst"])?;

    let mut index = dbg!(ArtefactIndex::new(
        &dir,
        Storage::Filesystem(remote_dir.path().into()),
    )?);
    index.add_build(&remote_dir.path().join("3.tar.zst"))?;

    assert!(
        index.get_build("3".parse()?).is_ok(),
        "didn't add build to index {:?}",
        index
    );

    index
        .calculate_patch("2".parse()?, "3".parse()?)
        .context("calc patches")?;

    dbg!(&index);

    index.get_patch("2".parse()?, "3".parse()?).unwrap();

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
