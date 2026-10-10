use super::{RunningServer, TestResult, connect_bounded_client};
use orchbee_workbench_protocol::{
    ProtocolVersion, SiteId, Snapshot, SnapshotReadIntent, WorkbenchAction, WorkbenchCommand,
    WorkbenchCommandOutcome, WorkbenchCommandResult, WorkbenchLabel, WorkbenchRejectionReason,
    WorkbenchRequest, WorkbenchRequestId, WorkbenchResponse, WorkbenchRevision,
    WorkbenchSnapshotOutcome, read_workbench_response, serialize_workbench_response,
    write_workbench_request,
};
use std::{
    io,
    os::unix::net::UnixStream,
    sync::{Arc, Barrier},
    thread,
};

fn read(stream: &mut UnixStream) -> TestResult<Snapshot> {
    let request = WorkbenchRequest::SnapshotRead(SnapshotReadIntent {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse("ffffffffffffffffffffffffffffffff")?,
    });
    write_workbench_request(stream, &request)?;
    match read_workbench_response(stream)? {
        WorkbenchResponse::Snapshot(result) => match result.outcome {
            WorkbenchSnapshotOutcome::Available { snapshot } => Ok(snapshot),
            outcome => Err(io::Error::other(format!("snapshot unavailable: {outcome:?}")).into()),
        },
        response => Err(io::Error::other(format!("wrong response: {response:?}")).into()),
    }
}
fn command(id: u128, site: &SiteId, revision: u64, action: WorkbenchAction) -> WorkbenchRequest {
    WorkbenchRequest::Command(WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&format!("{id:032x}")).unwrap(),
        site_id: site.clone(),
        expected_revision: WorkbenchRevision::new(revision),
        action,
    })
}
fn create(label: &str) -> WorkbenchAction {
    WorkbenchAction::ProjectCreate {
        label: WorkbenchLabel::parse(label).unwrap(),
    }
}

#[test]
fn an_unread_committed_response_is_reconciled_after_the_original_client_disconnects() -> TestResult
{
    let server = RunningServer::start()?;
    let (mut original, _) = connect_bounded_client(server.client_endpoint())?;
    let (mut observer, _) = connect_bounded_client(server.client_endpoint())?;
    let initial = read(&mut original)?;
    let request = command(
        1,
        &initial.site_id,
        0,
        create("Committed before response loss"),
    );
    write_workbench_request(&mut original, &request)?;
    let deadline = std::time::Instant::now() + super::PROCESS_TIMEOUT;
    loop {
        let observed = read(&mut observer)?;
        if observed.revision.value() == 1 {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "Server did not commit the submitted request"
        );
        thread::yield_now();
    }
    // Never consume the original response; a different Client observes the committed effect.
    drop(original);
    let reconciled = exchange(&mut observer, &request)?;
    assert!(matches!(
        reconciled.outcome,
        WorkbenchCommandOutcome::ProjectCreated { .. }
    ));
    assert_eq!(exchange(&mut observer, &request)?, reconciled);
    let after = read(&mut observer)?;
    assert_eq!(after.revision.value(), 1);
    assert_eq!(after.navigation.projects.len(), 1);
    Ok(())
}
fn exchange(
    stream: &mut UnixStream,
    request: &WorkbenchRequest,
) -> TestResult<WorkbenchCommandResult> {
    write_workbench_request(stream, request)?;
    match read_workbench_response(stream)? {
        WorkbenchResponse::Command(result) => Ok(result),
        response => Err(io::Error::other(format!("wrong response: {response:?}")).into()),
    }
}

#[test]
fn two_clients_modify_the_same_navigation_without_a_worker_and_reconcile_after_disconnect()
-> TestResult {
    let server = RunningServer::start()?;
    let (mut first, _) = connect_bounded_client(server.client_endpoint())?;
    let (mut second, _) = connect_bounded_client(server.client_endpoint())?;
    let initial = read(&mut first)?;
    assert_eq!(read(&mut second)?, initial);
    let request = command(1, &initial.site_id, 0, create("Shared"));
    let created = exchange(&mut first, &request)?;
    let WorkbenchCommandOutcome::ProjectCreated { project_id, .. } = &created.outcome else {
        panic!("expected project")
    };
    let after = read(&mut second)?;
    assert_eq!(after.navigation.projects[0].project_id, *project_id);
    let workspace = command(
        2,
        &initial.site_id,
        1,
        WorkbenchAction::WorkspaceCreate {
            project_id: Some(project_id.clone()),
            label: WorkbenchLabel::parse("Workspace")?,
        },
    );
    assert!(matches!(
        exchange(&mut second, &workspace)?.outcome,
        WorkbenchCommandOutcome::WorkspaceCreated { .. }
    ));
    assert_eq!(read(&mut first)?.terminal_contexts.len(), 2);
    drop(first);
    let (mut replacement, _) = connect_bounded_client(server.client_endpoint())?;
    assert_eq!(exchange(&mut replacement, &request)?, created);
    assert_eq!(read(&mut replacement)?, read(&mut second)?);
    assert_eq!(read(&mut second)?.revision.value(), 2);
    Ok(())
}

