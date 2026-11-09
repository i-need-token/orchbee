//! Freezes an exposed send target, dispatches its ordered records and precondition to that Worker
//! incarnation, and preserves prefix progress and cleanup in the Client result. It does not
//! re-resolve an admitted target, replay partial and unknown input effects, or retry an
//! unconfirmed prompt; the Worker owns the prompt gate's capability check.

use std::num::NonZeroU64;

use orchbee_capability_node_protocol::{
    SendPrecondition as WorkerPrecondition, SendProgress as WorkerProgress,
    SendPromptState as WorkerPromptState, SendRejection as WorkerRejection,
    SendUnavailable as WorkerUnavailableReason, TerminalSendOutcome as WorkerOutcome,
    TerminalSendRequest as WorkerRequest,
};
use orchbee_client_server_protocol::{
    RequestId, SendCleanupStatus as Cleanup, SendPrecondition, SendProgress, SendPromptState,
    SendRejectedReason as Rejected, SendTarget, SendUnavailableReason as Unavailable,
    ServerMessage, TerminalSendOutcome as Outcome, TerminalSendRequest, TerminalSendResult,
};

use crate::session_coordinator::{BindingViewerPreparationError, PreparedSendTarget};
use crate::terminal_coordinator::BindingViewerTarget;
use crate::worker_obcn_carrier::{
    WorkerOperationRequest, WorkerOperationResult, WorkerRequestError,
};

use super::{LocalTerminalService, placement};

impl LocalTerminalService {
    /// One-shot orchestration input: admit the exact retained binding, dispatch
    /// the pinned record sequence on the serialized Worker carrier, and report
    /// the Worker verdict with the independent cleanup axis and the delivery
    /// frontier preserved.
    pub(crate) fn execute_send(&self, request: &TerminalSendRequest) -> ServerMessage {
        self.deliver_send(request, |_, target| {
            // A Client's send refutes a pending resume of the terminal it reaches.
            #[cfg(windows)]
            self.note_client_input(*target);
            #[cfg(not(windows))]
            let _ = target;
            true
        })
    }

    /// Admits and dispatches one send. `admit` runs last under both admission locks, with the
    /// frozen target, and a false answer refuses the send before any record leaves; the Server's
    /// own deliveries check and consume their intent there.
    pub(super) fn deliver_send(
        &self,
        request: &TerminalSendRequest,
        admit: impl FnOnce(
            &mut crate::session_coordinator::SessionCoordinator,
            &BindingViewerTarget,
        ) -> bool,
    ) -> ServerMessage {
        let request_id = request.request_id().clone();
        let prepared = {
            let Ok(workers) = self.workers.lock() else {
                return send_result(
                    &request_id,
                    None,
                    unavailable(Unavailable::ServerBusy, Cleanup::NotNeeded),
                );
            };
            let handle = workers.request_handle();
            let Ok(mut coordinator) = self.coordinator.lock() else {
                return send_result(
                    &request_id,
                    None,
                    unavailable(Unavailable::ServerBusy, Cleanup::NotNeeded),
                );
            };
            // Resolve samples the carrier only after both admission locks are held.
            let prepared = coordinator.prepare_send_target(
                request.session_id(),
                request.worker_id(),
                request.terminal_id(),
                placement(handle.as_ref()),
            );
            let prepared = match prepared {
                Ok(prepared) => prepared,
                Err(error) => {
                    return send_result(&request_id, None, admission_outcome(error));
                }
            };
            let Some(handle) = handle else {
                return send_result(
                    &request_id,
                    None,
                    unavailable(Unavailable::TerminalRuntimeUnavailable, Cleanup::NotNeeded),
                );
            };
            let Some((worker_request, target)) = send_request(request, &prepared) else {
                return send_result(
                    &request_id,
                    None,
                    unavailable(Unavailable::ServerBusy, Cleanup::NotNeeded),
                );
            };
            if !admit(&mut coordinator, &prepared.target) {
                return send_result(
                    &request_id,
                    None,
                    rejected(Rejected::TerminalUnavailable, Cleanup::NotNeeded),
                );
            }
            (prepared, handle, worker_request, target)
        };
        let (prepared, handle, worker_request, target) = prepared;
        let outcome = match handle.request(WorkerOperationRequest::Send(worker_request)) {
            Ok(WorkerOperationResult::Send(result)) => worker_outcome(result.outcome()),
            Ok(_) => unreachable!("send request returned another operation result"),
            Err(error) => carrier_outcome(error),
        };
        if acknowledges_records(
            request.records().as_slice(),
            outcome.progress().records_accepted() as usize,
        ) && let Ok(mut coordinator) = self.coordinator.lock()
        {
            coordinator.acknowledge_terminal_input(prepared.target);
        }
        send_result(&request_id, Some(target), outcome)
    }
}

