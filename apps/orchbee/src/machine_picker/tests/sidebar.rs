use super::*;
use crate::machine_agents::{Observation as Agents, tests::checkpoint};

fn sidebar_picker(count: u64) -> Picker {
    let mut picker = Picker {
        available: true,
        ..Picker::default()
    };
    picker.observe_agent_sidebar(true);
    let mut rows = Vec::new();
    for index in 0..count {
        let mut row = agent_row();
        row.target.member = orchbee_workbench_protocol::TerminalPresentationId::parse(&format!(
            "{:032x}",
            index + 23
        ))
        .unwrap();
        row.target.physical = crate::machine_agents::tests::physical(index + 23);
        rows.push(row);
    }
    picker.observe_local(vec![Context {
        target: agent_row().target.context,
        project_id: None,
        project: None,
        workspace: Some("Standalone".into()),
    }]);
    picker.observe_agents(vec![(Choice::Local, Agents::Ready(checkpoint(rows)))]);
    picker
}

fn owner(picker: &Picker) -> SidebarTarget {
    picker
        .sidebar_navigation_rows()
        .iter()
        .find_map(|row| match row {
            SidebarRow::Context { .. } => Some(row.target()),
            SidebarRow::Project { .. } | SidebarRow::Agent { .. } => None,
        })
        .unwrap()
}

fn unified_window(picker: &Picker, cells: u16) -> SidebarWindow {
    let rows = picker.sidebar_navigation_rows();
    picker.space_window_for_row_gaps(&rows, &vec![1; rows.len()], &vec![0; rows.len()], cells)
}

#[test]
fn sidebar_and_picker_are_independent_agent_read_consumers_and_last_row_prunes_disclosure() {
    let mut picker = sidebar_picker(1);
    let context = owner(&picker);
    assert!(picker.wants_agents());
    assert!(picker.sidebar_context_agents_expanded(&context));

    picker.show_agents();
    picker.observe_agent_sidebar(false);
    assert!(
        picker.wants_agents(),
        "the open global picker remains a consumer"
    );
    picker.key(KeyEvent::press(KeyCode::Esc, Modifiers::NONE));
    assert!(!picker.wants_agents());
    assert!(
        picker.sidebar_context_agents_expanded(&context),
        "hiding the surface alone does not erase Client-local disclosure"
    );

    picker.observe_agents(Vec::new());
    assert!(picker.agent_sources.is_empty());
    assert!(!picker.sidebar_context_agents_expanded(&context));
}

#[test]
fn agent_configuration_resets_the_real_unified_window_without_changing_identity() {
    let mut picker = sidebar_picker(8);
    let context = owner(&picker);
    assert!(picker.sidebar_context_agents_expanded(&context));
    let targets = picker
        .sidebar_navigation_rows()
        .iter()
        .map(SidebarRow::target)
        .collect::<Vec<_>>();
    let window = {
        let rows = picker.sidebar_navigation_rows();
        let heights = vec![1; rows.len()];
        let gaps = vec![0; rows.len()];
        picker.space_window_for_row_gaps(&rows, &heights, &gaps, 3)
    };
    picker.queue_space_scroll(window, false);
    assert!(picker.settle_sidebar_scroll());
    let rows = picker.sidebar_navigation_rows();
    let heights = vec![1; rows.len()];
    let gaps = vec![0; rows.len()];
    assert!(
        picker
            .space_window_for_row_gaps(&rows, &heights, &gaps, 3)
            .top
            > 0
    );

    picker.configure_agent_sidebar(
        orchbee_local_site_runtime::WindowsAgentSidebarSettings::from_profile(
            r#"{"rows":[],"rows_by_agent":{},"row_gap":1}"#,
        )
        .unwrap(),
    );
    let rows = picker.sidebar_navigation_rows();
    assert_eq!(
        picker
            .space_window_for_row_gaps(&rows, &vec![1; rows.len()], &vec![1; rows.len()], 3,)
            .top,
        0
    );
    assert_eq!(
        rows.iter().map(SidebarRow::target).collect::<Vec<_>>(),
        targets
    );
}

