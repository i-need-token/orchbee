use super::*;

#[test]
fn automatic_creation_fingerprints_mode_and_equal_text_rename_survives_settlement() {
    let (mut authority, grant) = authority();
    let mut create = tab_command(&authority, 1, 0, "1");
    let WorkbenchAction::TabCreate {
        automatic_label, ..
    } = &mut create.action
    else {
        unreachable!()
    };
    *automatic_label = true;
    let (pending, metadata) = commit(&mut authority, &grant, create.clone(), binding(20));
    let created = authority.projection.snapshot().clone();
    let tab = &created.terminal_contexts[0].tabs[0];
    assert!(tab.automatic_label);
    assert_eq!(tab.label, "1");

    let mut conflict = create.clone();
    let WorkbenchAction::TabCreate {
        automatic_label, ..
    } = &mut conflict.action
    else {
        unreachable!()
    };
    *automatic_label = false;
    let WorkbenchTabCreateBegin::Ready(conflict) =
        authority.begin_terminal_tab_create(Some(&grant), conflict)
    else {
        panic!("mode is part of identity")
    };
    assert!(matches!(
        conflict.outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::RequestIdentityConflict,
            ..
        }
    ));
    assert_eq!(authority.projection.snapshot(), &created);

    let rename = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&identity(2)).unwrap(),
        site_id: authority.site_id().clone(),
        expected_revision: WorkbenchRevision::new(1),
        action: WorkbenchAction::TabRename {
            terminal_context_id: metadata.terminal_context_id().clone(),
            tab_id: metadata.tab_id().clone(),
            label: WorkbenchLabel::parse("1").unwrap(),
        },
    };
    let response = authority.execute(
        Some(&grant),
        WorkbenchRequest::Command(rename.clone())
            .try_into()
            .unwrap(),
    );
    assert!(
        matches!(response, WorkbenchResponse::Command(WorkbenchCommandResult {outcome:WorkbenchCommandOutcome::Renamed{revision},..}) if revision.value()==2)
    );
    let renamed = authority.projection.snapshot().clone();
    let mut expected = created;
    expected.revision = WorkbenchRevision::new(2);
    expected.terminal_contexts[0].tabs[0].automatic_label = false;
    assert_eq!(renamed, expected);

    let settled = authority
        .prepare_terminal_tab_settlement(
            pending,
            WorkbenchTerminalOutcome::Started {
                geometry: geometry(80),
            },
        )
        .unwrap()
        .commit();
    assert_eq!(authority.projection.snapshot(), &renamed);
    let WorkbenchTabCreateBegin::Ready(replayed) =
        authority.begin_terminal_tab_create(Some(&grant), create)
    else {
        panic!("settled replay")
    };
    assert!(
        matches!(replayed.outcome, WorkbenchCommandOutcome::TabCreated {revision,..} if revision.value()==1)
    );
    assert_eq!(replayed, settled);
    let replay = authority.execute(
        Some(&grant),
        WorkbenchRequest::Command(rename.clone())
            .try_into()
            .unwrap(),
    );
    assert!(
        matches!(replay, WorkbenchResponse::Command(WorkbenchCommandResult {outcome:WorkbenchCommandOutcome::Renamed{revision},..}) if revision.value()==2)
    );
    let same = WorkbenchCommand {
        request_id: WorkbenchRequestId::parse(&identity(3)).unwrap(),
        expected_revision: WorkbenchRevision::new(2),
        ..rename
    };
    let result = authority.execute(
        Some(&grant),
        WorkbenchRequest::Command(same).try_into().unwrap(),
    );
    assert!(
        matches!(result, WorkbenchResponse::Command(WorkbenchCommandResult {outcome:WorkbenchCommandOutcome::Unchanged{revision},..}) if revision.value()==2)
    );
    assert_eq!(authority.projection.snapshot(), &renamed);
}
