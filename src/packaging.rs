//! Package build using `tar` in the most deterministic way possible.

use anyhow::{Context, Result};
use std::{io::Write, path::Path};

pub fn package(source_dir: &Path, target: impl Write) -> Result<()> {
    let mut archive = tar::Builder::new(target);
    archive.mode(tar::HeaderMode::Deterministic);
    archive
        .append_dir_all(".", source_dir)
        .context("append files to archive")?;
    archive.finish().context("writing tar")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;

    #[test]
    fn determinsitic_tar() -> Result<()> {
        let dir = tempdir()?;
        let file1 = dir.path().join("foo");
        let file2 = dir.path().join("bar");
        random_file(&file1)?;
        random_file(&file2)?;

        let mut output1 = Vec::new();
        package(dir.path(), &mut output1)?;

        let dir = tempdir()?;
        fs::copy(&file1, dir.path().join("foo"))?;
        fs::copy(&file2, dir.path().join("bar"))?;
        let mut output2 = Vec::new();

        package(dir.path(), &mut output2)?;

        assert_eq!(output1, output2);

        Ok(())
    }
}
