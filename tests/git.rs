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

    let mut tags = get_tags(&r).unwrap();
    tags.sort_by(|a, b| a.time.cmp(&b.time));
    dbg!(tags);
}

#[derive(Debug, Clone)]
struct Tag {
    name: String,
    time: chrono::NaiveDateTime,
    id: git2::Oid,
}

fn get_tags(repo: &git2::Repository) -> Result<Vec<Tag>> {
    repo.references()
        .context("cannot load references from repo")?
        .filter_map(|x| x.ok())
        .filter(|r| r.is_tag())
        .map(|reference| {
            let commit = reference.peel_to_commit().with_context(|| {
                format!("cannot get commit for reference {:?}", reference.name())
            })?;
            Ok(Tag {
                name: reference.shorthand().map(String::from).unwrap_or_else(|| {
                    String::from_utf8_lossy(reference.shorthand_bytes()).to_string()
                }),
                time: chrono::NaiveDateTime::from_timestamp_opt(commit.time().seconds(), 0)
                    .context("cannot read commit time")?,
                id: commit.id(),
            })
        })
        .collect()
}

#[test]
fn tags_to_patch_from() {
    let tags = vec![
        "IL40.0.0".to_string(),
        "IL40.0.1".to_string(),
        "IL40.1.0".to_string(),
        "IL40.2.17".to_string(),
        "IL40.2.18".to_string(),
    ];
    let current_tag = "IL40.2.19";
    let patch_these = find_tags_to_patch(current_tag, &tags).unwrap();
    assert_eq!(
        patch_these,
        vec!["IL40.2.18".to_string(), "IL40.1.0".to_string()]
    );
}

fn find_tags_to_patch(current: &str, tags: &[String]) -> Result<Vec<String>> {
    use smol_str::SmolStr;
    use std::collections::BTreeMap;

    fn tag_to_slice(tag: &str) -> Vec<SmolStr> {
        tag.split('.').map(SmolStr::from).collect()
    }

    let current = tag_to_slice(current);
    let parsed_tags: Vec<Vec<SmolStr>> = tags.iter().map(|tag| tag_to_slice(tag)).collect();

    let mut to_patch = vec![];

    if let Some(last_num) = current.iter().last() {
        todo!("sub to max 0");
    }

    if let Some(second_to_last_num) = current.iter().rev().nth(1) {
        todo!("sub to max 0");
    }

    Ok(to_patch)
}
