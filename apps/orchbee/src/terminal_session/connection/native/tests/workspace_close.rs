//! Actual framed transport, mailbox evidence and NativeUi; no scripted UI verdicts.
use super::super::NativeExecutor;
use super::setup::{TestResult, admit};
use crate::{
    local_client::{LocalClient, test_server::FakeServer},
    terminal_session::{
        mailbox::QueuedCommand,
        state::{
            CarrierFailure,
            workspace_close::{WorkspaceCloseEvidence, WorkspaceCloseResult},
        },
        ui::workspace_close_tests::{assert_repair, ready, receipt, submit},
    },
};
use orchbee_workbench_protocol::*;
use std::{
    io::{self, Read},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Cut {
    StaleSource,
    UnknownStart,
    PendingReadLost,
    ClosedSnapshotLost,
    UiDeliveryLost,
    Complete,
}
fn exercise(cut: Cut) -> TestResult {
    let (mut ui, mailbox, _, snapshot) = ready();
    let lease = submit(&mut ui, &mailbox);
    let QueuedCommand::Refresh(marker) = lease.record() else {
        panic!("one Close lease");
    };
    let marker = marker.clone();
    let operation = marker.workspace_close().unwrap().clone();
    let expected = operation.clone();
    let server = FakeServer::start(move |mut stream| {
        admit(&mut stream)?;
        let WorkbenchRequest::SnapshotRead(read) =
            read_workbench_request(&mut stream).map_err(io::Error::other)?
        else {
            return Err(io::Error::other("fresh source check must precede mutation"));
        };
        let mut fresh = snapshot.clone();
        if cut == Cut::StaleSource {
            fresh.revision = WorkbenchRevision::new(2);
        }
        write_workbench_response(
            &mut stream,
            &WorkbenchResponse::Snapshot(WorkbenchSnapshotResult {
                protocol_version: ProtocolVersion,
                request_id: read.request_id,
                outcome: WorkbenchSnapshotOutcome::Available { snapshot: fresh },
            }),
        )
        .map_err(io::Error::other)?;
        if cut != Cut::StaleSource {
            let WorkbenchRequest::WorkspaceGroupClose(start) =
                read_workbench_request(&mut stream).map_err(io::Error::other)?
            else {
                return Err(io::Error::other("exact group Start expected"));
            };
            assert_eq!(start, expected.request);
            if cut == Cut::UnknownStart {
                return Ok(());
            }
            let mut known = receipt(&expected, cut == Cut::ClosedSnapshotLost);
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::WorkspaceGroupClose(known.clone()),
            )
            .map_err(io::Error::other)?;
            if cut != Cut::ClosedSnapshotLost {
                let WorkbenchRequest::WorkspaceGroupClose(read) =
                    read_workbench_request(&mut stream).map_err(io::Error::other)?
                else {
                    return Err(io::Error::other(
                        "only read-only observation may follow Start",
                    ));
                };
                assert_eq!(
                    read.action,
                    WorkspaceGroupCloseAction::Read {
                        operation_id: start.action.operation_id().clone()
                    }
                );
                assert_ne!(read.request_id, start.request_id);
                if cut == Cut::PendingReadLost {
                    return Ok(());
                }
                known = receipt(&expected, true);
                known.request_id = read.request_id;
                write_workbench_response(
                    &mut stream,
                    &WorkbenchResponse::WorkspaceGroupClose(known),
                )
                .map_err(io::Error::other)?;
            }
            let WorkbenchRequest::PublicationRead(read) =
                read_workbench_request(&mut stream).map_err(io::Error::other)?
            else {
                return Err(io::Error::other(
                    "canonical repair must follow retained Closed",
                ));
            };
            assert_eq!(read.known_revision, WorkbenchRevision::new(1));
            if cut == Cut::ClosedSnapshotLost {
                return Ok(());
            }
            let mut removed = snapshot;
            removed.revision = WorkbenchRevision::new(2);
            removed.navigation.projects[0].workspaces.clear();
            removed.terminal_contexts.pop();
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::Publication(WorkbenchPublicationResult {
                    protocol_version: ProtocolVersion,
                    request_id: read.request_id,
                    outcome: WorkbenchPublicationOutcome::Published {
                        publications: vec![WorkbenchPublicationDocument::Snapshot(removed)],
                    },
                }),
            )
            .map_err(io::Error::other)?;
        }
        let mut trailing = [0];
        assert_eq!(
            stream.read(&mut trailing)?,
            0,
            "no replay, Git mutation or terminal effect on this transport"
        );
        Ok(())
    })?;
    let client = LocalClient::connect(server.endpoint(), "native-close-test")?;
    let worker_mailbox = Arc::clone(&mailbox);
    let worker = thread::spawn(move || {
        let mut executor = NativeExecutor::new(client, Arc::clone(&worker_mailbox));
        let result = executor.run_refresh(&lease, marker);
        let evidence = executor.pending_workspace_close.take();
        if result.is_ok() {
            worker_mailbox.finish_lease(lease).unwrap();
        }
        (result, evidence)
    });
    let deadline = Instant::now() + Duration::from_secs(3);
    while !worker.is_finished() && Instant::now() < deadline {
        if cut == Cut::UiDeliveryLost {
            if mailbox.cells().0 == "event" {
                mailbox.close(CarrierFailure::Transport);
            }
        } else {
            ui.tick();
        }
        thread::sleep(Duration::from_millis(1));
    }
    if !worker.is_finished() {
        mailbox.close(CarrierFailure::Deadline);
    }
    let (result, evidence) = worker.join().unwrap();
    server.finish()?;
    if matches!(cut, Cut::Complete | Cut::StaleSource) {
        assert_eq!(result, Ok(()), "{cut:?}");
        assert!(evidence.is_none());
        let status = ui.view().workspace_close_status().unwrap();
        if cut == Cut::Complete {
            assert_eq!(
                status.status(),
                crate::terminal_session::WorkspaceCloseStatus::Closed
            );
        } else {
            assert_eq!(
                status.status(),
                crate::terminal_session::WorkspaceCloseStatus::Refused
            );
        }
        assert_repair(&ui, cut == Cut::Complete);
        assert!(mailbox.workspace_close_worker_loss().is_none());
    } else {
        assert!(result.is_err());
        let evidence = evidence.expect("known facts or unknown identity survive worker exit");
        assert_eq!(
            mailbox.workspace_close_worker_loss(),
            Some(evidence.clone())
        );
        match cut {
            Cut::UnknownStart => assert_eq!(evidence.result, WorkspaceCloseResult::Unknown),
            Cut::PendingReadLost => assert!(evidence.result.is_pending()),
            Cut::ClosedSnapshotLost | Cut::UiDeliveryLost => assert!(evidence.result.is_closed()),
            _ => unreachable!(),
        }
        assert!(
            ui.finalize_workspace_close_worker(Some(WorkspaceCloseEvidence {
                operation: operation.clone(),
                ..evidence
            }))
        );
        assert_eq!(
            ui.view().workspace_close_status().unwrap().operation_id(),
            Some(operation.request.action.operation_id())
        );
        let cursors = mailbox.cursors();
        ui.apply_client_action(crate::client_view::ClientAction::RequestWorkspaceClose);
        assert_eq!(mailbox.cursors(), cursors);
        if cut != Cut::UiDeliveryLost {
            assert!(mailbox.try_recv().unwrap().is_none());
        }
    }
    Ok(())
}
#[test]
fn workspace_close_carrier_fresh_source_refuses_drift_without_start() -> TestResult {
    exercise(Cut::StaleSource)
}
#[test]
fn workspace_close_carrier_lost_start_keeps_unknown_identity_without_replay() -> TestResult {
    exercise(Cut::UnknownStart)
}
#[test]
fn workspace_close_carrier_lost_read_preserves_pending_child_identity() -> TestResult {
    exercise(Cut::PendingReadLost)
}
#[test]
fn workspace_close_carrier_closed_receipt_survives_failed_snapshot() -> TestResult {
    exercise(Cut::ClosedSnapshotLost)
}
#[test]
fn workspace_close_carrier_closed_receipt_survives_cancelled_ui_delivery() -> TestResult {
    exercise(Cut::UiDeliveryLost)
}
#[test]
fn workspace_close_carrier_observes_same_operation_and_native_ui_lands_on_standalone() -> TestResult
{
    exercise(Cut::Complete)
}
