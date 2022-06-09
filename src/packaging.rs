//! Package build using `tar` in the most deterministic way possible.

use erreur::{Context, Result};
use std::{
    fs,
    io::{BufReader, Write},
    path::Path,
};
use walkdir::WalkDir;

pub fn package(source: &Path, target: impl Write) -> Result<()> {
    let mut archive = tar::Builder::new(target);
    archive.mode(tar::HeaderMode::Deterministic);
    log::debug!("writing files from `{}` to archive", source.display());

    let root = if source.is_file() {
        source
            .parent()
            .with_context(|| format!("can't find parent of `{}`", source.display()))?
    } else {
        source
    };

    let entries = WalkDir::new(source)
        .sort_by(|a, b| a.path().cmp(b.path()))
        .into_iter();

    for file in entries {
        let file = file.context("read file")?;
        if file.file_type().is_dir() {
            log::trace!("skipping directory entry in tar");
        } else if file.file_type().is_file() {
            add_file(&mut archive, &file, root)
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
    let is_sane_path = path.to_str().is_some();
    if !is_sane_path {
        log::warn!(
            "adding path `{}` to archive which is not UTF-8. \
            This will most likely break somewhere down the line \
            without us noticing until it's much too late.",
            path.display()
        );
    }
    let metadata = file.metadata().context("read metadata")?;

    // Welcome to this new tar file entry.
    //
    // We set the size, POSIX permission flags, and some defaults ourselves but
    // the call to `append_data` all the way down there will set the path with
    // the nice GNU extensions to handle long paths.
    let mut header = tar::Header::new_gnu();
    header.set_size(metadata.len());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        header.set_mode(metadata.permissions().mode())
    }
    #[cfg(not(unix))]
    {
        // if you run this on Windows, I guess you get read and execute permissions always
        header.set_mode(0o100755)
    }

    header.set_cksum();
    header
        .set_device_major(0)
        .context("set device major header")?;
    header
        .set_device_minor(0)
        .context("set device minor header")?;

    let file = BufReader::new(fs::File::open(file.path()).context("open file")?);

    // Note: This also sets the file path in the header, and then appends header
    // and payload to the archive.
    archive
        .append_data(&mut header, path, file)
        .context("append file")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{compress, test_helpers::*};
    use proptest::prelude::*;

    #[test]
    fn archive_a_file() {
        logger();

        let tmp = tempdir().unwrap();
        let archive = tmp.child("archive.tar.zst");

        let binary = tmp.child("do-the-work.sh");
        binary.write_str("#! /bin/sh\necho 'Done!'").unwrap();

        let mut output = compress(fs::File::create(&archive.path()).unwrap()).unwrap();
        package(binary.path(), &mut output).expect("package");
        output.finish().unwrap();

        archive.assert(predicate::path::is_file());

        let unarchive = tempdir().unwrap();
        untar(archive.path(), unarchive.path());
        ls(tmp.path());

        unarchive
            .child("do-the-work.sh")
            .assert(predicate::path::is_file());
    }

    #[test]
    fn archive_with_long_paths() {
        let tmp = tempdir().unwrap();
        let long_path = "StandaloneLinux64/What-in-the-actual-Hell/Managed/Unity.RenderPipelines.ShaderGraph.ShaderGraphLibrary.dll";
        tmp.child(long_path).write_str("archive me").unwrap();

        let target = tempdir().unwrap();
        let archive = target.child("archive.tar.zst");

        let mut output = compress(fs::File::create(archive.path()).unwrap()).unwrap();
        package(tmp.path(), &mut output).expect("package");
        output.finish().unwrap();

        archive.assert(predicate::path::is_file());

        ls(target.path());
        let unarchive = tempdir().unwrap();
        untar(archive.path(), unarchive.path());

        ls(unarchive.path());
    }

    #[test]
    #[cfg(unix)] // only tests POSIX ACLs
    fn archive_keeps_permission_bits() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempdir().unwrap();
        let archive = tmp.child("archive.tar.zst");

        let binary = tmp.child("do-the-work.sh");
        binary.write_str("#! /bin/sh\necho 'Done!'").unwrap();

        let read_and_execute = fs::Permissions::from_mode(0o100555);
        fs::set_permissions(binary.path(), read_and_execute.clone()).unwrap();

        let mut output = compress(fs::File::create(archive.path()).unwrap()).unwrap();
        package(binary.path(), &mut output).expect("package");
        output.finish().unwrap();

        archive.assert(predicate::path::is_file());

        ls(tmp.path());

        let unarchive = tempdir().unwrap();
        untar(archive.path(), unarchive.path());

        ls(unarchive.path());

        let perms_after_the_tar = unarchive
            .child("do-the-work.sh")
            .path()
            .metadata()
            .unwrap()
            .permissions();
        assert_eq!(perms_after_the_tar.mode(), read_and_execute.mode());
    }

    #[test]
    fn archive_is_fine() {
        let tmp = tempdir().expect("tempdir");
        let archive = tmp.child("archive.tar.zst");
        let src = tmp.child("src");
        src.create_dir_all().unwrap();
        src.child("Cargo.toml").write_str("[package]").unwrap();
        src.child("main.rs").write_str("fn main() {}").unwrap();

        let mut output = compress(fs::File::create(archive.path()).unwrap()).unwrap();
        package(src.path(), &mut output).expect("package");
        output.finish().unwrap();

        let unarchive = tempdir().unwrap();
        untar(archive.path(), unarchive.path());

        ls(src.path());

        unarchive
            .child("main.rs")
            .assert(predicate::path::is_file());
    }

    proptest! {
        #[test]
        fn determinsitic_tar(files in prop::collection::vec(r"[0-9A-Za-z][0-9A-Za-z/]+[0-9A-Za-z]", 1..10)) {
            let tmp = tempdir().expect("tempdir");
            let dir1 = tmp.path().join("dir1");
            let dir2 = tmp.path().join("dir2");

            // create some random files in random paths
            for f in &files {
                random_zstd_file(&dir1.join(f)).expect("random_file");
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

            prop_assert!(cmd.status.success());
        }
    }
}
