//! The compact, terminal-scoped lifecycle read. The response carries only the state needed to
//! observe a binding or seed the retained outcome of a new viewer.

use serde::{Deserialize, Serialize};

use crate::{
    CreationId, ProtocolError, ProtocolErrorCode, TerminalId,
    terminal_fact::{
        ExitResult, ExitResultWire, FailurePairWire, ProcessObservation, ProcessObservationWire,
        RuntimeFailure, TerminalIdentity, TerminalIdentityWire, canonical_u64,
        exit_result_from_wire, exit_result_to_wire, identity_from_wire, identity_to_wire,
        process_observation_from_wire, process_observation_to_wire, runtime_failure_from_wire,
        runtime_failure_to_wire, validate_exit_result, validate_process_observation,
    },
};

pub(super) const READ_TERMINAL_LIFECYCLE_KIND: u16 = 86;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadTerminalLifecycle {
    pub terminal_id: TerminalId,
    pub creation_id: Option<CreationId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalLifecycleSnapshot {
    pub identity: TerminalIdentity,
    pub revision: u64,
    pub state: TerminalLifecycleState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalLifecycleState {
    AwaitingStart,
    Running {
        terminate_requested: bool,
    },
    Finishing {
        terminate_requested: bool,
    },
    ContainingFailure,
    Exited {
        terminate_requested: bool,
        exit: ExitResult,
    },
    Failed {
        failure: RuntimeFailure,
        process: ProcessObservation,
    },
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ReadTerminalLifecycleWire {
    terminal_id: String,
    creation_id: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TerminalLifecycleSnapshotWire {
    identity: TerminalIdentityWire,
    revision: String,
    state: StateWire,
}

#[derive(Deserialize, Serialize)]
#[serde(
    tag = "phase",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum StateWire {
    AwaitingStart {},
    Running {
        terminate_requested: bool,
    },
    Finishing {
        terminate_requested: bool,
    },
    ContainingFailure {},
    Exited {
        terminate_requested: bool,
        exit: ExitResultWire,
    },
    Failed {
        failure: FailurePairWire,
        process: ProcessObservationWire,
    },
}

impl From<ReadTerminalLifecycle> for ReadTerminalLifecycleWire {
    fn from(value: ReadTerminalLifecycle) -> Self {
        Self {
            terminal_id: value.terminal_id.to_text(),
            creation_id: value.creation_id.map(CreationId::to_text),
        }
    }
}

impl TryFrom<ReadTerminalLifecycleWire> for ReadTerminalLifecycle {
    type Error = ProtocolError;

    fn try_from(wire: ReadTerminalLifecycleWire) -> Result<Self, Self::Error> {
        let invalid_target = |_| {
            ProtocolError::new(
                ProtocolErrorCode::InvalidField,
                "invalid lifecycle target identity",
            )
        };
        Ok(Self {
            terminal_id: TerminalId::parse(&wire.terminal_id).map_err(invalid_target)?,
            creation_id: wire
                .creation_id
                .as_deref()
                .map(CreationId::parse)
                .transpose()
                .map_err(invalid_target)?,
        })
    }
}

impl TryFrom<TerminalLifecycleSnapshotWire> for TerminalLifecycleSnapshot {
    type Error = ProtocolError;

    fn try_from(wire: TerminalLifecycleSnapshotWire) -> Result<Self, Self::Error> {
        Ok(Self {
            identity: identity_from_wire(wire.identity)?,
            revision: canonical_u64(&wire.revision)?,
            state: match wire.state {
                StateWire::AwaitingStart {} => TerminalLifecycleState::AwaitingStart,
                StateWire::Running {
                    terminate_requested,
                } => TerminalLifecycleState::Running {
                    terminate_requested,
                },
                StateWire::Finishing {
                    terminate_requested,
                } => TerminalLifecycleState::Finishing {
                    terminate_requested,
                },
                StateWire::ContainingFailure {} => TerminalLifecycleState::ContainingFailure,
                StateWire::Exited {
                    terminate_requested,
                    exit,
                } => TerminalLifecycleState::Exited {
                    terminate_requested,
                    exit: exit_result_from_wire(exit)?,
                },
                StateWire::Failed { failure, process } => TerminalLifecycleState::Failed {
                    failure: runtime_failure_from_wire(failure)?,
                    process: process_observation_from_wire(process)?,
                },
            },
        })
    }
}

impl TryFrom<TerminalLifecycleSnapshot> for TerminalLifecycleSnapshotWire {
    type Error = ProtocolError;

    fn try_from(value: TerminalLifecycleSnapshot) -> Result<Self, Self::Error> {
        Ok(Self {
            identity: identity_to_wire(value.identity),
            revision: value.revision.to_string(),
            state: match value.state {
                TerminalLifecycleState::AwaitingStart => StateWire::AwaitingStart {},
                TerminalLifecycleState::Running {
                    terminate_requested,
                } => StateWire::Running {
                    terminate_requested,
                },
                TerminalLifecycleState::Finishing {
                    terminate_requested,
                } => StateWire::Finishing {
                    terminate_requested,
                },
                TerminalLifecycleState::ContainingFailure => StateWire::ContainingFailure {},
                TerminalLifecycleState::Exited {
                    terminate_requested,
                    exit,
                } => {
                    validate_exit_result(exit)?;
                    StateWire::Exited {
                        terminate_requested,
                        exit: exit_result_to_wire(exit),
                    }
                }
                TerminalLifecycleState::Failed { failure, process } => {
                    validate_process_observation(process)?;
                    StateWire::Failed {
                        failure: runtime_failure_to_wire(failure),
                        process: process_observation_to_wire(process),
                    }
                }
            },
        })
    }
}
