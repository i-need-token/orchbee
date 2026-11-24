//! Read-only lifecycle observation behind an admitted viewer.
use super::{
    CURRENT_PROTOCOL_VERSION, ConnectionControl, CorrelationId, Envelope, Error,
    HostBrokerConnection, HostExchange, MessageRole, PayloadEncoding, TerminalViewerOutcome,
    UnexpectedAnswer, UnixStream, ViewerRejection, decode_connection_control,
    encode_transport_envelope, fresh_identity, response_envelope,
};
use crate::terminal_lifecycle_operation::lookup::{self, LookupFailure};
use orchbee_capability_node_protocol::HostTerminalIdentity;
use orchbee_terminal_protocol::{
    CreationId, HostEpoch, InventoryControl, LifecycleControl, ListTerminals,
    MAXIMUM_CONTROL_JSON_BYTES, ReadTerminalLifecycle, TerminalExited, TerminalFailed, TerminalId,
    TerminalLifecycleSnapshot, TerminalLifecycleState, TerminalList, decode_inventory_control,
    encode_inventory_control, encode_lifecycle_control,
};
use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};

/// A walk starts pages until this budget passes and ends as soon as the Host's inventory does. A
/// page already sent is read to its end within the connection's Host exchange bound, so a walk that
/// runs out of budget leaves the Host stream in step for the next request.
const INVENTORY_BUDGET: Duration = Duration::from_secs(2);

/// The inner refusal concerns only its own request, while the outer error is the
/// request's failure, as [`inventory`] describes.
type Observed<T> = Result<Result<T, ViewerRejection>, Box<dyn Error>>;

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
    host.pacing
        .set_site(crate::diagnostic_events::Site::Lifecycle);
    let found = match observe(
        host,
        server,
        ReadTerminalLifecycle {
            terminal_id: TerminalId::from_bytes(target.terminal_id().as_bytes())?,
            creation_id: Some(CreationId::from_bytes(target.creation_id().as_bytes())?),
        },
    )? {
        Ok(found) => found,
        Err(reason) => return Ok(TerminalViewerOutcome::Rejected(reason)),
    };
    Ok(match found {
        Some(snapshot) => TerminalViewerOutcome::LifecycleObserved {
            target,
            revision: snapshot.revision,
            settled: matches!(
                snapshot.state,
                TerminalLifecycleState::Exited { .. } | TerminalLifecycleState::Failed { .. }
            ),
        },
        None => TerminalViewerOutcome::Rejected(ViewerRejection::AttachmentUnavailable),
    })
}

/// Seed newly attached viewers from the retained Host outcome, including after a Client restart.
pub(super) fn snapshot(
    host: &HostBrokerConnection,
    server: &UnixStream,
    terminal_id: TerminalId,
) -> Observed<Option<Vec<u8>>> {
    let snapshot = match observe(
        host,
        server,
        ReadTerminalLifecycle {
            terminal_id,
            creation_id: None,
        },
    )? {
        Ok(Some(snapshot)) => snapshot,
        // The Host just accepted the Attach yet the terminal is absent.
        Ok(None) => return Ok(Err(ViewerRejection::HostRejected)),
        Err(reason) => return Ok(Err(reason)),
    };
    let event = match snapshot.state {
        TerminalLifecycleState::Exited { exit, .. } => {
            LifecycleControl::TerminalExited(TerminalExited {
                identity: snapshot.identity,
                final_revision: snapshot.revision,
                exit,
            })
        }
        TerminalLifecycleState::Failed { failure, process } => {
            LifecycleControl::TerminalFailed(TerminalFailed {
                identity: snapshot.identity,
                final_revision: snapshot.revision,
                failure,
                process,
            })
        }
        _ => return Ok(Ok(None)),
    };
    let event = encode_lifecycle_control(&event)?;
    Ok(Ok(Some(encode_transport_envelope(&Envelope {
        protocol: CURRENT_PROTOCOL_VERSION,
        host_epoch: Some(host.host_epoch()),
        correlation_id: None,
        optional_message: false,
        kind: event.kind,
        role: MessageRole::Event,
        payload_encoding: PayloadEncoding::ControlJson,
        payload: &event.payload,
    })?)))
}

fn observe(
    host: &HostBrokerConnection,
    server: &UnixStream,
    target: ReadTerminalLifecycle,
) -> Observed<Option<TerminalLifecycleSnapshot>> {
    if !host.terminal_lifecycle_read() {
        return inventory(host, server, target);
    }
    let Ok(mut exchange) = HostExchange::open(host, server) else {
        return Ok(Err(ViewerRejection::CapacityExceeded));
    };
    exchange.prepare().map_err(|class| exchange.error(class))?;
    let command = encode_lifecycle_control(&LifecycleControl::ReadTerminalLifecycle(target))?;
    let (response, correlation) =
        read_control(host, &mut exchange, command.kind, &command.payload)?;
    let envelope = response_envelope(&response, correlation, host.host_epoch())?;
    match lookup::decode(target, &envelope) {
        Ok(snapshot) => Ok(Ok(snapshot)),
        Err(LookupFailure::Refused) => Ok(Err(ViewerRejection::HostRejected)),
        Err(LookupFailure::IdentityConflict | LookupFailure::InvalidResponse) => {
            Err(UnexpectedAnswer::of(&response).into())
        }
    }
}

