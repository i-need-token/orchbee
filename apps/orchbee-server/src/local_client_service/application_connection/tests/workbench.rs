use crate::workbench_service::{WorkbenchExecution, WorkbenchPolicy, WorkbenchService};

#[test]
fn alternating_carriers_preserve_each_response_and_exact_record_boundaries() {
    let terminal = super::deferred_create().unwrap();
    let terminal_bytes = orchbee_client_server_protocol::encode_client_frame(&terminal).unwrap();
    let read = WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse("ffffffffffffffffffffffffffffffff").unwrap(),
    });
    let mut input = terminal_bytes.clone();
    write_workbench_request(&mut input, &read).unwrap();
    input.extend_from_slice(&terminal_bytes);
    let service = WorkbenchService::new(
        WorkbenchPolicy {
            read: true,
            manage_navigation: true,
            associate_directory: false,
            create_terminal: true,
            manage_terminal_lifecycle: false,
        },
        terminal_service(),
    );
    let expected_snapshot = response(service.execute(read));
    let mut stream = super::MemoryStream::new(input).with_chunks(1, 2);
    let _ended = super::super::run(
        &mut stream,
        |request| super::denied_result(&request),
        |request| service.execute(request),
        |_request| panic!("unexpected Agent request"),
    );
    let mut output = Cursor::new(stream.output);
    assert_eq!(
        orchbee_client_server_protocol::read_server_message(&mut output).unwrap(),
        super::denied_result(&terminal)
    );
    assert_eq!(
        read_workbench_response(&mut output).unwrap(),
        expected_snapshot
    );
    assert_eq!(
        orchbee_client_server_protocol::read_server_message(&mut output).unwrap(),
        super::denied_result(&terminal)
    );
    assert_eq!(
        output.position(),
        u64::try_from(output.get_ref().len()).unwrap()
    );
}
use orchbee_workbench_protocol::{
    ProtocolVersion, SessionId, SnapshotReadIntent, WorkbenchAction, WorkbenchCommand,
    WorkbenchCommandOutcome, WorkbenchLabel, WorkbenchRequest, WorkbenchRequestId,
    WorkbenchResponse, WorkbenchRevision, WorkbenchSnapshotOutcome, read_workbench_response,
    write_workbench_request,
};
use std::{
    io::{self, Cursor, Read, Write},
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};

