//! Fake Server peer for interactive Tab close confirmation.

use super::{
    Arc, AtomicBool, ClientMessage, ClientView, DispatchOutcome, FakeServer, Geometry, ID,
    InputDispatcher, Instant, Ordering, ProtocolVersion, Read, ServerMessage,
    TerminalControlAction, TerminalControlOutcome, TerminalControlResult, TerminalViewerAction,
    TerminalViewerOutcome, TerminalViewerResult, UnixStream, WorkbenchAction, WorkbenchClient,
    WorkbenchCommandOutcome, WorkbenchCommandResult, WorkbenchPublicationOutcome,
    WorkbenchPublicationResult, WorkbenchRequest, WorkbenchResponse, WorkbenchTabId,
    WorkbenchTerminalContextId, accept_native_frame_feedback, binding_viewer_id, dispatch_input,
    io, read_client_message, read_workbench_request, serve_native_controlling_frame,
    write_server_message, write_workbench_response,
};
use orchbee_workbench_protocol::{
    CloseOperationOutcome, CloseOperationResult, CloseOperationTarget, WorkbenchRequestId,
    WorkbenchTabCloseAttemptGeneration, WorkbenchTabCloseBlockedReason,
    WorkbenchTabCloseResumeOutcome, WorkbenchTabCloseResumeResult, WorkbenchTabCloseStatus,
};
use std::{convert::Infallible, sync::atomic::AtomicUsize};

pub struct CloseTestServer {
    pub server: FakeServer,
    pub teardown: Arc<AtomicBool>,
}

impl CloseTestServer {
    pub fn finish(self, workbench_client: WorkbenchClient) -> io::Result<()> {
        self.teardown.store(true, Ordering::SeqCst);
        drop(workbench_client);
        self.server.finish()
    }
}

