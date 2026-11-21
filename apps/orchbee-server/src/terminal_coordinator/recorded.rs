//! What the binding record keeps of the coordinator: each settled binding with the facts a Server
//! that did not create it needs to serve it, and the next identity the coordinator would allocate. A
//! binding whose launch or start is in flight or unknown is left out, as an adoption leaves it out.
//! A Server that adopts the Host such a record names imports both back.

use std::{num::NonZeroU64, sync::Arc};

use orchbee_capability_node_protocol::{
    AdoptBinding, AdoptBindingState, HostTerminalIdentity, LeaseExpiresAtUnixMilliseconds,
    TerminalFence, TerminalId, TerminalOperationAuthorization,
};
use orchbee_client_server_protocol::{
    CreateAcceptedState, CreateOutcome, Geometry, LaunchProfileProvenance, LaunchSelection,
    RequestId,
};

use super::{
    TerminalBinding, TerminalCompletionSeal, TerminalCoordinator, TerminalPhase, WorkerPlacement,
    make_authorization,
};

/// One settled binding as the binding record keeps it.
#[derive(Clone, Debug)]
pub(crate) struct RecordedBinding {
    /// The creation request the binding answers, which a Client's start names.
    pub(crate) reference: RequestId,
    /// What an adoption sends for it; it carries no Close, which the Workbench holds.
    pub(crate) binding: AdoptBinding,
    pub(crate) worker: WorkerPlacement,
    /// The geometry it started at; none while it awaits its start.
    pub(crate) geometry: Option<Geometry>,
    pub(crate) deferred: bool,
    pub(crate) interactive_shell: bool,
    pub(crate) profile_provenance: Option<LaunchProfileProvenance>,
    pub(crate) launch_selection: Option<LaunchSelection>,
}

/// What can change about a recorded binding while its terminal runs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RecordedState {
    pub(crate) terminal: TerminalId,
    pub(crate) worker: WorkerPlacement,
    pub(crate) identity: HostTerminalIdentity,
    pub(crate) geometry: Option<Geometry>,
    pub(crate) fence: TerminalFence,
}

impl TerminalCoordinator {
    /// Every settled binding a Presentation may show: created and awaiting its start, or started.
    /// Popups are left out; no Presentation shows them.
    pub(crate) fn recorded_bindings(&self) -> Vec<RecordedBinding> {
        self.bindings
            .iter()
            .filter_map(|(reference, binding)| {
                let state = recorded_state(binding)?;
                Some(RecordedBinding {
                    reference: reference.clone(),
                    binding: AdoptBinding {
                        terminal_id: state.terminal,
                        identity: state.identity,
                        state: if state.geometry.is_some() {
                            AdoptBindingState::Started
                        } else {
                            AdoptBindingState::AwaitingStart
                        },
                        fence: state.fence,
                        close: None,
                    },
                    worker: state.worker,
                    geometry: state.geometry,
                    deferred: binding.deferred,
                    interactive_shell: binding.interactive_shell,
                    profile_provenance: binding.profile_provenance.clone(),
                    launch_selection: binding.launch_selection.clone(),
                })
            })
            .collect()
    }

    /// [`Self::recorded_bindings`] without the facts fixed at creation, cheap enough to compare on
    /// every look.
    pub(crate) fn recorded_states(&self) -> Vec<RecordedState> {
        self.bindings.values().filter_map(recorded_state).collect()
    }

    /// The identity the coordinator allocates next; none once the identities are exhausted.
    pub(crate) const fn next_identity(&self) -> Option<u128> {
        self.next_identity
    }

    /// Allocates no identity below `floor` from now on; `None` exhausts the identities. An
    /// adopting Server raises it past every identity its predecessor may have issued.
    pub(crate) fn raise_identity_floor(&mut self, floor: Option<u128>) {
        self.next_identity = self
            .next_identity
            .zip(floor)
            .map(|(next, floor)| next.max(floor));
    }

    /// Enters `recorded`, a settled binding a binding record kept, as a binding of `worker`, the
    /// generation that adopted its Host: the recorded fence and phase with a fresh operation and
    /// lease, and the creation outcome its Client saw. One the Host's inventory lacked, `missing`,
    /// enters unavailable, its program gone. Returns its authorization; none when its creation
    /// request is already bound, the coordinator is full, no identity is left or the binding
    /// contradicts itself.
    pub(crate) fn import_recorded(
        &mut self,
        recorded: &RecordedBinding,
        missing: bool,
        worker: WorkerPlacement,
        expiry: LeaseExpiresAtUnixMilliseconds,
    ) -> Option<TerminalOperationAuthorization> {
        if self.bindings.contains_key(&recorded.reference) || self.at_binding_limit() {
            return None;
        }
        let identity = recorded.binding.identity;
        let phase = match (recorded.binding.state, recorded.geometry) {
            (AdoptBindingState::AwaitingStart, None) if recorded.deferred => {
                TerminalPhase::AwaitingStart(identity)
            }
            (AdoptBindingState::Started, Some(geometry)) => {
                TerminalPhase::Started { identity, geometry }
            }
            _ => return None,
        };
        // A deferred creation answered awaiting its start; an immediate one answered started.
        let created = match phase {
            TerminalPhase::Started { geometry, .. } if !recorded.deferred => {
                CreateAcceptedState::Started { geometry }
            }
            _ => CreateAcceptedState::AwaitingStart,
        };
        let operation = self.allocate_identity()?;
        let authorization = make_authorization(
            recorded.binding.terminal_id.as_bytes(),
            operation,
            worker,
            NonZeroU64::new(recorded.binding.fence.get())?,
            expiry,
        )?;
        self.bindings.insert(
            recorded.reference.clone(),
            TerminalBinding {
                popup_owned: false,
                popup_closing: false,
                authorization,
                completion_seal: Arc::new(TerminalCompletionSeal),
                deferred: recorded.deferred,
                create_outcome: Some(CreateOutcome::Accepted { state: created }),
                phase: if missing {
                    TerminalPhase::Unavailable(Some(identity))
                } else {
                    phase
                },
                profile_provenance: recorded.profile_provenance.clone(),
                launch_selection: recorded.launch_selection.clone(),
                interactive_shell: recorded.interactive_shell,
            },
        );
        Some(authorization)
    }
}

fn recorded_state(binding: &TerminalBinding) -> Option<RecordedState> {
    if binding.popup_owned {
        return None;
    }
    let (identity, geometry) = match binding.phase {
        TerminalPhase::AwaitingStart(identity) => (identity, None),
        TerminalPhase::Started { identity, geometry } => (identity, Some(geometry)),
        _ => return None,
    };
    Some(RecordedState {
        terminal: binding.authorization.terminal_id(),
        worker: WorkerPlacement {
            worker_id: binding.authorization.worker_id(),
            generation: binding.authorization.worker_generation(),
        },
        identity,
        geometry,
        fence: binding.authorization.fence(),
    })
}
