use super::*;
use crate::machines::Draft;
use crate::tui_pointer::FrameHitMap;
use std::ops::Deref;
fn profile(n: u8, label: &str, enabled: bool) -> Profile {
    Profile {
        id: ProfileId::parse(&format!("{n:032x}")).unwrap(),
        draft: Draft::parse(label, "example.invalid", "default").unwrap(),
        enabled,
    }
}
struct Paint {
    text: String,
    rows: Vec<String>,
    hits: FrameHitMap,
    width: u16,
    height: u16,
}

impl Deref for Paint {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.text
    }
}

impl Paint {
    fn target(&self, action: PickerAction) -> DrawnPickerTarget {
        for row in 0..self.height {
            for column in 0..self.width {
                if let Some(HitTarget::MachinePicker(target)) = self.hits.hit_target_at(column, row)
                    && target.action() == action
                {
                    return target.clone();
                }
            }
        }
        panic!("missing painted Machine picker target: {action:?}");
    }

    fn has_target(&self, action: PickerAction) -> bool {
        for row in 0..self.height {
            for column in 0..self.width {
                if matches!(
                    self.hits.hit_target_at(column, row),
                    Some(HitTarget::MachinePicker(target)) if target.action() == action
                ) {
                    return true;
                }
            }
        }
        false
    }

    fn has_any_target(&self) -> bool {
        (0..self.height)
            .any(|row| (0..self.width).any(|column| self.hits.hit_target_at(column, row).is_some()))
    }
}

