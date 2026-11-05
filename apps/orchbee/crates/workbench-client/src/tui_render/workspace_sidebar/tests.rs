use super::*;
use crate::machine_discovery::Context as NavigationContext;
use crate::machine_picker::Choice;
use crate::tui_hit_map::HitTarget;
use crate::tui_render::{project_shell_areas, render_terminal_shell};
use crate::workbench_projection::{ClientContextLocation, ClientContextTarget};
use orchbee_workbench_protocol::*;
use ratatui::{Terminal, backend::TestBackend, style::Modifier};
mod creation;
mod directory_change;
mod overflow;
mod states;
fn id(n: u64) -> String {
    format!("{n:032x}")
}
fn view(width: u16, height: u16) -> ClientView {
    let mut view = ClientView::new(width);
    view.machines.available = true;
    if width < 100 {
        for key in [
            crate::client_view::KeyEvent::press(
                crate::client_view::KeyCode::Char('b'),
                crate::client_view::Modifiers::CTRL,
            ),
            crate::client_view::KeyEvent::press(
                crate::client_view::KeyCode::Char('b'),
                crate::client_view::Modifiers::NONE,
            ),
        ] {
            let t = view.decide(crate::client_view::PrefixInput::Key(key));
            view.commit(
                t,
                crate::client_view::ClientActionDisposition::KeepNavigator,
            );
        }
    }
    view.machines.observe_agent_sidebar(
        project_shell_areas(Rect::new(0, 0, width, height), view.sidebar_surface())
            .sidebar
            .is_some(),
    );
    let session = SessionId::parse(&id(1)).unwrap();
    let rows: Vec<_> = (0..2)
        .map(|n| crate::machine_spaces::Row {
            terminal_directory: None,
            project_id: Some(WorkbenchProjectId::parse(&id(70)).unwrap()),
            target: ClientContextTarget {
                session_id: session.clone(),
                location: ClientContextLocation::Workspace {
                    workspace_id: WorkbenchWorkspaceId::parse(&id(80 + n)).unwrap(),
                    terminal_context_id: WorkbenchTerminalContextId::parse(&id(90 + n)).unwrap(),
                },
            },
            label: Some("same 工作区".into()),
            tokens: MetadataTokens::new(
                [(
                    MetadataTokenKey::parse("branch").unwrap(),
                    MetadataTokenValue::parse("custom branch").unwrap(),
                )]
                .into(),
            )
            .unwrap(),
            directory: Some(Directory {
                directory_id: WorkbenchDirectoryId::parse(&id(200)).unwrap(),
                worker_id: WorkerId::parse(&id(201)).unwrap(),
                path: WorkbenchDirectoryPath::parse("C:/repo").unwrap(),
            }),
            git_stale: false,
            held: None,
            git: Some(crate::machine_spaces::Git {
                branch: Some("feat/测试".into()),
                divergence: Some(GitDivergence {
                    ahead: 2,
                    behind: 3,
                }),
            }),
        })
        .collect();
    view.machines.observe_local(
        rows.iter()
            .map(|r| NavigationContext {
                target: r.target.clone(),
                project_id: r.project_id.clone(),
                project: Some("Repo".into()),
                workspace: r.label.clone(),
            })
            .collect(),
    );
    view.machines.observe_spaces(vec![(
        Choice::Local,
        crate::machine_spaces::checkpoint::Observation::Ready(std::sync::Arc::new(
            crate::machine_spaces::Checkpoint {
                session_id: session,
                revision: WorkbenchRevision::new(1),
                metadata_revision: WorkbenchRevision::new(2),
                git_requested: true,
                known_git: Vec::new().into(),
                projects: vec![crate::machine_spaces::ProjectSummary {
                    project_id: WorkbenchProjectId::parse(&id(70)).unwrap(),
                    label: "Repo".into(),
                }]
                .into(),
                rows: rows.into(),
            },
        )),
    )]);
    view.machines.observe_agents(vec![(
        Choice::Local,
        crate::machine_agents::Observation::Ready(crate::machine_agents::test_support::checkpoint(
            Vec::new(),
        )),
    )]);
    view
}
fn token(name: &str) -> serde_json::Value {
    serde_json::json!({"name":name,"style":{"fg":null,"bold":null,"dim":null},"rules":[]})
}
fn configure(view: &mut ClientView, rows: serde_json::Value, gap: u16) {
    view.machines.configure_space_sidebar(
        orchbee_local_runtime::SpaceSidebarSettings::from_profile(
            &serde_json::json!({"rows":rows,"row_gap":gap}).to_string(),
        )
        .unwrap(),
    );
}

fn agent_for(target: ClientContextTarget, number: u64) -> crate::machine_agents::Row {
    let mut agent = crate::machine_agents::test_support::row(number);
    agent.target.context = target;
    agent
}

fn expand_context_agents(view: &mut ClientView, target: ClientContextTarget) {
    let target = SidebarTarget::Context {
        machine: Choice::Local,
        target,
    };
    if !view.machines.sidebar_context_agents_expanded(&target) {
        assert!(view.machines.toggle_sidebar_context_agents(&target));
    }
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
    let b = terminal.backend().buffer();
    let rows = (0..height)
        .map(|y| {
            let mut text = String::new();
            let mut x = 0;
            while x < width.min(30) {
                let symbol = b[(x, y)].symbol();
                text.push_str(symbol);
                x += display_width(symbol).max(1) as u16;
            }
            text.trim_end().to_owned()
        })
        .collect();
    (rows, hits.finish())
}

fn hit_belongs_to_sidebar_target(hit: Option<&HitTarget>, target: &SidebarTarget) -> bool {
    match hit {
        Some(
            HitTarget::MachineGroupRow { target: actual, .. }
            | HitTarget::SidebarDisclosure(actual)
            | HitTarget::SidebarMenu(actual)
            | HitTarget::CreateMenu(Some(actual)),
        ) => actual == target,
        Some(
            HitTarget::MachineWorkspaceRow {
                selected: actual, ..
            }
            | HitTarget::MachineAgentRow {
                selected: actual, ..
            },
        ) => target.selection().as_ref() == Some(actual),
        _ => false,
    }
}

fn sidebar_target_rows(
    hits: &crate::tui_hit_map::FrameHitMap,
    width: u16,
    height: u16,
    target: &SidebarTarget,
) -> Vec<u16> {
    (0..height)
        .filter(|y| {
            (0..width).any(|x| hit_belongs_to_sidebar_target(hits.hit_target_at(x, *y), target))
        })
        .collect()
}
#[test]
fn composed_sidebar_has_no_outer_left_padding_and_keeps_nested_rows_and_hits_aligned() {
    for (width, height) in [(120, 40), (80, 24), (60, 12)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for theme in [
                Theme::terminal_native(),
                Theme::orchbee_light(),
                Theme::orchbee_dark(),
            ] {
                let mut view = view(width, height);
                configure(
                    &mut view,
                    serde_json::json!([
                        [token("state_icon"), token("workspace")],
                        [token("branch")]
                    ]),
                    0,
                );
                let active = view.machines.space_rows()[0].sidebar_target().unwrap();
                let SidebarTarget::Context { target, .. } = &active else {
                    unreachable!()
                };
                let mut agent = agent_for(target.clone(), 23);
                agent.metadata.title = Some(MetadataTokenValue::parse("Input handling").unwrap());
                let selected = agent.target.clone();
                view.machines.observe_agents(vec![(
                    Choice::Local,
                    crate::machine_agents::Observation::Ready(
                        crate::machine_agents::test_support::checkpoint(vec![agent]),
                    ),
                )]);
                let focused = view
                    .machines
                    .sidebar_navigation_rows()
                    .iter()
                    .find_map(|row| match row {
                        SidebarRow::Agent { .. } => Some(row.target()),
                        _ => None,
                    })
                    .unwrap();
                assert!(view.machines.focus_sidebar_target(&focused));
                let project = view.machines.sidebar_parent(&active).unwrap();
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                let mut hits = HitMapBuilder::new();
                terminal
                    .draw(|frame| {
                        render_terminal_shell(frame, &view, locale, None, &theme, &mut hits);
                    })
                    .unwrap();
                let buffer = terminal.backend().buffer();
                let hits = hits.finish();
                let sidebar_width =
                    project_shell_areas(Rect::new(0, 0, width, height), view.sidebar_surface())
                        .sidebar
                        .unwrap()
                        .area
                        .width;
                assert!(
                    (0..height).any(|y| {
                        hits.hit_target_at(0, y) == Some(&HitTarget::NewWorkspaceButton)
                    })
                );
                // An expanded Project at rest keeps its muted label and shows its disclosure.
                let project_y = (0..height).find(|y| {
                    matches!(
                        hits.hit_target_at(0, *y),
                        Some(HitTarget::MachineGroupRow { target, .. }) if target == &project
                    )
                });
                if let Some(project_y) = project_y {
                    assert!((0..sidebar_width).any(|x| matches!(
                        hits.hit_target_at(x, project_y),
                        Some(HitTarget::SidebarDisclosure(target)) if target == &project
                    )));
                    let project_tracks = layout::Tracks::resolve_for_width(
                        Rect::new(0, project_y, sidebar_width - 2, 1),
                        0,
                        sidebar_width,
                        auxiliary_actions(&view.machines.sidebar_source(), &project),
                        layout::RowKind::for_target(&project),
                    )
                    .unwrap();
                    assert_eq!(project_tracks.primary.x, 1);
                    assert_eq!(buffer[(project_tracks.primary.x, project_y)].symbol(), "R");
                    assert_eq!(
                        buffer[(project_tracks.primary.x, project_y)].fg,
                        theme.color(Role::TextMuted)
                    );
                    assert!(
                        !buffer[(project_tracks.primary.x, project_y)]
                            .modifier
                            .contains(ratatui::style::Modifier::BOLD)
                    );
                } else {
                    assert!(
                        height < 24,
                        "only the compact focused window may clip its parent"
                    );
                }
                let agent_y = (0..height)
                    .find(|y| {
                        matches!(
                            hits.hit_target_at(0, *y),
                            Some(HitTarget::MachineAgentRow { .. })
                        )
                    })
                    .unwrap();
                let agent_tracks = layout::Tracks::resolve_for_width(
                    Rect::new(0, agent_y, sidebar_width - 2, 1),
                    0,
                    sidebar_width,
                    auxiliary_actions(&view.machines.sidebar_source(), &focused),
                    layout::RowKind::for_target(&focused),
                )
                .unwrap();
                assert_eq!(
                    buffer[(agent_tracks.navigation.x, agent_y)].symbol(),
                    theme.glyphs.navigation_marker
                );
                assert_eq!(agent_tracks.guide.x, 1);
                assert_eq!(
                    buffer[(agent_tracks.guide.x, agent_y)].symbol(),
                    theme.glyphs.tree_guide
                );
                assert_eq!(
                    buffer[(agent_tracks.state.x, agent_y)].symbol(),
                    theme.agent_glyph(orchbee_workbench_protocol::MetadataState::Working)
                );
                assert_eq!(agent_tracks.state.x, 4);
                assert_eq!(buffer[(3, agent_y)].symbol(), " ");
                assert_eq!(buffer[(3, agent_y)].bg, theme.color(Role::SurfaceSelection));
                assert_eq!(buffer[(agent_tracks.primary.x, agent_y)].symbol(), "I");
                for x in 0..28 {
                    assert!(matches!(
                        hits.hit_target_at(x, agent_y),
                        Some(HitTarget::MachineAgentRow { selected: actual, .. })
                            if matches!(&actual.context, Some(crate::startup_directory::ClientStartupTarget::Agent(target)) if target.as_ref() == &selected)
                    ));
                }
                assert!(view.machines.request.is_none());
            }
        }
    }
}

