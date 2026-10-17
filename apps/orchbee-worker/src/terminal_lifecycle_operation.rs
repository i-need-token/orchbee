use orchbee_local_ipc::LocalStream as UnixStream;
use std::{error::Error, io, time::Duration};

use orchbee_capability_node_protocol::{
    HostEpoch as CapabilityHostEpoch, LIFECYCLE_INVENTORY_PAGE_TERMINALS, LifecycleCloseActivity,
    LifecycleHostFailure, LifecycleInventoryCursor, LifecycleInventoryFailure,
    LifecycleInventoryPage, LifecycleTargetObservation, LifecycleTerminalPhase,
    LifecycleUnknownReason, TerminalLifecycleAction, TerminalLifecycleOutcome,
    TerminalLifecycleRequest,
};
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, ConnectionControl, CorrelationId, Envelope, InventoryControl,
    LifecycleControl, ListTerminals, MessageRole, PayloadEncoding, TerminalInventoryCursor,
    TerminalSummary, TerminalTarget, decode_connection_control, decode_inventory_control,
    decode_lifecycle_control, decode_transport_envelope, encode_inventory_control,
    encode_lifecycle_control, encode_transport_envelope,
};

use crate::{
    host_connection::{HostBrokerConnection, HostExchange, PeerFailure, PreparedTargetCleanup},
    terminal_authorization::{
        AuthorizedHostOperationKernel, HostHandoffAdmission, LifecycleDispatch,
    },
    terminal_host_operation::HostOperationDisposition,
};

pub(crate) struct PreparedLifecycleOperation {
    dispatch: LifecycleDispatch,
    record: Vec<u8>,
    cleanup: Option<PreparedTargetCleanup>,
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
        let (kind, payload) = match request.action {
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
            action => {
                let control = encode_lifecycle_control(&match action {
                    TerminalLifecycleAction::Terminate => {
                        LifecycleControl::TerminateTerminal(target)
                    }
                    TerminalLifecycleAction::TerminateIfIdle => {
                        LifecycleControl::TerminateTerminalIfIdle(target)
                    }
                    TerminalLifecycleAction::Force => {
                        LifecycleControl::ForceTerminateTerminal(target)
                    }
                    TerminalLifecycleAction::Dispose => LifecycleControl::DisposeTerminal(target),
                    TerminalLifecycleAction::InventoryPage { .. }
                    | TerminalLifecycleAction::InspectInventoryPage { .. } => {
                        unreachable!("query matched above")
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
        let mut exchange = HostExchange::new(host, server, budget)
            .ok_or_else(|| io::Error::other("invalid lifecycle Host budget"))?;
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
        let (failure, reason) = match exchange.round_trip(&self.record) {
            Ok(record) => match response_outcome(self.dispatch.request, &record) {
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
                Err(_) => (PeerFailure::Host, LifecycleUnknownReason::InvalidResponse),
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
            TerminalLifecycleAction::TerminateIfIdle if failure.code == "confirmation-required" => {
                TerminalLifecycleOutcome::HostFailure(LifecycleHostFailure::ConfirmationRequired)
            }
            _ => TerminalLifecycleOutcome::HostFailure(LifecycleHostFailure::HostRefused),
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
    let outcome = match (request.action, accepted) {
        (
            TerminalLifecycleAction::TerminateIfIdle,
            LifecycleControl::TerminateTerminalIfIdleAccepted(observed),
        ) if observed == target => TerminalLifecycleOutcome::TerminateAccepted,
        (
            TerminalLifecycleAction::Terminate,
            LifecycleControl::TerminateTerminalAccepted(observed),
        ) if observed == target => TerminalLifecycleOutcome::TerminateAccepted,
        (
            TerminalLifecycleAction::Force,
            LifecycleControl::ForceTerminateTerminalAccepted(observed),
        ) if observed == target => TerminalLifecycleOutcome::ForceAccepted,
        (TerminalLifecycleAction::Dispose, LifecycleControl::DisposeTerminalAccepted(observed))
            if observed == target =>
        {
            TerminalLifecycleOutcome::Disposed
        }
        _ => return Err(io::Error::other("unexpected lifecycle ACK target or action").into()),
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
        let (core, phase) = summary_observation(summary);
        if *core.identity.host_epoch.as_bytes() != request.target.host_epoch().as_bytes() {
            return invalid;
        }
        let terminal_matches =
            *core.identity.terminal_id.as_bytes() == request.target.terminal_id().as_bytes();
        let creation_matches =
            *core.identity.creation_id.as_bytes() == request.target.creation_id().as_bytes();
        let duplicate = page.terminals.iter().take(index).any(|previous| {
            let identity = summary_observation(previous).0.identity;
            identity.terminal_id == core.identity.terminal_id
                || identity.creation_id == core.identity.creation_id
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
            target = Some(LifecycleTargetObservation {
                revision: core.revision,
                phase,
                close_activity: if request.authorization.capability_version.get() < 2 {
                    LifecycleCloseActivity::Unknown
                } else {
                    match core.close_activity {
                        Some(orchbee_terminal_protocol::TerminalCloseActivity::Idle) => {
                            LifecycleCloseActivity::Idle
                        }
                        Some(orchbee_terminal_protocol::TerminalCloseActivity::Active) => {
                            LifecycleCloseActivity::Active
                        }
                        Some(orchbee_terminal_protocol::TerminalCloseActivity::Unknown) | None => {
                            LifecycleCloseActivity::Unknown
                        }
                    }
                },
            });
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

pub(crate) fn summary_observation(
    summary: &TerminalSummary,
) -> (
    &orchbee_terminal_protocol::TerminalSummaryCore,
    LifecycleTerminalPhase,
) {
    match summary {
        TerminalSummary::AwaitingStart { core } => (core, LifecycleTerminalPhase::AwaitingStart),
        TerminalSummary::Running {
            core,
            terminate_requested,
            ..
        } => (
            core,
            LifecycleTerminalPhase::Running {
                terminate_requested: *terminate_requested,
            },
        ),
        TerminalSummary::Finishing {
            core,
            terminate_requested,
            ..
        } => (
            core,
            LifecycleTerminalPhase::Finishing {
                terminate_requested: *terminate_requested,
            },
        ),
        TerminalSummary::ContainingFailure { core, .. } => {
            (core, LifecycleTerminalPhase::ContainingFailure)
        }
        TerminalSummary::Exited {
            core,
            terminate_requested,
            ..
        } => (
            core,
            LifecycleTerminalPhase::Exited {
                terminate_requested: *terminate_requested,
            },
        ),
        TerminalSummary::Failed { core, .. } => (core, LifecycleTerminalPhase::Failed),
    }
}

#[cfg(test)]
mod tests;