#[test]
fn a_stalled_large_snapshot_writer_releases_the_gate_before_another_connection_reads_and_commits() {
    let service = Arc::new(WorkbenchService::new(
        WorkbenchPolicy {
            read: true,
            manage_navigation: true,
            associate_directory: false,
            create_terminal: true,
            manage_terminal_lifecycle: false,
        },
        terminal_service(),
    ));
    let request = WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse("ffffffffffffffffffffffffffffffff").unwrap(),
    });
    let WorkbenchResponse::Snapshot(initial) = response(service.execute(request.clone())) else {
        panic!("expected snapshot")
    };
    let WorkbenchSnapshotOutcome::Available { snapshot } = initial.outcome else {
        panic!("expected available")
    };
    // Fill real navigation through its production authority until the encoded projection bound.
    let mut revision = 0;
    let label = "\u{1f41d}".repeat(512);
    let mut project: Option<orchbee_workbench_protocol::WorkbenchProjectId> = None;
    for id in 1_u128..=1024 {
        let action = if let Some(project_id) = &project {
            WorkbenchAction::WorkspaceCreate {
                project_id: Some(project_id.clone()),
                label: WorkbenchLabel::parse(&label).unwrap(),
            }
        } else {
            WorkbenchAction::ProjectCreate {
                label: WorkbenchLabel::parse(&label).unwrap(),
            }
        };
        let command = command(id, &snapshot.session_id, revision, action);
        let WorkbenchResponse::Command(result) = response(service.execute(command)) else {
            panic!("expected command")
        };
        match result.outcome {
            WorkbenchCommandOutcome::ProjectCreated { project_id, .. } => {
                project = Some(project_id);
                revision += 1;
            }
            WorkbenchCommandOutcome::WorkspaceCreated { .. } => {
                revision += 1;
                if revision == 257 {
                    project = None;
                }
            }
            WorkbenchCommandOutcome::Rejected { .. } => break,
            outcome => panic!("unexpected population result {outcome:?}"),
        }
    }
    let mut input = Vec::new();
    write_workbench_request(&mut input, &request).unwrap();
    let (started_tx, started_rx) = mpsc::sync_channel(1);
    let (resume_tx, resume_rx) = mpsc::sync_channel(1);
    let stalled_service = Arc::clone(&service);
    let stalled = thread::spawn(move || {
        let mut stream = StalledWriter {
            input: Cursor::new(input),
            output: Vec::new(),
            started: Some(started_tx),
            resume: resume_rx,
        };
        let _ended = super::super::run(
            &mut stream,
            |_request| panic!("unexpected terminal request"),
            |request| stalled_service.execute(request),
            |_request| panic!("unexpected Agent request"),
        );
        stream.output
    });
    started_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let (completed_tx, completed_rx) = mpsc::sync_channel(1);
    let active_service = Arc::clone(&service);
    let active = thread::spawn(move || {
        let read = response(active_service.execute(request));
        let rename = WorkbenchAction::ProjectRename {
            project_id: orchbee_workbench_protocol::WorkbenchProjectId::parse(
                "00000000000000000000000000000002",
            )
            .unwrap(),
            label: WorkbenchLabel::parse("Shortened").unwrap(),
        };
        let changed = response(active_service.execute(command(
            10000,
            &snapshot.session_id,
            revision,
            rename,
        )));
        completed_tx.send((read, changed)).unwrap();
    });
    let completed = completed_rx.recv_timeout(Duration::from_secs(3));
    // Always release the parked thread, including when the regression keeps the authority locked.
    resume_tx.send(()).unwrap();
    let output = stalled.join().unwrap();
    active.join().unwrap();
    let (read, changed) =
        completed.expect("another Connection must complete before the snapshot writer resumes");
    assert!(
        output.len() > 900 * 1024,
        "witness requires a near-limit snapshot"
    );
    assert_eq!(
        read_workbench_response(&mut Cursor::new(output)).unwrap(),
        read
    );
    assert!(
        matches!(changed, WorkbenchResponse::Command(result) if matches!(result.outcome, WorkbenchCommandOutcome::Renamed { .. }))
    );
}

fn command(
    id: u128,
    session: &SessionId,
    revision: u64,
    action: WorkbenchAction,
) -> WorkbenchRequest {
    WorkbenchRequest::Command(WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{id:032x}")).unwrap(),
        session_id: session.clone(),
        expected_revision: WorkbenchRevision::new(revision),
        action,
    })
}

#[test]
fn postcommit_coordination_failure_closes_without_a_workbench_response() {
    let request = WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee").unwrap(),
    });
    let mut input = Vec::new();
    write_workbench_request(&mut input, &request).unwrap();
    let mut stream = super::MemoryStream::new(input);

    assert!(matches!(
        super::super::run(
            &mut stream,
            |_request| panic!("unexpected terminal request"),
            |_request| WorkbenchExecution::OutcomeUnknown,
            |_request| panic!("unexpected Agent request"),
        ),
        Err(super::ApplicationConnectionError::WorkbenchOutcomeUnknown)
    ));
    assert!(stream.output.is_empty());
}

fn response(execution: WorkbenchExecution) -> WorkbenchResponse {
    let WorkbenchExecution::Response(response) = execution else {
        panic!("test command unexpectedly closed with an unknown outcome")
    };
    response
}

struct StalledWriter {
    input: Cursor<Vec<u8>>,
    output: Vec<u8>,
    started: Option<mpsc::SyncSender<()>>,
    resume: mpsc::Receiver<()>,
}
impl Read for StalledWriter {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.input.read(buffer)
    }
}
impl Write for StalledWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if let Some(started) = self.started.take() {
            started.send(()).map_err(io::Error::other)?;
            self.resume
                .recv_timeout(Duration::from_secs(10))
                .map_err(io::Error::other)?;
        }
        self.output.extend_from_slice(buffer);
        Ok(buffer.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn terminal_service() -> Arc<crate::local_terminal_service::LocalTerminalService> {
    Arc::new(crate::local_terminal_service::LocalTerminalService::new(
        Arc::new(std::sync::Mutex::new(
            crate::worker_registry::WorkerRegistry::new(),
        )),
    ))
}
