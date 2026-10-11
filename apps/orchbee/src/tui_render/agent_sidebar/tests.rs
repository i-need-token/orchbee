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
    let mut view = view(120, 40, 1);
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
    let mut view = view(120, 40, 1);
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
    let mut view = view(120, 40, 1);
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
    let mut view = view(120, 40, 1);
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
    let mut view = view(120, 40, 1);
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
            Some(HitTarget::MachineWorkspaceSurface(_))
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
    assert_eq!(window.capacity, 2);
    view.machines.queue_space_scroll(window, false);
    assert!(view.machines.settle_sidebar_scroll());
    let (_, hits) = paint(&view, Locale::EnUs, &Theme::terminal_native(), 60, 12);
    let mut painted = std::collections::BTreeMap::new();
    for y in 0..12 {
        if let Some(HitTarget::MachineAgentRow { selected, window }) = hits.hit_target_at(2, y) {
            assert_eq!((window.top, window.end()), (2, 4));
            let Some(crate::terminal_session::NativeStartupTarget::Agent(target)) =
                &selected.context
            else {
                panic!()
            };
            *painted.entry(target.agent.as_str()).or_insert(0) += 1;
        }
    }
    assert_eq!(
        painted,
        std::collections::BTreeMap::from([("claude", 3), ("custom", 2)])
    );
}

#[test]
fn sidebar_token_styles_survive_grapheme_truncation_and_do_not_style_identity_markers() {
    let mut view = view(120, 40, 1);
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
        let ordinal_x = (0..30)
            .find(|x| buffer[(*x, y)].symbol() == "1")
            .expect("nested Agent ordinal");
        assert_eq!(
            buffer[(ordinal_x, y)].fg,
            theme.style(Role::HintLabel).fg.unwrap()
        );
        let expected = theme.sidebar_token_style(
            Role::TextPrimary,
            orchbee_local_site_runtime::WindowsSidebarTokenStyle {
                fg: Some([16, 32, 48]),
                bold: Some(true),
                dim: Some(false),
            },
            false,
            false,
        );
        let token_x = (0..30)
            .find(|x| {
                buffer[(*x, y)].fg == expected.fg.unwrap()
                    && buffer[(*x, y)].modifier.contains(Modifier::BOLD)
            })
            .expect("configured Agent token style is painted on the nested row");
        assert!(!buffer[(token_x, y)].modifier.contains(Modifier::DIM));
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
    let agents = (23..23 + count).map(row).collect::<Vec<_>>();
    let owner = row(23).target.context;
    view.machines
        .observe_local(vec![crate::machine_discovery::Context {
            target: owner,
            project_id: None,
            project: None,
            workspace: Some("Standalone".into()),
        }]);
    view.machines
        .observe_agents(vec![(Choice::Local, Agents::Ready(checkpoint(agents)))]);
    if count > 0 {
        let owner = view
            .machines
            .sidebar_navigation_rows()
            .iter()
            .find_map(|entry| match entry {
                crate::machine_picker::SidebarRow::Context { .. } => Some(entry.target()),
                _ => None,
            })
            .unwrap();
        assert!(view.machines.sidebar_context_agents_expanded(&owner));
    }
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
fn empty_agent_result_keeps_only_the_global_picker_entry() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [Theme::terminal_native(), Theme::rgb_test_dark()] {
            let view = view(120, 40, 0);
            let (rows, hits) = paint(&view, locale, &theme, 120, 40);
            let text = rows.join("\n");
            assert!(text.contains(locale.machine_picker_text("agent-sidebar-open")));
            assert!(!text.contains(locale.machine_picker_text("no-agents")));
            assert!((0..40).all(|y| (0..30).all(|x| !matches!(
                hits.hit_target_at(x, y),
                Some(HitTarget::MachineAgentRow { .. })
            ))));
            assert!((0..40).any(|y| {
                (0..30)
                    .any(|x| matches!(hits.hit_target_at(x, y), Some(HitTarget::OpenMachineAgents)))
            }));
        }
    }
}

