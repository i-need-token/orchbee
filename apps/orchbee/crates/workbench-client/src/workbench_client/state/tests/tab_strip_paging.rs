use super::*;
use crate::tui_pointer::TabStripStart;

fn start(tab: &WorkbenchTabId) -> TabStripStart {
    TabStripStart {
        tab: tab.clone(),
        header: false,
    }
}

fn select(state: &mut WorkbenchClientState, index: usize) {
    let deadlines = OperationDeadlines::from_sample(Instant::now()).unwrap();
    let key = AttachmentKey::new(2, state.tab(index).unwrap().tab_id().clone());
    let target = state.target_for_switch(index, key, deadlines).unwrap();
    assert!(state.begin_switch(target));
}

#[test]
fn a_paged_strip_lasts_for_its_current_tab_and_outlives_closing_its_first_tab() {
    // A pointer activation inside the paged window keeps it; any other change of the current Tab
    // returns the strip to the window around that Tab.
    for followed in [true, false] {
        let mut state = ready(6);
        let ids: Vec<_> = (0..6)
            .map(|index| state.tab(index).unwrap().tab_id().clone())
            .collect();
        state.scroll_tab_strip(start(&ids[3]));
        state.settle_tab_strip_scroll();
        assert_eq!(state.tab_strip_start(), Some(&start(&ids[3])));
        if followed {
            state.follow_tab_strip_scroll(&ids[4]);
        }
        select(&mut state, 4);
        assert_eq!(
            state.tab_strip_start().is_some(),
            followed,
            "the view never shows a stale window before the frame settles it"
        );
        state.settle_tab_strip_scroll();
        assert_eq!(
            state.tab_strip_start(),
            followed.then(|| start(&ids[3])).as_ref()
        );
    }

    // Closing the Tab that opens the window moves its start to the next surviving Tab, or to the
    // previous one past the last Tab; the current Tab keeps the window.
    let mut state = ready(6);
    let ids: Vec<_> = (0..6)
        .map(|index| state.tab(index).unwrap().tab_id().clone())
        .collect();
    let mut snapshot = snapshot(6, 2, None);
    for (closed, survivor) in [(3, 4), (5, 4)] {
        state.scroll_tab_strip(start(&ids[closed]));
        state.settle_tab_strip_scroll();
        snapshot.terminal_contexts[0]
            .tabs
            .retain(|tab| tab.tab_id != ids[closed]);
        state.cache = project_preferred_workspace(&snapshot).unwrap();
        state.settle_tab_strip_scroll();
        assert_eq!(state.tab_strip_start(), Some(&start(&ids[survivor])));
    }
}