#[test]
fn workspace_rows_render_real_branch_divergence_and_custom_names_with_exact_hits() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let mut ascii = Theme::terminal_native();
        ascii.glyphs = crate::tui_theme::GlyphSet::ASCII;
        for theme in [
            Theme::terminal_native(),
            Theme::rgb_test_dark(),
            Theme::orchbee_light(),
            Theme::orchbee_dark(),
            ascii,
        ] {
            let mut view = view(120, 40);
            configure(
                &mut view,
                serde_json::json!([
                    [token("state_icon"), token("workspace")],
                    [token("branch"), token("git_status")],
                    [token("$branch")]
                ]),
                0,
            );
            for row in view.machines.space_rows() {
                let (text_lines, _) =
                    lines(&view, row, locale, &theme, 80, LineShape::default()).into_parts();
                let projected = text_lines
                    .into_iter()
                    .map(|line| {
                        line.spans
                            .into_iter()
                            .map(|span| span.content.into_owned())
                            .collect::<String>()
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                assert!(projected.contains("feat/测试"), "{projected}");
                assert!(projected.contains("custom branch"), "{projected}");
            }
            let (rows, hits) = paint(&view, locale, &theme, 120, 40);
            let text = rows.join("\n");
            assert!(text.contains("custom branch"), "{text}");
            assert!(text.contains(&format!(
                "{}2 {}3",
                theme.glyphs.git_ahead, theme.glyphs.git_behind
            )));
            let targets = view
                .machines
                .space_rows()
                .iter()
                .map(|row| row.sidebar_target().unwrap())
                .collect::<Vec<_>>();
            assert_ne!(targets[0], targets[1]);
            for target in &targets {
                let exact = sidebar_target_rows(&hits, 30, 40, target);
                assert_eq!(exact.len(), 3, "{target:?}: {text}");
                assert!(
                    rows[usize::from(exact[0])].starts_with(" same"),
                    "a configured state icon paints nothing before a Workspace name: {text}"
                );
                assert!(exact.windows(2).all(|pair| pair[1] == pair[0] + 1));
                let painted_x = (0..30)
                    .filter(|x| {
                        hit_belongs_to_sidebar_target(hits.hit_target_at(*x, exact[0]), target)
                    })
                    .collect::<Vec<_>>();
                let band = Rect::new(
                    painted_x[0],
                    exact[0],
                    painted_x[painted_x.len() - 1] - painted_x[0] + 1,
                    exact.len() as u16,
                );
                let tracks = layout::Tracks::resolve_for_width(
                    band,
                    0,
                    30,
                    auxiliary_actions(&view.machines.sidebar_source(), target),
                    layout::RowKind::for_target(target),
                )
                .unwrap();
                let expected = truncate_display(
                    "custom branch",
                    usize::from(tracks.primary.width),
                    theme.glyphs.ellipsis,
                );
                assert!(
                    exact
                        .iter()
                        .any(|y| rows[usize::from(*y)].contains(&expected)),
                    "{target:?}: {text}"
                );
            }
            assert!((0..40).any(|y| matches!(
                hits.hit_target_at(2, y),
                Some(HitTarget::NewWorkspaceButton)
            )));
            assert!((0..40).any(|y| {
                (0..30).any(|x| matches!(hits.hit_target_at(x, y), Some(HitTarget::SearchButton)))
            }));
        }
    }
}

#[test]
fn fixed_actions_keep_complete_labels_and_exact_button_hits_above_the_first_group() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let mut view = view(60, 12);
        let (rows, hits) = paint(&view, locale, &Theme::terminal_native(), 60, 12);
        let action_row = |target: &HitTarget| {
            (0..12)
                .find(|y| (0..29).any(|x| hits.hit_target_at(x, *y) == Some(target)))
                .unwrap()
        };
        // No list heading: the first Project begins directly below the fixed actions.
        let first_group = action_row(&HitTarget::SearchButton) + 1;
        let header = &rows[usize::from(first_group)];
        assert!(header.trim_start().starts_with("Repo"), "{header:?}");
        assert!(!header.contains(locale.shell().workspaces), "{header:?}");

        for (command, target) in [
            (
                crate::machine_picker::SidebarCommand::NewWorkspace,
                HitTarget::NewWorkspaceButton,
            ),
            (
                crate::machine_picker::SidebarCommand::Search,
                HitTarget::SearchButton,
            ),
        ] {
            let y = action_row(&target);
            let row = &rows[usize::from(y)];
            assert!(row.contains("[ "), "{row:?}");
            assert!(row.contains(locale.sidebar_navigation_action(command)));
            assert!(row.contains(" ]"), "{row:?}");
            let exact = (0..29)
                .filter(|x| hits.hit_target_at(*x, y) == Some(&target))
                .collect::<Vec<_>>();
            assert!(!exact.is_empty());
            assert_eq!(
                exact,
                (*exact.first().unwrap()..=*exact.last().unwrap()).collect::<Vec<_>>()
            );
            assert!(exact.len() < 29, "the non-button remainder stays a surface");
            assert_eq!(
                hits.hit_target_at(*exact.last().unwrap() + 1, y),
                Some(&HitTarget::OverlaySurface)
            );
            assert!(matches!(
                hits.hit_target_at(0, first_group),
                Some(HitTarget::MachineGroupRow { .. })
            ));
            let local_hint = crate::machine_picker::sidebar_binding_label(
                command,
                crate::client_view::KeyLabelGlyphs::Unicode,
            )
            .unwrap();
            assert!(!row.contains(&local_hint));
        }
        assert!(view.machines.enter_sidebar_navigation());
        let (focused_rows, _) = paint(&view, locale, &Theme::terminal_native(), 60, 12);
        let search = usize::from(action_row(&HitTarget::SearchButton));
        let hint = crate::machine_picker::sidebar_binding_label(
            crate::machine_picker::SidebarCommand::Search,
            crate::client_view::KeyLabelGlyphs::Unicode,
        )
        .unwrap();
        assert!(
            focused_rows[search].contains(&hint),
            "{}",
            focused_rows[search]
        );
    }
}