#[test]
fn filtering_out_the_last_agent_closes_disclosure_and_keeps_global_picker_entry() {
    let mut view = view(120, 40, 1);
    let owner = view
        .machines
        .sidebar_navigation_rows()
        .iter()
        .find_map(|entry| match entry {
            crate::machine_picker::SidebarRow::Context { .. } => Some(entry.target()),
            _ => None,
        })
        .unwrap();
    assert!(view.machines.sidebar_context_agents_expanded(&owner));
    view.machines.show_agents();
    view.machines
        .key(KeyEvent::press(KeyCode::Char('f'), Modifiers::NONE));
    view.machines
        .key(KeyEvent::press(KeyCode::Esc, Modifiers::NONE));
    assert!(!view.machines.sidebar_context_agents_expanded(&owner));
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let (rows, hits) = paint(&view, locale, &Theme::terminal_native(), 120, 40);
        assert!(
            rows.join("\n")
                .contains(locale.machine_picker_text("agent-sidebar-open"))
        );
        assert!((0..40).all(|y| !matches!(
            hits.hit_target_at(2, y),
            Some(HitTarget::MachineAgentRow { .. })
        )));
    }
}

#[test]
fn nested_agents_use_global_ordinals_without_a_second_sidebar_region() {
    let view = view(120, 40, 2);
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let (rows, hits) = paint(&view, locale, &Theme::terminal_native(), 120, 40);
        let text = rows.join("\n");
        assert!(text.contains(" 1 "), "{rows:?}");
        assert!(text.contains(" 2 "), "{rows:?}");
        assert!(!text.contains(&format!("{} 2", locale.shell().agents)));
        assert!(text.contains(locale.machine_picker_text("agent-sidebar-open")));
        assert_eq!(
            (0..40)
                .filter(|y| matches!(
                    hits.hit_target_at(2, *y),
                    Some(HitTarget::MachineAgentRow { .. })
                ))
                .count(),
            4
        );
    }
}

#[test]
fn default_agent_rows_prioritize_task_identity_without_repeating_parent_context() {
    let mut view = view(120, 40, 1);
    let mut agent = row(23);
    agent.metadata.title =
        Some(orchbee_workbench_protocol::MetadataTokenValue::parse("Input handling").unwrap());
    let target = agent.target.clone();
    view.machines.observe_agents(vec![(
        Choice::Local,
        Agents::Ready(checkpoint(vec![agent])),
    )]);
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let mut ascii = Theme::terminal_native();
        ascii.glyphs = crate::tui_theme::GlyphSet::ASCII;
        for theme in [
            Theme::terminal_native(),
            Theme::one_dark(),
            Theme::one_light(),
            ascii,
        ] {
            let (rows, hits) = paint(&view, locale, &theme, 120, 40);
            let y = (0..40)
                .find(|y| {
                    matches!(
                        hits.hit_target_at(2, *y),
                        Some(HitTarget::MachineAgentRow { .. })
                    )
                })
                .unwrap();
            assert!(rows[y as usize].contains("Input handling"), "{rows:?}");
            assert!(rows[y as usize].contains(theme.glyphs.status_bullet));
            assert!(rows[y as usize + 1].contains("codex"));
            assert_eq!(
                display_width(rows[y as usize].split("Input handling").next().unwrap()),
                display_width(rows[y as usize + 1].split("codex").next().unwrap())
            );
            assert!(rows[y as usize + 1].contains(theme.glyphs.middle_dot));
            if theme.glyphs.middle_dot != "·" {
                assert!(!rows[y as usize + 1].contains('·'));
            }
            assert!(rows[y as usize + 1].contains(locale.machine_picker_text("agent-working")));
            assert!(!rows.join("\n").contains("Local /"));
            assert!(!rows.join("\n").contains("本地 /"));
            assert!(!rows.join("\n").contains("space /"));
            for row in [y, y + 1] {
                assert!(
                    matches!(hits.hit_target_at(2, row), Some(HitTarget::MachineAgentRow { selected, .. })
                    if matches!(&selected.context, Some(crate::terminal_session::NativeStartupTarget::Agent(actual)) if actual.as_ref() == &target))
                );
            }
        }
    }
}

