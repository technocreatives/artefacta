mod test_helpers;
use test_helpers::*;

#[test]
fn install_build_from_remote_directory() {
    let (local, remote) = init();
    let (local, remote) = (local.path(), remote.path());

    fs::write(remote.join("build1.tar.zst"), b"foobar").unwrap();
    fs::write(remote.join("build2.tar.zst"), b"foobarbaz").unwrap();

    artefacta(local, remote)
        .args(&["install", "build2"])
        .assert()
        .success();

    let current = local.join("current");
    assert!(current.exists(), "Added `current` symlink");

    assert!(
        local.join("build2.tar.zst").exists(),
        "new build was copied to local storage"
    );

    assert_eq!(
        local.join("build2.tar.zst"),
        fs::read_link(&current).unwrap(),
        "symlink points to new build"
    );
}
