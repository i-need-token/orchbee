//! Decodes the closed OBTF shell-observation extension into Host row-origin, readiness, and
//! correlated-command facts. It does not infer a command from screen text or decide whether a
//! Client operation completed.

use super::{Reader, error};
use crate::{ProtocolError, ProtocolErrorCode};
use std::num::NonZeroU64;

/// Host-owned row origin and a shell's current readiness and latest correlated command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalCommandObservation {
    pub output_epoch: NonZeroU64,
    pub ready: bool,
    pub command: Option<TerminalObservedCommand>,
}

/// A shell-echoed invocation identity; unrelated completion facts cannot satisfy it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalObservedCommand {
    pub id: [u8; 16],
    pub completed: bool,
    pub exit_code: i32,
    pub start_row: u64,
    pub start_column: u16,
    pub end_row: u64,
    pub end_column: u16,
    pub generation: NonZeroU64,
    pub output_epoch: NonZeroU64,
}

pub(super) fn decode(bytes: &[u8]) -> Result<TerminalCommandObservation, ProtocolError> {
    let invalid = || error(ProtocolErrorCode::InvalidField);
    if !matches!(bytes.len(), 10 | 66) {
        return Err(invalid());
    }
    let mut reader = Reader::new(bytes);
    let output_epoch = NonZeroU64::new(reader.u64()?).ok_or_else(invalid)?;
    let ready = match reader.u8()? {
        0 => false,
        1 => true,
        _ => return Err(invalid()),
    };
    let phase = reader.u8()?;
    let command = match phase {
        0 if bytes.len() == 10 => None,
        1 | 2 if bytes.len() == 66 => {
            let id: [u8; 16] = reader.take(16)?.try_into().map_err(|_| invalid())?;
            if id == [0; 16] {
                return Err(invalid());
            }
            let command = TerminalObservedCommand {
                id,
                completed: phase == 2,
                start_row: reader.u64()?,
                start_column: reader.u16()?,
                end_row: reader.u64()?,
                end_column: reader.u16()?,
                generation: NonZeroU64::new(reader.u64()?).ok_or_else(invalid)?,
                output_epoch: NonZeroU64::new(reader.u64()?).ok_or_else(invalid)?,
                exit_code: i32::from_le_bytes(reader.take(4)?.try_into().map_err(|_| invalid())?),
            };
            if !command.completed
                && (command.exit_code != 0 || command.end_row != 0 || command.end_column != 0)
            {
                return Err(invalid());
            }
            Some(command)
        }
        _ => return Err(invalid()),
    };
    Ok(TerminalCommandObservation {
        output_epoch,
        ready,
        command,
    })
}