/// Translate the admitted client records verbatim into Worker records: every
/// semantic distinction — text versus literal bytes, semantic key versus
/// modifier combo, literal write versus Host-bracketed paste — survives.
fn send_request(
    request: &TerminalSendRequest,
    prepared: &PreparedSendTarget,
) -> Option<(WorkerRequest, SendTarget)> {
    let mut records = Vec::with_capacity(request.records().len());
    for record in request.records().iter() {
        records.push(worker_record(record)?);
    }
    let target = send_target(&prepared.target)?;
    Some((
        WorkerRequest {
            request_id: prepared.identities.operation,
            worker_id: prepared.target.worker.worker_id,
            worker_generation: prepared.target.worker.generation,
            target: prepared.target.identity,
            attachment_id: prepared.identities.attachment,
            precondition: match request.precondition() {
                SendPrecondition::None => WorkerPrecondition::None,
                SendPrecondition::IdlePrompt => WorkerPrecondition::IdlePrompt,
            },
            records: orchbee_capability_node_protocol::SendRecords::new(records).ok()?,
        },
        target,
    ))
}

fn worker_record(
    record: &orchbee_client_server_protocol::SendRecord,
) -> Option<orchbee_capability_node_protocol::SendRecord> {
    use orchbee_capability_node_protocol as worker;
    use orchbee_client_server_protocol::SendRecord as Client;
    Some(match record {
        Client::Stroke(stroke) => {
            use orchbee_client_server_protocol::SendStrokeKey as Key;
            let key = match stroke.key() {
                Key::Named(key) => worker::SendStrokeKey::Named(send_key(key)),
                Key::Insert => worker::SendStrokeKey::Insert,
                Key::Function(number) => worker::SendStrokeKey::Function(number),
                Key::Character(value) => worker::SendStrokeKey::Character(value),
            };
            worker::SendRecord::Stroke(worker::SendKeyStroke::new(key, stroke.modifiers()).ok()?)
        }
        Client::Text(input) => worker::SendRecord::Text(
            worker::SendTextInput::new(
                worker::SendText::new(input.text().as_str().to_owned()).ok()?,
                input.enter(),
            )
            .ok()?,
        ),
        Client::Bytes { bytes } => worker::SendRecord::Bytes {
            bytes: worker::SendBytes::new(bytes.as_bytes().to_vec()).ok()?,
        },
        Client::Paste { content } => worker::SendRecord::Paste {
            content: worker::SendPasteContent::new(content.as_bytes().to_vec()).ok()?,
        },
    })
}

fn send_target(target: &BindingViewerTarget) -> Option<SendTarget> {
    Some(SendTarget::new(
        NonZeroU64::new(target.worker.generation.get())?,
        orchbee_client_server_protocol::HostEpoch::parse(&format!(
            "{:032x}",
            u128::from_be_bytes(target.identity.host_epoch().as_bytes())
        ))
        .ok()?,
        orchbee_client_server_protocol::HostTerminalId::parse(&format!(
            "{:032x}",
            u128::from_be_bytes(target.identity.terminal_id().as_bytes())
        ))
        .ok()?,
    ))
}

