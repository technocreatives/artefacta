use assert_cmd::Command;
use std::{fs, path::Path};
use tempfile::tempdir;

#[test]
#[ignore]
fn install_build_from_remote() {
    let local = tempdir().unwrap();
    let remote = tempdir().unwrap();

    fs::write(remote.path().join("build1.tar.zst"), b"foobar").unwrap();
    fs::write(remote.path().join("build2.tar.zst"), b"foobarbaz").unwrap();

    artefacta(local.path(), remote.path())
        .args(&["install", "build2"])
        .assert()
        .success();
}

fn artefacta(local: &Path, remote: &Path) -> Command {
    let mut cmd = Command::cargo_bin("artefacta").unwrap();
    cmd.env("ARTEFACTA_LOCAL_STORE", local);
    cmd.env("ARTEFACTA_REMOTE_STORE", remote);
    cmd
}
