use erreur::{Context, Result};
use smol_str::SmolStr;

#[derive(Debug, Clone)]
pub struct Tag {
    name: String,
    time: chrono::NaiveDateTime,
    id: git2::Oid,
}

pub fn get_tags(repo: &git2::Repository) -> Result<Vec<Tag>> {
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

/// assume versions are in format `….c.b.a` (or `…-c-b-a`)
pub fn find_tags_to_patch(current: &str, tags: &[String]) -> Result<Vec<String>> {
    fn dec(x: &SmolStr) -> Option<SmolStr> {
        let num = x.parse::<u32>().ok()?;
        let prev = num.checked_sub(1)?;
        Some(SmolStr::from(prev.to_string()))
    }

    fn tag_to_slice(tag: &str) -> Vec<SmolStr> {
        tag.split(|c| c == '.' || c == '-')
            .map(SmolStr::from)
            .collect()
    }

    let parsed_tags = tags.iter().map(|tag| tag_to_slice(tag)).collect::<Vec<_>>();
    let current = tag_to_slice(current);

    let mut to_patch = vec![];

    if let Some(x) = current.iter().rev().next().and_then(dec) {
        let mut prev = current.clone();
        let pos = current.len() - 1;
        prev[pos] = x;
        if let Some((idx, _)) = parsed_tags
            .iter()
            .enumerate()
            .find(|(_idx, tag)| tag == &&prev)
        {
            to_patch.push(tags[idx].clone());
        } else {
            log::debug!("no matching tag for {:?}", prev);
        }
    }

    if let Some(x) = current.iter().rev().nth(1).and_then(dec) {
        let mut prev = current.clone();
        let pos = current.len() - 2;
        prev[pos] = x;
        if let Some((idx, _)) = parsed_tags
            .iter()
            .enumerate()
            .filter(|(_idx, tag)| tag.starts_with(&prev[..=pos]))
            .last()
        {
            to_patch.push(tags[idx].clone());
        } else {
            log::debug!("no matching tag for {:?}", prev);
        }
    }

    Ok(to_patch)
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