fn send_key(
    key: orchbee_client_server_protocol::SendKey,
) -> orchbee_capability_node_protocol::SendKey {
    use orchbee_capability_node_protocol::SendKey as Worker;
    use orchbee_client_server_protocol::SendKey as Client;
    match key {
        Client::Enter => Worker::Enter,
        Client::Escape => Worker::Escape,
        Client::Tab => Worker::Tab,
        Client::Backspace => Worker::Backspace,
        Client::Delete => Worker::Delete,
        Client::Up => Worker::Up,
        Client::Down => Worker::Down,
        Client::Left => Worker::Left,
        Client::Right => Worker::Right,
        Client::Home => Worker::Home,
        Client::End => Worker::End,
        Client::PageUp => Worker::PageUp,
        Client::PageDown => Worker::PageDown,
    }
}

fn cleanup(status: orchbee_capability_node_protocol::SendCleanupStatus) -> Cleanup {
    use orchbee_capability_node_protocol::SendCleanupStatus as Worker;
    match status {
        Worker::NotNeeded => Cleanup::NotNeeded,
        Worker::Confirmed => Cleanup::Confirmed,
        Worker::Unconfirmed => Cleanup::Unconfirmed,
    }
}

/// The delivery frontier is a worker-side fact the Server reports verbatim;
/// the wire bounds are identical on both protocols so the translation never
/// invents a cursor.
fn progress(cursor: WorkerProgress) -> SendProgress {
    SendProgress::new(
        cursor.records_accepted(),
        cursor.first_stopped_record(),
        cursor.accepted_encoded_bytes(),
    )
    .expect("worker progress satisfies the shared bounds")
}

fn admission_outcome(error: BindingViewerPreparationError) -> Outcome {
    match error {
        BindingViewerPreparationError::ServerBusy => {
            unavailable(Unavailable::ServerBusy, Cleanup::NotNeeded)
        }
        BindingViewerPreparationError::TerminalRuntimeUnavailable => {
            unavailable(Unavailable::TerminalRuntimeUnavailable, Cleanup::NotNeeded)
        }
        BindingViewerPreparationError::TerminalUnavailable
        | BindingViewerPreparationError::WorkbenchUnavailable => {
            rejected(Rejected::TerminalUnavailable, Cleanup::NotNeeded)
        }
    }
}

fn carrier_outcome(error: WorkerRequestError) -> Outcome {
    match error {
        WorkerRequestError::Backpressure => {
            unavailable(Unavailable::ServerBusy, Cleanup::NotNeeded)
        }
        WorkerRequestError::WrongWorker | WorkerRequestError::WrongGeneration => {
            unavailable(Unavailable::TerminalRuntimeUnavailable, Cleanup::NotNeeded)
        }
        WorkerRequestError::Unavailable
        | WorkerRequestError::InvalidRequest(_)
        | WorkerRequestError::NotDelivered(_) => {
            unavailable(Unavailable::WorkerUnavailable, Cleanup::NotNeeded)
        }
        WorkerRequestError::PostDeliveryOutcomeUnknown(_) => Outcome::OutcomeUnknown {
            progress: SendProgress::none(),
            cleanup: Cleanup::Unconfirmed,
        },
    }
}

