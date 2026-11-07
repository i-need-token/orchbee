//! Serves the earlier-layout query and reopen. Listing reads only the in-memory index; reopen
//! reads one entry outside every gate, grafts its missing Workspaces and Tabs under the Session
//! gate, probes each reopened directory outside it, then holds or reserves each terminal and
//! dispatches the launches the way checkpoint restore does. No process or Agent comes back.

use super::{LocalTerminalService, create_reference, lease_expiry, placement};
use crate::{
    layout_history::LayoutHistory,
    terminal_coordinator::{Completion, CreateInputs},
};
use orchbee_server::{LayoutReopenError, WorkbenchCheckpoint};
use orchbee_workbench_protocol::{
    LayoutHistoryAction, LayoutHistoryFocus, LayoutHistoryOutcome, LayoutHistoryRejection,
    LayoutHistoryUnavailable, ProtocolVersion, WorkbenchLayoutHistoryRequest,
    WorkbenchLayoutHistoryResult, WorkerId,
};
use std::sync::Arc;

impl LocalTerminalService {
    /// Installs the Session's history once its storage is open. A second install is refused.
    pub(crate) fn install_layout_history(&self, history: Arc<LayoutHistory>) -> bool {
        self.layout_history.set(history).is_ok()
    }

    pub(crate) fn layout_history(&self) -> Option<&Arc<LayoutHistory>> {
        self.layout_history.get()
    }

    pub(crate) fn execute_layout_history(
        &self,
        request: WorkbenchLayoutHistoryRequest,
    ) -> WorkbenchLayoutHistoryResult {
        let outcome = self.layout_history_outcome(&request);
        WorkbenchLayoutHistoryResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            outcome,
        }
    }

    fn layout_history_outcome(
        &self,
        request: &WorkbenchLayoutHistoryRequest,
    ) -> LayoutHistoryOutcome {
        let unavailable = LayoutHistoryOutcome::Unavailable {
            reason: LayoutHistoryUnavailable::ServiceUnavailable,
        };
        let Some(history) = self.layout_history() else {
            return unavailable;
        };
        match request.action {
            LayoutHistoryAction::List {} => {
                let Ok(coordinator) = self.coordinator.lock() else {
                    return unavailable;
                };
                match coordinator.layout_history_summaries(history) {
                    Ok((session, _)) if session != request.session_id => {
                        LayoutHistoryOutcome::Rejected {
                            reason: LayoutHistoryRejection::SessionMismatch,
                        }
                    }
                    Ok((_, entries)) => LayoutHistoryOutcome::Listed { entries },
                    Err(_) => unavailable,
                }
            }
            LayoutHistoryAction::Reopen { saved_at } => match history.entry(saved_at) {
                Ok(Some(entry)) => self.reopen_layout_entry(request, &entry),
                Ok(None) => LayoutHistoryOutcome::NotFound {},
                Err(_) => unavailable,
            },
        }
    }

    /// Reopens what `entry` has and the Workbench lacks, then starts its terminals.
    fn reopen_layout_entry(
        &self,
        request: &WorkbenchLayoutHistoryRequest,
        entry: &WorkbenchCheckpoint,
    ) -> LayoutHistoryOutcome {
        let unavailable = || LayoutHistoryOutcome::Unavailable {
            reason: LayoutHistoryUnavailable::ServiceUnavailable,
        };
        let (reopened, targets) = {
            let Ok(workers) = self.workers.lock() else {
                return unavailable();
            };
            let Some(owner) = placement(workers.request_handle().as_ref()).and_then(|worker| {
                WorkerId::parse(&format!(
                    "{:032x}",
                    u128::from_be_bytes(worker.worker_id.as_bytes())
                ))
                .ok()
            }) else {
                return unavailable();
            };
            let Ok(mut coordinator) = self.coordinator.lock() else {
                return unavailable();
            };
            if coordinator.workbench_session_matches(&request.session_id) != Some(true) {
                return LayoutHistoryOutcome::Rejected {
                    reason: LayoutHistoryRejection::SessionMismatch,
                };
            }
            match coordinator.reopen_layout(entry, &owner) {
                Ok(reopened) => reopened,
                Err(LayoutReopenError::Busy) => {
                    return LayoutHistoryOutcome::Unavailable {
                        reason: LayoutHistoryUnavailable::Busy,
                    };
                }
                Err(LayoutReopenError::ResourceLimit) => {
                    return LayoutHistoryOutcome::Rejected {
                        reason: LayoutHistoryRejection::ResourceLimit,
                    };
                }
                Err(_) => return unavailable(),
            }
        };
        let Some(revision) = reopened.revision else {
            return LayoutHistoryOutcome::NothingToReopen {};
        };
        // The directory checks restore makes, outside every gate.
        let decided: Vec<_> = targets
            .into_iter()
            .map(|target| {
                let missing = target
                    .working_directory()
                    .filter(|path| !std::path::Path::new(path.as_str()).is_dir())
                    .cloned();
                (target, missing)
            })
            .collect();
        self.start_reopened_terminals(decided);
        LayoutHistoryOutcome::Reopened {
            revision,
            workspaces: reopened.workspaces,
            tabs: reopened.tabs,
            focus: reopened
                .first_tab
                .map(|(terminal_context_id, tab_id)| LayoutHistoryFocus {
                    terminal_context_id,
                    tab_id,
                }),
        }
    }

    /// Holds each target whose directory is missing and launches the rest through restore's
    /// reservation, one settled launch at a time. A target that changed meanwhile is skipped.
    fn start_reopened_terminals(
        &self,
        targets: Vec<(
            orchbee_server::WorkbenchRestoreTarget,
            Option<orchbee_workbench_protocol::WorkbenchDirectoryPath>,
        )>,
    ) {
        let capture = self.launch_capture();
        let (pending, handle) = {
            let Ok(workers) = self.workers.lock() else {
                return;
            };
            let handle = workers.request_handle();
            let Ok(mut coordinator) = self.coordinator.lock() else {
                return;
            };
            let mut pending = Vec::new();
            for (target, missing) in targets {
                if let Some(directory) = missing {
                    coordinator.hold_reopened_terminal(&target, directory);
                    continue;
                }
                let Ok(cwd) = target
                    .working_directory()
                    .map(|path| {
                        orchbee_client_server_protocol::WorkingDirectory::parse(path.as_str())
                    })
                    .transpose()
                else {
                    continue;
                };
                let inputs = CreateInputs::admitted_by(
                    create_reference(),
                    lease_expiry(),
                    handle.as_ref(),
                    capture.clone(),
                );
                if let Ok(terminal) = coordinator.prepare_checkpoint_terminal(&target, inputs, cwd)
                {
                    pending.push(terminal);
                }
            }
            (pending, handle)
        };
        for terminal in pending {
            let completion = handle.as_ref().map_or(Completion::Unavailable, |handle| {
                self.submit(handle, terminal.command())
            });
            let Ok(workers) = self.workers.lock() else {
                return;
            };
            let Ok(mut coordinator) = self.coordinator.lock() else {
                return;
            };
            let _settled = coordinator.complete(
                terminal,
                completion,
                placement(workers.request_handle().as_ref()),
            );
        }
    }
}
