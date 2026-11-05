//! Validates the OBTF OSC 7501 program status extension: the root-record state a program reported
//! about itself and when the Host received it. It does not decide Agent state or freshness.

use super::{Reader, error};
use crate::{ProtocolError, ProtocolErrorCode};
use std::num::NonZeroU64;

/// A program's self-reported state from the OSC 7501 Program Status protocol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalProgramState {
    Idle,
    Working,
    Blocked,
    Done,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalProgramStatus {
    pub state: TerminalProgramState,
    /// Real-time Unix nanoseconds read after the Host received the report.
    pub reported_at_nanoseconds: NonZeroU64,
}

pub(super) fn decode(bytes: &[u8]) -> Result<TerminalProgramStatus, ProtocolError> {
    if bytes.len() != 9 {
        return Err(error(ProtocolErrorCode::InvalidField));
    }
    let mut reader = Reader::new(bytes);
    let state = match reader.u8()? {
        1 => TerminalProgramState::Idle,
        2 => TerminalProgramState::Working,
        3 => TerminalProgramState::Blocked,
        4 => TerminalProgramState::Done,
        5 => TerminalProgramState::Error,
        _ => return Err(error(ProtocolErrorCode::InvalidField)),
    };
    Ok(TerminalProgramStatus {
        state,
        reported_at_nanoseconds: NonZeroU64::new(reader.u64()?)
            .ok_or_else(|| error(ProtocolErrorCode::InvalidField))?,
    })
}
