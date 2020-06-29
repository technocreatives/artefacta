mod test_helpers;
use test_helpers::*;

#[test]
fn find_related_tags_in_repo() {
    let repo = tempdir().unwrap();
    run("git init .", &repo.path());

    run("echo foo > wtf", &repo.path());
    run("git add .", &repo.path());
    run("git commit -m 'bump build1'", &repo.path());
    run("git tag build1", &repo.path());

    run("echo bar > wtf", &repo.path());
    run("git add .", &repo.path());
    run("git commit -m 'bump build2'", &repo.path());
    run("git tag build2", &repo.path());

    ls(&repo.path());

    run("git tag -l", &repo.path());

    let r = git2::Repository::discover(&repo.path()).unwrap();

    let mut tags = get_tags(&r).unwrap();
    tags.sort_by(|a, b| a.time.cmp(&b.time));
    dbg!(tags);
    // let tags = r.tag_names(None).unwrap();
    // dbg!(tags.iter().flatten().collect::<Vec<_>>());

    // for tag in tags.iter() {
    //     let tag = tag.unwrap();
    //     // let r = r.find_reference(tag).unwrap();
    //     // if r.is_tag() { continue; }
    //     // dbg!(r.name());
    // }

    // let refs = r
    //     .references()
    //     .unwrap()
    //     .filter_map(|x| x.ok())
    //     .filter(|r| r.is_tag());
    // for reference in refs {
    //     dbg!(reference.name(), reference.shorthand());
    //     let tag = reference.peel_to_commit().unwrap();
    //     dbg!(tag.summary());
    // }
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
