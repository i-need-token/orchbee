//! Typed optional OBTF progress samples; no interpretation of task completion or Agent state.
use super::error;
use crate::{ProtocolError, ProtocolErrorCode};

/// A decoded percentage in the closed wire range 0 through 100.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalProgressValue(u8);
impl TerminalProgressValue {
    /// Returns the validated percentage.
    pub const fn percent(self) -> u8 {
        self.0
    }
}
/// Current progress state; absence of the extension has the wire meaning of `Remove`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalProgress {
    Remove,
    Set(Option<TerminalProgressValue>),
    Error(Option<TerminalProgressValue>),
    Indeterminate,
    Pause(Option<TerminalProgressValue>),
}
pub(super) fn decode(bytes: &[u8]) -> Result<TerminalProgress, ProtocolError> {
    let [state, value] = bytes else {
        return Err(error(ProtocolErrorCode::InvalidField));
    };
    let value = match value {
        255 => None,
        0..=100 => Some(TerminalProgressValue(*value)),
        _ => return Err(error(ProtocolErrorCode::InvalidField)),
    };
    Ok(match state {
        0 if value.is_none() => TerminalProgress::Remove,
        1 => TerminalProgress::Set(value),
        2 => TerminalProgress::Error(value),
        3 if value.is_none() => TerminalProgress::Indeterminate,
        4 => TerminalProgress::Pause(value),
        _ => return Err(error(ProtocolErrorCode::InvalidField)),
    })
}
