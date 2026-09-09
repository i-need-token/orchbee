use super::*;
use crate::local_client::workbench::panel_split::{
    WorkbenchPanelSplitCommit, WorkbenchPanelSplitRequest, WorkbenchPanelSplitSubmission,
};

#[test]
fn panel_split_preserves_pending_and_created_commit_metadata() -> TestResult {
    for outcome in [
        WorkbenchCommandOutcome::PanelSplitPending {
            revision: WorkbenchRevision::new(8),
            terminal_context_id: context_id(30),
            tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
            panel_id: WorkbenchPanelId::parse(&id(32)).unwrap(),
            new_panel_id: WorkbenchPanelId::parse(&id(33)).unwrap(),
            split_id: orchbee_workbench_protocol::WorkbenchSplitId::parse(&id(34)).unwrap(),
            first_slot_id: orchbee_workbench_protocol::WorkbenchPanelSlotId::parse(&id(35))
                .unwrap(),
            second_slot_id: orchbee_workbench_protocol::WorkbenchPanelSlotId::parse(&id(36))
                .unwrap(),
            presentation_id: TerminalPresentationId::parse(&id(37)).unwrap(),
        },
        WorkbenchCommandOutcome::PanelSplitCreated {
            revision: WorkbenchRevision::new(8),
            terminal_context_id: context_id(30),
            tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
            panel_id: WorkbenchPanelId::parse(&id(32)).unwrap(),
            new_panel_id: WorkbenchPanelId::parse(&id(33)).unwrap(),
            split_id: orchbee_workbench_protocol::WorkbenchSplitId::parse(&id(34)).unwrap(),
            first_slot_id: orchbee_workbench_protocol::WorkbenchPanelSlotId::parse(&id(35))
                .unwrap(),
            second_slot_id: orchbee_workbench_protocol::WorkbenchPanelSlotId::parse(&id(36))
                .unwrap(),
            presentation_id: TerminalPresentationId::parse(&id(37)).unwrap(),
            terminal_outcome: WorkbenchTerminalOutcome::Started {
                geometry: terminal_geometry(),
            },
        },
    ] {
        let server_outcome = outcome.clone();
        let server = FakeServer::start(move |mut stream| {
            expect_hello(&mut stream, COHORT)?;
            admit(&mut stream, 61)?;
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::Command(panel_split_command(7))
            );
            write_workbench_response(
                &mut stream,
                &command_response(FIRST_REQUEST_ID, server_outcome),
            )
            .map_err(io::Error::other)?;
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::SnapshotRead(snapshot_request(SECOND_REQUEST_ID))
            );
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::Snapshot(snapshot_result(SECOND_REQUEST_ID)),
            )
            .map_err(io::Error::other)
        })?;
        let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
        let actual = client.submit_workbench_panel_split(
            WorkbenchPanelSplitRequest::try_from(panel_split_command(7)).unwrap(),
            client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
        );
        match (outcome, actual) {
            (
                WorkbenchCommandOutcome::PanelSplitPending { .. },
                WorkbenchPanelSplitSubmission::Committed(WorkbenchPanelSplitCommit::Pending {
                    revision,
                    terminal_context_id,
                    tab_id,
                    panel_id,
                    new_panel_id,
                    split_id,
                    first_slot_id,
                    second_slot_id,
                    presentation_id,
                }),
            ) => assert_eq!(
                (
                    revision,
                    terminal_context_id,
                    tab_id,
                    panel_id,
                    new_panel_id,
                    split_id,
                    first_slot_id,
                    second_slot_id,
                    presentation_id,
                ),
                (
                    WorkbenchRevision::new(8),
                    context_id(30),
                    WorkbenchTabId::parse(&id(31)).unwrap(),
                    WorkbenchPanelId::parse(&id(32)).unwrap(),
                    WorkbenchPanelId::parse(&id(33)).unwrap(),
                    orchbee_workbench_protocol::WorkbenchSplitId::parse(&id(34)).unwrap(),
                    orchbee_workbench_protocol::WorkbenchPanelSlotId::parse(&id(35)).unwrap(),
                    orchbee_workbench_protocol::WorkbenchPanelSlotId::parse(&id(36)).unwrap(),
                    TerminalPresentationId::parse(&id(37)).unwrap(),
                )
            ),
            (
                WorkbenchCommandOutcome::PanelSplitCreated { .. },
                WorkbenchPanelSplitSubmission::Committed(WorkbenchPanelSplitCommit::Created {
                    revision,
                    terminal_outcome: WorkbenchTerminalOutcome::Started { geometry },
                    ..
                }),
            ) => {
                assert_eq!(revision, WorkbenchRevision::new(8));
                assert_eq!(geometry, terminal_geometry());
            }
            other => panic!("unexpected panel.split classification {other:?}"),
        }
        assert_eq!(client.generation().map(NonZeroU64::get), Some(61));
        assert_eq!(
            client.submit_workbench_snapshot_read(snapshot_request(SECOND_REQUEST_ID))?,
            snapshot_result(SECOND_REQUEST_ID)
        );
        drop(client);
        server.finish()?;
    }
    Ok(())
}

fn panel_split_command(expected_revision: u64) -> WorkbenchCommand {
    workbench_command(
        FIRST_REQUEST_ID,
        expected_revision,
        WorkbenchAction::PanelSplit {
            terminal_context_id: context_id(30),
            tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
            panel_id: WorkbenchPanelId::parse(&id(32)).unwrap(),
            direction: orchbee_workbench_protocol::WorkbenchPanelSplitDirection::Right,
            label: workbench_label(),
            geometry: terminal_geometry(),
        },
    )
}