#[test]
fn workspace_raw_value_rules_and_empty_layouts_collapse_without_borrowing_custom_git() {
    let mut view = view(120, 40);
    let mut branch = token("branch");
    branch["rules"] = serde_json::json!([{"condition":{"kind":"Equals","value":"feat/测试"},"ignore_case":false,"style":{"fg":null,"bold":null,"dim":null},"hide":true}]);
    configure(
        &mut view,
        serde_json::json!([[branch], [token("$branch")]]),
        0,
    );
    let (rows, hits) = paint(&view, Locale::EnUs, &Theme::terminal_native(), 120, 40);
    assert!(!rows.join("\n").contains("feat/测试"));
    for target in view
        .machines
        .space_rows()
        .iter()
        .map(|row| row.sidebar_target().unwrap())
    {
        assert_eq!(sidebar_target_rows(&hits, 30, 40, &target).len(), 2);
    }
    configure(&mut view, serde_json::json!([]), 0);
    let (rows, hits) = paint(&view, Locale::EnUs, &Theme::terminal_native(), 120, 40);
    assert!(!rows.join("\n").contains("custom branch"));
    for target in view
        .machines
        .space_rows()
        .iter()
        .map(|row| row.sidebar_target().unwrap())
    {
        assert_eq!(sidebar_target_rows(&hits, 30, 40, &target).len(), 1);
    }
}
#[test]
fn workspace_sidebar_degrades_across_tiers_and_preserves_action_entries() {
    for (width, height) in [(120, 40), (80, 24), (60, 12), (50, 10), (15, 4)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let mut ascii = Theme::terminal_native();
            ascii.glyphs = crate::tui_theme::GlyphSet::ASCII;
            for theme in [
                Theme::terminal_native(),
                Theme::rgb_test_dark(),
                Theme::orchbee_light(),
                Theme::orchbee_dark(),
                ascii,
            ] {
                let view = view(width, height);
                let (_, hits) = paint(&view, locale, &theme, width, height);
                let visible =
                    project_shell_areas(Rect::new(0, 0, width, height), view.sidebar_surface())
                        .sidebar
                        .is_some();
                assert_eq!(
                    (0..height).any(|y| (0..width.min(30)).any(|x| matches!(
                        hits.hit_target_at(x, y),
                        Some(HitTarget::NewWorkspaceButton)
                    ))),
                    visible,
                    "{width}x{height}"
                );
                if !visible {
                    assert!((0..height).all(|y| (0..width).all(|x| !matches!(
                        hits.hit_target_at(x, y),
                        Some(HitTarget::MachineWorkspaceRow { .. })
                    ))));
                }
            }
        }
    }
}

#[test]
fn compact_sidebar_keeps_current_workspace_and_agent_unclipped() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let mut view = view(60, 14);
        configure(
            &mut view,
            serde_json::json!([[token("workspace")], [token("branch")], [token("$branch")]]),
            0,
        );
        let selected = view.machines.space_rows()[1].selection();
        let SidebarTarget::Context { target, .. } =
            view.machines.space_rows()[1].sidebar_target().unwrap()
        else {
            unreachable!()
        };
        let agent = agent_for(target.clone(), 23);
        let member = crate::workbench_projection::ClientMemberTarget {
            tab: agent.target.tab.clone(),
            panel: agent.target.panel.clone(),
            presentation: agent.target.member.clone(),
        };
        view.machines.observe_agents(vec![(
            Choice::Local,
            crate::machine_agents::Observation::Ready(
                crate::machine_agents::test_support::checkpoint(vec![agent]),
            ),
        )]);
        view.machines
            .observe_focus(Choice::Local, Some(target.clone()), Some(member));
        expand_context_agents(&mut view, target);
        let (_, hits) = paint(&view, locale, &Theme::terminal_native(), 60, 14);
        assert_eq!((0..14).filter(|y| matches!(hits.hit_target_at(4, *y), Some(HitTarget::MachineWorkspaceRow { selected: actual, .. }) if actual == &selected)).count(), 3);
        assert_eq!(
            (0..14)
                .filter(|y| matches!(
                    hits.hit_target_at(2, *y),
                    Some(HitTarget::MachineAgentRow { .. })
                ))
                .count(),
            1
        );
    }
}
#[test]
fn workspace_separators_stay_outside_entries_and_style_patches_preserve_count_roles() {
    let mut view = view(120, 40);
    let mut git = token("git_status");
    git["style"] = serde_json::json!({"fg":[16,32,48],"bold":true,"dim":false});
    configure(
        &mut view,
        serde_json::json!([[token("workspace")], [token("branch")], [git]]),
        1,
    );
    let active = view.machines.space_rows()[0].sidebar_target().unwrap();
    let focused = view.machines.space_rows()[1].sidebar_target().unwrap();
    let SidebarTarget::Context { target, .. } = &active else {
        unreachable!()
    };
    view.machines
        .observe_focus(Choice::Local, Some(target.clone()), None);
    assert!(view.machines.focus_sidebar_target(&focused));
    let theme = Theme::rgb_test_dark();
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    let mut builder = HitMapBuilder::new();
    terminal
        .draw(|frame| {
            render_terminal_shell(frame, &view, Locale::EnUs, None, &theme, &mut builder);
        })
        .unwrap();
    let map = builder.finish();
    let buffer = terminal.backend().buffer();
    let y = (0..40)
        .find(|y| (2..28).any(|x| buffer[(x, *y)].symbol() == "↑"))
        .unwrap();
    let x = (2..28).find(|x| buffer[(*x, y)].symbol() == "↑").unwrap();
    assert_eq!(buffer[(x, y)].fg, ratatui::style::Color::Rgb(16, 32, 48));
    assert!(buffer[(x, y)].modifier.contains(Modifier::BOLD));
    assert!(!buffer[(x, y)].modifier.contains(Modifier::DIM));
    for (index, surface) in [(0, Role::SurfaceCurrent), (1, Role::SurfaceSelection)] {
        let selection = view.machines.space_rows()[index].selection();
        let entry_rows: Vec<_> = (0..40)
            .filter(|y| {
                matches!(
                    map.hit_target_at(0, *y),
                    Some(HitTarget::MachineWorkspaceRow { selected, .. }) if selected == &selection
                )
            })
            .collect();
        // Separators between Workspaces are inert list surface: every entry covers exactly its
        // own three configured lines, whether or not a gap precedes it.
        let expected = 3;
        assert_eq!(entry_rows.len(), expected);
        assert_eq!(
            entry_rows[expected - 1],
            entry_rows[0] + expected as u16 - 1
        );
        for y in entry_rows {
            let mut x = 0;
            while x < 28 {
                let cell = &buffer[(x, y)];
                assert_eq!(cell.bg, theme.color(surface), "entry {index} at {x},{y}");
                x += u16::try_from(display_width(cell.symbol()).max(1)).unwrap();
            }
        }
    }
    for y in 0..40 {
        if matches!(
            map.hit_target_at(0, y),
            Some(HitTarget::MachineWorkspaceSurface(_) | HitTarget::MachineGroupRow { .. })
        ) {
            assert_eq!(buffer[(0, y)].bg, theme.color(Role::SurfaceChrome));
        }
    }
    assert!((0..40).any(|y| matches!(
        map.hit_target_at(2, y),
        Some(HitTarget::MachineWorkspaceSurface(_))
    )));
}

#[test]
fn long_branch_and_custom_text_shrink_before_fixed_divergence_counts() {
    let mut view = view(120, 40);
    let rows = view
        .machines
        .space_rows()
        .iter()
        .map(|r| {
            let mut observed = view.machines.space_checkpoint_row(r).unwrap().clone();
            observed.git.as_mut().unwrap().branch =
                Some(format!("worktree/{}", "测试🙂".repeat(20)));
            observed
        })
        .collect::<Vec<_>>();
    view.machines.observe_spaces(vec![(
        Choice::Local,
        crate::machine_spaces::checkpoint::Observation::Ready(std::sync::Arc::new(
            crate::machine_spaces::Checkpoint {
                session_id: SessionId::parse(&id(1)).unwrap(),
                revision: WorkbenchRevision::new(1),
                metadata_revision: WorkbenchRevision::new(2),
                git_requested: true,
                known_git: Vec::new().into(),
                projects: vec![crate::machine_spaces::ProjectSummary {
                    project_id: WorkbenchProjectId::parse(&id(70)).unwrap(),
                    label: "Repo".into(),
                }]
                .into(),
                rows: rows.into(),
            },
        )),
    )]);
    configure(
        &mut view,
        serde_json::json!([
            [token("workspace")],
            [token("branch"), token("$branch"), token("git_status")]
        ]),
        0,
    );
    let (rows, _) = paint(&view, Locale::EnUs, &Theme::rgb_test_dark(), 120, 40);
    assert_eq!(
        rows.iter().filter(|r| r.contains("↑2 ↓3")).count(),
        2,
        "{rows:?}"
    );
    assert!(rows.iter().any(|r| r.contains('…')));
}

