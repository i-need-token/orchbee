use serde::{Deserialize, Serialize};

use crate::{CreateFailureReason, CreateUnavailableReason, Geometry, RequestId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalOpenRequest {
    pub(crate) request_id: RequestId,
    pub(crate) geometry: Geometry,
}

impl TerminalOpenRequest {
    #[must_use]
    pub const fn new(request_id: RequestId, geometry: Geometry) -> Self {
        Self {
            request_id,
            geometry,
        }
    }

    #[must_use]
    pub const fn request_id(&self) -> &RequestId {
        &self.request_id
    }

    #[must_use]
    pub const fn geometry(&self) -> Geometry {
        self.geometry
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum OpenOutcome {
    Opened {
        terminal_create_request_id: RequestId,
    },
    Unavailable {
        reason: CreateUnavailableReason,
    },
    Failed {
        reason: CreateFailureReason,
    },
    OutcomeUnknown {},
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalOpenResult {
    pub(crate) request_id: RequestId,
    pub(crate) outcome: OpenOutcome,
}

impl TerminalOpenResult {
    #[must_use]
    pub const fn new(request_id: RequestId, outcome: OpenOutcome) -> Self {
        Self {
            request_id,
            outcome,
        }
    }

    #[must_use]
    pub const fn request_id(&self) -> &RequestId {
        &self.request_id
    }

    #[must_use]
    pub const fn outcome(&self) -> &OpenOutcome {
        &self.outcome
    }

    #[must_use]
    pub fn into_outcome(self) -> OpenOutcome {
        self.outcome
    }
}