pub(crate) fn peer_disconnected(error: &(dyn std::error::Error + 'static)) -> bool {
    error.downcast_ref::<io::Error>().is_some_and(|error| {
        matches!(
            error.kind(),
            io::ErrorKind::BrokenPipe | io::ErrorKind::ConnectionReset
        )
    }) || error.source().is_some_and(peer_disconnected)
}

pub fn dispatch_close_input(
    workbench_client: &mut WorkbenchClient,
    view: &mut ClientView,
    bytes: &[u8],
) {
    dispatch_close_input_at(
        &mut InputDispatcher::default(),
        workbench_client,
        view,
        bytes,
        Instant::now(),
    );
}

pub(crate) fn dispatch_close_input_at(
    dispatcher: &mut InputDispatcher,
    workbench_client: &mut WorkbenchClient,
    view: &mut ClientView,
    bytes: &[u8],
    now: Instant,
) {
    assert_eq!(
        dispatch_input(dispatcher, view, bytes, workbench_client, now, |_, _| {
            Ok::<_, Infallible>(())
        }),
        Ok(DispatchOutcome::Continue),
    );
}

pub fn interactive_close_server(
    geometry: Geometry,
    commands: Arc<AtomicUsize>,
    input_bytes: Arc<AtomicUsize>,
) -> io::Result<CloseTestServer> {
    status_close_server(geometry, commands, input_bytes, None)
}

pub(crate) fn status_close_server(
    geometry: Geometry,
    commands: Arc<AtomicUsize>,
    input_bytes: Arc<AtomicUsize>,
    resumes: Option<Arc<AtomicUsize>>,
) -> io::Result<CloseTestServer> {
    let teardown = Arc::new(AtomicBool::new(false));
    let server_teardown = Arc::clone(&teardown);
    let server = FakeServer::start(move |stream| {
        let mut stream = UnixStream::new(stream);
        let result = (|| {
            serve_native_controlling_frame(&mut stream, geometry)?;
            accept_native_frame_feedback(&mut stream)?;
            let mut operation: Option<WorkbenchRequestId> = None;
            loop {
                let mut magic = [0; 4];
                if stream.read_exact(&mut magic).is_err() {
                    return Ok(());
                }
                let mut reader = io::Cursor::new(magic).chain(&mut stream);
                if magic == *b"OBWP" {
                    let request = read_workbench_request(&mut reader).map_err(io::Error::other)?;
                    let command = match request {
                        // Publication probes ride the ordinary polls this loop serves.
                        WorkbenchRequest::PublicationRead(intent) => {
                            write_workbench_response(
                                &mut stream,
                                &WorkbenchResponse::Publication(WorkbenchPublicationResult {
                                    protocol_version: ProtocolVersion,
                                    request_id: intent.request_id,
                                    outcome: WorkbenchPublicationOutcome::Current {
                                        revision: intent.known_revision,
                                    },
                                }),
                            )
                            .map_err(io::Error::other)?;
                            continue;
                        }
                        WorkbenchRequest::Command(command) => command,
                        // Every close this peer follows was accepted, so it is read by identity.
                        WorkbenchRequest::CloseOperationRead(request) if resumes.is_some() => {
                            assert_eq!(Some(&request.operation_id), operation.as_ref());
                            write_workbench_response(
                                &mut stream,
                                &WorkbenchResponse::CloseOperation(CloseOperationResult {
                                    protocol_version: ProtocolVersion,
                                    request_id: request.request_id,
                                    session_id: request.session_id,
                                    operation_id: request.operation_id,
                                    outcome: CloseOperationOutcome::Available {
                                        target: CloseOperationTarget::Tab {
                                            terminal_context_id: WorkbenchTerminalContextId::parse(
                                                ID,
                                            )
                                            .unwrap(),
                                            tab_id: WorkbenchTabId::parse(ID).unwrap(),
                                        },
                                        generation: WorkbenchTabCloseAttemptGeneration::new(1)
                                            .unwrap(),
                                        status: WorkbenchTabCloseStatus::CleanupBlocked {
                                            reason: WorkbenchTabCloseBlockedReason::HostRefused,
                                            resumable: true,
                                        },
                                    },
                                }),
                            )
                            .map_err(io::Error::other)?;
                            continue;
                        }
                        WorkbenchRequest::TabCloseResume(request) if resumes.is_some() => {
                            assert_eq!(Some(&request.close_request_id), operation.as_ref());
                            assert_eq!(request.expected_attempt_generation.value(), 1);
                            write_workbench_response(
                                &mut stream,
                                &WorkbenchResponse::TabCloseResume(WorkbenchTabCloseResumeResult {
                                    protocol_version: ProtocolVersion,
                                    request_id: request.request_id,
                                    session_id: request.session_id,
                                    terminal_context_id: request.terminal_context_id,
                                    tab_id: request.tab_id,
                                    close_request_id: request.close_request_id,
                                    outcome: WorkbenchTabCloseResumeOutcome::Denied {},
                                }),
                            )
                            .map_err(io::Error::other)?;
                            resumes.as_ref().unwrap().fetch_add(1, Ordering::SeqCst);
                            continue;
                        }
                        other => {
                            return Err(io::Error::other(format!(
                                "unexpected Workbench operation: {other:?}"
                            )));
                        }
                    };
                    assert!(
                        matches!(command.action, WorkbenchAction::TabClose { ref terminal_context_id, ref tab_id } if terminal_context_id.as_str() == ID && tab_id.as_str() == ID)
                    );
                    let outcome = if resumes.is_some() {
                        operation = Some(command.request_id.clone());
                        WorkbenchCommandOutcome::TabCloseAccepted {
                            close_request_id: command.request_id.clone(),
                            terminal_context_id: WorkbenchTerminalContextId::parse(ID).unwrap(),
                            tab_id: WorkbenchTabId::parse(ID).unwrap(),
                            accepted_revision: command.expected_revision,
                        }
                    } else {
                        WorkbenchCommandOutcome::Denied {}
                    };
                    write_workbench_response(
                        &mut stream,
                        &WorkbenchResponse::Command(WorkbenchCommandResult {
                            protocol_version: ProtocolVersion,
                            request_id: command.request_id,
                            outcome,
                        }),
                    )
                    .map_err(io::Error::other)?;
                    commands.fetch_add(1, Ordering::SeqCst);
                    continue;
                }
                assert_eq!(magic, *b"OBCS");
                match read_client_message(&mut reader).map_err(io::Error::other)? {
                    ClientMessage::TerminalViewer(request)
                        if matches!(
                            request.action(),
                            TerminalViewerAction::Poll { attachment_id }
                                if attachment_id == &binding_viewer_id()?
                        ) =>
                    {
                        write_server_message(
                            &mut stream,
                            &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                                request.request_id().clone(),
                                TerminalViewerOutcome::Empty {},
                            )),
                        )
                        .map_err(io::Error::other)?;
                    }
                    ClientMessage::TerminalControl(request) => {
                        let outcome = match request.action() {
                            TerminalControlAction::Input {
                                attachment_id,
                                bytes,
                                ..
                            } => {
                                assert_eq!(attachment_id, &binding_viewer_id()?);
                                input_bytes.fetch_add(bytes.as_bytes().len(), Ordering::SeqCst);
                                TerminalControlOutcome::InputAccepted {}
                            }
                            TerminalControlAction::Resize {
                                attachment_id,
                                geometry,
                                ..
                            } => {
                                assert_eq!(attachment_id, &binding_viewer_id()?);
                                TerminalControlOutcome::Resized {
                                    geometry: *geometry,
                                }
                            }
                            other => {
                                return Err(io::Error::other(format!(
                                    "unexpected control: {other:?}"
                                )));
                            }
                        };
                        write_server_message(
                            &mut stream,
                            &ServerMessage::TerminalControlResult(TerminalControlResult::new(
                                request.request_id().clone(),
                                outcome,
                            )),
                        )
                        .map_err(io::Error::other)?;
                    }
                    other => {
                        return Err(io::Error::other(format!(
                            "unexpected terminal operation: {other:?}"
                        )));
                    }
                }
            }
        })();
        // The asserted interaction is complete before finish starts. A reply racing the
        // intentional client shutdown may fail; all earlier transport failures remain errors.
        match result {
            Err(error) if server_teardown.load(Ordering::SeqCst) && peer_disconnected(&error) => {
                Ok(())
            }
            result => result,
        }
    })?;
    Ok(CloseTestServer { server, teardown })
}