#[test]
fn folder_location_names_only_its_folder_and_gives_the_whole_directory_under_the_pointer() {
    use crate::tui_hit_map::tooltip::Key;
    let mut view = view(120, 40);
    // Longer than the 80-scalar label cap and the tooltip's two lines, so the tooltip keeps both
    // ends of the very directory the row stands for.
    let path = "C:/Users/someone/AppData/Local/Temp/orchbee-review-long-directory-0123456789/nested-checkout-folder/checkout";
    let rows = view
        .machines
        .space_rows()
        .iter()
        .map(|r| {
            let mut observed = view.machines.space_checkpoint_row(r).unwrap().clone();
            observed.git = None;
            observed.directory.as_mut().unwrap().path =
                WorkbenchDirectoryPath::parse(path).unwrap();
            observed
        })
        .collect::<Vec<_>>();
    let target = rows[0].target.clone();
    view.machines.observe_spaces(vec![(
        Choice::Local,
        crate::machine_spaces::checkpoint::Observation::Ready(std::sync::Arc::new(
            crate::machine_spaces::Checkpoint {
                session_id: SessionId::parse(&id(1)).unwrap(),
                revision: WorkbenchRevision::new(1),
                metadata_revision: WorkbenchRevision::new(2),
                git_requested: true,
                known_git: Vec::new().into(),
                projects: vec![crate::machine_spaces::ProjectSummary {
                    project_id: WorkbenchProjectId::parse(&id(70)).unwrap(),
                    label: "Repo".into(),
                }]
                .into(),
                rows: rows.into(),
            },
        )),
    )]);
    view.machines.observe_agents(vec![(
        Choice::Local,
        crate::machine_agents::Observation::Ready(crate::machine_agents::test_support::checkpoint(
            vec![agent_for(target, 23)],
        )),
    )]);
    let (rows, _) = paint(&view, Locale::EnUs, &Theme::rgb_test_dark(), 120, 40);
    // The Workspace with an Agent and its sibling without one name the same folder, and no line
    // shows any other part of its directory.
    let locations = rows
        .iter()
        .filter(|row| row.contains("checkout"))
        .collect::<Vec<_>>();
    assert_eq!(locations.len(), 2, "{rows:?}");
    for location in &locations {
        assert_eq!(location.split('│').next().unwrap().trim(), "checkout");
    }
    assert!(!rows.iter().any(|row| row.contains("C:/")), "{rows:?}");
    // Only the folder's own cells give the whole directory, to be cut in the middle if long.
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    let mut hits = HitMapBuilder::new();
    terminal
        .draw(|frame| {
            let theme = Theme::rgb_test_dark();
            render_terminal_shell(frame, &view, Locale::EnUs, None, &theme, &mut hits);
        })
        .unwrap();
    let paths = hits
        .tooltip_contents()
        .filter(|content| {
            matches!(content, TooltipContent::Describe { text: Some(text), legend }
                if text.text == path && text.middle && legend.is_empty())
        })
        .count();
    assert_eq!(paths, 2);
    let map = hits.finish();
    let buffer = terminal.backend().buffer();
    let y = (0..40)
        .find(|&y| {
            (0..30)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .contains("checkout")
        })
        .unwrap();
    let offered: Vec<_> = (0..30)
        .filter(|&x| matches!(map.tooltip_at(x, y), Some(Key::Part { .. })))
        .map(|x| buffer[(x, y)].symbol())
        .collect();
    assert_eq!(offered.concat(), "checkout");

    // A home folder reads `~` under Windows spelling; a root keeps its own spelling.
    for (directory, home, name) in [
        ("C:\\Users\\Someone", Some("c:/users/someone/"), "~"),
        (
            "C:\\Users\\Someone\\Desktop\\",
            Some("C:\\Users\\Someone"),
            "Desktop",
        ),
        ("/home/someone/code/orchbee", None, "orchbee"),
        ("C:\\", Some("C:\\Users\\Someone"), "C:\\"),
        ("/", None, "/"),
    ] {
        assert_eq!(folder_name(directory, home), name, "{directory}");
    }
}

#[test]
fn cut_names_give_their_whole_text_only_when_cut_and_each_cut_line_its_own() {
    use crate::tui_layout::SidebarWidth;
    let project = "Payments platform service repository";
    let workspace = "Checkout redesign experiment";
    let branch = "feature/checkout-redesign-experiment";
    let title = "Respond to greeting | Desktop";
    let mut view = view(140, 40);
    let rows = view
        .machines
        .space_rows()
        .iter()
        .map(|r| {
            let mut observed = view.machines.space_checkpoint_row(r).unwrap().clone();
            observed.label = Some(workspace.into());
            observed.git.as_mut().unwrap().branch = Some(branch.into());
            observed
        })
        .collect::<Vec<_>>();
    view.machines.observe_local(
        rows.iter()
            .map(|r| NavigationContext {
                target: r.target.clone(),
                project_id: r.project_id.clone(),
                project: Some(project.into()),
                workspace: r.label.clone(),
            })
            .collect(),
    );
    let target = rows[0].target.clone();
    view.machines.observe_spaces(vec![(
        Choice::Local,
        crate::machine_spaces::checkpoint::Observation::Ready(std::sync::Arc::new(
            crate::machine_spaces::Checkpoint {
                session_id: SessionId::parse(&id(1)).unwrap(),
                revision: WorkbenchRevision::new(1),
                metadata_revision: WorkbenchRevision::new(2),
                git_requested: true,
                known_git: Vec::new().into(),
                projects: vec![crate::machine_spaces::ProjectSummary {
                    project_id: WorkbenchProjectId::parse(&id(70)).unwrap(),
                    label: project.into(),
                }]
                .into(),
                rows: rows.into(),
            },
        )),
    )]);
    let mut agent = agent_for(target.clone(), 23);
    agent.label = "1".into();
    agent.terminal_title = TerminalTitleObservation::Observed {
        revision: WorkbenchRevision::new(1),
        title: Some(title.into()),
    };
    view.machines.observe_agents(vec![(
        Choice::Local,
        crate::machine_agents::Observation::Ready(crate::machine_agents::test_support::checkpoint(
            vec![agent],
        )),
    )]);
    expand_context_agents(&mut view, target);
    let agent_row = format!("{title} · codex");
    let mut width = SidebarWidth::DEFAULT;
    for (columns, cut) in [(60, false), (30, true)] {
        let next = SidebarWidth::new(columns).unwrap();
        assert!(view.machines.set_sidebar_width(width, next));
        width = next;
        let mut terminal = Terminal::new(TestBackend::new(140, 40)).unwrap();
        let mut hits = HitMapBuilder::new();
        terminal
            .draw(|frame| {
                let theme = Theme::rgb_test_dark();
                render_terminal_shell(frame, &view, Locale::EnUs, None, &theme, &mut hits);
            })
            .unwrap();
        let named: Vec<_> = hits
            .tooltip_contents()
            .filter_map(|content| match content {
                TooltipContent::Describe {
                    text: Some(text), ..
                } => Some(text.text.as_str()),
                TooltipContent::Describe { text: None, .. } | TooltipContent::Action { .. } => None,
            })
            .collect();
        let buffer = terminal.backend().buffer();
        let painted = (0..40)
            .map(|y| {
                (0..columns)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>();
        if !cut {
            // Whole names paint whole and offer nothing to read.
            for name in [project, workspace, branch, agent_row.as_str()] {
                assert!(painted.iter().any(|row| row.contains(name)), "{painted:?}");
            }
            assert!(named.is_empty(), "{named:?}");
            continue;
        }
        for name in [project, workspace, branch, agent_row.as_str()] {
            assert!(!painted.iter().any(|row| row.contains(name)), "{painted:?}");
            assert!(named.contains(&name), "{name}: {named:?}");
        }
        // The Workspace's first line gives its name and its second line the branch it cuts.
        let y = painted
            .iter()
            .position(|row| row.trim_start().starts_with("Checkout"))
            .unwrap() as u16;
        let read = |y| match hits.tooltip_content_at(4, y) {
            Some(TooltipContent::Describe {
                text: Some(text), ..
            }) => Some(text.text.clone()),
            _ => None,
        };
        assert_eq!(read(y).as_deref(), Some(workspace));
        assert_eq!(read(y + 1).as_deref(), Some(branch));
    }
}

fn paint_navigation_buffers(
    view: &ClientView,
    locale: Locale,
    theme: &Theme,
    width: u16,
    height: u16,
) -> (
    ratatui::buffer::Buffer,
    ratatui::buffer::Buffer,
    crate::tui_hit_map::FrameHitMap,
) {
    paint_navigation_buffers_with_pointer(view, locale, theme, width, height, Default::default())
}

fn paint_navigation_buffers_with_pointer(
    view: &ClientView,
    locale: Locale,
    theme: &Theme,
    width: u16,
    height: u16,
    pointer: crate::tui_hit_map::PointerProjection,
) -> (
    ratatui::buffer::Buffer,
    ratatui::buffer::Buffer,
    crate::tui_hit_map::FrameHitMap,
) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut hits = HitMapBuilder::with_pointer(pointer);
    let mut candidate = None;
    let rows = view.machines.sidebar_navigation_rows();
    let source = view.machines.sidebar_source();
    terminal
        .draw(|frame| {
            render(
                frame,
                Rect::new(0, 0, width, height),
                RenderContext {
                    held: None,
                    creation: None,
                    view,
                    locale,
                    theme,
                    layer: HitLayer::Chrome,
                    sidebar_width: width,
                    trailing_padding: 0,
                    owns_keyboard: view.machines.sidebar_navigation_active(),
                    source: &source,
                },
                &rows,
                Some(&mut hits),
            );
            candidate = Some(frame.buffer_mut().clone());
        })
        .unwrap();
    (
        candidate.expect("Workspace Sidebar candidate frame"),
        terminal.backend().buffer().clone(),
        hits.finish(),
    )
}

fn paint_navigation(
    view: &ClientView,
    locale: Locale,
    theme: &Theme,
    width: u16,
    height: u16,
) -> (ratatui::buffer::Buffer, crate::tui_hit_map::FrameHitMap) {
    let (_, sink, hits) = paint_navigation_buffers(view, locale, theme, width, height);
    (sink, hits)
}

fn hidden_by_candidate_wide_glyph(candidate: &ratatui::buffer::Buffer, x: u16, y: u16) -> bool {
    (0..x).rev().take(4).any(|start| {
        let width = display_width(candidate[(start, y)].symbol());
        width > 1 && usize::from(start) + width > usize::from(x)
    })
}

#[test]
fn sidebar_context_menus_have_no_overflow_trigger_and_revealed_create_stays_right_aligned() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for width in [26, 40] {
            let mut view = view(120, 40);
            configure(
                &mut view,
                serde_json::json!([[token("workspace")], [token("branch")]]),
                0,
            );
            let active = view.machines.space_rows()[0].sidebar_target().unwrap();
            let other = view.machines.space_rows()[1].sidebar_target().unwrap();
            let SidebarTarget::Context { target, .. } = &active else {
                unreachable!()
            };
            view.machines
                .observe_focus(Choice::Local, Some(target.clone()), None);
            let project = view.machines.sidebar_parent(&active).unwrap();
            let theme = Theme::orchbee_dark();
            let (ordinary_buffer, ordinary) = paint_navigation(&view, locale, &theme, width, 10);
            for expected in [&project, &active, &other] {
                let rows = sidebar_target_rows(&ordinary, width, 10, expected);
                assert!(!rows.is_empty(), "{expected:?}");
                let menu_y = rows[0];
                assert!((0..width).all(|x| !matches!(
                    ordinary.hit_target_at(x, menu_y),
                    Some(HitTarget::SidebarMenu(target)) if target == expected
                )));
                for y in rows.into_iter().skip(1) {
                    assert!((0..width).all(|x| !matches!(
                        ordinary.hit_target_at(x, y),
                        Some(HitTarget::SidebarMenu(_))
                    )));
                }
            }
            let project_rows = sidebar_target_rows(&ordinary, width, 10, &project);
            let project_y = project_rows[0];
            let project_tracks = layout::Tracks::resolve(
                Rect::new(0, project_y, width, project_rows.len() as u16),
                0,
                auxiliary_actions(&view.machines.sidebar_source(), &project),
                layout::RowKind::for_target(&project),
            )
            .unwrap();
            let create = project_tracks
                .create
                .expect("Project create remains visible at every admitted width");
            assert_eq!(create.right(), width);
            let create_hit = HitTarget::CreateMenu(Some(project.clone()));
            // Create is revealed by pointer presence or keyboard focus, never published at rest.
            assert_ne!(ordinary_buffer[(create.x, project_y)].symbol(), "[");
            assert!(
                (create.x..create.right())
                    .all(|x| ordinary.hit_target_at(x, project_y) != Some(&create_hit))
            );
            assert!(view.machines.focus_sidebar_target(&project));
            let (revealed_buffer, revealed) = paint_navigation(&view, locale, &theme, width, 10);
            assert_eq!(revealed_buffer[(create.x, project_y)].symbol(), "[");
            assert!(
                (create.x..create.right())
                    .all(|x| revealed.hit_target_at(x, project_y) == Some(&create_hit))
            );

            assert!(view.machines.focus_sidebar_target(&other));
            let (focused_buffer, focused_hits) = paint_navigation(&view, locale, &theme, width, 10);
            let other_rows = sidebar_target_rows(&focused_hits, width, 10, &other);
            assert!((0..width).all(|x| !matches!(
                focused_hits.hit_target_at(x, other_rows[0]),
                Some(HitTarget::SidebarMenu(_))
            )));
            // A Workspace row's keyboard marker shares the Project heading's edge column.
            assert_eq!(
                focused_buffer[(0, other_rows[0])].symbol(),
                theme.glyphs.navigation_marker
            );
            let active_rows = sidebar_target_rows(&focused_hits, width, 10, &active);
            assert!((0..width).all(|x| !matches!(
                focused_hits.hit_target_at(x, active_rows[0]),
                Some(HitTarget::SidebarMenu(actual)) if actual == &active
            )));
            assert_eq!(view.machines.current_context.as_ref(), Some(target));

            view.machines.leave_sidebar_navigation();
            let (restored_buffer, restored_hits) =
                paint_navigation(&view, locale, &theme, width, 10);
            let restored_other_rows = sidebar_target_rows(&restored_hits, width, 10, &other);
            assert_eq!(restored_buffer[(0, restored_other_rows[0])].symbol(), " ");
            assert!((0..width).all(|x| !matches!(
                restored_hits.hit_target_at(x, restored_other_rows[0]),
                Some(HitTarget::SidebarMenu(actual)) if actual == &other
            )));
            assert!(view.machines.request.is_none());
        }
    }
}

