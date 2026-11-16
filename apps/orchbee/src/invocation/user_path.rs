//! Inert path syntax accepts the existing Windows and absolute wire vocabularies.
//! The final execution adapter owns native expansion and supported path semantics.
use orchbee_local_runtime::{UserPathError, UserPathGrammar, UserPathOperand};
use orchbee_workbench_protocol::WorkbenchDirectoryPath;
use std::{ffi::OsString, path::PathBuf};

/// The grammar the executing CLI resolves a literal path operand in: Windows takes its drive
/// grammar, macOS and Linux the POSIX grammar.
pub(crate) const HOST_GRAMMAR: UserPathGrammar = if cfg!(windows) {
    UserPathGrammar::Windows
} else {
    UserPathGrammar::Posix
};

/// Resolves an admitted operand in `grammar` from the supplied context only.
///
/// # Errors
/// Returns the grammar's `Input` error, also for a result outside the Workbench path bound, or
/// its `Context` error.
pub(crate) fn resolve(
    value: &str,
    grammar: UserPathGrammar,
    environment: impl FnMut(&str) -> Option<OsString>,
    cwd: impl FnOnce() -> std::io::Result<PathBuf>,
) -> Result<WorkbenchDirectoryPath, UserPathError> {
    let path = grammar.normalize(value, environment, cwd)?;
    WorkbenchDirectoryPath::parse(&path).map_err(|_| UserPathError::Input)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ParsedUserPath {
    Windows(UserPathOperand),
    Absolute(WorkbenchDirectoryPath),
}

impl ParsedUserPath {
    pub(crate) fn parse(value: &str) -> Result<Self, UserPathError> {
        UserPathOperand::parse(value)
            .map(Self::Windows)
            .or_else(|_| {
                WorkbenchDirectoryPath::parse(value)
                    .map(Self::Absolute)
                    .map_err(|_| UserPathError::Input)
            })
    }

    pub(crate) fn as_str(&self) -> &str {
        match self {
            Self::Windows(path) => path.as_str(),
            Self::Absolute(path) => path.as_str(),
        }
    }
}

/// Absolute path vocabulary for inert local inputs, independent of the parsing host.
pub(super) fn absolute(value: &str) -> bool {
    let bytes = value.as_bytes();
    value.starts_with('/')
        || matches!(bytes, [drive, b':', b'/' | b'\\', ..] if drive.is_ascii_alphabetic())
        || value.strip_prefix(r"\\").is_some_and(|tail| {
            let mut parts = tail.split(['/', '\\']);
            parts.next().is_some_and(|part| !part.is_empty())
                && parts.next().is_some_and(|part| !part.is_empty())
        })
}

#[cfg(test)]
mod tests;