#[test]
fn default_agent_detail_keeps_status_readable_without_empty_provider_fragments() {
    use orchbee_workbench_protocol::{MetadataState, MetadataTokenValue};
    for (width, height) in [(120, 40), (80, 24), (60, 12)] {
        let mut view = view(width, height, 1);
        let mut agent = row(23);
        agent.metadata.state_labels.insert(
            MetadataState::Working,
            MetadataTokenValue::parse("Investigating").unwrap(),
        );
        view.machines.observe_agents(vec![(
            Choice::Local,
            Agents::Ready(checkpoint(vec![agent])),
        )]);
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for theme in [
                Theme::terminal_native(),
                Theme::one_dark(),
                Theme::one_light(),
            ] {
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                let mut hits = HitMapBuilder::new();
                terminal
                    .draw(|frame| {
                        render_terminal_shell(frame, &view, locale, None, &theme, &mut hits);
                    })
                    .unwrap();
                let hits = hits.finish();
                let y = (0..height)
                    .find(|y| {
                        matches!(
                            hits.hit_target_at(2, *y),
                            Some(HitTarget::MachineAgentRow { .. })
                        )
                    })
                    .unwrap();
                let buffer = terminal.backend().buffer();
                let x = (0..30)
                    .find(|x| buffer[(*x, y + 1)].symbol() == "I")
                    .unwrap();
                assert_eq!(
                    buffer[(x, y + 1)].fg,
                    theme.style(Role::TextPrimary).fg.unwrap_or_default()
                );
                assert!(!buffer[(x, y + 1)].modifier.contains(Modifier::DIM));
                let detail = (0..30)
                    .map(|x| buffer[(x, y + 1)].symbol())
                    .collect::<String>();
                assert!(detail.contains("Investigating"), "{detail}");
                assert!(!detail.contains(theme.glyphs.ellipsis), "{detail}");
                assert!(!detail.contains('·'), "{detail}");
            }
        }
    }
}

#[test]
fn default_agent_detail_keeps_only_complete_provider_labels_across_cell_budgets() {
    let mut view = view(120, 40, 1);
    let mut agent = row(23);
    agent.metadata.state_labels.insert(
        orchbee_workbench_protocol::MetadataState::Working,
        orchbee_workbench_protocol::MetadataTokenValue::parse("Investigating").unwrap(),
    );
    view.machines.observe_agents(vec![(
        Choice::Local,
        Agents::Ready(checkpoint(vec![agent])),
    )]);
    let rows = view.machines.sidebar_navigation_rows();
    let row = rows
        .iter()
        .find_map(|row| match row {
            crate::machine_picker::SidebarRow::Agent { row, .. } => Some(row),
            _ => None,
        })
        .unwrap();
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [
            Theme::terminal_native(),
            Theme::one_light(),
            Theme::one_dark(),
        ] {
            for width in 17..=30 {
                let lines = tokens::lines(&view, row, 0, locale, &theme, width, false);
                let detail = lines[1]
                    .spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>();
                assert!(detail.ends_with("Investigating"), "{width}: {detail}");
                assert!(display_width(&detail) <= width, "{width}: {detail}");
                assert_eq!(detail.contains("codex"), width >= 25, "{width}: {detail}");
                assert_eq!(
                    detail.contains(theme.glyphs.middle_dot),
                    width >= 25,
                    "{width}: {detail}"
                );
                assert!(!detail.contains(theme.glyphs.ellipsis), "{width}: {detail}");
            }
        }
    }
}

