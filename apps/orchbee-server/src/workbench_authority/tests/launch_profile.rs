use super::*;
use orchbee_workbench_protocol::{
    LaunchProfileCatalogOutcome, LaunchProfileCatalogRevision, LaunchProfileId, LaunchProfileName,
    LaunchProfileSelector, LaunchSelection, MAXIMUM_LAUNCH_PROFILE_ENTRIES,
    WorkbenchLaunchProfileCatalogRead, WorkbenchLaunchProfileCatalogResult,
};

fn launch(exec: &str, args: &[&str]) -> LaunchSelection {
    LaunchSelection::new(
        exec.to_owned(),
        args.iter().map(|arg| (*arg).to_owned()).collect(),
    )
    .unwrap()
}

fn create_action(name: &str, launch: LaunchSelection) -> WorkbenchAction {
    WorkbenchAction::LaunchProfileCreate {
        name: LaunchProfileName::parse(name).unwrap(),
        launch,
    }
}

fn delete_action(selector: &str) -> WorkbenchAction {
    WorkbenchAction::LaunchProfileDelete {
        selector: LaunchProfileSelector::parse(selector).unwrap(),
    }
}

fn catalog_read(id: u128, selector: Option<&str>) -> WorkbenchRequest {
    WorkbenchRequest::LaunchProfileCatalogRead(WorkbenchLaunchProfileCatalogRead {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&identity(id)).unwrap(),
        selector: selector.map(|value| LaunchProfileSelector::parse(value).unwrap()),
    })
}

fn catalog_outcome(
    authority: &mut WorkbenchAuthority,
    grant: Option<&WorkbenchGrant>,
    request: WorkbenchRequest,
) -> LaunchProfileCatalogOutcome {
    let WorkbenchResponse::LaunchProfileCatalog(WorkbenchLaunchProfileCatalogResult {
        outcome,
        ..
    }) = authority.execute(grant, request.try_into().unwrap())
    else {
        panic!("expected launch-profile catalog result")
    };
    outcome
}

fn command_at(
    session: &SessionId,
    id: u128,
    revision: u64,
    action: WorkbenchAction,
) -> WorkbenchCommand {
    WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&identity(id)).unwrap(),
        session_id: session.clone(),
        expected_revision: WorkbenchRevision::new(revision),
        action,
    }
}

fn created_id(result: &WorkbenchCommandResult) -> LaunchProfileId {
    let WorkbenchCommandOutcome::LaunchProfileCreated { profile_id, .. } = &result.outcome else {
        panic!("expected launch-profile created outcome")
    };
    profile_id.clone()
}

fn create_profile(
    authority: &mut WorkbenchAuthority,
    grant: &WorkbenchGrant,
    request: u128,
    revision: u64,
    name: &str,
    launch: LaunchSelection,
) -> WorkbenchCommandResult {
    submit(
        authority,
        grant,
        command(request, revision, create_action(name, launch)),
    )
}

#[test]
fn catalog_create_mints_session_scoped_ids_and_advances_both_revisions() {
    let (mut authority, grant) = authority();
    let session_hex = authority.session_id().as_str().to_owned();
    let cursor = authority.last_identity;

    let first = create_profile(
        &mut authority,
        &grant,
        1,
        0,
        "build",
        launch("/bin/echo", &[]),
    );
    let WorkbenchCommandOutcome::LaunchProfileCreated {
        revision,
        catalog_revision,
        profile_id,
    } = &first.outcome
    else {
        panic!("expected created outcome")
    };
    assert_eq!(*revision, WorkbenchRevision::new(1));
    assert_eq!(*catalog_revision, LaunchProfileCatalogRevision::new(1));
    // <sessionHex>:<counterHex> — the Session prefix is what makes a dead Session's ids
    // unable to alias another Session's catalog. The counter shares the
    // authority identity cursor, so the first profile id mints its next value.
    assert_eq!(
        profile_id.as_str(),
        format!("{session_hex}:{:032x}", cursor + 1)
    );

    let second = create_profile(
        &mut authority,
        &grant,
        2,
        1,
        "build",
        launch("/bin/echo", &["--fast"]),
    );
    let WorkbenchCommandOutcome::LaunchProfileCreated {
        revision: second_revision,
        catalog_revision: second_catalog_revision,
        profile_id: second_id,
    } = &second.outcome
    else {
        panic!("expected created outcome")
    };
    assert_eq!(*second_revision, WorkbenchRevision::new(2));
    assert_eq!(
        *second_catalog_revision,
        LaunchProfileCatalogRevision::new(2)
    );
    assert_eq!(
        second_id.as_str(),
        format!("{session_hex}:{:032x}", cursor + 2)
    );

    // The catalog is private Server state: the public snapshot carries only the
    // advanced revision, never catalog contents.
    let snapshot = snapshot(&mut authority, &grant);
    assert_eq!(snapshot.revision, WorkbenchRevision::new(2));

    let WorkbenchCommandOutcome::LaunchProfileDeleted {
        revision: delete_revision,
        catalog_revision: delete_catalog_revision,
        profile_id: deleted_id,
    } = submit(
        &mut authority,
        &grant,
        command(3, 2, delete_action(profile_id.as_str())),
    )
    .outcome
    else {
        panic!("expected deleted outcome")
    };
    assert_eq!(delete_revision, WorkbenchRevision::new(3));
    assert_eq!(
        delete_catalog_revision,
        LaunchProfileCatalogRevision::new(3)
    );
    assert_eq!(deleted_id, *profile_id);
}