#[test]
fn project_context_and_auxiliary_tracks_cover_every_multirow_cell_without_overlap() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [
            Theme::terminal_native(),
            Theme::rgb_test_dark(),
            Theme::orchbee_light(),
            Theme::orchbee_dark(),
        ] {
            for width in [26, 40] {
                let mut view = view(120, 40);
                configure(
                    &mut view,
                    serde_json::json!([[token("workspace")], [token("branch")]]),
                    0,
                );
                let active = view.machines.space_rows()[0].sidebar_target().unwrap();
                let focused = view.machines.space_rows()[1].sidebar_target().unwrap();
                let SidebarTarget::Context { target, .. } = &active else {
                    unreachable!()
                };
                view.machines
                    .observe_focus(Choice::Local, Some(target.clone()), None);
                let project = view.machines.sidebar_parent(&active).unwrap();
                // Keyboard focus reveals the Project's disclosure and Create controls.
                assert!(view.machines.focus_sidebar_target(&project));
                let (_, _, hits) = paint_navigation_buffers(&view, locale, &theme, width, 10);
                let project_rows = sidebar_target_rows(&hits, width, 10, &project);
                let project_tracks = layout::Tracks::resolve(
                    Rect::new(0, project_rows[0], width, project_rows.len() as u16),
                    0,
                    auxiliary_actions(&view.machines.sidebar_source(), &project),
                    layout::RowKind::for_target(&project),
                )
                .unwrap()
                .with_disclosure_after(display_width("Repo"));
                for y in &project_rows {
                    for x in 0..width {
                        let hit = hits.hit_target_at(x, *y);
                        if project_tracks.disclosure.contains((x, *y).into()) {
                            assert_eq!(hit, Some(&HitTarget::SidebarDisclosure(project.clone())));
                        } else if *y == project_tracks.content_y
                            && project_tracks
                                .create
                                .is_some_and(|rect| x >= rect.x && x < rect.right())
                        {
                            assert_eq!(hit, Some(&HitTarget::CreateMenu(Some(project.clone()))));
                        } else {
                            assert!(matches!(
                                hit,
                                Some(HitTarget::MachineGroupRow { target, .. })
                                    if target == &project
                            ));
                        }
                    }
                }

                assert!(view.machines.focus_sidebar_target(&focused));
                let (candidate, sink, hits) =
                    paint_navigation_buffers(&view, locale, &theme, width, 10);
                // Unfocused, the Project keeps only its disclosure; Create is hidden again.
                for y in sidebar_target_rows(&hits, width, 10, &project) {
                    for x in 0..width {
                        let hit = hits.hit_target_at(x, y);
                        if project_tracks.disclosure.contains((x, y).into()) {
                            assert_eq!(hit, Some(&HitTarget::SidebarDisclosure(project.clone())));
                        } else {
                            assert!(matches!(
                                hit,
                                Some(HitTarget::MachineGroupRow { target, .. }) if target == &project
                            ));
                        }
                    }
                }
                let focused_rows = sidebar_target_rows(&hits, width, 10, &focused);
                assert_eq!(focused_rows.len(), 2);
                for y in &focused_rows {
                    for x in 0..width {
                        assert!(matches!(
                            hits.hit_target_at(x, *y),
                            Some(HitTarget::MachineWorkspaceRow { selected, .. })
                                if selected == &view.machines.space_rows()[1].selection()
                        ));
                    }

                    for x in 0..width {
                        assert_eq!(candidate[(x, *y)].bg, theme.color(Role::SurfaceSelection));
                    }
                }
                assert_eq!(
                    candidate[(0, focused_rows[0])].symbol(),
                    theme.glyphs.navigation_marker
                );

                let active_rows = sidebar_target_rows(&hits, width, 10, &active);
                for y in &active_rows {
                    for x in 0..width {
                        assert_eq!(candidate[(x, *y)].bg, theme.color(Role::SurfaceCurrent));
                    }
                }
                let mut hidden_differences = Vec::new();
                for y in 0..10 {
                    for x in 0..width {
                        let candidate_cell = &candidate[(x, y)];
                        let sink_cell = &sink[(x, y)];
                        if candidate_cell != sink_cell {
                            assert!(
                                hidden_by_candidate_wide_glyph(&candidate, x, y),
                                "TestBackend changed visible Sidebar cell ({x}, {y}): candidate={candidate_cell:?} sink={sink_cell:?}"
                            );
                            assert_eq!(candidate_cell.symbol(), " ");
                            hidden_differences.push((x, y));
                        }
                    }
                }
                assert!(
                    !hidden_differences.is_empty(),
                    "the locked TestBackend must expose its wide-glyph continuation limitation"
                );
                assert_eq!(view.machines.current_context.as_ref(), Some(target));

                assert!(view.machines.toggle_sidebar_group(&project));
                let (collapsed_buffer, collapsed) =
                    paint_navigation(&view, locale, &theme, width, 10);
                let collapsed_project_rows = sidebar_target_rows(&collapsed, width, 10, &project);
                assert_eq!(
                    collapsed_buffer[(0, collapsed_project_rows[0])].bg,
                    theme.color(Role::SurfaceSelection)
                );
                assert!((0..10).all(|y| (0..width).all(|x| !matches!(
                    collapsed.hit_target_at(x, y),
                    Some(HitTarget::MachineWorkspaceRow { .. })
                ))));
                assert_eq!(view.machines.current_context.as_ref(), Some(target));
                assert!(view.machines.request.is_none());
            }
        }
    }
}

