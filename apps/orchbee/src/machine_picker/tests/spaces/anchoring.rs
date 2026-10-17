//! Manual reading follows identities through mutations; explicit focus alone resumes reveal.
use super::*;

fn top_target(picker: &Picker, cells: u16) -> SidebarTarget {
    let rows = picker.sidebar_navigation_rows();
    let window =
        picker.space_window_for_row_gaps(&rows, &vec![1; rows.len()], &vec![0; rows.len()], cells);
    rows[window.top].target()
}

#[test]
fn inserted_and_reordered_rows_retain_the_top_identity_and_removed_rows_use_old_neighbors() {
    for count in [8, 20] {
        for reverse in [false, true] {
            let mut picker = picker(count);
            let rows = picker.sidebar_navigation_rows();
            let window = picker.space_window_for_row_gaps(
                &rows,
                &vec![1; rows.len()],
                &vec![0; rows.len()],
                3,
            );
            picker.queue_space_scroll(window, false);
            assert!(picker.settle_sidebar_scroll());
            let anchor = top_target(&picker, 3);
            let old = picker
                .sidebar_navigation_rows()
                .iter()
                .map(SidebarRow::target)
                .collect::<Vec<_>>();
            let position = old.iter().position(|target| target == &anchor).unwrap();
            let next = old[position + 1].clone();
            let mut changed = contexts(count + 1);
            changed.rotate_right(1);
            if reverse {
                changed.reverse();
            }
            picker.observe_local(changed.clone());
            assert_eq!(top_target(&picker, 2), anchor);
            assert!(picker.request.is_none());
            // Restore old order before removing the anchor, so the expected successor
            // comes from the visible reading order rather than a label or new ordinal.
            picker.observe_local(contexts(count));
            changed = contexts(count);
            changed.retain(|context| {
                Row {
                    choice: Choice::Local,
                    label: String::new(),
                    enabled: true,
                    observation: None,
                    context: Some(context.clone()),
                    agent: None,
                    agent_status: None,
                }
                .sidebar_target()
                .as_ref()
                    != Some(&anchor)
            });
            picker.observe_local(changed);
            assert_eq!(top_target(&picker, 2), next);
            picker.observe_local(Vec::new());
            assert_eq!(picker.space_window_for_row_gaps(&[], &[], &[], 3).top, 0);
        }
    }
}

#[test]
fn duplicate_labels_and_resized_variable_height_rows_preserve_identity_until_explicit_reveal() {
    let mut picker = picker(10);
    let rows = picker.sidebar_navigation_rows();
    let window =
        picker.space_window_for_row_gaps(&rows, &vec![1; rows.len()], &vec![0; rows.len()], 3);
    picker.queue_space_scroll(window, false);
    picker.settle_sidebar_scroll();
    let anchor = top_target(&picker, 3);
    let mut changed = contexts(11);
    changed.rotate_right(1);
    picker.observe_local(changed);
    for cells in [2, 5, 3] {
        let rows = picker.sidebar_navigation_rows();
        let heights = vec![2; rows.len()];
        let window = picker.space_window_for_row_gaps(&rows, &heights, &vec![1; rows.len()], cells);
        assert_eq!(rows[window.top].target(), anchor);
    }
    assert!(picker.enter_sidebar_navigation());
    picker.sidebar_key(KeyEvent::press(KeyCode::End, Modifiers::NONE));
    let rows = picker.sidebar_navigation_rows();
    let window =
        picker.space_window_for_row_gaps(&rows, &vec![1; rows.len()], &vec![0; rows.len()], 3);
    assert_eq!(window.end(), rows.len());
    assert!(picker.request.is_none());
}

#[test]
fn a_background_creation_slot_above_the_window_does_not_change_its_logical_anchor() {
    let mut picker = picker(12);
    let rows = picker.sidebar_navigation_rows();
    let window =
        picker.space_window_for_row_gaps(&rows, &vec![1; rows.len()], &vec![0; rows.len()], 3);
    picker.queue_space_scroll(window, false);
    picker.settle_sidebar_scroll();
    let anchor = top_target(&picker, 3);
    let rows = picker.sidebar_navigation_rows();
    let real_top = picker
        .space_window_for_row_gaps(&rows, &vec![1; rows.len()], &vec![0; rows.len()], 3)
        .top;
    let pending = picker.space_window_with_creation(
        &rows,
        &vec![1; rows.len() + 1],
        &vec![0; rows.len() + 1],
        3,
        Some(1),
        None,
    );
    assert_eq!(pending.top, real_top + 1);
    assert_eq!(rows[pending.top - 1].target(), anchor);
    picker.queue_space_scroll(pending, true);
    picker.settle_sidebar_scroll();
    let rows = picker.sidebar_navigation_rows();
    let logical =
        picker.space_window_for_row_gaps(&rows, &vec![1; rows.len()], &vec![0; rows.len()], 3);
    assert_eq!(logical.top, real_top.saturating_sub(3));
    assert!(picker.request.is_none());
}
