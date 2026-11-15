//! Prepares and commits retirement of a disposed physical target's viewer, controller, title,
//! and color state, retaining bounded tombstones for late events. It also remembers attachments
//! detached in this Host epoch, so their late events are dropped, and attachments whose Detach no
//! request had room for, which later cleanups detach. It does not dispose the Host terminal or
//! publish cleanup before its owner confirms the physical outcome.

use std::{collections::VecDeque, io};

use orchbee_capability_node_protocol::{HostTerminalId, HostTerminalIdentity, OperationId};
use orchbee_terminal_protocol::{
    AttachmentControl, CURRENT_PROTOCOL_VERSION, HistoryControl, HostEpoch, MessageRole,
    PayloadEncoding, ProjectionDemandControl, SelectionControl, SelectionExportControl,
    TerminalEffectEvent, decode_attachment_control, decode_history_control,
    decode_projection_demand_control, decode_selection_control,
    decode_selection_export_chunk_route, decode_selection_export_control,
    decode_terminal_effect_event, decode_transport_envelope, inspect_terminal_frame_route,
};

use super::{HostBrokerConnection, viewers::MAXIMUM_VIEWERS};

#[derive(Clone, Copy)]
struct RemovedTarget {
    identity: HostTerminalIdentity,
    viewers: [Option<OperationId>; MAXIMUM_VIEWERS],
    controller: Option<OperationId>,
    disposed: bool,
}

/// Detached attachments remembered for their late events; the oldest is forgotten first.
const MAXIMUM_DETACHED_ATTACHMENTS: usize = 256;
/// While this many Detaches wait for cleanup, a request that would open another controller or
/// transient attachment is refused before it asks the Host. Each controller is released once and
/// at most eight exist, so the backlog never holds more than seventeen.
const DETACH_BACKLOG_LIMIT: usize = 8;

#[derive(Default)]
pub(super) struct RetiredTargets {
    // At most one target per generation-local retained Close binding. Capacity is
    // obtained before the request; the ACK path only overwrites an existing cell.
    targets: Vec<RemovedTarget>,
    /// Viewer and controller attachments detached in this Host epoch. The Host may still have
    /// sent events for them before it handled the Detach.
    detached: VecDeque<[u8; 16]>,
    /// Controller and transient attachments this Worker no longer uses whose Detach did not fit
    /// in a request, oldest first.
    pending: VecDeque<[u8; 16]>,
}

#[derive(Clone, Copy)]
pub(crate) struct PreparedTargetCleanup {
    index: usize,
    target: RemovedTarget,
}

impl HostBrokerConnection {
    pub(crate) fn prepare_target_cleanup(
        &self,
        identity: HostTerminalIdentity,
    ) -> io::Result<PreparedTargetCleanup> {
        let target = RemovedTarget {
            identity,
            viewers: self
                .viewers
                .borrow()
                .target_ids(identity.terminal_id())
                .map_err(io::Error::other)?,
            controller: self
                .controllers
                .borrow()
                .for_terminal(identity.terminal_id()),
            disposed: false,
        };
        let mut retired = self.retired_targets.borrow_mut();
        let index = if let Some(index) = retired
            .targets
            .iter()
            .position(|target| target.identity == identity)
        {
            index
        } else {
            if retired.targets.len()
                >= crate::terminal_authorization::MAXIMUM_RETAINED_TERMINAL_BINDINGS
            {
                return Err(io::Error::other("retained Close target capacity exhausted"));
            }
            retired.targets.try_reserve(1).map_err(io::Error::other)?;
            let index = retired.targets.len();
            retired.targets.push(target);
            index
        };
        Ok(PreparedTargetCleanup { index, target })
    }

