//! Read-only inventory behind an admitted viewer; never enters the exclusive lifecycle kernel.
use super::{
    CURRENT_PROTOCOL_VERSION, CorrelationId, Duration, Envelope, Error, HostBrokerConnection,
    HostExchange, MessageRole, PayloadEncoding, TerminalViewerOutcome, UnixStream, ViewerRejection,
    decode_transport_envelope, encode_transport_envelope, fresh_identity, invalid, io,
};
use orchbee_capability_node_protocol::{HostTerminalIdentity, LifecycleTerminalPhase};
use orchbee_terminal_protocol::{
    InventoryControl, ListTerminals, decode_inventory_control, encode_inventory_control,
};
use std::collections::BTreeSet;

pub(super) fn read(
    host: &HostBrokerConnection,
    server: &UnixStream,
    attachment: orchbee_capability_node_protocol::OperationId,
    target: HostTerminalIdentity,
) -> Result<TerminalViewerOutcome, Box<dyn Error>> {
    if host.host_epoch().as_bytes() != &target.host_epoch().as_bytes()
        || host.viewers.borrow().target(attachment) != Some(target.terminal_id())
    {
        return Ok(TerminalViewerOutcome::Rejected(
            ViewerRejection::AttachmentUnavailable,
        ));
    }
    let found = inventory(
        host,
        server,
        &target.terminal_id().as_bytes(),
        Some(&target.creation_id().as_bytes()),
    )?;
    Ok(match found {
        Some(summary) => {
            let (core, phase) = crate::terminal_lifecycle_operation::summary_observation(&summary);
            TerminalViewerOutcome::LifecycleObserved {
                target,
                revision: core.revision,
                settled: matches!(
                    phase,
                    LifecycleTerminalPhase::Exited { .. } | LifecycleTerminalPhase::Failed
                ),
            }
        }
        None => TerminalViewerOutcome::Rejected(ViewerRejection::AttachmentUnavailable),
    })
}

/// Seed newly attached viewers from retained inventory, including after a Client restart.
/// No running process is inferred from a missing notification.
pub(super) fn snapshot(
    host: &HostBrokerConnection,
    server: &UnixStream,
    terminal: orchbee_capability_node_protocol::HostTerminalId,
) -> Result<Option<Vec<u8>>, Box<dyn Error>> {
    use orchbee_terminal_protocol::{
        LifecycleControl, TerminalExited, TerminalFailed, TerminalSummary, encode_lifecycle_control,
    };
    let event = match inventory(host, server, &terminal.as_bytes(), None)? {
        Some(TerminalSummary::Exited { core, exit, .. }) => {
            LifecycleControl::TerminalExited(TerminalExited {
                identity: core.identity,
                final_revision: core.revision,
                exit,
            })
        }
        Some(TerminalSummary::Failed {
            core,
            failure,
            process,
            ..
        }) => LifecycleControl::TerminalFailed(TerminalFailed {
            identity: core.identity,
            final_revision: core.revision,
            failure,
            process,
        }),
        Some(_) => return Ok(None),
        None => return Err(invalid("lifecycle target absent after attach").into()),
    };
    let event = encode_lifecycle_control(&event)?;
    Ok(Some(encode_transport_envelope(&Envelope {
        protocol: CURRENT_PROTOCOL_VERSION,
        host_epoch: Some(host.host_epoch()),
        correlation_id: None,
        optional_message: false,
        kind: event.kind,
        role: MessageRole::Event,
        payload_encoding: PayloadEncoding::ControlJson,
        payload: &event.payload,
    })?))
}

