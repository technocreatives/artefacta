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

    for file in entries {
        let file = file.context("read file")?;
        if file.file_type().is_dir() {
            log::trace!("skipping directory entry in tar");
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

    #[test]
    fn archive_is_fine() {
        use zstd::stream::write::Encoder as ZstdEncoder;

        let tmp = tempdir().expect("tempdir");
        let archive = tmp.path().join("archive.tar.zst");

        let mut output = ZstdEncoder::new(fs::File::create(&archive).unwrap(), 3).unwrap();
        package("src".as_ref(), &mut output).expect("package");
        output.finish().unwrap();

        let cmd = std::process::Command::new("tar")
            .arg("-Izstd")
            .arg("-xvf")
            .arg(&archive)
            .current_dir(tmp.path())
            .output()
            .expect("tar");
        dbg!(&cmd);
        assert!(cmd.status.success());

        let ls = std::process::Command::new("ls").output().unwrap();
        dbg!(&ls);
    }

    proptest! {
        #[test]
        fn determinsitic_tar(files in prop::collection::vec(r"[0-9A-Za-z][0-9A-Za-z/]+[0-9A-Za-z]", 1..10)) {
            let tmp = tempdir().expect("tempdir");
            let dir1 = tmp.path().join("dir1");
            let dir2 = tmp.path().join("dir2");

            // create some random files in random paths
            for f in &files {
                random_file(&dir1.join(f)).expect("random_file");
            }

            // package this dir
            let mut output1 = Vec::new();
            package(&dir1, &mut output1).expect("package");

            // copy this dir to a new one!
            let cmd = std::process::Command::new("cp")
                .arg("-r")
                .arg("dir1")
                .arg("dir2")
                .current_dir(tmp.path())
                .output()
                .expect("cp");
            dbg!(&cmd);
            prop_assert!(cmd.status.success());

            // package copied dir
            let mut output2 = Vec::new();
            package(&dir2, &mut output2).expect("package");

            // read both back in
            let mut arch1 = tar::Archive::new(Cursor::new(&output1));
            let mut arch2 = tar::Archive::new(Cursor::new(&output2));

            // assert they have the same entries (quick to compare)
            arch1.entries().expect("tar entries").zip(arch2.entries().expect("tar entries")).for_each(|(f1, f2)| {
                assert_eq!(
                    format!("{:?}", f1.expect("read file").header()),
                    format!("{:?}", f2.expect("read file").header())
                );
            });

            // assert the archives are actually bit-identical
            prop_assert!(output1 == output2);

            // and now for good measure check they can also be read by the system's `tar`
            fs::write(tmp.path().join("dir1.tar"), output1).expect("write tar");
            let cmd = std::process::Command::new("tar")
                .arg("--list")
                .arg("--file")
                .arg("dir1.tar")
                .current_dir(tmp.path())
                .output()
                .expect("tar");
            // dbg!(&cmd);
            prop_assert!(cmd.status.success());
        }
    }
}
