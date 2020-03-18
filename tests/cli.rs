use assert_cmd::Command;

#[path = "../src/test_helpers.rs"]
mod test_helpers;
use test_helpers::*;

#[test]
fn install_build_from_remote() {
    logger();

    let local = tempdir().unwrap();
    let remote = tempdir().unwrap();

    fs::write(remote.path().join("build1.tar.zst"), b"foobar").unwrap();
    fs::write(remote.path().join("build2.tar.zst"), b"foobarbaz").unwrap();

    artefacta(local.path(), remote.path())
        .args(&["install", "build2"])
        .assert()
        .success();

    // Added "current" symlink
    let current = local.path().join("current");
    assert!(current.exists());

    // symlink points to new build that was also copied to local storage
    let curent_path = fs::read_link(&current).unwrap();
    assert_eq!(local.path().join("build2.tar.zst"), curent_path);
}

fn artefacta(local: &Path, remote: &Path) -> Command {
    let mut cmd = Command::cargo_bin("artefacta").unwrap();
    cmd.env("ARTEFACTA_LOCAL_STORE", local);
    cmd.env("ARTEFACTA_REMOTE_STORE", remote);
    cmd.arg("--verbose");
    cmd
}
