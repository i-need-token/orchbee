//! Bounded ephemeral views and delivery receipts. No transport or UI effects occur here.
use orchbee_workbench_protocol::{
    ClientViewAction, ClientViewAvailability, ClientViewCompletion, ClientViewCompletionOutcome,
    ClientViewControl, ClientViewControlReason, ClientViewControlStatus, ClientViewIntent,
    ClientViewObservation, ClientViewOperation, ClientViewOutcome, ClientViewRecord,
    ClientViewSequence, ClientViewUnavailable, MAXIMUM_CLIENT_VIEWS,
    MAXIMUM_ENCODED_CLIENT_ATTENTION_BYTES, MAXIMUM_ENCODED_CLIENT_LAYOUT_BYTES, WorkbenchClientId,
    WorkbenchRequestId,
};
use std::{
    collections::BTreeMap,
    num::NonZeroU64,
    time::{Duration, Instant},
};

const QUEUE_LIFETIME: Duration = Duration::from_secs(5);
const RECEIPT_RETENTION: Duration = Duration::from_secs(60);
const MAXIMUM_RECEIPTS: usize = 256;
const MAXIMUM_PAYLOAD_RESERVATION: usize = 16 * MAXIMUM_ENCODED_CLIENT_LAYOUT_BYTES;

pub(crate) struct GraphicsInfo {
    pub(crate) client_id: WorkbenchClientId,
    pub(crate) capabilities: Option<orchbee_workbench_protocol::PaneGraphicsCapabilities>,
    pub(crate) visible: Option<bool>,
    pub(crate) drawn: Option<orchbee_workbench_protocol::WorkbenchRevision>,
}

struct PublishedView {
    record: ClientViewRecord,
    graphics: Option<orchbee_workbench_protocol::PaneGraphicsClientObservation>,
}
impl PublishedView {
    fn observe(&self, _now: Instant) -> ClientViewRecord {
        // The authenticated connection owns liveness. Semantic view/graphics revisions still
        // fence every control; an idle Client no longer needs timer heartbeats to stay current.
        self.record.clone()
    }
}
struct Receipt {
    operation: ClientViewOperation,
    submitted_at: Instant,
    completed_at: Option<Instant>,
    payload_reservation: usize,
}
impl Receipt {
    fn settle(&mut self, status: ClientViewControlStatus, now: Instant) {
        self.payload_reservation = match &status {
            ClientViewControlStatus::TerminalAttentionRead { attention, .. } => {
                serde_json::to_vec(attention)
                    .map_or(MAXIMUM_ENCODED_CLIENT_ATTENTION_BYTES, |encoded| {
                        encoded.len()
                    })
            }
            ClientViewControlStatus::AttentionRead { attention, .. } => {
                serde_json::to_vec(attention)
                    .map_or(MAXIMUM_ENCODED_CLIENT_ATTENTION_BYTES, |encoded| {
                        encoded.len()
                    })
            }
            ClientViewControlStatus::LayoutRead { layout, .. } => serde_json::to_vec(layout)
                .map_or(MAXIMUM_ENCODED_CLIENT_LAYOUT_BYTES, |encoded| encoded.len()),
            ClientViewControlStatus::PopupOpened { .. }
            | ClientViewControlStatus::PopupClosed { .. }
            | ClientViewControlStatus::PresentationRead { .. }
            | ClientViewControlStatus::ViewportRead { .. }
            | ClientViewControlStatus::NavigationRead { .. }
            | ClientViewControlStatus::LayoutAdjusted { .. }
            | ClientViewControlStatus::DirectionFocused { .. }
            | ClientViewControlStatus::DirectionUnchanged { .. }
            | ClientViewControlStatus::Queued { .. }
            | ClientViewControlStatus::Delivered { .. }
            | ClientViewControlStatus::Applied { .. }
            | ClientViewControlStatus::ZoomUnchanged { .. }
            | ClientViewControlStatus::ZoomToggled { .. }
            | ClientViewControlStatus::Rejected { .. }
            | ClientViewControlStatus::Incomplete { .. } => 0,
        };
        self.operation.status = status;
        self.completed_at = Some(now);
    }
}