    pub(crate) fn commit_target_cleanup(&self, prepared: PreparedTargetCleanup) {
        // The prepared cell cannot be removed or replaced during the single Host
        // slot. All allocation, conversion and target discovery precede handoff.
        self.retired_targets.borrow_mut().targets[prepared.index] = RemovedTarget {
            disposed: true,
            ..prepared.target
        };
        self.titles.borrow_mut().remove(prepared.target.identity);
        self.host_colors
            .borrow_mut()
            .remove(prepared.target.identity.terminal_id().as_bytes());
        let mut viewers = self.viewers.borrow_mut();
        for id in prepared.target.viewers.into_iter().flatten() {
            viewers.remove(id);
        }
        if let Some(id) = prepared.target.controller {
            self.controllers.borrow_mut().remove(id);
        }
    }

    /// A newly admitted attachment no longer counts as detached, so its own events are delivered.
    pub(crate) fn note_attached(&self, id: OperationId) {
        self.retired_targets
            .borrow_mut()
            .detached
            .retain(|detached| *detached != id.as_bytes());
    }

    /// Forgets a viewer this Worker detached and remembers it for late events.
    pub(crate) fn forget_viewer(&self, id: OperationId) {
        self.viewers.borrow_mut().remove(id);
        self.retired_targets.borrow_mut().note_detached(id);
    }

    /// Forgets a controller this Worker released and remembers it for late events.
    pub(crate) fn forget_controller(&self, id: OperationId) {
        self.controllers.borrow_mut().remove(id);
        self.retired_targets.borrow_mut().note_detached(id);
    }

    /// Leaves the Detach of an attachment this Worker no longer uses to the next cleanups. Its late
    /// events are dropped meanwhile.
    pub(crate) fn defer_detach(&self, id: [u8; 16]) {
        let mut retired = self.retired_targets.borrow_mut();
        retired.note_detached_bytes(id);
        if !retired.pending.contains(&id) {
            retired.pending.push_back(id);
        }
    }

    /// The deferred Detaches, oldest first.
    pub(crate) fn pending_detaches(&self) -> Vec<[u8; 16]> {
        self.retired_targets
            .borrow()
            .pending
            .iter()
            .copied()
            .collect()
    }

    /// A deferred Detach that a cleanup has settled, whatever the Host answered.
    pub(crate) fn settle_detach(&self, id: [u8; 16]) {
        self.retired_targets
            .borrow_mut()
            .pending
            .retain(|pending| *pending != id);
    }

    /// Whether a request that would open another controller or transient attachment waits until
    /// cleanup has caught up.
    pub(crate) fn detach_backlog_full(&self) -> bool {
        self.retired_targets.borrow().pending.len() >= DETACH_BACKLOG_LIMIT
    }

    pub(crate) fn disposed_viewer(&self, id: OperationId) -> bool {
        self.retired_targets.borrow().viewer(id.as_bytes())
    }

    pub(crate) fn disposed_terminal(&self, terminal: HostTerminalId) -> bool {
        self.retired_targets
            .borrow()
            .targets
            .iter()
            .any(|target| target.disposed && target.identity.terminal_id() == terminal)
    }
}

impl RetiredTargets {
    fn note_detached(&mut self, id: OperationId) {
        self.note_detached_bytes(id.as_bytes());
    }

    fn note_detached_bytes(&mut self, id: [u8; 16]) {
        if self.detached.contains(&id) {
            return;
        }
        if self.detached.len() == MAXIMUM_DETACHED_ATTACHMENTS {
            self.detached.pop_front();
        }
        self.detached.push_back(id);
    }

    /// A viewer of a disposed target. Disposal answers a later Attach of the same identity, so
    /// only these count for [`HostBrokerConnection::disposed_viewer`].
    fn viewer(&self, id: [u8; 16]) -> bool {
        self.targets
            .iter()
            .filter(|target| target.disposed)
            .any(|target| {
                target
                    .viewers
                    .iter()
                    .flatten()
                    .any(|viewer| viewer.as_bytes() == id)
            })
    }

    /// An attachment whose late events are dropped: a disposed target's viewer, or one this Worker
    /// detached or no longer uses.
    fn retired_viewer(&self, id: [u8; 16]) -> bool {
        self.detached.contains(&id) || self.pending.contains(&id) || self.viewer(id)
    }

