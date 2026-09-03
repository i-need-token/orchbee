use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, de};

pub const MAXIMUM_WORKING_DIRECTORY_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkingDirectoryError {
    NotAbsolute,
    ContainsNul,
    TooLong { actual: usize, maximum: usize },
}

impl fmt::Display for WorkingDirectoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAbsolute => {
                formatter.write_str("working directory must be an absolute POSIX path")
            }
            Self::ContainsNul => formatter.write_str("working directory must not contain NUL"),
            Self::TooLong { actual, maximum } => write!(
                formatter,
                "working directory has {actual} UTF-8 bytes; maximum is {maximum}"
            ),
        }
    }
}

impl std::error::Error for WorkingDirectoryError {}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct WorkingDirectory(String);

impl WorkingDirectory {
    pub fn parse(value: &str) -> Result<Self, WorkingDirectoryError> {
        if !value.starts_with('/') {
            return Err(WorkingDirectoryError::NotAbsolute);
        }
        if value.contains('\0') {
            return Err(WorkingDirectoryError::ContainsNul);
        }
        if value.len() > MAXIMUM_WORKING_DIRECTORY_BYTES {
            return Err(WorkingDirectoryError::TooLong {
                actual: value.len(),
                maximum: MAXIMUM_WORKING_DIRECTORY_BYTES,
            });
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for WorkingDirectory {
    fn deserialize<DeserializerType>(
        deserializer: DeserializerType,
    ) -> Result<Self, DeserializerType::Error>
    where
        DeserializerType: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(de::Error::custom)
    }
}

impl fmt::Display for WorkingDirectory {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests;
