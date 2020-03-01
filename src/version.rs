use anyhow::Result;
use std::{convert::TryFrom, fmt, str::FromStr};

const MAX_LENGTH: usize = 23;

/// Super-short string, all data stored inline
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Version {
    len: u8,
    data: [u8; MAX_LENGTH],
}

impl Version {
    pub fn as_str(&self) -> &str {
        // Data is a valid subset of an UTF-8 string by construction
        unsafe { std::str::from_utf8_unchecked(&self.data[..self.len as usize]) }
    }
}

impl fmt::Debug for Version {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_tuple("Version")
            .field(&self.as_str())
            .finish()
    }
}

impl FromStr for Version {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        anyhow::ensure!(
            s.len() <= MAX_LENGTH,
            "string `{}` too long to use as version",
            s,
        );

        let mut v = [0u8; MAX_LENGTH];
        let len = std::cmp::min(s.len(), MAX_LENGTH);

        anyhow::ensure!(
            s.is_char_boundary(len),
            "version string too long and cut-off point not at char boundary",
        );

        s.as_bytes()[..len]
            .iter()
            .enumerate()
            .for_each(|(i, c)| v[i] = *c);

        Ok(Version {
            len: len as u8,
            data: v,
        })
    }
}

impl<'a> TryFrom<&'a str> for Version {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<Self> {
        s.parse()
    }
}

impl<'a> TryFrom<&'a String> for Version {
    type Error = anyhow::Error;

    fn try_from(s: &String) -> Result<Self> {
        s.parse()
    }
}


#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use super::*;

    proptest! {
        #[test]
        fn version_from_str_then_print(s in "\\PC*") {
            if let Ok(v) = Version::from_str(&s) {
                let _x = v.as_str();
            }
        }
    }
}
