//! Defines bounded reads of committed Workbench publications and explicit fallback or
//! unavailable outcomes. It carries journal observations without retaining publications,
//! applying them, or advancing a Client cursor.

use serde::{Deserialize, Serialize};

use crate::{
    identity::{ProtocolVersion, WorkbenchRequestId, WorkbenchRevision},
    model::WorkbenchPublicationDocument,
};

/// The maximum number of committed publications one `published` outcome may carry. The Server
/// journal retains at most this many entries, so a covered read never exceeds the wire list bound.
pub const MAXIMUM_RESULT_PUBLICATIONS: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublicationReadIntent {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub known_revision: WorkbenchRevision,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum WorkbenchPublicationOutcome {
    Current {
        revision: WorkbenchRevision,
    },
    Published {
        publications: Vec<WorkbenchPublicationDocument>,
    },
    Denied {},
    Unavailable {
        reason: crate::command::WorkbenchSnapshotUnavailableReason,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchPublicationResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub outcome: WorkbenchPublicationOutcome,
}