#[test]
fn agents_are_nested_by_default_and_project_collapse_retains_disclosure() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let mut view = view(120, 40);
        let active = view.machines.space_rows()[0].sidebar_target().unwrap();
        let other = view.machines.space_rows()[1].sidebar_target().unwrap();
        let SidebarTarget::Context { target, .. } = &active else {
            unreachable!()
        };
        view.machines
            .observe_focus(Choice::Local, Some(target.clone()), None);
        view.machines.observe_agents(vec![(
            Choice::Local,
            crate::machine_agents::Observation::Ready(
                crate::machine_agents::test_support::checkpoint(vec![
                    agent_for(target.clone(), 23),
                    agent_for(target.clone(), 24),
                ]),
            ),
        )]);

        let (_, collapsed_hits) = paint(&view, locale, &Theme::terminal_native(), 120, 40);
        assert!((0..40).any(|y| matches!(
            collapsed_hits.hit_target_at(2, y),
            Some(HitTarget::MachineAgentRow { .. })
        )));
        // Workspace disclosure always shows at rest, but only on a Workspace with Agents.
        for (owner, shown) in [(&active, true), (&other, false)] {
            assert_eq!(
                (0..40).any(|y| (0..30).any(|x| matches!(
                    collapsed_hits.hit_target_at(x, y),
                    Some(HitTarget::SidebarDisclosure(actual)) if actual == owner
                ))),
                shown
            );
        }

        assert!(view.machines.sidebar_context_agents_expanded(&active));
        let (rows, expanded_hits) = paint(&view, locale, &Theme::terminal_native(), 120, 40);
        assert_eq!(
            (0..40)
                .filter(|y| matches!(
                    expanded_hits.hit_target_at(2, *y),
                    Some(HitTarget::MachineAgentRow { .. })
                ))
                .count(),
            2
        );
        assert!(
            !rows
                .join("\n")
                .contains(locale.machine_picker_text("no-agents"))
        );

        let project = view.machines.sidebar_parent(&active).unwrap();
        assert!(view.machines.toggle_sidebar_group(&project));
        assert!(view.machines.sidebar_context_agents_expanded(&active));
        let (_, hidden_hits) = paint(&view, locale, &Theme::terminal_native(), 120, 40);
        assert!((0..40).all(|y| !matches!(
            hidden_hits.hit_target_at(2, y),
            Some(HitTarget::MachineAgentRow { .. })
        )));
        assert!(view.machines.toggle_sidebar_group(&project));
        assert!(view.machines.sidebar_context_agents_expanded(&active));
        let (_, restored_hits) = paint(&view, locale, &Theme::terminal_native(), 120, 40);
        assert!((0..40).any(|y| matches!(
            restored_hits.hit_target_at(2, y),
            Some(HitTarget::MachineAgentRow { .. })
        )));
        assert_eq!(view.machines.current_context.as_ref(), Some(target));
        assert!(view.machines.request.is_none());
    }
}

#[test]
fn collapsed_rows_keep_focus_disclosure_and_current_context() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [Theme::terminal_native(), Theme::rgb_test_dark()] {
            for width in [24, 40] {
                let mut view = view(120, 40);
                configure(&mut view, serde_json::json!([[token("workspace")]]), 0);
                let targets = view
                    .machines
                    .space_rows()
                    .iter()
                    .map(|row| row.sidebar_target().unwrap())
                    .collect::<Vec<_>>();
                let contexts = targets
                    .iter()
                    .map(|target| match target {
                        SidebarTarget::Context { target, .. } => target.clone(),
                        _ => unreachable!(),
                    })
                    .collect::<Vec<_>>();
                let mut waiting = agent_for(contexts[0].clone(), 23);
                waiting.state = PanelAgentState::Blocked;
                view.machines.observe_agents(vec![(
                    Choice::Local,
                    crate::machine_agents::Observation::Ready(
                        crate::machine_agents::test_support::checkpoint(vec![
                            waiting,
                            agent_for(contexts[1].clone(), 24),
                        ]),
                    ),
                )]);
                view.machines
                    .observe_focus(Choice::Local, Some(contexts[0].clone()), None);
                let project = view.machines.sidebar_parent(&targets[0]).unwrap();
                for target in [&targets[0], &project] {
                    if matches!(target, SidebarTarget::Project { .. }) {
                        assert!(view.machines.toggle_sidebar_group(target));
                    } else {
                        assert!(view.machines.toggle_sidebar_context_agents(target));
                    }
                    assert!(view.machines.focus_sidebar_target(target));
                    let (buffer, hits) = paint_navigation(&view, locale, &theme, width, 20);
                    let mut text = String::new();
                    for y in 0..20 {
                        let mut x = 0;
                        while x < width {
                            let symbol = buffer[(x, y)].symbol();
                            text.push_str(symbol);
                            x += display_width(symbol).max(1) as u16;
                        }
                        text.push('\n');
                    }
                    let target_rows = sidebar_target_rows(&hits, width, 20, target);
                    assert!(!target_rows.is_empty(), "{text}");
                    let y = target_rows[0];
                    // Workspace rows share the Project heading's edge marker column.
                    assert_eq!(buffer[(0, y)].symbol(), theme.glyphs.navigation_marker);
                    assert_eq!(buffer[(0, y)].bg, theme.color(Role::SurfaceSelection));
                    assert!(target_rows.iter().any(|row| (0..width).any(|x| matches!(
                        hits.hit_target_at(x, *row),
                        Some(HitTarget::SidebarDisclosure(actual)) if actual == target
                    ))));
                    for row in target_rows {
                        for x in 0..width {
                            assert!(
                                hit_belongs_to_sidebar_target(hits.hit_target_at(x, row), target),
                                "collapsed row lost its target at {x}, {row}"
                            );
                        }
                    }
                    assert_eq!(view.machines.current_context.as_ref(), Some(&contexts[0]));
                    assert!(view.machines.request.is_none());
                }
            }
        }
    }
}

fn ungrouped_view(label: &str) -> ClientView {
    let mut view = view(120, 40);
    let mut rows = view
        .machines
        .space_rows()
        .iter()
        .map(|row| view.machines.space_checkpoint_row(row).unwrap().clone())
        .collect::<Vec<_>>();
    for row in &mut rows {
        row.project_id = None;
    }
    let current = rows[0].target.clone();
    view.machines.observe_spaces(vec![(
        Choice::Local,
        crate::machine_spaces::checkpoint::Observation::Ready(std::sync::Arc::new(
            crate::machine_spaces::Checkpoint {
                session_id: current.session_id.clone(),
                revision: WorkbenchRevision::new(3),
                metadata_revision: WorkbenchRevision::new(2),
                git_requested: true,
                known_git: Vec::new().into(),
                rows: rows.into(),
                projects: vec![crate::machine_spaces::ProjectSummary {
                    project_id: WorkbenchProjectId::parse(&id(70)).unwrap(),
                    label: label.into(),
                }]
                .into(),
            },
        )),
    )]);
    view.machines
        .observe_focus(Choice::Local, Some(current), None);
    view
}

#[test]
fn projects_precede_recents_and_create_has_no_overflow_slot() {
    let mut view = ungrouped_view("Empty group");
    let tree = view.machines.sidebar_navigation_rows();
    assert!(matches!(tree[0], SidebarRow::Project { .. }));
    assert!(matches!(tree[1], SidebarRow::Recents { .. }));
    assert!(matches!(tree[2], SidebarRow::Context { .. }));
    assert!(matches!(tree[3], SidebarRow::Context { .. }));
    let workspace = tree[2].target();
    let project = tree[0].target();
    let recents = tree[1].target();
    assert_eq!(
        view.machines.sidebar_parent(&workspace),
        Some(recents.clone())
    );
    assert!(view.machines.focus_sidebar_target(&project));
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let (buffer, hits) = paint_navigation(&view, locale, &Theme::terminal_native(), 40, 20);
        let first_workspace = (0..20)
            .filter(|y| {
                (0..40).any(|x| {
                    matches!(
                        hits.hit_target_at(x, *y),
                        Some(HitTarget::MachineWorkspaceRow { .. })
                    )
                })
            })
            .min()
            .unwrap();
        let project_start = (0..20)
            .find(|y| {
                (0..40).any(|x| {
                    matches!(hits.hit_target_at(x, *y), Some(HitTarget::MachineGroupRow { target, .. }) if target == &project)
                })
            })
            .unwrap();
        // Project, group gap, Recents heading, then the first Workspace outside any Project.
        assert_eq!(first_workspace, project_start + 3);
        assert_eq!(
            buffer[(0, project_start)].symbol(),
            Theme::terminal_native().glyphs.navigation_marker
        );
        let heading = (0..40)
            .map(|x| buffer[(x, first_workspace - 1)].symbol())
            .collect::<String>();
        // Wide glyphs leave blank continuation cells; the heading carries no member count, only
        // its disclosure.
        assert_eq!(
            heading.replace(' ', ""),
            format!(
                "{}{}",
                locale.machine_picker_text("sidebar-recents"),
                Theme::terminal_native().glyphs.disclosure_open
            ),
            "{heading:?}"
        );
        for x in 0..40 {
            assert_eq!(buffer[(x, project_start + 1)].symbol(), " ");
            for y in [project_start + 1, first_workspace - 1] {
                assert_eq!(
                    buffer[(x, y)].bg,
                    Theme::terminal_native().color(Role::SurfaceChrome)
                );
            }
            assert!(matches!(
                hits.hit_target_at(x, project_start + 1),
                Some(HitTarget::MachineWorkspaceSurface(_))
            ));
            // The Recents heading folds like a Project but offers no menu or Create.
            assert!(matches!(
                hits.hit_target_at(x, first_workspace - 1),
                Some(
                    HitTarget::MachineGroupRow { target, .. }
                        | HitTarget::SidebarDisclosure(target)
                ) if target == &recents
            ));
            assert!(matches!(
                hits.hit_target_at(x, project_start),
                Some(
                    HitTarget::MachineGroupRow { target, .. }
                        | HitTarget::SidebarDisclosure(target)
                        | HitTarget::SidebarMenu(target)
                        | HitTarget::CreateMenu(Some(target))
                ) if target == &project
            ));
        }
        assert!((0..20).all(|y| {
            (0..40).all(|x| !matches!(hits.hit_target_at(x, y), Some(HitTarget::SidebarMenu(_))))
        }));
        assert!((0..20).any(|y| (0..40).any(
            |x| hits.hit_target_at(x, y) == Some(&HitTarget::CreateMenu(Some(project.clone())))
        )));
    }
}

