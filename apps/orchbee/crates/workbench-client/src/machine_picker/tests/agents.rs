use super::*;
use crate::machine_agents::{
    Observation as Agents,
    view::{Order, Scope},
};
use std::time::{Duration, Instant};

#[test]
fn ordinal_agent_focus_uses_source_sorted_targets_and_never_counts_placeholders() {
    use crate::machine_agents::test_support::checkpoint;
    use orchbee_workbench_protocol::*;
    let remote = profile(1, "remote", true);
    let disabled = profile(2, "disabled", false);
    let choice = Choice::Remote(remote.id.clone());
    let mut picker = Picker {
        available: true,
        ..Picker::default()
    };
    picker.replace(&[remote, disabled.clone()]);
    focus(&mut picker);
    let mut local = checkpoint(vec![agent_row()]).as_ref().clone();
    local.view.spec=Some(parse_agent_view_spec(br#"{"source":"tools.views","filter":{"op":"eq","field":"status","value":"blocked"},"sort":[{"field":{"token":"phase"},"order":"desc"}]}"#).unwrap());
    let local = Agents::Ready(std::sync::Arc::new(local));
    let now = Instant::now();
    picker.observe_agents(vec![
        (Choice::Local, local.clone()),
        (choice.clone(), Agents::Loading),
    ]);
    picker.focus_agent(1, now);
    assert!(!picker.drive_agent_navigation(now));
    let mut first = agent_row();
    first.state = PanelAgentState::Blocked;
    first.tokens = MetadataTokens::new(
        [(
            MetadataTokenKey::parse("phase").unwrap(),
            MetadataTokenValue::parse("10").unwrap(),
        )]
        .into(),
    )
    .unwrap();
    let mut second = first.clone();
    second.target.member = TerminalPresentationId::parse(&format!("{:032x}", 24)).unwrap();
    second.target.physical = crate::machine_agents::test_support::physical(24);
    second.tokens = MetadataTokens::new(
        [(
            MetadataTokenKey::parse("phase").unwrap(),
            MetadataTokenValue::parse("2").unwrap(),
        )]
        .into(),
    )
    .unwrap();
    let observations = vec![
        (Choice::Local, local),
        (
            choice.clone(),
            Agents::Ready(checkpoint(vec![first.clone(), second.clone()])),
        ),
        (Choice::Remote(disabled.id), Agents::Loading),
    ];
    picker.observe_agents(observations.clone());
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let text = paint(&picker, locale, 100, 30);
        assert!(text.contains("1 remote /"));
        assert!(text.contains("2 remote /"));
        assert!(!text.contains("3 Local /") && !text.contains("3 本地 /"));
    }
    assert!(picker.drive_agent_navigation(now));
    assert_eq!(
        picker.request.take(),
        Some(Selection {
            machine: choice.clone(),
            context: Some(ClientStartupTarget::Agent(Box::new(first.target)))
        })
    );
    picker.focus_agent(0, now);
    assert!(picker.drive_agent_navigation(now));
    assert_eq!(
        picker.request.take(),
        Some(Selection {
            machine: choice,
            context: Some(ClientStartupTarget::Agent(Box::new(second.target)))
        })
    );
    picker.agent_view.scope = Scope::Machine;
    picker.rebuild_workspaces(None);
    picker.focus_agent(0, now);
    assert!(picker.drive_agent_navigation(now));
    assert!(
        !picker.open && picker.request.is_none(),
        "filtered empty list has no fallback"
    );
    picker.observe_agents(vec![
        (Choice::Local, Agents::Unavailable),
        observations[1].clone(),
    ]);
    picker.focus_agent(0, now);
    assert!(picker.drive_agent_navigation(now));
    assert_eq!(picker.error, Some("agents-unavailable"));
    assert!(
        picker.request.is_none(),
        "unknown current source cannot select a foreign Agent"
    );
}

#[test]
fn ordinal_intent_waits_cancels_expires_and_absent_or_current_indices_do_not_attach() {
    use crate::machine_agents::test_support::checkpoint;
    let mut picker = picker();
    focus(&mut picker);
    let choice = Choice::Remote(profile(1, "remote", true).id);
    let now = Instant::now();
    picker.observe_agents(vec![
        (Choice::Local, Agents::Ready(checkpoint(vec![agent_row()]))),
        (choice.clone(), Agents::Loading),
    ]);
    picker.focus_agent(0, now);
    assert!(!picker.drive_agent_navigation(now));
    picker.key(KeyEvent::press(KeyCode::Esc, Modifiers::NONE));
    assert!(!picker.drive_agent_navigation(now));
    assert!(picker.request.is_none());
    picker.focus_agent(0, now);
    assert!(picker.drive_agent_navigation(now + Duration::from_secs(8)));
    assert!(picker.request.is_none());
    assert_eq!(picker.error, Some("agents-unavailable"));
    picker.observe_agents(vec![
        (Choice::Local, Agents::Ready(checkpoint(vec![agent_row()]))),
        (choice, Agents::Ready(checkpoint(Vec::new()))),
    ]);
    for index in [0, 8, u8::MAX] {
        picker.focus_agent(index, now);
        assert!(picker.drive_agent_navigation(now));
        assert!(
            !picker.open && picker.request.is_none(),
            "ordinal {index} must not reattach or wrap"
        );
    }
}

#[test]
fn active_source_view_drives_list_and_global_cycle_and_failed_source_never_clears_it() {
    use crate::machine_agents::{Observation as Agents, test_support::checkpoint};
    use orchbee_workbench_protocol::*;
    let remote = profile(1, "remote", true);
    let mut picker = Picker {
        available: true,
        ..Picker::default()
    };
    picker.replace(std::slice::from_ref(&remote));
    let mut idle = agent_row();
    idle.state = PanelAgentState::Idle;
    let mut blocked = agent_row();
    blocked.state = PanelAgentState::Blocked;
    blocked.target.member = TerminalPresentationId::parse(&format!("{:032x}", 24)).unwrap();
    blocked.target.physical = crate::machine_agents::test_support::physical(24);
    let mut local = checkpoint(vec![idle.clone(), blocked.clone()])
        .as_ref()
        .clone();
    local.view=AgentViewSnapshot{revision:WorkbenchRevision::new(1),spec:Some(parse_agent_view_spec(br#"{"source":"cli.views","label":"Blocked","filter":{"op":"eq","field":"status","value":"blocked"},"sort":[{"field":"attention","order":"desc"}]}"#).unwrap())};
    let observations = vec![
        (Choice::Local, Agents::Ready(std::sync::Arc::new(local))),
        (
            Choice::Remote(remote.id.clone()),
            Agents::Ready(checkpoint(vec![idle])),
        ),
    ];
    picker.observe_agents(observations.clone());
    picker.navigate_agent(false, Instant::now());
    let candidates: Vec<_> = picker
        .agent_rows
        .iter()
        .filter(|r| r.agent.is_some())
        .collect();
    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].agent.as_ref().unwrap().target.member,
        blocked.target.member
    );
    let text = paint(&picker, Locale::EnUs, 90, 24);
    assert!(text.contains("Blocked"));
    assert!(text.contains("Sort from view"));
    picker.agent_control(Control::Order);
    assert_eq!(picker.error, Some("agent-source-sort"));
    picker.navigate_agent(false, Instant::now());
    assert!(picker.drive_agent_navigation(Instant::now()));
    assert_eq!(
        picker.request.take().unwrap().context,
        Some(crate::startup_directory::ClientStartupTarget::Agent(
            Box::new(blocked.target)
        ))
    );
    picker.observe_agents(vec![
        (Choice::Local, Agents::Unavailable),
        observations[1].clone(),
    ]);
    assert!(
        picker.agent_rows.iter().all(|r| r.agent.is_none()),
        "unknown active source must not apply a cleared view to other machines"
    );
}

fn picker() -> Picker {
    let mut picker = Picker {
        available: true,
        ..Picker::default()
    };
    picker.replace(&[profile(1, "remote", true), profile(2, "disabled", false)]);
    picker.observe_agents(vec![
        (
            Choice::Local,
            Agents::Ready(crate::machine_agents::test_support::checkpoint(vec![
                agent_row(),
            ])),
        ),
        (
            Choice::Remote(profile(1, "remote", true).id),
            Agents::Ready(crate::machine_agents::test_support::checkpoint(vec![
                agent_row(),
            ])),
        ),
    ]);
    picker.show();
    picker.set_mode(Mode::Agents);
    picker
}

#[test]
fn unchanged_source_and_attention_versions_skip_projection_but_ack_reprojects_once() {
    let mut picker = picker();
    let sources = picker.agent_sources.clone();
    let rows = picker.agent_rows.clone();
    assert!(!picker.observe_agents(sources.clone()));
    assert!(std::sync::Arc::ptr_eq(&rows, &picker.agent_rows));
    assert!(picker.needs_agent_attention(Some((1, 0))));
    assert!(!picker.needs_agent_attention(Some((1, 0))));
    let row = agent_row();
    let attention = orchbee_workbench_protocol::ClientTerminalAgentAttentionSnapshot {
        report_revision: orchbee_workbench_protocol::WorkbenchRevision::new(2),
        outer_focused: None,
        entries: vec![
            orchbee_workbench_protocol::ClientTerminalAgentAttentionEntry {
                binding: row.target.physical,
                state: Some(
                    orchbee_workbench_protocol::ClientTerminalAgentAttentionVersion {
                        sequence: row.sequence,
                        seen: false,
                    },
                ),
            },
        ],
    };
    assert!(picker.observe_agent_attention(&[attention]));
    assert!(!picker.agent_rows[0].agent.as_ref().unwrap().seen);
    assert!(!picker.observe_agents(sources));
    assert!(!picker.agent_rows[0].agent.as_ref().unwrap().seen);
    picker.reset_agent_attention_read();
    assert!(
        picker.needs_agent_attention(Some((1, 0))),
        "a replacement Client cannot inherit an identical private version stamp"
    );
    picker.current = Choice::Remote(profile(1, "remote", true).id);
    assert!(picker.needs_agent_attention(Some((1, 0))));
}
fn focus(picker: &mut Picker) {
    let target = agent_row().target;
    picker.observe_focus(
        picker.current.clone(),
        Some(target.context),
        Some(ClientMemberTarget {
            tab: target.tab,
            panel: target.panel,
            presentation: target.member,
        }),
    );
}
#[test]
fn query_enter_escape_clear_and_bounds_never_activate_a_terminal() {
    let mut picker = picker();
    picker.show_agent_search();
    for c in "same terminal".chars() {
        picker.key(KeyEvent::press(KeyCode::Char(c), Modifiers::NONE));
    }
    assert_eq!(picker.agent_view.query, "same terminal");
    picker.key(KeyEvent::press(KeyCode::Enter, Modifiers::NONE));
    assert!(!picker.editing_query);
    assert!(picker.open);
    assert!(picker.request.is_none());
    picker.key(KeyEvent::press(KeyCode::Char('/'), Modifiers::NONE));
    picker.key(KeyEvent::press(KeyCode::Backspace, Modifiers::NONE));
    assert_eq!(picker.agent_view.query, "same termina");
    picker.key(KeyEvent::press(KeyCode::Char('u'), Modifiers::CTRL));
    for _ in 0..140 {
        picker.key(KeyEvent::press(KeyCode::Char('界'), Modifiers::NONE));
    }
    picker.key(KeyEvent::press(KeyCode::Char('\u{202e}'), Modifiers::NONE));
    assert_eq!(picker.agent_view.query.chars().count(), 128);
    let painted = paint(&picker, Locale::ZhCn, 50, 30);
    let cursor = painted
        .cursor
        .expect("the query editor owns the terminal cursor");
    let row = &painted.rows[usize::from(cursor.y)];
    let before: String = row
        .chars()
        .take(usize::from(cursor.x))
        .filter(|c| !c.is_whitespace())
        .collect();
    assert!(
        before.ends_with('界'),
        "the editor keeps its end visible for wide characters: {row:?}"
    );
    picker.key(KeyEvent::press(KeyCode::Esc, Modifiers::NONE));
    assert!(picker.open);
    assert!(!picker.editing_query);
    assert!(picker.request.is_none());
    picker.key(KeyEvent::press(KeyCode::Esc, Modifiers::NONE));
    assert!(!picker.open);
}
#[test]
fn drawn_controls_filter_without_reusing_old_mouse_authority() {
    let mut picker = picker();
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let text = paint(&picker, locale, 100, 30);
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        let prompt: String = locale
            .machine_picker_text("agent-search-prompt")
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        assert!(compact.contains(&format!("/{prompt}")), "{compact}");
    }
    let painted = paint(&picker, Locale::EnUs, 100, 30);
    let old_row = painted.target(PickerAction::Row(1));
    let state = painted.target(PickerAction::Control(Control::State));
    assert!(picker.activate_drawn_picker(&state));
    assert_eq!(
        picker.agent_view.state,
        Some(orchbee_workbench_protocol::MetadataState::Blocked)
    );
    assert!(!picker.activate_drawn_picker(&old_row));
    assert!(picker.request.is_none());
    assert!(picker.visible_rows().iter().all(|r| !r.enabled));
    assert!(paint(&picker, Locale::EnUs, 100, 30).contains("No matching agents"));
    picker.key(KeyEvent::press(KeyCode::Char('f'), Modifiers::NONE));
    assert_eq!(
        picker.agent_view.state,
        Some(orchbee_workbench_protocol::MetadataState::Error)
    );
    assert!(picker.visible_rows().iter().all(|r| !r.enabled));
    picker.key(KeyEvent::press(KeyCode::Char('f'), Modifiers::NONE));
    assert_eq!(
        picker.agent_view.state,
        Some(orchbee_workbench_protocol::MetadataState::Done)
    );
    assert!(picker.visible_rows().iter().all(|r| !r.enabled));
    picker.key(KeyEvent::press(KeyCode::Char('f'), Modifiers::NONE));
    assert_eq!(
        picker.visible_rows().iter().filter(|r| r.enabled).count(),
        2
    );
    let tiny = paint(&picker, Locale::ZhCn, 20, 5);
    assert!(!tiny.has_target(PickerAction::Control(Control::State)));
    let view = picker.agent_view.clone();
    assert!(!picker.activate_drawn_picker(&state));
    assert_eq!(picker.agent_view, view);
}
#[test]
fn view_scope_reprojects_on_current_focus_and_keeps_machine_identity() {
    let mut picker = picker();
    focus(&mut picker);
    picker.agent_control(Control::Scope);
    assert_eq!(picker.agent_view.scope, Scope::Machine);
    assert_eq!(
        picker.visible_rows().iter().filter(|r| r.enabled).count(),
        1
    );
    let context = picker.current_context.clone();
    let member = picker.current_member.clone();
    let remote = Choice::Remote(profile(1, "remote", true).id);
    assert!(picker.observe_focus(remote.clone(), context.clone(), member.clone()));
    assert_eq!(
        picker
            .visible_rows()
            .iter()
            .find(|r| r.enabled)
            .unwrap()
            .choice,
        remote
    );
    assert!(picker.observe_focus(Choice::Local, context, member));
    picker.agent_control(Control::Scope);
    assert_eq!(picker.agent_view.scope, Scope::Context);
    assert!(picker.current_agent(&Choice::Local, &agent_row()));
    assert!(!picker.current_agent(&Choice::Remote(profile(1, "remote", true).id), &agent_row()));
    picker.observe_focus(picker.current.clone(), None, None);
    assert!(picker.visible_rows().iter().all(|r| !r.enabled));
    picker.agent_control(Control::Scope);
    assert_eq!(
        picker.visible_rows().iter().filter(|r| r.enabled).count(),
        2
    );
}
#[test]
fn global_agent_navigation_waits_wraps_and_uses_filtered_stable_order() {
    let mut picker = picker();
    focus(&mut picker);
    let remote = Choice::Remote(profile(1, "remote", true).id);
    let now = Instant::now();
    picker.navigate_agent(false, now);
    picker.observe_agents(vec![
        (
            Choice::Local,
            Agents::Ready(crate::machine_agents::test_support::checkpoint(vec![
                agent_row(),
            ])),
        ),
        (remote.clone(), Agents::Loading),
    ]);
    assert!(!picker.drive_agent_navigation(now));
    assert!(picker.request.is_none());
    let mut blocked = agent_row();
    blocked.state = orchbee_workbench_protocol::PanelAgentState::Blocked;
    blocked.sequence = orchbee_workbench_protocol::WorkbenchRevision::new(2);
    picker.agent_view.order = Order::Priority;
    picker.observe_agents(vec![
        (
            Choice::Local,
            Agents::Ready(crate::machine_agents::test_support::checkpoint(vec![
                agent_row(),
            ])),
        ),
        (
            remote.clone(),
            Agents::Ready(crate::machine_agents::test_support::checkpoint(vec![
                blocked.clone(),
            ])),
        ),
    ]);
    assert!(picker.drive_agent_navigation(now));
    assert_eq!(
        picker.request.take(),
        Some(Selection {
            machine: remote.clone(),
            context: Some(ClientStartupTarget::Agent(Box::new(blocked.target.clone())))
        })
    );
    picker.navigate_agent(true, now);
    assert!(picker.drive_agent_navigation(now));
    assert_eq!(picker.request.take().unwrap().machine, remote);
    picker.agent_view.state = Some(orchbee_workbench_protocol::MetadataState::Working);
    picker.rebuild_workspaces(None);
    picker.navigate_agent(false, now);
    assert!(picker.drive_agent_navigation(now));
    assert!(!picker.open);
    assert!(
        picker.request.is_none(),
        "only current eligible member must not reattach"
    );
}
#[test]
fn agent_navigation_cancels_expires_and_never_selects_unavailable_or_empty_rows() {
    let mut picker = picker();
    let now = Instant::now();
    picker.navigate_agent(false, now);
    picker.observe_agents(vec![(Choice::Local, Agents::Loading)]);
    assert!(!picker.drive_agent_navigation(now));
    picker.key(KeyEvent::press(KeyCode::Esc, Modifiers::NONE));
    assert!(!picker.drive_agent_navigation(now));
    assert!(picker.request.is_none());
    picker.navigate_agent(false, now);
    assert!(picker.drive_agent_navigation(now + Duration::from_secs(8)));
    assert_eq!(picker.error, Some("agents-unavailable"));
    assert!(picker.request.is_none());
    for observation in [
        Agents::Unavailable,
        Agents::Ready(crate::machine_agents::test_support::checkpoint(Vec::new())),
    ] {
        picker.observe_agents(vec![(Choice::Local, observation)]);
        picker.navigate_agent(true, now);
        assert!(picker.drive_agent_navigation(now));
        assert!(picker.request.is_none());
    }
    picker.observe_agents(vec![(
        Choice::Local,
        Agents::Ready(crate::machine_agents::test_support::checkpoint(vec![
            agent_row(),
        ])),
    )]);
    for reverse in [false, true] {
        picker.navigate_agent(reverse, now);
        assert!(picker.drive_agent_navigation(now));
        assert_eq!(picker.request.take().unwrap().machine, Choice::Local);
    }
}
#[test]
fn sorting_preserves_selected_physical_identity_and_invalidates_pointer_hits() {
    let mut picker = picker();
    picker.selected = 1;
    let selected = picker.selection();
    let painted = paint(&picker, Locale::EnUs, 100, 30);
    let old = painted.target(PickerAction::Row(1));
    let mut other = agent_row();
    other.state = orchbee_workbench_protocol::PanelAgentState::Blocked;
    other.sequence = orchbee_workbench_protocol::WorkbenchRevision::new(2);
    picker.observe_agents(vec![
        (
            Choice::Local,
            Agents::Ready(crate::machine_agents::test_support::checkpoint(vec![other])),
        ),
        (
            Choice::Remote(profile(1, "remote", true).id),
            Agents::Ready(crate::machine_agents::test_support::checkpoint(vec![
                agent_row(),
            ])),
        ),
    ]);
    picker.agent_control(Control::Order);
    assert_eq!(picker.selection(), selected);
    assert!(!picker.activate_drawn_picker(&old));
    assert!(picker.request.is_none());
}

#[test]
fn agent_metadata_is_visible_without_changing_activation_identity() {
    use orchbee_workbench_protocol::{
        MetadataState, MetadataTokenKey, MetadataTokenValue, MetadataTokens,
    };
    let mut picker = picker();
    let mut row = agent_row();
    row.metadata.title = Some(MetadataTokenValue::parse("Fix docs").unwrap());
    row.metadata.display_agent = Some(MetadataTokenValue::parse("Friendly Agent").unwrap());
    row.metadata.state_labels.insert(
        MetadataState::Working,
        MetadataTokenValue::parse("Reviewing").unwrap(),
    );
    row.tokens = MetadataTokens::new(
        [(
            MetadataTokenKey::parse("issue").unwrap(),
            MetadataTokenValue::parse("ORCH-42").unwrap(),
        )]
        .into(),
    )
    .unwrap();
    picker.observe_agents(vec![(
        Choice::Local,
        Agents::Ready(crate::machine_agents::test_support::checkpoint(vec![
            row.clone(),
        ])),
    )]);
    let text = paint(&picker, Locale::EnUs, 100, 30);
    for label in ["Fix docs", "Friendly Agent", "Reviewing", "issue: ORCH-42"] {
        assert!(text.contains(label), "{label}: {}", text.text.as_str());
    }
    let target = text.target(PickerAction::Row(0));
    assert!(picker.activate_drawn_picker(&target));
    assert_eq!(
        picker.request.take(),
        Some(Selection {
            machine: Choice::Local,
            context: Some(ClientStartupTarget::Agent(Box::new(row.target)))
        })
    );
}
