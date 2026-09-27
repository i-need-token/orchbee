use super::*;
use crate::local_client::workbench::panel_relocate::{
    WorkbenchPanelRelocateRequest, WorkbenchPanelRelocateSubmission,
};
use orchbee_workbench_protocol::{WorkbenchPanelRelocateDestination, WorkbenchPanelTransfer};

fn request(destination: WorkbenchPanelRelocateDestination) -> WorkbenchCommand {
    workbench_command(
        FIRST_REQUEST_ID,
        7,
        WorkbenchAction::PanelRelocate {
            terminal_context_id: context_id(30),
            tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
            panel_id: WorkbenchPanelId::parse(&id(32)).unwrap(),
            destination,
        },
    )
}

fn outcome() -> WorkbenchCommandOutcome {
    WorkbenchCommandOutcome::PanelRelocatedToNewTab {
        revision: WorkbenchRevision::new(8),
        terminal_context_id: context_id(30),
        tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
        panel_id: WorkbenchPanelId::parse(&id(32)).unwrap(),
        new_tab_id: WorkbenchTabId::parse(&id(41)).unwrap(),
        transfer: Some(WorkbenchPanelTransfer {
            target_terminal_context_id: context_id(40),
            closed_source_tab: true,
            closed_source_workspace_id: Some(WorkbenchWorkspaceId::parse(&id(33)).unwrap()),
            created_workspace_id: None,
        }),
    }
}

fn exchange(
    request: WorkbenchCommand,
    response: WorkbenchCommandOutcome,
    known: bool,
) -> Result<WorkbenchPanelRelocateSubmission, Box<dyn std::error::Error>> {
    let expected = request.clone();
    let server = FakeServer::start(move |mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 61)?;
        assert_eq!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::Command(expected)
        );
        write_workbench_response(&mut stream, &command_response(FIRST_REQUEST_ID, response))
            .map_err(io::Error::other)?;
        if known {
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::SnapshotRead(snapshot_request(SECOND_REQUEST_ID))
            );
            write_workbench_response(
                &mut stream,
                &WorkbenchResponse::Snapshot(snapshot_result(SECOND_REQUEST_ID)),
            )
            .map_err(io::Error::other)
        } else {
            let mut byte = [0];
            assert_eq!(
                std::io::Read::read(&mut stream, &mut byte)?,
                0,
                "invalid relocation retires without another effect"
            );
            Ok(())
        }
    })?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let result = client.submit_workbench_panel_relocate(
        WorkbenchPanelRelocateRequest::try_from(request).unwrap(),
        client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
    );
    if known {
        assert!(client.generation().is_some());
        assert_eq!(
            client.submit_workbench_snapshot_read(snapshot_request(SECOND_REQUEST_ID))?,
            snapshot_result(SECOND_REQUEST_ID)
        );
    } else {
        assert!(client.generation().is_none());
    }
    drop(client);
    server.finish()?;
    Ok(result)
}

#[test]
fn relocation_transport_preserves_cross_context_and_new_workspace_effects() -> TestResult {
    for new_workspace in [false, true] {
        let destination = if new_workspace {
            WorkbenchPanelRelocateDestination::NewWorkspace {
                project_id: WorkbenchProjectId::parse(&id(21)).unwrap(),
                primary_directory_id: None,
                label: workbench_label(),
                tab_label: None,
            }
        } else {
            WorkbenchPanelRelocateDestination::NewTab {
                terminal_context_id: Some(context_id(40)),
                label: None,
            }
        };
        let mut response = outcome();
        let WorkbenchCommandOutcome::PanelRelocatedToNewTab {
            transfer: Some(transfer),
            ..
        } = &mut response
        else {
            unreachable!()
        };
        transfer.created_workspace_id =
            new_workspace.then(|| WorkbenchWorkspaceId::parse(&id(42)).unwrap());
        let expected = transfer.clone();
        let WorkbenchPanelRelocateSubmission::Committed(commit) =
            exchange(request(destination), response, true)?
        else {
            panic!("commit")
        };
        assert_eq!(commit.transfer, Some(expected));
        assert_eq!(commit.target_tab_id.as_str(), id(41));
    }
    Ok(())
}

#[test]
fn relocation_transport_retires_impossible_or_wrong_destination_receipts() -> TestResult {
    for defect in 0..6 {
        let destination = WorkbenchPanelRelocateDestination::NewTab {
            terminal_context_id: Some(context_id(40)),
            label: None,
        };
        let mut response = outcome();
        let WorkbenchCommandOutcome::PanelRelocatedToNewTab {
            transfer,
            new_tab_id,
            revision,
            ..
        } = &mut response
        else {
            unreachable!()
        };
        match defect {
            0 => transfer.as_mut().unwrap().target_terminal_context_id = context_id(99),
            1 => {
                transfer.as_mut().unwrap().created_workspace_id =
                    Some(WorkbenchWorkspaceId::parse(&id(42)).unwrap())
            }
            2 => transfer.as_mut().unwrap().closed_source_tab = false,
            3 => *transfer = None,
            4 => *new_tab_id = WorkbenchTabId::parse(&id(31)).unwrap(),
            _ => *revision = WorkbenchRevision::new(9),
        }
        assert!(matches!(
            exchange(request(destination), response, false)?,
            WorkbenchPanelRelocateSubmission::OutcomeUnknown { .. }
        ));
    }
    Ok(())
}
