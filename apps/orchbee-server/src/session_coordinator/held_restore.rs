//! Starts the terminal of a Presentation a checkpoint restore held because its directory was
//! unavailable: it admits the request against the current hold, reserves and binds the terminal
//! under the Session gate exactly as restore does, and settles the one launch. It does not probe
//! the directory, dispatch the launch, or decide Agent resume delivery.

use super::{PostcommitCoordinationError, SessionCoordinator};
use crate::terminal_coordinator::{Completion, CreateInputs, PendingOperation, WorkerPlacement};
use orchbee_client_server_protocol::WorkingDirectory;
use orchbee_workbench_protocol::{
    PanelNode, Presentation, PresentationBinding, ProtocolVersion, TerminalPresentationId,
    WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchCommandResult,
    WorkbenchCommandUnavailableReason, WorkbenchDirectoryPath, WorkbenchPanelId,
    WorkbenchRejectionReason, WorkbenchRestoreDirectory, WorkbenchRevision, WorkbenchTabId,
    WorkbenchTerminalContextId,
};

/// What admission needs probed before the launch: the saved directory, or nothing for home.
pub(crate) struct HeldRestoreProbe {
    pub(crate) directory: Option<WorkbenchDirectoryPath>,
    /// The kept output the held Presentation replays when it starts, if any.
    pub(crate) output: Option<(TerminalPresentationId, u64)>,
}

/// One reserved and bound restore launch, waiting for its dispatch to settle.
pub(crate) struct PendingHeldRestore {
    terminal: Box<PendingOperation>,
    request_id: orchbee_workbench_protocol::WorkbenchRequestId,
    terminal_context_id: WorkbenchTerminalContextId,
    tab_id: WorkbenchTabId,
    panel_id: WorkbenchPanelId,
    presentation_id: TerminalPresentationId,
    binding: PresentationBinding,
    revision: WorkbenchRevision,
    /// The directory a retained Agent reference still waits for, when the shell starts there.
    agent_resume: Option<WorkbenchDirectoryPath>,
}

impl PendingHeldRestore {
    pub(crate) fn command(&self) -> &orchbee_capability_node_protocol::CapabilityNodeMessage {
        self.terminal.command()
    }

    /// Attaches the held Presentation's kept output; a refused seed starts without it.
    pub(crate) fn set_output_seed(&mut self, seed: Vec<u8>) {
        if self.terminal.set_output_seed(seed).is_err() {
            eprintln!(
                "orchbee-server diagnostic component=checkpoint operation=terminal-output stage=seed cause=refused"
            );
        }
    }
}

/// A settled restore launch and, when it started in the saved directory with a retained Agent
/// reference, the Presentation and directory a resume must find.
pub(crate) struct CompletedHeldRestore {
    pub(crate) result: WorkbenchCommandResult,
    pub(crate) agent_resume: Option<(TerminalPresentationId, WorkbenchDirectoryPath)>,
}

struct Target<'a> {
    terminal_context_id: &'a WorkbenchTerminalContextId,
    tab_id: &'a WorkbenchTabId,
    panel_id: &'a WorkbenchPanelId,
    presentation_id: &'a TerminalPresentationId,
    directory: WorkbenchRestoreDirectory,
}

fn target(command: &WorkbenchCommand) -> Option<Target<'_>> {
    match &command.action {
        WorkbenchAction::PanelRestoreTerminal {
            terminal_context_id,
            tab_id,
            panel_id,
            presentation_id,
            directory,
        } => Some(Target {
            terminal_context_id,
            tab_id,
            panel_id,
            presentation_id,
            directory: *directory,
        }),
        // This adapter owns only the held restore action.
        _ => None,
    }
}

impl SessionCoordinator {
    /// Admits a held restore against the current hold and names the directory to probe.
    pub(crate) fn held_restore_probe(
        &self,
        command: &WorkbenchCommand,
    ) -> Result<HeldRestoreProbe, Box<WorkbenchCommandResult>> {
        let target = target(command).ok_or_else(|| {
            unavailable(
                command,
                WorkbenchCommandUnavailableReason::ServiceUnavailable,
            )
        })?;
        let held = self.held_directory(command, &target)?;
        Ok(HeldRestoreProbe {
            directory: match target.directory {
                WorkbenchRestoreDirectory::Saved => Some(held.0),
                WorkbenchRestoreDirectory::Home => None,
            },
            output: self
                .restart_output(target.presentation_id)
                .map(|token| (target.presentation_id.clone(), token)),
        })
    }