#[test]
fn shortest_sidebar_keeps_complete_current_context_and_agent_when_both_fit() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [Theme::terminal_native(), Theme::rgb_test_dark()] {
            let mut view = view(60, 12);
            configure(
                &mut view,
                serde_json::json!([[token("workspace")], [token("branch")], [token("$branch")]]),
                0,
            );
            let selected = view.machines.space_rows()[1].selection();
            let SidebarTarget::Context { target, .. } =
                view.machines.space_rows()[1].sidebar_target().unwrap()
            else {
                unreachable!()
            };
            let agent = agent_for(target.clone(), 23);
            let member = crate::workbench_projection::ClientMemberTarget {
                tab: agent.target.tab.clone(),
                panel: agent.target.panel.clone(),
                presentation: agent.target.member.clone(),
            };
            view.machines.observe_agents(vec![(
                Choice::Local,
                crate::machine_agents::Observation::Ready(
                    crate::machine_agents::test_support::checkpoint(vec![agent]),
                ),
            )]);
            view.machines
                .observe_focus(Choice::Local, Some(target.clone()), Some(member));
            expand_context_agents(&mut view, target);
            let (rows, hits) = paint(&view, locale, &theme, 60, 12);
            assert_eq!((0..12).filter(|y| matches!(hits.hit_target_at(4,*y), Some(HitTarget::MachineWorkspaceRow { selected: actual, .. }) if actual == &selected)).count(), 3);
            assert_eq!(
                (0..12)
                    .filter(|y| matches!(
                        hits.hit_target_at(2, *y),
                        Some(HitTarget::MachineAgentRow { .. })
                    ))
                    .count(),
                1
            );
            // The footer divider yields; only the Agent's tree connector draws a rule glyph.
            assert!(!rows.iter().any(|row| row.contains("───")), "{rows:?}");
            assert!(
                rows.join("\n")
                    .contains(locale.machine_picker_text("agent-search"))
            );
        }
    }
}

#[test]
fn duplicate_project_and_workspace_labels_paint_alone_and_stay_separate_rows_when_narrow() {
    let mut view = view(120, 40);
    let mut rows: Vec<_> = view
        .machines
        .space_rows()
        .iter()
        .map(|row| view.machines.space_checkpoint_row(row).unwrap().clone())
        .collect();
    for (index, row) in rows.iter_mut().enumerate() {
        let ClientContextLocation::Workspace { workspace_id, .. } = &mut row.target.location else {
            unreachable!()
        };
        *workspace_id =
            WorkbenchWorkspaceId::parse(&format!("{}{}", index + 1, "0".repeat(31))).unwrap();
        row.label = Some("same 测试🙂".repeat(20));
    }
    view.machines.observe_spaces(vec![(
        Choice::Local,
        crate::machine_spaces::checkpoint::Observation::Ready(std::sync::Arc::new(
            crate::machine_spaces::Checkpoint {
                session_id: SessionId::parse(&id(1)).unwrap(),
                revision: WorkbenchRevision::new(2),
                metadata_revision: WorkbenchRevision::new(2),
                git_requested: false,
                known_git: Vec::new().into(),
                projects: [70, 71]
                    .into_iter()
                    .map(|n| crate::machine_spaces::ProjectSummary {
                        project_id: WorkbenchProjectId::parse(&id(n)).unwrap(),
                        label: "same 项目🙂".repeat(20),
                    })
                    .collect(),
                rows: rows.into(),
            },
        )),
    )]);
    configure(&mut view, serde_json::json!([[token("workspace")]]), 0);
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let theme = Theme::terminal_native();
        // Duplicate names are allowed: no row adds an ID or a line to tell its peers apart.
        let (buffer, hits) = paint_navigation(&view, locale, &theme, 24, 30);
        for row in view.machines.space_rows() {
            let selection = row.selection();
            let painted: Vec<String> = (0..30)
                .filter(|y| matches!(
                    hits.hit_target_at(0, *y),
                    Some(HitTarget::MachineWorkspaceRow { selected, .. }) if selected == &selection
                ))
                .map(|y| {
                    (0..24)
                        .map(|x| buffer[(x, y)].symbol())
                        .collect::<String>()
                })
                .collect();
            assert_eq!(painted.len(), 1, "{painted:?}");
            assert!(painted[0].trim_start().starts_with("same"), "{painted:?}");
            assert!(!painted[0].contains('['), "{painted:?}");
        }
        assert!((0..30).any(|y| matches!(
            hits.hit_target_at(4, y),
            Some(HitTarget::MachineGroupRow { target: SidebarTarget::Project { project_id, .. }, .. })
                if project_id.as_str() == id(71)
        )));
    }
}

