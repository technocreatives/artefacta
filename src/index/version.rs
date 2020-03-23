use anyhow::Result;
use std::{convert::TryFrom, fmt, str::FromStr};

/// Short string in specific format. Cheap to clone.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Version {
    data: smol_str::SmolStr,
}

impl Version {
    pub fn as_str(&self) -> &str {
        self.data.as_str()
    }
}

impl fmt::Debug for Version {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_tuple("Version").field(&self.as_str()).finish()
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.data)
    }
}

impl FromStr for Version {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // TODO: Validate for specific format
        Ok(Version { data: s.into() })
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
