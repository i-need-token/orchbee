use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

pub const MAXIMUM_DIRECTORY_PATH_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkbenchDirectoryPathError;

impl fmt::Display for WorkbenchDirectoryPathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "directory path must be an absolute POSIX UTF-8 path of 1 through 4096 bytes without NUL",
        )
    }
}

impl std::error::Error for WorkbenchDirectoryPathError {}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkbenchDirectoryPath(String);

impl WorkbenchDirectoryPath {
    pub fn parse(value: &str) -> Result<Self, WorkbenchDirectoryPathError> {
        if !value.starts_with('/')
            || value.len() > MAXIMUM_DIRECTORY_PATH_BYTES
            || value.contains('\0')
        {
            return Err(WorkbenchDirectoryPathError);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for WorkbenchDirectoryPath {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for WorkbenchDirectoryPath {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(de::Error::custom)
    }
}
