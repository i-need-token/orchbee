//! Bounded Host disposal uses typed inventory facts and never retries an uncertain exchange; it
//! asks again only after the Host refused a disposal because an outcome is not yet published.
use crate::host_connection::{HostBrokerConnection, HostExchange, PeerFailure};
use orchbee_capability_node_protocol::{
    HostCreationId, HostEpoch, HostTerminalId, HostTerminalIdentity, OperationId, ShutdownFailure,
    ShutdownStage,
};
use orchbee_local_ipc::LocalStream;
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, ConnectionControl, CorrelationId, Envelope, InventoryControl,
    LifecycleControl, ListTerminals, MessageRole, PayloadEncoding, TerminalIdentity,
    TerminalSummary, TerminalTarget, TypedFailure, decode_connection_control,
    decode_inventory_control, decode_lifecycle_control, decode_transport_envelope,
    encode_inventory_control, encode_lifecycle_control, encode_transport_envelope,
};
use std::time::Instant;

/// The Host refuses to dispose a settled terminal until its next event poll publishes the outcome;
/// the refusal is retryable and leaves the terminal unchanged.
const OUTCOME_PUBLICATION_PENDING: &str = "outcome-publication-pending";

pub(super) struct Failure {
    pub(super) stage: ShutdownStage,
    pub(super) failure: ShutdownFailure,
    pub(super) retire: bool,
}

fn failed(stage: ShutdownStage, failure: ShutdownFailure, retire: bool) -> Failure {
    Failure {
        stage,
        failure,
        retire,
    }
}

struct Exchange<'a> {
    host: &'a HostBrokerConnection,
    server: &'a LocalStream,
    deadline: Instant,
    correlation: CorrelationId,
}