#[test]
fn default_agent_identity_uses_provider_for_blank_titles_without_repeating_it() {
    for label in ["", " \t\n", "codex"] {
        let mut view = view(120, 40, 1);
        let mut agent = row(23);
        agent.label = label.into();
        view.machines.observe_agents(vec![(
            Choice::Local,
            Agents::Ready(checkpoint(vec![agent])),
        )]);
        let (rows, hits) = paint(&view, Locale::EnUs, &Theme::terminal_native(), 120, 40);
        let y = (0..40)
            .find(|y| {
                matches!(
                    hits.hit_target_at(2, *y),
                    Some(HitTarget::MachineAgentRow { .. })
                )
            })
            .unwrap();
        assert!(rows[y as usize].contains("codex"), "{rows:?}");
        assert!(rows[y as usize + 1].contains("Working"));
        assert!(!rows[y as usize + 1].contains("codex"));
        assert!(!rows[y as usize + 1].contains('·'));
    }
}

#[test]
fn sidebar_menu_entry_uses_the_configured_binding_and_drawn_geometry() {
    for (profile, expected) in [
        ("[keys]\n", Some("Ctrl+B F2")),
        (
            "[keys]\nprefix='ctrl+a'\nmenu='prefix+f3'",
            Some("Ctrl+A F3"),
        ),
        ("[keys]\nmenu='alt+f3'", Some("Alt+F3")),
        ("[keys]\nmenu=[]", None),
    ] {
        let mut view = view(120, 40, 0);
        view.configure_keys(
            &orchbee_local_site_runtime::WindowsKeySettings::from_profile(profile).unwrap(),
        );
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let (rows, hits) = paint(&view, locale, &Theme::terminal_native(), 120, 40);
            let menu = &rows[38];
            assert!(menu.contains(locale.menu_button()));
            if let Some(binding) = expected {
                assert!(menu.contains(binding), "{menu}");
            } else {
                assert!(!menu.contains("F2"));
            }
            assert!(matches!(
                hits.hit_target_at(2, 38),
                Some(HitTarget::ActionMenuButton)
            ));
            assert!(matches!(
                hits.hit_target_at(27, 38),
                Some(HitTarget::ActionMenuButton)
            ));
            assert!(!matches!(
                hits.hit_target_at(28, 38),
                Some(HitTarget::ActionMenuButton)
            ));
        }
    }
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
                    assert!(joined.contains(locale.menu_button()), "{rows:?}");
                    assert!(
                        joined.contains(locale.machine_picker_text("agent-working")),
                        "{width}x{height} {rows:?}"
                    );
                    assert!(joined.contains("/21"), "{rows:?}");
                    assert!(
                        joined.contains(&format!(
                            "codex · {}",
                            locale.machine_picker_text("agent-working")
                        )),
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
                            assert!(window.capacity > 0 && window.end() <= 21);
                            assert!(y + 1 < height - 1, "row crossed status: {y}");
                            assert!(!rows[y as usize + 1].trim().is_empty());
                        }
                    }
                } else {
                    assert!((0..height).all(|y| !matches!(
                        hits.hit_target_at(2, y),
                        Some(HitTarget::ActionMenuButton)
                    )));
                    assert!(!joined.contains("codex ·"), "hidden sidebar drew: {rows:?}");
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
            "                             │",
            "Workspaces  Browse Filter +  │",
            "                             │",
            "  ▾ ● Standalone             │",
            "    1 ● build                │",
            "        codex · Working      │",
            "                             │",
            "                             │",
            "                             │",
            "                             │",
            "[Menu]             Ctrl+B F2 │"
        ]
    );
    assert!(
        (0..30).any(|x| matches!(hits.hit_target_at(x, 1), Some(HitTarget::OpenMachineAgents)))
    );
    assert!(matches!(
        hits.hit_target_at(2, 4),
        Some(HitTarget::MachineAgentRow { .. })
    ));
    assert!(matches!(
        hits.hit_target_at(27, 5),
        Some(HitTarget::MachineAgentRow { .. })
    ));
    assert!(matches!(
        hits.hit_target_at(28, 5),
        Some(HitTarget::OverlaySurface)
    ));
    assert!(
        hits.hit_target_at(30, 5).is_none(),
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
    let bullet = Theme::rgb_test_dark().glyphs.status_bullet;
    assert!(joined.contains(&format!("1 {bullet}")) && joined.contains(&format!("2 {bullet}")));
    assert!(!joined.contains(&format!("3 {bullet}")));
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
    wide.label = format!("{} build\u{1b}]0;title\u{7}", wide.workspace);
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
        Theme::terminal_native().quantize(crate::tui_theme::ColorLevel::Monochrome),
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
        let ordinal_x = (0..30)
            .find(|x| terminal.backend().buffer()[(*x, y)].symbol() == "1")
            .expect("current nested Agent ordinal");
        assert_eq!(
            terminal.backend().buffer()[(ordinal_x, y)].fg,
            theme
                .current_context_style(theme.style(Role::HintLabel))
                .fg
                .unwrap_or_default()
        );
        assert_eq!(
            terminal.backend().buffer()[(2, y)].bg,
            theme.color(Role::SurfaceCurrent)
        );
        assert!((0..30).any(|x| {
            terminal.backend().buffer()[(x, y)]
                .modifier
                .contains(Modifier::BOLD)
        }));
        let before_seen = view
            .machines
            .sidebar_navigation_rows()
            .iter()
            .find_map(|entry| match entry {
                crate::machine_picker::SidebarRow::Agent { row, .. } => {
                    row.sidebar_agent().map(|agent| agent.seen)
                }
                _ => None,
            })
            .unwrap();
        let (rows, _) = paint(&view, Locale::EnUs, &theme, 120, 40);
        let marker = theme.current_context_marker();
        assert!(
            rows[y as usize].contains(&format!("1 {}", theme.glyphs.status_bullet)),
            "{:?}",
            rows[y as usize]
        );
        if marker == " " {
            assert_eq!(
                rows[y as usize].matches(theme.glyphs.active_marker).count(),
                0,
                "RGB/ANSI themes rely on the current-row style without an extra text marker"
            );
        } else {
            assert_eq!(rows[y as usize].matches(marker).count(), 1);
            assert_eq!(terminal.backend().buffer()[(1, y)].symbol(), marker);
        }
        assert_eq!(
            view.machines
                .sidebar_navigation_rows()
                .iter()
                .find_map(|entry| match entry {
                    crate::machine_picker::SidebarRow::Agent { row, .. } => {
                        row.sidebar_agent().map(|agent| agent.seen)
                    }
                    _ => None,
                })
                .unwrap(),
            before_seen
        );
        view.machines
            .observe_agents(vec![(Choice::Local, Agents::Unavailable)]);
        let (rows, hits) = paint(&view, Locale::EnUs, &theme, 120, 40);
        assert!(!rows.join("\n").contains("Agents unavailable"));
        assert!((0..40).all(|y| !matches!(
            hits.hit_target_at(2, y),
            Some(HitTarget::MachineAgentRow { .. })
        )));
    }
}

