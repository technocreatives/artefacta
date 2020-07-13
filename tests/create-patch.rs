mod test_helpers;
use test_helpers::*;

#[test]
fn create_a_patch_from_remote_builds() {
    let (local, remote) = init();
    let (local, remote) = (local.path(), remote.path());

    random_zstd_file(remote.join("build1.tar.zst")).unwrap();
    random_zstd_file(remote.join("build2.tar.zst")).unwrap();

    artefacta(local, remote)
        .args(&["create-patch", "build1", "build2"])
        .succeeds();

    assert!(local.join("build1-build2.patch.zst").exists());
}

#[test]
fn patches_cant_have_same_to_and_from() {
    let (local, remote) = init();
    let (local, remote) = (local.path(), remote.path());

    random_zstd_file(remote.join("build1.tar.zst")).unwrap();
    random_zstd_file(remote.join("build2.tar.zst")).unwrap();

    artefacta(local, remote)
        .args(&["create-patch", "build1", "build1"]) // sic!
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Rejecting to create patch between same versions",
        ));
}