fn paint(picker: &Picker, locale: Locale, width: u16, height: u16) -> Paint {
    let backend = ratatui::backend::TestBackend::new(width, height);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let mut hits = HitMapBuilder::new();
    terminal
        .draw(|frame| {
            picker.render(frame, locale, &Theme::terminal_native(), &mut hits);
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    let rows = (0..height)
        .map(|row| {
            (0..width)
                .map(|column| buffer[(column, row)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>();
    let text = rows.concat();
    Paint {
        text,
        rows,
        hits: hits.finish(),
        width,
        height,
    }
}

#[test]
fn picker_wheel_is_regional_bounded_reversible_and_keyboard_can_resume() {
    let mut picker = Picker {
        available: true,
        ..Picker::default()
    };
    picker.replace(
        &(1..=60)
            .map(|n| profile(n, "same", true))
            .collect::<Vec<_>>(),
    );
    picker.show();
    let source = picker.picker_source().unwrap();
    let initial = source.drawn_list(picker.picker_window(7)).unwrap();
    let second = picker
        .scroll_drawn_picker(&initial, false, 15)
        .expect("one flush moves at most the painted viewport");
    assert_eq!(second.window().top, 7);
    assert!(
        picker.scroll_drawn_picker(&initial, false, 3).is_none(),
        "the previous viewport is stale after the first settled chunk"
    );
    let third = picker.scroll_drawn_picker(&second, false, 3).unwrap();
    assert_eq!(third.window().top, 10);
    let reversed = picker.scroll_drawn_picker(&third, true, 3).unwrap();
    assert_eq!(reversed.window().top, 7);
    picker.key(KeyEvent::press(KeyCode::Home, Modifiers::NONE));
    assert_eq!(picker.picker_window(7).top, 0);
}

#[test]
fn picker_categories_query_controls_and_close_are_visible_mouse_paths() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for (width, height) in [(38, 14), (80, 24), (120, 40)] {
            let mut picker = Picker {
                available: true,
                ..Picker::default()
            };
            picker.show();
            let painted = paint(&picker, locale, width, height);
            for mode in [Mode::Machines, Mode::Workspaces, Mode::Agents] {
                assert!(painted.has_target(PickerAction::Mode(mode)));
            }
            let agents = painted.target(PickerAction::Mode(Mode::Agents));
            assert!(picker.activate_drawn_picker(&agents));
            let painted = paint(&picker, locale, width, height);
            let search = painted.target(PickerAction::Control(Control::Search));
            assert!(picker.activate_drawn_picker(&search));
            picker.key(KeyEvent::press(KeyCode::Char('z'), Modifiers::NONE));
            let painted = paint(&picker, locale, width, height);
            let clear = painted.target(PickerAction::Control(Control::Clear));
            assert!(picker.activate_drawn_picker(&clear));
            assert!(picker.agent_view.query.is_empty());
            let painted = paint(&picker, locale, width, height);
            let done = painted.target(PickerAction::Control(Control::Done));
            assert!(picker.activate_drawn_picker(&done));
            assert!(!picker.editing_query);
            let painted = paint(&picker, locale, width, height);
            let close = painted.target(PickerAction::Close);
            assert!(picker.activate_drawn_picker(&close));
            assert!(!picker.open);
            assert!(picker.selection_scope().is_none());
        }
    }
}

#[test]
fn minimal_picker_retains_a_mouse_exit_when_only_the_close_control_fits() {
    for (width, height) in [(20, 5), (33, 8)] {
        let mut picker = Picker {
            available: true,
            ..Picker::default()
        };
        picker.show();
        let painted = paint(&picker, Locale::EnUs, width, height);
        assert!(painted.contains('x'));
        let close = painted.target(PickerAction::Close);
        assert!(picker.activate_drawn_picker(&close));
        assert!(!picker.open);
    }
}

#[test]
fn too_small_picker_paints_only_an_atomic_escape_hint_without_publishing_targets() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for (width, height) in [(100, 1), (100, 3), (100, 4), (19, 24)] {
            let mut picker = Picker {
                available: true,
                ..Picker::default()
            };
            picker.show();
            let source = picker.picker_source();
            let painted = paint(&picker, locale, width, height);

            assert!(painted.contains("Esc"), "{locale:?} {width}x{height}");
            assert!(!painted.has_any_target(), "{locale:?} {width}x{height}");
            assert!(
                painted.rows[..usize::from(height.saturating_sub(1))]
                    .iter()
                    .all(|row| row.trim().is_empty()),
                "{locale:?} {width}x{height} painted outside the capture status"
            );
            assert_eq!(picker.picker_source(), source);
            assert!(picker.open);
            assert!(picker.selection_scope().is_some());
        }

        let mut editing = Picker {
            available: true,
            ..Picker::default()
        };
        editing.show_agent_search();
        let painted = paint(&editing, locale, 19, 24);
        assert!(painted.contains("Esc"), "{locale:?}: {:?}", painted.text);
        assert!(
            !painted.contains("Enter") && !painted.contains("Ctrl"),
            "{locale:?}: only the first complete hint fits: {:?}",
            painted.text
        );
    }

    let mut picker = Picker {
        available: true,
        ..Picker::default()
    };
    picker.show();
    let _ = paint(&picker, Locale::EnUs, 19, 24);
    picker.key(KeyEvent::press(KeyCode::Esc, Modifiers::NONE));
    assert!(!picker.open);
    assert!(picker.selection_scope().is_none());
}
#[test]
fn rendered_rows_select_exact_ids_and_reordering_invalidates_old_hits() {
    let a = profile(1, "same", true);
    let b = profile(2, "same", true);
    let mut picker = Picker {
        available: true,
        ..Picker::default()
    };
    picker.replace(&[a.clone(), b.clone()]);
    picker.show();
    let text = paint(&picker, Locale::EnUs, 100, 30);
    assert!(text.contains("Machines"));
    let old = text.target(PickerAction::Row(2));
    picker.replace(&[b.clone(), a]);
    assert!(!picker.activate_drawn_picker(&old));
    assert!(picker.request.is_none());
    let text = paint(&picker, Locale::ZhCn, 100, 30);
    assert!(
        text.chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .contains("机器")
    );
    let current_b = text.target(PickerAction::Row(1));
    assert!(picker.activate_drawn_picker(&current_b));
    assert_eq!(picker.request.take(), Some(Choice::Remote(b.id).into()));
}
#[test]
fn disabled_selection_and_small_resize_never_fall_through_to_a_terminal() {
    let mut picker = Picker {
        available: true,
        ..Picker::default()
    };
    picker.replace(&[profile(1, "offline", false)]);
    picker.show();
    let painted = paint(&picker, Locale::EnUs, 80, 24);
    let disabled = painted.target(PickerAction::Row(1));
    assert!(!disabled.available());
    assert!(!picker.activate_drawn_picker(&disabled));
    assert_eq!(picker.error, None);
    assert!(picker.request.is_none());
    let tiny = paint(&picker, Locale::ZhCn, 1, 1);
    assert!(!tiny.has_target(PickerAction::Row(0)));
    assert!(!tiny.has_target(PickerAction::Row(1)));
}
#[test]
fn machine_overview_keeps_status_visible_and_localizes_without_claiming_stale_online() {
    let p = profile(1, &"long-name".repeat(10), true);
    let mut picker = Picker {
        available: true,
        ..Picker::default()
    };
    picker.replace(std::slice::from_ref(&p));
    picker.show();
    picker.selected = 1;
    let overview = crate::machine_discovery::Overview {
        site: orchbee_workbench_protocol::SiteId::parse(p.id.as_str()).unwrap(),
        revision: orchbee_workbench_protocol::WorkbenchRevision::new(1),
        projects: 2,
        workspaces: 3,
        tabs: 7,
        labels: vec!["Project A".into(), "Workspace B".into()],
        contexts: std::sync::Arc::from([]),
    };
    picker.observe(&[(p.id.clone(), Observation::Online(overview))]);
    let text = paint(&picker, Locale::EnUs, 80, 24);
    assert!(text.contains("Online"));
    assert!(text.contains("3 workspaces"));
    assert!(text.contains("Project A"));
    let text = paint(&picker, Locale::ZhCn, 80, 24);
    assert!(
        text.chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .contains("在线")
    );
    let status_only = text.target(PickerAction::Row(1));
    picker.observe(&[(p.id, Observation::Stale)]);
    assert!(
        picker.activate_drawn_picker(&status_only),
        "status-only refreshes retain the same semantic row authority"
    );
    assert!(picker.request.take().is_some());
    let text = paint(&picker, Locale::EnUs, 80, 24);
    assert!(text.contains("Stale"));
    assert!(text.contains("0 machines"));
    assert!(!text.contains("Workspace B"));
}
fn context(n: u8) -> Context {
    Context {
        target: NativeContextTarget {
            site_id: orchbee_workbench_protocol::SiteId::parse(&format!("{:032x}", 9)).unwrap(),
            location: crate::terminal_session::NativeContextLocation::Standalone(
                orchbee_workbench_protocol::WorkbenchTerminalContextId::parse(&format!("{n:032x}"))
                    .unwrap(),
            ),
        },
        project_id: None,
        project: None,
        workspace: Some("same workspace".into()),
    }
}
#[test]
fn workspace_mode_routes_full_local_remote_identity_and_preserves_selection_on_reorder() {
    let a = profile(1, "same machine", true);
    let b = profile(2, "same machine", true);
    let mut picker = Picker {
        available: true,
        ..Picker::default()
    };
    picker.replace(&[a.clone(), b.clone()]);
    picker.observe_local(vec![context(11)]);
    let overview = crate::machine_discovery::Overview {
        site: context(11).target.site_id,
        revision: orchbee_workbench_protocol::WorkbenchRevision::new(1),
        projects: 1,
        workspaces: 2,
        tabs: 0,
        labels: Vec::new(),
        contexts: vec![context(11), context(12)].into(),
    };
    picker.observe(&[
        (a.id.clone(), Observation::Online(overview.clone())),
        (b.id.clone(), Observation::Online(overview.clone())),
    ]);
    picker.show();
    picker.key(KeyEvent::press(KeyCode::Tab, Modifiers::NONE));
    assert_eq!(picker.mode, Mode::Workspaces);
    assert_eq!(picker.visible_rows().len(), 5);
    assert!(paint(&picker, Locale::EnUs, 90, 26).contains("same workspace"));
    for _ in 1..picker.visible_rows().len() {
        picker.key(KeyEvent::press(KeyCode::Down, Modifiers::NONE));
    }
    picker.key(KeyEvent::press(KeyCode::Enter, Modifiers::NONE));
    let expected = Selection {
        machine: Choice::Remote(b.id.clone()),
        context: Some(context(12).target.into()),
    };
    assert_eq!(picker.request.take(), Some(expected.clone()));
    let mut reordered = overview.clone();
    reordered.contexts = vec![context(12), context(11)].into();
    picker.observe(&[
        (a.id.clone(), Observation::Online(overview)),
        (b.id.clone(), Observation::Online(reordered)),
    ]);
    assert_eq!(picker.selection(), Some(expected));
    let painted = paint(&picker, Locale::EnUs, 90, 26);
    let old = painted.target(PickerAction::Row(picker.selected));
    picker.observe(&[(a.id, Observation::Stale), (b.id, Observation::Stale)]);
    assert!(!picker.activate_drawn_picker(&old));
    assert!(picker.request.is_none());
    assert_eq!(picker.visible_rows().len(), 1);
    let painted = paint(&picker, Locale::ZhCn, 90, 26);
    let local = painted.target(PickerAction::Row(0));
    assert!(picker.activate_drawn_picker(&local));
    assert_eq!(
        picker.request.take(),
        Some(Selection {
            machine: Choice::Local,
            context: Some(context(11).target.into())
        })
    );
    let painted = paint(&picker, Locale::EnUs, 90, 26);
    let machines = painted.target(PickerAction::Mode(Mode::Machines));
    assert!(picker.activate_drawn_picker(&machines));
    assert_eq!(picker.mode, Mode::Machines);
}
#[test]
fn workspace_mode_scrolls_and_refuses_stale_hits_after_small_resize() {
    let mut picker = Picker {
        available: true,
        ..Picker::default()
    };
    picker.observe_local((1..=30).map(context).collect());
    picker.show();
    let painted = paint(&picker, Locale::EnUs, 80, 24);
    let workspaces = painted.target(PickerAction::Mode(Mode::Workspaces));
    assert!(picker.activate_drawn_picker(&workspaces));
    for _ in 1..picker.visible_rows().len() {
        picker.key(KeyEvent::press(KeyCode::Down, Modifiers::NONE));
    }
    let text = paint(&picker, Locale::EnUs, 80, 24);
    assert!(text.contains("30/30"));
    let last = text.target(PickerAction::Row(29));
    assert_eq!(
        last.source().0.rows[29].selection.context,
        Some(context(30).target.into())
    );
    let tiny = paint(&picker, Locale::ZhCn, 20, 5);
    assert!(!tiny.has_target(PickerAction::Row(29)));
    assert!(picker.request.is_none());
    picker.observe_local(Vec::new());
    assert!(paint(&picker, Locale::EnUs, 80, 24).contains("No available workspaces"));
}
fn agent_row() -> crate::machine_agents::Row {
    crate::machine_agents::Row {
        seen: true,
        order: crate::machine_agents::PlacementOrder::default(),
        target: crate::machine_agents::tests::target(),
        label: "same terminal".into(),
        workspace: "project / workspace".into(),
        tab_label: None,
        terminal_title: orchbee_workbench_protocol::TerminalTitleObservation::Unavailable {},
        state: orchbee_workbench_protocol::PanelAgentState::Working,
        sequence: orchbee_workbench_protocol::WorkbenchRevision::new(1),
        metadata: orchbee_workbench_protocol::PanelMetadataEffective::default(),
        tokens: orchbee_workbench_protocol::MetadataTokens::default(),
    }
}
#[test]
fn agent_mode_routes_duplicate_local_and_remote_identities_and_invalidates_stale_hits() {
    use crate::machine_agents::Observation as Agents;
    let a = profile(1, "same machine", true);
    let b = profile(2, "same machine", true);
    let mut picker = Picker {
        available: true,
        ..Picker::default()
    };
    picker.replace(&[a.clone(), b.clone()]);
    let rows = crate::machine_agents::tests::checkpoint(vec![agent_row()]);
    picker.observe_agents(vec![
        (Choice::Local, Agents::Ready(rows.clone())),
        (Choice::Remote(a.id.clone()), Agents::Ready(rows.clone())),
        (Choice::Remote(b.id.clone()), Agents::Ready(rows)),
    ]);
    picker.show();
    picker.key(KeyEvent::press(KeyCode::Tab, Modifiers::NONE));
    picker.key(KeyEvent::press(KeyCode::Tab, Modifiers::NONE));
    assert!(picker.wants_agents());
    assert_eq!(picker.visible_rows().len(), 3);
    let text = paint(&picker, Locale::EnUs, 100, 30);
    assert!(text.contains("Working"));
    let remote_b = text.target(PickerAction::Row(2));
    assert!(picker.activate_drawn_picker(&remote_b));
    let expected = Selection {
        machine: Choice::Remote(b.id.clone()),
        context: Some(NativeStartupTarget::Agent(Box::new(agent_row().target))),
    };
    assert_eq!(picker.request.take(), Some(expected.clone()));
    picker.replace(&[b.clone(), a.clone()]);
    assert_eq!(picker.selection(), Some(expected));
    let painted = paint(&picker, Locale::ZhCn, 100, 30);
    let old = painted.target(PickerAction::Row(picker.selected));
    picker.observe_agents(vec![(
        Choice::Local,
        Agents::Ready(crate::machine_agents::tests::checkpoint(vec![agent_row()])),
    )]);
    assert!(!picker.activate_drawn_picker(&old));
    assert!(
        picker.request.is_none(),
        "old frame is no authority for activation"
    );
    picker.key(KeyEvent::press(KeyCode::Home, Modifiers::NONE));
    picker.key(KeyEvent::press(KeyCode::Enter, Modifiers::NONE));
    assert_eq!(picker.request.take().unwrap().machine, Choice::Local);
    picker.key(KeyEvent::press(KeyCode::Tab, Modifiers::NONE));
    assert_eq!(picker.mode, Mode::Machines);
}
#[test]
fn agent_mode_distinguishes_empty_loading_and_unavailable_without_fallback_selection() {
    use crate::machine_agents::Observation as Agents;
    let mut picker = Picker {
        available: true,
        ..Picker::default()
    };
    picker.replace(&[profile(1, "remote", true)]);
    picker.show();
    let painted = paint(&picker, Locale::EnUs, 100, 30);
    let agents = painted.target(PickerAction::Mode(Mode::Agents));
    assert!(picker.activate_drawn_picker(&agents));
    for (observation, key) in [
        (Agents::Loading, "agents-loading"),
        (
            Agents::Ready(crate::machine_agents::tests::checkpoint(Vec::new())),
            "no-agents",
        ),
        (Agents::Unavailable, "agents-unavailable"),
    ] {
        picker.observe_agents(vec![(Choice::Local, observation)]);
        picker.key(KeyEvent::press(KeyCode::Home, Modifiers::NONE));
        picker.key(KeyEvent::press(KeyCode::Enter, Modifiers::NONE));
        assert!(picker.request.is_none());
        assert_eq!(picker.error, Some(key));
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let text = paint(&picker, locale, 100, 30);
            let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
            let expected: String = locale
                .machine_picker_text(key)
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();
            assert!(compact.contains(&expected));
        }
    }
    let old = paint(&picker, Locale::EnUs, 100, 30).target(PickerAction::Row(0));
    let tiny = paint(&picker, Locale::ZhCn, 20, 5);
    assert!(!tiny.has_target(PickerAction::Row(0)));
    assert!(!picker.activate_drawn_picker(&old));
    assert!(picker.request.is_none());
}

mod agents;
mod sidebar;

mod navigation;
mod spaces;
