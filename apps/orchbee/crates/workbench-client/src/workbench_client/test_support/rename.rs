//! Fake Server peer for native Tab rename.

use super::{
    Arc, AtomicBool, AtomicU8, CREATED_TAB_ID, ClientMessage, FakeServer, Geometry, ID, Ordering,
    ProtocolVersion, Read, ServerMessage, TerminalControlAction, TerminalControlOutcome,
    TerminalControlResult, TerminalViewerAction, TerminalViewerOutcome, TerminalViewerResult,
    UnixStream, WorkbenchAction, WorkbenchClient, WorkbenchCommandOutcome, WorkbenchCommandResult,
    WorkbenchPublicationResult, WorkbenchRequest, WorkbenchResponse, WorkbenchRevision,
    WorkbenchSnapshotOutcome, WorkbenchSnapshotResult, accept_native_frame_feedback,
    available_snapshot, binding, binding_attach_outcome, binding_viewer_id, full_frame_record, io,
    publication_outcome_for, read_client_message, read_workbench_request,
    serve_native_controlling_frame, two_tabs, write_server_message, write_workbench_response,
};
use std::sync::Mutex;

pub struct RenameServer {
    pub server: FakeServer,
    pub commands: Arc<Mutex<Vec<orchbee_workbench_protocol::WorkbenchCommand>>>,
    pub inputs: Arc<Mutex<Vec<u8>>>,
    pub teardown: Arc<AtomicBool>,
    pub snapshots: Arc<AtomicU8>,
    pub detached: Arc<AtomicBool>,
    pub successor_ready: Arc<AtomicBool>,
}

#[derive(Clone, Copy)]
pub enum RenameResponse {
    Denied,
    Unchanged,
    Changed,
    Recover,
    Removed,
    Successor,
    SuccessorReady,
    WrongLabel,
    Unknown,
    LostSnapshot,
    TabUnavailable,
}

impl RenameServer {
    pub fn start(geometry: Geometry) -> io::Result<Self> {
        Self::with_response(geometry, RenameResponse::Denied)
    }