#[test]
fn bounded_sidebar_width_records_only_its_border_and_never_overrides_covering_help() {
    use crate::{client_view::ClientAction, tui_layout::SidebarWidth};
    for preferred in [24, 30, 60] {
        for (width, height) in [
            (120, 40),
            (80, 24),
            (60, 12),
            (120, 23),
            (120, 10),
            (50, 40),
            (15, 4),
        ] {
            let mut view = view(120, 40, 2);
            view.machines
                .set_sidebar_width(SidebarWidth::DEFAULT, SidebarWidth::new(preferred).unwrap());
            view.resize(width);
            let transition = view.decide_action(ClientAction::FocusBusinessSidebar);
            view.commit(transition, SessionActionDisposition::KeepNavigator);
            let area = Rect::new(0, 0, width, height);
            let target = DrawnSidebarWidth::new(area, view.sidebar_surface());
            view.machines.observe_agent_sidebar(target.is_some());
            for locale in [Locale::EnUs, Locale::ZhCn] {
                let mut ascii = Theme::terminal_native();
                ascii.glyphs = crate::tui_theme::GlyphSet::ASCII;
                for theme in [Theme::terminal_native(), ascii] {
                    let (_, map) = paint(&view, locale, &theme, width, height);
                    for y in 0..height {
                        for x in 0..width {
                            let actual = match map.hit_target_at(x, y) {
                                Some(HitTarget::SidebarResize(target)) => Some(*target),
                                _ => None,
                            };
                            let expected =
                                target.filter(|target| target.edge().contains((x, y).into()));
                            assert_eq!(
                                actual, expected,
                                "{width}x{height}, preferred {preferred}, ({x},{y})"
                            );
                        }
                    }
                    if target.is_some() {
                        let mut covered = view.clone();
                        let transition = covered.decide_action(ClientAction::OpenShortcutHelp);
                        covered.commit(transition, SessionActionDisposition::KeepNavigator);
                        let (_, covered_map) = paint(&covered, locale, &theme, width, height);
                        assert!((0..height).all(|y| (0..width).all(|x| !matches!(
                            covered_map.hit_target_at(x, y),
                            Some(HitTarget::SidebarResize(_))
                        ))));
                    }
                }
            }
            assert_eq!(view.machines.sidebar_width().columns(), preferred);
        }
    }
}

