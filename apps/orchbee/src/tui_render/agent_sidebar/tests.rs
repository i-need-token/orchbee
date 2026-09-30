use super::*;
use crate::{
    client_view::{KeyCode, KeyEvent, Modifiers, PrefixInput, SessionActionDisposition},
    machine_agents::{
        Observation as Agents,
        tests::{checkpoint, row},
    },
    machine_picker::Choice,
};
use ratatui::{Terminal, backend::TestBackend};

fn view(width: u16, height: u16, count: u64) -> ClientView {
    let mut view = ClientView::new(width);
    view.machines.available = true;
    if width < 100 {
        for key in [
            KeyEvent::press(KeyCode::Char('b'), Modifiers::CTRL),
            KeyEvent::press(KeyCode::Char('b'), Modifiers::NONE),
        ] {
            let transition = view.decide(PrefixInput::Key(key));
            view.commit(transition, SessionActionDisposition::KeepNavigator);
        }
    }
    view.machines.observe_agent_sidebar(
        project_shell_areas(Rect::new(0, 0, width, height), view.sidebar_surface())
            .sidebar
            .is_some(),
    );
    view.machines.observe_agents(vec![(
        Choice::Local,
        Agents::Ready(checkpoint((23..23 + count).map(row).collect())),
    )]);
    view
}
fn paint(
    view: &ClientView,
    locale: Locale,
    theme: &Theme,
    width: u16,
    height: u16,
) -> (Vec<String>, crate::tui_pointer::FrameHitMap) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut hits = HitMapBuilder::new();
    terminal
        .draw(|frame| {
            render_terminal_shell(frame, view, locale, None, theme, &mut hits);
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    let rows = (0..height)
        .map(|y| {
            let mut row = String::new();
            let mut x = 0;
            while x < width.min(30) {
                let symbol = buffer[(x, y)].symbol();
                row.push_str(symbol);
                x += display_width(symbol).max(1) as u16;
            }
            row.trim_end().to_owned()
        })
        .collect();
    (rows, hits.finish())
}

#[test]
fn sidebar_renders_complete_rows_at_all_tiers_locales_and_themes() {
    for (width, height) in [(80, 24), (120, 40), (60, 12), (50, 10), (15, 4)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for theme in [Theme::terminal_native(), Theme::rgb_test_dark()] {
                let view = view(width, height, 20);
                let (rows, hits) = paint(&view, locale, &theme, width, height);
                let joined = rows.join("\n");
                let visible =
                    project_shell_areas(Rect::new(0, 0, width, height), view.sidebar_surface())
                        .sidebar
                        .is_some();
                if visible {
                    assert!(
                        joined.contains(locale.machine_picker_text("agent-working")),
                        "{width}x{height} {rows:?}"
                    );
                    assert!(joined.contains("/20"), "{rows:?}");
                    assert!(
                        joined.contains(if locale == Locale::EnUs {
                            "Local / space / build"
                        } else {
                            "本地 / space / build"
                        }),
                        "{rows:?}"
                    );
                    let mut inspected = Vec::new();
                    for y in 0..height {
                        if let Some(HitTarget::MachineAgentRow { window, selected }) =
                            hits.hit_target_at(2, y)
                        {
                            if inspected.contains(selected) {
                                continue;
                            }
                            inspected.push(selected.clone());
                            assert!(window.capacity > 0 && window.end() <= 20);
                            assert!(y + 1 < height - 1, "row crossed status: {y}");
                            assert!(!rows[y as usize + 1].trim().is_empty());
                        }
                    }
                } else {
                    assert!(
                        !joined.contains("space / build"),
                        "hidden sidebar drew: {rows:?}"
                    );
                    assert!((0..height).all(|y| !matches!(
                        hits.hit_target_at(2, y),
                        Some(HitTarget::MachineAgentRow { .. })
                    )));
                }
            }
        }
    }
}

#[test]
fn compact_sidebar_visual_rows_and_mouse_bounds_match_the_painted_window() {
    let view = view(60, 12, 1);
    let (rows, hits) = paint(&view, Locale::EnUs, &Theme::terminal_native(), 60, 12);
    assert_eq!(
        &rows[..11],
        &[
            " ORCHBEE                     │",
            "                             │",
            "  SITE                       │",
            "  ● Backend unavailable      │",
            "  Agents 1                   │",
            "  f All states · s Placemen… │",
            "   1 Working codex           │",
            "  Local / space / build      │",
            "                             │",
            "                             │",
            "  List / filters       1-1/1 │"
        ]
    );
    assert!(matches!(
        hits.hit_target_at(2, 4),
        Some(HitTarget::OpenMachineAgents)
    ));
    assert!(matches!(
        hits.hit_target_at(2, 6),
        Some(HitTarget::MachineAgentRow { .. })
    ));
    assert!(matches!(
        hits.hit_target_at(27, 7),
        Some(HitTarget::MachineAgentRow { .. })
    ));
    assert!(matches!(
        hits.hit_target_at(28, 7),
        Some(HitTarget::OverlaySurface)
    ));
    assert!(
        hits.hit_target_at(30, 7).is_none(),
        "outside overlay must dismiss without selecting"
    );
}

