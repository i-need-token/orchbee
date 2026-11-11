//! Submits one captured Panel split, placement, or close operation and reads the matching
//! canonical projection while retaining known commit evidence. It does not install UI
//! membership or replay an uncertain structural effect.

use super::{ClientExecutor, Result, SetupEntryExpiry, SetupOwner, SetupTarget, closed, event};
use crate::local_client::workbench::panel_close::{
    WorkbenchPanelCloseNotSubmitted, WorkbenchPanelCloseRequest, WorkbenchPanelCloseSubmission,
};
use crate::local_client::workbench::panel_placement::{
    WorkbenchPanelPlacementNotSubmitted, WorkbenchPanelPlacementPrecommit,
    WorkbenchPanelPlacementRequest, WorkbenchPanelPlacementSubmission,
};
use crate::local_client::workbench::panel_split::{
    WorkbenchPanelSplitNotSubmitted, WorkbenchPanelSplitRequest, WorkbenchPanelSplitSubmission,
};
use crate::workbench_client::mailbox::{CommandLease, MutationResult, UiReply};
use crate::workbench_state::{
    CarrierFailure, MembershipResultKey, PaneKey, PanelCommit, PanelDecision, PanelIntent,
    PanelOperation, PanelResult, PendingPanelCommit,
};
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchAction, WorkbenchCommand, WorkbenchRejectionReason,
    WorkbenchSnapshotOutcome,
};

