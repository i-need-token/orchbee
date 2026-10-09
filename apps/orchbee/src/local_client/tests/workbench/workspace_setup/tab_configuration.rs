use super::*;
use orchbee_workbench_protocol::{TabConfiguration, TabGroupColor, WorkbenchTabGroupId};

fn exchange(
    create: bool,
    outcome: WorkbenchCommandOutcome,
) -> Result<WorkbenchSetupSubmission, Box<dyn std::error::Error>> {
    let operation = if create {
        TabConfiguration::CreateGroup {
            tab_id: WorkbenchTabId::parse(&id(31))?,
            label: workbench_label(),
            color: TabGroupColor::Green,
        }
    } else {
        TabConfiguration::SetPinned {
            tab_id: WorkbenchTabId::parse(&id(31))?,
            pinned: true,
        }
    };
    let command = workbench_command(
        FIRST_REQUEST_ID,
        7,
        WorkbenchAction::TabsConfigure {
            terminal_context_id: context_id(30),
            operation,
        },
    );
    let captured = command.clone();
    let server = FakeServer::start(move |mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 61)?;
        assert_eq!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::Command(captured)
        );
        write_workbench_response(&mut stream, &command_response(FIRST_REQUEST_ID, outcome))
            .map_err(io::Error::other)
    })?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let submission = client.submit_workbench_setup(
        WorkbenchSetupRequest::try_from(command).unwrap(),
        client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
    );
    if matches!(submission, WorkbenchSetupSubmission::OutcomeUnknown { .. }) {
        assert!(client.generation().is_none());
    }
    drop(client);
    server.finish()?;
    Ok(submission)
}

#[test]
fn tab_configuration_wire_receipts_keep_group_identity_and_reject_incompatible_results()
-> TestResult {
    let group_id = WorkbenchTabGroupId::parse(&id(41))?;
    for create in [false, true] {
        let returned = create.then_some(group_id.clone());
        let WorkbenchSetupSubmission::Committed(commit) = exchange(
            create,
            WorkbenchCommandOutcome::TabsConfigured {
                revision: WorkbenchRevision::new(8),
                group_id: returned.clone(),
            },
        )?
        else {
            panic!("known commit")
        };
        assert_eq!(
            commit,
            WorkbenchSetupCommit::TabsConfigured {
                revision: WorkbenchRevision::new(8),
                group_id: returned,
                unchanged: false
            }
        );
        for (revision, returned) in [
            (8, (!create).then_some(group_id.clone())),
            (7, create.then_some(group_id.clone())),
            (9, create.then_some(group_id.clone())),
        ] {
            assert!(matches!(
                exchange(
                    create,
                    WorkbenchCommandOutcome::TabsConfigured {
                        revision: WorkbenchRevision::new(revision),
                        group_id: returned
                    }
                )?,
                WorkbenchSetupSubmission::OutcomeUnknown { .. }
            ));
        }
    }
    assert!(matches!(
        exchange(
            true,
            WorkbenchCommandOutcome::Unchanged {
                revision: WorkbenchRevision::new(7)
            }
        )?,
        WorkbenchSetupSubmission::OutcomeUnknown { .. }
    ));
    assert!(matches!(
        exchange(
            false,
            WorkbenchCommandOutcome::Unchanged {
                revision: WorkbenchRevision::new(7)
            }
        )?,
        WorkbenchSetupSubmission::Committed(WorkbenchSetupCommit::TabsConfigured {
            unchanged: true,
            ..
        })
    ));
    assert!(matches!(
        exchange(
            true,
            WorkbenchCommandOutcome::Unavailable {
                reason: WorkbenchCommandUnavailableReason::TerminalBusy
            }
        )?,
        WorkbenchSetupSubmission::DefinitePrecommit(_)
    ));
    Ok(())
}
