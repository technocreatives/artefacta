mod test_helpers;
use test_helpers::*;

#[test]
fn install_build_from_remote_directory() {
    let (local, remote) = init();
    let (local, remote) = (local.path(), remote.path());

    random_zstd_file(remote.join("build1.tar.zst")).unwrap();
    random_zstd_file(remote.join("build2.tar.zst")).unwrap();

    artefacta(local, remote)
        .args(&["install", "build2"])
        .succeeds();

    let current = local.join("current");
    assert!(current.exists(), "Added `current` symlink");

    assert!(
        local.join("build2.tar.zst").exists(),
        "new build was copied to local storage"
    );

    assert_eq!(
        local.join("build2.tar.zst").canonicalize().unwrap(),
        fs::read_link(&current).unwrap(),
        "symlink points to new build"
    );
}

#[test]
fn upgrade_to_a_build_already_cached() {
    let (local, remote) = init();
    let (local, remote) = (local.path(), remote.path());

    random_zstd_file(remote.join("build1.tar.zst")).unwrap();
    random_zstd_file(remote.join("build2.tar.zst")).unwrap();

    // "cache" the builds locally
    fs::copy(remote.join("build1.tar.zst"), local.join("build1.tar.zst")).unwrap();
    fs::copy(remote.join("build2.tar.zst"), local.join("build2.tar.zst")).unwrap();

    // "install" build1
    std::os::unix::fs::symlink(local.join("build1.tar.zst"), local.join("current")).unwrap();

    artefacta(local, remote)
        .args(&["install", "build2"])
        .succeeds();

    let current = local.join("current");
    assert_eq!(
        local.join("build2.tar.zst").canonicalize().unwrap(),
        fs::read_link(&current).unwrap(),
        "symlink points to new build"
    );
}

#[test]
fn upgrade_to_new_build_without_patches() {
    let (local, remote) = init();
    let (local, remote) = (local.path(), remote.path());

    random_zstd_file(remote.join("build1.tar.zst")).unwrap();
    random_zstd_file(remote.join("build2.tar.zst")).unwrap();

    // "install" build1
    fs::copy(remote.join("build1.tar.zst"), local.join("build1.tar.zst")).unwrap();
    std::os::unix::fs::symlink(local.join("build1.tar.zst"), local.join("current")).unwrap();

    artefacta(local, remote)
        .args(&["install", "build2"])
        .succeeds();

    let current = local.join("current");
    assert_eq!(
        local.join("build2.tar.zst").canonicalize().unwrap(),
        fs::read_link(&current).unwrap(),
        "symlink points to new build"
    );
}

#[test]
fn size_is_different_between_remote_and_local() {
    let (local, remote) = init();
    let (local, remote) = (local.path(), remote.path());

    // two textfiles, both alike in dignity
    fs::write(local.join("build1.tar.zst"), b"lorem ipsum").unwrap();
    fs::write(remote.join("build1.tar.zst"), b"dolor sit amet").unwrap();

    artefacta(local, remote)
        .args(&["install", "build1"])
        .assert()
        .success()
        .stderr(
            predicate::str::is_match(
                "Using locally cached file for `build1` but size on remote differs",
            )
            .unwrap(),
        );
}
