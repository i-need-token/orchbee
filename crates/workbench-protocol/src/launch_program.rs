//! The file name of the program a Presentation's current binding launches. It names the program
//! for display; no directory, argument or environment enters the projection.

use serde::{Deserialize, Deserializer, Serialize, de};

use crate::ContractError;

/// The byte budget of one launched-program file name.
pub const MAXIMUM_LAUNCH_PROGRAM_BYTES: usize = 128;

/// A nonempty file name of at most 128 UTF-8 bytes without a path separator or control character.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct PresentationLaunchProgram(String);

impl PresentationLaunchProgram {
    /// # Errors
    /// Rejects an empty or oversized name, a path separator and a control character.
    pub fn parse(value: &str) -> Result<Self, ContractError> {
        if value.is_empty()
            || value.len() > MAXIMUM_LAUNCH_PROGRAM_BYTES
            || value
                .chars()
                .any(|character| matches!(character, '/' | '\\') || character.is_control())
        {
            return Err(ContractError::InvalidDocument);
        }
        Ok(Self(value.to_owned()))
    }

    /// The last component of an executable path written with either separator, or `None` when
    /// that component is not a launched-program name.
    #[must_use]
    pub fn from_executable(path: &str) -> Option<Self> {
        Self::parse(path.rsplit(['/', '\\']).next().unwrap_or(path)).ok()
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for PresentationLaunchProgram {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::parse(&String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}