#[derive(Default)]
pub(crate) struct ClientViewRegistry {
    events: Option<orchbee_server::SessionEventJournal>,
    clients: BTreeMap<WorkbenchClientId, PublishedView>,
    receipts: BTreeMap<WorkbenchRequestId, Receipt>,
}
impl ClientViewRegistry {
    pub(crate) fn observe_graphics(
        &mut self,
        generation: NonZeroU64,
        client_id: &WorkbenchClientId,
        heartbeat: ClientViewSequence,
        observation: orchbee_workbench_protocol::PaneGraphicsClientObservation,
        now: Instant,
    ) -> Result<(), orchbee_workbench_protocol::PaneGraphicsFailure> {
        use orchbee_workbench_protocol::PaneGraphicsFailure as Failure;
        if client_id.as_str() != format!("{:032x}", generation.get()) {
            return Err(Failure::Denied);
        }
        let row = self.clients.get_mut(client_id).ok_or(Failure::NotFound)?;
        if row.observe(now).availability != ClientViewAvailability::Current
            || row.record.heartbeat != heartbeat
            || row.record.view.workbench_revision != observation.workbench_revision
        {
            return Err(Failure::SourceChanged);
        }
        row.graphics = Some(observation);
        Ok(())
    }

    pub(crate) fn graphics_info(
        &self,
        requested: Option<&WorkbenchClientId>,
        source: &orchbee_workbench_protocol::SemanticPane,
        revision: orchbee_workbench_protocol::WorkbenchRevision,
        now: Instant,
    ) -> Result<Option<GraphicsInfo>, orchbee_workbench_protocol::PaneGraphicsFailure> {
        use orchbee_workbench_protocol::PaneGraphicsFailure as Failure;
        let row = if let Some(id) = requested {
            self.clients.get(id).ok_or(Failure::NotFound)?
        } else {
            let mut fresh = self
                .clients
                .values()
                .filter(|row| row.observe(now).availability == ClientViewAvailability::Current);
            let Some(row) = fresh.next() else {
                return Ok(None);
            };
            if fresh.next().is_some() {
                return Ok(None);
            }
            row
        };
        let mut info = GraphicsInfo {
            client_id: row.record.client_id.clone(),
            capabilities: None,
            visible: None,
            drawn: None,
        };
        if row.observe(now).availability == ClientViewAvailability::Current
            && let Some(observation) = &row.graphics
        {
            info.capabilities = Some(observation.capabilities.clone());
            if let Some(pane) = observation
                .panes
                .iter()
                .find(|pane| pane.target.panel_id == source.panel_id)
            {
                if pane.target.presentation_id.as_ref() == Some(&source.presentation_id)
                    && pane.binding == source.binding
                {
                    info.visible = Some(true);
                    info.drawn = pane.drawn_revision.filter(|drawn| *drawn <= revision);
                }
            } else if observation.complete {
                info.visible = Some(false);
            }
        }
        Ok(Some(info))
    }
    pub(crate) fn with_events(events: Option<orchbee_server::SessionEventJournal>) -> Self {
        Self {
            events,
            ..Self::default()
        }
    }
    pub(crate) fn observation_busy(&self, id: &WorkbenchClientId) -> bool {
        self.receipts.values().any(|receipt| {
            &receipt.operation.command.client_id == id && receipt.operation.status.pending()
        })
    }
    pub(crate) fn finish_observation(&mut self, id: &WorkbenchRequestId, now: Instant) {
        if let Some(receipt) = self.receipts.get_mut(id)
            && matches!(
                receipt.operation.command.intent,
                ClientViewIntent::Viewports { .. }
            )
            && !receipt.operation.status.pending()
        {
            // Preserve a short exact-completion replay window for the Client heartbeat owner.
            receipt.completed_at = now.checked_sub(RECEIPT_RETENTION - QUEUE_LIFETIME);
        }
    }
    fn publish_view(
        &self,
        previous: Option<&ClientViewObservation>,
        current: &ClientViewRecord,
        completion: Option<&ClientViewControlStatus>,
    ) {
        use orchbee_workbench_protocol::{SemanticEventData as Data, SemanticEventKind as Kind};
        let Some(journal) = &self.events else {
            return;
        };
        let old = previous.and_then(|view| view.selection.as_ref());
        let new = current.view.selection.as_ref();
        let workspace =
            new.and_then(|selection| journal.workspace_for_context(&selection.terminal_context_id));
        let data = Data::Client {
            client_id: current.client_id.clone(),
            view: current.view.clone(),
            workspace_id: workspace.clone(),
        };
        let emit = |kind| {
            journal.publish(
                current.view.workbench_revision,
                workspace.clone(),
                kind,
                data.clone(),
            );
        };
        if new.is_some_and(|new| {
            old.is_none_or(|old| old.terminal_context_id != new.terminal_context_id)
        }) {
            emit(Kind::WorkspaceFocused);
        }
        if new.and_then(|new| new.tab_id.as_ref()).is_some()
            && old.and_then(|old| old.tab_id.as_ref()) != new.and_then(|new| new.tab_id.as_ref())
        {
            emit(Kind::TabFocused);
        }
        if new.and_then(|new| new.panel_id.as_ref()).is_some()
            && old.and_then(|old| old.panel_id.as_ref())
                != new.and_then(|new| new.panel_id.as_ref())
        {
            emit(Kind::PaneFocused);
        }
        let adjusted = matches!(completion, Some(ClientViewControlStatus::LayoutAdjusted { adjustment, .. }) if adjustment.change.changed());
        let changed = previous.is_some_and(|previous| {
            previous.split_revision.is_some()
                && current.view.split_revision.is_some()
                && previous.split_revision != current.view.split_revision
        }) || old.zip(new).is_some_and(|(old, new)| {
            old.zoomed != new.zoomed || old.panel_count != new.panel_count
        });
        if changed || adjusted {
            emit(Kind::LayoutUpdated);
        }
    }
    pub(crate) fn execute(
        &mut self,
        generation: NonZeroU64,
        action: ClientViewAction,
        now: Instant,
    ) -> ClientViewOutcome {
        self.expire(now);
        match action {
            ClientViewAction::Synchronize {
                heartbeat,
                view_revision,
                view,
                completion,
            } => {
                let Ok(client_id) = WorkbenchClientId::parse(&format!("{:032x}", generation.get()))
                else {
                    return unavailable(ClientViewUnavailable::SequenceExhausted);
                };
                self.synchronize(client_id, heartbeat, view_revision, *view, completion, now)
            }
            ClientViewAction::List {} => ClientViewOutcome::Listed {
                clients: self
                    .clients
                    .values()
                    .map(|client| client.observe(now))
                    .collect(),
            },
            ClientViewAction::Get { client_id } => {
                self.clients
                    .get(&client_id)
                    .map_or(ClientViewOutcome::NotFound {}, |client| {
                        ClientViewOutcome::Found {
                            client: Box::new(client.observe(now)),
                        }
                    })
            }
            ClientViewAction::Submit { command } => self.submit(*command, now),
            ClientViewAction::Read { operation_id } => self
                .receipts
                .get(&operation_id)
                .map_or(ClientViewOutcome::NotFound {}, |receipt| {
                    operation_outcome(&receipt.operation)
                }),
        }
    }

