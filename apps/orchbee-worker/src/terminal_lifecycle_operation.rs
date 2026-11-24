//! Prepares exact-binding Host lifecycle exchanges and maps inventory, inspection,
//! termination, and disposal responses into authorization-kernel outcomes. It performs no
//! independent target selection and never turns an uncertain mutation into a retry.

use orchbee_local_ipc::LocalStream as UnixStream;
use std::{error::Error, io, time::Duration};

use orchbee_capability_node_protocol::{
    HostEpoch as CapabilityHostEpoch, LIFECYCLE_INVENTORY_PAGE_TERMINALS, LifecycleHostFailure,
    LifecycleInventoryCursor, LifecycleInventoryFailure, LifecycleInventoryPage,
    LifecycleUnknownReason, TerminalLifecycleAction, TerminalLifecycleOutcome,
    TerminalLifecycleRequest,
};
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, ConnectionControl, CorrelationId, CreationId, Envelope,
    InventoryControl, LifecycleControl, ListTerminals, MAXIMUM_CONTROL_JSON_BYTES, MessageRole,
    PayloadEncoding, ReadTerminalLifecycle, TerminalInventoryCursor, TerminalTarget,
    decode_connection_control, decode_inventory_control, decode_lifecycle_control,
    decode_transport_envelope, encode_inventory_control, encode_lifecycle_control,
    encode_transport_envelope,
};

use crate::{
    host_connection::{
        HostBrokerConnection, HostExchange, PeerFailure, PreparedTargetCleanup, record_kind,
    },
    terminal_authorization::{
        AuthorizedHostOperationKernel, HostHandoffAdmission, LifecycleDispatch,
    },
    terminal_host_operation::HostOperationDisposition,
};

pub(crate) mod lookup;

pub(crate) struct PreparedLifecycleOperation {
    dispatch: LifecycleDispatch,
    record: Vec<u8>,
    cleanup: Option<PreparedTargetCleanup>,
    lookup: Option<ReadTerminalLifecycle>,
}

impl PreparedLifecycleOperation {
    pub(crate) fn new(
        dispatch: LifecycleDispatch,
        host: &HostBrokerConnection,
    ) -> Result<Self, Box<dyn Error>> {
        let request = dispatch.request;
        let target = TerminalTarget {
            terminal_id: orchbee_terminal_protocol::TerminalId::from_bytes(
                request.target.terminal_id().as_bytes(),
            )?,
        };
        let mut lookup = None;
        let (kind, payload) = match request.action {
            TerminalLifecycleAction::InventoryPage { cursor: None }
            | TerminalLifecycleAction::InspectInventoryPage { cursor: None }
                if host.terminal_lifecycle_read() =>
            {
                let read = ReadTerminalLifecycle {
                    terminal_id: target.terminal_id,
                    creation_id: Some(CreationId::from_bytes(
                        request.target.creation_id().as_bytes(),
                    )?),
                };
                lookup = Some(read);
                let control =
                    encode_lifecycle_control(&LifecycleControl::ReadTerminalLifecycle(read))?;
                (control.kind, control.payload)
            }
            TerminalLifecycleAction::InventoryPage { cursor }
            | TerminalLifecycleAction::InspectInventoryPage { cursor } => {
                let control =
                    encode_inventory_control(&InventoryControl::ListTerminals(ListTerminals {
                        cursor: cursor.map(|cursor| TerminalInventoryCursor {
                            inventory_generation: cursor.inventory_generation,
                            offset: cursor.offset,
                        }),
                        maximum_terminals: LIFECYCLE_INVENTORY_PAGE_TERMINALS,
                    }))?;
                (control.kind, control.payload)
            }
            TerminalLifecycleAction::Retire => {
                return Err(io::Error::other("Retire is answered without the Host").into());
            }
            action => {
                let control = encode_lifecycle_control(&match action {
                    TerminalLifecycleAction::Terminate => {
                        LifecycleControl::TerminateTerminal(target)
                    }
                    TerminalLifecycleAction::Force => {
                        LifecycleControl::ForceTerminateTerminal(target)
                    }
                    TerminalLifecycleAction::Dispose => LifecycleControl::DisposeTerminal(target),
                    TerminalLifecycleAction::InventoryPage { .. }
                    | TerminalLifecycleAction::InspectInventoryPage { .. }
                    | TerminalLifecycleAction::Retire => {
                        unreachable!("matched above")
                    }
                })?;
                (control.kind, control.payload)
            }
        };
        let record = encode_transport_envelope(&Envelope {
            correlation_id: Some(CorrelationId::from_bytes(request.request_id.as_bytes())?),
            host_epoch: Some(host.host_epoch()),
            kind,
            optional_message: false,
            payload: &payload,
            payload_encoding: PayloadEncoding::ControlJson,
            protocol: CURRENT_PROTOCOL_VERSION,
            role: MessageRole::Request,
        })?;
        let cleanup = if request.action == TerminalLifecycleAction::Dispose {
            Some(host.prepare_target_cleanup(request.target)?)
        } else {
            None
        };
        Ok(Self {
            dispatch,
            record,
            cleanup,
            lookup,
        })
    }

