//! Where focus returns when a Tab close holds the focused Panel.
use super::{Focus, focus_return_candidate};
use crate::workbench_projection::ClientContextLocation;
use orchbee_workbench_protocol::{
    WorkbenchPanelId, WorkbenchTabId, WorkbenchTerminalContextId, WorkbenchWorkspaceId,
};

fn hex(seed: u64) -> String {
    format!("{seed:032x}")
}

fn workspace(seed: u64) -> ClientContextLocation {
    ClientContextLocation::Workspace {
        workspace_id: WorkbenchWorkspaceId::parse(&hex(seed + 200)).expect("test workspace id"),
        terminal_context_id: WorkbenchTerminalContextId::parse(&hex(seed))
            .expect("test context id"),
    }
}

fn focus(location: &ClientContextLocation, tab: u64, panel: u64) -> Focus {
    Focus {
        location: location.clone(),
        tab: WorkbenchTabId::parse(&hex(tab)).expect("test tab id"),
        panel: WorkbenchPanelId::parse(&hex(panel)).expect("test panel id"),
    }
}

#[test]
fn focus_return_never_enters_another_workspace() {
    let origin = workspace(1);
    let other = workspace(2);
    let focused = focus(&origin, 1, 11);
    let closed = [focused.tab.clone()];
    // The most recent Panel is in another Workspace, so the next Panel of the origin is picked.
    let history = [focus(&other, 2, 21), focus(&origin, 3, 31)];
    assert_eq!(
        focus_return_candidate(&closed, &focused, &history, |_| true),
        Some(&history[1])
    );
    // Without a Panel of the origin Workspace, nothing is picked and focus stays in its Workspace.
    let only_other = [focus(&other, 2, 21)];
    assert_eq!(
        focus_return_candidate(&closed, &focused, &only_other, |_| true),
        None
    );
}