impl Exchange<'_> {
    /// One request and its exact reply; a Host refusal is returned as data for the caller to judge.
    fn send(
        &self,
        stage: ShutdownStage,
        kind: u16,
        payload: &[u8],
    ) -> Result<Result<Vec<u8>, TypedFailure>, Failure> {
        if Instant::now() >= self.deadline {
            return Err(failed(stage, ShutdownFailure::Deadline, false));
        }
        let invalid = || failed(stage, ShutdownFailure::Protocol, true);
        let request = encode_transport_envelope(&Envelope {
            correlation_id: Some(self.correlation),
            host_epoch: Some(self.host.host_epoch()),
            kind,
            optional_message: false,
            payload,
            payload_encoding: PayloadEncoding::ControlJson,
            protocol: CURRENT_PROTOCOL_VERSION,
            role: MessageRole::Request,
        })
        .map_err(|_| invalid())?;
        let mut exchange = HostExchange::before(self.host, self.server, self.deadline);
        let response = exchange
            .prepare()
            .and_then(|()| exchange.round_trip(&request))
            .map_err(|failure| {
                failed(
                    stage,
                    if exchange.deadline_elapsed() {
                        ShutdownFailure::Deadline
                    } else if failure == PeerFailure::Protocol {
                        ShutdownFailure::Protocol
                    } else {
                        ShutdownFailure::Transport
                    },
                    true,
                )
            })?;
        let envelope = decode_transport_envelope(&response).map_err(|_| invalid())?;
        if envelope.role != MessageRole::Response
            || envelope.protocol != CURRENT_PROTOCOL_VERSION
            || envelope.correlation_id != Some(self.correlation)
            || envelope.host_epoch != Some(self.host.host_epoch())
            || envelope.optional_message
            || envelope.payload_encoding != PayloadEncoding::ControlJson
        {
            return Err(invalid());
        }
        if let Ok(ConnectionControl::TypedFailure(refusal)) =
            decode_connection_control(envelope.kind, envelope.role, envelope.payload)
        {
            return Ok(Err(refusal));
        }
        Ok(Ok(response))
    }

    fn first(&self) -> Result<Option<(TerminalIdentity, bool)>, Failure> {
        let invalid = || failed(ShutdownStage::Inventory, ShutdownFailure::Protocol, true);
        let control = encode_inventory_control(&InventoryControl::ListTerminals(ListTerminals {
            cursor: None,
            maximum_terminals: 1,
        }))
        .map_err(|_| invalid())?;
        let Ok(record) = self.send(ShutdownStage::Inventory, control.kind, &control.payload)?
        else {
            return Err(failed(
                ShutdownStage::Inventory,
                ShutdownFailure::HostRefused,
                false,
            ));
        };
        let envelope = decode_transport_envelope(&record).map_err(|_| invalid())?;
        let InventoryControl::TerminalList(mut page) =
            decode_inventory_control(envelope.kind, envelope.role, envelope.payload)
                .map_err(|_| invalid())?
        else {
            return Err(invalid());
        };
        if page.terminals.len() > 1 || (page.terminals.is_empty() && page.next_cursor.is_some()) {
            return Err(invalid());
        }
        let Some(summary) = page.terminals.pop() else {
            return Ok(None);
        };
        let (core, disposable) = match summary {
            TerminalSummary::AwaitingStart { core }
            | TerminalSummary::Exited { core, .. }
            | TerminalSummary::Failed { core, .. } => (core, true),
            TerminalSummary::Running { core, .. }
            | TerminalSummary::Finishing { core, .. }
            | TerminalSummary::ContainingFailure { core, .. } => (core, false),
        };
        if core.identity.host_epoch != self.host.host_epoch() {
            return Err(invalid());
        }
        Ok(Some((core.identity, disposable)))
    }

    fn lifecycle(&self, identity: TerminalIdentity, dispose: bool) -> Result<(), Failure> {
        let stage = if dispose {
            ShutdownStage::Dispose
        } else {
            ShutdownStage::Force
        };
        let invalid = || failed(stage, ShutdownFailure::Protocol, true);
        let target = TerminalTarget {
            terminal_id: identity.terminal_id,
        };
        let control = encode_lifecycle_control(&if dispose {
            LifecycleControl::DisposeTerminal(target)
        } else {
            LifecycleControl::ForceTerminateTerminal(target)
        })
        .map_err(|_| invalid())?;
        let cleanup = if dispose {
            let identity = HostTerminalIdentity::new(
                HostCreationId::from_bytes(*identity.creation_id.as_bytes())
                    .map_err(|_| invalid())?,
                HostEpoch::from_bytes(*identity.host_epoch.as_bytes()).map_err(|_| invalid())?,
                HostTerminalId::from_bytes(*identity.terminal_id.as_bytes())
                    .map_err(|_| invalid())?,
            );
            Some(
                self.host
                    .prepare_target_cleanup(identity)
                    .map_err(|_| failed(stage, ShutdownFailure::HostRefused, false))?,
            )
        } else {
            None
        };
        let record = loop {
            match self.send(stage, control.kind, &control.payload)? {
                Ok(record) => break record,
                // The inventory read settled this terminal; the Host's next event poll publishes
                // its outcome, so the same disposal is asked again within the stop's deadline.
                Err(refusal) if dispose && refusal.code == OUTCOME_PUBLICATION_PENDING => {
                    if !super::pause_before(self.deadline) {
                        return Err(failed(stage, ShutdownFailure::Deadline, false));
                    }
                }
                Err(_) => return Err(failed(stage, ShutdownFailure::HostRefused, false)),
            }
        };
        let envelope = decode_transport_envelope(&record).map_err(|_| invalid())?;
        let control = decode_lifecycle_control(envelope.kind, envelope.role, envelope.payload)
            .map_err(|_| invalid())?;
        match (dispose, control) {
            (true, LifecycleControl::DisposeTerminalAccepted(actual))
            | (false, LifecycleControl::ForceTerminateTerminalAccepted(actual))
                if actual == target => {}
            // This lifecycle exchange requires its exact target-bearing acknowledgment; other control records cannot confirm it.
            _ => return Err(invalid()),
        }
        if let Some(cleanup) = cleanup {
            self.host.commit_target_cleanup(cleanup);
        }
        Ok(())
    }
}

pub(super) fn drain(
    host: &HostBrokerConnection,
    server: &LocalStream,
    operation: OperationId,
    deadline: Instant,
    disposed: &mut u32,
) -> Result<(), Failure> {
    let exchange = Exchange {
        host,
        server,
        deadline,
        correlation: CorrelationId::from_bytes(operation.as_bytes()).expect("nonzero operation"),
    };
    let mut forced = None;
    loop {
        let Some((identity, disposable)) = exchange.first()? else {
            return Ok(());
        };
        if let Some(previous) = forced
            && previous != identity
        {
            return Err(failed(
                ShutdownStage::Inventory,
                ShutdownFailure::Protocol,
                true,
            ));
        }
        if disposable {
            if *disposed == u32::MAX {
                return Err(failed(
                    ShutdownStage::Dispose,
                    ShutdownFailure::HostRefused,
                    false,
                ));
            }
            exchange.lifecycle(identity, true)?;
            *disposed += 1;
            forced = None;
        } else if forced.is_none() {
            exchange.lifecycle(identity, false)?;
            forced = Some(identity);
        } else if !super::pause_before(deadline) {
            return Err(failed(
                ShutdownStage::Inventory,
                ShutdownFailure::Deadline,
                false,
            ));
        }
    }
}
