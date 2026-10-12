//! Extension 4 is the same closed 25-byte OSC 133 completion fact consumed by the GUI.
use super::{Reader, error};
use crate::{ProtocolError, ProtocolErrorCode};
use std::num::NonZeroU64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalCommandCompletion {
    pub sequence: NonZeroU64,
    pub duration_milliseconds: u64,
    pub completed_at_milliseconds: NonZeroU64,
    pub exit_code: u8,
}

pub(super) fn decode(bytes: &[u8]) -> Result<TerminalCommandCompletion, ProtocolError> {
    if bytes.len() != 25 {
        return Err(error(ProtocolErrorCode::InvalidField));
    }
    let mut reader = Reader::new(bytes);
    Ok(TerminalCommandCompletion {
        sequence: NonZeroU64::new(reader.u64()?)
            .ok_or_else(|| error(ProtocolErrorCode::InvalidField))?,
        duration_milliseconds: reader.u64()?,
        completed_at_milliseconds: NonZeroU64::new(reader.u64()?)
            .ok_or_else(|| error(ProtocolErrorCode::InvalidField))?,
        exit_code: reader.u8()?,
    })
}
