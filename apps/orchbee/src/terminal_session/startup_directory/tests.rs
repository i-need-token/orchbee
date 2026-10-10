use super::*;
use orchbee_workbench_protocol::*;

fn id(value: u8) -> String {
    format!("{value:032x}")
}
pub(in crate::terminal_session) fn empty() -> Snapshot {
    let mut snapshot = crate::machine_agents::tests::fixture();
    snapshot.terminal_contexts[0].tabs.clear();
    snapshot
}
pub(in crate::terminal_session) fn project_only() -> Snapshot {
    let mut snapshot = empty();
    snapshot.revision = WorkbenchRevision::new(2);
    snapshot.navigation.projects.push(Project {
        label: "Default".into(),
        project_id: WorkbenchProjectId::parse(&id(3)).unwrap(),
    });
    snapshot
}
pub(in crate::terminal_session) fn created() -> Snapshot {
    let mut snapshot = empty();
    snapshot.revision = WorkbenchRevision::new(4);
    snapshot.navigation.directories.push(Directory {
        directory_id: WorkbenchDirectoryId::parse(&id(4)).unwrap(),
        worker_id: WorkerId::parse(&id(99)).unwrap(),
        path: WorkbenchDirectoryPath::parse("C:/project space").unwrap(),
    });
    snapshot.navigation.workspaces.push(Workspace {
        project_id: None,
        label: "project space".into(),
        primary_directory_id: Some(snapshot.navigation.directories[0].directory_id.clone()),
        workspace_id: WorkbenchWorkspaceId::parse(&id(5)).unwrap(),
        terminal_context_id: WorkbenchTerminalContextId::parse(&id(6)).unwrap(),
    });
    let tab = crate::machine_agents::tests::fixture().terminal_contexts[0]
        .tabs
        .remove(0);
    snapshot.terminal_contexts.push(TerminalContext {
        terminal_context_id: snapshot.navigation.workspaces[0]
            .terminal_context_id
            .clone(),
        groups: vec![],
        tabs: vec![tab],
    });
    snapshot.navigation.workspace_order =
        vec![snapshot.navigation.workspaces[0].workspace_id.clone()];
    snapshot
}
pub(in crate::terminal_session) fn commit() -> WorkspaceLaunchCommit {
    WorkspaceLaunchCommit {
        revision: WorkbenchRevision::new(3),
        project_id: None,
        workspace_id: WorkbenchWorkspaceId::parse(&id(5)).unwrap(),
        directory_id: WorkbenchDirectoryId::parse(&id(4)).unwrap(),
        terminal_context_id: WorkbenchTerminalContextId::parse(&id(6)).unwrap(),
        tab_id: WorkbenchTabId::parse(&id(10)).unwrap(),
        panel_id: WorkbenchPanelId::parse(&id(11)).unwrap(),
        presentation_id: TerminalPresentationId::parse(&id(12)).unwrap(),
        terminal_outcome: Some(WorkbenchTerminalOutcome::Started {
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
        }),
    }
}
pub(in crate::terminal_session) fn directory() -> StartupDirectory {
    StartupDirectory::new(Some(Path::new("C:/project space")), Locale::EnUs, true)
}

#[test]
fn directory_startup_only_empty_without_saved_position_or_restore_block_admits_creation() {
    let directory = directory();
    assert!(directory.admits(&empty(), false));
    assert!(!directory.admits(&empty(), true));
    assert!(
        !StartupDirectory::new(Some(Path::new("C:/repo")), Locale::EnUs, false)
            .admits(&empty(), false)
    );
    assert!(directory.admits(&project_only(), false));
    let mut empty_workspace = created();
    for context in &mut empty_workspace.terminal_contexts {
        context.tabs.clear();
    }
    assert!(!directory.admits(&empty_workspace, false));
    assert!(!directory.admits(&created(), false));
    assert!(!directory.admits(&crate::machine_agents::tests::fixture(), false));
    assert!(
        StartupDirectory::new(None, Locale::EnUs, true)
            .parameters()
            .is_err()
    );
    assert!(
        StartupDirectory::new(Some(Path::new("relative")), Locale::EnUs, true)
            .parameters()
            .is_err()
    );
    let parameters = directory.parameters().unwrap();
    assert_eq!(parameters.cwd.as_str(), "C:/project space");
    assert_eq!(parameters.workspace_label.as_str(), "project space");
    let root = StartupDirectory::new(Some(Path::new("C:/")), Locale::ZhCn, true)
        .parameters()
        .unwrap();
    assert_eq!(root.workspace_label.as_str(), "工作区");
}

