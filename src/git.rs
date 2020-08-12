use erreur::{Context, Result};
use smol_str::SmolStr;

#[derive(Debug, Clone)]
pub struct Tag {
    pub name: String,
    pub time: chrono::NaiveDateTime,
    pub id: git2::Oid,
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

pub fn tag_to_slice(tag: &str) -> Vec<SmolStr> {
    tag.to_lowercase()
        .split(|c| c == '.' || c == '-')
        .map(SmolStr::from)
        .collect()
}

/// assume versions are in format `….c.b.a` (or `…-c-b-a`)
pub fn find_tags_to_patch(current: &str, tags: &[String]) -> Result<Vec<String>> {
    fn dec(x: &SmolStr) -> Option<SmolStr> {
        let num = x.parse::<u32>().ok()?;
        let prev = num.checked_sub(1)?;
        Some(SmolStr::from(prev.to_string()))
    }

    let tags = {
        let mut tags = tags.to_vec();
        tags.sort_by(|a, b| human_sort::compare(a, b));
        tags
    };
    let parsed_tags = tags.iter().map(|tag| tag_to_slice(tag)).collect::<Vec<_>>();
    let current = tag_to_slice(current);
    let to_patch: Vec<String> = (0..current.len())
        .filter_map(|pos_from_end| {
            if let Some(x) = current.iter().rev().nth(pos_from_end).and_then(dec) {
                let pos = current.len() - pos_from_end - 1;
                let prev = {
                    let mut prev = current.to_vec();
                    prev[pos] = x;
                    prev
                };

                if let Some((idx, _)) = parsed_tags
                    .iter()
                    .enumerate()
                    .filter(|(_idx, tag)| tag.starts_with(&prev[..=pos]))
                    .last()
                {
                    return Some(tags[idx].clone());
                } else {
                    log::debug!("no matching tag for {:?}", prev);
                }
            }
            None
        })
        .collect();

    Ok(to_patch)
}

#[test]
fn tags_to_patch_from_works() {
    crate::test_helpers::logger();
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

#[test]
fn tags_to_patch_from_1() {
    crate::test_helpers::logger();
    let tags = vec![];
    let current_tag = "IL40.2.19";
    let patch_these = find_tags_to_patch(current_tag, &tags).unwrap();
    assert!(patch_these.is_empty());
}

#[test]
fn tags_to_patch_from_2() {
    crate::test_helpers::logger();
    let tags = vec!["garbage".to_string(), "v1.5-1.beta.1".to_string()];
    let current_tag = "v2.0.0";
    let patch_these = find_tags_to_patch(current_tag, &tags).unwrap();
    assert!(patch_these.is_empty());
}

#[test]
fn tags_to_patch_from_3() {
    crate::test_helpers::logger();
    let tags = vec![
        "IL40.0.0".to_string(),
        "IL40.0.1".to_string(),
        "IL40.1.x".to_string(),
        "IL40.2.17".to_string(),
        "IL40.2.18".to_string(),
    ];
    let current_tag = "IL40.2.19";
    let patch_these = find_tags_to_patch(current_tag, &tags).unwrap();
    assert_eq!(
        patch_these,
        vec!["IL40.2.18".to_string(), "IL40.1.x".to_string()]
    );
}

#[test]
fn tags_to_patch_from_4() {
    let tags = vec![
        "IL40.0.1".to_string(),
        "IL40.1.0".to_string(),
        "IL40.2.17".to_string(),
        "IL40.2.18".to_string(),
        "IL40.x.0".to_string(),
    ];
    let current_tag = "IL40.2.19";
    let patch_these = find_tags_to_patch(current_tag, &tags).unwrap();
    assert_eq!(
        patch_these,
        vec!["IL40.2.18".to_string(), "IL40.1.0".to_string()]
    );
}

#[test]
fn tags_to_patch_from_fuzzy() {
    let tags = vec![
        "IL40.0.1".to_string(),
        "IL40.1.0".to_string(),
        "IL40.2.17".to_string(),
        "IL40.2.18".to_string(),
        "IL40.x.0".to_string(),
    ];
    let current_tag = "il40-2-19";
    let patch_these = find_tags_to_patch(current_tag, &tags).unwrap();
    assert_eq!(
        patch_these,
        vec!["IL40.2.18".to_string(), "IL40.1.0".to_string()]
    );
}

#[test]
fn tags_to_patch_from_5() {
    let tags = vec![
        "il60-0-8".to_string(),
        "il60-0-9".to_string(),
        "il60-0-10".to_string(),
        "il60-0-11".to_string(),
    ];
    let current_tag = "il60-1-0";
    let patch_these = find_tags_to_patch(current_tag, &tags).unwrap();
    assert_eq!(patch_these, vec!["il60-0-11".to_string()]);
}

#[test]
fn tags_to_patch_from_5_sorted() {
    let tags = vec![
        "il60-0-10".to_string(),
        "il60-0-11".to_string(),
        "il60-0-8".to_string(),
        "il60-0-9".to_string(),
    ];
    let current_tag = "il60-1-0";
    let patch_these = find_tags_to_patch(current_tag, &tags).unwrap();
    assert_eq!(patch_these, vec!["il60-0-11".to_string()]);
}
