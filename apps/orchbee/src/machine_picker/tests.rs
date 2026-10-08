use super::*;
use crate::machines::Draft;
fn profile(n: u8, label: &str, enabled: bool) -> Profile {
    Profile {
        id: ProfileId::parse(&format!("{n:032x}")).unwrap(),
        draft: Draft::parse(label, "example.invalid", "default").unwrap(),
        enabled,
    }
}
fn paint(picker: &mut Picker, locale: Locale, width: u16, height: u16) -> String {
    let backend = ratatui::backend::TestBackend::new(width, height);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            let indicator = picker.indicator_area(locale, Rect::new(0, height - 1, width, 1));
            picker.render(frame, locale, &Theme::terminal_native(), indicator);
        })
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect()
}
fn click(rect: Rect) -> MouseEvent {
    MouseEvent {
        action: MouseAction::Press,
        button: MouseButton::Left,
        mods: Modifiers::NONE,
        column: rect.x,
        row: rect.y,
    }
}
#[test]
fn indicator_uses_display_width_keeps_its_chevron_and_revokes_hidden_hits() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let mut picker = Picker {
            available: true,
            current: Choice::Remote(profile(1, "remote", true).id),
            current_label: "远程开发机器".repeat(12),
            ..Picker::default()
        };
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).unwrap();
        let indicator = picker
            .indicator_area(locale, Rect::new(4, 23, 70, 1))
            .unwrap();
        assert_eq!(indicator, Rect::new(46, 23, 28, 1));
        let theme = Theme::one_light();
        terminal
            .draw(|frame| picker.render(frame, locale, &theme, Some(indicator)))
            .unwrap();
        let row: String = (indicator.x..indicator.right())
            .map(|x| terminal.backend().buffer()[(x, 23)].symbol())
            .collect();
        assert!(row.contains("…"));
        assert!(row.contains("▾"));
        let mut x = indicator.x;
        while x < indicator.right() {
            let cell = &terminal.backend().buffer()[(x, 23)];
            assert_eq!(cell.bg, theme.color(Role::SurfaceChrome));
            assert_eq!(cell.fg, theme.color(Role::AccentFocus));
            x += u16::try_from(display_width(cell.symbol()).max(1)).unwrap();
        }
        terminal
            .draw(|frame| picker.render(frame, locale, &theme, None))
            .unwrap();
        assert!(!picker.pointer(click(indicator)));
        assert!(!picker.open);
    }
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
    let text = paint(&mut picker, Locale::EnUs, 100, 30);
    assert!(text.contains("Machines"));
    let rect = picker.painted[2].0;
    picker.pointer(click(rect));
    assert_eq!(
        picker.request.take(),
        Some(Choice::Remote(b.id.clone()).into())
    );
    picker.replace(&[b.clone(), a]);
    picker.pointer(click(rect));
    assert!(picker.request.is_none());
    assert!(
        paint(&mut picker, Locale::ZhCn, 100, 30)
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .contains("机器")
    );
    picker.pointer(click(picker.painted[1].0));
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
    paint(&mut picker, Locale::EnUs, 80, 24);
    assert!(picker.pointer(click(picker.painted[1].0)));
    assert_eq!(picker.error, Some("disabled"));
    assert!(picker.request.is_none());
    picker.invalidate();
    paint(&mut picker, Locale::ZhCn, 1, 1);
    assert!(picker.painted.is_empty());
    assert!(picker.pointer(click(Rect::new(0, 0, 1, 1))));
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
    let text = paint(&mut picker, Locale::EnUs, 80, 24);
    assert!(text.contains("Online"));
    assert!(text.contains("3 workspaces"));
    assert!(text.contains("Project A"));
    let text = paint(&mut picker, Locale::ZhCn, 80, 24);
    assert!(
        text.chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .contains("在线")
    );
    let rect = picker.painted[1].0;
    picker.observe(&[(p.id, Observation::Stale)]);
    assert!(picker.pointer(click(rect)));
    assert!(
        picker.request.is_none(),
        "old painted rows are invalid after a status update"
    );
    let text = paint(&mut picker, Locale::EnUs, 80, 24);
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
    assert!(paint(&mut picker, Locale::EnUs, 90, 26).contains("same workspace"));
    picker.key(KeyEvent::press(KeyCode::End, Modifiers::NONE));
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
    paint(&mut picker, Locale::EnUs, 90, 26);
    let old = picker.painted[picker.selected].0;
    picker.observe(&[(a.id, Observation::Stale), (b.id, Observation::Stale)]);
    picker.pointer(click(old));
    assert!(picker.request.is_none());
    assert_eq!(picker.visible_rows().len(), 1);
    paint(&mut picker, Locale::ZhCn, 90, 26);
    picker.pointer(click(picker.painted[0].0));
    assert_eq!(
        picker.request.take(),
        Some(Selection {
            machine: Choice::Local,
            context: Some(context(11).target.into())
        })
    );
    picker.pointer(click(picker.mode_hits[0].0));
    assert_eq!(picker.mode, Mode::Machines);
    assert!(picker.painted.is_empty());
}
#[test]
fn workspace_mode_scrolls_and_refuses_stale_hits_after_small_resize() {
    let mut picker = Picker {
        available: true,
        ..Picker::default()
    };
    picker.observe_local((1..=30).map(context).collect());
    picker.show();
    paint(&mut picker, Locale::EnUs, 80, 24);
    picker.pointer(click(picker.mode_hits[1].0));
    picker.key(KeyEvent::press(KeyCode::End, Modifiers::NONE));
    let text = paint(&mut picker, Locale::EnUs, 80, 24);
    assert!(text.contains("30/30"));
    let last = picker.painted.last().unwrap().0;
    assert_eq!(
        picker.painted.last().unwrap().1.context,
        Some(context(30).target.into())
    );
    paint(&mut picker, Locale::ZhCn, 20, 5);
    assert!(picker.painted.is_empty());
    picker.pointer(click(last));
    assert!(picker.request.is_none());
    picker.observe_local(Vec::new());
    assert!(paint(&mut picker, Locale::EnUs, 80, 24).contains("No available workspaces"));
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
    let text = paint(&mut picker, Locale::EnUs, 100, 30);
    assert!(text.contains("Working"));
    picker.pointer(click(picker.painted[2].0));
    let expected = Selection {
        machine: Choice::Remote(b.id.clone()),
        context: Some(NativeStartupTarget::Agent(Box::new(agent_row().target))),
    };
    assert_eq!(picker.request.take(), Some(expected.clone()));
    picker.replace(&[b.clone(), a.clone()]);
    assert_eq!(picker.selection(), Some(expected));
    paint(&mut picker, Locale::ZhCn, 100, 30);
    let old = picker.painted[picker.selected].0;
    picker.observe_agents(vec![(
        Choice::Local,
        Agents::Ready(crate::machine_agents::tests::checkpoint(vec![agent_row()])),
    )]);
    picker.pointer(click(old));
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
    paint(&mut picker, Locale::EnUs, 100, 30);
    picker.pointer(click(picker.mode_hits[2].0));
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
            let text = paint(&mut picker, locale, 100, 30);
            let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
            let expected: String = locale
                .machine_picker_text(key)
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();
            assert!(compact.contains(&expected));
        }
    }
    let old = picker.painted[0].0;
    paint(&mut picker, Locale::ZhCn, 20, 5);
    assert!(picker.painted.is_empty());
    picker.pointer(click(old));
    assert!(picker.request.is_none());
}

mod agents;
mod sidebar;

mod navigation;
mod spaces;