    /// Reserves the terminal and binds the held Presentation under this one gate. `probed` is the
    /// saved directory the caller found available; a changed hold refuses rather than launching
    /// somewhere the person did not see. Home drops the retained directory and Agent reference in
    /// the same critical section.
    pub(crate) fn prepare_held_restore(
        &mut self,
        command: &WorkbenchCommand,
        inputs: CreateInputs,
        probed: Option<&WorkbenchDirectoryPath>,
    ) -> Result<PendingHeldRestore, Box<WorkbenchCommandResult>> {
        let busy = || unavailable(command, WorkbenchCommandUnavailableReason::ServerBusy);
        let target = target(command).ok_or_else(|| {
            unavailable(
                command,
                WorkbenchCommandUnavailableReason::ServiceUnavailable,
            )
        })?;
        let (held, agent_resume) = self.held_directory(command, &target)?;
        let cwd = match target.directory {
            WorkbenchRestoreDirectory::Saved if probed == Some(&held) => {
                Some(WorkingDirectory::parse(held.as_str()).map_err(|_| {
                    rejected(self, command, WorkbenchRejectionReason::PanelUnavailable)
                })?)
            }
            WorkbenchRestoreDirectory::Saved => {
                return Err(rejected(
                    self,
                    command,
                    WorkbenchRejectionReason::PanelUnavailable,
                ));
            }
            WorkbenchRestoreDirectory::Home => None,
        };
        let worker = inputs.worker().ok_or_else(|| {
            unavailable(
                command,
                WorkbenchCommandUnavailableReason::NoCompatibleWorker,
            )
        })?;
        let owner = orchbee_workbench_protocol::WorkerId::parse(&format!(
            "{:032x}",
            u128::from_be_bytes(worker.worker_id.as_bytes())
        ))
        .map_err(|_| busy())?;
        let restore = self
            .workbench
            .as_ref()
            .ok_or_else(busy)?
            .restoration_target(&owner, target.presentation_id)
            .map_err(|_| busy())?
            .filter(|restore| {
                restore.terminal_context_id() == target.terminal_context_id
                    && restore.tab_id() == target.tab_id
                    && restore.panel_id() == target.panel_id
            })
            .ok_or_else(|| rejected(self, command, WorkbenchRejectionReason::PanelUnavailable))?;
        let (terminal, binding, revision) = self
            .prepare_restore_binding(&restore, inputs, cwd)
            .map_err(|_| busy())?;
        let agent_resume = match target.directory {
            WorkbenchRestoreDirectory::Saved => agent_resume.then_some(held),
            WorkbenchRestoreDirectory::Home => {
                if let Some(workbench) = self.workbench.as_mut() {
                    workbench.forget_restart_intent(target.presentation_id);
                }
                None
            }
        };
        Ok(PendingHeldRestore {
            terminal,
            request_id: command.request_id.clone(),
            terminal_context_id: target.terminal_context_id.clone(),
            tab_id: target.tab_id.clone(),
            panel_id: target.panel_id.clone(),
            presentation_id: target.presentation_id.clone(),
            binding,
            revision,
            agent_resume,
        })
    }

