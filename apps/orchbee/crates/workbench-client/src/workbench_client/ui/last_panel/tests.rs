//! Where focus returns across Workspaces once a Tab close settles.
use super::{
    ClosingTab, Focus, FocusReturnStep, capture_focus_return, input_drops_focus_return,
    panel_focus_after_tab_close, record_tab_close_request, settle_focus_return,
};
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
fn a_cross_workspace_return_waits_for_its_close_then_switches_to_its_tab_and_panel() {
    let origin = workspace(1);
    let other = workspace(2);
    // The closing Tab holds the focused Panel; the most recent Panel outside it is in another
    // Workspace.
    let history = [focus(&origin, 1, 11), focus(&other, 2, 21)];
    let closing = history[0].tab.clone();
    let picked = panel_focus_after_tab_close(
        std::slice::from_ref(&closing),
        Some(&history[0]),
        &history,
        |entry| &entry.tab,
        |_| true,
    )
    .expect("the other Workspace's Panel is picked");
    // The accepted close holds one return, even when the same close is observed again.
    let mut slot = None;
    capture_focus_return(&mut slot, &origin, &closing, picked, 0);
    capture_focus_return(&mut slot, &origin, &closing, picked, 0);
    let pending = slot.expect("one pending return");
    // While the close is open, nothing moves.
    assert_eq!(
        settle_focus_return(&pending, ClosingTab::Leaving, false, Some(1)),
        FocusReturnStep::Wait
    );
    // Once the close is over, the switch lands on the other Workspace at its position, on the
    // chosen Tab and Panel.
    let FocusReturnStep::Switch { index, target } =
        settle_focus_return(&pending, ClosingTab::Gone, true, Some(1))
    else {
        panic!("the return switches once the close is over");
    };
    assert_eq!(index, 1);
    assert_eq!(target, &focus(&other, 2, 21));
    // A Panel that is no longer open leaves the post-close landing in place.
    assert_eq!(
        settle_focus_return(&pending, ClosingTab::Gone, true, None),
        FocusReturnStep::Drop
    );
}

#[test]
fn input_after_a_tab_close_request_ends_its_return_and_a_new_request_ends_an_older_one() {
    let origin = workspace(1);
    let other = workspace(2);
    let history = [focus(&origin, 1, 11), focus(&other, 2, 21)];
    let closing = history[0].tab.clone();
    let picked = panel_focus_after_tab_close(
        std::slice::from_ref(&closing),
        Some(&history[0]),
        &history,
        |entry| &entry.tab,
        |_| true,
    )
    .expect("the other Workspace's Panel is picked");
    // The request records epoch 4 after its own input, and the return it holds carries that epoch.
    let mut slot = None;
    let mut requested = 0;
    record_tab_close_request(&mut slot, &mut requested, 4);
    capture_focus_return(&mut slot, &origin, &closing, picked, requested);
    let pending = slot.clone().expect("one pending return");
    // Nothing has moved the keyboard since the request, so the return stands.
    assert!(!input_drops_focus_return(&pending, 4));
    // A key press, paste or mouse button press after the request ends it.
    assert!(input_drops_focus_return(&pending, 5));
    // A new Tab close request ends the return that an earlier close held.
    record_tab_close_request(&mut slot, &mut requested, 5);
    assert_eq!(slot, None);
}
