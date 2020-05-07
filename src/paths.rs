use erreur::{ensure, Context, Result};
use std::{convert::TryFrom, path::Path};

use crate::index::Version;

pub fn path_as_string(path: impl AsRef<Path>) -> Result<String> {
    let path = path.as_ref();
    Ok(path
        .to_str()
        .with_context(|| format!("file name `{:?}` not valid UTF-8", path))?
        .to_string())
}

pub fn file_name(path: impl AsRef<Path>) -> Result<String> {
    let path = path.as_ref();
    let file_name = path
        .file_stem()
        .with_context(|| format!("no file stem for `{:?}`", path))?;
    let name = path_as_string(file_name)?;
    let name = name
        .splitn(2, '.')
        .next()
        .expect("`splitn` always returns at least one item");
    Ok(name.to_string())
}

pub fn build_path_from_version(v: Version) -> Result<String> {
    Ok(format!("{}.tar.zst", v.as_str()))
}

pub fn build_version_from_path(path: impl AsRef<Path>) -> Result<Version> {
    let path = path.as_ref();
    let name = file_name(path).with_context(|| format!("get name of `{:?}`", path))?;
    Version::try_from(&name)
        .with_context(|| format!("parse name `{}` from path `{:?}` as version", name, path))
}

pub fn patch_versions_from_path(path: impl AsRef<Path>) -> Result<(Version, Version)> {
    let path = path.as_ref();
    let name = file_name(path).with_context(|| format!("get name of `{:?}`", path))?;
    let parts: Vec<&str> = name.splitn(2, '-').collect();
    ensure!(
        parts.len() == 2,
        "patch file name pattern is not `<hash>-<hash>`: `{}`",
        name,
    );
    Version::try_from(parts[0])
        .into_iter()
        .zip(Version::try_from(parts[1]))
        .next()
        .with_context(|| format!("parse name `{}` from path `{:?}` as version", name, path))
}
