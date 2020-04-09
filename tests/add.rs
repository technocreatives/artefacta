mod test_helpers;
use test_helpers::*;

#[test]
fn add_existing_tar_zst_file_by_copying_it() {
    let (local, remote) = init();
    let (local, remote) = (local.path(), remote.path());

    let scratch = tempdir().unwrap();
    let scratch = scratch.path();

    fs::write(scratch.join("build1.tar.zst"), b"foobar").unwrap();

    artefacta(local, remote)
        .arg("add")
        .arg(scratch.join("build1.tar.zst"))
        .assert()
        .success();

    ls(local);

    assert!(
        local.join("build1.tar.zst").exists(),
        "build was copied to local storage"
    );
}

#[test]
fn add_existing_tar_zst_file_by_copying_it_with_remote_sync() {
    let (local, remote) = init();
    let (local, remote) = (local.path(), remote.path());

    let scratch = tempdir().unwrap();
    let scratch = scratch.path();

    fs::write(scratch.join("build1.tar.zst"), b"foobar").unwrap();

    artefacta(local, remote)
        .arg("add")
        .arg(scratch.join("build1.tar.zst"))
        .arg("--upload")
        .ok()
        .unwrap();

    ls(local);
    ls(remote);

    assert!(
        local.join("build1.tar.zst").exists(),
        "build was copied to local storage"
    );

    assert!(
        remote.join("build1.tar.zst").exists(),
        "build was copied to remote storage"
    );
}

#[test]
fn add_directory_by_packaging_it_as_a_tar_zst() {
    let (local, remote) = init();
    let (local, remote) = (local.path(), remote.path());

    let build_dir = tempdir().unwrap();
    let build_dir = build_dir.path();

    fs::write(build_dir.join("lib.rs"), b"fn main() { /* code here */ }").unwrap();
    fs::write(build_dir.join("Cargo.toml"), b"[package]").unwrap();

    artefacta(local, remote)
        .arg("add-package")
        .arg("build1")
        .arg(&build_dir)
        .assert()
        .success();

    assert!(
        local.join("build1.tar.zst").exists(),
        "build was copied to local storage"
    );
}