    pub(crate) fn execute(
        self,
        kernel: &mut AuthorizedHostOperationKernel,
        host: &HostBrokerConnection,
        server: &UnixStream,
        now: impl FnOnce() -> u64,
        budget: Duration,
    ) -> Result<HostOperationDisposition, Box<dyn Error>> {
        if budget.is_zero() {
            return Err(io::Error::other("invalid lifecycle Host budget").into());
        }
        // With no room left in the request nothing reaches the Host, and the operation fails
        // before handoff for the Client to ask again.
        let Ok(mut exchange) = HostExchange::new(host, server, budget) else {
            kernel.lifecycle_pre_handoff_failure(self.dispatch, now())?;
            return Ok(HostOperationDisposition::ResultReady);
        };
        if let Err(failure) = exchange.prepare() {
            exchange.close(failure);
            if failure == PeerFailure::Server {
                return Ok(HostOperationDisposition::ServerClosed);
            }
            kernel.lifecycle_pre_handoff_failure(self.dispatch, now())?;
            return Ok(HostOperationDisposition::HostClosed);
        }
        let epoch = CapabilityHostEpoch::from_bytes(*host.host_epoch().as_bytes())?;
        if kernel.begin_lifecycle_handoff(self.dispatch, now(), epoch)?
            == HostHandoffAdmission::ResultReady
        {
            return Ok(HostOperationDisposition::ResultReady);
        }
        let response = if self.lookup.is_some() {
            exchange.round_trip_bounded(&self.record, MAXIMUM_CONTROL_JSON_BYTES)
        } else {
            exchange.round_trip(&self.record)
        };
        let (failure, reason) = match response {
            Ok(record) => match response_outcome(self.dispatch.request, &record, self.lookup) {
                Ok(outcome) => {
                    // Retain exact known ACK before removing the prepared IDs. This
                    // synchronous boundary cannot admit/replay or deliver a result
                    // until the nonallocating target cleanup is complete.
                    kernel.record_lifecycle_completion(self.dispatch, outcome)?;
                    if outcome == TerminalLifecycleOutcome::Disposed
                        && let Some(cleanup) = self.cleanup
                    {
                        host.commit_target_cleanup(cleanup);
                    }
                    return Ok(HostOperationDisposition::ResultReady);
                }
                // An answer that arrived whole but does not settle the step leaves the stream in
                // step: only this step's effect is unknown, and the connection serves on.
                Err(_) => {
                    crate::diagnostic_events::host_unexpected(
                        host.pacing.site(),
                        record_kind(&record),
                        crate::diagnostic_events::Anomaly::Answer,
                    );
                    kernel.record_lifecycle_completion(
                        self.dispatch,
                        TerminalLifecycleOutcome::OutcomeUnknown(
                            LifecycleUnknownReason::InvalidResponse,
                        ),
                    )?;
                    return Ok(HostOperationDisposition::ResultReady);
                }
            },
            Err(failure) => (
                failure,
                if exchange.deadline_elapsed() {
                    LifecycleUnknownReason::Deadline
                } else {
                    LifecycleUnknownReason::TransportLost
                },
            ),
        };
        exchange.close(failure);
        kernel.record_lifecycle_completion(
            self.dispatch,
            TerminalLifecycleOutcome::OutcomeUnknown(reason),
        )?;
        Ok(match failure {
            PeerFailure::Host | PeerFailure::Protocol => HostOperationDisposition::HostClosed,
            PeerFailure::Server => HostOperationDisposition::ServerClosed,
        })
    }
}

