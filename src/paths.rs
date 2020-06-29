use erreur::{Context, Result};
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

    // get rid of pesky .tar suffixes
    let name = name.trim_end_matches(".tar").to_string();

    Ok(name)
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
