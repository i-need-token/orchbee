//! Validated case-sensitive Session selection names; platform adapters own scope derivation and private identity records.
use std::io;
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionName(String);

impl SessionName {
    pub fn parse(name: &str) -> io::Result<Self> {
        if name.is_empty()
            || name.len() > 64
            || matches!(name, "." | "..")
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid Session name",
            ));
        }
        Ok(Self(name.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use windows::{NAMED_PREFIX, claimed, read, register, validate_scope};
