//! Bounded literal launch environment. This boundary validates names and byte
//! budgets without reading process state, expanding values, or exposing secrets.
use crate::ProtocolError;
use std::fmt;

/// Maximum caller-supplied entries in one launch.
pub const MAXIMUM_LAUNCH_ENVIRONMENT_ENTRIES: usize = 32;
/// Combined UTF-8 name/value bytes, leaving space for maximum cwd and argv.
pub const MAXIMUM_LAUNCH_ENVIRONMENT_BYTES: usize = 1024;

/// Validated literal overlay; empty values are retained, duplicate or managed
/// names are rejected ignoring ASCII case. Debug output never includes values.
#[derive(Clone, Default, Eq, PartialEq)]
pub struct LaunchEnvironment(Vec<(String, String)>);

impl LaunchEnvironment {
    /// An overlay with no caller-supplied entries.
    #[must_use]
    pub const fn empty() -> Self {
        Self(Vec::new())
    }

    /// Validate the complete overlay before admission. No partial overlay escapes.
    /// # Errors
    /// Rejects malformed/reserved/duplicate names, NUL, count and byte overflow.
    pub fn new(entries: Vec<(String, String)>) -> Result<Self, ProtocolError> {
        if entries.len() > MAXIMUM_LAUNCH_ENVIRONMENT_ENTRIES {
            return Err(ProtocolError::InvalidPayload);
        }
        let mut bytes = 0usize;
        for (index, (name, value)) in entries.iter().enumerate() {
            let mut letters = name.bytes();
            if name.len() > 128
                || !letters
                    .next()
                    .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
                || !letters.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
                || value.contains('\0')
                || name.to_ascii_uppercase().starts_with("ORCHBEE_")
                || ["TERM", "COLORTERM", "TERM_PROGRAM"]
                    .iter()
                    .any(|reserved| name.eq_ignore_ascii_case(reserved))
                || entries[..index]
                    .iter()
                    .any(|(previous, _)| name.eq_ignore_ascii_case(previous))
            {
                return Err(ProtocolError::InvalidPayload);
            }
            bytes = bytes
                .checked_add(name.len())
                .and_then(|n| n.checked_add(value.len()))
                .ok_or(ProtocolError::InvalidPayload)?;
            if bytes > MAXIMUM_LAUNCH_ENVIRONMENT_BYTES {
                return Err(ProtocolError::InvalidPayload);
            }
        }
        Ok(Self(entries))
    }

    /// Literal name/value pairs in submitted order; keys have unique Windows identity.
    #[must_use]
    pub fn entries(&self) -> &[(String, String)] {
        &self.0
    }
}

impl fmt::Debug for LaunchEnvironment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LaunchEnvironment")
            .field(
                "names",
                &self.0.iter().map(|(name, _)| name).collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
