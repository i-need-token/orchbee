use super::*;
use std::thread;
mod sidebar;
use crate::local_client::test_support::server::FakeServer;
use crate::workbench_client::state::worktree_launch::{
    WorktreeMode,
    tests::{canonical, confirm, discovery, fixture},
};
use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
use orchbee_workbench_protocol::*;
use std::{io::Read, num::NonZeroU64, sync::mpsc};

fn id(n: u128) -> WorkbenchRequestId {
    WorkbenchRequestId::parse(&format!("{n:032x}")).unwrap()
}
fn handshake(stream: &mut LocalStream, generation: u64) -> io::Result<()> {
    assert!(matches!(
        read_message(stream).map_err(io::Error::other)?,
        LocalClientMessage::ClientHello(_)
    ));
    write_message(
        stream,
        &LocalClientMessage::ClientAccepted {
            generation: NonZeroU64::new(generation).unwrap(),
        },
    )
    .map_err(io::Error::other)
}
fn plan() -> Plan {
    let (mut l, o) = fixture(1, WorktreeMode::Open);
    confirm(&mut l, &o).unwrap()
}
fn job(action: Action) -> Job {
    Job {
        action,
        requests: [id(101), id(102), id(103), id(104), id(105)],
        viewers: std::array::from_fn(|index| {
            orchbee_client_server_protocol::RequestId::parse(&format!("{:032x}", 1000 + index))
                .unwrap()
        }),
        deadline: Instant::now() + Duration::from_secs(3),
        configured_root: Ok(None),
    }
}
fn lane(server: &FakeServer) -> Lane {
    let client = LocalClient::connect(server.endpoint(), "worktree-lane-test").unwrap();
    Lane::spawn(client.origin(), client.operation_clock()).unwrap()
}
fn take(lane: &Lane) -> Reply {
    let until = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(reply) = lane.take() {
            return reply;
        }
        assert!(Instant::now() < until, "bounded lane reply");
        thread::sleep(Duration::from_millis(1));
    }
}
fn unknown() -> WorkbenchWorktreeOutcome {
    WorkbenchWorktreeOutcome::NotFound {}
}

#[test]
fn reads_use_retained_operation_and_reject_foreign_or_missing_replies_without_start() {
    for fault in 0..3 {
        let expected = plan().operation_id;
        let server = FakeServer::start_connections(2, move |index, mut stream| {
            handshake(&mut stream, index as u64 + 1)?;
            if index == 0 {
                return Ok(());
            }
            let WorkbenchRequest::WorktreeOperation(request) =
                read_workbench_request(&mut stream).map_err(io::Error::other)?
            else {
                panic!("read only")
            };
            assert_eq!(
                request.action,
                WorkbenchWorktreeAction::Read {
                    operation_id: expected.clone()
                }
            );
            if fault == 2 {
                return Ok(());
            }
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::WorktreeOperation(WorkbenchWorktreeResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    operation_id: if fault == 1 {
                        id(999)
                    } else {
                        expected.clone()
                    },
                    outcome: unknown(),
                }),
            )
            .map_err(io::Error::other)?;
            assert_eq!(stream.read(&mut [0])?, 0);
            Ok(())
        })
        .unwrap();
        let mut lane = lane(&server);
        assert!(lane.submit(job(Action::Read(Box::new(plan())))));
        let Reply::Operation {
            exchange, result, ..
        } = take(&lane)
        else {
            panic!("operation reply")
        };
        assert_eq!(exchange, Exchange::Read);
        if fault == 0 {
            assert_eq!(result.unwrap(), unknown());
        } else {
            assert!(result.unwrap_err());
        }
        assert!(lane.close());
        server.finish().unwrap();
    }
}

