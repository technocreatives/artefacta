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

#[test]
fn upgrade_to_new_build_with_patches() {
    let (machine1, remote) = init();
    let (machine1, remote) = (machine1.path(), remote.path());
    let (machine2, _) = init();
    let machine2 = machine2.path();

    let mut content = random_bytes(1024).unwrap();
    zstd_file(remote.join("build1.tar.zst"), &content).unwrap();
    content.extend(random_bytes(32).unwrap());
    zstd_file(remote.join("build2.tar.zst"), &content).unwrap();

    artefacta(machine1, remote)
        .args(&["create-patch", "build1", "build2"])
        .succeeds();
    artefacta(machine1, remote).args(&["sync"]).succeeds();

    artefacta(machine2, remote)
        .args(&["install", "build1"])
        .succeeds();
    artefacta(machine2, remote)
        .args(&["install", "build2"])
        .succeeds();
    assert!(machine2.join("build1-build2.patch.zst").exists());

    let current = machine2.join("current");
    assert_eq!(
        machine2.join("build2.tar.zst").canonicalize().unwrap(),
        fs::read_link(&current).unwrap(),
        "symlink points to new build"
    );
}

#[test]
fn upgrade_to_new_build_despite_broken_patches() {
    let (local, remote) = init();
    let (local, remote) = (local.path(), remote.path());

    let mut content = random_bytes(1024).unwrap();
    zstd_file(remote.join("build1.tar.zst"), &content).unwrap();
    content.extend(random_bytes(32).unwrap());
    zstd_file(remote.join("build2.tar.zst"), &content).unwrap();
    artefacta(local, remote)
        .args(&["install", "build1"])
        .succeeds();

    // this file is not a valid patch!
    zstd_file(
        remote.join("build1-build2.patch.zst"),
        &random_bytes(144).unwrap(),
    )
    .unwrap();

    artefacta(local, remote)
        .args(&["install", "build2"])
        .succeeds();

    assert_eq!(
        local.join("build2.tar.zst").canonicalize().unwrap(),
        fs::read_link(local.join("current")).unwrap(),
        "symlink points to new build"
    );
}
