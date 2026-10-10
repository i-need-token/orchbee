use super::*;
use crate::{machine_discovery::Context, terminal_session::NativeContextLocation};
use orchbee_workbench_protocol::*;
fn contexts(count: u64) -> Vec<Context> {
    let site = SiteId::parse(&format!("{:032x}", 1)).unwrap();
    (0..count)
        .map(|n| Context {
            target: NativeContextTarget {
                site_id: site.clone(),
                location: NativeContextLocation::Workspace {
                    workspace_id: WorkbenchWorkspaceId::parse(&format!("{:032x}", n + 100))
                        .unwrap(),
                    terminal_context_id: WorkbenchTerminalContextId::parse(&format!(
                        "{:032x}",
                        n + 200
                    ))
                    .unwrap(),
                },
            },
            project_id: Some(WorkbenchProjectId::parse(&format!("{:032x}", 2)).unwrap()),
            project: Some("Repo".into()),
            workspace: Some("same 工作区".into()),
        })
        .collect()
}
fn picker(count: u64) -> Picker {
    let mut picker = Picker {
        available: true,
        ..Picker::default()
    };
    picker.observe_agent_sidebar(true);
    picker.observe_local(contexts(count));
    picker
}
fn window(picker: &Picker, heights: &[u16], cells: u16, gap: u16) -> SidebarWindow {
    let rows = picker.sidebar_navigation_rows();
    assert_eq!(rows.len(), heights.len());
    picker.space_window_for_row_gaps(&rows, heights, &vec![gap; rows.len()], cells)
}
#[test]
fn unified_tree_window_scrolls_and_focus_clips_oversized_rows() {
    let mut picker = picker(7);
    let heights = [1, 1, 3, 2, 1, 4, 1, 2];
    let spaces = window(&picker, &heights, 6, 1);
    assert_eq!((spaces.top, spaces.end(), spaces.maximum), (0, 2, 6));
    picker.queue_space_scroll(spaces, false);
    assert!(picker.settle_sidebar_scroll());
    assert_eq!(window(&picker, &heights, 6, 1).top, 2);
    for _ in 0..4 {
        let w = window(&picker, &heights, 6, 1);
        picker.queue_space_scroll(w, false);
        picker.settle_sidebar_scroll();
    }
    assert_eq!(window(&picker, &heights, 6, 1).top, 6);
    let context = picker.workspace_rows[4]
        .context
        .as_ref()
        .unwrap()
        .target
        .clone();
    picker.observe_focus(Choice::Local, Some(context), None);
    let clipped = window(&picker, &[1, 1, 1, 1, 1, 16, 1, 1], 2, 0);
    assert_eq!((clipped.top, clipped.capacity), (5, 1));
}
#[test]
fn workspace_geometry_changes_discard_queued_wheel_and_rows_require_exact_current_identity() {
    let mut picker = picker(8);
    let old = picker.workspace_rows[1].selection();
    let w = window(&picker, &[1; 9], 3, 0);
    picker.queue_space_scroll(w, false);
    let mut changed = contexts(8);
    changed.reverse();
    picker.observe_local(changed);
    assert!(!picker.settle_sidebar_scroll());
    assert!(picker.activate_sidebar_workspace(&old));
    assert_eq!(picker.request, Some(old.clone()));
    picker.open = false;
    picker.request = None;
    picker.observe_local(contexts(1));
    assert!(!picker.activate_sidebar_workspace(&old));
    let valid = picker.workspace_rows[0].selection();
    picker.observe_focus(
        Choice::Local,
        picker.workspace_rows[0]
            .context
            .as_ref()
            .map(|c| c.target.clone()),
        None,
    );
    assert!(picker.activate_sidebar_workspace(&valid));
    assert!(!picker.open);
    assert!(picker.request.is_none());
    picker.observe_agent_sidebar(false);
    assert!(!picker.activate_sidebar_workspace(&valid));
}
#[test]
fn workspace_keyboard_picker_and_sidebar_request_the_same_duplicate_label_target() {
    let mut picker = picker(3);
    let exact = picker.workspace_rows[1].selection();
    picker.show_workspaces();
    picker.key(KeyEvent::press(KeyCode::Down, Modifiers::NONE));
    picker.key(KeyEvent::press(KeyCode::Enter, Modifiers::NONE));
    assert_eq!(picker.request, Some(exact.clone()));
    picker.open = false;
    picker.request = None;
    assert!(picker.activate_sidebar_workspace(&exact));
    assert_eq!(picker.request, Some(exact));
}
#[test]
fn workspace_state_uses_complete_post_attention_rows_before_agent_view_filters() {
    let mut picker = picker(2);
    let context = picker.workspace_rows[0]
        .context
        .as_ref()
        .unwrap()
        .target
        .clone();
    let mut working = crate::machine_agents::tests::row(23);
    working.target.context = context.clone();
    let mut blocked = crate::machine_agents::tests::row(24);
    blocked.target.context = context.clone();
    blocked.state = PanelAgentState::Blocked;
    let mut cp = crate::machine_agents::tests::checkpoint(vec![working, blocked])
        .as_ref()
        .clone();
    cp.view.spec=Some(parse_agent_view_spec(br#"{"source":"test.filter","filter":{"op":"eq","field":"status","value":"idle"},"sort":[]}"#).unwrap());
    picker.observe_agents(vec![(
        Choice::Local,
        crate::machine_agents::Observation::Ready(std::sync::Arc::new(cp)),
    )]);
    assert!(picker.agent_rows.iter().all(|r| r.agent.is_none()));
    let first = picker.workspace_rows[0].sidebar_target().unwrap();
    let counts = picker.sidebar_agent_counts(&first).unwrap();
    assert_eq!((counts.total, counts.waiting), (2, 1));
    let project = picker.sidebar_parent(&first).unwrap();
    assert_eq!(
        picker.sidebar_agent_counts(&project),
        None,
        "an incomplete Agent checkpoint cannot assert a complete Project count"
    );
    assert_eq!(
        picker.workspace_state(&picker.workspace_rows[0]),
        MetadataState::Blocked
    );
    assert_eq!(
        picker.workspace_state(&picker.workspace_rows[1]),
        MetadataState::Unknown,
        "a newly observed context absent from the complete checkpoint is not idle"
    );
    let context = picker.workspace_rows[1]
        .context
        .as_ref()
        .unwrap()
        .target
        .clone();
    let mut empty = crate::machine_agents::tests::checkpoint(Vec::new())
        .as_ref()
        .clone();
    empty.contexts = vec![context].into();
    picker.observe_agents(vec![(
        Choice::Local,
        crate::machine_agents::Observation::Ready(std::sync::Arc::new(empty)),
    )]);
    assert_eq!(
        picker.workspace_state(&picker.workspace_rows[1]),
        MetadataState::Idle
    );
    picker.observe_agents(vec![(
        Choice::Local,
        crate::machine_agents::Observation::Unavailable,
    )]);
    assert_eq!(
        picker.workspace_state(&picker.workspace_rows[1]),
        MetadataState::Unknown
    );
    assert_eq!(picker.sidebar_agent_counts(&first), None);
}
#[test]
fn workspace_state_priority_is_blocked_done_working_idle_unknown_after_seen_projection() {
    let mut picker = picker(1);
    let context = picker.workspace_rows[0]
        .context
        .as_ref()
        .unwrap()
        .target
        .clone();
    for (states, expected) in [
        (
            vec![
                (PanelAgentState::Unknown, true),
                (PanelAgentState::Idle, true),
            ],
            MetadataState::Idle,
        ),
        (
            vec![
                (PanelAgentState::Idle, true),
                (PanelAgentState::Working, true),
            ],
            MetadataState::Working,
        ),
        (
            vec![
                (PanelAgentState::Working, true),
                (PanelAgentState::Idle, false),
            ],
            MetadataState::Done,
        ),
        (
            vec![
                (PanelAgentState::Idle, false),
                (PanelAgentState::Blocked, true),
            ],
            MetadataState::Blocked,
        ),
    ] {
        let rows = states
            .into_iter()
            .enumerate()
            .map(|(i, (state, seen))| {
                let mut row = crate::machine_agents::tests::row(i as u64 + 23);
                row.target.context = context.clone();
                row.state = state;
                row.seen = seen;
                row
            })
            .collect();
        picker.agent_observations = vec![(
            Choice::Local,
            crate::machine_agents::Observation::Ready(crate::machine_agents::tests::checkpoint(
                rows,
            )),
        )];
        assert_eq!(picker.workspace_state(&picker.workspace_rows[0]), expected);
    }
}
