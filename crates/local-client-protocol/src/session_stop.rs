//! Bounded Session stop control facts; only the lifecycle owner grants completion authority.
use crate::ProtocolError;

/// Nonzero stop identity, retained by callers across status queries and explicit retries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StopId([u8; 16]);
impl StopId {
    /// Rejects the reserved all-zero identity.
    pub fn new(bytes: [u8; 16]) -> Result<Self, ProtocolError> {
        if bytes == [0; 16] {
            Err(ProtocolError::InvalidPayload)
        } else {
            Ok(Self(bytes))
        }
    }
    /// Returns the exact wire identity.
    pub const fn bytes(self) -> [u8; 16] {
        self.0
    }
}

/// One of at most 64 explicit attempts for a Server lifetime's accepted stop identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StopAttempt(u8);
impl StopAttempt {
    /// Admits only the inclusive range 1..=64.
    pub fn new(value: u8) -> Result<Self, ProtocolError> {
        if (1..=64).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ProtocolError::InvalidPayload)
        }
    }
    /// Returns the bounded attempt number.
    pub const fn get(self) -> u8 {
        self.0
    }
}

/// Start admits at most once; Status observes; Retry advances only a failed matching stop.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StopAction {
    Start,
    Status,
    Retry,
}

/// Control request sent after a handshake answer; admission must precede any lifecycle effect.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionStopRequest {
    pub id: StopId,
    pub action: StopAction,
}

/// Last stage reached by the lifecycle owner, not proof that a later stage completed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum StopPhase {
    Admission = 1,
    BackgroundJobs,
    Inventory,
    Force,
    Dispose,
    HostExit,
    Worker,
    WorkerExit,
    Ingress,
    Clients,
    Completion,
}

/// Closed failure facts; uncertainty never authorizes automatic mutation replay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum StopFailure {
    Deadline = 1,
    Unavailable,
    OutcomeUnknown,
    Protocol,
    Refused,
    ProcessWait,
    RecordWrite,
}

/// Observable control state. Completion is deliberately absent: only the durable receipt
/// and released lifetime lock can establish it after the control listener has closed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionStopStatus {
    NotRecorded,
    Pending {
        attempt: StopAttempt,
        phase: StopPhase,
    },
    Failed {
        attempt: StopAttempt,
        phase: StopPhase,
        failure: StopFailure,
        disposed_terminals: u32,
    },
    Busy {
        active_id: StopId,
    },
    Unmanaged,
}

/// Response correlated to the caller's exact stop identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionStopResult {
    pub id: StopId,
    pub status: SessionStopStatus,
}

pub(super) fn encode_request(request: &SessionStopRequest) -> Vec<u8> {
    let mut bytes = vec![4];
    bytes.extend_from_slice(&request.id.bytes());
    bytes.push(match request.action {
        StopAction::Start => 1,
        StopAction::Status => 2,
        StopAction::Retry => 3,
    });
    bytes
}

pub(super) fn decode_request(bytes: &[u8]) -> Result<SessionStopRequest, ProtocolError> {
    if bytes.len() != 18 {
        return Err(ProtocolError::InvalidPayload);
    }
    let id = StopId::new(
        bytes[1..17]
            .try_into()
            .map_err(|_| ProtocolError::InvalidPayload)?,
    )?;
    let action = match bytes[17] {
        1 => StopAction::Start,
        2 => StopAction::Status,
        3 => StopAction::Retry,
        _ => return Err(ProtocolError::InvalidPayload),
    };
    Ok(SessionStopRequest { id, action })
}

pub(super) fn encode_result(result: &SessionStopResult) -> Vec<u8> {
    let mut bytes = vec![5];
    bytes.extend_from_slice(&result.id.bytes());
    match result.status {
        SessionStopStatus::NotRecorded => bytes.push(1),
        SessionStopStatus::Pending { attempt, phase } => {
            bytes.extend_from_slice(&[2, attempt.get(), phase as u8])
        }
        SessionStopStatus::Failed {
            attempt,
            phase,
            failure,
            disposed_terminals,
        } => {
            bytes.extend_from_slice(&[3, attempt.get(), phase as u8, failure as u8]);
            bytes.extend_from_slice(&disposed_terminals.to_be_bytes());
        }
        SessionStopStatus::Busy { active_id } => {
            bytes.push(4);
            bytes.extend_from_slice(&active_id.bytes());
        }
        SessionStopStatus::Unmanaged => bytes.push(5),
    }
    bytes
}

pub(super) fn decode_result(bytes: &[u8]) -> Result<SessionStopResult, ProtocolError> {
    let invalid = ProtocolError::InvalidPayload;
    let id = StopId::new(
        bytes
            .get(1..17)
            .ok_or(invalid)?
            .try_into()
            .map_err(|_| invalid)?,
    )?;
    let status = match (bytes.get(17), bytes.len()) {
        (Some(1), 18) => SessionStopStatus::NotRecorded,
        (Some(2), 20) => SessionStopStatus::Pending {
            attempt: StopAttempt::new(bytes[18])?,
            phase: phase(bytes[19])?,
        },
        (Some(3), 25) => SessionStopStatus::Failed {
            attempt: StopAttempt::new(bytes[18])?,
            phase: phase(bytes[19])?,
            failure: failure(bytes[20])?,
            disposed_terminals: u32::from_be_bytes(bytes[21..25].try_into().map_err(|_| invalid)?),
        },
        (Some(4), 34) => SessionStopStatus::Busy {
            active_id: StopId::new(bytes[18..34].try_into().map_err(|_| invalid)?)?,
        },
        (Some(5), 18) => SessionStopStatus::Unmanaged,
        _ => return Err(invalid),
    };
    Ok(SessionStopResult { id, status })
}

fn phase(value: u8) -> Result<StopPhase, ProtocolError> {
    Ok(match value {
        1 => StopPhase::Admission,
        2 => StopPhase::BackgroundJobs,
        3 => StopPhase::Inventory,
        4 => StopPhase::Force,
        5 => StopPhase::Dispose,
        6 => StopPhase::HostExit,
        7 => StopPhase::Worker,
        8 => StopPhase::WorkerExit,
        9 => StopPhase::Ingress,
        10 => StopPhase::Clients,
        11 => StopPhase::Completion,
        _ => return Err(ProtocolError::InvalidPayload),
    })
}
fn failure(value: u8) -> Result<StopFailure, ProtocolError> {
    Ok(match value {
        1 => StopFailure::Deadline,
        2 => StopFailure::Unavailable,
        3 => StopFailure::OutcomeUnknown,
        4 => StopFailure::Protocol,
        5 => StopFailure::Refused,
        6 => StopFailure::ProcessWait,
        7 => StopFailure::RecordWrite,
        _ => return Err(ProtocolError::InvalidPayload),
    })
}

#[cfg(test)]
mod tests;
