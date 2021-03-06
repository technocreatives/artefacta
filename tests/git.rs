mod test_helpers;
use test_helpers::*;

#[test]
fn auto_patch_from_git_repo() {
    let (local, remote) = init();
    let (local, remote) = (local.path(), remote.path());
    let repo = tempdir().unwrap();
    let repo = repo.path();

    run("git init .", &repo);
    run("git config user.email 'git-test@example.com'", &repo);
    run("git config user.name 'Author Name'", &repo);

    run("mkdir src", &repo);
    run("echo foo > src/wtf", &repo);
    run("git add .", &repo);
    run("git commit -m 'bump 0.1.0'", &repo);
    run("git tag 0.1.0", &repo);
    artefacta(local, remote)
        .arg("add-package")
        .arg("0.1.0")
        .arg(repo.join("src"))
        .succeeds();

    run("echo bar > src/wtf", &repo);
    run("git add .", &repo);
    run("git commit -m 'bump 0.1.1'", &repo);
    run("git tag 0.1.1", &repo);
    artefacta(local, remote)
        .arg("add-package")
        .arg("0.1.1")
        .arg(repo.join("src"))
        .succeeds();
    artefacta(local, remote)
        .arg("auto-patch")
        .arg("--repo-root")
        .arg(&repo)
        .arg("0.1.1")
        .succeeds();

    run("echo baz > src/wtf", &repo);
    run("git add .", &repo);
    run("git commit -m 'bump 0.2.0'", &repo);
    run("git tag 0.2.0", &repo);
    artefacta(local, remote)
        .arg("add-package")
        .arg("0.2.0")
        .arg(repo.join("src"))
        .succeeds();
    artefacta(local, remote)
        .arg("auto-patch")
        .arg("--repo-root")
        .arg(&repo)
        .arg("0.2.0")
        .succeeds();

    run("echo lorem > src/wtf", &repo);
    run("git add .", &repo);
    run("git commit -m 'bump 0.2.1'", &repo);
    run("git tag 0.2.1", &repo);
    artefacta(local, remote)
        .arg("add-package")
        .arg("0.2.1")
        .arg(repo.join("src"))
        .succeeds();
    artefacta(local, remote)
        .arg("auto-patch")
        .arg("--repo-root")
        .arg(&repo)
        .arg("0.2.1")
        .succeeds();

    run("git tag -l", &repo);
    ls(&local);

    assert!(local.join("0.1.0-0.1.1.patch.zst").exists());
    assert!(local.join("0.1.1-0.2.0.patch.zst").exists());
    assert!(local.join("0.1.1-0.2.1.patch.zst").exists());
    assert!(local.join("0.2.0-0.2.1.patch.zst").exists());
}

#[test]
fn auto_patch_from_git_repo_with_prefix() {
    let (local, remote) = init();
    let (local, remote) = (local.path(), remote.path());
    let repo = tempdir().unwrap();
    let repo = repo.path();

    run("git init .", &repo);
    run("git config user.email 'git-test@example.com'", &repo);
    run("git config user.name 'Author Name'", &repo);

    run("mkdir src", &repo);
    run("echo foo > src/wtf", &repo);
    run("git add .", &repo);
    run("git commit -m 'bump 0.1.0'", &repo);
    run("git tag 0.1.0", &repo);
    artefacta(local, remote)
        .arg("add-package")
        .arg("wtf-0.1.0")
        .arg(repo.join("src"))
        .succeeds();

    run("echo bar > src/wtf", &repo);
    run("git add .", &repo);
    run("git commit -m 'bump 0.1.1'", &repo);
    run("git tag 0.1.1", &repo);
    artefacta(local, remote)
        .arg("add-package")
        .arg("wtf-0.1.1")
        .arg(repo.join("src"))
        .succeeds();
    artefacta(local, remote)
        .arg("auto-patch")
        .arg("--repo-root")
        .arg(&repo)
        .arg("--prefix=wtf-")
        .arg("0.1.1")
        .succeeds();

    run("git tag -l", &repo);
    ls(&local);

    assert!(local.join("wtf-0.1.0.tar.zst").exists());
    assert!(local.join("wtf-0.1.1.tar.zst").exists());
    assert!(local.join("wtf-0.1.0---wtf-0.1.1.patch.zst").exists());
}
