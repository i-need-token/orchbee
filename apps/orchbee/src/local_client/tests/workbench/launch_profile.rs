use super::*;
use crate::local_client::workbench::launch_profile::{
    LaunchProfileCatalogReadError, LaunchProfileCatalogReadFailure,
    LaunchProfileOutcomeUnknownReason, WorkbenchLaunchProfileCreateRequest,
    WorkbenchLaunchProfileDeleteRequest, WorkbenchLaunchProfileMutationPrecommit,
    WorkbenchLaunchProfileMutationSubmission,
};
use orchbee_workbench_protocol::{
    LaunchProfileCatalogOutcome, LaunchProfileCatalogRevision, LaunchProfileEntry, LaunchProfileId,
    LaunchProfileName, LaunchProfileSelector, LaunchSelection, WorkbenchLaunchProfileCatalogRead,
    WorkbenchLaunchProfileCatalogResult,
};

fn create_command(request_id: &str, expected_revision: u64) -> WorkbenchCommand {
    workbench_command(
        request_id,
        expected_revision,
        WorkbenchAction::LaunchProfileCreate {
            name: LaunchProfileName::parse("dev-shell").unwrap(),
            launch: LaunchSelection::new("/bin/zsh".to_owned(), vec!["-f".to_owned()]).unwrap(),
        },
    )
}

fn delete_command(request_id: &str, expected_revision: u64) -> WorkbenchCommand {
    workbench_command(
        request_id,
        expected_revision,
        WorkbenchAction::LaunchProfileDelete {
            selector: LaunchProfileSelector::parse("dev-shell").unwrap(),
        },
    )
}

fn catalog_request(request_id: &str, selector: Option<&str>) -> WorkbenchLaunchProfileCatalogRead {
    WorkbenchLaunchProfileCatalogRead {
        protocol_version: ProtocolVersion,
        request_id: workbench_id(request_id),
        selector: selector.map(|value| LaunchProfileSelector::parse(value).unwrap()),
    }
}

fn catalog_response(request_id: &str, outcome: LaunchProfileCatalogOutcome) -> WorkbenchResponse {
    WorkbenchResponse::LaunchProfileCatalog(WorkbenchLaunchProfileCatalogResult {
        protocol_version: ProtocolVersion,
        request_id: workbench_id(request_id),
        outcome,
    })
}

fn profile_entry(seed: u128) -> LaunchProfileEntry {
    LaunchProfileEntry {
        profile_id: LaunchProfileId::parse(&format!("{}:{seed:x}", id(20))).unwrap(),
        name: LaunchProfileName::parse(&format!("p{seed}")).unwrap(),
        launch: LaunchSelection::new("/bin/zsh".to_owned(), Vec::new()).unwrap(),
    }
}

#[test]
fn opaque_profile_requests_reject_other_actions_unchanged() {
    assert!(
        WorkbenchLaunchProfileCreateRequest::try_from(create_command(FIRST_REQUEST_ID, 7)).is_ok()
    );
    assert!(
        WorkbenchLaunchProfileDeleteRequest::try_from(delete_command(FIRST_REQUEST_ID, 7)).is_ok()
    );
    for action in [
        WorkbenchAction::ProjectCreate {
            label: workbench_label(),
        },
        WorkbenchAction::TabRename {
            terminal_context_id: context_id(30),
            tab_id: WorkbenchTabId::parse(&id(31)).unwrap(),
            label: workbench_label(),
        },
        tab_create_command(FIRST_REQUEST_ID, 7).action,
        WorkbenchAction::LaunchProfileDelete {
            selector: LaunchProfileSelector::parse("x").unwrap(),
        },
    ] {
        let command = workbench_command(FIRST_REQUEST_ID, 7, action);
        assert_eq!(
            WorkbenchLaunchProfileCreateRequest::try_from(command.clone()).unwrap_err(),
            command
        );
    }
    for action in [
        WorkbenchAction::ProjectCreate {
            label: workbench_label(),
        },
        WorkbenchAction::LaunchProfileCreate {
            name: LaunchProfileName::parse("x").unwrap(),
            launch: LaunchSelection::new("/bin/sh".to_owned(), Vec::new()).unwrap(),
        },
    ] {
        let command = workbench_command(FIRST_REQUEST_ID, 7, action);
        assert_eq!(
            WorkbenchLaunchProfileDeleteRequest::try_from(command.clone()).unwrap_err(),
            command
        );
    }
}

type MutationCase = (
    WorkbenchCommandOutcome,
    Box<dyn Fn(&WorkbenchLaunchProfileMutationSubmission) -> bool + Send>,
);