fn response_outcome(
    request: TerminalLifecycleRequest,
    record: &[u8],
    lookup: Option<ReadTerminalLifecycle>,
) -> Result<TerminalLifecycleOutcome, Box<dyn Error>> {
    let envelope = decode_transport_envelope(record)?;
    if envelope.role != MessageRole::Response
        || envelope.correlation_id.map(|id| *id.as_bytes()) != Some(request.request_id.as_bytes())
        || envelope.host_epoch.map(|epoch| *epoch.as_bytes())
            != Some(request.target.host_epoch().as_bytes())
        || envelope.protocol != CURRENT_PROTOCOL_VERSION
        || envelope.optional_message
        || envelope.payload_encoding != PayloadEncoding::ControlJson
    {
        return Err(io::Error::other("unexpected lifecycle response envelope").into());
    }
    if let Some(target) = lookup {
        return Ok(match lookup::decode(target, &envelope) {
            Ok(snapshot) => {
                TerminalLifecycleOutcome::TargetObserved(snapshot.map(lookup::observation))
            }
            Err(lookup::LookupFailure::Refused) => {
                TerminalLifecycleOutcome::InventoryFailure(LifecycleInventoryFailure::PageRejected)
            }
            Err(lookup::LookupFailure::IdentityConflict) => {
                TerminalLifecycleOutcome::InventoryFailure(
                    LifecycleInventoryFailure::IdentityConflict,
                )
            }
            Err(lookup::LookupFailure::InvalidResponse) => {
                return Err(io::Error::other("invalid targeted lifecycle response").into());
            }
        });
    }
    if let Ok(ConnectionControl::TypedFailure(failure)) =
        decode_connection_control(envelope.kind, envelope.role, envelope.payload)
    {
        return Ok(match request.action {
            TerminalLifecycleAction::InventoryPage { .. }
            | TerminalLifecycleAction::InspectInventoryPage { .. } => {
                TerminalLifecycleOutcome::InventoryFailure(
                    if failure.code == "stale-inventory-cursor" {
                        LifecycleInventoryFailure::StaleCursor
                    } else {
                        LifecycleInventoryFailure::PageRejected
                    },
                )
            }
            TerminalLifecycleAction::Dispose if failure.code == "cleanup-incomplete" => {
                TerminalLifecycleOutcome::HostFailure(LifecycleHostFailure::CleanupIncomplete)
            }
            TerminalLifecycleAction::Terminate
            | TerminalLifecycleAction::Force
            | TerminalLifecycleAction::Dispose
            | TerminalLifecycleAction::Retire => {
                TerminalLifecycleOutcome::HostFailure(LifecycleHostFailure::HostRefused)
            }
        });
    }
    if matches!(
        request.action,
        TerminalLifecycleAction::InventoryPage { .. }
            | TerminalLifecycleAction::InspectInventoryPage { .. }
    ) {
        return Ok(inventory_outcome(
            request,
            envelope.kind,
            envelope.role,
            envelope.payload,
        ));
    }
    let accepted = decode_lifecycle_control(envelope.kind, envelope.role, envelope.payload)?;
    let target = TerminalTarget {
        terminal_id: orchbee_terminal_protocol::TerminalId::from_bytes(
            request.target.terminal_id().as_bytes(),
        )?,
    };
    let outcome = match request.action {
        TerminalLifecycleAction::Terminate if matches!(&accepted, LifecycleControl::TerminateTerminalAccepted(observed) if observed == &target) => {
            TerminalLifecycleOutcome::TerminateAccepted
        }
        TerminalLifecycleAction::Force if matches!(&accepted, LifecycleControl::ForceTerminateTerminalAccepted(observed) if observed == &target) => {
            TerminalLifecycleOutcome::ForceAccepted
        }
        TerminalLifecycleAction::Dispose if matches!(&accepted, LifecycleControl::DisposeTerminalAccepted(observed) if observed == &target) => {
            TerminalLifecycleOutcome::Disposed
        }
        TerminalLifecycleAction::Terminate
        | TerminalLifecycleAction::Force
        | TerminalLifecycleAction::Dispose
        | TerminalLifecycleAction::InventoryPage { .. }
        | TerminalLifecycleAction::InspectInventoryPage { .. }
        | TerminalLifecycleAction::Retire => {
            return Err(io::Error::other("unexpected lifecycle ACK target or action").into());
        }
    };
    Ok(outcome)
}

fn inventory_outcome(
    request: TerminalLifecycleRequest,
    kind: u16,
    role: MessageRole,
    payload: &[u8],
) -> TerminalLifecycleOutcome {
    let invalid =
        TerminalLifecycleOutcome::InventoryFailure(LifecycleInventoryFailure::InvalidPage);
    let Ok(InventoryControl::TerminalList(page)) = decode_inventory_control(kind, role, payload)
    else {
        return invalid;
    };
    let mut target = None;
    for (index, summary) in page.terminals.iter().enumerate() {
        let snapshot = lookup::from_summary(summary);
        if *snapshot.identity.host_epoch.as_bytes() != request.target.host_epoch().as_bytes() {
            return invalid;
        }
        let terminal_matches =
            *snapshot.identity.terminal_id.as_bytes() == request.target.terminal_id().as_bytes();
        let creation_matches =
            *snapshot.identity.creation_id.as_bytes() == request.target.creation_id().as_bytes();
        let duplicate = page.terminals.iter().take(index).any(|previous| {
            let identity = lookup::from_summary(previous).identity;
            identity.terminal_id == snapshot.identity.terminal_id
                || identity.creation_id == snapshot.identity.creation_id
        });
        if duplicate && !terminal_matches && !creation_matches {
            return invalid;
        }
        if terminal_matches || creation_matches {
            if !terminal_matches || !creation_matches || target.is_some() || duplicate {
                return TerminalLifecycleOutcome::InventoryFailure(
                    LifecycleInventoryFailure::IdentityConflict,
                );
            }
            target = Some(lookup::observation(snapshot));
        }
    }
    let Ok(returned_count) = u16::try_from(page.terminals.len()) else {
        return invalid;
    };
    let outcome = TerminalLifecycleOutcome::InventoryPage(LifecycleInventoryPage {
        inventory_generation: page.inventory_generation,
        next_cursor: page.next_cursor.map(|cursor| LifecycleInventoryCursor {
            inventory_generation: cursor.inventory_generation,
            offset: cursor.offset,
        }),
        returned_count,
        target,
    });
    if orchbee_capability_node_protocol::TerminalLifecycleResult::new(request, outcome).is_err() {
        invalid
    } else {
        outcome
    }
}

#[cfg(test)]
mod tests;
