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

    // The reopened items stand in for the saved ones under fresh identities, so a second reopen
    // finds nothing missing, and so does a later entry that names the reopened identities.
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
}