    /// Settles the one dispatched launch and records a known failure on its Presentation, as any
    /// creation does. Nothing here retries.
    pub(crate) fn complete_held_restore(
        &mut self,
        pending: PendingHeldRestore,
        completion: Completion,
        current_worker: Option<WorkerPlacement>,
    ) -> Result<CompletedHeldRestore, PostcommitCoordinationError> {
        let reference = pending.terminal.create_reference().clone();
        let message = self
            .complete(pending.terminal, completion, current_worker)
            .map_err(|_mismatch| PostcommitCoordinationError)?;
        // The restored launch geometry `prepare_restore_binding` requested.
        let geometry = orchbee_workbench_protocol::WorkbenchTerminalGeometry::new(80, 24, 8, 16)
            .map_err(|_| PostcommitCoordinationError)?;
        let terminal_outcome = super::tab_create::terminal_outcome(&reference, geometry, &message)
            .ok_or(PostcommitCoordinationError)?;
        if let Some(workbench) = self.workbench.as_mut() {
            super::tab_create::record_launch_failure(
                workbench,
                &terminal_outcome,
                &pending.presentation_id,
                &pending.binding,
                None,
            );
        }
        let started = matches!(
            terminal_outcome,
            orchbee_workbench_protocol::WorkbenchTerminalOutcome::Started { .. }
        );
        let agent_resume = pending
            .agent_resume
            .filter(|_| started)
            .map(|directory| (pending.presentation_id.clone(), directory));
        Ok(CompletedHeldRestore {
            result: WorkbenchCommandResult {
                protocol_version: ProtocolVersion,
                request_id: pending.request_id,
                outcome: WorkbenchCommandOutcome::PanelTerminalCreated {
                    revision: pending.revision,
                    terminal_context_id: pending.terminal_context_id,
                    tab_id: pending.tab_id,
                    panel_id: pending.panel_id,
                    presentation_id: pending.presentation_id,
                    terminal_outcome,
                },
            },
            agent_resume,
        })
    }

    /// The directory the addressed Presentation's hold names and whether its Agent reference still
    /// waits, or the refusal for a request that no longer addresses a held Presentation.
    fn held_directory(
        &self,
        command: &WorkbenchCommand,
        target: &Target<'_>,
    ) -> Result<(WorkbenchDirectoryPath, bool), Box<WorkbenchCommandResult>> {
        let workbench = self.workbench.as_ref().ok_or_else(|| {
            unavailable(
                command,
                WorkbenchCommandUnavailableReason::ServiceUnavailable,
            )
        })?;
        let snapshot = workbench.snapshot();
        if command.session_id != snapshot.session_id {
            return Err(rejected(
                self,
                command,
                WorkbenchRejectionReason::SessionMismatch,
            ));
        }
        let held = snapshot
            .terminal_contexts
            .iter()
            .find(|context| &context.terminal_context_id == target.terminal_context_id)
            .and_then(|context| context.tabs.iter().find(|tab| &tab.tab_id == target.tab_id))
            .and_then(|tab| presentation(&tab.root, target.panel_id, target.presentation_id))
            .filter(|presentation| presentation.binding.is_none())
            .and_then(|presentation| presentation.launch_failure.as_ref())
            .and_then(|failure| {
                failure
                    .unavailable_directory()
                    .map(|directory| (directory.clone(), failure.agent_resume))
            });
        held.ok_or_else(|| rejected(self, command, WorkbenchRejectionReason::PanelUnavailable))
    }
}

fn presentation<'a>(
    node: &'a PanelNode,
    panel: &WorkbenchPanelId,
    presentation: &TerminalPresentationId,
) -> Option<&'a Presentation> {
    match node {
        PanelNode::Panel {
            panel_id,
            primary_presentation,
            child_presentations,
            ..
        } if panel_id == panel => std::iter::once(primary_presentation)
            .chain(child_presentations)
            .find(|member| &member.presentation_id == presentation),
        PanelNode::Panel { .. } => None,
        PanelNode::Split { children, .. } => children
            .iter()
            .find_map(|slot| self::presentation(&slot.node, panel, presentation)),
    }
}

fn unavailable(
    command: &WorkbenchCommand,
    reason: WorkbenchCommandUnavailableReason,
) -> Box<WorkbenchCommandResult> {
    Box::new(WorkbenchCommandResult {
        protocol_version: ProtocolVersion,
        request_id: command.request_id.clone(),
        outcome: WorkbenchCommandOutcome::Unavailable { reason },
    })
}

fn rejected(
    coordinator: &SessionCoordinator,
    command: &WorkbenchCommand,
    reason: WorkbenchRejectionReason,
) -> Box<WorkbenchCommandResult> {
    Box::new(WorkbenchCommandResult {
        protocol_version: ProtocolVersion,
        request_id: command.request_id.clone(),
        outcome: WorkbenchCommandOutcome::Rejected {
            revision: coordinator
                .workbench
                .as_ref()
                .map_or(WorkbenchRevision::new(0), |workbench| {
                    workbench.snapshot().revision
                }),
            reason,
        },
    })
}