#[test]
fn catalog_read_lists_gets_and_resolves_id_first_then_name_cardinality() {
    let (mut authority, grant) = authority();
    let reader = WorkbenchGrant::read_only(grant.session_id.clone());

    assert_eq!(
        catalog_outcome(&mut authority, Some(&reader), catalog_read(100, None)),
        LaunchProfileCatalogOutcome::Catalog {
            session_id: grant.session_id.clone(),
            catalog_revision: LaunchProfileCatalogRevision::new(0),
            entries: Vec::new(),
        }
    );

    let first = created_id(&create_profile(
        &mut authority,
        &grant,
        1,
        0,
        "shell",
        launch("/bin/zsh", &[]),
    ));
    let second = created_id(&create_profile(
        &mut authority,
        &grant,
        2,
        1,
        "shell",
        launch("/bin/bash", &["--norc"]),
    ));
    let third = created_id(&create_profile(
        &mut authority,
        &grant,
        3,
        2,
        "build",
        launch("/usr/bin/make", &["-j8"]),
    ));

    let LaunchProfileCatalogOutcome::Catalog {
        catalog_revision,
        entries,
        ..
    } = catalog_outcome(&mut authority, Some(&reader), catalog_read(101, None))
    else {
        panic!("expected full catalog")
    };
    assert_eq!(catalog_revision, LaunchProfileCatalogRevision::new(3));
    assert_eq!(entries.len(), 3);

    // Exact profile-id match resolves before any name lookup runs.
    let LaunchProfileCatalogOutcome::Profile { profile, .. } = catalog_outcome(
        &mut authority,
        Some(&reader),
        catalog_read(102, Some(first.as_str())),
    ) else {
        panic!("expected single profile")
    };
    assert_eq!(profile.profile_id, first);
    assert_eq!(profile.launch.executable(), "/bin/zsh");

    // A unique name resolves to its entry; a zero-cardinality name is not-found.
    let LaunchProfileCatalogOutcome::Profile { profile, .. } = catalog_outcome(
        &mut authority,
        Some(&reader),
        catalog_read(103, Some("build")),
    ) else {
        panic!("expected unique name resolution")
    };
    assert_eq!(profile.profile_id, third);
    assert_eq!(
        catalog_outcome(
            &mut authority,
            Some(&reader),
            catalog_read(104, Some("absent"))
        ),
        LaunchProfileCatalogOutcome::NotFound {}
    );

    // Name cardinality above one resolves ambiguous with the same-resolution
    // candidate id list — never an exec/argv summary.
    assert_eq!(
        catalog_outcome(
            &mut authority,
            Some(&reader),
            catalog_read(105, Some("shell"))
        ),
        LaunchProfileCatalogOutcome::Ambiguous {
            candidate_ids: vec![first.clone(), second.clone()],
        }
    );

    // Id-first wins even when another entry's *name* equals the selector: the
    // fourth profile is named after the first profile's id string.
    let fourth = created_id(&create_profile(
        &mut authority,
        &grant,
        4,
        3,
        first.as_str(),
        launch("/bin/false", &[]),
    ));
    let LaunchProfileCatalogOutcome::Profile { profile, .. } = catalog_outcome(
        &mut authority,
        Some(&reader),
        catalog_read(106, Some(first.as_str())),
    ) else {
        panic!("id-first resolution expected")
    };
    assert_eq!(profile.profile_id, first);
    assert_ne!(profile.profile_id, fourth);
}

