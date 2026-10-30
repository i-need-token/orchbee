use super::super::workspace_sidebar::agent_tokens;
use super::super::{project_shell_areas, render_terminal_shell};
use super::*;
use crate::{
    client_view::{
        ClientAction, ClientActionDisposition, KeyCode, KeyEvent, Modifiers, PrefixAction,
        PrefixInput,
    },
    machine_agents::{
        Observation as Agents,
        test_support::{checkpoint, row},
    },
    machine_picker::Choice,
};
use ratatui::{Terminal, backend::TestBackend, style::Modifier};
mod empty;

#[test]
fn default_agent_title_prefers_explicit_name_then_task_and_never_an_automatic_number() {
    for (name, title, expected) in [
        (Some("alpha"), "Fix input", "alpha"),
        (None, "Fix input", "Fix input"),
        (Some("  "), "1", "codex"),
        (None, "", "codex"),
    ] {
        let mut view = view(120, 40, 1);
        let mut agent = row(23);
        agent.name = name.map(str::to_owned);
        agent.label = title.into();
        agent.metadata.title = None;
        view.machines.observe_agents(vec![(
            Choice::Local,
            Agents::Ready(checkpoint(vec![agent])),
        )]);
        let navigation = view.machines.sidebar_navigation_rows();
        let agent = navigation
            .iter()
            .find_map(|entry| match entry {
                crate::machine_picker::SidebarRow::Agent { row, .. } => Some(row),
                _ => None,
            })
            .unwrap();
        let lines = agent_tokens::lines(&view, agent, Locale::EnUs, &Theme::terminal_native());
        let actual: String = lines[0]
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        assert_eq!(actual, expected);
    }
}

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
                let navigation = view.machines.sidebar_navigation_rows();
                let projected = navigation
                    .iter()
                    .find_map(|entry| match entry {
                        crate::machine_picker::SidebarRow::Agent { row, .. } => {
                            Some(agent_tokens::lines(&view, row, locale, &theme))
                        }
                        _ => None,
                    })
                    .unwrap()
                    .into_iter()
                    .map(|line| {
                        line.spans
                            .into_iter()
                            .map(|span| span.content.into_owned())
                            .collect::<String>()
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                assert!(projected.contains("custom title"), "{projected}");
                if let Some(raw) = title {
                    assert!(projected.contains(raw), "{projected}");
                    assert_eq!(projected.matches("修复🙂标题").count(), 2);
                } else {
                    assert!(!projected.contains("修复🙂标题"));
                }
                let (rows, hits) = paint(&view, locale, &theme, 120, 40);
                let text = rows.join("\n");
                assert!(text.contains("custom"), "{rows:?}");
                if title.is_some() {
                    assert_eq!(rows.iter().filter(|s| s.contains("修复")).count(), 2);
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
                assert!(exact.iter().all(|s|matches!(&s.context,Some(crate::workbench_client::ClientStartupTarget::Agent(t)) if t.as_ref()==&target)));
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
        orchbee_local_runtime::AgentSidebarSettings::from_profile(&profile).unwrap(),
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
            let navigation = view.machines.sidebar_navigation_rows();
            let projected = navigation
                .iter()
                .find_map(|entry| match entry {
                    crate::machine_picker::SidebarRow::Agent { row, .. } => {
                        Some(agent_tokens::lines(&view, row, locale, &theme))
                    }
                    _ => None,
                })
                .unwrap()
                .into_iter()
                .map(|line| {
                    line.spans
                        .into_iter()
                        .map(|span| span.content.into_owned())
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("\n");
            assert!(
                projected.contains("Writing code")
                    && projected.contains("Task · Named pane")
                    && projected.contains("codex · ORCH-42"),
                "{projected}"
            );
            let (rows, hits) = paint(&view, locale, &theme, 120, 40);
            let rendered = (0..40)
                .filter_map(|y| match hits.hit_target_at(2, y) {
                    Some(HitTarget::MachineAgentRow { selected, .. })
                        if matches!(&selected.context,Some(crate::workbench_client::ClientStartupTarget::Agent(actual)) if actual.as_ref()==&target) =>
                    {
                        Some(rows[y as usize].as_str())
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            assert!(
                rendered.contains("Writing code")
                    && rendered.contains("Task · Named pane")
                    && rendered.contains("codex · ORCH-42")
                    && !rendered.contains(theme.glyphs.ellipsis),
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
            assert!((0..40).filter_map(|y| match hits.hit_target_at(2,y) {Some(HitTarget::MachineAgentRow{selected,..})=>Some(selected),_=>None}).all(|selected| matches!(&selected.context,Some(crate::workbench_client::ClientStartupTarget::Agent(t)) if t.as_ref()==&target)));
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
        matches!(&hits[0].context,Some(crate::workbench_client::ClientStartupTarget::Agent(t)) if t.as_ref()==&target)
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
            let Some(crate::workbench_client::ClientStartupTarget::Agent(t)) = &selected.context
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
        vec![2, 4, 3]
    );
    for ys in spans.values() {
        let gap = ys[0];
        assert!(matches!(
            hits.hit_target_at(2, gap),
            Some(HitTarget::MachineAgentRow { .. })
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
    // Settings alone on the last row leaves a 12-row host the same six-row list body.
    // Explicitly show the compact sidebar; the full-width view's intent was docked.
    for key in [
        KeyEvent::press(KeyCode::Char('b'), Modifiers::CTRL),
        KeyEvent::press(KeyCode::Char('b'), Modifiers::NONE),
    ] {
        let t = view.decide(PrefixInput::Key(key));
        view.commit(t, ClientActionDisposition::KeepNavigator);
    }
    let (_, hits) = paint(&view, Locale::EnUs, &Theme::terminal_native(), 60, 12);
    let window = (0..12)
        .find_map(|y| match hits.hit_target_at(2, y) {
            Some(HitTarget::MachineAgentRow { window, .. }) => Some(*window),
            _ => None,
        })
        .unwrap();
    // The Recents heading and its Workspace each take one row before the first Agent.
    assert_eq!(window.capacity, 3);
    view.machines.queue_space_scroll(window, false);
    assert!(view.machines.settle_sidebar_scroll());
    let (_, hits) = paint(&view, Locale::EnUs, &Theme::terminal_native(), 60, 12);
    let mut painted = std::collections::BTreeMap::new();
    for y in 0..12 {
        if let Some(HitTarget::MachineAgentRow { selected, window }) = hits.hit_target_at(2, y) {
            // The six-row body fits both remaining entries exactly: the first
            // visible entry owns no leading gap (three Claude rows), then the
            // custom entry owns its one-row gap plus two content rows.
            assert_eq!((window.top, window.end()), (3, 5));
            let Some(crate::workbench_client::ClientStartupTarget::Agent(target)) =
                &selected.context
            else {
                panic!()
            };
            *painted.entry(target.agent.as_str()).or_insert(0) += 1;
        }
    }
    assert_eq!(
        painted,
        std::collections::BTreeMap::from([("claude", 3), ("custom", 3)])
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
        let connector_x = (0..30)
            .find(|x| buffer[(*x, y)].symbol() == theme.glyphs.tree_guide)
            .expect("nested Agent connector");
        assert_eq!(
            buffer[(connector_x, y)].fg,
            theme.style(Role::FrameGuide).fg.unwrap()
        );
        let expected = theme.sidebar_token_style(
            Role::TextPrimary,
            orchbee_local_runtime::SidebarTokenStyle {
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
            view.commit(transition, ClientActionDisposition::KeepNavigator);
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
            workspace: Some("Workspace".into()),
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
            assert!(text.contains(locale.machine_picker_text("agent-search")));
            assert!(!text.contains(locale.machine_picker_text("no-agents")));
            assert!((0..40).all(|y| (0..30).all(|x| !matches!(
                hits.hit_target_at(x, y),
                Some(HitTarget::MachineAgentRow { .. })
            ))));
            assert!((0..40).any(|y| {
                (0..30).any(|x| matches!(hits.hit_target_at(x, y), Some(HitTarget::SearchButton)))
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
                .contains(locale.machine_picker_text("agent-search"))
        );
        assert!((0..40).all(|y| !matches!(
            hits.hit_target_at(2, y),
            Some(HitTarget::MachineAgentRow { .. })
        )));
    }
}

#[test]
fn nested_agents_share_one_tree_without_a_second_sidebar_region() {
    let view = view(120, 40, 2);
    let theme = Theme::terminal_native();
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let (rows, hits) = paint(&view, locale, &theme, 120, 40);
        let text = rows.join("\n");
        assert!(text.contains(theme.glyphs.tree_guide), "{rows:?}");
        assert!(!text.contains("├─") && !text.contains("╰─"), "{rows:?}");
        assert!(!text.contains(&format!("{} 2", locale.shell().agents)));
        assert!(text.contains(locale.machine_picker_text("agent-search")));
        assert_eq!(
            (0..40)
                .filter(|y| matches!(
                    hits.hit_target_at(2, *y),
                    Some(HitTarget::MachineAgentRow { .. })
                ))
                .count(),
            2,
            "each Agent occupies one identity line"
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
            let navigation = view.machines.sidebar_navigation_rows();
            let projected = navigation
                .iter()
                .find_map(|entry| match entry {
                    crate::machine_picker::SidebarRow::Agent { row, .. } => {
                        Some(agent_tokens::lines(&view, row, locale, &theme))
                    }
                    _ => None,
                })
                .unwrap();
            let projected_title = projected[0]
                .spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>();
            assert_eq!(projected_title, "Input handling");
            assert_eq!(projected.len(), 1, "state text lives in the hover hint");
            let y = (0..40)
                .find(|y| {
                    matches!(
                        hits.hit_target_at(2, *y),
                        Some(HitTarget::MachineAgentRow { .. })
                    )
                })
                .unwrap();
            assert!(rows[y as usize].contains("Input"), "{rows:?}");
            assert!(
                rows[y as usize].contains(
                    theme.agent_glyph(orchbee_workbench_protocol::MetadataState::Working)
                )
            );
            assert!(!rows.join("\n").contains("Local /"));
            assert!(!rows.join("\n").contains("本地 /"));
            assert!(!rows.join("\n").contains("space /"));
            assert!(
                matches!(hits.hit_target_at(2, y), Some(HitTarget::MachineAgentRow { selected, .. })
                if matches!(&selected.context, Some(crate::workbench_client::ClientStartupTarget::Agent(actual)) if actual.as_ref() == &target))
            );
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
        assert!(!rows[y as usize].contains("Working"), "{rows:?}");
        assert!(!rows[y as usize].contains('·'), "{rows:?}");
    }
}

#[test]
fn settings_sits_alone_on_the_sidebar_last_row_as_an_exact_action_without_a_shortcut() {
    use crate::tui_layout::SidebarWidth;

    for (width, height) in [(120, 40), (80, 24), (60, 12)] {
        for count in [0, 1, 30] {
            let mut view = view(width, height, count);
            view.machines
                .set_sidebar_width(SidebarWidth::DEFAULT, SidebarWidth::new(30).unwrap());
            for locale in [Locale::EnUs, Locale::ZhCn] {
                let (rows, hits) = paint(&view, locale, &Theme::terminal_native(), width, height);
                let last = height - 1;
                let footer = &rows[usize::from(last)];
                assert!(footer.contains("[ "), "{footer:?}");
                assert!(footer.contains(locale.settings_title()), "{footer:?}");
                assert!(footer.contains(" ]"), "{footer:?}");
                let configured_hint = view
                    .bindings()
                    .action_labels_with_glyphs(
                        PrefixAction::Client(ClientAction::OpenSettings),
                        Theme::terminal_native().glyphs.key_labels(),
                    )
                    .into_iter()
                    .next()
                    .unwrap();
                assert!(!footer.contains(&configured_hint), "{footer:?}");
                let exact = (0..29)
                    .filter(|x| {
                        matches!(
                            hits.hit_target_at(*x, last),
                            Some(HitTarget::SettingsButton)
                        )
                    })
                    .collect::<Vec<_>>();
                assert!(!exact.is_empty());
                assert_eq!(
                    exact,
                    (*exact.first().unwrap()..=*exact.last().unwrap()).collect::<Vec<_>>()
                );
                assert!(exact.len() < 29);
                // No rule or shortcut row separates the list from Settings.
                for y in [last - 2, last - 1] {
                    let row = &rows[usize::from(y)];
                    assert!(!row.starts_with('─'), "{row:?}");
                    assert!(
                        !row.contains(locale.sidebar_navigation_action(
                            crate::machine_picker::SidebarCommand::Exit
                        )),
                        "{row:?}"
                    );
                }
            }
        }
    }
}

/// The range row exists only while the list overflows, so a list that fits reaches the row
/// directly above Settings.
#[test]
fn list_runs_down_to_settings_and_reserves_the_range_row_only_while_overflowing() {
    for (width, height) in [(120, 40), (80, 24)] {
        let above = height - 2;
        let mut reached = false;
        for count in 0..40 {
            let view = view(width, height, count);
            let (rows, hits) = paint(
                &view,
                Locale::EnUs,
                &Theme::terminal_native(),
                width,
                height,
            );
            let browse = Locale::EnUs.machine_picker_text("workspace-sidebar-open");
            let list_row = |y: u16| {
                (0..28).any(|x| {
                    matches!(
                        hits.hit_target_at(x, y),
                        Some(
                            HitTarget::MachineAgentRow { .. }
                                | HitTarget::MachineWorkspaceRow { .. }
                        )
                    )
                })
            };
            let range = rows[usize::from(above)].contains(browse);
            if range {
                assert!(list_row(above - 1), "{width}x{height} {count}: {rows:?}");
            }
            reached |= !range && list_row(above);
        }
        assert!(reached, "{width}x{height}: no list ever reached Settings");
    }
}

#[test]
fn sidebar_input_ownership_never_moves_settings_or_adds_a_hint_row() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [Theme::terminal_native(), Theme::one_light()] {
            for input in [
                SidebarInput::Terminal,
                SidebarInput::Navigation,
                SidebarInput::Suppressed,
            ] {
                let mut view = view(120, 40, 1);
                if input != SidebarInput::Terminal {
                    assert!(view.machines.enter_sidebar_navigation());
                }
                let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
                let mut hits = HitMapBuilder::new();
                terminal
                    .draw(|frame| {
                        let sidebar = project_shell_areas(frame.area(), view.sidebar_surface())
                            .sidebar
                            .unwrap();
                        render(
                            frame,
                            sidebar,
                            RenderContext {
                                held: None,
                                creation: None,
                                view: &view,
                                sidebar_surface: view.sidebar_surface(),
                                locale,
                                backend_status: SessionStatus {
                                    text: "Ready",
                                    role: Role::StatusSuccess,
                                },
                                theme: &theme,
                                input,
                            },
                            Some(&mut hits),
                        );
                    })
                    .unwrap();
                let buffer = terminal.backend().buffer();
                let mut above = String::new();
                let mut x = 0;
                while x < 28 {
                    let symbol = buffer[(x, 38)].symbol();
                    above.push_str(symbol);
                    x += u16::try_from(display_width(symbol).max(1)).unwrap();
                }
                // Return and Help moved to the floating status; the row above Settings is list.
                assert!(above.trim().is_empty(), "{input:?}: {above:?}");
                let hits = hits.finish();
                assert!(
                    (0..28).any(|x| matches!(
                        hits.hit_target_at(x, 39),
                        Some(HitTarget::SettingsButton)
                    ))
                );
                assert!((0..28).all(|x| !matches!(
                    hits.hit_target_at(x, 38),
                    Some(HitTarget::SettingsButton)
                )));
            }
        }
    }
}

#[test]
fn sidebar_border_is_the_collection_focus_cue_without_changing_current_context() {
    let mut view = view(60, 12, 1);
    let current = view.machines.current_context.clone();
    let theme = Theme::one_dark();
    let border = |view: &ClientView| {
        let mut terminal = Terminal::new(TestBackend::new(60, 12)).unwrap();
        let mut hits = HitMapBuilder::new();
        terminal
            .draw(|frame| {
                render_terminal_shell(frame, view, Locale::EnUs, None, &theme, &mut hits);
            })
            .unwrap();
        terminal.backend().buffer()[(29, 1)].fg
    };
    assert_eq!(border(&view), theme.color(Role::FrameDivider));
    assert!(view.machines.enter_sidebar_navigation());
    assert_eq!(border(&view), theme.color(Role::AccentFocus));
    assert_eq!(view.machines.current_context, current);
    assert!(view.machines.request.is_none());
}

#[test]
fn captured_input_suppresses_sidebar_focus_chrome_and_local_hints_without_forgetting_navigation() {
    let mut view = view(60, 12, 1);
    assert!(view.machines.enter_sidebar_navigation());
    let theme = Theme::one_dark();
    let local_hint = crate::machine_picker::sidebar_binding_label(
        crate::machine_picker::SidebarCommand::NewWorkspace,
        theme.glyphs.key_labels(),
    )
    .unwrap();
    let paint_with_owner = |owns_keyboard| {
        let mut terminal = Terminal::new(TestBackend::new(60, 12)).unwrap();
        terminal
            .draw(|frame| {
                let sidebar = project_shell_areas(Rect::new(0, 0, 60, 12), view.sidebar_surface())
                    .sidebar
                    .unwrap();
                render(
                    frame,
                    sidebar,
                    RenderContext {
                        held: None,
                        creation: None,
                        view: &view,
                        sidebar_surface: view.sidebar_surface(),
                        locale: Locale::EnUs,
                        backend_status: SessionStatus {
                            text: "Ready",
                            role: Role::StatusSuccess,
                        },
                        theme: &theme,
                        input: if owns_keyboard {
                            SidebarInput::Navigation
                        } else {
                            SidebarInput::Suppressed
                        },
                    },
                    None,
                );
            })
            .unwrap();
        terminal.backend().buffer().clone()
    };

    let owned = paint_with_owner(true);
    let captured = paint_with_owner(false);
    let text = |buffer: &ratatui::buffer::Buffer| {
        buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>()
    };
    assert_eq!(owned[(29, 1)].fg, theme.color(Role::AccentFocus));
    assert!(text(&owned).contains(&local_hint));
    assert!(text(&owned).contains(theme.glyphs.navigation_marker));
    assert_eq!(captured[(29, 1)].fg, theme.color(Role::FrameDivider));
    assert!(!text(&captured).contains(&local_hint));
    assert!(!text(&captured).contains(theme.glyphs.navigation_marker));
    assert!(view.machines.sidebar_navigation_active());
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
                    assert!(joined.contains(locale.settings_title()), "{rows:?}");
                    assert!(
                        joined.contains(
                            theme.agent_glyph(orchbee_workbench_protocol::MetadataState::Working)
                        ),
                        "{width}x{height} {rows:?}"
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
                            // The Recents heading, its Workspace and twenty Agents.
                            assert!(window.capacity > 0 && window.end() <= 22);
                            // The range footer appears exactly when the window clips entries.
                            assert_eq!(
                                joined.contains("/22"),
                                window.top > 0 || window.end() < 22,
                                "{rows:?}"
                            );
                            let exact = (0..height)
                                .filter(|candidate| {
                                    matches!(hits.hit_target_at(2, *candidate), Some(HitTarget::MachineAgentRow { selected: actual, .. }) if actual == selected)
                                })
                                .collect::<Vec<_>>();
                            assert_eq!(exact.len(), 1, "{rows:?}");
                            assert!(exact.iter().all(|candidate| *candidate < height - 1));
                            assert!(rows[usize::from(exact[0])].contains("build"), "{rows:?}");
                        }
                    }
                } else {
                    assert!((0..height).all(|y| !matches!(
                        hits.hit_target_at(2, y),
                        Some(HitTarget::SettingsButton)
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
    let theme = Theme::terminal_native();
    let (rows, hits) = paint(&view, Locale::EnUs, &theme, 60, 12);
    assert!(rows[1].contains("[ New workspace ]"), "{rows:?}");
    assert!(rows[2].contains("[ Search ]"), "{rows:?}");
    // The Recents heading names the group without a member count.
    assert_eq!(
        rows[3].split_whitespace().collect::<Vec<_>>(),
        ["Recents", "│"],
        "{rows:?}"
    );
    assert!(rows[4].contains("Workspace"), "{rows:?}");
    assert!(!rows[4].contains(theme.glyphs.ellipsis), "{rows:?}");
    assert!(
        rows[5].contains(theme.glyphs.tree_guide) && rows[5].contains("build"),
        "{rows:?}"
    );
    assert!(rows[11].contains("[ Settings ]"), "{rows:?}");
    assert!((0..30).any(|x| matches!(hits.hit_target_at(x, 2), Some(HitTarget::SearchButton))));
    for x in [2, 27] {
        assert!(matches!(
            hits.hit_target_at(x, 5),
            Some(HitTarget::MachineAgentRow { .. })
        ));
    }
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
fn sidebar_shares_source_order_and_never_lists_unavailable_rows() {
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
    let glyphs = Theme::rgb_test_dark().glyphs;
    assert_eq!(
        rows.iter()
            .filter(|line| line
                .chars()
                .nth(1)
                .is_some_and(|ch| glyphs.tree_guide.starts_with(ch)))
            .count(),
        2,
        "{joined}"
    );
    assert!(!joined.contains("remote"), "{joined}");
    let selected = (0..40)
        .find_map(|y| match hits.hit_target_at(2, y) {
            Some(HitTarget::MachineAgentRow { selected, .. }) => Some(selected),
            _ => None,
        })
        .unwrap();
    assert!(
        matches!(&selected.context,Some(crate::workbench_client::ClientStartupTarget::Agent(t)) if t.member==row(24).target.member)
    );
}

#[test]
fn sidebar_truncates_wide_and_unsafe_text_without_changing_navigation_identity() {
    let mut view = view(120, 40, 1);
    let mut wide = row(23);
    wide.workspace = "宽项目👩‍💻".repeat(10);
    wide.label = format!("{} build\u{1b}]0;title\u{7}", wide.workspace);
    wide.sequence = orchbee_workbench_protocol::WorkbenchRevision::new(2);
    let target = wide.target.clone();
    let mut round = checkpoint(vec![wide]).as_ref().clone();
    round.report_revision = orchbee_workbench_protocol::WorkbenchRevision::new(3);
    view.machines.observe_agents(vec![(
        Choice::Local,
        Agents::Ready(std::sync::Arc::new(round)),
    )]);
    let theme = Theme::terminal_native();
    let (rows, hits) = paint(&view, Locale::ZhCn, &theme, 120, 40);
    let rendered = (0..40)
        .filter_map(|y| match hits.hit_target_at(2, y) {
            Some(HitTarget::MachineAgentRow { selected, .. })
                if matches!(&selected.context,Some(crate::workbench_client::ClientStartupTarget::Agent(actual)) if actual.as_ref()==&target) =>
            {
                Some(rows[y as usize].as_str())
            }
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains(theme.glyphs.ellipsis)
            && rendered.contains(theme.glyphs.status_bullet)
            && !rendered.contains('\u{1b}')
            && !rendered.contains('\u{7}'),
        "{rendered:?}: {rows:?}"
    );
    assert!(rendered.contains('宽'), "{rows:?}");
    let selected = (0..40)
        .find_map(|y| match hits.hit_target_at(2, y) {
            Some(HitTarget::MachineAgentRow { selected, .. }) => Some(selected),
            _ => None,
        })
        .unwrap();
    assert!(
        matches!(&selected.context,Some(crate::workbench_client::ClientStartupTarget::Agent(t)) if t.as_ref()==&target)
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
            Some(crate::workbench_client::ClientMemberTarget {
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
        // The Agent surface includes a blank cell before its state, after the guide and gap.
        assert_eq!(
            terminal.backend().buffer()[(5, y)].bg,
            theme.color(Role::SurfaceCurrent)
        );
        let title_x = (0..30)
            .find(|x| {
                terminal.backend().buffer()[(*x, y)]
                    .symbol()
                    .chars()
                    .any(char::is_alphanumeric)
            })
            .unwrap();
        assert!(
            !terminal.backend().buffer()[(title_x, y)]
                .modifier
                .contains(Modifier::BOLD)
        );
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
        // The fill alone marks the current row, underlined only where no fill can show.
        assert_eq!(
            terminal.backend().buffer()[(title_x, y)]
                .modifier
                .contains(Modifier::UNDERLINED),
            theme
                .underline_collapsed_current()
                .style(Role::SurfaceCurrent)
                .add_modifier
                .contains(Modifier::UNDERLINED)
        );
        let (rows, _) = paint(&view, Locale::EnUs, &theme, 120, 40);
        assert!(
            rows[y as usize]
                .contains(theme.agent_glyph(orchbee_workbench_protocol::MetadataState::Working)),
            "{:?}",
            rows[y as usize]
        );
        assert_eq!(
            rows[y as usize].matches(theme.glyphs.active_marker).count(),
            0,
            "no theme adds a text marker to the current row"
        );
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
            view.commit(transition, ClientActionDisposition::KeepNavigator);
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
                        covered.commit(transition, ClientActionDisposition::KeepNavigator);
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
fn width_draft_moves_painted_geometry_and_rejects_an_equal_rect_with_stale_preference() {
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
        let resize = crate::tui_layout::ResizeProjection::Sidebar {
            source: target,
            width: drag.width,
        };
        terminal
            .draw(|frame| {
                crate::tui_render::render_terminal_shell_with_resize(
                    frame, &view, locale, None, &theme, &mut hits, &resize,
                );
                assert!(render_width_preview(frame, &view, &drag, &theme, &hits));
            })
            .unwrap();
        assert_eq!(view, before);
        assert_eq!(view.machines.sidebar_width(), SidebarWidth::DEFAULT);
        let map = hits.finish();
        for y in 0..39 {
            assert_eq!(
                map.hit_target_at(drag.preview.x, y),
                Some(&HitTarget::SidebarResize(drag.projected_target()))
            );
            assert!(!matches!(
                map.hit_target_at(target.edge().x, y),
                Some(HitTarget::SidebarResize(_))
            ));
            assert_eq!(
                terminal.backend().buffer()[(drag.preview.x, y)].symbol(),
                "│"
            );
        }
        view.machines
            .set_sidebar_width(SidebarWidth::DEFAULT, SidebarWidth::new(60).unwrap());
        view.resize(60);
        if matches!(
            view.sidebar_surface(),
            crate::client_view::SidebarSurface::Hidden
        ) {
            let transition =
                view.decide_action(crate::client_view::ClientAction::ToggleBusinessSidebar);
            view.commit(transition, ClientActionDisposition::KeepNavigator);
        }
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
                assert!(!render_width_preview(frame, &view, &drag, &theme, &hits));
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
fn width_preview_changes_only_the_single_line_foreground_without_footer_feedback() {
    for (width, height) in [(60, 12), (80, 24), (120, 40)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let mut ascii = Theme::terminal_native();
            ascii.glyphs = crate::tui_theme::GlyphSet::ASCII;
            for theme in [
                Theme::terminal_native(),
                Theme::one_light(),
                Theme::one_dark(),
                ascii,
            ] {
                for displacement in [-6, 0, 10] {
                    let view = view(width, height, 1);
                    let area = Rect::new(0, 0, width, height);
                    let target = DrawnSidebarWidth::new(area, view.sidebar_surface()).unwrap();
                    let mut drag = SidebarWidthDrag::begin(target, target.edge().x, 5).unwrap();
                    drag.update(target.edge().x.saturating_add_signed(displacement));
                    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                    let mut hits = HitMapBuilder::new();
                    let resize = crate::tui_layout::ResizeProjection::Sidebar {
                        source: target,
                        width: drag.width,
                    };
                    terminal
                        .draw(|frame| {
                            crate::tui_render::render_terminal_shell_with_resize(
                                frame, &view, locale, None, &theme, &mut hits, &resize,
                            );
                            frame.buffer_mut().set_string(
                                target.sidebar.area.right(),
                                height - 1,
                                "终端 tail 🙂 untouched",
                                theme.style(Role::TextPrimary),
                            );
                            let before = frame.buffer_mut().clone();
                            assert!(render_width_preview(frame, &view, &drag, &theme, &hits));
                            for y in 0..height {
                                for x in 0..width {
                                    let cell = &frame.buffer_mut()[(x, y)];
                                    let original = &before[(x, y)];
                                    if drag.preview.contains((x, y).into()) {
                                        assert_eq!(cell.bg, original.bg, "no highlight strip");
                                        assert_eq!(cell.modifier, original.modifier);
                                        assert_eq!(cell.fg, theme.color(Role::AccentFocus));
                                        assert_eq!(
                                            cell.symbol(),
                                            if theme.glyphs.ellipsis == ".." {
                                                "|"
                                            } else {
                                                "│"
                                            }
                                        );
                                    } else {
                                        assert_eq!(cell, original, "no shadow or hint at {x},{y}");
                                    }
                                }
                            }
                        })
                        .unwrap();
                }
            }
        }
    }
}
