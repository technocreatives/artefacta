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
        .assert()
        .success();

    assert!(
        local.join("build1.tar.zst").exists(),
        "build was copied to local storage"
    );

    assert!(
        remote.join("build1.tar.zst").exists(),
        "build was copied to local storage"
    );
}
