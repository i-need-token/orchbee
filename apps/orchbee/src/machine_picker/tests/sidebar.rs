use super::*;
use crate::machine_agents::{Observation as Agents, tests::checkpoint};

fn sidebar_picker(count: u64) -> Picker {
    let mut picker = Picker {
        available: true,
        ..Picker::default()
    };
    picker.observe_agent_sidebar(true);
    let rows = (0..count)
        .map(|index| {
            let mut row = agent_row();
            row.target.member = orchbee_workbench_protocol::TerminalPresentationId::parse(
                &format!("{:032x}", index + 23),
            )
            .unwrap();
            row.target.physical = crate::machine_agents::tests::physical(index + 23);
            row
        })
        .collect();
    picker.observe_agents(vec![(Choice::Local, Agents::Ready(checkpoint(rows)))]);
    picker
}

#[test]
fn sidebar_variable_heights_fit_entries_gaps_final_page_and_exact_focus() {
    let mut picker = sidebar_picker(7);
    let heights = [1, 3, 2, 1, 4, 1, 2];
    let window = picker.sidebar_window_for_heights(&heights, 6, 1);
    assert_eq!((window.top, window.end(), window.maximum), (0, 2, 5));
    picker.queue_sidebar_scroll(window, false);
    assert!(picker.settle_sidebar_scroll());
    let window = picker.sidebar_window_for_heights(&heights, 6, 1);
    assert_eq!((window.top, window.end()), (2, 4));
    for _ in 0..5 {
        let window = picker.sidebar_window_for_heights(&heights, 6, 1);
        picker.queue_sidebar_scroll(window, false);
        picker.settle_sidebar_scroll();
    }
    let window = picker.sidebar_window_for_heights(&heights, 6, 1);
    assert_eq!((window.top, window.end()), (5, 7));
    let current = picker.agent_rows[4].agent.as_ref().unwrap().clone();
    picker.observe_focus(
        Choice::Local,
        Some(current.target.context),
        Some(NativeMemberTarget {
            tab: current.target.tab,
            panel: current.target.panel,
            presentation: current.target.member,
        }),
    );
    assert_eq!(
        (
            picker.sidebar_window_for_heights(&heights, 6, 1).top,
            picker.sidebar_window_for_heights(&heights, 6, 1).end()
        ),
        (3, 5)
    );
    let oversized = picker.sidebar_window_for_heights(&[20; 7], 3, u16::MAX);
    assert_eq!(
        (oversized.top, oversized.capacity, oversized.maximum),
        (4, 1, 6)
    );
    assert_eq!(
        picker.sidebar_window_for_heights(&heights, 0, 0).capacity,
        0
    );
}

#[test]
fn sidebar_and_picker_are_independent_read_consumers() {
    let mut picker = sidebar_picker(1);
    assert!(picker.wants_agents());
    picker.show_agents();
    picker.observe_agent_sidebar(false);
    assert!(picker.wants_agents());
    picker.key(KeyEvent::press(KeyCode::Esc, Modifiers::NONE));
    assert!(!picker.wants_agents());
    picker.observe_agents(vec![]);
    assert!(picker.agent_sources.is_empty());
    picker.observe_agent_sidebar(true);
    assert!(picker.wants_agents());
    assert!(picker.agent_rows.iter().all(|row| row.agent.is_none()));
}

#[test]
fn sidebar_configuration_changes_cancel_old_geometry_without_changing_agent_authority() {
    let mut picker = sidebar_picker(20);
    let targets = picker
        .sidebar_rows()
        .iter()
        .map(Row::selection)
        .collect::<Vec<_>>();
    picker.queue_sidebar_scroll(picker.sidebar_window(4), false);
    assert!(picker.settle_sidebar_scroll());
    let settings = orchbee_local_site_runtime::WindowsAgentSidebarSettings::from_profile(
        r#"{"rows":[],"rows_by_agent":{},"row_gap":1}"#,
    )
    .unwrap();
    picker.configure_agent_sidebar(settings.clone());
    assert_eq!(picker.sidebar_window(4).top, 0);
    assert!(!picker.settle_sidebar_scroll());
    assert_eq!(
        picker
            .sidebar_rows()
            .iter()
            .map(Row::selection)
            .collect::<Vec<_>>(),
        targets
    );
    picker.queue_sidebar_scroll(picker.sidebar_window(4), false);
    picker.settle_sidebar_scroll();
    picker.configure_agent_sidebar(settings);
    assert_eq!(
        picker.sidebar_window(4).top,
        3,
        "an identical profile is a no-op"
    );
    // Metadata may change visible heights; old wheel work cannot own a new projection.
    for _ in 0..3 {
        picker.queue_sidebar_scroll(picker.sidebar_window(4), false);
    }
    picker.observe_agents(vec![(
        Choice::Local,
        Agents::Ready(checkpoint(vec![agent_row()])),
    )]);
    assert!(!picker.settle_sidebar_scroll());
}