    pub fn with_response(geometry: Geometry, response: RenameResponse) -> io::Result<Self> {
        let commands = Arc::new(Mutex::new(Vec::<
            orchbee_workbench_protocol::WorkbenchCommand,
        >::new()));
        let inputs = Arc::new(Mutex::new(Vec::new()));
        let teardown = Arc::new(AtomicBool::new(false));
        let recorded = Arc::clone(&commands);
        let forwarded = Arc::clone(&inputs);
        let stopping = Arc::clone(&teardown);
        let snapshots = Arc::new(AtomicU8::new(0));
        let snapshot_count = Arc::clone(&snapshots);
        let detached = Arc::new(AtomicBool::new(false));
        let saw_detach = Arc::clone(&detached);
        let successor_ready = Arc::new(AtomicBool::new(false));
        let saw_successor_feedback = Arc::clone(&successor_ready);
        let server = FakeServer::start(move |stream| {
            let mut stream = UnixStream::new(stream);
            let result = (|| {
                serve_native_controlling_frame(&mut stream, geometry)?;
                accept_native_frame_feedback(&mut stream)?;
                let mut successor_stage = 0;
                loop {
                    let mut magic = [0; 4];
                    if stream.read_exact(&mut magic).is_err() {
                        return Ok(());
                    }
                    let mut reader = io::Cursor::new(magic).chain(&mut stream);
                    if magic == *b"OBWP" {
                        let request =
                            read_workbench_request(&mut reader).map_err(io::Error::other)?;
                        let command = match request {
                            WorkbenchRequest::CloseOperationRead(_)
                            | WorkbenchRequest::TabCloseRead(_)
                            | WorkbenchRequest::TabCloseResume(_)
                            | WorkbenchRequest::WorkspaceCloseRead(_)
                            | WorkbenchRequest::WorkspaceCloseResume(_)
                            | WorkbenchRequest::WorktreeListRead(_)
                            | WorkbenchRequest::WorktreeOperation(_)
                            | WorkbenchRequest::WorktreeQuery(_)
                            | WorkbenchRequest::AgentManifests(_)
                            | WorkbenchRequest::Configuration(_)
                            | WorkbenchRequest::Settings(_)
                            | WorkbenchRequest::ConfiguredCommand(_)
                            | WorkbenchRequest::Events(_)
                            | WorkbenchRequest::PaneGraphics(_)
                            | WorkbenchRequest::Plugin(_)
                            | WorkbenchRequest::TerminalEditor(_)
                            | WorkbenchRequest::WindowTitle(_)
                            | WorkbenchRequest::Notification(_)
                            | WorkbenchRequest::PanelAgent(_)
                            | WorkbenchRequest::AgentView(_)
                            | WorkbenchRequest::PanelMetadata(_)
                            | WorkbenchRequest::WorkspaceMetadata(_)
                            | WorkbenchRequest::WorkspaceGit(_)
                            | WorkbenchRequest::ClientView(_)
                            | WorkbenchRequest::WorkspaceGroupClose(_)
                            | WorkbenchRequest::LaunchProfileCatalogRead(_) => {
                                return Err(io::Error::other(
                                    "Rename unexpectedly issued a non-command request",
                                ));
                            }
                            WorkbenchRequest::Command(command) => command,
                            WorkbenchRequest::SnapshotRead(request) => {
                                let Some(outcome) =
                                    next_rename_snapshot(&recorded, &snapshot_count, response)
                                else {
                                    return Ok(());
                                };
                                write_workbench_response(
                                    &mut stream,
                                    &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
                                        protocol_version: ProtocolVersion,
                                        request_id: request.request_id,
                                        outcome,
                                    }),
                                )
                                .map_err(io::Error::other)?;
                                continue;
                            }
                            WorkbenchRequest::PublicationRead(request) => {
                                let Some(outcome) =
                                    next_rename_snapshot(&recorded, &snapshot_count, response)
                                else {
                                    return Ok(());
                                };
                                write_workbench_response(
                                    &mut stream,
                                    &WorkbenchResponse::Publication(WorkbenchPublicationResult {
                                        protocol_version: ProtocolVersion,
                                        request_id: request.request_id,
                                        outcome: publication_outcome_for(
                                            outcome,
                                            request.known_revision,
                                        ),
                                    }),
                                )
                                .map_err(io::Error::other)?;
                                continue;
                            }
                        };
                        assert!(
                            matches!(&command.action, WorkbenchAction::TabRename {terminal_context_id, tab_id, ..}
                            if terminal_context_id.as_str() == ID && tab_id.as_str() == ID)
                        );
                        recorded.lock().unwrap().push(command.clone());
                        if matches!(response, RenameResponse::Unknown) {
                            return Ok(());
                        }
                        let outcome = match response {
                            RenameResponse::Denied => WorkbenchCommandOutcome::Denied {},
                            RenameResponse::Unchanged => WorkbenchCommandOutcome::Unchanged { revision: command.expected_revision },
                            RenameResponse::TabUnavailable => WorkbenchCommandOutcome::Rejected { revision: command.expected_revision, reason: orchbee_workbench_protocol::WorkbenchRejectionReason::TabUnavailable },
                            _ => WorkbenchCommandOutcome::Renamed { revision: WorkbenchRevision::new(2) },
                        };
                        write_workbench_response(
                            &mut stream,
                            &WorkbenchResponse::Command(WorkbenchCommandResult {
                                protocol_version: ProtocolVersion,
                                request_id: command.request_id.clone(),
                                outcome,
                            }),
                        )
                        .map_err(io::Error::other)?;
                        continue;
                    }
                    assert_eq!(magic, *b"OBCS");
                    let response = match read_client_message(&mut reader)
                        .map_err(io::Error::other)?
                    {
                        ClientMessage::TerminalViewer(request)
                            if matches!(
                                request.action(),
                                TerminalViewerAction::Detach { attachment_id }
                                    if attachment_id == &binding_viewer_id()?
                            ) =>
                        {
                            assert!(matches!(
                                response,
                                RenameResponse::Removed
                                    | RenameResponse::Successor
                                    | RenameResponse::SuccessorReady
                            ));
                            assert!(!saw_detach.swap(true, Ordering::SeqCst));
                            ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                                request.request_id().clone(),
                                TerminalViewerOutcome::Detached {},
                            ))
                        }
                        ClientMessage::TerminalViewer(request)
                            if matches!(
                                request.action(),
                                TerminalViewerAction::AttachBinding { .. }
                            ) =>
                        {
                            assert!(matches!(
                                response,
                                RenameResponse::Successor | RenameResponse::SuccessorReady
                            ));
                            assert!(saw_detach.load(Ordering::SeqCst));
                            assert!(
                                matches!(request.action(), TerminalViewerAction::AttachBinding { terminal_id, .. } if terminal_id.as_str() == CREATED_TAB_ID)
                            );
                            let outcome = if matches!(response, RenameResponse::SuccessorReady) {
                                assert_eq!(successor_stage, 0);
                                successor_stage = 1;
                                binding_attach_outcome()?
                            } else {
                                TerminalViewerOutcome::Rejected { reason: orchbee_client_server_protocol::ViewerRejectedReason::TerminalUnavailable }
                            };
                            ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                                request.request_id().clone(),
                                outcome,
                            ))
                        }
                        ClientMessage::TerminalViewer(request)
                            if matches!(
                                request.action(),
                                TerminalViewerAction::Poll { attachment_id }
                                    if attachment_id == &binding_viewer_id()?
                            ) =>
                        {
                            let outcome = if successor_stage == 3 {
                                successor_stage = 4;
                                TerminalViewerOutcome::Record(
                                    full_frame_record(1, 1, geometry)
                                        .map_err(|error| io::Error::other(error.to_string()))?,
                                )
                            } else {
                                TerminalViewerOutcome::Empty {}
                            };
                            ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                                request.request_id().clone(),
                                outcome,
                            ))
                        }
                        ClientMessage::TerminalViewer(request)
                            if matches!(
                                request.action(),
                                TerminalViewerAction::FrameApplied { attachment_id, .. }
                                    if attachment_id == &binding_viewer_id()?
                            ) =>
                        {
                            assert_eq!(successor_stage, 4);
                            successor_stage = 5;
                            saw_successor_feedback.store(true, Ordering::SeqCst);
                            ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                                request.request_id().clone(),
                                TerminalViewerOutcome::FeedbackAccepted {},
                            ))
                        }
                        ClientMessage::TerminalControl(request) => {
                            let outcome = match request.action() {
                                TerminalControlAction::Clear { .. } => {
                                    panic!("unexpected clear in controller fixture")
                                }
                                TerminalControlAction::Takeover { .. } => {
                                    panic!("unexpected takeover in rename fixture")
                                }
                                TerminalControlAction::HostColors { .. } => {
                                    panic!("unexpected host colors in rename fixture")
                                }
                                TerminalControlAction::Acquire { attachment_id } => {
                                    assert_eq!(attachment_id, &binding_viewer_id()?);
                                    assert_eq!(successor_stage, 1);
                                    successor_stage = 2;
                                    TerminalControlOutcome::Acquired {
                                        controller_id:
                                            orchbee_client_server_protocol::ControllerId::parse(ID)
                                                .map_err(io::Error::other)?,
                                    }
                                }
                                TerminalControlAction::Input {
                                    attachment_id,
                                    bytes,
                                    ..
                                } => {
                                    assert_eq!(attachment_id, &binding_viewer_id()?);
                                    forwarded.lock().unwrap().extend(bytes.as_bytes());
                                    TerminalControlOutcome::InputAccepted {}
                                }
                                TerminalControlAction::Resize {
                                    attachment_id,
                                    geometry,
                                    ..
                                } => {
                                    assert_eq!(attachment_id, &binding_viewer_id()?);
                                    if successor_stage == 2 {
                                        successor_stage = 3;
                                    }
                                    TerminalControlOutcome::Resized {
                                        geometry: *geometry,
                                    }
                                }
                                TerminalControlAction::Paste { .. } => {
                                    TerminalControlOutcome::Pasted {
                                        accepted_bytes:
                                            orchbee_client_server_protocol::PasteAcceptedBytes::new(
                                                1,
                                            )
                                            .map_err(io::Error::other)?,
                                    }
                                }
                            };
                            ServerMessage::TerminalControlResult(TerminalControlResult::new(
                                request.request_id().clone(),
                                outcome,
                            ))
                        }
                        other => {
                            return Err(io::Error::other(format!(
                                "unexpected terminal request {other:?}"
                            )));
                        }
                    };
                    write_server_message(&mut stream, &response).map_err(io::Error::other)?;
                }
            })();
            match result {
                Err(error) if stopping.load(Ordering::SeqCst) && disconnected(&error) => Ok(()),
                other => other,
            }
        })?;
        Ok(Self {
            server,
            commands,
            inputs,
            teardown,
            snapshots,
            detached,
            successor_ready,
        })
    }

    pub fn finish(self, workbench_client: WorkbenchClient) -> io::Result<()> {
        self.teardown.store(true, Ordering::SeqCst);
        drop(workbench_client);
        self.server.finish()
    }
}

