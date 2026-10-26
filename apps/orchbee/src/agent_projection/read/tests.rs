//! Named-pipe witnesses prove that incomplete or drifting reads never establish a target set.
use super::*;
use crate::local_client::tests::server::FakeServer;
use orchbee_agent_observation_protocol::{
    AgentListOutcomeEnvelope, AgentRequest, AgentResult, read_agent_request, write_agent_result,
};
use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
use orchbee_local_ipc::LocalStream;
use orchbee_workbench_protocol::{
    PanelAgentAction, PanelAgentOutcome, PanelAgentResult, ProtocolVersion, TerminalAgentStateRow,
    WorkbenchRequest, WorkbenchResponse, WorkbenchSnapshotOutcome, WorkbenchSnapshotResult,
    read_workbench_request, write_workbench_response,
};
use std::{io, io::Read, num::NonZeroU64, time::Duration};

#[derive(Clone, Copy, Debug)]
enum Case {
    Stable,
    Denied,
    EmptyContinuation,
    Duplicate,
    PageDrift,
    PartialPhysical,
    CanonicalDrift,
    ReportDrift,
    WorkbenchDrift,
}
fn accept(stream: &mut LocalStream) -> io::Result<()> {
    assert!(matches!(
        read_message(stream).map_err(io::Error::other)?,
        LocalClientMessage::ClientHello { .. }
    ));
    write_message(
        stream,
        &LocalClientMessage::ClientAccepted {
            generation: NonZeroU64::new(1).unwrap(),
        },
    )
    .map_err(io::Error::other)
}
fn list(stream: &mut LocalStream, outcome: AgentListOutcome, continued: bool) -> io::Result<()> {
    let AgentRequest::Read(AgentReadRequest::List {
        request_id,
        include_ended,
        limit,
        cursor,
    }) = read_agent_request(stream).map_err(io::Error::other)?
    else {
        panic!("only canonical list reads are allowed");
    };
    assert!(!include_ended);
    assert_eq!(limit, AgentSafeInteger::new(100));
    assert_eq!(cursor.is_some(), continued);
    write_agent_result(
        stream,
        &AgentResult::Read(AgentReadResult::List {
            request_id,
            result: AgentListOutcomeEnvelope { outcome },
        }),
    )
    .map_err(io::Error::other)
}
fn listed(revision: u64, continued: bool, empty: bool) -> AgentListOutcome {
    AgentListOutcome::Listed {
        revision: AgentSafeInteger::new(revision),
        next_cursor: continued.then(|| AgentListCursor::parse(
            "v1.abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456.abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQ"
        ).unwrap()),
        items: if empty {
            Vec::new()
        } else {
            vec![crate::agent_projection::tests::summary("canonical", 12, "codex")]
        },
    }
}
fn states(stream: &mut LocalStream, case: Case, fence: bool) -> io::Result<()> {
    let WorkbenchRequest::PanelAgent(request) =
        read_workbench_request(stream).map_err(io::Error::other)?
    else {
        panic!("only physical state reads are allowed");
    };
    let PanelAgentAction::States { presentation_ids } = request.action else {
        panic!("no Agent effect is allowed");
    };
    assert_eq!(presentation_ids.len(), if fence { 0 } else { 4 });
    let mut presentations = presentation_ids
        .into_iter()
        .map(|id| {
            let n = u64::from_str_radix(id.as_str(), 16).unwrap();
            TerminalAgentStateRow {
                presentation_id: id,
                snapshot: Some(crate::agent_projection::tests::state(n, None)),
            }
        })
        .collect::<Vec<_>>();
    if matches!(case, Case::PartialPhysical) && !fence {
        presentations.pop();
    }
    write_workbench_response(
        stream,
        &WorkbenchResponse::PanelAgent(PanelAgentResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            session_id: request.session_id,
            outcome: PanelAgentOutcome::States {
                workbench_revision: WorkbenchRevision::new(1),
                report_revision: WorkbenchRevision::new(
                    if fence && matches!(case, Case::ReportDrift) {
                        8
                    } else {
                        7
                    },
                ),
                presentations,
            },
        }),
    )
    .map_err(io::Error::other)
}
fn peer(case: Case) -> io::Result<FakeServer> {
    FakeServer::start(move |mut stream| {
        accept(&mut stream)?;
        if matches!(case, Case::Denied) {
            return list(
                &mut stream,
                AgentListOutcome::Rejected {
                    reason: AgentListRejectionReason::ReadDenied,
                },
                false,
            );
        }
        if matches!(case, Case::EmptyContinuation) {
            return list(&mut stream, listed(2, true, true), false);
        }
        if matches!(case, Case::Duplicate | Case::PageDrift) {
            list(&mut stream, listed(2, true, false), false)?;
            return list(
                &mut stream,
                listed(
                    if matches!(case, Case::PageDrift) {
                        3
                    } else {
                        2
                    },
                    false,
                    false,
                ),
                true,
            );
        }
        list(&mut stream, listed(2, false, false), false)?;
        states(&mut stream, case, false)?;
        if matches!(case, Case::PartialPhysical) {
            return Ok(());
        }
        list(
            &mut stream,
            listed(
                if matches!(case, Case::CanonicalDrift) {
                    3
                } else {
                    2
                },
                false,
                false,
            ),
            false,
        )?;
        if matches!(case, Case::CanonicalDrift) {
            return Ok(());
        }
        states(&mut stream, case, true)?;
        if matches!(case, Case::ReportDrift) {
            return Ok(());
        }
        let WorkbenchRequest::SnapshotRead(request) =
            read_workbench_request(&mut stream).map_err(io::Error::other)?
        else {
            panic!("final topology fence");
        };
        let mut snapshot = machine_agents::tests::fixture();
        if matches!(case, Case::WorkbenchDrift) {
            snapshot.revision = WorkbenchRevision::new(2);
        }
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
                protocol_version: ProtocolVersion,
                request_id: request.request_id,
                outcome: WorkbenchSnapshotOutcome::Available { snapshot },
            }),
        )
        .map_err(io::Error::other)
    })
}
#[test]
fn complete_read_requires_both_sources_and_all_revision_fences() -> io::Result<()> {
    for (case, expected) in [
        (Case::Stable, None),
        (Case::Denied, Some(Error::Denied)),
        (Case::EmptyContinuation, Some(Error::Invalid)),
        (Case::Duplicate, Some(Error::Invalid)),
        (Case::PageDrift, Some(Error::Changed)),
        (Case::PartialPhysical, Some(Error::Unavailable)),
        (Case::CanonicalDrift, Some(Error::Changed)),
        (Case::ReportDrift, Some(Error::Changed)),
        (Case::WorkbenchDrift, Some(Error::Changed)),
    ] {
        let peer = peer(case)?;
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut client = LocalClient::connect_before(peer.endpoint(), "test-cohort", deadline)
            .map_err(io::Error::other)?;
        let result = read(
            &mut client,
            &machine_agents::tests::fixture(),
            false,
            deadline,
            &AtomicU8::new(0),
        );
        match expected {
            Some(error) => assert_eq!(result.unwrap_err(), error, "{case:?}"),
            None => {
                let checkpoint = result.unwrap();
                assert_eq!(checkpoint.rows.len(), 1);
                assert_eq!(checkpoint.rows[0].source(), "canonical");
                assert!(checkpoint.rows[0].placement.is_some());
            }
        }
        drop(client);
        peer.finish()?;
    }
    Ok(())
}
#[test]
fn expired_and_cancelled_reads_have_no_source_or_effect_requests() -> io::Result<()> {
    for cancelled in [false, true] {
        let peer = FakeServer::start(|mut stream| {
            accept(&mut stream)?;
            assert_eq!(stream.read(&mut [0])?, 0);
            Ok(())
        })?;
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut client = LocalClient::connect_before(peer.endpoint(), "test-cohort", deadline)
            .map_err(io::Error::other)?;
        let result = read(
            &mut client,
            &machine_agents::tests::fixture(),
            false,
            if cancelled { deadline } else { Instant::now() },
            &AtomicU8::new(u8::from(cancelled)),
        );
        assert_eq!(
            result.unwrap_err(),
            if cancelled {
                Error::Cancelled
            } else {
                Error::Timeout
            }
        );
        drop(client);
        peer.finish()?;
    }
    Ok(())
}

