use super::*;
use orchbee_workbench_protocol::{
    PresentationBinding, PresentationLaunchProgram, TerminalId, WorkbenchDirectoryPath,
};

fn binding(id: u128, owner: u128) -> PresentationBinding {
    PresentationBinding {
        terminal_id: TerminalId::parse(&identity(id)).unwrap(),
        worker_id: worker(owner),
    }
}

fn replace_snapshot(authority: &mut WorkbenchAuthority, edit: impl FnOnce(&mut Snapshot)) {
    let mut snapshot = authority.snapshot().clone();
    edit(&mut snapshot);
    authority.projection = WorkbenchProjectionStore::load(
        &serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot)).unwrap(),
    )
    .unwrap();
}

#[test]
fn enumerates_every_primary_and_child_with_exact_membership_and_workspace_directory() {
    let authority = restore(&checkpoint()).unwrap();
    let targets = authority.restoration_targets(&worker(400)).unwrap();
    assert_eq!(targets.len(), 5);
    for ((target, presentation), panel) in targets
        .iter()
        .zip([11, 15, 14, 23, 18])
        .zip([10, 10, 13, 22, 17])
    {
        assert_eq!(target.presentation_id().as_str(), identity(presentation));
        assert_eq!(target.panel_id().as_str(), identity(panel));
        assert_eq!(target.terminal_context_id().as_str(), identity(5));
        assert_eq!(
            target.tab_id().as_str(),
            identity(if presentation == 18 { 16 } else { 7 })
        );
        assert_eq!(
            target.working_directory().unwrap().as_str(),
            "C:\\work space\\项目"
        );
    }
    assert_eq!(
        authority.restoration_targets(&worker(200)).err(),
        Some(WorkbenchRestoreError::ForeignWorker)
    );
}

#[test]
fn preparation_is_effect_free_and_all_commits_preserve_retained_configuration() {
    let mut authority = restore(&checkpoint()).unwrap();
    let targets = authority.restoration_targets(&worker(400)).unwrap();
    let before = authority.snapshot().clone();
    let retained = authority.checkpoint(&worker(400)).unwrap();
    {
        let _uncommitted = authority
            .prepare_restored_terminal(&targets[0], binding(900, 400), None)
            .unwrap();
    }
    assert_eq!(authority.snapshot(), &before);
    let program = PresentationLaunchProgram::parse("pwsh.exe").ok();
    for (index, target) in targets.iter().enumerate() {
        let revision = authority
            .prepare_restored_terminal(target, binding(900 + index as u128, 400), program.clone())
            .unwrap()
            .commit();
        assert_eq!(revision.value(), index as u64 + 1);
        assert_eq!(
            authority.restoration_targets(&worker(400)).unwrap().len(),
            4 - index
        );
    }
    // Each binding published its program in its own revision; the checkpoint keeps neither.
    let published = serde_json::to_string(authority.snapshot()).unwrap();
    assert_eq!(
        published.matches(r#""launchProgram":"pwsh.exe""#).count(),
        targets.len()
    );
    assert_eq!(authority.checkpoint(&worker(400)).unwrap(), retained);
    assert_eq!(authority.last_identity, 40);
    assert!(authority.ledger.is_empty());
    assert_eq!(authority.catalog_revision, 0);
    assert_eq!(
        authority
            .prepare_restored_terminal(&targets[0], binding(999, 400), None)
            .err(),
        Some(WorkbenchRestoreError::AlreadyBound)
    );
}

#[test]
fn unconfigured_workspaces_keep_the_runtime_default_directory() {
    let unconfigured = restore(&changed(|value| {
        value["navigation"]["workspaces"][0]["primaryDirectoryId"] = Value::Null;
    }))
    .unwrap();
    let targets = unconfigured.restoration_targets(&worker(400)).unwrap();
    assert_eq!(targets.len(), 5);
    assert!(
        targets
            .iter()
            .all(|target| target.working_directory().is_none())
    );
}

#[test]
fn refuses_old_session_foreign_worker_removed_membership_and_changed_directory() {
    let mut authority = restore(&checkpoint()).unwrap();
    let target = authority
        .restoration_targets(&worker(400))
        .unwrap()
        .remove(0);
    let before = authority.snapshot().clone();
    assert_eq!(
        authority
            .prepare_restored_terminal(&target, binding(900, 500), None)
            .err(),
        Some(WorkbenchRestoreError::ForeignWorker)
    );
    assert_eq!(authority.snapshot(), &before);
    let mut another_session =
        WorkbenchAuthority::from_checkpoint(&checkpoint(), session(301), &worker(400)).unwrap();
    assert_eq!(
        another_session
            .prepare_restored_terminal(&target, binding(900, 400), None)
            .err(),
        Some(WorkbenchRestoreError::TargetChanged)
    );
    replace_snapshot(&mut authority, |snapshot| {
        snapshot.navigation.directories[0].path =
            WorkbenchDirectoryPath::parse("C:\\different").unwrap()
    });
    let changed = authority.snapshot().clone();
    assert_eq!(
        authority
            .prepare_restored_terminal(&target, binding(900, 400), None)
            .err(),
        Some(WorkbenchRestoreError::TargetChanged)
    );
    assert_eq!(authority.snapshot(), &changed);
    let mut removed = restore(&checkpoint()).unwrap();
    replace_snapshot(&mut removed, |snapshot| {
        snapshot.terminal_contexts[0].tabs[0].tab_id =
            orchbee_workbench_protocol::WorkbenchTabId::parse(&identity(80)).unwrap()
    });
    assert_eq!(
        removed
            .prepare_restored_terminal(&target, binding(900, 400), None)
            .err(),
        Some(WorkbenchRestoreError::TargetChanged)
    );
}

#[test]
fn duplicate_physical_binding_retirement_and_revision_exhaustion_do_not_partially_commit() {
    let mut authority = restore(&checkpoint()).unwrap();
    let targets = authority.restoration_targets(&worker(400)).unwrap();
    authority
        .prepare_restored_terminal(&targets[0], binding(900, 400), None)
        .unwrap()
        .commit();
    let before = authority.snapshot().clone();
    assert!(matches!(
        authority.prepare_restored_terminal(&targets[1], binding(900, 400), None),
        Err(WorkbenchRestoreError::InvalidProjection(_))
    ));
    assert_eq!(authority.snapshot(), &before);
    replace_snapshot(&mut authority, |snapshot| {
        snapshot.revision = WorkbenchRevision::new(u64::MAX)
    });
    let exhausted = authority.snapshot().clone();
    assert_eq!(
        authority
            .prepare_restored_terminal(&targets[1], binding(901, 400), None)
            .err(),
        Some(WorkbenchRestoreError::RevisionExhausted)
    );
    assert_eq!(authority.snapshot(), &exhausted);
    authority.retired = true;
    assert_eq!(
        authority.restoration_targets(&worker(400)).err(),
        Some(WorkbenchRestoreError::Unavailable)
    );
    assert_eq!(
        authority
            .prepare_restored_terminal(&targets[1], binding(901, 400), None)
            .err(),
        Some(WorkbenchRestoreError::Unavailable)
    );
}
