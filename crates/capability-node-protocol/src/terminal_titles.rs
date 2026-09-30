//! Read-only physical title observations, separate from viewer and Agent authority.
use crate::{HostTerminalIdentity, OperationId, ProtocolError, WorkerGeneration, WorkerId};
use std::num::NonZeroU64;

pub const MAXIMUM_TERMINAL_TITLE_ROWS: usize = 4;
pub const MAXIMUM_TERMINAL_TITLE_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalTitlesRequest {
    pub request_id: OperationId,
    pub worker_id: WorkerId,
    pub worker_generation: WorkerGeneration,
    pub identities: Vec<HostTerminalIdentity>,
}
impl TerminalTitlesRequest {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.identities.len() > MAXIMUM_TERMINAL_TITLE_ROWS
            || self.identities.iter().enumerate().any(|(i, id)| {
                self.identities[..i]
                    .iter()
                    .any(|prior| prior.terminal_id() == id.terminal_id())
            })
        {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalTitleObservation {
    Unavailable,
    Observed {
        revision: NonZeroU64,
        title: Option<String>,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalTitleRow {
    pub identity: HostTerminalIdentity,
    pub observation: TerminalTitleObservation,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalTitlesResult {
    pub request_id: OperationId,
    pub rows: Vec<TerminalTitleRow>,
}
impl TerminalTitlesResult {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.rows.len() > MAXIMUM_TERMINAL_TITLE_ROWS
            || self.rows.iter().enumerate().any(|(i, row)| {
                self.rows[..i].iter().any(|p| p.identity.terminal_id() == row.identity.terminal_id())
                    || matches!(&row.observation, TerminalTitleObservation::Observed { title: Some(title), .. }
                        if title.len() > MAXIMUM_TERMINAL_TITLE_BYTES)
            })
        {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(())
    }
    pub fn answers(&self, request: &TerminalTitlesRequest) -> bool {
        self.validate().is_ok()
            && request.validate().is_ok()
            && self.request_id == request.request_id
            && request
                .identities
                .iter()
                .eq(self.rows.iter().map(|r| &r.identity))
    }
}
