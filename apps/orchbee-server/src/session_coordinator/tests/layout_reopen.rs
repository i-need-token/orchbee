//! Reopening an earlier layout opens only what is missing, changes nothing open, and holds a
//! reopened terminal whose saved directory is gone.
use super::*;
use orchbee_server::WorkbenchCheckpoint;
use orchbee_workbench_protocol::{PresentationBinding, TerminalId, WorkerId as WorkbenchWorkerId};

fn worker() -> WorkbenchWorkerId {
    WorkbenchWorkerId::parse(&"09".repeat(16)).unwrap()
}

fn binding(value: u8) -> PresentationBinding {
    PresentationBinding {
        terminal_id: TerminalId::parse(&format!("{value:02x}").repeat(16)).unwrap(),
        worker_id: worker(),
    }
}

fn tabs(snapshot: &Snapshot) -> usize {
    snapshot
        .terminal_contexts
        .iter()
        .map(|context| context.tabs.len())
        .sum()
}

#[test]
fn reopen_adds_only_missing_items_and_holds_a_missing_directory() {
    let session_id = SessionId::parse(&format!("{:032x}", 999)).unwrap();
    let mut earlier = WorkbenchAuthority::workspace_fixture(session_id.clone()).unwrap();
    earlier.append_fixture_tab(binding(0x21)).unwrap();
    let one_tab = earlier.checkpoint(&worker()).unwrap();
    earlier.append_fixture_tab(binding(0x22)).unwrap();
    let entry = WorkbenchCheckpoint::parse(&earlier.checkpoint(&worker()).unwrap()).unwrap();

    // A missing Tab of an open Workspace returns; the open Tab stays exactly as it was.
    let mut coordinator = SessionCoordinator::new(Some(
        WorkbenchAuthority::from_checkpoint(&one_tab, session_id.clone(), &worker()).unwrap(),
    ));
    let before = coordinator.workbench.as_ref().unwrap().snapshot().clone();
    let (reopened, targets) = coordinator.reopen_layout(&entry, &worker()).unwrap();
    assert_eq!((reopened.workspaces, reopened.tabs), (0, 1));
    assert_eq!(targets.len(), 1);
    let after = coordinator.workbench.as_ref().unwrap().snapshot().clone();
    assert_eq!(after.revision.value(), before.revision.value() + 1);
    assert_eq!(after.navigation, before.navigation);
    let (old, new) = (&before.terminal_contexts[0], &after.terminal_contexts[0]);
    assert_eq!(new.tabs.len(), 2);
    assert_eq!(new.tabs[0], old.tabs[0]);
    let (context, tab) = reopened.first_tab.clone().unwrap();
    assert_eq!(
        (&context, &tab),
        (&new.terminal_context_id, &new.tabs[1].tab_id)
    );
    let workbench = coordinator.workbench.as_ref().unwrap();
    assert_eq!(
        entry
            .layout()
            .unwrap()
            .summary(&workbench.open_layout())
            .tabs_not_open,
        0
    );

    // The saved directory does not exist here, so the reopened terminal is held.
    let target = &targets[0];
    let directory = target.working_directory().cloned().unwrap();
    assert!(!std::path::Path::new(directory.as_str()).is_dir());
    coordinator.hold_reopened_terminal(target, directory);
    let held = coordinator.workbench.as_ref().unwrap().snapshot();
    assert_eq!(held.terminal_contexts[0].tabs[0], old.tabs[0]);
    let reopened_tab = serde_json::to_string(&held.terminal_contexts[0].tabs[1]).unwrap();
    assert!(reopened_tab.contains(target.presentation_id().as_str()));
    assert!(reopened_tab.contains("\"launchFailure\":{"));

    // A missing Workspace comes back whole, with its directory and both Tabs.
    let mut coordinator =
        SessionCoordinator::new(Some(WorkbenchAuthority::new(session_id).unwrap()));
    let (reopened, targets) = coordinator.reopen_layout(&entry, &worker()).unwrap();
    assert_eq!(
        (reopened.workspaces, reopened.tabs, targets.len()),
        (1, 2, 2)
    );
    let after = coordinator.workbench.as_ref().unwrap().snapshot().clone();
    assert_eq!(after.navigation.workspaces.len(), 1);
    assert_eq!(after.navigation.directories.len(), 1);
    assert_eq!(tabs(&after), 2);

    // A second reopen finds nothing missing, and so does a later entry that names what reopened.
    let (again, targets) = coordinator.reopen_layout(&entry, &worker()).unwrap();
    assert_eq!(
        (again.revision, again.workspaces, again.tabs, targets.len()),
        (None, 0, 0, 0)
    );
    let later = coordinator
        .workbench
        .as_ref()
        .unwrap()
        .checkpoint(&worker())
        .unwrap();
    let later = WorkbenchCheckpoint::parse(&later).unwrap();
    let (again, _) = coordinator.reopen_layout(&later, &worker()).unwrap();
    assert_eq!(again.revision, None);
    let workbench = coordinator.workbench.as_ref().unwrap();
    let open = workbench.open_layout();
    for kept in [&entry, &later] {
        let summary = kept.layout().unwrap().summary(&open);
        assert_eq!((summary.workspaces_not_open, summary.tabs_not_open), (0, 0));
    }

    // Restored from a checkpoint whose allocation is already past the entry's identities, as a
    // real Workbench's is, the reopened Tab takes a fresh identity. Its lineage counts it as the
    // saved Tab, and the checkpoint keeps that lineage, so the answer survives a Server restart.
    let mut document: serde_json::Value = serde_json::from_slice(&one_tab).unwrap();
    document["lastIdentity"] = serde_json::Value::from(format!("{:032x}", 0x400));
    let bumped = serde_json::to_vec(&document).unwrap();
    let session = SessionId::parse(&format!("{:032x}", 1000)).unwrap();
    let mut coordinator = SessionCoordinator::new(Some(
        WorkbenchAuthority::from_checkpoint(&bumped, session.clone(), &worker()).unwrap(),
    ));
    let (reopened, _) = coordinator.reopen_layout(&entry, &worker()).unwrap();
    let (_, tab) = reopened.first_tab.unwrap();
    assert!(
        earlier.snapshot().terminal_contexts[0]
            .tabs
            .iter()
            .all(|saved| saved.tab_id != tab)
    );
    let (again, _) = coordinator.reopen_layout(&entry, &worker()).unwrap();
    assert_eq!(again.revision, None);
    let saved = coordinator
        .workbench
        .as_ref()
        .unwrap()
        .checkpoint(&worker())
        .unwrap();
    assert!(String::from_utf8_lossy(&saved).contains("\"layoutLineage\":[{\"reopened\":"));
    let mut coordinator = SessionCoordinator::new(Some(
        WorkbenchAuthority::from_checkpoint(&saved, session, &worker()).unwrap(),
    ));
    let workbench = coordinator.workbench.as_ref().unwrap();
    let summary = entry.layout().unwrap().summary(&workbench.open_layout());
    assert_eq!((summary.workspaces_not_open, summary.tabs_not_open), (0, 0));
    let (again, _) = coordinator.reopen_layout(&entry, &worker()).unwrap();
    assert_eq!(again.revision, None);
}
