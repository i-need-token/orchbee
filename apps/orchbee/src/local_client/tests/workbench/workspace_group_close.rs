use super::*;
use orchbee_workbench_protocol::{
    WorkbenchWorkspaceGroupCloseRequest as R, WorkbenchWorkspaceGroupCloseResult as Reply,
    WorkspaceGroupCloseAction as A, WorkspaceGroupCloseMember, WorkspaceGroupCloseOutcome as O,
    WorkspaceGroupCloseStatus as S,
};

fn request(action: A) -> R {
    R {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(FIRST_REQUEST_ID).unwrap(),
        site_id: SiteId::parse(&id(1)).unwrap(),
        action,
    }
}
fn start() -> R {
    request(A::Start {
        operation_id: WorkbenchRequestId::parse(SECOND_REQUEST_ID).unwrap(),
        project_id: WorkbenchProjectId::parse(&id(21)).unwrap(),
        workspace_id: WorkbenchWorkspaceId::parse(&id(22)).unwrap(),
    })
}
fn pending(request: &R) -> Reply {
    Reply {
        protocol_version: ProtocolVersion,
        request_id: request.request_id.clone(),
        operation_id: request.action.operation_id().clone(),
        outcome: O::Available {
            project_id: WorkbenchProjectId::parse(&id(21)).unwrap(),
            attempt: 1,
            workspace_id: WorkbenchWorkspaceId::parse(&id(22)).unwrap(),
            status: S::Pending {},
            members: vec![WorkspaceGroupCloseMember {
                project_id: WorkbenchProjectId::parse(&id(21)).unwrap(),
                workspace_id: WorkbenchWorkspaceId::parse(&id(22)).unwrap(),
                label: WorkbenchLabel::parse("main").unwrap(),
                close: None,
            }],
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
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 60)?;
        for request in [expected_first, expected_read] {
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::WorkspaceGroupClose(request.clone())
            );
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::WorkspaceGroupClose(pending(&request)),
            )
            .map_err(io::Error::other)?;
        }
        Ok(())
    })?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let deadline = client.operation_clock().now() + SERVER_GUARD_TIMEOUT;
    for request in [first, read] {
        let reply = client
            .submit_workspace_group_close(request.clone(), deadline, None)
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
fn wrong_operation_project_workspace_or_resume_generation_retires_the_exchange_without_replay()
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
                let O::Available {
                    project_id,
                    members,
                    ..
                } = &mut reply.outcome
                else {
                    unreachable!()
                };
                *project_id = WorkbenchProjectId::parse(&id(60)).unwrap();
                members[0].project_id = project_id.clone();
            }
            3 => {
                let O::Available {
                    workspace_id,
                    members,
                    ..
                } = &mut reply.outcome
                else {
                    unreachable!()
                };
                *workspace_id = WorkbenchWorkspaceId::parse(&id(60)).unwrap();
                members[0].workspace_id = workspace_id.clone();
            }
            _ => {} // Resume must return the new attempt, not its failed predecessor.
        }
        let expected = request.clone();
        let server = FakeServer::start(move |mut stream| {
            expect_hello(&mut stream, COHORT)?;
            admit(&mut stream, 60)?;
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::WorkspaceGroupClose(expected)
            );
            write_workbench_response(&mut stream, &WorkbenchResponse::WorkspaceGroupClose(reply))
                .map_err(io::Error::other)?;
            let mut byte = [0];
            assert_eq!(
                stream.read(&mut byte)?,
                0,
                "must close instead of replaying"
            );
            Ok(())
        })?;
        let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
        let deadline = client.operation_clock().now() + SERVER_GUARD_TIMEOUT;
        let error = client
            .submit_workspace_group_close(request.clone(), deadline, None)
            .unwrap_err();
        assert!(error.submitted);
        assert_eq!(error.reason, "response-correlation");
        assert!(client.generation().is_none());
        let unavailable = client
            .submit_workspace_group_close(request, deadline, None)
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
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 60)?;
        assert!(matches!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::WorkspaceGroupClose(_)
        ));
        Ok(())
    })?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let deadline = client.operation_clock().now() + SERVER_GUARD_TIMEOUT;
    let error = client
        .submit_workspace_group_close(start(), deadline, None)
        .unwrap_err();
    assert!(error.submitted);
    assert_eq!(error.reason, "response-read");
    assert!(client.generation().is_none());
    drop(client);
    server.finish()?;
    Ok(())
}
