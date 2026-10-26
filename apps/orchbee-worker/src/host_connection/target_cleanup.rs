//! Prepares and commits retirement of a disposed physical target's viewer, controller, title,
//! and color state, retaining bounded tombstones for late events. It does not dispose the Host
//! terminal or publish cleanup before its owner confirms the physical outcome.

use std::io;

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

#[derive(Default)]
pub(super) struct RetiredTargets {
    // At most one target per generation-local retained Close binding. Capacity is
    // obtained before the request; the ACK path only overwrites an existing cell.
    targets: Vec<RemovedTarget>,
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
                && self.viewer(*route.attachment_id.as_bytes()));
        }
        if envelope.kind == 65 && envelope.payload_encoding == PayloadEncoding::RawBytes {
            let route =
                decode_selection_export_chunk_route(envelope.kind, envelope.role, envelope.payload)
                    .map_err(io::Error::other)?;
            return Ok(self.viewer(*route.attachment_id.as_bytes()));
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
            return Ok(self.viewer(*event.attachment_id.as_bytes()));
        }
        if envelope.kind == 53 {
            let HistoryControl::HistoryChanged(event) =
                decode_history_control(envelope.kind, envelope.role, envelope.payload)
                    .map_err(io::Error::other)?
            else {
                return Ok(false);
            };
            return Ok(self.viewer(*event.attachment_id.as_bytes()));
        }
        if envelope.kind == 62 {
            let SelectionControl::ChangedEvent(event) =
                decode_selection_control(envelope.kind, envelope.role, envelope.payload)
                    .map_err(io::Error::other)?
            else {
                return Ok(false);
            };
            return Ok(self.viewer(*event.attachment_id.as_bytes()));
        }
        if envelope.kind == 67 {
            let SelectionExportControl::FinishedEvent(event) =
                decode_selection_export_control(envelope.kind, envelope.role, envelope.payload)
                    .map_err(io::Error::other)?
            else {
                return Ok(false);
            };
            return Ok(self.viewer(*event.attachment_id.as_bytes()));
        }
        Ok(
            match decode_terminal_effect_event(envelope.kind, envelope.role, envelope.payload) {
                Ok(TerminalEffectEvent::Bell(event)) => {
                    self.viewer(*event.attachment_id.as_bytes())
                }
                Ok(TerminalEffectEvent::ClipboardWriteRequest(event)) => {
                    self.viewer(*event.attachment_id.as_bytes())
                }
                // This event stream observes only its declared lifecycle/effect records; unrelated records cannot advance it.
                _ => false,
            },
        )
    }
}

#[cfg(test)]
mod tests;
