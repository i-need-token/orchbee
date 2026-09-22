use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// Maximum argument values one explicit terminal launch selection carries.
/// The selection's argument list is argv[1..] — the Host synthesizes argv[0]
/// from the executable; argv0 is never transmitted.
pub const MAXIMUM_LAUNCH_SELECTION_ARGUMENTS: usize = 128;
/// Maximum combined UTF-8 bytes of the executable path plus every argument
/// value in one explicit launch selection.
pub const MAXIMUM_LAUNCH_SELECTION_BYTES: usize = 2048;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LaunchSelectionError {
    EmptyExecutable,
    ExecutableNotAbsolute,
    ContainsNul,
    TooManyArguments { actual: usize, maximum: usize },
    TooLong { actual: usize, maximum: usize },
}

impl fmt::Display for LaunchSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyExecutable => formatter.write_str("launch executable must not be empty"),
            Self::ExecutableNotAbsolute => {
                formatter.write_str("launch executable must be an absolute local-platform path")
            }
            Self::ContainsNul => {
                formatter.write_str("launch executable and arguments must not contain NUL")
            }
            Self::TooManyArguments { actual, maximum } => write!(
                formatter,
                "launch selection has {actual} arguments; maximum is {maximum}"
            ),
            Self::TooLong { actual, maximum } => write!(
                formatter,
                "launch selection has {actual} UTF-8 bytes; maximum is {maximum}"
            ),
        }
    }
}

impl std::error::Error for LaunchSelectionError {}

/// An explicit terminal launch program authorized end-to-end: one absolute
/// executable path and its verbatim argument list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchSelection {
    executable: String,
    arguments: Vec<String>,
}

impl LaunchSelection {
    pub fn new(executable: String, arguments: Vec<String>) -> Result<Self, LaunchSelectionError> {
        if executable.is_empty() {
            return Err(LaunchSelectionError::EmptyExecutable);
        }
        if !crate::working_directory::is_absolute_path(executable.as_bytes()) {
            return Err(LaunchSelectionError::ExecutableNotAbsolute);
        }
        if executable.contains('\0') || arguments.iter().any(|argument| argument.contains('\0')) {
            return Err(LaunchSelectionError::ContainsNul);
        }
        if arguments.len() > MAXIMUM_LAUNCH_SELECTION_ARGUMENTS {
            return Err(LaunchSelectionError::TooManyArguments {
                actual: arguments.len(),
                maximum: MAXIMUM_LAUNCH_SELECTION_ARGUMENTS,
            });
        }
        let bytes = executable.len() + arguments.iter().map(String::len).sum::<usize>();
        if bytes > MAXIMUM_LAUNCH_SELECTION_BYTES {
            return Err(LaunchSelectionError::TooLong {
                actual: bytes,
                maximum: MAXIMUM_LAUNCH_SELECTION_BYTES,
            });
        }
        Ok(Self {
            executable,
            arguments,
        })
    }

    #[must_use]
    pub fn executable(&self) -> &str {
        &self.executable
    }

    #[must_use]
    pub fn arguments(&self) -> &[String] {
        &self.arguments
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LaunchSelectionRef<'a> {
    exec: &'a str,
    args: &'a [String],
}

impl Serialize for LaunchSelection {
    fn serialize<SerializerType>(
        &self,
        serializer: SerializerType,
    ) -> Result<SerializerType::Ok, SerializerType::Error>
    where
        SerializerType: Serializer,
    {
        LaunchSelectionRef {
            exec: &self.executable,
            args: &self.arguments,
        }
        .serialize(serializer)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OwnedLaunchSelection {
    exec: String,
    args: Vec<String>,
}

impl<'de> Deserialize<'de> for LaunchSelection {
    fn deserialize<DeserializerType>(
        deserializer: DeserializerType,
    ) -> Result<Self, DeserializerType::Error>
    where
        DeserializerType: Deserializer<'de>,
    {
        let value = OwnedLaunchSelection::deserialize(deserializer)?;
        Self::new(value.exec, value.args).map_err(de::Error::custom)
    }
}

#[cfg(test)]
mod tests;
