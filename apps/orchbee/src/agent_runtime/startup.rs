//! Startup execution capability. Unix has no Agent-launch adapter and refuses before effects.
pub(crate) use super::processes::{Selection, interpret};
pub(crate) use super::{SendVerdict, wait_backoff};
use crate::cli::{
    Failure, FailureKind,
    results::{self, Projection},
    target_details,
};
use crate::invocation::{QueryCommand, Resource, Target};
use orchbee_agent_observation_protocol::ObapRequestId;
use orchbee_workbench_protocol::Snapshot;
use serde_json::Value;
#[cfg(windows)]
#[path = "startup/windows.rs"]
mod platform;
#[cfg(not(windows))]
#[path = "startup/unix.rs"]
mod platform;
pub(crate) use platform::{
    admit, read_agent_observation, read_terminal_processes_cancellable, snapshot_from_origin,
    stream_snapshot, submit_prompt_on_session,
};

pub(crate) fn obap_request_id() -> Result<ObapRequestId, Failure> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    ObapRequestId::parse(&format!("{:032x}", u128::from_be_bytes(bytes)))
        .map_err(|_| Failure::new(FailureKind::InternalFailure))
}

/// Frozen protocol facts, independent of process-facing stdout/stderr presentation.
pub(crate) struct Receipt {
    pub(super) status: u8,
    pub(super) verdict: SendVerdict,
    pub(super) accepted: bool,
    pub(super) document: Value,
}
impl Receipt {
    pub(crate) fn exit_status(&self) -> u8 {
        self.status
    }
    pub(crate) fn verdict(&self) -> SendVerdict {
        self.verdict
    }
    pub(crate) fn accepted(&self) -> bool {
        self.accepted
    }
    pub(crate) fn document(&self) -> Value {
        self.document.clone()
    }
}

pub(crate) fn primary_terminal(snapshot: &Snapshot, target: &Target) -> Result<Target, Failure> {
    let Projection::PanelGet(panel) =
        results::project(&QueryCommand::PanelGet(target.clone()), snapshot)?
    else {
        return Err(Failure::new(FailureKind::InternalFailure));
    };
    panel
        .primary_presentation
        .terminal_id
        .map(Target::Id)
        .ok_or_else(|| {
            Failure::new(FailureKind::NotFound)
                .details(target_details(Resource::Panel, &panel.row.panel_id))
        })
}
