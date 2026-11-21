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

/// The default first line of one Agent row and the whole text a cut line gives in its tooltip.
fn default_line(
    agent: crate::machine_agents::Row,
    theme: &Theme,
    width: usize,
) -> (Vec<ratatui::text::Span<'static>>, String) {
    let mut view = view(120, 40, 0);
    view.machines.observe_agents(vec![(
        Choice::Local,
        Agents::Ready(checkpoint(vec![agent])),
    )]);
    let navigation = view.machines.sidebar_navigation_rows();
    let row = navigation
        .iter()
        .find_map(|entry| match entry {
            crate::machine_picker::SidebarRow::Agent { row, .. } => Some(row),
            _ => None,
        })
        .unwrap();
    let projected = agent_tokens::projection(&view, row, Locale::EnUs, theme, width);
    let names = projected.names().to_vec();
    assert_eq!(names.len(), 1);
    let (mut lines, _) = projected.into_parts();
    (lines.remove(0).spans, names[0].1.text.clone())
}

#[test]
fn default_agent_row_names_the_agent_then_its_provider_and_never_an_automatic_number() {
    use orchbee_workbench_protocol::{MetadataTokenValue, TerminalTitleObservation};
    let observed = |title: Option<&str>| TerminalTitleObservation::Observed {
        revision: orchbee_workbench_protocol::WorkbenchRevision::new(1),
        title: title.map(str::to_owned),
    };
    let unavailable = TerminalTitleObservation::Unavailable {};
    // (person's name, explicit title, Panel label, custom label, terminal title, row)
    for (name, explicit, label, custom, title, expected) in [
        // A person's name, then an explicit title or Panel name, outrank the program's title.
        (
            Some("alpha"),
            Some("Fix input"),
            "frontend",
            true,
            observed(Some("Respond to greeting")),
            "alpha · Codex",
        ),
        (
            None,
            Some("Fix input"),
            "frontend",
            false,
            observed(Some("Respond to greeting")),
            "Fix input · Codex",
        ),
        (
            None,
            None,
            "frontend",
            true,
            observed(Some("Respond to greeting")),
            "frontend · Codex",
        ),
        // The program's title, without its activity prefix, outranks a creation label.
        (
            None,
            None,
            "build",
            false,
            observed(Some("⠋ Respond to greeting | Desktop")),
            "Respond to greeting | Desktop · Codex",
        ),
        (
            None,
            None,
            "build",
            false,
            unavailable.clone(),
            "build · Codex",
        ),
        (None, None, "build", false, observed(None), "build · Codex"),
        // A title naming only the provider or its program, a console launch path, an automatic
        // number and blank text name nothing, so the provider shows alone.
        (None, None, "1", false, observed(Some("Codex")), "Codex"),
        (None, None, "1", false, observed(Some("✳ codex")), "Codex"),
        (
            None,
            None,
            "1",
            false,
            observed(Some(r"C:\Tools\codex.exe")),
            "Codex",
        ),
        (Some("  "), None, "1", false, unavailable.clone(), "Codex"),
        (None, None, " \t", false, observed(Some("  ")), "Codex"),
        (Some("codex"), None, "", false, unavailable.clone(), "Codex"),
    ] {
        let mut agent = row(23);
        agent.name = name.map(str::to_owned);
        agent.metadata.title = explicit.map(|title| MetadataTokenValue::parse(title).unwrap());
        agent.label = label.into();
        agent.label_custom = custom;
        agent.terminal_title = title.clone();
        let (spans, whole) = default_line(agent, &Theme::terminal_native(), usize::MAX);
        let actual: String = spans.iter().map(|span| span.content.as_ref()).collect();
        assert_eq!(actual, expected, "{title:?}");
        assert_eq!(whole, expected);
    }
    // A Claude idle title names its provider even when spelled as the product.
    let mut agent = row(23);
    agent.target.agent = "claude".into();
    agent.label = "1".into();
    agent.terminal_title = observed(Some("✳ Claude Code"));
    let (spans, _) = default_line(agent, &Theme::terminal_native(), usize::MAX);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].content, "Claude Code");
    // Gemini CLI's title only reports its state and folder, so its provider shows alone.
    for title in ["◇  Ready (shop-api)", "✦  Working… (shop-api)"] {
        let mut agent = row(23);
        agent.target.agent = "gemini".into();
        agent.label = "1".into();
        agent.terminal_title = observed(Some(title));
        let (spans, _) = default_line(agent, &Theme::terminal_native(), usize::MAX);
        assert_eq!(spans.len(), 1, "{title}");
        assert_eq!(spans[0].content, "Gemini CLI", "{title}");
    }
}

