use super::*;
use crate::machine_agents::{
    Observation as Agents,
    view::{Order, Scope},
};
use std::time::{Duration, Instant};

fn picker() -> Picker {
    let mut picker = Picker {
        available: true,
        ..Picker::default()
    };
    picker.replace(&[profile(1, "remote", true), profile(2, "disabled", false)]);
    picker.observe_agents(vec![
        (Choice::Local, Agents::Ready(vec![agent_row()].into())),
        (
            Choice::Remote(profile(1, "remote", true).id),
            Agents::Ready(vec![agent_row()].into()),
        ),
    ]);
    picker.show();
    picker.set_mode(Mode::Agents);
    picker
}
fn focus(picker: &mut Picker) {
    let target = agent_row().target;
    picker.observe_focus(
        picker.current.clone(),
        Some(target.context),
        Some(NativeMemberTarget {
            tab: target.tab,
            panel: target.panel,
            presentation: target.member,
        }),
    );
}
#[test]
fn query_enter_escape_clear_and_bounds_never_activate_a_terminal() {
    let mut picker = picker();
    picker.key(KeyEvent::press(KeyCode::Char('/'), Modifiers::NONE));
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
    assert!(
        paint(&mut picker, Locale::ZhCn, 50, 30).contains("▏"),
        "the editor keeps its end visible for wide characters"
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
        let text = paint(&mut picker, locale, 100, 30);
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(compact.contains(locale.machine_picker_text("agent-search")));
    }
    let old_row = picker.painted[1].0;
    let state = picker
        .control_hits
        .iter()
        .find(|(_, c)| *c == Control::State)
        .unwrap()
        .0;
    picker.pointer(click(state));
    assert_eq!(
        picker.agent_view.state,
        Some(orchbee_workbench_protocol::PanelAgentState::Blocked)
    );
    assert!(picker.painted.is_empty());
    assert!(picker.control_hits.is_empty());
    picker.pointer(click(old_row));
    assert!(picker.request.is_none());
    assert!(picker.visible_rows().iter().all(|r| !r.enabled));
    assert!(paint(&mut picker, Locale::EnUs, 100, 30).contains("No matching agents"));
    picker.key(KeyEvent::press(KeyCode::Char('f'), Modifiers::NONE));
    assert_eq!(
        picker.visible_rows().iter().filter(|r| r.enabled).count(),
        2
    );
    let old_control = picker.control_hits.first().map(|v| v.0).unwrap_or(state);
    paint(&mut picker, Locale::ZhCn, 20, 5);
    assert!(picker.painted.is_empty());
    assert!(picker.control_hits.is_empty());
    let view = picker.agent_view.clone();
    picker.pointer(click(old_control));
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
        (Choice::Local, Agents::Ready(vec![agent_row()].into())),
        (remote.clone(), Agents::Loading),
    ]);
    assert!(!picker.drive_agent_navigation(now));
    assert!(picker.request.is_none());
    let mut blocked = agent_row();
    blocked.state = orchbee_workbench_protocol::PanelAgentState::Blocked;
    picker.agent_view.order = Order::Priority;
    picker.observe_agents(vec![
        (Choice::Local, Agents::Ready(vec![agent_row()].into())),
        (remote.clone(), Agents::Ready(vec![blocked.clone()].into())),
    ]);
    assert!(picker.drive_agent_navigation(now));
    assert_eq!(
        picker.request.take(),
        Some(Selection {
            machine: remote.clone(),
            context: Some(NativeStartupTarget::Agent(Box::new(blocked.target.clone())))
        })
    );
    picker.navigate_agent(true, now);
    assert!(picker.drive_agent_navigation(now));
    assert_eq!(picker.request.take().unwrap().machine, remote);
    picker.agent_view.state = Some(orchbee_workbench_protocol::PanelAgentState::Working);
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
    for observation in [Agents::Unavailable, Agents::Ready(std::sync::Arc::from([]))] {
        picker.observe_agents(vec![(Choice::Local, observation)]);
        picker.navigate_agent(true, now);
        assert!(picker.drive_agent_navigation(now));
        assert!(picker.request.is_none());
    }
    picker.observe_agents(vec![(
        Choice::Local,
        Agents::Ready(vec![agent_row()].into()),
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
    paint(&mut picker, Locale::EnUs, 100, 30);
    let old = picker.painted[1].0;
    let mut other = agent_row();
    other.state = orchbee_workbench_protocol::PanelAgentState::Blocked;
    picker.observe_agents(vec![
        (Choice::Local, Agents::Ready(vec![other].into())),
        (
            Choice::Remote(profile(1, "remote", true).id),
            Agents::Ready(vec![agent_row()].into()),
        ),
    ]);
    picker.agent_control(Control::Order);
    assert_eq!(picker.selection(), selected);
    assert!(picker.painted.is_empty());
    picker.pointer(click(old));
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
        Agents::Ready(vec![row.clone()].into()),
    )]);
    let text = paint(&mut picker, Locale::EnUs, 100, 30);
    for label in ["Fix docs", "Friendly Agent", "Reviewing", "issue: ORCH-42"] {
        assert!(text.contains(label), "{label}: {text}");
    }
    picker.pointer(click(picker.painted[0].0));
    assert_eq!(
        picker.request.take(),
        Some(Selection {
            machine: Choice::Local,
            context: Some(NativeStartupTarget::Agent(Box::new(row.target)))
        })
    );
}