#[test]
fn duplicate_agent_titles_paint_alone_and_stay_separate_rows() {
    let mut view = view(120, 40);
    let SidebarTarget::Context { target, .. } =
        view.machines.space_rows()[0].sidebar_target().unwrap()
    else {
        unreachable!()
    };
    let mut first = agent_for(target.clone(), 23);
    let mut second = agent_for(target.clone(), 24);
    first.label = "same Agent title".into();
    second.label = "same Agent title".into();
    view.machines.observe_agents(vec![(
        Choice::Local,
        crate::machine_agents::Observation::Ready(crate::machine_agents::test_support::checkpoint(
            vec![first, second],
        )),
    )]);
    expand_context_agents(&mut view, target);

    for width in [30, 24] {
        let (buffer, hits) =
            paint_navigation(&view, Locale::EnUs, &Theme::terminal_native(), width, 20);
        let text = (0..20)
            .map(|y| {
                (0..width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        // The title gives way to keep the provider whole, and no ID tag tells the rows apart.
        assert_eq!(text.matches("same Ag").count(), 2, "{width}: {text}");
        assert_eq!(text.matches(" · codex").count(), 2, "{width}: {text}");
        assert!(!text.contains('['), "{width}: {text}");
        let mut selected = Vec::new();
        for y in 0..20 {
            if let Some(HitTarget::MachineAgentRow { selected: row, .. }) = hits.hit_target_at(2, y)
                && !selected.contains(row)
            {
                selected.push(row.clone());
            }
        }
        assert_eq!(selected.len(), 2);
    }
}

#[test]
fn colored_current_context_and_agent_keep_plain_identity_state_and_unread_cues_independent() {
    for theme in [Theme::terminal_native(), Theme::rgb_test_dark()] {
        let mut view = view(120, 40);
        let mut workspace = token("workspace");
        workspace["style"]["bold"] = serde_json::json!(false);
        configure(&mut view, serde_json::json!([[workspace]]), 0);
        view.machines.configure_agent_sidebar(
            orchbee_local_runtime::AgentSidebarSettings::from_profile(
                &serde_json::json!({
                    "rows": [[{
                        "name": "pane",
                        "style": {"fg": null, "bold": false, "dim": null},
                        "rules": []
                    }]],
                    "rows_by_agent": {},
                    "row_gap": 0
                })
                .to_string(),
            )
            .unwrap(),
        );
        let context = view.machines.space_rows()[0].sidebar_target().unwrap();
        let SidebarTarget::Context {
            target: context_target,
            ..
        } = &context
        else {
            unreachable!()
        };
        let mut agent = agent_for(context_target.clone(), 23);
        agent.seen = false;
        agent.metadata.title = Some(MetadataTokenValue::parse("Input handling").unwrap());
        let member = agent.target.clone();
        view.machines.observe_agents(vec![(
            Choice::Local,
            crate::machine_agents::Observation::Ready(
                crate::machine_agents::test_support::checkpoint(vec![agent]),
            ),
        )]);
        expand_context_agents(&mut view, context_target.clone());
        view.machines.observe_focus(
            Choice::Local,
            Some(context_target.clone()),
            Some(crate::workbench_projection::ClientMemberTarget {
                tab: member.tab.clone(),
                panel: member.panel.clone(),
                presentation: member.member.clone(),
            }),
        );
        assert!(!view.machines.sidebar_navigation_active());
        let agent = view
            .machines
            .sidebar_navigation_rows()
            .iter()
            .find_map(|row| matches!(row, SidebarRow::Agent { .. }).then(|| row.target()))
            .unwrap();

        let (candidate, _, hits) = paint_navigation_buffers(&view, Locale::EnUs, &theme, 120, 40);
        for target in [&context, &agent] {
            let rows = sidebar_target_rows(&hits, 120, 40, target);
            assert_eq!(rows.len(), 1);
            let y = rows[0];
            let tracks = layout::Tracks::resolve(
                Rect::new(0, y, 120, 1),
                0,
                auxiliary_actions(&view.machines.sidebar_source(), target),
                layout::RowKind::for_target(target),
            )
            .unwrap();
            assert_eq!(candidate[(tracks.navigation.x, y)].symbol(), " ");
            // The fill alone marks current identity; no row adds a text marker.
            assert!((0..120).all(|x| candidate[(x, y)].symbol() != theme.glyphs.active_marker));
            // Only the Agent row has a state column.
            if target == &agent {
                assert_eq!(
                    candidate[(tracks.state.x, y)].symbol(),
                    theme.agent_glyph(orchbee_workbench_protocol::MetadataState::Working)
                );
            }
            // An expanded Workspace leaves the unread cue to its visible Agent row.
            assert_eq!(
                candidate[(tracks.unread.x, y)].symbol(),
                if target == &context {
                    " "
                } else {
                    theme.glyphs.status_bullet
                }
            );
            let identity_x = (tracks.primary.x..tracks.primary.right())
                .find(|x| candidate[(*x, y)].symbol() != " ")
                .unwrap();
            assert!(!candidate[(identity_x, y)].modifier.contains(Modifier::BOLD));
            // The Agent surface includes leading padding; the tree lead stays on chrome.
            for x in 0..120 {
                assert_eq!(
                    candidate[(x, y)].bg,
                    theme.color(if target != &context && x < tracks.surface().x {
                        Role::SurfaceChrome
                    } else {
                        Role::SurfaceCurrent
                    })
                );
                assert!(hit_belongs_to_sidebar_target(
                    hits.hit_target_at(x, y),
                    target
                ));
            }
        }
    }
}

#[test]
fn monochrome_current_workspace_underlines_without_bold_or_text_marker_beside_focus() {
    let mut view = view(120, 40);
    let mut workspace = token("workspace");
    workspace["style"]["bold"] = serde_json::json!(false);
    configure(&mut view, serde_json::json!([[workspace]]), 0);
    let active = view.machines.space_rows()[0].sidebar_target().unwrap();
    let SidebarTarget::Context { target, .. } = &active else {
        unreachable!()
    };
    let mut agent = agent_for(target.clone(), 23);
    agent.seen = false;
    view.machines.observe_agents(vec![(
        Choice::Local,
        crate::machine_agents::Observation::Ready(crate::machine_agents::test_support::checkpoint(
            vec![agent],
        )),
    )]);
    // Folded Agent rows hand their unread cue and states to the Workspace row.
    if view.machines.sidebar_context_agents_expanded(&active) {
        assert!(view.machines.toggle_sidebar_context_agents(&active));
    }
    view.machines
        .observe_focus(Choice::Local, Some(target.clone()), None);
    let theme = Theme::terminal_native().quantize(crate::tui_theme::ColorLevel::Monochrome);
    for focused in [false, true] {
        if focused {
            assert!(view.machines.focus_sidebar_target(&active));
        }
        let (buffer, hits) = paint_navigation(&view, Locale::EnUs, &theme, 26, 10);
        let rows = sidebar_target_rows(&hits, 26, 10, &active);
        assert_eq!(
            rows.len(),
            2,
            "a folded Workspace adds its Agents' status line"
        );
        let y = rows[0];
        let tracks = layout::Tracks::resolve(
            Rect::new(0, y, 26, rows.len() as u16),
            0,
            auxiliary_actions(&view.machines.sidebar_source(), &active),
            layout::RowKind::for_target(&active),
        )
        .unwrap();
        assert_eq!(
            buffer[(tracks.navigation.x, y)].symbol(),
            if focused {
                theme.glyphs.navigation_marker
            } else {
                " "
            }
        );
        assert!((0..26).all(|x| buffer[(x, y)].symbol() != theme.glyphs.active_marker));
        assert_eq!(
            buffer[(tracks.primary.x, y + 1)].symbol(),
            theme.agent_glyph(orchbee_workbench_protocol::MetadataState::Working)
        );
        assert_eq!(
            buffer[(tracks.unread.x, y)].symbol(),
            theme.glyphs.status_bullet
        );
        let identity = &buffer[(tracks.primary.x, y)];
        assert!(!identity.modifier.contains(Modifier::BOLD), "{focused}");
        // Monochrome cannot fill the current row, so an underline stands in at rest.
        if !focused {
            assert!(identity.modifier.contains(Modifier::UNDERLINED));
        }
    }
}

#[test]
fn alignment_sidebar_tree_keeps_current_focus_children_and_whole_shell_evidence() {
    for (name, width, height, locale) in [
        (
            "alignment-sidebar-tree-en-us-120x40-default",
            120,
            40,
            Locale::EnUs,
        ),
        (
            "alignment-sidebar-tree-zh-cn-80x24-default",
            80,
            24,
            Locale::ZhCn,
        ),
    ] {
        let mut view = view(width, height);
        view.configure_color_level(crate::tui_theme::ColorLevel::Truecolor);
        view.configure_theme(crate::tui_theme::ThemeName::default());
        configure(
            &mut view,
            serde_json::json!([[token("state_icon"), token("workspace")], [token("branch")]]),
            0,
        );
        let active = view.machines.space_rows()[0].sidebar_target().unwrap();
        let active_selection = view.machines.space_rows()[0].selection();
        let focused_selection = view.machines.space_rows()[1].selection();
        let SidebarTarget::Context { target, .. } = &active else {
            unreachable!()
        };
        view.machines
            .observe_focus(Choice::Local, Some(target.clone()), None);
        let mut agent = agent_for(target.clone(), 23);
        agent.metadata.title = Some(MetadataTokenValue::parse("Input handling").unwrap());
        let agent_selection = agent.target.clone();
        view.machines.observe_agents(vec![(
            Choice::Local,
            crate::machine_agents::Observation::Ready(
                crate::machine_agents::test_support::checkpoint(vec![agent]),
            ),
        )]);
        expand_context_agents(&mut view, target.clone());
        assert!(view.machines.focus_sidebar_target(&active));
        assert!(matches!(
            view.machines
                .sidebar_key(crate::client_view::KeyEvent::press(
                    crate::client_view::KeyCode::End,
                    crate::client_view::Modifiers::NONE,
                )),
            crate::machine_picker::SidebarIntent::None
        ));
        assert_eq!(view.machines.current_context.as_ref(), Some(target));

        let theme = view.configured_theme().unwrap();
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        let mut hits = HitMapBuilder::new();
        terminal
            .draw(|frame| {
                render_terminal_shell(frame, &view, locale, None, &theme, &mut hits);
            })
            .unwrap();
        let hits = hits.finish();
        assert!((0..height).any(|y| (0..width).any(|x| matches!(
            hits.hit_target_at(x, y),
            Some(HitTarget::MachineWorkspaceRow { selected, .. })
                if selected == &active_selection
        ))));
        assert!((0..height).any(|y| (0..width).any(|x| matches!(
            hits.hit_target_at(x, y),
            Some(HitTarget::MachineWorkspaceRow { selected, .. })
                if selected == &focused_selection
        ))));
        assert!((0..height).any(|y| (0..width).any(|x| matches!(
            hits.hit_target_at(x, y),
            Some(HitTarget::MachineAgentRow { selected, .. })
                if matches!(&selected.context, Some(crate::startup_directory::ClientStartupTarget::Agent(target)) if target.as_ref() == &agent_selection)
        ))));
        assert!((0..height).all(|y| {
            (0..width).all(|x| !matches!(hits.hit_target_at(x, y), Some(HitTarget::SidebarMenu(_))))
        }));
        crate::tui_render::test_capture::capture_screen(&mut terminal, name, locale, &theme);
        let before = terminal.backend().buffer().clone();
        let now = std::time::Instant::now();
        for step in 0..10 {
            assert!(
                view.machines.advance_working_motion(
                    now + std::time::Duration::from_millis(step * 100),
                    false
                )
            );
        }
        let mut animated_hits = HitMapBuilder::new();
        terminal
            .draw(|frame| {
                render_terminal_shell(frame, &view, locale, None, &theme, &mut animated_hits);
            })
            .unwrap();
        let after = terminal.backend().buffer();
        let mut changed = 0;
        for y in 0..height {
            for x in 0..width {
                if before[(x, y)] != after[(x, y)] {
                    assert_eq!(before[(x, y)].symbol(), theme.glyphs.working_frames[0]);
                    assert_eq!(after[(x, y)].symbol(), theme.glyphs.working_frames[0]);
                    assert_ne!(before[(x, y)].style(), after[(x, y)].style());
                    changed += 1;
                }
            }
        }
        assert!(
            changed > 0,
            "the Sidebar must repaint observed Working Agents between ticks"
        );
    }
}