#[test]
fn width_preview_keeps_canonical_geometry_and_rejects_an_equal_rect_with_stale_preference() {
    use crate::tui_layout::SidebarWidth;
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let theme = Theme::rgb_test_dark();
        let mut view = view(120, 40, 1);
        view.machines.enter_sidebar_navigation();
        let before = view.clone();
        let target =
            DrawnSidebarWidth::new(Rect::new(0, 0, 120, 40), view.sidebar_surface()).unwrap();
        let mut drag = SidebarWidthDrag::begin(target, target.edge().x, 5).unwrap();
        drag.update(target.edge().x + 10);
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        let mut hits = HitMapBuilder::new();
        terminal
            .draw(|frame| {
                render_terminal_shell(frame, &view, locale, None, &theme, &mut hits);
                assert!(render_width_preview(
                    frame, &view, locale, &drag, &theme, &hits
                ));
            })
            .unwrap();
        assert_eq!(view, before);
        assert_eq!(view.machines.sidebar_width(), SidebarWidth::DEFAULT);
        let map = hits.finish();
        for y in 0..39 {
            assert_eq!(
                map.hit_target_at(target.edge().x, y),
                Some(&HitTarget::SidebarResize(target))
            );
            assert!(!matches!(
                map.hit_target_at(drag.preview.x, y),
                Some(HitTarget::SidebarResize(_))
            ));
            assert_eq!(
                terminal.backend().buffer()[(drag.preview.x, y)].symbol(),
                "│"
            );
        }
        let mut status = String::new();
        let mut column = 0;
        while column < 120 {
            let symbol = terminal.backend().buffer()[(column, 39)].symbol();
            status.push_str(symbol);
            column += u16::try_from(display_width(symbol).max(1)).unwrap();
        }
        assert!(
            status.contains(&locale.sidebar_width_drag_hint(40, true)),
            "{status}"
        );
        let hint_x =
            120 - u16::try_from(display_width(&locale.sidebar_width_drag_hint(40, true))).unwrap();
        assert_eq!(terminal.backend().buffer()[(hint_x - 1, 39)].symbol(), " ");
        view.machines
            .set_sidebar_width(SidebarWidth::DEFAULT, SidebarWidth::new(60).unwrap());
        view.resize(60);
        let transition = view.decide_action(crate::client_view::ClientAction::FocusBusinessSidebar);
        view.commit(transition, SessionActionDisposition::KeepNavigator);
        let old = DrawnSidebarWidth::new(Rect::new(0, 0, 60, 12), view.sidebar_surface()).unwrap();
        let drag = SidebarWidthDrag::begin(old, old.edge().x, 3).unwrap();
        view.machines
            .set_sidebar_width(old.preferred(), SidebarWidth::new(40).unwrap());
        let current = DrawnSidebarWidth::new(old.viewport, view.sidebar_surface()).unwrap();
        assert_eq!(old.sidebar, current.sidebar);
        assert_ne!(old, current);
        let mut terminal = Terminal::new(TestBackend::new(60, 12)).unwrap();
        let mut hits = HitMapBuilder::new();
        terminal
            .draw(|frame| {
                render_terminal_shell(frame, &view, locale, None, &theme, &mut hits);
                let before = frame.buffer_mut().clone();
                assert!(!render_width_preview(
                    frame, &view, locale, &drag, &theme, &hits
                ));
                assert_eq!(
                    *frame.buffer_mut(),
                    before,
                    "rejected preview paints nothing"
                );
            })
            .unwrap();
    }
}

