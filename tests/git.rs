mod test_helpers;
use test_helpers::*;

#[test]
fn find_related_tags_in_repo() {
    let repo = tempdir().unwrap();
    run("git init .", &repo.path());
    run("git config user.email 'git-test@example.com'", &repo.path());
    run("git config user.name 'Author Name'", &repo.path());

    run("echo foo > wtf", &repo.path());
    run("git add .", &repo.path());
    run("git commit -m 'bump build1'", &repo.path());
    run("git tag build1", &repo.path());

    run("echo bar > wtf", &repo.path());
    run("git add .", &repo.path());
    run("git commit -m 'bump build2'", &repo.path());
    run("git tag build2", &repo.path());

    run("echo baz > wtf", &repo.path());
    run("git add .", &repo.path());
    run("git commit -m 'bump build3'", &repo.path());
    run("git tag build3", &repo.path());

    ls(&repo.path());

    run("git tag -l", &repo.path());

    let r = git2::Repository::discover(&repo.path()).unwrap();

    // let mut tags = get_tags(&r).unwrap();
    // tags.sort_by(|a, b| a.time.cmp(&b.time));
    // dbg!(tags);
}
