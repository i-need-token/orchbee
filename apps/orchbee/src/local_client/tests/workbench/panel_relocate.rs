use super::*;
use crate::local_client::workbench::panel_relocate::{
    WorkbenchPanelRelocateRequest, WorkbenchPanelRelocateSubmission,
};
use orchbee_workbench_protocol::{
    WorkbenchDirectoryId, WorkbenchDirectoryPath, WorkbenchPanelRelocateDestination,
    WorkbenchPanelTransfer,
};

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
            created_directory_id: None,
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
    for case in 0..3 {
        let new_workspace = case != 0;
        let new_directory = case == 2;
        let destination = if new_workspace {
            WorkbenchPanelRelocateDestination::NewWorkspace {
                project_id: WorkbenchProjectId::parse(&id(21)).unwrap(),
                primary_directory_id: None,
                primary_directory_path: new_directory
                    .then(|| WorkbenchDirectoryPath::parse("C:/retained/source").unwrap()),
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
        transfer.created_directory_id =
            new_directory.then(|| WorkbenchDirectoryId::parse(&id(43)).unwrap());
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
    for defect in 0..7 {
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
            5 => *revision = WorkbenchRevision::new(9),
            _ => {
                transfer.as_mut().unwrap().created_directory_id =
                    Some(WorkbenchDirectoryId::parse(&id(43)).unwrap());
            }
        }
        assert!(matches!(
            exchange(request(destination), response, false)?,
            WorkbenchPanelRelocateSubmission::OutcomeUnknown { .. }
        ));
    }
    Ok(())
}

#[test]
fn relocation_directory_failure_is_precommit_and_missing_creation_is_unknown() -> TestResult {
    let destination = WorkbenchPanelRelocateDestination::NewWorkspace {
        project_id: WorkbenchProjectId::parse(&id(21)).unwrap(),
        primary_directory_id: None,
        primary_directory_path: Some(WorkbenchDirectoryPath::parse("C:/retained/source").unwrap()),
        label: workbench_label(),
        tab_label: None,
    };
    for reason in [
        WorkbenchCommandUnavailableReason::DirectoryUnavailable,
        WorkbenchCommandUnavailableReason::NoCompatibleWorker,
        WorkbenchCommandUnavailableReason::ServerBusy,
    ] {
        assert!(matches!(
            exchange(
                request(destination.clone()),
                WorkbenchCommandOutcome::Unavailable { reason },
                true,
            )?,
            WorkbenchPanelRelocateSubmission::DefinitePrecommit(_)
        ));
    }
    let mut response = outcome();
    let WorkbenchCommandOutcome::PanelRelocatedToNewTab {
        transfer: Some(transfer),
        ..
    } = &mut response
    else {
        unreachable!()
    };
    transfer.created_workspace_id = Some(WorkbenchWorkspaceId::parse(&id(42)).unwrap());
    assert!(matches!(
        exchange(request(destination), response, false)?,
        WorkbenchPanelRelocateSubmission::OutcomeUnknown { .. }
    ));
    Ok(())
}

#[test]
fn panel_relocation_wake_after_submission_retires_unknown_without_replay() -> TestResult {
    let (submitted, waiting) = std::sync::mpsc::channel();
    let (wake_read, mut wake_write) = UnixStream::pair()?;
    let server = FakeServer::start(move |mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 51)?;
        let WorkbenchRequest::Command(command) =
            read_workbench_request(&mut stream).map_err(io::Error::other)?
        else {
            panic!("command")
        };
        assert!(matches!(
            command.action,
            WorkbenchAction::PanelRelocate { .. }
        ));
        submitted.send(()).map_err(io::Error::other)?;
        assert_eq!(
            stream.read(&mut [0])?,
            0,
            "cancelled transfer must not replay"
        );
        Ok(())
    })?;
    let wake = std::thread::spawn(move || -> io::Result<()> {
        waiting
            .recv_timeout(Duration::from_secs(5))
            .map_err(io::Error::other)?;
        wake_write.write_all(b"x")
    });
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let command = request(WorkbenchPanelRelocateDestination::NewTab {
        terminal_context_id: Some(context_id(40)),
        label: None,
    });
    let answer = client.submit_workbench_panel_relocate_with_wake(
        WorkbenchPanelRelocateRequest::try_from(command).unwrap(),
        client.operation_clock().now() + Duration::from_secs(5),
        Some(wake_read.as_local_io()),
    );
    assert!(matches!(
        answer,
        WorkbenchPanelRelocateSubmission::OutcomeUnknown { .. }
    ));
    assert!(client.generation().is_none());
    wake.join().unwrap()?;
    drop(client);
    server.finish()?;
    Ok(())
}
