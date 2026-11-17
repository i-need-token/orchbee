//! The `workbench.terminal-runtime` read: it answers from the Session's terminal-runtime state, and
//! its State stream follows that state's own revisions, never commits or terminal output. A change
//! wakes every watching Client connection, whose dispatcher reads the state once and sends a frame
//! only when its bytes changed, so a slow Client receives the latest state rather than a backlog.
use super::LocalTerminalService;
use crate::terminal_runtime::TerminalRuntimeState as State;
use orchbee_server::SessionChangeSignal;
use orchbee_workbench_protocol::{
    ProtocolVersion, TerminalRuntimeFailure as Failure, TerminalRuntimeOutcome as Outcome,
    TerminalRuntimeRequest, TerminalRuntimeResult, TerminalRuntimeState, WorkbenchRevision,
    WorkbenchTerminalRuntime,
};
use std::sync::Arc;

impl LocalTerminalService {
    /// Wakes `signal` after every change of the terminal-runtime state.
    pub(crate) fn watch_terminal_runtime(&self, signal: &Arc<SessionChangeSignal>) {
        self.terminal_runtime.watch(signal);
    }

    pub(crate) fn execute_terminal_runtime(
        &self,
        request: TerminalRuntimeRequest,
    ) -> TerminalRuntimeResult {
        let outcome = match self.workbench_session_matches(&request.session_id) {
            Some(true) => {
                let current = self.terminal_runtime.current();
                Outcome::Current {
                    runtime: WorkbenchTerminalRuntime {
                        revision: WorkbenchRevision::new(current.revision.get()),
                        state: match current.state {
                            State::Serving => TerminalRuntimeState::Serving,
                            State::Reconnecting => TerminalRuntimeState::Reconnecting,
                            State::GivenUp => TerminalRuntimeState::GivenUp,
                        },
                    },
                }
            }
            Some(false) => Outcome::Failed {
                reason: Failure::Denied,
            },
            None => Outcome::Failed {
                reason: Failure::Unavailable,
            },
        };
        TerminalRuntimeResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            session_id: request.session_id,
            outcome,
        }
    }
}
