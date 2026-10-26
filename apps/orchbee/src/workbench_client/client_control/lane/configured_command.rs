//! One command exchange owner; Client heartbeats and popup delivery remain independent.
use super::{
    AsLocalIo, ClientViewAction, ClientViewControl, ClientViewIntent, CommandCompletion,
    CommandFailure, Duration, JoinHandle, LocalClientOrigin, LocalOperationClock, LocalStream,
    ProtocolVersion, SessionId, Shared, WorkbenchRequestId, io, random_request_id, thread,
};

pub(super) struct Invocation {
    wake: LocalStream,
    task: Option<JoinHandle<()>>,
}
impl Invocation {
    pub(super) fn start(
        shared: Shared,
        origin: LocalClientOrigin,
        clock: LocalOperationClock,
        session_id: SessionId,
        registered: orchbee_workbench_protocol::ClientViewRecord,
        invocation: orchbee_workbench_protocol::ConfiguredCommandInvocation,
    ) -> io::Result<Self> {
        let (wake_read, wake) = LocalStream::pair()?;
        let task = thread::Builder::new()
            .name("orchbee-configured-command".into())
            .spawn(move || {
                let command_id = invocation.command_id.clone();
                let result = origin.reconnect_before_cancellable(
                    clock.now() + Duration::from_secs(2),
                    clock.clone(),
                    Some(wake_read.as_local_io()),
                );
                let completion = match result {
                    Ok(mut client) => exchange(
                        &mut client,
                        &clock,
                        &session_id,
                        &wake_read,
                        &registered,
                        invocation,
                    ),
                    Err(_) => CommandCompletion {
                        command_id,
                        reply: Err(CommandFailure::NotSubmitted),
                        focus: None,
                    },
                };
                let (lock, signal) = &*shared;
                if let Ok(mut handoff) = lock.lock() {
                    handoff.invocation_pending = false;
                    handoff.invocation_result = Some(completion);
                }
                signal.notify_all();
            })?;
        Ok(Self {
            wake,
            task: Some(task),
        })
    }
    pub(super) fn finished(&self) -> bool {
        self.task.as_ref().is_none_or(JoinHandle::is_finished)
    }
}
impl Drop for Invocation {
    fn drop(&mut self) {
        let _ = self.wake.shutdown(std::net::Shutdown::Both);
        if let Some(task) = self.task.take() {
            let _ = task.join();
        }
    }
}
fn exchange(
    client: &mut crate::local_client::LocalClient,
    clock: &LocalOperationClock,
    session_id: &SessionId,
    wake: &LocalStream,
    registered: &orchbee_workbench_protocol::ClientViewRecord,
    invocation: orchbee_workbench_protocol::ConfiguredCommandInvocation,
) -> CommandCompletion {
    let command_id = invocation.command_id.clone();
    let mut bytes = [0u8; 16];
    let reply = if getrandom::fill(&mut bytes).is_ok() {
        if let Ok(id) = WorkbenchRequestId::parse(&format!("{:032x}", u128::from_be_bytes(bytes))) {
            client
                .configured_command(
                    orchbee_workbench_protocol::ConfiguredCommandRequest {
                        protocol_version: ProtocolVersion,
                        request_id: id,
                        session_id: session_id.clone(),
                        action: orchbee_workbench_protocol::ConfiguredCommandAction::Invoke {
                            invocation: Box::new(invocation),
                        },
                    },
                    clock.now() + Duration::from_secs(10),
                    Some(wake.as_local_io()),
                )
                .map_err(|error| {
                    if error.submitted {
                        CommandFailure::OutcomeUnknown
                    } else {
                        CommandFailure::NotSubmitted
                    }
                })
        } else {
            Err(CommandFailure::NotSubmitted)
        }
    } else {
        Err(CommandFailure::NotSubmitted)
    };
    let focus = reply.as_ref().ok().and_then(|reply| {
                if let orchbee_workbench_protocol::ConfiguredCommandOutcome::Pane { result, .. } = &reply.outcome
                    && let orchbee_workbench_protocol::WorkbenchCommandOutcome::PanelSplitCreated { revision, terminal_context_id, tab_id, new_panel_id, .. } = &result.outcome {
                    let id = random_request_id().ok_or(false);
                    Some(id.and_then(|id| client.exchange_client_view(orchbee_workbench_protocol::WorkbenchClientViewRequest {
                        protocol_version: ProtocolVersion, request_id: id.clone(), session_id: session_id.clone(),
                        action: ClientViewAction::Submit {command: Box::new(ClientViewControl {
                            operation_id: id, client_id: registered.client_id.clone(), expected_view_revision: registered.view_revision,
                            minimum_workbench_revision: *revision, intent: ClientViewIntent::Panel {
                                terminal_context_id: terminal_context_id.clone(), tab_id: tab_id.clone(), panel_id: new_panel_id.clone(),
                            },
                        })},
                    }, clock.now() + Duration::from_secs(2), Some(wake.as_local_io())).map_err(|error| !matches!(error, crate::local_client::workbench::client_view::ClientViewExchangeError::Encoding | crate::local_client::workbench::client_view::ClientViewExchangeError::Unavailable))))
                } else { None }
            });
    CommandCompletion {
        command_id,
        reply,
        focus,
    }
}