#[test]
fn width_preview_separates_status_without_erasing_the_preceding_complete_grapheme() {
    for (width, height) in [(60, 12), (120, 40)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let mut ascii = Theme::terminal_native();
            ascii.glyphs = crate::tui_theme::GlyphSet::ASCII;
            for theme in [Theme::terminal_native(), Theme::rgb_test_dark(), ascii] {
                // The final glyph starts at the separator or spans it from the left.
                for (glyph, columns_before_hint) in [("X", 1), ("中", 1), ("中", 2), ("🙂", 2)]
                {
                    let view = view(width, height, 1);
                    let area = Rect::new(0, 0, width, height);
                    let target = DrawnSidebarWidth::new(area, view.sidebar_surface()).unwrap();
                    let drag = SidebarWidthDrag::begin(target, target.edge().x, 5).unwrap();
                    let status = project_shell_areas(area, view.sidebar_surface()).status;
                    let hint = locale.sidebar_width_drag_hint(30, width >= 100);
                    let hint_x = width - u16::try_from(display_width(&hint)).unwrap();
                    let erased_x = hint_x - columns_before_hint;
                    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                    let mut hits = HitMapBuilder::new();
                    terminal
                        .draw(|frame| {
                            render_terminal_shell(frame, &view, locale, None, &theme, &mut hits);
                            frame.buffer_mut().set_string(
                                status.x,
                                status.y,
                                "p".repeat(usize::from(width)),
                                theme.style(Role::TextPrimary),
                            );
                            frame.buffer_mut().set_string(
                                erased_x - 2,
                                status.y,
                                format!("前{glyph}"),
                                theme.style(Role::AccentFocus),
                            );
                            let before = frame.buffer_mut().clone();
                            assert!(render_width_preview(
                                frame, &view, locale, &drag, &theme, &hits
                            ));
                            for column in status.x..erased_x {
                                assert_eq!(
                                    frame.buffer_mut()[(column, status.y)],
                                    before[(column, status.y)],
                                    "preserve the complete prefix and its style"
                                );
                            }
                            for column in erased_x..hint_x {
                                assert_eq!(frame.buffer_mut()[(column, status.y)].symbol(), " ");
                            }
                        })
                        .unwrap();
                    let buffer = terminal.backend().buffer();
                    let mut painted_hint = String::new();
                    let mut column = hint_x;
                    while column < status.right() {
                        let symbol = buffer[(column, status.y)].symbol();
                        painted_hint.push_str(symbol);
                        column += u16::try_from(display_width(symbol).max(1)).unwrap();
                    }
                    assert_eq!(painted_hint, hint);
                }
            }
        }
    }
}