#[test]
fn catalog_reads_deny_without_session_identity_and_never_touch_the_ledger() {
    let (mut authority, grant) = authority();
    create_profile(
        &mut authority,
        &grant,
        1,
        0,
        "shell",
        launch("/bin/zsh", &[]),
    );
    let reader = WorkbenchGrant::read_only(grant.session_id.clone());
    let wrong_session =
        WorkbenchGrant::manage_navigation(SessionId::parse(&identity(200)).unwrap());

    for denied in [None, Some(&wrong_session)] {
        assert_eq!(
            catalog_outcome(&mut authority, denied, catalog_read(200, None)),
            LaunchProfileCatalogOutcome::Denied {}
        );
        assert_eq!(
            catalog_outcome(&mut authority, denied, catalog_read(201, Some("shell"))),
            LaunchProfileCatalogOutcome::Denied {}
        );
    }
    // A read-only grant observes the catalog; reads never enter the command
    // ledger, so a later command may reuse the request identity.
    assert!(matches!(
        catalog_outcome(&mut authority, Some(&reader), catalog_read(202, None)),
        LaunchProfileCatalogOutcome::Catalog { .. }
    ));
    assert_eq!(authority.ledger.len(), 1);
    assert!(matches!(
        submit(&mut authority, &grant, command(200, 1, create("after"))).outcome,
        WorkbenchCommandOutcome::ProjectCreated { .. }
    ));
}

#[test]
fn catalog_mutation_replays_and_conflicting_request_identity_is_rejected() {
    let (mut authority, grant) = authority();
    let create = command(1, 0, create_action("shell", launch("/bin/zsh", &[])));
    let settled = submit(&mut authority, &grant, create.clone());
    let profile_id = created_id(&settled);

    // Identical replay returns the settled result — including the minted id —
    // without minting a second catalog entry.
    assert_eq!(submit(&mut authority, &grant, create.clone()), settled);
    let LaunchProfileCatalogOutcome::Catalog { entries, .. } =
        catalog_outcome(&mut authority, Some(&grant), catalog_read(500, None))
    else {
        panic!("expected catalog")
    };
    assert_eq!(entries.len(), 1);

    // Same request identity, different action: conflict, not a second effect.
    assert_eq!(
        submit(
            &mut authority,
            &grant,
            command(1, 1, create_action("other", launch("/bin/sh", &[]))),
        )
        .outcome,
        WorkbenchCommandOutcome::Rejected {
            revision: WorkbenchRevision::new(1),
            reason: WorkbenchRejectionReason::RequestIdentityConflict,
        }
    );

    // A settled delete replays identically; a fresh request deleting the same
    // (now absent) selector is a journaled launch-profile-unavailable, not an
    // error.
    let delete = command(2, 1, delete_action(profile_id.as_str()));
    let deleted = submit(&mut authority, &grant, delete.clone());
    assert!(matches!(
        deleted.outcome,
        WorkbenchCommandOutcome::LaunchProfileDeleted { .. }
    ));
    assert_eq!(submit(&mut authority, &grant, delete), deleted);
    let missing = submit(
        &mut authority,
        &grant,
        command(3, 2, delete_action(profile_id.as_str())),
    );
    assert_eq!(
        missing.outcome,
        WorkbenchCommandOutcome::LaunchProfileUnavailable {}
    );
    // The journaled miss replays too.
    assert_eq!(
        submit(
            &mut authority,
            &grant,
            command(3, 2, delete_action(profile_id.as_str())),
        ),
        missing
    );
}

