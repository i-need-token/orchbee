//! Shared ratio receipts advance exact tree fences; focus remains Client-owned.
use super::{Engine, error, internal, ownership};
use crate::cli::{
    Failure, FailureKind,
    client_control::{Answer, exchange as control},
    query,
};
use orchbee_workbench_protocol::{
    ClientPanelLayout, ClientViewAction, ClientViewAvailability, ClientViewControl,
    ClientViewControlStatus, ClientViewIntent, ClientViewOutcome, ClientViewPhase,
    ClientViewRecord, Tab,
};

impl Engine<'_> {
    pub(super) fn current_view(&mut self) -> Result<ClientViewRecord, Failure> {
        let selected = self.selected.as_ref().ok_or_else(internal)?.clone();
        loop {
            self.check()?;
            let reply = control::exchange(
                &mut self.client,
                &self.snapshot.session_id,
                ClientViewAction::Get {
                    client_id: selected.client_id.clone(),
                },
                self.deadline,
                self.signals,
            )?;
            let ClientViewOutcome::Found { client } = reply.outcome else {
                return Err(error(FailureKind::ServiceUnavailable, "client-unavailable"));
            };
            if client.availability != ClientViewAvailability::Current
                || client.view.phase == ClientViewPhase::Unavailable
            {
                return Err(error(FailureKind::ServiceUnavailable, "client-unavailable"));
            }
            let original = &selected.view.selection;
            let current = &client.view.selection;
            let repaired =
                original
                    .as_ref()
                    .zip(current.as_ref())
                    .is_some_and(|(before, after)| {
                        before.terminal_context_id == self.destination.context
                            && before.tab_id.is_none()
                            && after.terminal_context_id == self.destination.context
                            && after.tab_id == self.receipt.new_tab
                            && after.tab_id.is_some()
                            && after.panel_id.as_ref() == self.panels.first()
                            && usize::from(after.panel_count) == self.panels.len()
                            && !after.zoomed
                    });
            let empty = original == current
                && current.as_ref().is_some_and(|selection| {
                    selection.tab_id.is_none()
                        && selection.panel_id.is_none()
                        && selection.panel_count == 0
                        && !selection.zoomed
                });
            if client.view.phase == ClientViewPhase::Ready
                && (client.view.workbench_revision >= self.snapshot.revision || empty)
            {
                if original != current && !repaired {
                    return Err(error(FailureKind::Denied, "client-selection-changed"));
                }
                return Ok(*client);
            }
            self.pause();
        }
    }
    fn control(
        &mut self,
        phase: &'static str,
        intent: ClientViewIntent,
    ) -> Result<Answer, Failure> {
        self.receipt.stage = phase;
        let current = self.current_view()?;
        self.check()?;
        let result = control::submit_control(
            &mut self.client,
            &self.snapshot.session_id,
            ClientViewControl {
                operation_id: query::request_id()?,
                client_id: current.client_id.clone(),
                expected_view_revision: current.view_revision,
                minimum_workbench_revision: self.snapshot.revision,
                intent,
            },
            self.deadline,
            self.signals,
        );
        match &result {
            Ok(answer) => self.receipt.record(phase, answer.document()),
            Err(failure) => self.receipt.record(phase, failure.document()),
        }
        let answer = result?;
        if answer.exit_code() != 0 {
            // A pending/uncertain Client operation may still execute. Preserve the new tree.
            let pending=answer.reply.as_ref().is_some_and(|reply| matches!(&reply.outcome,ClientViewOutcome::Operation{operation} if operation.status.pending()));
            let kind = if pending || answer.document()["result"]["observation"]["kind"] == "unknown"
            {
                FailureKind::OutcomeUnknown
            } else {
                match answer.exit_code() {
                    130 => FailureKind::Cancelled,
                    7 => FailureKind::Timeout,
                    8 => FailureKind::OutcomeUnknown,
                    4 => FailureKind::ServiceUnavailable,
                    5 => FailureKind::TransportLoss,
                    _ => FailureKind::Denied,
                }
            };
            return Err(error(kind, "client-unavailable"));
        }
        self.check()?;
        Ok(answer)
    }
    pub(super) fn read_layout(&mut self, tab: &Tab) -> Result<ClientPanelLayout, Failure> {
        let panel = crate::cli::results::panel_ids_in(&tab.root)
            .first()
            .cloned()
            .cloned()
            .ok_or_else(internal)?;
        let answer = self.control(
            "directory",
            ClientViewIntent::Layout {
                terminal_context_id: self.destination.context.clone(),
                tab_id: tab.tab_id.clone(),
                panel_id: panel.clone(),
            },
        )?;
        let Some(reply) = answer.reply else {
            return Err(internal());
        };
        let ClientViewOutcome::Operation { operation } = reply.outcome else {
            return Err(internal());
        };
        let ClientViewControlStatus::LayoutRead { layout, .. } = operation.status else {
            return Err(error(FailureKind::Denied, "client-unavailable"));
        };
        if layout.workbench_revision != self.snapshot.revision
            || layout.tab_id != tab.tab_id
            || layout.terminal_context_id != self.destination.context
            || layout.source_panel_id != panel
        {
            return Err(error(FailureKind::Denied, "layout-changed"));
        }
        Ok(*layout)
    }
    pub(super) fn set_ratio(&mut self, index: usize) -> Result<(), Failure> {
        self.refresh(self.snapshot.revision)?;
        let split = &self.plan.splits[index];
        if self.selected.is_none() {
            let expected = self.expected.as_ref().ok_or_else(internal)?;
            let (identity, previous) =
                crate::cli::layout_topology::split(&expected.root, &split.path)?;
            if identity != self.splits[index] {
                return Err(ownership::changed());
            }
            self.receipt.stage = "ratio";
            let (effect, result) = crate::cli::layout_topology::submit_ratio(
                &mut self.client,
                &self.snapshot,
                &self.destination.context,
                &expected.tab_id,
                identity.clone(),
                previous,
                split.ratio,
                None,
                self.deadline,
                self.signals.poll_fd(),
            )?;
            self.receipt.record("ratio", effect);
            let revision = result?;
            self.expected = Some(ownership::with_shared_ratio(
                expected,
                &identity,
                previous,
                split.ratio,
            )?);
            return self.refresh(revision);
        }
        let answer = self.control(
            "ratio",
            ClientViewIntent::SetSplitRatio {
                terminal_context_id: self.destination.context.clone(),
                tab_id: self.new_tab()?,
                path: split.path.clone(),
                ratio: split.ratio,
                expected_split: Some(self.splits[index].clone()),
            },
        )?;
        let Some(reply) = answer.reply else {
            return Err(internal());
        };
        let ClientViewOutcome::Operation { operation } = reply.outcome else {
            return Err(internal());
        };
        let ClientViewControlStatus::LayoutAdjusted { adjustment, .. } = operation.status else {
            return Err(error(FailureKind::Denied, "client-unavailable"));
        };
        let orchbee_workbench_protocol::ClientLayoutChange::Split { split: changed } =
            &adjustment.change
        else {
            return Err(internal());
        };
        let identity = &self.splits[index];
        if changed.split_id != identity.split_id
            || changed.first_slot_id != identity.first_slot_id
            || changed.second_slot_id != identity.second_slot_id
            || changed.ratio != self.plan.splits[index].ratio
            || adjustment.workbench_revision < self.snapshot.revision
        {
            return Err(internal());
        }
        self.expected = Some(ownership::with_ratio(
            self.expected.as_ref().ok_or_else(internal)?,
            &self.destination.context,
            &adjustment,
        )?);
        // Preserve the known commit even if the subsequent read fails; never absorb other edits.
        self.refresh(adjustment.workbench_revision)
    }
    pub(super) fn select_created(&mut self) -> Result<(), Failure> {
        if !self.destination.focus {
            return Ok(());
        }
        self.refresh(self.snapshot.revision)?;
        let answer = self.control(
            "focus",
            ClientViewIntent::Panel {
                terminal_context_id: self.destination.context.clone(),
                tab_id: self.new_tab()?,
                panel_id: self.panels[0].clone(),
            },
        )?;
        if let Some(reply) = answer.reply
            && let ClientViewOutcome::Operation { operation } = reply.outcome
            && let ClientViewControlStatus::Applied { view } = operation.status
        {
            let selection = view.selection.as_ref().ok_or_else(internal)?;
            if selection.terminal_context_id != self.destination.context
                || selection.tab_id.as_ref() != Some(&self.new_tab()?)
                || selection.panel_id.as_ref() != self.panels.first()
                || selection.zoomed
            {
                return Err(error(FailureKind::Denied, "client-selection-changed"));
            }
            // Only an acknowledged navigation effect updates our expected selection.
            self.selected.as_mut().ok_or_else(internal)?.view = view;
            return Ok(());
        }
        Err(error(FailureKind::Denied, "client-unavailable"))
    }
}