#[test]
fn sidebar_activation_re_resolves_visible_physical_identity_after_reorder() {
    let mut picker = sidebar_picker(2);
    let context = owner(&picker);
    assert!(picker.sidebar_context_agents_expanded(&context));
    let selected = picker.agent_rows[1].selection();
    let mut reordered = picker.agent_sources[0].1.clone();
    if let Agents::Ready(round) = &mut reordered {
        let mut round = round.as_ref().clone();
        let mut rows = round.rows.to_vec();
        rows.reverse();
        round.rows = rows.into();
        reordered = Agents::Ready(std::sync::Arc::new(round));
    }
    picker.observe_agents(vec![(Choice::Local, reordered)]);
    assert!(picker.activate_sidebar_agent(&selected));
    assert_eq!(picker.request.take(), Some(selected.clone()));

    picker.open = false;
    assert!(picker.toggle_sidebar_context_agents(&context));
    assert!(
        !picker.activate_sidebar_agent(&selected),
        "a collapsed row is not a current painted-tree candidate"
    );
    assert!(picker.toggle_sidebar_context_agents(&context));
    picker.observe_agents(vec![(
        Choice::Local,
        Agents::Ready(checkpoint(vec![agent_row()])),
    )]);
    assert!(!picker.activate_sidebar_agent(&selected));
    assert!(picker.request.is_none());
}

#[test]
fn current_agent_is_a_captured_noop_and_hidden_nested_rows_cannot_activate() {
    let mut picker = sidebar_picker(1);
    let owner = owner(&picker);
    assert!(picker.sidebar_context_agents_expanded(&owner));
    let selected = picker.agent_rows[0].selection();
    let target = picker.agent_rows[0].agent.as_ref().unwrap().target.clone();
    picker.observe_focus(
        Choice::Local,
        Some(target.context),
        Some(crate::terminal_session::NativeMemberTarget {
            tab: target.tab,
            panel: target.panel,
            presentation: target.member,
        }),
    );
    assert!(picker.activate_sidebar_agent(&selected));
    assert!(!picker.open);
    assert!(picker.request.is_none());
    picker.observe_agent_sidebar(false);
    assert!(!picker.activate_sidebar_agent(&selected));
    assert!(picker.request.is_none());
}

#[test]
fn unified_wheel_coalesces_caps_reverses_and_cancels_on_hide_or_picker_open() {
    let mut picker = sidebar_picker(30);
    let owner = owner(&picker);
    assert!(picker.sidebar_context_agents_expanded(&owner));
    let initial = unified_window(&picker, 4);
    for _ in 0..10 {
        picker.queue_space_scroll(initial, false);
    }
    assert_eq!(unified_window(&picker, 4).top, 0);
    assert!(picker.settle_sidebar_scroll());
    assert_eq!(unified_window(&picker, 4).top, 4);
    assert!(picker.settle_sidebar_scroll());
    assert_eq!(unified_window(&picker, 4).top, 8);
    picker.queue_space_scroll(unified_window(&picker, 4), true);
    assert!(picker.settle_sidebar_scroll());
    assert_eq!(unified_window(&picker, 4).top, 5);
    assert!(picker.request.is_none());

    picker.queue_space_scroll(unified_window(&picker, 4), false);
    picker.observe_agent_sidebar(false);
    assert!(!picker.settle_sidebar_scroll());
    picker.observe_agent_sidebar(true);
    picker.queue_space_scroll(unified_window(&picker, 4), false);
    picker.show_agents();
    assert!(!picker.settle_sidebar_scroll());
}