#[test]
fn directory_startup_created_member_requires_known_started_and_exact_receipt_membership() {
    let parameters = directory().parameters().unwrap();
    let (cache, member) =
        validate_workspace(&created(), &empty().site_id, &commit(), &parameters).unwrap();
    assert!(
        matches!(cache.location(), NativeContextLocation::Workspace {workspace_id,..} if workspace_id.as_str()==id(5))
    );
    assert_eq!(member.tab.as_str(), id(10));
    assert_eq!(member.panel.as_str(), id(11));
    assert_eq!(member.presentation.as_str(), id(12));
    // Organization can change after the launch without changing the created terminal scope.
    let mut grouped = created();
    grouped.navigation.projects = project_only().navigation.projects;
    grouped.navigation.workspaces[0].project_id =
        Some(grouped.navigation.projects[0].project_id.clone());
    assert!(validate_workspace(&grouped, &empty().site_id, &commit(), &parameters).is_ok());
    let mut grouped_commit = commit();
    grouped_commit.project_id = grouped.navigation.workspaces[0].project_id.clone();
    assert!(validate_workspace(&grouped, &empty().site_id, &grouped_commit, &parameters).is_err());
    for outcome in [
        None,
        Some(WorkbenchTerminalOutcome::OutcomeUnknown {}),
        Some(WorkbenchTerminalOutcome::Failed {
            reason: WorkbenchTerminalFailureReason::TerminalCreateFailed,
        }),
        Some(WorkbenchTerminalOutcome::Unavailable {
            reason: WorkbenchTerminalUnavailableReason::TerminalRuntimeUnavailable,
        }),
    ] {
        let mut receipt = commit();
        receipt.terminal_outcome = outcome;
        assert!(validate_workspace(&created(), &empty().site_id, &receipt, &parameters).is_err());
    }
    let mut wrong = commit();
    wrong.presentation_id = TerminalPresentationId::parse(&id(13)).unwrap();
    assert!(validate_workspace(&created(), &empty().site_id, &wrong, &parameters).is_err());
    let mut foreign = created();
    foreign.site_id = SiteId::parse(&id(77)).unwrap();
    assert!(validate_workspace(&foreign, &empty().site_id, &commit(), &parameters).is_err());
    let mut stale = created();
    stale.revision = WorkbenchRevision::new(2);
    assert!(validate_workspace(&stale, &empty().site_id, &commit(), &parameters).is_err());
    let mut altered = created();
    altered.navigation.directories[0].path = WorkbenchDirectoryPath::parse("C:/other").unwrap();
    assert!(validate_workspace(&altered, &empty().site_id, &commit(), &parameters).is_err());
    let mut worker = created();
    worker.navigation.directories[0].worker_id = WorkerId::parse(&id(88)).unwrap();
    assert!(validate_workspace(&worker, &empty().site_id, &commit(), &parameters).is_err());
    let mut removed = created();
    removed.navigation.workspaces.clear();
    removed.navigation.workspace_order.clear();
    assert!(validate_workspace(&removed, &empty().site_id, &commit(), &parameters).is_err());
}

#[test]
fn directory_startup_known_receipts_remain_available_after_followup_failure_in_both_locales() {
    let directory = directory();
    assert!(directory.failure_note(Locale::EnUs).unwrap().is_none());
    directory
        .retain(Progress::Workspace(Arc::new(commit())))
        .unwrap();
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let note = directory.failure_note(locale).unwrap().unwrap();
        assert!(note.contains(&id(5)));
        assert!(note.contains("orchbee workspace list"));
    }
    directory
        .retain(Progress::WorkspaceUnknown(
            WorkbenchRequestId::parse(&id(91)).unwrap(),
        ))
        .unwrap();
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let note = directory.failure_note(locale).unwrap().unwrap();
        assert!(note.contains(&id(91)) && !note.contains(&id(5)));
    }
}