#[test]
fn sidebar_follows_exact_focus_until_scrolled_and_resumes_on_focus_change() {
    let mut picker = sidebar_picker(15);
    let row = picker.agent_rows[12].agent.as_ref().unwrap().clone();
    picker.observe_focus(
        Choice::Local,
        Some(row.target.context.clone()),
        Some(NativeMemberTarget {
            tab: row.target.tab.clone(),
            panel: row.target.panel.clone(),
            presentation: row.target.member.clone(),
        }),
    );
    let window = picker.sidebar_window(4);
    assert_eq!((window.top, window.end(), window.total), (9, 13, 15));
    picker.queue_sidebar_scroll(window, true);
    assert!(picker.settle_sidebar_scroll());
    assert_eq!(picker.sidebar_window(4).top, 6);
    let next = picker.agent_rows[0].agent.as_ref().unwrap().clone();
    picker.observe_focus(
        Choice::Local,
        Some(next.target.context),
        Some(NativeMemberTarget {
            tab: next.target.tab,
            panel: next.target.panel,
            presentation: next.target.member,
        }),
    );
    assert_eq!(picker.sidebar_window(4).top, 0);
    assert_eq!(picker.sidebar_window(0).end(), 0);
}

#[test]
fn sidebar_wheel_coalesces_caps_reverses_and_cancels_without_navigation() {
    let mut picker = sidebar_picker(30);
    let window = picker.sidebar_window(4);
    for _ in 0..10 {
        picker.queue_sidebar_scroll(window, false);
    }
    assert_eq!(picker.sidebar_window(4).top, 0);
    assert!(picker.settle_sidebar_scroll());
    assert_eq!(picker.sidebar_window(4).top, 4);
    assert!(picker.settle_sidebar_scroll());
    assert_eq!(picker.sidebar_window(4).top, 8);
    picker.queue_sidebar_scroll(picker.sidebar_window(4), true);
    assert!(picker.settle_sidebar_scroll());
    assert_eq!(picker.sidebar_window(4).top, 5);
    assert!(picker.request.is_none());
    picker.observe_agent_sidebar(false);
    assert!(!picker.settle_sidebar_scroll());
    picker.observe_agent_sidebar(true);
    picker.queue_sidebar_scroll(picker.sidebar_window(4), false);
    picker.show_agents();
    assert!(!picker.settle_sidebar_scroll());
}

#[test]
fn replacing_source_view_resets_manual_sidebar_window_without_acknowledging() {
    let mut picker = sidebar_picker(20);
    picker.queue_sidebar_scroll(picker.sidebar_window(4), false);
    assert!(picker.settle_sidebar_scroll());
    assert_eq!(picker.sidebar_window(4).top, 3);
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
    assert_eq!(picker.sidebar_window(4).top, 0);
    assert!(!picker.settle_sidebar_scroll());
    assert!(picker.request.is_none());
}

#[test]
fn sidebar_activation_re_resolves_frozen_identity_after_reorder_without_ordinal_fallback() {
    let mut picker = sidebar_picker(2);
    let selected = picker.agent_rows[1].selection();
    let before_seen = picker.agent_rows[1].agent.as_ref().unwrap().seen;
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
    assert_eq!(
        picker.agent_rows[0].agent.as_ref().unwrap().seen,
        before_seen
    );
    picker.open = false;
    picker.observe_agents(vec![(
        Choice::Local,
        Agents::Ready(checkpoint(vec![agent_row()])),
    )]);
    assert!(!picker.activate_sidebar_agent(&selected));
    assert!(!picker.open);
    assert!(picker.request.is_none());
    picker.observe_agents(vec![(Choice::Local, Agents::Unavailable)]);
    assert!(!picker.activate_sidebar_agent(&selected));
}

#[test]
fn sidebar_current_member_does_not_attach_and_hidden_rows_cannot_activate() {
    let mut picker = sidebar_picker(1);
    let selected = picker.agent_rows[0].selection();
    let target = agent_row().target;
    picker.observe_focus(
        Choice::Local,
        Some(target.context),
        Some(NativeMemberTarget {
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
}

#[test]
fn sidebar_source_failures_and_disabled_endpoints_never_activate_foreign_targets() {
    let mut picker = sidebar_picker(1);
    let remote = profile(1, "remote", true);
    let choice = Choice::Remote(remote.id.clone());
    picker.replace(std::slice::from_ref(&remote));
    picker.observe_agents(vec![
        (Choice::Local, Agents::Ready(checkpoint(vec![agent_row()]))),
        (choice.clone(), Agents::Ready(checkpoint(vec![agent_row()]))),
    ]);
    let selected = picker.agent_rows[1].selection();
    let mut disabled = remote;
    disabled.enabled = false;
    picker.replace(&[disabled.clone()]);
    assert!(!picker.activate_sidebar_agent(&selected));
    disabled.enabled = true;
    picker.replace(&[disabled]);
    picker.observe_agents(vec![
        (Choice::Local, Agents::Unavailable),
        (choice, Agents::Ready(checkpoint(vec![agent_row()]))),
    ]);
    assert_eq!(picker.sidebar_rows().len(), 1);
    assert!(!picker.activate_sidebar_agent(&selected));
    assert!(picker.request.is_none());
}
