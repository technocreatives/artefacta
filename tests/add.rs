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
fn add_file_by_packaging_it_as_a_tar_zst() {
    let (local, remote) = init();
    let (local, remote) = (local.path(), remote.path());

    let build_dir = tempdir().unwrap();
    let binary = build_dir.child("do-the-work.sh");
    binary.write_str("ELF").unwrap();

    artefacta(local, remote)
        .arg("add-package")
        .arg("build1")
        .arg(binary.path())
        .assert()
        .success();

    assert!(
        local.join("build1.tar.zst").exists(),
        "build was copied to local storage"
    );

    let unarchive = tempdir().unwrap();
    untar(local.join("build1.tar.zst"), unarchive.path());

    unarchive
        .child("do-the-work.sh")
        .assert(predicate::path::is_file());
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

#[test]
fn add_package_with_invalid_version() {
    let (local, remote) = init();
    let (local, remote) = (local.path(), remote.path());

    let build_dir = tempdir().unwrap();
    let build_dir = build_dir.path();

    fs::write(build_dir.join("lib.rs"), b"fn main() { /* code here */ }").unwrap();
    fs::write(build_dir.join("Cargo.toml"), b"[package]").unwrap();

    artefacta(local, remote)
        .arg("add-package")
        .arg("build-1-2---3")
        .arg(&build_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid version format"));
}

#[test]
fn upload_a_build() {
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
fn add_build_locally_and_calculate_a_patch() {
    let (local, remote) = init();
    let (local, remote) = (local.path(), remote.path());

    let scratch = tempdir().unwrap();
    let scratch = scratch.path();

    crate::test_helpers::random_zstd_file(local.join("build1.tar.zst")).unwrap();
    crate::test_helpers::random_zstd_file(scratch.join("build2.tar.zst")).unwrap();

    artefacta(local, remote)
        .arg("add")
        .arg(scratch.join("build2.tar.zst"))
        .arg("--calc-patch-from=build1")
        .assert()
        .success();

    ls(local);
    ls(remote);

    assert!(
        local.join("build2.tar.zst").exists(),
        "build was copied to remote storage"
    );

    assert!(
        local.join("build1-build2.patch.zst").exists(),
        "build was copied to remote storage"
    );
}

#[test]
fn adding_file_that_does_not_exist() {
    let (local, remote) = init();
    let (local, remote) = (local.path(), remote.path());

    let scratch = tempdir().unwrap();
    let scratch = scratch.path();

    crate::test_helpers::random_zstd_file(scratch.join("right-name.tar.zst")).unwrap();

    artefacta(local, remote)
        .arg("add")
        .arg(scratch.join("wrong-name.tar.zst"))
        .assert()
        .failure()
        .stderr(
            predicate::str::is_match("Tried to add `(.*?)` as new build, but file does not exist")
                .unwrap(),
        );
}