    pub(crate) fn disconnect(&mut self, generation: NonZeroU64, now: Instant) {
        self.expire(now);
        let Ok(client_id) = WorkbenchClientId::parse(&format!("{:032x}", generation.get())) else {
            return;
        };
        self.clients.remove(&client_id);
        for receipt in self
            .receipts
            .values_mut()
            .filter(|receipt| receipt.operation.command.client_id == client_id)
        {
            let status = match receipt.operation.status {
                ClientViewControlStatus::Queued {} => ClientViewControlStatus::Rejected {
                    reason: ClientViewControlReason::ClientDisconnected,
                },
                ClientViewControlStatus::Delivered {} => ClientViewControlStatus::Incomplete {
                    reason: ClientViewControlReason::ClientDisconnected,
                    view: None,
                },
                ClientViewControlStatus::PopupOpened { .. }
                | ClientViewControlStatus::PopupClosed { .. }
                | ClientViewControlStatus::PresentationRead { .. }
                | ClientViewControlStatus::ViewportRead { .. }
                | ClientViewControlStatus::NavigationRead { .. }
                | ClientViewControlStatus::TerminalAttentionRead { .. }
                | ClientViewControlStatus::AttentionRead { .. }
                | ClientViewControlStatus::LayoutAdjusted { .. }
                | ClientViewControlStatus::DirectionFocused { .. }
                | ClientViewControlStatus::DirectionUnchanged { .. }
                | ClientViewControlStatus::LayoutRead { .. }
                | ClientViewControlStatus::Applied { .. }
                | ClientViewControlStatus::ZoomUnchanged { .. }
                | ClientViewControlStatus::ZoomToggled { .. }
                | ClientViewControlStatus::Rejected { .. }
                | ClientViewControlStatus::Incomplete { .. } => continue,
            };
            receipt.settle(status, now);
        }
    }