#[test]
fn competing_sessions_commit_one_revision_and_same_identity_races_settle_once() -> TestResult {
    for race in 0..3 {
        let server = RunningServer::start()?;
        let (mut first, _) = connect_bounded_client(server.client_endpoint())?;
        let (mut second, _) = connect_bounded_client(server.client_endpoint())?;
        let initial = read(&mut first)?;
        let first_request = command(1, &initial.site_id, 0, create("A"));
        let second_request = command(
            if race == 0 { 2 } else { 1 },
            &initial.site_id,
            0,
            create(if race == 1 { "A" } else { "B" }),
        );
        let barrier = Arc::new(Barrier::new(2));
        let peer_barrier = Arc::clone(&barrier);
        let peer = thread::spawn(move || {
            peer_barrier.wait();
            let result = exchange(&mut second, &second_request).map_err(|error| error.to_string());
            (second, result)
        });
        barrier.wait();
        let first_result = exchange(&mut first, &first_request)?;
        let (mut second, second_result) = peer.join().unwrap();
        let second_result = second_result.map_err(io::Error::other)?;
        if race == 1 {
            assert_eq!(first_result, second_result);
        } else {
            let outcomes = [&first_result.outcome, &second_result.outcome];
            assert_eq!(
                outcomes
                    .iter()
                    .filter(|outcome| matches!(
                        outcome,
                        WorkbenchCommandOutcome::ProjectCreated { .. }
                    ))
                    .count(),
                1
            );
            let reason = if race == 0 {
                WorkbenchRejectionReason::RevisionMismatch
            } else {
                WorkbenchRejectionReason::RequestIdentityConflict
            };
            assert_eq!(outcomes.iter().filter(|outcome| matches!(outcome, WorkbenchCommandOutcome::Rejected { reason: actual, .. } if *actual == reason)).count(), 1);
        }
        let snapshot = read(&mut first)?;
        assert_eq!(snapshot.revision.value(), 1);
        assert_eq!(snapshot.navigation.projects.len(), 1);
        assert_eq!(snapshot, read(&mut second)?);
    }
    Ok(())
}

#[test]
fn snapshots_larger_than_the_terminal_carrier_limit_cross_the_admitted_stream() -> TestResult {
    let server = RunningServer::start()?;
    let (mut writer, _) = connect_bounded_client(server.client_endpoint())?;
    let initial = read(&mut writer)?;
    let label = "L".repeat(512);
    for index in 0..72 {
        assert!(matches!(
            exchange(
                &mut writer,
                &command(
                    u128::from(index) + 1,
                    &initial.site_id,
                    index,
                    create(&label)
                )
            )?
            .outcome,
            WorkbenchCommandOutcome::ProjectCreated { .. }
        ));
    }
    let (mut reader, _) = connect_bounded_client(server.client_endpoint())?;
    let snapshot = read(&mut reader)?;
    assert_eq!(snapshot, read(&mut writer)?);
    let response =
        WorkbenchResponse::Snapshot(orchbee_workbench_protocol::WorkbenchSnapshotResult {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse("ffffffffffffffffffffffffffffffff")?,
            outcome: WorkbenchSnapshotOutcome::Available { snapshot },
        });
    assert!(serialize_workbench_response(&response)?.len() > 32 * 1024);
    Ok(())
}

#[test]
fn terminal_and_workbench_records_alternate_on_one_admitted_connection() -> TestResult {
    use orchbee_client_server_protocol::{
        ClientMessage, RequestId, ServerMessage, StartMode, TerminalCreateRequest,
        read_server_message, write_client_message,
    };
    let server = RunningServer::start()?;
    let (mut client, _) = connect_bounded_client(server.client_endpoint())?;
    let initial = read(&mut client)?;
    let request_id = RequestId::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")?;
    let terminal = ClientMessage::TerminalCreate(TerminalCreateRequest::new(
        request_id.clone(),
        None,
        None,
        StartMode::Deferred,
        None,
    ));
    write_client_message(&mut client, &terminal)?;
    let first = read_server_message(&mut client)?;
    let ServerMessage::TerminalCreateResult(result) = &first else {
        panic!("expected terminal result")
    };
    assert_eq!(result.request_id(), &request_id);
    let created = exchange(
        &mut client,
        &command(1, &initial.site_id, 0, create("Mixed carriers")),
    )?;
    assert!(matches!(
        created.outcome,
        WorkbenchCommandOutcome::ProjectCreated { .. }
    ));
    write_client_message(&mut client, &terminal)?;
    assert_eq!(read_server_message(&mut client)?, first);
    assert_eq!(
        read(&mut client)?.navigation.projects[0].label,
        "Mixed carriers"
    );
    Ok(())
}
