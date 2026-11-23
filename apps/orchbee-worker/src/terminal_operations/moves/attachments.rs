//! What follows a move's claim in the two Hosts' attachments, and the old copy's end at the next
//! turn (`proposals/platform/host-handover.md`, The steps of a move). Each controller grant becomes
//! a controller attachment in the new Host with the same viewer, identities, and consumed sequence,
//! so input the Server held during the move still passes; each viewer closes there for its Client
//! to attach again, since a Client checks the Host epoch of every projection record.

use std::{error::Error, time::Duration};

use orchbee_capability_node_protocol::{HostTerminalId, HostTerminalIdentity, OperationId};
use orchbee_local_ipc::LocalStream as UnixStream;
use orchbee_terminal_protocol::{
    AttachTerminal, AttachmentCapabilityState, AttachmentControl, AttachmentId, AttachmentTarget,
    CorrelationId, HandoverControl, LifecycleControl, PayloadEncoding, ProjectionDemand,
    TerminalId, TerminalTarget, decode_attachment_control, decode_lifecycle_control,
    decode_transport_envelope, encode_attachment_control, encode_lifecycle_control,
};

use super::steps::classify;
use crate::{
    host_connection::{HostBrokerConnection, NotStarted, UnexpectedAnswer, exchange_within},
    session_keeper::{Serving, request},
    worker_identity::fresh_identity,
};

/// Each attachment step, a Detach, an Attach, or the old copy's end.
const STEP_BUDGET: Duration = Duration::from_secs(2);

/// Moves the attachments of the terminal that ran as `from` in `old` to `new`, where it runs as
/// `to`. An attachment step this request has no room for is left to cleanup, and a controller
/// the new Host does not take is gone, so its Client acquires control again once it attached.
pub(super) fn transfer(
    old: &HostBrokerConnection,
    new: &HostBrokerConnection,
    server: &UnixStream,
    from: HostTerminalId,
    to: HostTerminalId,
) -> Result<(), Box<dyn Error>> {
    let viewers = old.viewers.borrow().target_ids(from)?;
    for id in viewers.into_iter().flatten() {
        let grant = old.controllers.borrow().for_viewer(id);
        let open = old.viewers.borrow().open(id);
        let demand = old
            .viewers
            .borrow()
            .demand(id)
            .unwrap_or(ProjectionDemand::Paused);
        if let Some(grant) = grant {
            detach(old, server, grant.controller_id)?;
            old.forget_controller(grant.controller_id);
        }
        detach(old, server, id)?;
        old.forget_viewer(id);
        if !open
            || new
                .viewers
                .borrow_mut()
                .adopt_moved(id, to, demand)
                .is_err()
        {
            continue;
        }
        let Some(grant) = grant.filter(|grant| grant.capability_revision.is_some()) else {
            continue;
        };
        let Some(revision) = attach_controller(new, server, grant.controller_id, to)? else {
            continue;
        };
        if !new
            .controllers
            .borrow_mut()
            .adopt_moved(grant, to, revision)
        {
            detach(new, server, grant.controller_id)?;
        }
    }
    Ok(())
}

/// Ends the old copy of a terminal that moved: a running one is dropped, which ends no program,
/// and a settled one disposed. Detaches no earlier request had room for go first, since a drop
/// needs every attachment gone. Returns whether the old copy is gone; a refusal leaves it for the
/// next turn.
pub(super) fn end_old_copy(
    old: &HostBrokerConnection,
    server: &UnixStream,
    identity: HostTerminalIdentity,
    settled: bool,
) -> Result<bool, Box<dyn Error>> {
    if NotStarted::optional(crate::terminal_viewer_operation::settle_deferred_detaches(
        old, server,
    ))?
    .is_none()
    {
        return Ok(false);
    }
    let terminal = TerminalId::from_bytes(identity.terminal_id().as_bytes())?;
    let ended = if settled {
        dispose(old, server, terminal)?
    } else {
        let drop = HandoverControl::DropTerminal(terminal);
        match request(old, Serving::Within(server, STEP_BUDGET), &drop) {
            Ok(HandoverControl::TerminalDropped(dropped)) => dropped == terminal,
            Ok(_) => false,
            Err(error) => {
                let refused = classify(error)?;
                crate::diagnostic_events::terminal_move_failed(
                    "drop",
                    refused.class(),
                    refused.reason(),
                    false,
                );
                false
            }
        }
    };
    if ended {
        old.forget_moved(identity);
    }
    Ok(ended)
}