#[test]
fn profile_mutations_preserve_committed_and_compatible_precommit_outcomes() -> TestResult {
    let profile_id = LaunchProfileId::parse(&format!("{}:1", id(20)))?;
    let cases: Vec<MutationCase> = vec![
        (
            WorkbenchCommandOutcome::LaunchProfileCreated {
                revision: WorkbenchRevision::new(8),
                catalog_revision: LaunchProfileCatalogRevision::new(1),
                profile_id: profile_id.clone(),
            },
            Box::new(move |submission| {
                matches!(
                    submission,
                    WorkbenchLaunchProfileMutationSubmission::Created {
                        revision,
                        catalog_revision,
                        profile_id: actual,
                    } if *revision == WorkbenchRevision::new(8)
                        && *catalog_revision == LaunchProfileCatalogRevision::new(1)
                        && actual == &profile_id
                )
            }),
        ),
        (
            WorkbenchCommandOutcome::LaunchProfileUnavailable {},
            Box::new(|submission| {
                matches!(
                    submission,
                    WorkbenchLaunchProfileMutationSubmission::ProfileUnavailable
                )
            }),
        ),
        (
            WorkbenchCommandOutcome::LaunchProfileAmbiguous {
                candidate_ids: vec![
                    LaunchProfileId::parse(&format!("{}:1", id(20)))?,
                    LaunchProfileId::parse(&format!("{}:2", id(20)))?,
                ],
            },
            Box::new(|submission| {
                matches!(
                    submission,
                    WorkbenchLaunchProfileMutationSubmission::ProfileAmbiguous {
                        candidate_ids,
                    } if candidate_ids.len() == 2
                )
            }),
        ),
        (
            WorkbenchCommandOutcome::Denied {},
            Box::new(|submission| {
                matches!(
                    submission,
                    WorkbenchLaunchProfileMutationSubmission::DefinitePrecommit(
                        WorkbenchLaunchProfileMutationPrecommit::Denied
                    )
                )
            }),
        ),
        (
            WorkbenchCommandOutcome::Rejected {
                revision: WorkbenchRevision::new(41),
                reason: WorkbenchRejectionReason::ResourceLimit,
            },
            Box::new(|submission| {
                matches!(
                    submission,
                    WorkbenchLaunchProfileMutationSubmission::DefinitePrecommit(
                        WorkbenchLaunchProfileMutationPrecommit::Rejected {
                            reason: WorkbenchRejectionReason::ResourceLimit,
                            ..
                        }
                    )
                )
            }),
        ),
        (
            WorkbenchCommandOutcome::Unavailable {
                reason: WorkbenchCommandUnavailableReason::RequestLedgerFull,
            },
            Box::new(|submission| {
                matches!(
                    submission,
                    WorkbenchLaunchProfileMutationSubmission::DefinitePrecommit(
                        WorkbenchLaunchProfileMutationPrecommit::Unavailable { .. }
                    )
                )
            }),
        ),
    ];
    // Drive each outcome through one fresh session so a retired connection
    // cannot mask the next classification.
    for (index, (outcome, matches_expected)) in cases.into_iter().enumerate() {
        let request_id = id(300 + index as u128);
        let server_request_id = request_id.clone();
        let server = FakeServer::start(move |mut stream| {
            expect_hello(&mut stream, COHORT)?;
            admit(&mut stream, 42)?;
            assert_eq!(
                read_workbench_request(&mut stream).map_err(io::Error::other)?,
                WorkbenchRequest::Command(create_command(&server_request_id, 7))
            );
            write_workbench_response(&mut stream, &command_response(&server_request_id, outcome))
                .map_err(io::Error::other)
        })?;
        let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
        let submission = client.submit_workbench_launch_profile_create(
            WorkbenchLaunchProfileCreateRequest::try_from(create_command(&request_id, 7))
                .expect("create fixture"),
            client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
        );
        assert!(matches_expected(&submission), "unexpected {submission:?}");
        drop(client);
        server.finish()?;
    }
    Ok(())
}

