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

#[test]
fn live_raw_and_stripped_titles_and_custom_title_keep_distinct_values_and_exact_targets() {
    use orchbee_workbench_protocol::{
        MetadataTokenKey, MetadataTokenValue, MetadataTokens, TerminalTitleObservation,
        WorkbenchRevision,
    };
    let mut view = view(120, 40, 0);
    let mut agent = row(23);
    agent.tokens = MetadataTokens::new(
        [(
            MetadataTokenKey::parse("terminal_title").unwrap(),
            MetadataTokenValue::parse("custom title").unwrap(),
        )]
        .into(),
    )
    .unwrap();
    let target = agent.target.clone();
    configure(
        &mut view,
        serde_json::json!([
            [configured_token("terminal_title")],
            [configured_token("terminal_title_stripped")],
            [configured_token("$terminal_title")]
        ]),
        serde_json::json!({}),
        0,
    );
    for (revision, title, expected_height) in [
        (1, Some("⠋ 修复🙂标题"), 3),
        (2, Some("⠙ 修复🙂标题"), 3),
        (3, None, 1),
    ] {
        agent.terminal_title = TerminalTitleObservation::Observed {
            revision: WorkbenchRevision::new(revision),
            title: title.map(str::to_owned),
        };
        view.machines.observe_agents(vec![(
            Choice::Local,
            Agents::Ready(checkpoint(vec![agent.clone()])),
        )]);
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for theme in [
                Theme::terminal_native(),
                Theme::rgb_test_dark(),
                Theme::one_light(),
                Theme::one_dark(),
            ] {
                let (rows, hits) = paint(&view, locale, &theme, 120, 40);
                let text = rows.join("\n");
                assert!(text.contains("custom title"));
                if let Some(raw) = title {
                    assert!(text.contains(raw));
                    assert_eq!(rows.iter().filter(|s| s.contains("修复🙂标题")).count(), 2);
                } else {
                    assert!(!text.contains("修复🙂标题"));
                }
                let exact: Vec<_> = (0..40)
                    .filter_map(|y| match hits.hit_target_at(2, y) {
                        Some(HitTarget::MachineAgentRow { selected, .. }) => Some(selected),
                        _ => None,
                    })
                    .collect();
                assert_eq!(exact.len(), expected_height);
                assert!(exact.iter().all(|s|matches!(&s.context,Some(crate::terminal_session::NativeStartupTarget::Agent(t)) if t.as_ref()==&target)));
            }
        }
    }
}

#[test]
fn title_rules_match_raw_or_stripped_forms_before_sanitizing_and_missing_never_borrows_pane_title()
{
    use orchbee_workbench_protocol::{TerminalTitleObservation, WorkbenchRevision};
    let mut view = view(120, 40, 0);
    let mut agent = row(23);
    agent.terminal_title = TerminalTitleObservation::Observed {
        revision: WorkbenchRevision::new(1),
        title: Some("Administrator: ⠋ raw\u{202e}value".into()),
    };
    view.machines.observe_agents(vec![(
        Choice::Local,
        Agents::Ready(checkpoint(vec![agent.clone()])),
    )]);
    let hidden = |name, value| serde_json::json!({"name":name,"style":{"fg":null,"bold":null,"dim":null},"rules":[{"condition":{"kind":"Equals","value":value},"ignore_case":false,"style":{"fg":null,"bold":null,"dim":null},"hide":true}]});
    configure(
        &mut view,
        serde_json::json!([
            [hidden(
                "terminal_title",
                "Administrator: ⠋ raw\u{202e}value"
            )],
            [hidden("terminal_title_stripped", "raw\u{202e}value")]
        ]),
        serde_json::json!({}),
        0,
    );
    let (rows, hits) = paint(&view, Locale::EnUs, &Theme::terminal_native(), 120, 40);
    assert!(!rows.join("\n").contains("rawvalue"));
    assert_eq!(
        (0..40)
            .filter(|y| matches!(
                hits.hit_target_at(2, *y),
                Some(HitTarget::MachineAgentRow { .. })
            ))
            .count(),
        1
    );
    agent.terminal_title = TerminalTitleObservation::Unavailable {};
    view.machines.observe_agents(vec![(
        Choice::Local,
        Agents::Ready(checkpoint(vec![agent])),
    )]);
    configure(
        &mut view,
        serde_json::json!([
            [configured_token("terminal_title")],
            [configured_token("terminal_title_stripped")]
        ]),
        serde_json::json!({}),
        0,
    );
    let (rows, hits) = paint(&view, Locale::ZhCn, &Theme::terminal_native(), 120, 40);
    assert!(!rows.join("\n").contains("build-23"));
    assert_eq!(
        (0..40)
            .filter(|y| matches!(
                hits.hit_target_at(2, *y),
                Some(HitTarget::MachineAgentRow { .. })
            ))
            .count(),
        1
    );
}

