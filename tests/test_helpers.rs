#![allow(unused)]

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
    cmd.env("RUST_LOG", "info,artefacta=trace");
    cmd.timeout(std::time::Duration::from_secs(10));
    cmd
}

pub fn ls(path: impl AsRef<Path>) {
    let path = path.as_ref();
    let res = Command::new("ls").current_dir(path).output().unwrap();
    println!(
        "> ls {}\n{}---",
        path.display(),
        String::from_utf8_lossy(&res.stdout)
    );
}