#[test]
fn profile_delete_commit_and_revision_mismatch_retire_the_session() -> TestResult {
    let profile_id = LaunchProfileId::parse(&format!("{}:9", id(20)))?;
    let server = FakeServer::start(move |mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 43)?;
        assert_eq!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::Command(delete_command(FIRST_REQUEST_ID, 7))
        );
        write_workbench_response(
            &mut stream,
            &command_response(
                FIRST_REQUEST_ID,
                WorkbenchCommandOutcome::LaunchProfileDeleted {
                    revision: WorkbenchRevision::new(8),
                    catalog_revision: LaunchProfileCatalogRevision::new(4),
                    profile_id: profile_id.clone(),
                },
            ),
        )
        .map_err(io::Error::other)?;
        // A committed revision that is not the expected successor retires the
        // session instead of presenting a possibly-misapplied effect.
        assert_eq!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::Command(delete_command(SECOND_REQUEST_ID, 7))
        );
        write_workbench_response(
            &mut stream,
            &command_response(
                SECOND_REQUEST_ID,
                WorkbenchCommandOutcome::LaunchProfileDeleted {
                    revision: WorkbenchRevision::new(9),
                    catalog_revision: LaunchProfileCatalogRevision::new(5),
                    profile_id: profile_id.clone(),
                },
            ),
        )
        .map_err(io::Error::other)
    })?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let deleted = client.submit_workbench_launch_profile_delete(
        WorkbenchLaunchProfileDeleteRequest::try_from(delete_command(FIRST_REQUEST_ID, 7))
            .expect("delete fixture"),
        client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
    );
    assert!(matches!(
        deleted,
        WorkbenchLaunchProfileMutationSubmission::Deleted {
            revision,
            catalog_revision,
            ..
        } if revision == WorkbenchRevision::new(8)
            && catalog_revision == LaunchProfileCatalogRevision::new(4)
    ));
    let unknown = client.submit_workbench_launch_profile_delete(
        WorkbenchLaunchProfileDeleteRequest::try_from(delete_command(SECOND_REQUEST_ID, 7))
            .expect("delete fixture"),
        client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
    );
    assert!(matches!(
        unknown,
        WorkbenchLaunchProfileMutationSubmission::OutcomeUnknown {
            reason: LaunchProfileOutcomeUnknownReason::RevisionMismatch { .. },
            ..
        }
    ));
    assert_eq!(client.generation(), None);
    drop(client);
    server.finish()?;
    Ok(())
}

#[test]
fn catalog_read_returns_every_typed_outcome_and_retires_on_incompatible_response() -> TestResult {
    let site_id = SiteId::parse(&id(20))?;
    let server = FakeServer::start(move |mut stream| {
        expect_hello(&mut stream, COHORT)?;
        admit(&mut stream, 44)?;
        let responses = [
            catalog_response(
                FIRST_REQUEST_ID,
                LaunchProfileCatalogOutcome::Catalog {
                    site_id: site_id.clone(),
                    catalog_revision: LaunchProfileCatalogRevision::new(2),
                    entries: vec![profile_entry(1), profile_entry(2)],
                },
            ),
            catalog_response(
                SECOND_REQUEST_ID,
                LaunchProfileCatalogOutcome::Ambiguous {
                    candidate_ids: vec![
                        LaunchProfileId::parse(&format!("{}:1", id(20)))
                            .map_err(io::Error::other)?,
                        LaunchProfileId::parse(&format!("{}:2", id(20)))
                            .map_err(io::Error::other)?,
                    ],
                },
            ),
            // An unrelated response shape must retire the session: the read's
            // answer cannot be trusted after a carrier-level confusion.
            command_response(&id(400), WorkbenchCommandOutcome::Denied {}),
        ];
        assert_eq!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::LaunchProfileCatalogRead(catalog_request(FIRST_REQUEST_ID, None))
        );
        write_workbench_response(&mut stream, &responses[0]).map_err(io::Error::other)?;
        assert_eq!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::LaunchProfileCatalogRead(catalog_request(
                SECOND_REQUEST_ID,
                Some("shell"),
            ))
        );
        write_workbench_response(&mut stream, &responses[1]).map_err(io::Error::other)?;
        assert_eq!(
            read_workbench_request(&mut stream).map_err(io::Error::other)?,
            WorkbenchRequest::LaunchProfileCatalogRead(catalog_request(&id(400), None))
        );
        write_workbench_response(&mut stream, &responses[2]).map_err(io::Error::other)
    })?;
    let mut client = LocalClient::connect(server.endpoint(), COHORT)?;
    let listed = client.submit_workbench_launch_profile_catalog_read_before(
        catalog_request(FIRST_REQUEST_ID, None),
        client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
    )?;
    assert!(matches!(
        listed.outcome,
        LaunchProfileCatalogOutcome::Catalog { ref entries, .. } if entries.len() == 2
    ));
    let ambiguous = client.submit_workbench_launch_profile_catalog_read_before(
        catalog_request(SECOND_REQUEST_ID, Some("shell")),
        client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
    )?;
    assert!(matches!(
        ambiguous.outcome,
        LaunchProfileCatalogOutcome::Ambiguous { ref candidate_ids }
            if candidate_ids.len() == 2
    ));
    let failed = client.submit_workbench_launch_profile_catalog_read_before(
        catalog_request(&id(400), None),
        client.operation_clock().now() + SERVER_GUARD_TIMEOUT,
    );
    assert!(matches!(
        failed,
        Err(LaunchProfileCatalogReadError::Failed {
            reason: LaunchProfileCatalogReadFailure::UnexpectedResponse,
            ..
        })
    ));
    assert_eq!(client.generation(), None);
    drop(client);
    server.finish()?;
    Ok(())
}
