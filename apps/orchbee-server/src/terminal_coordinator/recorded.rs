//! What the binding record keeps of the coordinator: each settled binding with the facts a Server
//! that did not create it needs to serve it, and the next identity the coordinator would allocate. A
//! binding whose launch or start is in flight or unknown is left out, as an adoption leaves it out.

use orchbee_capability_node_protocol::{
    AdoptBinding, AdoptBindingState, HostTerminalIdentity, TerminalFence, TerminalId,
};
use orchbee_client_server_protocol::{
    Geometry, LaunchProfileProvenance, LaunchSelection, RequestId,
};

use super::{TerminalBinding, TerminalCoordinator, TerminalPhase, WorkerPlacement};

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