#[test]
fn source_view_updates_preserve_the_anchor_and_explicit_query_edits_reset_the_window() {
    let mut picker = sidebar_picker(20);
    let owner = owner(&picker);
    assert!(picker.sidebar_context_agents_expanded(&owner));
    picker.queue_space_scroll(unified_window(&picker, 4), false);
    assert!(picker.settle_sidebar_scroll());
    assert!(unified_window(&picker, 4).top > 0);
    let before_window = unified_window(&picker, 4);

    let Agents::Ready(before) = &picker.agent_sources[0].1 else {
        panic!()
    };
    let mut after = before.as_ref().clone();
    after.view.spec = Some(
        orchbee_workbench_protocol::parse_agent_view_spec(
            br#"{"source":"review","filter":{"op":"eq","field":"status","value":"working"}}"#,
        )
        .unwrap(),
    );
    picker.observe_agents(vec![(
        Choice::Local,
        Agents::Ready(std::sync::Arc::new(after)),
    )]);
    assert_eq!(unified_window(&picker, 4), before_window);
    assert!(!picker.settle_sidebar_scroll());
    assert!(picker.request.is_none());

    assert!(picker.sidebar_context_has_agents(&owner));
    if !picker.sidebar_context_agents_expanded(&owner) {
        assert!(picker.toggle_sidebar_context_agents(&owner));
    }
    picker.queue_space_scroll(unified_window(&picker, 4), false);
    assert!(picker.settle_sidebar_scroll());
    assert!(unified_window(&picker, 4).top > 0);
    picker.mode = Mode::Agents;
    picker.open = true;
    picker.editing_query = true;
    assert!(picker.agent_key(KeyCode::Char('b'), Modifiers::NONE));
    assert_eq!(unified_window(&picker, 4).top, 0);
    assert!(!picker.settle_sidebar_scroll());
    assert!(picker.request.is_none());
}

#[test]
fn unified_window_follows_the_exact_current_agent_before_its_owner_context() {
    let mut picker = sidebar_picker(10);
    let owner = owner(&picker);
    assert!(picker.sidebar_context_agents_expanded(&owner));
    let current = picker
        .agent_rows
        .last()
        .unwrap()
        .agent
        .as_ref()
        .unwrap()
        .target
        .clone();
    picker.observe_focus(
        Choice::Local,
        Some(current.context.clone()),
        Some(crate::terminal_session::NativeMemberTarget {
            tab: current.tab.clone(),
            panel: current.panel.clone(),
            presentation: current.member.clone(),
        }),
    );
    let rows = picker.sidebar_navigation_rows();
    assert!(
        matches!(rows.last(), Some(SidebarRow::Agent { row, .. }) if row.sidebar_agent().is_some_and(|agent| agent.target == current))
    );
    let mut heights = vec![1; rows.len()];
    *heights.last_mut().unwrap() = 3;
    let window = picker.space_window_for_row_gaps(&rows, &heights, &vec![0; rows.len()], 3);
    assert_eq!(window.end(), rows.len());
    assert_eq!(window.capacity, 1);
}

#[test]
fn unavailable_or_disabled_endpoint_never_activates_a_foreign_agent() {
    let mut picker = sidebar_picker(1);
    let context = owner(&picker);
    assert!(picker.sidebar_context_agents_expanded(&context));
    let remote = profile(1, "remote", true);
    let choice = Choice::Remote(remote.id.clone());
    picker.replace(std::slice::from_ref(&remote));
    picker.observe_agents(vec![
        (Choice::Local, Agents::Ready(checkpoint(vec![agent_row()]))),
        (choice.clone(), Agents::Ready(checkpoint(vec![agent_row()]))),
    ]);
    let selected = picker
        .agent_rows
        .iter()
        .find(|row| row.choice == choice)
        .unwrap()
        .selection();
    let mut disabled = remote;
    disabled.enabled = false;
    picker.replace(&[disabled]);
    assert!(!picker.activate_sidebar_agent(&selected));
    picker.observe_agents(vec![(Choice::Local, Agents::Unavailable)]);
    assert!(!picker.activate_sidebar_agent(&selected));
    assert!(picker.request.is_none());
}