    fn expire(&mut self, now: Instant) {
        for receipt in self.receipts.values_mut() {
            if matches!(receipt.operation.status, ClientViewControlStatus::Queued {})
                && now.saturating_duration_since(receipt.submitted_at) >= QUEUE_LIFETIME
            {
                receipt.settle(
                    ClientViewControlStatus::Rejected {
                        reason: ClientViewControlReason::Expired,
                    },
                    now,
                );
            }
        }
        self.receipts.retain(|_, receipt| {
            receipt.completed_at.is_none_or(|completed| {
                now.saturating_duration_since(completed) < RECEIPT_RETENTION
            })
        });
    }

    fn submit(&mut self, command: ClientViewControl, now: Instant) -> ClientViewOutcome {
        if let Some(receipt) = self.receipts.get(&command.operation_id) {
            return if receipt.operation.command == command {
                operation_outcome(&receipt.operation)
            } else {
                ClientViewOutcome::Conflict {}
            };
        }
        let Some(client) = self
            .clients
            .get(&command.client_id)
            .map(|client| client.observe(now))
        else {
            return unavailable(ClientViewUnavailable::ClientUnavailable);
        };
        let payload_reservation = if matches!(command.intent, ClientViewIntent::Presentation { .. })
        {
            orchbee_workbench_protocol::MAXIMUM_ENCODED_CLIENT_PRESENTATION_BYTES
        } else if matches!(command.intent, ClientViewIntent::Viewports { .. }) {
            orchbee_workbench_protocol::MAXIMUM_ENCODED_CLIENT_VIEWPORT_BYTES
        } else if matches!(command.intent, ClientViewIntent::Selections { .. }) {
            orchbee_workbench_protocol::MAXIMUM_ENCODED_CLIENT_NAVIGATION_BYTES
        } else if command.intent.is_attention_read() {
            MAXIMUM_ENCODED_CLIENT_ATTENTION_BYTES
        } else if matches!(command.intent, ClientViewIntent::Layout { .. }) {
            MAXIMUM_ENCODED_CLIENT_LAYOUT_BYTES
        } else {
            0
        };
        let reserved: usize = self
            .receipts
            .values()
            .map(|receipt| receipt.payload_reservation)
            .sum();
        if self.receipts.len() >= MAXIMUM_RECEIPTS
            || reserved.saturating_add(payload_reservation) > MAXIMUM_PAYLOAD_RESERVATION
        {
            return unavailable(ClientViewUnavailable::ReceiptCapacity);
        }
        let reason = if client.availability != ClientViewAvailability::Current
            || client.view_revision != command.expected_view_revision
        {
            Some(ClientViewControlReason::StaleView)
        } else if self.receipts.values().any(|receipt| {
            receipt.operation.command.client_id == command.client_id
                && receipt.operation.status.pending()
        }) || command.intent.blocked_by_busy_phase(client.view.phase)
        {
            Some(ClientViewControlReason::Busy)
        } else {
            None
        };
        let status = reason.map_or(ClientViewControlStatus::Queued {}, |reason| {
            ClientViewControlStatus::Rejected { reason }
        });
        let completed_at = (!status.pending()).then_some(now);
        let operation = ClientViewOperation { command, status };
        let outcome = operation_outcome(&operation);
        self.receipts.insert(
            operation.command.operation_id.clone(),
            Receipt {
                operation,
                submitted_at: now,
                completed_at,
                payload_reservation: if completed_at.is_none() {
                    payload_reservation
                } else {
                    0
                },
            },
        );
        outcome
    }

