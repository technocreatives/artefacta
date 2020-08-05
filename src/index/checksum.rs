use erreur::{ensure, Result};
use hex_fmt::HexFmt;
use sha2::Digest;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Checksum {
    Sha256([u8; 32]),
}

impl Checksum {
    pub fn validate(&self, buf: &[u8]) -> Result<()> {
        match self {
            Checksum::Sha256(expected) => {
                let expected = &expected[..];
                let got = sha2::Sha256::digest(buf);
                let got = &got[..];
                ensure!(
                    got == expected,
                    "checksum mismatch, got `{}`, expected `{}`",
                    HexFmt(got),
                    HexFmt(expected),
                );
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_sha256() {
        let checksum = Checksum::Sha256([
            7, 18, 62, 31, 72, 35, 86, 196, 21, 246, 132, 64, 122, 59, 135, 35, 225, 11, 44, 187,
            192, 184, 252, 214, 40, 44, 73, 211, 124, 156, 26, 188,
        ]);
        checksum.validate(b"lol").unwrap();
    }
}
