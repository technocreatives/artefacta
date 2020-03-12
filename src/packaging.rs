//! Package build using `tar` in the most deterministic way possible.

use anyhow::{Context, Result};
use std::{
    fs,
    io::{BufReader, Write},
    path::Path,
};
use walkdir::WalkDir;

pub fn package(source_dir: &Path, target: impl Write) -> Result<()> {
    let mut archive = tar::Builder::new(target);
    archive.mode(tar::HeaderMode::Deterministic);

    let entries = WalkDir::new(source_dir)
        .sort_by(|a, b| a.path().cmp(b.path()))
        .into_iter();

    for file in dbg!(entries) {
        let file = file.context("read file")?;
        if file.file_type().is_dir() {
            dbg!(file);
        } else if file.file_type().is_file() {
            add_file(&mut archive, &file, source_dir)
                .with_context(|| format!("add `{}` to archive", file.path().display()))?;
        }
    }

    archive.finish().context("writing tar")?;

    Ok(())
}

fn add_file<W: Write>(
    archive: &mut tar::Builder<W>,
    file: &walkdir::DirEntry,
    root: &Path,
) -> Result<()> {
    let path = file.path().strip_prefix(root).context("root path prefix")?;
    dbg!(&path);
    let mut header = tar::Header::new_gnu();

    header
        .set_path(path)
        .context("set path in archive header")?;
    header.set_size(file.metadata().context("read metadata")?.len());
    header.set_cksum();
    let file = BufReader::new(fs::File::open(file.path()).context("open file")?);

    archive
        .append_data(&mut header, path, file)
        .context("append file")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn determinsitic_tar(files in prop::collection::vec(r"[[:alnum:]]+", 1..10)) {
            dbg!(&files);
            let dir1 = tempdir().expect("tempdir");
            for f in &files {
                random_file(&dir1.path().join(f)).expect("random_file");
            }

            let mut output1 = Vec::new();
            package(dir1.path(), &mut output1).expect("package");


            let dir2 = tempdir().expect("tempdir");
            for f in &files {
                fs::copy(&dir1.path().join(f), &dir2.path().join(f)).expect("copy");
            }

            let mut output2 = Vec::new();
            package(dir2.path(), &mut output2).expect("package");


            let mut arch1 = tar::Archive::new(Cursor::new(&output1));
            let mut arch2 = tar::Archive::new(Cursor::new(&output2));

            arch1.entries().expect("tar entries").zip(arch2.entries().expect("tar entries")).for_each(|(f1, f2)| {
                assert_eq!(
                    format!("{:?}", f1.expect("read file").header()),
                    format!("{:?}", f2.expect("read file").header())
                );
            });

            prop_assert!(output1 == output2);
        }
    }
}