fn read_control(
    host: &HostBrokerConnection,
    exchange: &mut HostExchange<'_>,
    kind: u16,
    payload: &[u8],
) -> Result<(Vec<u8>, CorrelationId), Box<dyn Error>> {
    let correlation = CorrelationId::from_bytes(fresh_identity()?)?;
    let record = encode_transport_envelope(&Envelope {
        correlation_id: Some(correlation),
        host_epoch: Some(host.host_epoch()),
        kind,
        optional_message: false,
        payload,
        payload_encoding: PayloadEncoding::ControlJson,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Request,
    })?;
    let response = exchange
        .round_trip_bounded(&record, MAXIMUM_CONTROL_JSON_BYTES)
        .map_err(|class| exchange.error(class))?;
    Ok((response, correlation))
}

/// Walks the Host inventory page by page until it ends. Every accepted page advances the cursor
/// ([`valid_page`]), so the walk ends with the inventory or with its budget.
///
/// The inner refusal concerns only this walk, and the Host stream stays in step: the budget passed,
/// or the request had no room, before the last page (`CapacityExceeded`), or the Host refused a
/// page with a correlated typed failure (`HostRejected`). A correlated page that breaks the paging
/// or identity contract is an [`UnexpectedAnswer`], which refuses the request and closes its
/// viewer. Any other error ends the connection: the exchange failed, including a sent page the
/// Host left unanswered past its bound, or the Host broke the envelope.
fn inventory(
    host: &HostBrokerConnection,
    server: &UnixStream,
    target: ReadTerminalLifecycle,
) -> Observed<Option<TerminalLifecycleSnapshot>> {
    let deadline = Instant::now() + INVENTORY_BUDGET;
    // A page the request has no room for ends the walk like a spent budget.
    let Ok(preflight) = HostExchange::open(host, server) else {
        return Ok(Err(ViewerRejection::CapacityExceeded));
    };
    preflight
        .prepare()
        .map_err(|class| preflight.error(class))?;
    let mut cursor = None;
    let mut found = None;
    let mut terminals = BTreeSet::new();
    let mut creations = BTreeSet::new();
    loop {
        if Instant::now() >= deadline {
            return Ok(Err(ViewerRejection::CapacityExceeded));
        }
        let Ok(mut exchange) = HostExchange::open(host, server) else {
            return Ok(Err(ViewerRejection::CapacityExceeded));
        };
        let command = encode_inventory_control(&InventoryControl::ListTerminals(ListTerminals {
            cursor,
            maximum_terminals: 128,
        }))?;
        let (response, correlation) =
            read_control(host, &mut exchange, command.kind, &command.payload)?;
        let envelope = response_envelope(&response, correlation, host.host_epoch())?;
        if matches!(
            decode_connection_control(envelope.kind, envelope.role, envelope.payload),
            Ok(ConnectionControl::TypedFailure(_))
        ) {
            return Ok(Err(ViewerRejection::HostRejected));
        }
        let Ok(InventoryControl::TerminalList(page)) =
            decode_inventory_control(envelope.kind, envelope.role, envelope.payload)
        else {
            return Err(UnexpectedAnswer::of(&response).into());
        };
        if !valid_page(
            &page,
            cursor,
            host.host_epoch(),
            target,
            &mut terminals,
            &mut creations,
            &mut found,
        ) {
            return Err(UnexpectedAnswer::of(&response).into());
        }
        cursor = page.next_cursor;
        if cursor.is_none() {
            return Ok(Ok(found));
        }
    }
}

/// Whether a page continues the walk: its cursor advances it and its identities are new.
fn valid_page(
    page: &TerminalList,
    cursor: Option<orchbee_terminal_protocol::TerminalInventoryCursor>,
    epoch: HostEpoch,
    target: ReadTerminalLifecycle,
    terminals: &mut BTreeSet<TerminalId>,
    creations: &mut BTreeSet<CreationId>,
    found: &mut Option<TerminalLifecycleSnapshot>,
) -> bool {
    let offset = cursor.map_or(0, |c| c.offset);
    if page.terminals.len() > 128
        || cursor.is_some_and(|c| c.inventory_generation != page.inventory_generation)
        || page.next_cursor.is_some_and(|next| {
            next.inventory_generation != page.inventory_generation
                || page.terminals.is_empty()
                || Some(next.offset) != offset.checked_add(page.terminals.len() as u64)
        })
    {
        return false;
    }
    for summary in &page.terminals {
        let snapshot = lookup::from_summary(summary);
        let terminal = snapshot.identity.terminal_id;
        let creation = snapshot.identity.creation_id;
        if snapshot.identity.host_epoch != epoch
            || !terminals.insert(terminal)
            || !creations.insert(creation)
        {
            return false;
        }
        if terminal == target.terminal_id || target.creation_id == Some(creation) {
            if terminal != target.terminal_id
                || target
                    .creation_id
                    .is_some_and(|expected| expected != creation)
                || found.is_some()
            {
                return false;
            }
            *found = Some(snapshot);
        }
    }
    true
}
