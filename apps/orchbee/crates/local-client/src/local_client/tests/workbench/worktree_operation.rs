use super::*;
use orchbee_workbench_protocol::{
    WorkbenchWorktreeAction as A, WorkbenchWorktreeOutcome as O, WorkbenchWorktreeRequest as R,
    WorkbenchWorktreeResult as Reply, WorktreeGitEffect, WorktreeIntent,
    WorktreeOperationPhase as Phase, WorktreeOperationStatus as S, WorktreeProgress,
    WorktreeSource, WorktreeWorkspaceDisposition,
};

fn request(action: A) -> R {
    R {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(FIRST_REQUEST_ID).unwrap(),
        session_id: SessionId::parse(&id(1)).unwrap(),
        action,
    }
}
fn start() -> R {
    request(A::Start {
        operation_id: WorkbenchRequestId::parse(SECOND_REQUEST_ID).unwrap(),
        project_id: Some(WorkbenchProjectId::parse(&id(21)).unwrap()),
        intent: WorktreeIntent::Create {
            source: WorktreeSource::Directory {
                path: orchbee_workbench_protocol::WorkbenchDirectoryPath::parse("C:/repo").unwrap(),
            },
            branch: None,
            base: None,
            path: None,
            label: None,
            geometry: terminal_geometry(),
            trust_repository: false,
        },
    })
}
fn pending(request: &R) -> Reply {
    Reply {
        protocol_version: ProtocolVersion,
        request_id: request.request_id.clone(),
        operation_id: request.action.operation_id().clone(),
        outcome: O::Available {
            project_id: Some(WorkbenchProjectId::parse(&id(21)).unwrap()),
            attempt: 1,
            status: Box::new(S::Pending {
                phase: Phase::ObservingSource,
                progress: WorktreeProgress {
                    path: None,
                    checkout: None,
                    git_effect: WorktreeGitEffect::None,
                    restoration: None,
                    workspace: WorktreeWorkspaceDisposition::Unassociated {},
                },
            }),
        },
    }
}

#[test]
fn retained_identity_is_distinct_from_exchange_identity_and_reads_keep_the_session() -> TestResult {
    let first = start();
    let mut read = request(A::Read {
        operation_id: first.action.operation_id().clone(),
    });
    read.request_id = WorkbenchRequestId::parse(&id(50)).unwrap();
    let expected_first = first.clone();
    let expected_read = read.clone();
    let server = FakeServer::start(move |mut stream| {
        handshake::expect_hello(&mut stream)?;
        admit(&mut stream, 60)?;
        for request in [expected_first, expected_read] {
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::WorktreeOperation(request.clone())
            );
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::WorktreeOperation(pending(&request)),
            )
            .map_err(io::Error::other)?;
        }
        Ok(())
    })?;
    let mut client = LocalClient::connect(server.endpoint())?;
    let deadline = client.operation_clock().now() + SERVER_GUARD_TIMEOUT;
    for request in [first, read] {
        let reply = client
            .submit_worktree_operation(request.clone(), deadline, None)
            .unwrap();
        assert_eq!(reply.operation_id, *request.action.operation_id());
        assert_eq!(reply.request_id, request.request_id);
        assert_eq!(client.generation().map(NonZeroU64::get), Some(60));
    }
    drop(client);
    server.finish()?;
    Ok(())
}

#[test]
fn wrong_operation_project_phase_or_resume_generation_retires_the_exchange_without_replay()
-> TestResult {
    for case in 0..5 {
        let request = if case == 4 {
            request(A::Resume {
                operation_id: WorkbenchRequestId::parse(SECOND_REQUEST_ID).unwrap(),
                expected_attempt: 1,
            })
        } else {
            start()
        };
        let mut reply = pending(&request);
        match case {
            0 => reply.operation_id = WorkbenchRequestId::parse(&id(60)).unwrap(),
            1 => reply.request_id = WorkbenchRequestId::parse(&id(60)).unwrap(),
            2 => {
                let O::Available { project_id, .. } = &mut reply.outcome else {
                    unreachable!()
                };
                *project_id = Some(WorkbenchProjectId::parse(&id(60)).unwrap());
            }
            3 => {
                let O::Available { status, .. } = &mut reply.outcome else {
                    unreachable!()
                };
                let S::Pending { phase, .. } = status.as_mut() else {
                    unreachable!()
                };
                *phase = Phase::Removing;
            }
            _ => {} // Resume must return the new attempt, not its failed predecessor.
        }
        let expected = request.clone();
        let server = FakeServer::start(move |mut stream| {
            handshake::expect_hello(&mut stream)?;
            admit(&mut stream, 60)?;
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::WorktreeOperation(expected)
            );
            write_workbench_response(&mut stream, &WorkbenchResponse::WorktreeOperation(reply))
                .map_err(io::Error::other)?;
            let mut byte = [0];
            assert_eq!(
                stream.read(&mut byte)?,
                0,
                "must close instead of replaying"
            );
            Ok(())
        })?;
        let mut client = LocalClient::connect(server.endpoint())?;
        let deadline = client.operation_clock().now() + SERVER_GUARD_TIMEOUT;
        let error = client
            .submit_worktree_operation(request.clone(), deadline, None)
            .unwrap_err();
        assert!(error.submitted);
        assert_eq!(error.reason, "response-correlation");
        assert!(client.generation().is_none());
        let unavailable = client
            .submit_worktree_operation(request, deadline, None)
            .unwrap_err();
        assert!(!unavailable.submitted);
        drop(client);
        server.finish()?;
    }
    Ok(())
}

#[test]
fn peer_eof_after_handoff_is_unknown_and_cannot_be_replayed_on_the_retired_session() -> TestResult {
    let server = FakeServer::start(move |mut stream| {
        handshake::expect_hello(&mut stream)?;
        admit(&mut stream, 60)?;
        assert!(matches!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::WorktreeOperation(_)
        ));
        Ok(())
    })?;
    let mut client = LocalClient::connect(server.endpoint())?;
    let deadline = client.operation_clock().now() + SERVER_GUARD_TIMEOUT;
    let error = client
        .submit_worktree_operation(start(), deadline, None)
        .unwrap_err();
    assert!(error.submitted);
    assert_eq!(error.reason, "response-read");
    assert!(client.generation().is_none());
    drop(client);
    server.finish()?;
    Ok(())
}