#[test]
fn sidebar_shares_source_order_numbering_and_never_numbers_unavailable_rows() {
    let mut view = view(120, 40, 2);
    let mut round = checkpoint(vec![row(23), row(24)]).as_ref().clone();
    round.rows = round
        .rows
        .iter()
        .cloned()
        .map(|mut r| {
            r.sequence = orchbee_workbench_protocol::WorkbenchRevision::new(
                if r.target.member == row(23).target.member {
                    1
                } else {
                    2
                },
            );
            r
        })
        .collect();
    round.view.spec=Some(orchbee_workbench_protocol::parse_agent_view_spec(br#"{"source":"tools","label":"review","sort":[{"field":"state_change_seq","order":"desc"}]}"#).unwrap());
    let remote = crate::machines::Profile {
        id: crate::machines::ProfileId::parse(&format!("{:032x}", 1)).unwrap(),
        draft: crate::machines::Draft::parse("remote", "example.invalid", "default").unwrap(),
        enabled: true,
    };
    let choice = Choice::Remote(remote.id.clone());
    view.machines.replace(&[remote]);
    view.machines.observe_agents(vec![
        (Choice::Local, Agents::Ready(std::sync::Arc::new(round))),
        (choice, Agents::Unavailable),
    ]);
    let (rows, hits) = paint(&view, Locale::EnUs, &Theme::rgb_test_dark(), 120, 40);
    let joined = rows.join("\n");
    assert!(joined.contains("View: review"));
    assert!(joined.contains("1 Working codex") && joined.contains("2 Working codex"));
    assert!(!joined.contains("3 remote"));
    let selected = (0..40)
        .find_map(|y| match hits.hit_target_at(2, y) {
            Some(HitTarget::MachineAgentRow { selected, .. }) => Some(selected),
            _ => None,
        })
        .unwrap();
    assert!(
        matches!(&selected.context,Some(crate::terminal_session::NativeStartupTarget::Agent(t)) if t.member==row(24).target.member)
    );
}

#[test]
fn sidebar_truncates_wide_and_unsafe_text_without_changing_navigation_identity() {
    let mut view = view(120, 40, 1);
    let mut wide = row(23);
    wide.target.agent = "码🦀\u{1b}[2J\nprovider".into();
    wide.workspace = "宽项目👩‍💻".repeat(10);
    wide.label = "build\u{1b}]0;title\u{7}".into();
    wide.sequence = orchbee_workbench_protocol::WorkbenchRevision::new(2);
    let target = wide.target.clone();
    let mut round = checkpoint(vec![wide]).as_ref().clone();
    round.report_revision = orchbee_workbench_protocol::WorkbenchRevision::new(2);
    view.machines.observe_agents(vec![(
        Choice::Local,
        Agents::Ready(std::sync::Arc::new(round)),
    )]);
    let (rows, hits) = paint(&view, Locale::ZhCn, &Theme::terminal_native(), 120, 40);
    let joined = rows.join("\n");
    assert!(joined.contains('…') && !joined.contains('\u{1b}') && !joined.contains('\u{7}'));
    assert!(joined.contains("宽项目👩‍💻"), "{rows:?}");
    let selected = (0..40)
        .find_map(|y| match hits.hit_target_at(2, y) {
            Some(HitTarget::MachineAgentRow { selected, .. }) => Some(selected),
            _ => None,
        })
        .unwrap();
    assert!(
        matches!(&selected.context,Some(crate::terminal_session::NativeStartupTarget::Agent(t)) if t.as_ref()==&target)
    );
}

#[test]
fn sidebar_current_and_pending_rows_use_theme_roles_and_source_failure_stays_explicit() {
    for theme in [Theme::terminal_native(), Theme::rgb_test_dark()] {
        let mut view = view(120, 40, 1);
        let agent = row(23);
        view.machines.observe_focus(
            Choice::Local,
            Some(agent.target.context.clone()),
            Some(crate::terminal_session::NativeMemberTarget {
                tab: agent.target.tab,
                panel: agent.target.panel,
                presentation: agent.target.member,
            }),
        );
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        let mut hits = HitMapBuilder::new();
        terminal
            .draw(|frame| {
                render_terminal_shell(frame, &view, Locale::EnUs, None, &theme, &mut hits);
            })
            .unwrap();
        let hits = hits.finish();
        let y = (0..40)
            .find(|y| {
                matches!(
                    hits.hit_target_at(2, *y),
                    Some(HitTarget::MachineAgentRow { .. })
                )
            })
            .unwrap();
        assert_eq!(
            terminal.backend().buffer()[(2, y)].fg,
            theme.style(Role::AccentFocus).fg.unwrap_or_default()
        );
        let before_seen = view.machines.sidebar_rows()[0]
            .sidebar_agent()
            .unwrap()
            .seen;
        let (rows, _) = paint(&view, Locale::EnUs, &theme, 120, 40);
        assert!(rows[y as usize].contains("*1 Working codex"));
        assert_eq!(
            view.machines.sidebar_rows()[0]
                .sidebar_agent()
                .unwrap()
                .seen,
            before_seen
        );
        view.machines
            .observe_agents(vec![(Choice::Local, Agents::Unavailable)]);
        let (rows, hits) = paint(&view, Locale::EnUs, &theme, 120, 40);
        assert!(rows.join("\n").contains("Agents unavailable"));
        assert!(!rows.join("\n").contains("Agents 0"));
        assert!((0..40).all(|y| !matches!(
            hits.hit_target_at(2, y),
            Some(HitTarget::MachineAgentRow { .. })
        )));
    }
}