pub fn disconnected(error: &(dyn std::error::Error + 'static)) -> bool {
    error.downcast_ref::<io::Error>().is_some_and(|error| {
        matches!(
            error.kind(),
            io::ErrorKind::BrokenPipe | io::ErrorKind::ConnectionReset
        )
    }) || error.source().is_some_and(disconnected)
}

/// Computes the scripted outcome for whichever Workbench read arrived. The first read before any
/// rename command is the bootstrap snapshot; later reads walk the ordinal script. `None` tells the
/// caller to drop the connection (`LostSnapshot`).
pub fn next_rename_snapshot(
    recorded: &Arc<Mutex<Vec<orchbee_workbench_protocol::WorkbenchCommand>>>,
    snapshot_count: &Arc<AtomicU8>,
    response: RenameResponse,
) -> Option<WorkbenchSnapshotOutcome> {
    if recorded.lock().unwrap().is_empty() {
        return Some(available_snapshot(Some(binding())));
    }
    let ordinal = snapshot_count.fetch_add(1, Ordering::SeqCst);
    if matches!(response, RenameResponse::LostSnapshot) {
        return None;
    }
    let WorkbenchSnapshotOutcome::Available { mut snapshot } = available_snapshot(Some(binding()))
    else {
        unreachable!()
    };
    snapshot.revision = WorkbenchRevision::new(if ordinal == 0 { 2 } else { 3 });
    if matches!(
        response,
        RenameResponse::Successor | RenameResponse::SuccessorReady
    ) {
        snapshot = two_tabs();
        snapshot.revision = WorkbenchRevision::new(3);
        snapshot.terminal_contexts[0].tabs.remove(0);
    } else if matches!(response, RenameResponse::Removed) {
        snapshot.revision = WorkbenchRevision::new(3);
        snapshot.terminal_contexts[0].tabs.clear();
    } else if let WorkbenchAction::TabRename { label, .. } =
        &recorded.lock().unwrap().last().unwrap().action
    {
        snapshot.terminal_contexts[0].tabs[0].label =
            if matches!(response, RenameResponse::WrongLabel) {
                "other".to_owned()
            } else {
                label.as_str().to_owned()
            };
    }
    Some(
        if matches!(response, RenameResponse::Recover) && ordinal == 0 {
            WorkbenchSnapshotOutcome::Denied {}
        } else {
            WorkbenchSnapshotOutcome::Available { snapshot }
        },
    )
}
