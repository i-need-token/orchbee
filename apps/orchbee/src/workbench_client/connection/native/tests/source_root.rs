//! Primary-root sampling borrows the live primary Viewer or releases its own temporary Viewer.
use super::super::AttachedTerminal;
use super::super::ClientExecutor;
use super::setup::{TestResult, admit, fixture_geometry, snapshot};
use crate::local_client::LocalClient;
use crate::local_client::tests::server::FakeServer;
use crate::workbench_client::primary_directory::tests::process_record;
use crate::workbench_client::state::{AttachmentKey, ControllerMode, PaneKey, PaneRef};
use crate::workbench_client::workbench_projection::{
    ClientContextTabs, ClientTerminalBinding, project_preferred_workspace,
};
use crate::workbench_client::{
    mailbox::CommandMailbox,
    state::{CarrierFailure, OperationDeadlines},
};
use orchbee_client_server_protocol::{
    ClientMessage, ServerMessage, TerminalViewerAction, TerminalViewerOutcome,
    TerminalViewerResult, ViewerId, read_client_message, write_server_message,
};
use orchbee_workbench_protocol::{WorkbenchDirectoryPath, WorkbenchSnapshotOutcome};
use std::io::{self, Read};
use std::{sync::Arc, time::Instant};

/// The source facts a primary-root carrier samples from one initial_workspace snapshot.
pub(in crate::workbench_client::connection::native) struct SourceFixture {
    pub(in crate::workbench_client::connection::native) generation: u64,
    pub(in crate::workbench_client::connection::native) baseline: ClientContextTabs,
    pub(in crate::workbench_client::connection::native) primary: Option<ClientTerminalBinding>,
    pub(in crate::workbench_client::connection::native) primary_pane: Option<PaneRef>,
    pub(in crate::workbench_client::connection::native) deadlines: OperationDeadlines,
}

fn preparation() -> TestResult<(SourceFixture, WorkbenchSnapshotOutcome)> {
    let snapshot = snapshot(1, 1);
    let baseline = project_preferred_workspace(&snapshot).unwrap();
    let primary = baseline.tabs()[0]
        .panel(0)
        .unwrap()
        .primary()
        .binding()
        .cloned();
    Ok((
        SourceFixture {
            generation: 1,
            baseline,
            primary,
            primary_pane: None,
            deadlines: OperationDeadlines::from_sample(Instant::now()).unwrap(),
        },
        WorkbenchSnapshotOutcome::Available { snapshot },
    ))
}

pub(in crate::workbench_client::connection::native) fn sample(
    mut prepare: impl FnMut(
        &mut ClientExecutor,
        &SourceFixture,
        &WorkbenchSnapshotOutcome,
    ) -> Result<Option<WorkbenchDirectoryPath>, CarrierFailure>,
) -> TestResult {
    for (cwd, wrong_identity) in [
        (Some("C:/项目 space"), false),
        (None, false),
        (Some("relative"), false),
        (Some("C:/valid"), true),
    ] {
        let record = process_record(cwd, wrong_identity)?;
        let (preparation, outcome) = preparation()?;
        let binding = preparation.primary.clone().unwrap();
        let session = preparation.baseline.session_id().clone();
        let server = FakeServer::start(move |mut stream| {
            admit(&mut stream)?;
            let expected = [
                TerminalViewerAction::AttachBinding {
                    session_id: orchbee_client_server_protocol::SessionId::parse(session.as_str())
                        .map_err(io::Error::other)?,
                    worker_id: orchbee_client_server_protocol::WorkerId::parse(
                        binding.worker_id().as_str(),
                    )
                    .map_err(io::Error::other)?,
                    terminal_id: orchbee_client_server_protocol::TerminalId::parse(
                        binding.terminal_id().as_str(),
                    )
                    .map_err(io::Error::other)?,
                },
                TerminalViewerAction::ReadProcessInfo {
                    attachment_id: ViewerId::parse(&"01".repeat(16)).map_err(io::Error::other)?,
                },
                TerminalViewerAction::Detach {
                    attachment_id: ViewerId::parse(&"01".repeat(16)).map_err(io::Error::other)?,
                },
            ];
            let outcomes = [
                TerminalViewerOutcome::Attached {
                    attachment_id: ViewerId::parse(&"01".repeat(16)).map_err(io::Error::other)?,
                    host_epoch: orchbee_client_server_protocol::HostEpoch::parse(&"04".repeat(16))
                        .map_err(io::Error::other)?,
                    host_terminal_id: orchbee_client_server_protocol::HostTerminalId::parse(
                        &"02".repeat(16),
                    )
                    .map_err(io::Error::other)?,
                },
                TerminalViewerOutcome::Record(record),
                TerminalViewerOutcome::Detached {},
            ];
            let mut ids = Vec::new();
            for (action, outcome) in expected.into_iter().zip(outcomes) {
                let ClientMessage::TerminalViewer(request) =
                    read_client_message(&mut stream).map_err(io::Error::other)?
                else {
                    return Err(io::Error::other("read-only viewer expected"));
                };
                if request.action() != &action || ids.contains(request.request_id()) {
                    return Err(io::Error::other(
                        "wrong source, viewer, request identity or release order",
                    ));
                }
                ids.push(request.request_id().clone());
                write_server_message(
                    &mut stream,
                    &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                        request.request_id().clone(),
                        outcome,
                    )),
                )
                .map_err(io::Error::other)?;
            }
            let mut trailing = [0];
            if stream.read(&mut trailing)? != 0 {
                return Err(io::Error::other(
                    "unscripted creation or repeated preparation",
                ));
            }
            Ok(())
        })?;
        let client = LocalClient::connect(server.endpoint(), "native-source-root-test")?;
        let mailbox = Arc::new(CommandMailbox::new(10, fixture_geometry()?));
        let mut executor = ClientExecutor::new(client, mailbox);
        let result = prepare(&mut executor, &preparation, &outcome);
        if wrong_identity {
            assert_eq!(result, Err(CarrierFailure::Protocol));
        } else if let Some(cwd) = cwd.filter(|cwd| *cwd != "relative") {
            assert_eq!(result, Ok(Some(WorkbenchDirectoryPath::parse(cwd)?)));
        } else {
            assert_eq!(result, Ok(None));
        }
        assert!(
            executor.attachments.is_empty(),
            "temporary Viewer cannot become a pane attachment"
        );
        drop(executor);
        server.finish()?;
    }
    Ok(())
}

