//! An admitted Client asks the Server to stop its Session, as `server stop` does: the Server records
//! the stop under the Client's identity, runs it once, and ends every terminal, the Worker, every
//! Client connection, and itself. Recording the stop refuses every later record, this connection's
//! included, so its progress and completion are read with `server status`, never here.
use crate::{ProtocolVersion, SessionId, WorkbenchRequestId, WorkbenchStopId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchSessionStopRequest {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    #[serde(rename = "sessionId")]
    pub session_id: SessionId,
    /// The identity `server status` reads the stop under, chosen by the requester.
    pub stop_id: WorkbenchStopId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionStopRejection {
    SessionMismatch,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SessionStopOutcome {
    /// The Server holds a stop under the requested identity and runs it once.
    Recorded {},
    /// Another identity's stop holds the Session; nothing was recorded.
    Busy {},
    /// No lifecycle owner can stop this Session; nothing was recorded.
    Unmanaged {},
    Rejected {
        reason: SessionStopRejection,
    },
    /// This Client connection carries no authority to stop the Session.
    Denied {},
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchSessionStopResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub outcome: SessionStopOutcome,
}