#[test]
fn removal_resume_sends_one_fenced_resume_and_reads_without_source_discovery_or_start_replay() {
    let (mut launcher, origin, _) =
        crate::workbench_client::state::worktree_launch::tests::removal::fixture();
    let plan = confirm(&mut launcher, &origin).unwrap();
    let operation = plan.operation_id.clone();
    let expected = operation.clone();
    let outcome = crate::workbench_client::state::worktree_launch::tests::removal::failure(2, true);
    let reply = outcome.clone();
    let server = FakeServer::start_connections(2, move |index, mut stream| {
        handshake(&mut stream, index as u64 + 1)?;
        if index == 0 {
            return Ok(());
        }
        for resume in [true, false] {
            let WorkbenchRequest::WorktreeOperation(request) =
                read_workbench_request(&mut stream).map_err(io::Error::other)?
            else {
                panic!("recovery must not sample a Viewer, list Git or read unrelated snapshots");
            };
            assert_eq!(
                request.action,
                if resume {
                    WorkbenchWorktreeAction::Resume {
                        operation_id: expected.clone(),
                        expected_attempt: 1,
                    }
                } else {
                    WorkbenchWorktreeAction::Read {
                        operation_id: expected.clone(),
                    }
                }
            );
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::WorktreeOperation(WorkbenchWorktreeResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    operation_id: expected.clone(),
                    outcome: reply.clone(),
                }),
            )
            .map_err(io::Error::other)?;
        }
        assert_eq!(stream.read(&mut [0])?, 0);
        Ok(())
    })
    .unwrap();
    let mut lane = lane(&server);
    assert!(lane.submit(job(Action::Resume {
        plan: Box::new(plan.clone()),
        expected_attempt: 1
    })));
    assert!(
        matches!(take(&lane), Reply::Operation { exchange: Exchange::Resume, result: Ok(ref result), .. } if result == &outcome)
    );
    assert!(lane.submit(job(Action::Read(Box::new(plan)))));
    assert!(
        matches!(take(&lane), Reply::Operation { exchange: Exchange::Read, result: Ok(ref result), .. } if result == &outcome)
    );
    assert!(lane.close());
    server.finish().unwrap();
}

#[test]
fn stalled_read_has_one_bounded_slot_and_joined_interruptible_shutdown() {
    let (tx, rx) = mpsc::sync_channel(1);
    let server = FakeServer::start_connections(2, move |index, mut stream| {
        handshake(&mut stream, index as u64 + 1)?;
        if index == 0 {
            return Ok(());
        }
        assert!(matches!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::WorktreeOperation(_)
        ));
        tx.send(()).unwrap();
        assert_eq!(stream.read(&mut [0])?, 0);
        Ok(())
    })
    .unwrap();
    let mut lane = lane(&server);
    assert!(lane.submit(job(Action::Read(Box::new(plan())))));
    rx.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(!lane.submit(job(Action::Read(Box::new(plan())))));
    let now = Instant::now();
    assert!(lane.close());
    assert!(now.elapsed() < Duration::from_secs(1));
    server.finish().unwrap();
}

#[test]
fn known_worktree_receipt_survives_a_lost_followup_snapshot() {
    let plan = plan();
    let operation = plan.operation_id.clone();
    let outcome = WorkbenchWorktreeOutcome::Available {
        project_id: plan.project.clone(),
        attempt: 1,
        status: Box::new(WorktreeOperationStatus::Opened {
            checkout: WorktreeCheckoutFacts {
                repository_root: WorkbenchDirectoryPath::parse("C:/repo").unwrap(),
                common_directory: WorkbenchDirectoryPath::parse("C:/repo/.git").unwrap(),
                path: WorkbenchDirectoryPath::parse("C:/trees/猫").unwrap(),
                branch: None,
                dirty: false,
            },
            workspace_id: WorkbenchWorkspaceId::parse(&format!("{:032x}", 80)).unwrap(),
            reused: true,
            terminal_outcome: None,
        }),
    };
    let expected = outcome.clone();
    let server = FakeServer::start_connections(2, move |index, mut stream| {
        handshake(&mut stream, index as u64 + 1)?;
        if index == 0 {
            return Ok(());
        }
        let WorkbenchRequest::WorktreeOperation(request) =
            read_workbench_request(&mut stream).map_err(io::Error::other)?
        else {
            panic!("read retained operation")
        };
        assert_eq!(
            request.action,
            WorkbenchWorktreeAction::Read {
                operation_id: operation.clone()
            }
        );
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::WorktreeOperation(WorkbenchWorktreeResult {
                protocol_version: ProtocolVersion,
                request_id: request.request_id,
                operation_id: operation.clone(),
                outcome: expected.clone(),
            }),
        )
        .map_err(io::Error::other)?;
        assert!(matches!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::SnapshotRead(_)
        ));
        Ok(())
    })
    .unwrap();
    let mut lane = lane(&server);
    assert!(lane.submit(job(Action::Read(Box::new(plan)))));
    let Reply::Operation {
        exchange,
        result,
        snapshot,
        ..
    } = take(&lane)
    else {
        panic!("retained receipt")
    };
    assert_eq!(exchange, Exchange::Read);
    assert_eq!(result.unwrap(), outcome);
    assert!(snapshot.is_none());
    assert!(lane.close());
    server.finish().unwrap();
}