fn configured_token(name: &str) -> serde_json::Value {
    serde_json::json!({"name":name,"style":{"fg":null,"bold":null,"dim":null},"rules":[]})
}
fn configure(
    view: &mut ClientView,
    rows: serde_json::Value,
    overrides: serde_json::Value,
    gap: u16,
) {
    let profile =
        serde_json::json!({"rows":rows,"rows_by_agent":overrides,"row_gap":gap}).to_string();
    view.machines.configure_agent_sidebar(
        orchbee_local_site_runtime::WindowsAgentSidebarSettings::from_profile(&profile).unwrap(),
    );
}

#[test]
fn configured_sidebar_values_share_metadata_labels_and_keep_provider_identity_for_overrides() {
    use orchbee_workbench_protocol::{
        MetadataState, MetadataTokenKey, MetadataTokenValue, MetadataTokens, WorkbenchRevision,
    };
    let mut view = view(120, 40, 0);
    let mut agent = row(23);
    agent.target.agent = "claude-code".into();
    agent.sequence = WorkbenchRevision::new(2);
    agent.metadata.display_agent = Some(MetadataTokenValue::parse("codex").unwrap());
    agent.metadata.title = Some(MetadataTokenValue::parse("Named pane").unwrap());
    agent.metadata.state_labels.insert(
        MetadataState::Working,
        MetadataTokenValue::parse("Writing code").unwrap(),
    );
    agent.tokens = MetadataTokens::new(
        [(
            MetadataTokenKey::parse("issue").unwrap(),
            MetadataTokenValue::parse("ORCH-42").unwrap(),
        )]
        .into(),
    )
    .unwrap();
    let target = agent.target.clone();
    let mut round = checkpoint(vec![agent]).as_ref().clone();
    round.report_revision = WorkbenchRevision::new(2);
    view.machines.observe_agents(vec![(
        Choice::Local,
        Agents::Ready(std::sync::Arc::new(round)),
    )]);
    let token = configured_token;
    configure(
        &mut view,
        serde_json::json!([[token("agent")]]),
        serde_json::json!({"codex":[[token("$missing")]],"claude":[[token("state_icon"),token("state_text")],[token("machine"),token("workspace")],[token("tab"),token("pane")],[token("agent"),token("$issue")]]}),
        0,
    );
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [
            Theme::terminal_native(),
            Theme::rgb_test_dark(),
            Theme::one_light(),
            Theme::one_dark(),
        ] {
            let (rows, hits) = paint(&view, locale, &theme, 120, 40);
            let joined = rows.join("\n");
            assert!(
                joined.contains("Writing code")
                    && joined.contains("Task · Named pane")
                    && joined.contains("codex · ORCH-42"),
                "{rows:?}"
            );
            assert_eq!(
                (0..40)
                    .filter(|y| matches!(
                        hits.hit_target_at(2, *y),
                        Some(HitTarget::MachineAgentRow { .. })
                    ))
                    .count(),
                4
            );
            assert!((0..40).filter_map(|y| match hits.hit_target_at(2,y) {Some(HitTarget::MachineAgentRow{selected,..})=>Some(selected),_=>None}).all(|selected| matches!(&selected.context,Some(crate::terminal_session::NativeStartupTarget::Agent(t)) if t.as_ref()==&target)));
        }
    }
}

