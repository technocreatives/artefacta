mod test_helpers;
use test_helpers::*;

#[test]
fn empty() {
    let (local, remote) = init();
    let (local, remote) = (local.path(), remote.path());

    artefacta(local, remote).args(&["debug"]).succeeds();
}

#[test]
fn local_empty() {
    let (local, remote) = init();
    let (local, remote) = (local.path(), remote.path());

    random_zstd_file(remote.join("build1.tar.zst")).unwrap();

    artefacta(local, remote).args(&["debug"]).succeeds();
}

#[test]
fn remote_empty() {
    let (local, remote) = init();
    let (local, remote) = (local.path(), remote.path());

    random_zstd_file(local.join("build1.tar.zst")).unwrap();

    artefacta(local, remote).args(&["debug"]).succeeds();
}

#[test]
fn local_has_patches_with_no_builds() {
    let (local, remote) = init();
    let (local, remote) = (local.path(), remote.path());

    random_zstd_file(remote.join("build1.tar.zst")).unwrap();
    random_zstd_file(remote.join("build2.tar.zst")).unwrap();

    artefacta(local, remote)
        .args(&["create-patch", "build1", "build2"])
        .succeeds();

    fs::remove_file(local.join("build1.tar.zst")).unwrap();

    artefacta(local, remote)
        .args(&["debug"])
        .assert()
        .success()
        .stderr(predicate::str::contains("failed to add patch").not());
}
