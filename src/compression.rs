use erreur::{Context, Result};
use std::{
    env,
    io::{Read, Write},
};
use zstd::stream::{decode_all, write::Encoder as ZstdEncoder};

pub fn compress<W: Write>(w: W) -> Result<ZstdEncoder<W>> {
    ZstdEncoder::new(w, compression_level()).context("Can't instantiate ZSTD encoder")
}

pub fn decompress<R: Read>(r: R) -> Result<Vec<u8>> {
    decode_all(r).context("Can't read zstd compressed file")
}

const LEVEL_VAR: &str = "ARTEFACTA_COMPRESSION_LEVEL";

#[cfg(test)]
const DEFAULT_LEVEL: i32 = 14;

#[cfg(not(test))]
const DEFAULT_LEVEL: i32 = 1;

fn compression_level() -> i32 {
    if let Ok(x) = env::var(LEVEL_VAR) {
        match x.parse::<i32>() {
            Ok(x) => x,
            Err(e) => {
                log::warn!("Can't parse `{}` as integer: {}", LEVEL_VAR, e);
                DEFAULT_LEVEL
            }
        }
    } else {
        DEFAULT_LEVEL
    }
}
