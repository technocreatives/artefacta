mod test_helpers;
use test_helpers::*;

#[test]
#[ignore]
fn s3_works() {
    let (machine1, scratch) = init();
    let (machine1, scratch) = (machine1.path(), scratch.path());
    let remote = "s3://REPLACEME.ams3.digitaloceanspaces.com/test";

    random_zstd_file(scratch.join("build1.tar.zst")).unwrap();

    artefacta(machine1, remote)
        .arg("add")
        .arg(scratch.join("build1.tar.zst"))
        .arg("--upload")
        .succeeds();

    let machine2 = tempdir().unwrap();
    let machine2 = machine2.path();

    artefacta(machine2, remote)
        .args(&["install", "build1"])
        .succeeds();
}