    fn synchronize(
        &mut self,
        client_id: WorkbenchClientId,
        heartbeat: ClientViewSequence,
        view_revision: ClientViewSequence,
        view: ClientViewObservation,
        completion: Option<ClientViewCompletion>,
        now: Instant,
    ) -> ClientViewOutcome {
        if view.validate().is_err() {
            return ClientViewOutcome::Conflict {};
        }
        if let Some(previous) = self.clients.get(&client_id) {
            if heartbeat < previous.record.heartbeat
                || view_revision < previous.record.view_revision
                || (view_revision == previous.record.view_revision && view != previous.record.view)
                || (heartbeat == previous.record.heartbeat
                    && view_revision != previous.record.view_revision)
            {
                return ClientViewOutcome::Conflict {};
            }
        } else if self.clients.len() >= MAXIMUM_CLIENT_VIEWS {
            return unavailable(ClientViewUnavailable::ServerBusy);
        }

        // Validate completion before publishing or settling anything. Only this admitted Session
        // can acknowledge a delivered command, and Applied must carry its actually observed target.
        let completion_status = if let Some(completion) = &completion {
            let Some(receipt) = self.receipts.get(&completion.operation_id) else {
                return ClientViewOutcome::Conflict {};
            };
            if receipt.operation.command.client_id != client_id
                || !matches!(
                    receipt.operation.status,
                    ClientViewControlStatus::Delivered {}
                )
            {
                return ClientViewOutcome::Conflict {};
            }
            let status = match &completion.outcome {
                ClientViewCompletionOutcome::PresentationRead { presentation } => {
                    ClientViewControlStatus::PresentationRead {
                        presentation: presentation.clone(),
                        view: view.clone(),
                    }
                }
                ClientViewCompletionOutcome::ViewportRead { viewport } => {
                    ClientViewControlStatus::ViewportRead {
                        viewport: viewport.clone(),
                        view: view.clone(),
                    }
                }
                ClientViewCompletionOutcome::NavigationRead { navigation } => {
                    ClientViewControlStatus::NavigationRead {
                        navigation: navigation.clone(),
                        view: view.clone(),
                    }
                }
                ClientViewCompletionOutcome::TerminalAttentionRead { attention } => {
                    ClientViewControlStatus::TerminalAttentionRead {
                        attention: attention.clone(),
                        view: view.clone(),
                    }
                }
                ClientViewCompletionOutcome::AttentionRead { attention } => {
                    ClientViewControlStatus::AttentionRead {
                        attention: attention.clone(),
                        view: view.clone(),
                    }
                }
                ClientViewCompletionOutcome::LayoutAdjusted { adjustment } => {
                    ClientViewControlStatus::LayoutAdjusted {
                        adjustment: adjustment.clone(),
                        view: view.clone(),
                    }
                }
                ClientViewCompletionOutcome::DirectionFocused { panel_id } => {
                    ClientViewControlStatus::DirectionFocused {
                        panel_id: panel_id.clone(),
                        view: view.clone(),
                    }
                }
                ClientViewCompletionOutcome::DirectionUnchanged { layout_revision } => {
                    ClientViewControlStatus::DirectionUnchanged {
                        layout_revision: *layout_revision,
                        view: view.clone(),
                    }
                }
                ClientViewCompletionOutcome::LayoutRead { layout } => {
                    ClientViewControlStatus::LayoutRead {
                        layout: layout.clone(),
                        view: view.clone(),
                    }
                }
                ClientViewCompletionOutcome::ZoomToggled { previously_zoomed } => {
                    ClientViewControlStatus::ZoomToggled {
                        previously_zoomed: *previously_zoomed,
                        view: view.clone(),
                    }
                }
                ClientViewCompletionOutcome::ZoomUnchanged {} => {
                    ClientViewControlStatus::ZoomUnchanged { view: view.clone() }
                }
                ClientViewCompletionOutcome::PopupOpened { popup_id } => {
                    ClientViewControlStatus::PopupOpened {
                        popup_id: popup_id.clone(),
                        view: view.clone(),
                    }
                }
                ClientViewCompletionOutcome::PopupClosed { had_popup } => {
                    ClientViewControlStatus::PopupClosed {
                        had_popup: *had_popup,
                        view: view.clone(),
                    }
                }
                ClientViewCompletionOutcome::Applied {} => {
                    ClientViewControlStatus::Applied { view: view.clone() }
                }
                ClientViewCompletionOutcome::Rejected { reason } => {
                    ClientViewControlStatus::Rejected { reason: *reason }
                }
                ClientViewCompletionOutcome::Incomplete { reason } => {
                    ClientViewControlStatus::Incomplete {
                        reason: *reason,
                        view: Some(view.clone()),
                    }
                }
            };
            if (ClientViewOperation {
                command: receipt.operation.command.clone(),
                status: status.clone(),
            })
            .validate()
            .is_err()
            {
                return ClientViewOutcome::Conflict {};
            }
            Some(status)
        } else {
            None
        };
        let published = PublishedView {
            record: ClientViewRecord {
                client_id: client_id.clone(),
                heartbeat,
                view_revision,
                view,
                availability: ClientViewAvailability::Current,
            },
            graphics: self
                .clients
                .get(&client_id)
                .filter(|previous| previous.record.heartbeat == heartbeat)
                .and_then(|previous| previous.graphics.clone()),
        };
        let client = published.observe(now);
        let previous = self
            .clients
            .get(&client_id)
            .map(|previous| previous.record.view.clone());
        self.clients.insert(client_id.clone(), published);
        self.publish_view(previous.as_ref(), &client, completion_status.as_ref());
        if let (Some(completion), Some(status)) = (completion, completion_status)
            && let Some(receipt) = self.receipts.get_mut(&completion.operation_id)
        {
            receipt.settle(status, now);
        }
        let mut control = None;
        if let Some(receipt) = self.receipts.values_mut().find(|receipt| {
            receipt.operation.command.client_id == client_id
                && matches!(receipt.operation.status, ClientViewControlStatus::Queued {})
        }) {
            let reason = if client.availability != ClientViewAvailability::Current
                || client.view_revision != receipt.operation.command.expected_view_revision
            {
                Some(ClientViewControlReason::StaleView)
            } else if receipt
                .operation
                .command
                .intent
                .blocked_by_busy_phase(client.view.phase)
            {
                Some(ClientViewControlReason::Busy)
            } else {
                None
            };
            if let Some(reason) = reason {
                receipt.settle(ClientViewControlStatus::Rejected { reason }, now);
            } else {
                receipt.operation.status = ClientViewControlStatus::Delivered {};
                control = Some(Box::new(receipt.operation.command.clone()));
            }
        }
        ClientViewOutcome::Synchronized {
            client: Box::new(client),
            control,
        }
    }
}

fn unavailable(reason: ClientViewUnavailable) -> ClientViewOutcome {
    ClientViewOutcome::Unavailable { reason }
}
fn operation_outcome(operation: &ClientViewOperation) -> ClientViewOutcome {
    ClientViewOutcome::Operation {
        operation: Box::new(operation.clone()),
    }
}

#[cfg(test)]
mod tests;