/// Disposes a settled terminal's old copy; whether the Host accepted.
fn dispose(
    host: &HostBrokerConnection,
    server: &UnixStream,
    terminal: TerminalId,
) -> Result<bool, Box<dyn Error>> {
    let target = TerminalTarget {
        terminal_id: terminal,
    };
    let command = encode_lifecycle_control(&LifecycleControl::DisposeTerminal(target))?;
    let response = match exchange_within(
        host,
        server,
        STEP_BUDGET,
        CorrelationId::from_bytes(fresh_identity()?)?,
        command.kind,
        &command.payload,
        PayloadEncoding::ControlJson,
    ) {
        Ok(response) => response,
        Err(error) if NotStarted::is(error.as_ref()) || error.is::<UnexpectedAnswer>() => {
            return Ok(false);
        }
        Err(error) => return Err(error),
    };
    let envelope = decode_transport_envelope(&response)?;
    Ok(
        decode_lifecycle_control(envelope.kind, envelope.role, envelope.payload).ok()
            == Some(LifecycleControl::DisposeTerminalAccepted(target)),
    )
}

/// Attaches a controller to `terminal` in `host` under the moved grant's identity; the capability
/// revision the Host granted, or `None` when it refused or the request has no room.
fn attach_controller(
    host: &HostBrokerConnection,
    server: &UnixStream,
    controller: OperationId,
    terminal: HostTerminalId,
) -> Result<Option<std::num::NonZeroU64>, Box<dyn Error>> {
    let attachment_id = AttachmentId::from_bytes(controller.as_bytes())?;
    let terminal_id = TerminalId::from_bytes(terminal.as_bytes())?;
    let capabilities = AttachmentCapabilityState {
        read: false,
        input: true,
        resize: true,
    };
    let command = encode_attachment_control(&AttachmentControl::AttachTerminal(AttachTerminal {
        attachment_id,
        terminal_id,
        capabilities,
        request_resize_lease: true,
    }))?;
    let response = match exchange_within(
        host,
        server,
        STEP_BUDGET,
        CorrelationId::from_bytes(fresh_identity()?)?,
        command.kind,
        &command.payload,
        PayloadEncoding::ControlJson,
    ) {
        Ok(response) => response,
        Err(error) if NotStarted::is(error.as_ref()) || error.is::<UnexpectedAnswer>() => {
            return Ok(None);
        }
        Err(error) => return Err(error),
    };
    let envelope = decode_transport_envelope(&response)?;
    let Ok(AttachmentControl::AttachmentCapabilities(accepted)) =
        decode_attachment_control(envelope.kind, envelope.role, envelope.payload)
    else {
        return Ok(None);
    };
    if accepted.attachment_id != attachment_id
        || accepted.terminal_id != terminal_id
        || accepted.capabilities != capabilities
        || !accepted.resize_lease
    {
        // A grant that disagrees with what was asked is never used.
        detach(host, server, controller)?;
        return Ok(None);
    }
    Ok(Some(accepted.capability_revision))
}

/// Detaches attachment `id` from `host`: any correlated answer ends it, and one this request has
/// no room for is left to cleanup.
fn detach(
    host: &HostBrokerConnection,
    server: &UnixStream,
    id: OperationId,
) -> Result<(), Box<dyn Error>> {
    let target = AttachmentTarget {
        attachment_id: AttachmentId::from_bytes(id.as_bytes())?,
    };
    let command = encode_attachment_control(&AttachmentControl::DetachTerminal(target))?;
    match exchange_within(
        host,
        server,
        STEP_BUDGET,
        CorrelationId::from_bytes(fresh_identity()?)?,
        command.kind,
        &command.payload,
        PayloadEncoding::ControlJson,
    ) {
        Ok(_) => Ok(()),
        Err(error) if NotStarted::is(error.as_ref()) => {
            host.defer_detach(id.as_bytes());
            Ok(())
        }
        Err(error) if error.is::<UnexpectedAnswer>() => Ok(()),
        Err(error) => Err(error),
    }
}