#[test]
fn configured_sidebar_hides_raw_token_matches_and_missing_rows_without_hiding_navigation() {
    use orchbee_workbench_protocol::{MetadataTokenKey, MetadataTokenValue, MetadataTokens};
    let mut view = view(120, 40, 0);
    let mut agent = row(23);
    agent.tokens = MetadataTokens::new(
        [(
            MetadataTokenKey::parse("hidden").unwrap(),
            MetadataTokenValue::parse("raw\u{202e}value").unwrap(),
        )]
        .into(),
    )
    .unwrap();
    let target = agent.target.clone();
    view.machines.observe_agents(vec![(
        Choice::Local,
        Agents::Ready(checkpoint(vec![agent])),
    )]);
    let hidden = serde_json::json!({"name":"$hidden","style":{"fg":null,"bold":null,"dim":null},"rules":[{"condition":{"kind":"Equals","value":"raw\u{202e}value"},"ignore_case":false,"style":{"fg":null,"bold":null,"dim":null},"hide":true}]});
    configure(
        &mut view,
        serde_json::json!([[configured_token("$missing")], [hidden]]),
        serde_json::json!({}),
        0,
    );
    let (rows, hits) = paint(&view, Locale::EnUs, &Theme::terminal_native(), 120, 40);
    assert!(!rows.join("\n").contains("rawvalue"));
    let hits = (0..40)
        .filter_map(|y| match hits.hit_target_at(2, y) {
            Some(HitTarget::MachineAgentRow { selected, .. }) => Some(selected),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(hits.len(), 1);
    assert!(
        matches!(&hits[0].context,Some(crate::terminal_session::NativeStartupTarget::Agent(t)) if t.as_ref()==&target)
    );
}

#[test]
fn variable_sidebar_heights_gaps_and_small_viewports_publish_exact_hit_geometry() {
    use orchbee_workbench_protocol::WorkbenchRevision;
    let mut view = view(120, 40, 0);
    let mut agents: Vec<_> = (23..26).map(row).collect();
    agents[1].target.agent = "claude".into();
    agents[2].target.agent = "custom".into();
    let mut round = checkpoint(agents).as_ref().clone();
    round.report_revision = WorkbenchRevision::new(2);
    view.machines.observe_agents(vec![(
        Choice::Local,
        Agents::Ready(std::sync::Arc::new(round)),
    )]);
    let token = configured_token;
    configure(
        &mut view,
        serde_json::json!([[token("pane")], [token("agent")]]),
        serde_json::json!({"codex":[[token("pane")]],"claude":[[token("state_text")],[token("pane")],[token("agent")]]}),
        1,
    );
    let (rows, hits) = paint(&view, Locale::EnUs, &Theme::terminal_native(), 120, 40);
    let mut spans = std::collections::BTreeMap::new();
    for y in 0..40 {
        if let Some(HitTarget::MachineAgentRow { selected, .. }) = hits.hit_target_at(2, y) {
            let Some(crate::terminal_session::NativeStartupTarget::Agent(t)) = &selected.context
            else {
                panic!()
            };
            spans
                .entry(t.member.clone())
                .or_insert_with(Vec::new)
                .push(y);
        }
    }
    assert_eq!(
        spans.values().map(Vec::len).collect::<Vec<_>>(),
        vec![1, 3, 2]
    );
    for ys in spans.values().take(2) {
        let gap = ys.last().unwrap() + 1;
        assert!(matches!(
            hits.hit_target_at(2, gap),
            Some(HitTarget::MachineAgentSurface(_))
        ));
        assert!(
            rows[usize::from(gap)]
                .trim()
                .trim_matches('│')
                .trim()
                .is_empty()
        );
    }
    view.resize(60);
    // Explicitly show the compact sidebar; the full-width view's intent was docked.
    for key in [
        KeyEvent::press(KeyCode::Char('b'), Modifiers::CTRL),
        KeyEvent::press(KeyCode::Char('b'), Modifiers::NONE),
    ] {
        let t = view.decide(PrefixInput::Key(key));
        view.commit(t, SessionActionDisposition::KeepNavigator);
    }
    let (_, hits) = paint(&view, Locale::EnUs, &Theme::terminal_native(), 60, 12);
    let window = (0..12)
        .find_map(|y| match hits.hit_target_at(2, y) {
            Some(HitTarget::MachineAgentRow { window, .. }) => Some(*window),
            _ => None,
        })
        .unwrap();
    assert_eq!(window.capacity, 1);
    view.machines.queue_sidebar_scroll(window, false);
    assert!(view.machines.settle_sidebar_scroll());
    let (_, hits) = paint(&view, Locale::EnUs, &Theme::terminal_native(), 60, 12);
    let mut painted = 0;
    for y in 0..12 {
        if let Some(HitTarget::MachineAgentRow { selected, window }) = hits.hit_target_at(2, y) {
            assert_eq!((window.top, window.end()), (1, 2));
            assert!(
                matches!(&selected.context,Some(crate::terminal_session::NativeStartupTarget::Agent(t)) if t.agent=="claude")
            );
            painted += 1;
        }
    }
    assert_eq!(painted, 3);
}

#[test]
fn sidebar_token_styles_survive_grapheme_truncation_and_do_not_style_identity_markers() {
    let mut view = view(120, 40, 0);
    let mut agent = row(23);
    agent.label = "宽👩‍💻e\u{301}".repeat(20);
    view.machines.observe_agents(vec![(
        Choice::Local,
        Agents::Ready(checkpoint(vec![agent])),
    )]);
    configure(
        &mut view,
        serde_json::json!([[{"name":"pane","style":{"fg":[16,32,48],"bold":true,"dim":false},"rules":[]}]]),
        serde_json::json!({}),
        0,
    );
    for theme in [
        Theme::terminal_native(),
        Theme::rgb_test_dark(),
        Theme::one_light(),
        Theme::one_dark(),
    ] {
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        let mut hits = HitMapBuilder::new();
        terminal
            .draw(|frame| {
                render_terminal_shell(frame, &view, Locale::ZhCn, None, &theme, &mut hits);
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
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(2, y)].fg, theme.style(Role::HintKey).fg.unwrap());
        let expected = theme.sidebar_token_style(
            Role::TextPrimary,
            orchbee_local_site_runtime::WindowsSidebarTokenStyle {
                fg: Some([16, 32, 48]),
                bold: Some(true),
                dim: Some(false),
            },
        );
        assert_eq!(buffer[(5, y)].fg, expected.fg.unwrap());
        assert!(buffer[(5, y)].modifier.contains(Modifier::BOLD));
        assert!(!buffer[(5, y)].modifier.contains(Modifier::DIM));
        assert!((2..28).any(|x| buffer[(x, y)].symbol() == "…"));
        assert!((0..120).all(|x| !buffer[(x, y)].symbol().starts_with('\u{301}')));
    }
}

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
            for theme in [
                Theme::terminal_native(),
                Theme::rgb_test_dark(),
                Theme::one_light(),
                Theme::one_dark(),
            ] {
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
    for theme in [
        Theme::terminal_native(),
        Theme::rgb_test_dark(),
        Theme::one_light(),
        Theme::one_dark(),
    ] {
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
