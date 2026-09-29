//! A per-launch policy, distinct from an already resolved explicit executable.
use crate::ProtocolError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultShellMode {
    Auto,
    Login,
    NonLogin,
}
impl DefaultShellMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Login => "login",
            Self::NonLogin => "non_login",
        }
    }
}

/// The wire owns byte bounds; the Worker owns platform syntax and resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefaultShellSelection {
    executable: String,
    mode: DefaultShellMode,
}
impl DefaultShellSelection {
    pub fn new(executable: String, mode: DefaultShellMode) -> Result<Self, ProtocolError> {
        if executable.len() > 4096 || executable.chars().any(char::is_control) {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self { executable, mode })
    }
    pub fn executable(&self) -> &str {
        &self.executable
    }
    pub const fn mode(&self) -> DefaultShellMode {
        self.mode
    }
}