    pub(super) fn accept_event(&self, record: &[u8], epoch: HostEpoch) -> io::Result<bool> {
        let envelope = decode_transport_envelope(record).map_err(io::Error::other)?;
        if envelope.protocol != CURRENT_PROTOCOL_VERSION
            || envelope.role != MessageRole::Event
            || envelope.correlation_id.is_some()
            || envelope.host_epoch != Some(epoch)
            || envelope.optional_message
        {
            return Ok(false);
        }
        if matches!(envelope.kind, 41 | 42)
            && envelope.payload_encoding == PayloadEncoding::TerminalFrame
        {
            let route = inspect_terminal_frame_route(envelope.payload).map_err(io::Error::other)?;
            return Ok(route.host_epoch == epoch
                && (envelope.kind == 41) == route.base_revision.is_none()
                && self.retired_viewer(*route.attachment_id.as_bytes()));
        }
        if envelope.kind == 65 && envelope.payload_encoding == PayloadEncoding::RawBytes {
            let route =
                decode_selection_export_chunk_route(envelope.kind, envelope.role, envelope.payload)
                    .map_err(io::Error::other)?;
            return Ok(self.retired_viewer(*route.attachment_id.as_bytes()));
        }
        if envelope.payload_encoding != PayloadEncoding::ControlJson {
            return Ok(false);
        }
        if envelope.kind == 23 {
            let AttachmentControl::ResizeLeaseChanged(event) =
                decode_attachment_control(envelope.kind, envelope.role, envelope.payload)
                    .map_err(io::Error::other)?
            else {
                return Ok(false);
            };
            if self.detached.contains(event.attachment_id.as_bytes())
                || self.pending.contains(event.attachment_id.as_bytes())
            {
                return Ok(true);
            }
            return Ok(self.targets.iter().any(|target| {
                target.disposed
                    && target.identity.terminal_id().as_bytes() == *event.terminal_id.as_bytes()
                    && target
                        .controller
                        .is_some_and(|id| id.as_bytes() == *event.attachment_id.as_bytes())
            }));
        }
        if envelope.kind == 49 {
            let ProjectionDemandControl::ProjectionBlocked(event) =
                decode_projection_demand_control(envelope.kind, envelope.role, envelope.payload)
                    .map_err(io::Error::other)?
            else {
                return Ok(false);
            };
            return Ok(self.retired_viewer(*event.attachment_id.as_bytes()));
        }
        if envelope.kind == 53 {
            let HistoryControl::HistoryChanged(event) =
                decode_history_control(envelope.kind, envelope.role, envelope.payload)
                    .map_err(io::Error::other)?
            else {
                return Ok(false);
            };
            return Ok(self.retired_viewer(*event.attachment_id.as_bytes()));
        }
        if envelope.kind == 62 {
            let SelectionControl::ChangedEvent(event) =
                decode_selection_control(envelope.kind, envelope.role, envelope.payload)
                    .map_err(io::Error::other)?
            else {
                return Ok(false);
            };
            return Ok(self.retired_viewer(*event.attachment_id.as_bytes()));
        }
        if envelope.kind == 67 {
            let SelectionExportControl::FinishedEvent(event) =
                decode_selection_export_control(envelope.kind, envelope.role, envelope.payload)
                    .map_err(io::Error::other)?
            else {
                return Ok(false);
            };
            return Ok(self.retired_viewer(*event.attachment_id.as_bytes()));
        }
        Ok(
            match decode_terminal_effect_event(envelope.kind, envelope.role, envelope.payload) {
                Ok(TerminalEffectEvent::Bell(event)) => {
                    self.retired_viewer(*event.attachment_id.as_bytes())
                }
                Ok(TerminalEffectEvent::ClipboardWriteRequest(event)) => {
                    self.retired_viewer(*event.attachment_id.as_bytes())
                }
                // This event stream observes only its declared lifecycle/effect records; unrelated records cannot advance it.
                _ => false,
            },
        )
    }
}

#[cfg(test)]
mod tests;