#[test]
fn default_agent_row_cuts_its_title_before_its_provider_and_keeps_the_title_when_narrow() {
    use orchbee_workbench_protocol::TerminalTitleObservation;
    let theme = Theme::rgb_test_dark();
    let mut ascii = Theme::rgb_test_dark();
    ascii.glyphs = crate::tui_theme::GlyphSet::ASCII;
    // (glyphs, title, width, first line, painted as)
    for (theme, title, width, expected) in [
        (
            theme,
            "Respond to greeting",
            27,
            "Respond to greeting · Codex",
        ),
        (theme, "Respond to greeting", 20, "Respond to … · Codex"),
        // The title keeps three cells before its ellipsis beside the provider ...
        (theme, "Respond to greeting", 12, "Res… · Codex"),
        // ... and narrower still tells siblings apart by its title alone, cut by the row later.
        (theme, "Respond to greeting", 11, "Respond to greeting"),
        // Wide characters are never split and the provider stays whole.
        (theme, "修复标题问题", 15, "修复标… · Codex"),
        (theme, "修复标题问题", 14, "修复… · Codex"),
        (ascii, "Respond to greeting", 13, "Res.. - Codex"),
    ] {
        let mut agent = row(23);
        agent.label = "1".into();
        agent.terminal_title = TerminalTitleObservation::Observed {
            revision: orchbee_workbench_protocol::WorkbenchRevision::new(1),
            title: Some(title.into()),
        };
        let (spans, whole) = default_line(agent, &theme, width);
        let actual: String = spans.iter().map(|span| span.content.as_ref()).collect();
        assert_eq!(actual, expected, "{title} in {width}");
        assert_eq!(
            whole,
            format!("{title} {} Codex", theme.glyphs.middle_dot),
            "the tooltip gives the whole row"
        );
        // The title uses primary text and the separator and provider quieter secondary text.
        let role = |span: &ratatui::text::Span<'_>| span.style.fg;
        assert_eq!(role(&spans[0]), theme.style(Role::TextPrimary).fg);
        if spans.len() > 1 {
            assert_eq!(spans.len(), 3);
            assert!(display_width(&actual) <= width, "{actual}");
            for span in &spans[1..] {
                assert_eq!(role(span), theme.style(Role::TextSecondary).fg);
            }
        }
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
        // Opening the narrow overlay gives it the keyboard; a pointer press leaves that navigation,
        // the state these paint checks draw, without the keyboard cursor and its hints.
        if !view.sidebar_screen() {
            view.machines.leave_sidebar_navigation();
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
) -> (Vec<String>, crate::tui_hit_map::FrameHitMap) {
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
            Theme::orchbee_dark(),
            Theme::orchbee_light(),
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
            assert_eq!(
                projected_title,
                format!("Input handling {} Codex", theme.glyphs.middle_dot)
            );
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
                if matches!(&selected.context, Some(crate::startup_directory::ClientStartupTarget::Agent(actual)) if actual.as_ref() == &target))
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
        assert!(rows[y as usize].contains("Codex"), "{rows:?}");
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
                // A filled action pads one cell on each side of its complete label.
                assert!(
                    footer.contains(&format!(" {} ", locale.settings_title())),
                    "{footer:?}"
                );
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

/// On the touch-spaced mobile list each entry owns the blank line under it, so a press there counts
/// for the entry above; below the spacing height entries keep one line each.
#[test]
fn touch_spaced_mobile_rows_own_the_blank_line_below_them() {
    for (height, spaced) in [(28, true), (23, false)] {
        let width = 46;
        let view = view(width, height, 12);
        let (rows, hits) = paint(&view, Locale::EnUs, &Theme::orchbee_dark(), width, height);
        let row_at = |y: u16| match hits.hit_target_at(4, y) {
            Some(HitTarget::MachineAgentRow { selected, .. }) => Some(selected.clone()),
            _ => None,
        };
        let starts: Vec<u16> = (0..height)
            .filter(|y| row_at(*y).is_some() && (*y == 0 || row_at(*y - 1) != row_at(*y)))
            .collect();
        assert!(starts.len() > 2, "{height}: {rows:?}");
        for y in starts {
            assert_eq!(
                row_at(y + 1) == row_at(y),
                spaced,
                "{height} at {y}: {rows:?}"
            );
            if spaced {
                assert!(
                    !rows[usize::from(y + 1)].contains("build"),
                    "{height} at {y}: {rows:?}"
                );
            }
        }
    }
}

#[test]
fn sidebar_input_ownership_never_moves_settings_or_adds_a_hint_row() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [Theme::terminal_native(), Theme::orchbee_light()] {
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
                                native: None,
                                update: None,
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
    let theme = Theme::orchbee_dark();
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
    let theme = Theme::orchbee_dark();
    let local_hint = crate::machine_picker::sidebar_binding_label(
        crate::machine_picker::SidebarCommand::Search,
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
                        native: None,
                        update: None,
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
                Theme::orchbee_light(),
                Theme::orchbee_dark(),
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
                            // The scrollbar appears exactly when the window clips entries, with
                            // its thumb painted in the column it publishes.
                            let scrollbar = (0..width).find_map(|x| {
                                (0..height).find_map(|y| match hits.hit_target_at(x, y) {
                                    Some(HitTarget::SidebarScrollbar { track, range, .. }) => {
                                        Some((*track, *range))
                                    }
                                    _ => None,
                                })
                            });
                            assert_eq!(
                                scrollbar.is_some(),
                                window.top > 0 || window.end() < 22,
                                "{width}x{height} {rows:?}"
                            );
                            // `paint` keeps each row's first 30 columns.
                            if let Some((track, range)) =
                                scrollbar.filter(|(track, _)| track.x < 30)
                            {
                                let thumb = range.thumb(track).unwrap();
                                assert!(
                                    rows[usize::from(thumb.y)]
                                        .contains(theme.glyphs.scrollbar_thumb),
                                    "{rows:?}"
                                );
                            }
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
                    assert!(!joined.contains("Codex ·"), "hidden sidebar drew: {rows:?}");
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
    let row_of = |text: &str| rows.iter().position(|row| row.contains(text)).unwrap();
    let search = row_of(" Search ");
    assert!(row_of(" New workspace ") < search, "{rows:?}");
    // The heading over ungrouped Workspaces names the group without a member count; its
    // disclosure always shows.
    let heading = Locale::EnUs.machine_picker_text("sidebar-recents");
    let recents = row_of(heading);
    assert_eq!(
        rows[recents].split_whitespace().collect::<Vec<_>>(),
        [heading, theme.glyphs.disclosure_open, "│"],
        "{rows:?}"
    );
    let workspace = recents
        + 1
        + rows[recents + 1..]
            .iter()
            .position(|row| row.contains("Workspace"))
            .unwrap();
    assert!(!rows[workspace].contains(theme.glyphs.ellipsis), "{rows:?}");
    let agent = row_of("build");
    assert!(rows[agent].contains(theme.glyphs.tree_guide), "{rows:?}");
    assert!(search < recents && recents < workspace && workspace < agent);
    assert!(rows[11].contains(" Settings "), "{rows:?}");
    let (search, agent) = (search as u16, agent as u16);
    assert!(
        (0..30).any(|x| matches!(hits.hit_target_at(x, search), Some(HitTarget::SearchButton)))
    );
    for x in [2, 27] {
        assert!(matches!(
            hits.hit_target_at(x, agent),
            Some(HitTarget::MachineAgentRow { .. })
        ));
    }
    assert!(matches!(
        hits.hit_target_at(28, agent),
        Some(HitTarget::OverlaySurface)
    ));
    assert!(
        hits.hit_target_at(30, agent).is_none(),
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
    let remote = crate::machines::Profile::new(
        crate::machines::ProfileId::parse(&format!("{:032x}", 1)).unwrap(),
        crate::machines::Draft::parse("remote", "example.invalid", "default").unwrap(),
    );
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
        matches!(&selected.context,Some(crate::startup_directory::ClientStartupTarget::Agent(t)) if t.member==row(24).target.member)
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
                if matches!(&selected.context,Some(crate::startup_directory::ClientStartupTarget::Agent(actual)) if actual.as_ref()==&target) =>
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
        matches!(&selected.context,Some(crate::startup_directory::ClientStartupTarget::Agent(t)) if t.as_ref()==&target)
    );
}

#[test]
fn compact_overlay_sidebar_rows_and_fixed_actions_paint_the_hover_band_under_the_pointer() {
    use crate::terminal_input::{
        MouseAction, MouseButton, MouseEvent, TerminalInputPort, TerminalInputUnavailable,
    };
    use crate::tui_pointer::{MotionObservability, PointerState};
    struct NoInput;
    impl TerminalInputPort for NoInput {
        fn forward(&mut self, _: &[u8]) -> Result<(), TerminalInputUnavailable> {
            panic!("overlay sidebar hover leaked terminal input")
        }
    }
    let (width, height) = (80, 22);
    for theme in [
        Theme::terminal_native(),
        Theme::orchbee_light(),
        Theme::rgb_test_dark(),
    ] {
        let paint = |view: &ClientView, pointer: &PointerState| {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            let mut hits = HitMapBuilder::with_pointer(pointer.projection());
            terminal
                .draw(|frame| {
                    render_terminal_shell(frame, view, Locale::EnUs, None, &theme, &mut hits);
                })
                .unwrap();
            (terminal.backend().buffer().clone(), hits.finish())
        };
        let band_cells =
            |buffer: &ratatui::buffer::Buffer, y: u16, hover: ratatui::style::Style| {
                (0..width)
                    .filter(|x| {
                        let cell = &buffer[(*x, y)];
                        if hover.add_modifier.contains(Modifier::REVERSED) {
                            cell.modifier.contains(Modifier::REVERSED)
                        } else {
                            Some(cell.bg) == hover.bg
                        }
                    })
                    .map(|x| buffer[(x, y)].symbol().to_owned())
                    .collect::<String>()
            };
        let banded = |buffer: &ratatui::buffer::Buffer, y: u16, hover: ratatui::style::Style| {
            display_width(&band_cells(buffer, y, hover))
        };
        for is_target in [
            (|target: Option<&HitTarget>| {
                matches!(target, Some(HitTarget::MachineWorkspaceRow { .. }))
            }) as fn(Option<&HitTarget>) -> bool,
            |target| matches!(target, Some(HitTarget::SearchButton)),
        ] {
            let mut view = view(width, height, 1);
            assert_eq!(
                view.overlay(),
                Some(crate::client_view::Overlay::BusinessSidebar)
            );
            let mut pointer = PointerState::default();
            pointer.set_capture(true);
            pointer.set_motion_observability(MotionObservability::AnyMotion);
            let (rest, map) = paint(&view, &pointer);
            let (x, y) = (0..height)
                .flat_map(|y| (0..width).map(move |x| (x, y)))
                .find(|(x, y)| is_target(map.hit_target_at(*x, *y)))
                .unwrap();
            let search = matches!(map.hit_target_at(x, y), Some(HitTarget::SearchButton));
            let row_hover = theme.style(Role::SurfaceHover);
            if !search {
                assert_eq!(banded(&rest, y, row_hover), 0, "{}", theme.name);
            }
            pointer.publish(map);
            let _ = pointer.event(
                &mut view,
                &mut NoInput,
                std::time::Instant::now(),
                MouseEvent {
                    column: x,
                    row: y,
                    action: MouseAction::Motion,
                    button: MouseButton::None,
                    mods: Modifiers::NONE,
                },
            );
            let (hovered, _) = paint(&view, &pointer);
            // A fixed action paints its own fill, which moves under the pointer in a color theme;
            // a band theme paints the same band at rest.
            let hover = if search {
                let fill = hovered[(x, y)].style();
                if !fill.add_modifier.contains(Modifier::REVERSED) {
                    assert_ne!(fill.bg, rest[(x, y)].style().bg, "{}", theme.name);
                }
                fill
            } else {
                row_hover
            };
            assert!(
                banded(&hovered, y, hover) >= 6,
                "{} {:?} at ({x}, {y})",
                theme.name,
                row_text(&rest, y)
            );
            // A fixed action's hover band is its padded label extent alone.
            if search {
                assert_eq!(band_cells(&hovered, y, hover), " Search ", "{}", theme.name);
            }
        }
    }
}

fn row_text(buffer: &ratatui::buffer::Buffer, y: u16) -> String {
    (0..buffer.area.width)
        .map(|x| buffer[(x, y)].symbol())
        .collect()
}

#[test]
fn sidebar_current_and_pending_rows_use_theme_roles_and_source_failure_stays_explicit() {
    for theme in [
        Theme::terminal_native(),
        Theme::rgb_test_dark(),
        Theme::orchbee_light(),
        Theme::orchbee_dark(),
        Theme::terminal_native().quantize(crate::tui_theme::ColorLevel::Monochrome),
    ] {
        let mut view = view(120, 40, 1);
        let agent = row(23);
        view.machines.observe_focus(
            Choice::Local,
            Some(agent.target.context.clone()),
            Some(crate::workbench_projection::ClientMemberTarget {
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
        // The fill alone marks an RGB current row, underlined only where no fill can show;
        // terminal-native also underlines every cell of its filled band, from the blank cell
        // before the state glyph to the row's trailing edge, as Monochrome does.
        let underlined = theme
            .underline_collapsed_current()
            .style(Role::SurfaceCurrent)
            .add_modifier
            .contains(Modifier::UNDERLINED);
        assert_eq!(
            underlined,
            theme.name == "terminal-native",
            "{}",
            theme.name
        );
        if theme.color(Role::SurfaceCurrent) != theme.color(Role::SurfaceChrome) {
            let band = (0..120)
                .filter(|x| {
                    matches!(
                        hits.hit_target_at(*x, y),
                        Some(HitTarget::MachineAgentRow { .. })
                    ) && terminal.backend().buffer()[(*x, y)].bg
                        == theme.color(Role::SurfaceCurrent)
                })
                .collect::<Vec<_>>();
            assert!(band.len() >= 20, "{} {band:?}", theme.name);
            for x in band {
                assert_eq!(
                    terminal.backend().buffer()[(x, y)]
                        .modifier
                        .contains(Modifier::UNDERLINED),
                    underlined,
                    "{} x={x}",
                    theme.name
                );
            }
        }
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
                        let transition =
                            covered.decide_action(ClientAction::OpenWorkbenchNavigator);
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
                Theme::orchbee_light(),
                Theme::orchbee_dark(),
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