fn inventory(
    host: &HostBrokerConnection,
    server: &UnixStream,
    terminal: &[u8; 16],
    creation: Option<&[u8; 16]>,
) -> Result<Option<orchbee_terminal_protocol::TerminalSummary>, Box<dyn Error>> {
    let mut exchange = HostExchange::new(host, server, Duration::from_secs(2))
        .ok_or_else(|| invalid("lifecycle observation budget"))?;
    exchange
        .prepare()
        .map_err(|_| invalid("lifecycle observation preflight"))?;
    let mut cursor = None;
    let mut found = None;
    let mut terminals = BTreeSet::new();
    let mut creations = BTreeSet::new();
    for _ in 0..8 {
        let correlation = CorrelationId::from_bytes(fresh_identity()?)?;
        let command = encode_inventory_control(&InventoryControl::ListTerminals(ListTerminals {
            cursor,
            maximum_terminals: 128,
        }))?;
        let record = encode_transport_envelope(&Envelope {
            correlation_id: Some(correlation),
            host_epoch: Some(host.host_epoch()),
            kind: command.kind,
            optional_message: false,
            payload: &command.payload,
            payload_encoding: PayloadEncoding::ControlJson,
            protocol: CURRENT_PROTOCOL_VERSION,
            role: MessageRole::Request,
        })?;
        let response = exchange
            .round_trip_bounded(
                &record,
                orchbee_terminal_protocol::MAXIMUM_CONTROL_JSON_BYTES,
            )
            .map_err(|_| invalid("lifecycle observation exchange"))?;
        let envelope = decode_transport_envelope(&response)?;
        if envelope.correlation_id != Some(correlation)
            || envelope.host_epoch != Some(host.host_epoch())
            || envelope.role != MessageRole::Response
            || envelope.protocol != CURRENT_PROTOCOL_VERSION
            || envelope.optional_message
            || envelope.payload_encoding != PayloadEncoding::ControlJson
        {
            return Err(invalid("lifecycle observation envelope").into());
        }
        let InventoryControl::TerminalList(page) =
            decode_inventory_control(envelope.kind, envelope.role, envelope.payload)?
        else {
            return Err(invalid("lifecycle observation payload").into());
        };
        validate_page(
            &page,
            cursor,
            (host.host_epoch(), terminal, creation),
            &mut terminals,
            &mut creations,
            &mut found,
        )?;
        cursor = page.next_cursor;
        if cursor.is_none() {
            return Ok(found);
        }
    }
    Err(invalid("lifecycle observation page limit").into())
}

fn validate_page(
    page: &orchbee_terminal_protocol::TerminalList,
    cursor: Option<orchbee_terminal_protocol::TerminalInventoryCursor>,
    target: (
        orchbee_terminal_protocol::HostEpoch,
        &[u8; 16],
        Option<&[u8; 16]>,
    ),
    terminals: &mut BTreeSet<[u8; 16]>,
    creations: &mut BTreeSet<[u8; 16]>,
    found: &mut Option<orchbee_terminal_protocol::TerminalSummary>,
) -> Result<(), io::Error> {
    let (epoch, target_terminal, target_creation) = target;
    let offset = cursor.map_or(0, |c| c.offset);
    if page.terminals.len() > 128
        || cursor.is_some_and(|c| c.inventory_generation != page.inventory_generation)
        || page.next_cursor.is_some_and(|next| {
            next.inventory_generation != page.inventory_generation
                || page.terminals.is_empty()
                || Some(next.offset) != offset.checked_add(page.terminals.len() as u64)
        })
    {
        return Err(invalid("lifecycle observation cursor"));
    }
    for summary in &page.terminals {
        let (core, _) = crate::terminal_lifecycle_operation::summary_observation(summary);
        let terminal = *core.identity.terminal_id.as_bytes();
        let creation = *core.identity.creation_id.as_bytes();
        if core.identity.host_epoch != epoch
            || !terminals.insert(terminal)
            || !creations.insert(creation)
        {
            return Err(invalid("lifecycle observation identity"));
        }
        if &terminal == target_terminal || target_creation == Some(&creation) {
            if &terminal != target_terminal
                || target_creation.is_some_and(|expected| expected != &creation)
                || found.is_some()
            {
                return Err(invalid("lifecycle observation target"));
            }
            *found = Some(summary.clone());
        }
    }
    Ok(())
}
