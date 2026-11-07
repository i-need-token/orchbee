//! Where focus returns when a Tab close holds the focused Panel.
use super::super::last_panel::panel_focus_after_tab_close;
use orchbee_workbench_protocol::WorkbenchTabId;

fn tab(value: u8) -> WorkbenchTabId {
    WorkbenchTabId::parse(&format!("{value:032x}")).expect("test tab id")
}

struct Place {
    tab: WorkbenchTabId,
    panel: u8,
}

fn place(tab_value: u8, panel: u8) -> Place {
    Place {
        tab: tab(tab_value),
        panel,
    }
}

#[test]
fn focus_returns_to_the_most_recent_panel_outside_the_closed_tab() {
    let history = [place(1, 11), place(2, 21), place(3, 31)];
    let picked = panel_focus_after_tab_close(
        &[tab(1)],
        Some(&history[0]),
        &history,
        |entry| &entry.tab,
        |_| true,
    );
    assert_eq!(picked.map(|entry| entry.panel), Some(21));
}

#[test]
fn closed_and_missing_entries_are_skipped_on_the_way_back() {
    let history = [place(1, 11), place(2, 21), place(3, 31), place(4, 41)];
    let picked = panel_focus_after_tab_close(
        &[tab(1), tab(2)],
        Some(&history[0]),
        &history,
        |entry| &entry.tab,
        |entry| entry.tab != tab(3),
    );
    assert_eq!(picked.map(|entry| entry.panel), Some(41));
}

#[test]
fn focus_stays_put_when_the_focused_panel_is_outside_the_closed_tabs() {
    let history = [place(1, 11), place(2, 21), place(3, 31)];
    let outside = panel_focus_after_tab_close(
        &[tab(2)],
        Some(&history[0]),
        &history,
        |entry| &entry.tab,
        |_| true,
    );
    assert!(outside.is_none());
    let unfocused =
        panel_focus_after_tab_close(&[tab(1)], None, &history, |entry| &entry.tab, |_| true);
    assert!(unfocused.is_none());
}

#[test]
fn no_panel_is_picked_without_an_eligible_entry() {
    let history = [place(1, 11), place(2, 21)];
    let all_closed = panel_focus_after_tab_close(
        &[tab(1), tab(2)],
        Some(&history[0]),
        &history,
        |entry| &entry.tab,
        |_| true,
    );
    assert!(all_closed.is_none());
    let none_open = panel_focus_after_tab_close(
        &[tab(1)],
        Some(&history[0]),
        &history,
        |entry| &entry.tab,
        |_| false,
    );
    assert!(none_open.is_none());
}