fn worker_outcome(outcome: WorkerOutcome) -> Outcome {
    match outcome {
        WorkerOutcome::Unavailable {
            reason,
            progress: cursor,
            cleanup: status,
        } => Outcome::Unavailable {
            reason: match reason {
                WorkerUnavailableReason::HostUnavailable => Unavailable::HostUnavailable,
                WorkerUnavailableReason::HostProtocolFault => Unavailable::HostProtocolFault,
                WorkerUnavailableReason::InputBudgetExhausted => Unavailable::InputBudgetExhausted,
            },
            progress: progress(cursor),
            cleanup: cleanup(status),
        },
        WorkerOutcome::Rejected {
            reason,
            progress: cursor,
            cleanup: status,
        } => match reason {
            WorkerRejection::WrongWorker => Outcome::Unavailable {
                reason: Unavailable::WorkerUnavailable,
                progress: progress(cursor),
                cleanup: cleanup(status),
            },
            reason => Outcome::Rejected {
                reason: match reason {
                    WorkerRejection::StaleWorkerGeneration => Rejected::StaleWorkerGeneration,
                    WorkerRejection::StaleHostEpoch => Rejected::StaleHostEpoch,
                    WorkerRejection::TerminalUnavailable => Rejected::TerminalUnavailable,
                    WorkerRejection::HostRejected => Rejected::HostRejected,
                    WorkerRejection::HostInputInvalid => Rejected::HostInputInvalid,
                    WorkerRejection::HostInputLimitExceeded => Rejected::HostInputLimitExceeded,
                    WorkerRejection::HostInputBackpressure => Rejected::HostInputBackpressure,
                    WorkerRejection::HostInputUnavailable => Rejected::HostInputUnavailable,
                    WorkerRejection::PromptUnconfirmed(state) => {
                        Rejected::PromptUnconfirmed(prompt_state(state))
                    }
                    WorkerRejection::HostPromptGateUnsupported => {
                        Rejected::HostPromptGateUnsupported
                    }
                    WorkerRejection::WrongWorker => unreachable!("handled above"),
                },
                progress: progress(cursor),
                cleanup: cleanup(status),
            },
        },
        WorkerOutcome::Accepted {
            progress: cursor,
            cleanup: status,
        } => Outcome::Accepted {
            progress: progress(cursor),
            cleanup: cleanup(status),
        },
        WorkerOutcome::OutcomeUnknown {
            progress: cursor,
            cleanup: status,
        } => Outcome::OutcomeUnknown {
            progress: progress(cursor),
            cleanup: cleanup(status),
        },
    }
}

const fn prompt_state(state: WorkerPromptState) -> SendPromptState {
    match state {
        WorkerPromptState::NotShell => SendPromptState::NotShell,
        WorkerPromptState::Busy => SendPromptState::Busy,
        WorkerPromptState::FullScreen => SendPromptState::FullScreen,
        WorkerPromptState::Unmarked => SendPromptState::Unmarked,
        WorkerPromptState::NotAtPrompt => SendPromptState::NotAtPrompt,
        WorkerPromptState::InputPending => SendPromptState::InputPending,
        WorkerPromptState::Changed => SendPromptState::Changed,
        WorkerPromptState::Settling => SendPromptState::Settling,
    }
}

fn rejected(reason: Rejected, status: Cleanup) -> Outcome {
    Outcome::Rejected {
        reason,
        progress: SendProgress::none(),
        cleanup: status,
    }
}
fn unavailable(reason: Unavailable, status: Cleanup) -> Outcome {
    Outcome::Unavailable {
        reason,
        progress: SendProgress::none(),
        cleanup: status,
    }
}

fn acknowledges_records(
    records: &[orchbee_client_server_protocol::SendRecord],
    accepted: usize,
) -> bool {
    records.iter().take(accepted).any(|record| match record {
        orchbee_client_server_protocol::SendRecord::Bytes { bytes } => {
            super::controller_connection::contains_typed_input(bytes.as_bytes())
        }
        orchbee_client_server_protocol::SendRecord::Text(_)
        | orchbee_client_server_protocol::SendRecord::Stroke(_)
        | orchbee_client_server_protocol::SendRecord::Paste { .. } => true,
    })
}

#[cfg(test)]
mod tests;

fn send_result(
    request_id: &RequestId,
    target: Option<SendTarget>,
    outcome: Outcome,
) -> ServerMessage {
    let result =
        TerminalSendResult::new(request_id.clone(), target.clone(), outcome).or_else(|_| {
            TerminalSendResult::new(
                request_id.clone(),
                target,
                Outcome::OutcomeUnknown {
                    progress: SendProgress::none(),
                    cleanup: Cleanup::Unconfirmed,
                },
            )
        });
    match result {
        Ok(result) => ServerMessage::TerminalSendResult(result),
        // Every constructed outcome pair is valid, so one of the two builds succeeds.
        Err(_) => unreachable!("validated send outcomes are encodable"),
    }
}