#[test]
fn uncertain_start_reconnects_for_read_without_replaying_start() {
    let plan = plan();
    let expected = plan.clone();
    let server = FakeServer::start_connections(3, move |index, mut stream| {
        handshake(&mut stream, index as u64 + 1)?;
        if index == 0 {
            return Ok(());
        }
        if index == 1 {
            let WorkbenchRequest::SnapshotRead(request) =
                read_workbench_request(&mut stream).map_err(io::Error::other)?
            else {
                panic!("snapshot preflight")
            };
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    outcome: WorkbenchSnapshotOutcome::Available {
                        snapshot: canonical(1, 1),
                    },
                }),
            )
            .map_err(io::Error::other)?;
            let WorkbenchRequest::WorktreeQuery(request) =
                read_workbench_request(&mut stream).map_err(io::Error::other)?
            else {
                panic!("source preflight")
            };
            let d = discovery();
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::WorktreeQuery(WorkbenchWorktreeQueryResult {
                    protocol_version: ProtocolVersion,
                    request_id: request.request_id,
                    outcome: WorkbenchWorktreeQueryOutcome::Found {
                        session_id: expected.origin.cache.session_id().clone(),
                        workbench_revision: expected.origin.cache.revision(),
                        worker_id: d.worker,
                        source: d.source,
                        entries: d.entries,
                    },
                }),
            )
            .map_err(io::Error::other)?;
        }
        let WorkbenchRequest::WorktreeOperation(request) =
            read_workbench_request(&mut stream).map_err(io::Error::other)?
        else {
            panic!("operation")
        };
        if index == 1 {
            assert!(matches!(
                request.action,
                WorkbenchWorktreeAction::Start { .. }
            ));
            return Ok(());
        }
        assert_eq!(
            request.action,
            WorkbenchWorktreeAction::Read {
                operation_id: expected.operation_id.clone()
            }
        );
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::WorktreeOperation(WorkbenchWorktreeResult {
                protocol_version: ProtocolVersion,
                request_id: request.request_id,
                operation_id: expected.operation_id.clone(),
                outcome: unknown(),
            }),
        )
        .map_err(io::Error::other)?;
        assert_eq!(stream.read(&mut [0])?, 0);
        Ok(())
    })
    .unwrap();
    let mut lane = lane(&server);
    assert!(lane.submit(job(Action::Start(Box::new(plan.clone())))));
    let Reply::Operation {
        exchange, result, ..
    } = take(&lane)
    else {
        panic!("start reply")
    };
    assert_eq!(exchange, Exchange::Start);
    assert!(result.unwrap_err());
    assert!(lane.submit(job(Action::Read(Box::new(plan)))));
    let Reply::Operation {
        exchange, result, ..
    } = take(&lane)
    else {
        panic!("read reply")
    };
    assert_eq!(exchange, Exchange::Read);
    assert_eq!(result.unwrap(), unknown());
    assert!(lane.close());
    server.finish().unwrap();
}
