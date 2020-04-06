use assert_cmd::Command;

#[path = "../src/test_helpers.rs"]
mod unit_test_helpers;
pub use unit_test_helpers::*;

pub fn init() -> (TempDir, TempDir) {
    logger();

    let local = tempdir().unwrap();
    let remote = tempdir().unwrap();

    (local, remote)
}

pub fn artefacta(local: &Path, remote: &Path) -> Command {
    let mut cmd = Command::cargo_bin("artefacta").unwrap();
    cmd.env("ARTEFACTA_LOCAL_STORE", local);
    cmd.env("ARTEFACTA_REMOTE_STORE", remote);
    cmd.arg("--verbose");
    cmd
}