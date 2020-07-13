mod test_helpers;
use test_helpers::*;

#[test]
fn upgrade_to_new_build_with_patches() {
    let (machine1, remote) = init();
    let (machine1, remote) = (machine1.path(), remote.path());

    // 3 builds, all slightly different
    let mut content = random_bytes(1024).unwrap();
    zstd_file(remote.join("build1.tar.zst"), &content).unwrap();
    content.extend(random_bytes(32).unwrap());
    zstd_file(remote.join("build2.tar.zst"), &content).unwrap();
    content.extend(random_bytes(32).unwrap());
    zstd_file(remote.join("build3.tar.zst"), &content).unwrap();

    // 2 patches
    artefacta(machine1, remote)
        .args(&["create-patch", "build1", "build2"])
        .succeeds();
    assert!(machine1.join("build1-build2.patch.zst").exists());
    artefacta(machine1, remote)
        .args(&["create-patch", "build2", "build3"])
        .succeeds();
    assert!(machine1.join("build1-build2.patch.zst").exists());

    // sync to remote
    artefacta(machine1, remote).args(&["sync"]).succeeds();
    assert!(remote.join("build1-build2.patch.zst").exists());
    assert!(remote.join("build2-build3.patch.zst").exists());

    // and now let's install some builds
    let (machine2, _) = init();
    let machine2 = machine2.path();

    artefacta(machine2, remote).args(&["debug"]).succeeds();

    artefacta(machine2, remote)
        .args(&["install", "build1"])
        .succeeds();
    assert!(machine2.join("build1.tar.zst").exists());

    ls(remote);
    artefacta(machine2, remote)
        .args(&["install", "build3"])
        .succeeds();
    assert!(machine2.join("build3.tar.zst").exists());
    assert!(machine2.join("build1-build2.patch.zst").exists());
    assert!(machine2.join("build2-build3.patch.zst").exists());
}