#[test]
fn cancellation_and_deadline_interrupt_an_inflight_source_without_retry() -> io::Result<()> {
    use std::sync::Arc;
    for cancelled in [false, true] {
        let cancel = Arc::new(AtomicU8::new(0));
        let server_cancel = Arc::clone(&cancel);
        let peer = FakeServer::start(move |mut stream| {
            accept(&mut stream)?;
            assert!(matches!(
                read_agent_request(&mut stream).map_err(io::Error::other)?,
                AgentRequest::Read(AgentReadRequest::List { .. })
            ));
            if cancelled {
                server_cancel.store(1, std::sync::atomic::Ordering::Release);
            }
            // No answer is sent. The original admission must close without a
            // second source request or any operation after interruption.
            assert_eq!(stream.read(&mut [0])?, 0);
            Ok(())
        })?;
        let admission = Instant::now() + Duration::from_secs(3);
        let mut client = LocalClient::connect_before(peer.endpoint(), "test-cohort", admission)
            .map_err(io::Error::other)?;
        let deadline = if cancelled {
            admission
        } else {
            Instant::now() + Duration::from_millis(100)
        };
        let result = read(
            &mut client,
            &machine_agents::tests::fixture(),
            false,
            deadline,
            &cancel,
        );
        assert_eq!(
            result.unwrap_err(),
            if cancelled {
                Error::Cancelled
            } else {
                Error::Timeout
            }
        );
        drop(client);
        peer.finish()?;
    }
    Ok(())
}
