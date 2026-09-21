use std::num::NonZeroU64;

use orchbee_capability_node_protocol::{
    SendProgress as WorkerProgress, SendRejection as WorkerRejection,
    SendUnavailable as WorkerUnavailableReason, TerminalSendOutcome as WorkerOutcome,
    TerminalSendRequest as WorkerRequest,
};
use orchbee_client_server_protocol::{
    RequestId, SendCleanupStatus as Cleanup, SendProgress, SendRejectedReason as Rejected,
    SendTarget, SendUnavailableReason as Unavailable, ServerMessage,
    TerminalSendOutcome as Outcome, TerminalSendRequest, TerminalSendResult,
};

use crate::site_coordinator::{BindingViewerPreparationError, PreparedSendTarget};
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
                request.site_id(),
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
            match send_request(request, &prepared) {
                Some((worker_request, target)) => (prepared, handle, worker_request, target),
                None => {
                    return send_result(
                        &request_id,
                        None,
                        unavailable(Unavailable::ServerBusy, Cleanup::NotNeeded),
                    );
                }
            }
        };
        let (_prepared, handle, worker_request, target) = prepared;
        let outcome = match handle.request(WorkerOperationRequest::Send(worker_request)) {
            Ok(WorkerOperationResult::Send(result)) => worker_outcome(result.outcome()),
            Ok(_) => unreachable!("send request returned another operation result"),
            Err(error) => carrier_outcome(error),
        };
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
        Client::Text(input) => worker::SendRecord::Text(
            worker::SendTextInput::new(
                worker::SendText::new(input.text().as_str().to_owned()).ok()?,
                input.enter(),
            )
            .ok()?,
        ),
        Client::Key { key } => worker::SendRecord::Key {
            key: send_key(*key),
        },
        Client::Combo { modifiers, base } => worker::SendRecord::Combo {
            modifiers: worker::SendModifiers::new(
                modifiers.control,
                modifiers.alt,
                modifiers.shift,
            )
            .ok()?,
            base: worker_combo_base(*base)?,
        },
        Client::Bytes { bytes } => worker::SendRecord::Bytes {
            bytes: worker::SendBytes::new(bytes.as_bytes().to_vec()).ok()?,
        },
        Client::Paste { content } => worker::SendRecord::Paste {
            content: worker::SendPasteContent::new(content.as_bytes().to_vec()).ok()?,
        },
    })
}

fn worker_combo_base(
    base: orchbee_client_server_protocol::SendComboBase,
) -> Option<orchbee_capability_node_protocol::SendComboBase> {
    use orchbee_capability_node_protocol::SendComboBase as Worker;
    use orchbee_client_server_protocol::SendComboBase as Client;
    Some(match base {
        Client::Named(key) => Worker::new_named(send_key(key)).ok()?,
        Client::Insert => Worker::Insert,
        Client::Function(number) => Worker::new_function(number).ok()?,
        Client::Space => Worker::Space,
        Client::Letter(letter) => Worker::new_letter(letter).ok()?,
        Client::Digit(digit) => Worker::new_digit(digit).ok()?,
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
        Client::CtrlC => Worker::CtrlC,
        Client::CtrlD => Worker::CtrlD,
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
                WorkerUnavailableReason::InputBudgetExhausted => {
                    Unavailable::InputBudgetExhausted
                }
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