impl ClientExecutor {
    pub(super) fn run_panel(
        &mut self,
        lease: &CommandLease,
        operation: PanelOperation,
    ) -> Result<()> {
        if self
            .mailbox
            .mark_panel_in_flight(&operation)
            .map_err(closed)?
            != MutationResult::Applied
        {
            return Err(CarrierFailure::Protocol);
        }
        let cutoff = self.effective_lease_cutoff(lease, operation.deadline())?;
        let baseline = operation.baseline();
        let command = |action| WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: operation.request_id().clone(),
            session_id: baseline.session_id().clone(),
            expected_revision: baseline.revision(),
            action,
        };
        let result = match operation.intent() {
            PanelIntent::Split {
                direction,
                label,
                geometry,
            } => {
                // The catalog id names the Agent; the Server resolves the command.
                let agent_start = match operation.agent_start() {
                    Some(provider) => Some(
                        orchbee_workbench_protocol::AgentStartSelector::parse(provider)
                            .ok_or(CarrierFailure::Protocol)?,
                    ),
                    None => None,
                };
                let request =
                    WorkbenchPanelSplitRequest::try_from(command(WorkbenchAction::PanelSplit {
                        right_click: Default::default(),
                        terminal_context_id: baseline
                            .terminal_context_id()
                            .ok_or(CarrierFailure::Protocol)?
                            .clone(),
                        tab_id: operation.target().tab_id().clone(),
                        panel_id: operation.panel_id().clone(),
                        direction: *direction,
                        label: label.clone(),
                        custom_label: None,
                        geometry: *geometry,
                        launch: None,
                        profile: None,
                        options: Box::new(orchbee_workbench_protocol::LaunchOptions {
                            agent_start,
                            ..Default::default()
                        }),
                    }))
                    .map_err(|_command| CarrierFailure::Protocol)?;
                match self.client.submit_workbench_panel_split(request, cutoff) {
                    WorkbenchPanelSplitSubmission::DefinitelyNotSubmitted(reason) => {
                        if matches!(
                            reason,
                            WorkbenchPanelSplitNotSubmitted::ConnectionUnavailable
                        ) || self.client.generation().is_none()
                        {
                            return Err(CarrierFailure::Transport);
                        }
                        PanelResult::NotSubmitted
                    }
                    WorkbenchPanelSplitSubmission::DefinitePrecommit(precommit) => {
                        PanelResult::Precommit(precommit.into())
                    }
                    WorkbenchPanelSplitSubmission::Committed(commit) => {
                        if self.pending_panel.is_some() || self.pending_create.is_some() {
                            return Err(CarrierFailure::Protocol);
                        }
                        let commit = PanelCommit::Split(commit);
                        self.pending_panel =
                            Some(PendingPanelCommit::new(operation.clone(), commit.clone()));
                        // Keep the commit worker-owned through Snapshot failure, timed deposit
                        // and join.
                        PanelResult::Committed {
                            commit: Box::new(commit),
                            outcome: self.read_panel_snapshot(cutoff)?,
                        }
                    }
                    WorkbenchPanelSplitSubmission::OutcomeUnknown { .. } => {
                        // The adapter retires every ambiguous handoff; no new identity can
                        // replay it.
                        return Err(CarrierFailure::OutcomeUnknown);
                    }
                }
            }
            intent @ (PanelIntent::Close | PanelIntent::CloseIfIdle) => {
                let terminal_context_id = baseline
                    .terminal_context_id()
                    .ok_or(CarrierFailure::Protocol)?
                    .clone();
                let tab_id = operation.target().tab_id().clone();
                let panel_id = operation.panel_id().clone();
                let action = if matches!(intent, PanelIntent::CloseIfIdle) {
                    WorkbenchAction::PanelCloseIfIdle {
                        terminal_context_id,
                        tab_id,
                        panel_id,
                    }
                } else {
                    WorkbenchAction::PanelClose {
                        terminal_context_id,
                        tab_id,
                        panel_id,
                    }
                };
                let request = WorkbenchPanelCloseRequest::try_from(command(action))
                    .map_err(|_command| CarrierFailure::Protocol)?;
                match self.client.submit_workbench_panel_close(request, cutoff) {
                    WorkbenchPanelCloseSubmission::DefinitelyNotSubmitted(reason) => {
                        if matches!(
                            reason,
                            WorkbenchPanelCloseNotSubmitted::ConnectionUnavailable
                        ) || self.client.generation().is_none()
                        {
                            return Err(CarrierFailure::Transport);
                        }
                        PanelResult::NotSubmitted
                    }
                    WorkbenchPanelCloseSubmission::DefinitePrecommit(precommit) => {
                        PanelResult::Precommit(precommit.into())
                    }
                    WorkbenchPanelCloseSubmission::Accepted(accepted) => {
                        PanelResult::CloseAccepted(accepted)
                    }
                    WorkbenchPanelCloseSubmission::OutcomeUnknown { .. } => {
                        return Err(CarrierFailure::OutcomeUnknown);
                    }
                }
            }
            PanelIntent::Move {
                target_panel_id,
                direction,
            } => {
                let request =
                    WorkbenchPanelPlacementRequest::try_from(command(WorkbenchAction::PanelMove {
                        terminal_context_id: baseline
                            .terminal_context_id()
                            .ok_or(CarrierFailure::Protocol)?
                            .clone(),
                        tab_id: operation.target().tab_id().clone(),
                        panel_id: operation.panel_id().clone(),
                        target_panel_id: target_panel_id.clone(),
                        direction: *direction,
                    }))
                    .map_err(|_command| CarrierFailure::Protocol)?;
                match self
                    .client
                    .submit_workbench_panel_placement(request, cutoff)
                {
                    WorkbenchPanelPlacementSubmission::DefinitelyNotSubmitted(reason) => {
                        if matches!(
                            reason,
                            WorkbenchPanelPlacementNotSubmitted::ConnectionUnavailable
                        ) || self.client.generation().is_none()
                        {
                            return Err(CarrierFailure::Transport);
                        }
                        PanelResult::NotSubmitted
                    }
                    WorkbenchPanelPlacementSubmission::DefinitePrecommit(
                        WorkbenchPanelPlacementPrecommit::Rejected {
                            revision,
                            reason: WorkbenchRejectionReason::TabUnavailable,
                        },
                    ) => PanelResult::TabUnavailable {
                        revision,
                        outcome: self.read_panel_snapshot(cutoff)?,
                    },
                    WorkbenchPanelPlacementSubmission::DefinitePrecommit(precommit) => {
                        PanelResult::Precommit(precommit.into())
                    }
                    WorkbenchPanelPlacementSubmission::Unchanged { revision } => {
                        PanelResult::Unchanged(revision)
                    }
                    WorkbenchPanelPlacementSubmission::Committed(commit) => {
                        if self.pending_panel.is_some() || self.pending_create.is_some() {
                            return Err(CarrierFailure::Protocol);
                        }
                        let commit = PanelCommit::Move(commit);
                        self.pending_panel =
                            Some(PendingPanelCommit::new(operation.clone(), commit.clone()));
                        PanelResult::Committed {
                            commit: Box::new(commit),
                            outcome: self.read_panel_snapshot(cutoff)?,
                        }
                    }
                    WorkbenchPanelPlacementSubmission::OutcomeUnknown { .. } => {
                        return Err(CarrierFailure::OutcomeUnknown);
                    }
                }
            }
        };
        let wait = self
            .mailbox
            .publish_panel_result_before(operation.clone(), result, cutoff, &self.clock)
            .map_err(event)?;
        let reply = self
            .mailbox
            .await_ui_reply(&wait)
            .map_err(closed)?
            .ok_or(CarrierFailure::Protocol)?;
        let UiReply::Panel(settlement) = reply else {
            return Err(CarrierFailure::Protocol);
        };
        if self
            .pending_panel
            .as_ref()
            .is_some_and(|pending| pending.operation() != &operation)
        {
            return Err(CarrierFailure::Protocol);
        }
        let result = match settlement.decision() {
            PanelDecision::Continue => {
                if settlement.desired().map(PaneKey::anchor) != Some(operation.target()) {
                    return Err(CarrierFailure::Protocol);
                }
                Ok(())
            }
            PanelDecision::Cleanup(cleanup) => {
                if cleanup.old() != operation.target()
                    || cleanup.result()
                        != &MembershipResultKey::Panel(operation.request_id().clone())
                    || settlement.desired().map(PaneKey::anchor) != Some(operation.target())
                {
                    return Err(CarrierFailure::Protocol);
                }
                self.run_membership_cleanup(lease, cleanup.clone())
            }
            PanelDecision::AttachRecovered(target) => {
                if settlement.desired() != Some(target.selected().key())
                    || !self.attachments.is_empty()
                {
                    return Err(CarrierFailure::Protocol);
                }
                self.setup_target(
                    SetupTarget::operation(target.clone())?,
                    SetupOwner::Lease(lease),
                    SetupEntryExpiry::Retire,
                )
                .map(|_| ())
            }
            PanelDecision::SettledWithoutAttachment if self.attachments.is_empty() => Ok(()),
            PanelDecision::SettledWithoutAttachment => Err(CarrierFailure::Protocol),
            PanelDecision::Retire => Err(CarrierFailure::AuthorityDivergence),
        };
        if result.is_ok() {
            self.pending_panel = None;
        }
        result
    }

    fn read_panel_snapshot(
        &mut self,
        cutoff: std::time::Instant,
    ) -> Result<WorkbenchSnapshotOutcome> {
        self.ensure_before(cutoff)?;
        let request_id = self
            .mailbox
            .allocate_workbench_request_id()
            .map_err(closed)?;
        self.read_workbench_before(request_id, cutoff)
    }
}