pub(in crate::workbench_client::connection::native) fn borrow(
    mut prepare: impl FnMut(
        &mut ClientExecutor,
        &SourceFixture,
        &WorkbenchSnapshotOutcome,
    ) -> Result<Option<WorkbenchDirectoryPath>, CarrierFailure>,
) -> TestResult {
    for wrong_identity in [false, true] {
        let record = process_record(Some("C:/live-root"), wrong_identity)?;
        let (mut preparation, outcome) = preparation()?;
        let tab = &preparation.baseline.tabs()[0];
        let pane = PaneRef::new(
            PaneKey::new(
                AttachmentKey::new(1, tab.tab_id().clone()),
                tab.panel(0).unwrap().panel_id().clone(),
            ),
            1,
        );
        preparation.primary_pane = Some(pane.clone());
        let attachment_id = ViewerId::parse(&"01".repeat(16))?;
        let expected = attachment_id.clone();
        let server = FakeServer::start(move |mut stream| {
            admit(&mut stream)?;
            let ClientMessage::TerminalViewer(request) =
                read_client_message(&mut stream).map_err(io::Error::other)?
            else {
                return Err(io::Error::other("borrowed read expected"));
            };
            if request.action()
                != &(TerminalViewerAction::ReadProcessInfo {
                    attachment_id: expected,
                })
            {
                return Err(io::Error::other("live Viewer must never be attached twice"));
            }
            write_server_message(
                &mut stream,
                &ServerMessage::TerminalViewerResult(TerminalViewerResult::new(
                    request.request_id().clone(),
                    TerminalViewerOutcome::Record(record),
                )),
            )
            .map_err(io::Error::other)?;
            let mut trailing = [0];
            if stream.read(&mut trailing)? != 0 {
                return Err(io::Error::other(
                    "borrowed Viewer must not be detached, resized or replayed",
                ));
            }
            Ok(())
        })?;
        let client = LocalClient::connect(server.endpoint(), "native-source-root-test")?;
        let mut executor = ClientExecutor::new(
            client,
            Arc::new(CommandMailbox::new(10, fixture_geometry()?)),
        );
        executor.attachments.push(AttachedTerminal {
            key: pane.clone(),
            attachment_id,
            host_epoch: orchbee_client_server_protocol::HostEpoch::parse(&"04".repeat(16))?,
            host_terminal_id: orchbee_client_server_protocol::HostTerminalId::parse(
                &"02".repeat(16),
            )?,
            setup: None,
            announced: true,
            mode: Some(ControllerMode::ReadOnly),
            next_sequence: 47,
            accepted_geometry: None,
            reported_host_colors: None,
        });
        let result = prepare(&mut executor, &preparation, &outcome);
        assert_eq!(
            result,
            if wrong_identity {
                Err(CarrierFailure::Protocol)
            } else {
                Ok(Some(WorkbenchDirectoryPath::parse("C:/live-root")?))
            }
        );
        assert_eq!(executor.attachments.len(), 1);
        assert_eq!(executor.attachments[0].key, pane);
        assert_eq!(executor.attachments[0].next_sequence, 47);
        assert_eq!(executor.attachments[0].mode, Some(ControllerMode::ReadOnly));
        assert!(executor.attachments[0].announced && executor.attachments[0].setup.is_none());
        drop(executor);
        server.finish()?;
    }
    Ok(())
}