#[test]
fn catalog_entry_count_and_serialized_bounds_reject_without_mutation() {
    let (mut authority, grant) = authority();
    for index in 0..MAXIMUM_LAUNCH_PROFILE_ENTRIES {
        let revision = u64::try_from(index).unwrap();
        assert!(matches!(
            create_profile(
                &mut authority,
                &grant,
                100 + index as u128,
                revision,
                &format!("p{index}"),
                launch("/bin/true", &[]),
            )
            .outcome,
            WorkbenchCommandOutcome::LaunchProfileCreated { .. }
        ));
    }
    let before = snapshot(&mut authority, &grant);
    let rejected = create_profile(
        &mut authority,
        &grant,
        999,
        u64::try_from(MAXIMUM_LAUNCH_PROFILE_ENTRIES).unwrap(),
        "overflow",
        launch("/bin/false", &[]),
    );
    assert_eq!(
        rejected.outcome,
        WorkbenchCommandOutcome::Rejected {
            revision: before.revision,
            reason: WorkbenchRejectionReason::ResourceLimit,
        }
    );
    assert_eq!(snapshot(&mut authority, &grant), before);
    let LaunchProfileCatalogOutcome::Catalog { entries, .. } =
        catalog_outcome(&mut authority, Some(&grant), catalog_read(501, None))
    else {
        panic!("expected catalog")
    };
    assert_eq!(entries.len(), MAXIMUM_LAUNCH_PROFILE_ENTRIES);

    // Serialized-byte ceiling: JSON escaping inflates the catalog document, so
    // control-heavy-but-legal entries (~12 KiB serialized each) cross the byte
    // budget well before the entry-count cap — rejected with zero mutation.
    let (mut byte_authority, byte_grant) = super::authority();
    let dense_name = "\u{1}".repeat(500);
    let dense_launch = launch(&format!("/{}", "\u{1}".repeat(1500)), &[]);
    let mut created = 0_u64;
    loop {
        let result = create_profile(
            &mut byte_authority,
            &byte_grant,
            2000 + created as u128,
            created,
            &dense_name,
            dense_launch.clone(),
        );
        if matches!(
            result.outcome,
            WorkbenchCommandOutcome::Rejected {
                reason: WorkbenchRejectionReason::ResourceLimit,
                ..
            }
        ) {
            break;
        }
        created += 1;
        assert!(created <= MAXIMUM_LAUNCH_PROFILE_ENTRIES as u64);
    }
    assert!(created < MAXIMUM_LAUNCH_PROFILE_ENTRIES as u64);
}

#[test]
fn catalog_mutation_at_revision_ceiling_rejects_with_zero_catalog_mutation() {
    let (mut authority, grant) = authority();
    create_profile(
        &mut authority,
        &grant,
        1,
        0,
        "shell",
        launch("/bin/zsh", &[]),
    );
    // Drive the public revision to the ceiling the same way the exhaustion
    // suite does: a prepared projection advance, not a ledger write.
    let mut exhausted = snapshot(&mut authority, &grant);
    exhausted.revision = WorkbenchRevision::new(u64::MAX);
    let bytes =
        serialize_document(&WorkbenchProtocolDocument::Snapshot(exhausted.clone())).unwrap();
    authority.projection.advance_projection(&bytes).unwrap();

    assert_eq!(
        submit(
            &mut authority,
            &grant,
            command(2, u64::MAX, create_action("late", launch("/bin/sh", &[]))),
        )
        .outcome,
        WorkbenchCommandOutcome::Rejected {
            revision: WorkbenchRevision::new(u64::MAX),
            reason: WorkbenchRejectionReason::RevisionExhausted,
        }
    );
    // Zero mutation: catalog contents and catalog revision are untouched.
    let LaunchProfileCatalogOutcome::Catalog {
        catalog_revision,
        entries,
        ..
    } = catalog_outcome(&mut authority, Some(&grant), catalog_read(600, None))
    else {
        panic!("expected catalog")
    };
    assert_eq!(catalog_revision, LaunchProfileCatalogRevision::new(1));
    assert_eq!(entries.len(), 1);
}

#[test]
fn profile_ids_do_not_alias_across_sessions() {
    let (mut first_authority, first_grant) = authority();
    let mut second_authority =
        WorkbenchAuthority::new(SessionId::parse(&identity(200)).unwrap()).unwrap();
    let second_grant = WorkbenchGrant::manage_navigation(SessionId::parse(&identity(200)).unwrap());

    let first_id = created_id(&create_profile(
        &mut first_authority,
        &first_grant,
        1,
        0,
        "shell",
        launch("/bin/zsh", &[]),
    ));
    let second_id = created_id(&submit(
        &mut second_authority,
        &second_grant,
        command_at(
            &second_grant.session_id,
            1,
            0,
            create_action("shell", launch("/bin/zsh", &[])),
        ),
    ));
    // Same counter, different Session prefix: an id from one Session can never select
    // a profile in another Session, by construction.
    assert_ne!(first_id, second_id);
    assert_eq!(
        catalog_outcome(
            &mut second_authority,
            Some(&second_grant),
            catalog_read(700, Some(first_id.as_str())),
        ),
        LaunchProfileCatalogOutcome::NotFound {}
    );
}
