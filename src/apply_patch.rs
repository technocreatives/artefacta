use anyhow::{Context, Result};
use std::{
    fs::File,
    io::{BufReader, Cursor, Read},
    path::Path,
};
use zstd::stream::read::Decoder as ZstdDecoder;

pub fn apply_patch(archive: &Path, patch: &Path) -> Result<impl Read> {
    let patch_file =
        File::open(patch).with_context(|| format!("open file `{}`", patch.display()))?;
    let patch_decompressed = ZstdDecoder::new(patch_file)
        .with_context(|| format!("read zstd compressed file `{}`", patch.display()))?;

    let archive_file =
        File::open(archive).with_context(|| format!("open file `{}`", archive.display()))?;
    let archive_decompressed = zstd::stream::decode_all(BufReader::new(archive_file))
        .with_context(|| format!("read zstd compressed file `{}`", archive.display()))?;

    bipatch::Reader::new(patch_decompressed, Cursor::new(archive_decompressed))
        .context("read patch")
}
