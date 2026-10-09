use super::*;
use orchbee_workbench_protocol::{TabCreationPlacement, WorkbenchTabGroupId};

#[test]
fn placement_refusals_remain_definite_only_for_the_requested_placement() -> TestResult {
    for (placement, reason) in [
        (
            TabCreationPlacement::InGroup {
                group_id: WorkbenchTabGroupId::parse(&id(90))?,
            },
            WorkbenchRejectionReason::TabGroupUnavailable,
        ),
        (
            TabCreationPlacement::Duplicate {
                tab_id: WorkbenchTabId::parse(&id(91))?,
            },
            WorkbenchRejectionReason::TabUnavailable,
        ),
    ] {
        let mut command = tab_create_command(FIRST_REQUEST_ID, 7);
        let WorkbenchAction::TabCreate {
            placement: target, ..
        } = &mut command.action
        else {
            unreachable!()
        };
        *target = Some(placement);
        let expected = command.clone();
        let server = FakeServer::start(move |mut stream| {
            expect_hello(&mut stream, COHORT)?;
            admit(&mut stream, 40)?;
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::Command(expected)
            );
            write_workbench_response(
                &mut stream,
                &command_response(
                    FIRST_REQUEST_ID,
                    WorkbenchCommandOutcome::Rejected {
                        revision: WorkbenchRevision::new(7),
                        reason,
                    },
                ),
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
        let outcome = client.submit_workbench_tab_create(
            WorkbenchTabCreateRequest::try_from(command).unwrap(),
            client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
        );
        assert!(
            matches!(outcome, WorkbenchTabCreateSubmission::DefinitePrecommit(WorkbenchTabCreatePrecommit::Rejected { reason: actual, .. }) if actual == reason)
        );
        client.submit_workbench_snapshot_read(snapshot_request(SECOND_REQUEST_ID))?;
        drop(client);
        server.finish()?;
        let incompatible = invalid_typed_submission(
            tab_create_command(FIRST_REQUEST_ID, 7),
            command_response(
                FIRST_REQUEST_ID,
                WorkbenchCommandOutcome::Rejected {
                    revision: WorkbenchRevision::new(7),
                    reason,
                },
            ),
        )?;
        assert!(matches!(
            incompatible,
            WorkbenchTabCreateSubmission::OutcomeUnknown {
                reason: WorkbenchTabCreateOutcomeUnknownReason::ActionIncompatible,
                ..
            }
        ));
    }
    Ok(())
}
