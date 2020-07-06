#![allow(unused)]

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
    cmd.arg("--verbose");
    cmd.timeout(std::time::Duration::from_secs(10));
    cmd
}

pub fn run(cmd: &str, dir: impl AsRef<Path>) {
    Command::new("bash")
        .arg("-c")
        .arg(cmd)
        .current_dir(dir.as_ref())
        .succeeds();
}

pub trait CommandExt {
    fn succeeds(&mut self);
}

impl CommandExt for Command {
    fn succeeds(&mut self) {
        use std::time::{Duration, Instant};
        let start = Instant::now();

        let output = self.unwrap();
        println!("> {:?} (exit code {:?})", self, output.status.code());

        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.is_empty() {
            eprintln!("[stdout] {}", stdout.trim_end());
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.is_empty() {
            eprintln!("[stderr] {}", stderr.trim_end());
        }

        let end = Instant::now();
        eprintln!("< took {:?}", end.duration_since(start));
        assert!(output.status.success(), "failed to run {:?}", self);
    }
}
