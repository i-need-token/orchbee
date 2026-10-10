use super::*;
use crate::{
    machine_discovery::Context as NavigationContext,
    machine_picker::Choice,
    terminal_session::{NativeContextLocation, NativeContextTarget},
};
use orchbee_workbench_protocol::*;
use ratatui::{Terminal, backend::TestBackend};
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
                crate::client_view::SessionActionDisposition::KeepNavigator,
            );
        }
    }
    view.machines.observe_agent_sidebar(
        project_shell_areas(Rect::new(0, 0, width, height), view.sidebar_surface())
            .sidebar
            .is_some(),
    );
    let site = SiteId::parse(&id(1)).unwrap();
    let rows: Vec<_> = (0..2)
        .map(|n| crate::machine_spaces::Row {
            project_id: Some(WorkbenchProjectId::parse(&id(70)).unwrap()),
            target: NativeContextTarget {
                site_id: site.clone(),
                location: NativeContextLocation::Workspace {
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
        crate::machine_spaces::Observation::Ready(std::sync::Arc::new(
            crate::machine_spaces::Checkpoint {
                site_id: site,
                revision: WorkbenchRevision::new(1),
                metadata_revision: WorkbenchRevision::new(2),
                git_requested: true,
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
        crate::machine_agents::Observation::Ready(crate::machine_agents::tests::checkpoint(
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
        orchbee_local_site_runtime::WindowsSpaceSidebarSettings::from_profile(
            &serde_json::json!({"rows":rows,"row_gap":gap}).to_string(),
        )
        .unwrap(),
    );
}

fn agent_for(target: NativeContextTarget, number: u64) -> crate::machine_agents::Row {
    let mut agent = crate::machine_agents::tests::row(number);
    agent.target.context = target;
    agent
}

fn expand_context_agents(view: &mut ClientView, target: NativeContextTarget) {
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
) -> (Vec<String>, crate::tui_pointer::FrameHitMap) {
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
#[test]
fn workspace_rows_render_real_branch_divergence_and_custom_names_with_exact_hits() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let mut ascii = Theme::terminal_native();
        ascii.glyphs = crate::tui_theme::GlyphSet::ASCII;
        for theme in [
            Theme::terminal_native(),
            Theme::rgb_test_dark(),
            Theme::one_light(),
            Theme::one_dark(),
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
            let (rows, hits) = paint(&view, locale, &theme, 120, 40);
            let text = rows.join("\n");
            assert_eq!(
                rows.iter().filter(|r| r.contains("custom branch")).count(),
                2,
                "{text}"
            );
            assert!(text.contains("feat/测试"));
            assert!(text.contains(&format!(
                "{}2 {}3",
                theme.glyphs.git_ahead, theme.glyphs.git_behind
            )));
            let targets: Vec<_> = (0..40)
                .filter_map(|y| match hits.hit_target_at(2, y) {
                    Some(HitTarget::MachineWorkspaceRow { selected, .. }) => Some(selected),
                    _ => None,
                })
                .collect();
            assert_eq!(targets.len(), 6);
            assert_ne!(targets[0], targets[3]);
            assert!(targets[..3].iter().all(|t| t == &targets[0]));
            assert!(targets[3..].iter().all(|t| t == &targets[3]));
            assert!((0..40).any(|y| matches!(
                hits.hit_target_at(2, y),
                Some(HitTarget::OpenMachineWorkspaces)
            )));
            assert!((0..40).any(|y| {
                (0..30)
                    .any(|x| matches!(hits.hit_target_at(x, y), Some(HitTarget::OpenMachineAgents)))
            }));
        }
    }
}

#[test]
fn default_width_header_keeps_complete_labels_and_disjoint_full_width_controls() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let view = view(60, 12);
        let (rows, hits) = paint(&view, locale, &Theme::terminal_native(), 60, 12);
        let header = &rows[1];
        for label in [
            locale.shell().workspaces,
            locale.machine_picker_text("agent-sidebar-open"),
        ] {
            assert!(header.contains(label), "{locale:?}: {header:?}");
        }
        assert!(!header.contains('…'), "{locale:?}: {header:?}");
        assert!(
            header.contains('2'),
            "the complete Workspace count survives grouping"
        );

        let workspaces = (0..30)
            .filter(|x| {
                matches!(
                    hits.hit_target_at(*x, 1),
                    Some(HitTarget::OpenMachineWorkspaces)
                )
            })
            .collect::<Vec<_>>();
        let agents = (0..30)
            .filter(|x| {
                matches!(
                    hits.hit_target_at(*x, 1),
                    Some(HitTarget::OpenMachineAgents)
                )
            })
            .collect::<Vec<_>>();
        let create = (0..30)
            .filter(|x| matches!(hits.hit_target_at(*x, 1), Some(HitTarget::CreateMenu(None))))
            .collect::<Vec<_>>();
        assert_eq!(workspaces, (2..agents[0]).collect::<Vec<_>>());
        assert_eq!(agents, (agents[0]..25).collect::<Vec<_>>());
        assert_eq!(create, (25..28).collect::<Vec<_>>());
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
    assert_eq!(
        (0..40)
            .filter(|y| matches!(
                hits.hit_target_at(2, *y),
                Some(HitTarget::MachineWorkspaceRow { .. })
            ))
            .count(),
        2
    );
    configure(&mut view, serde_json::json!([]), 0);
    let (rows, hits) = paint(&view, Locale::EnUs, &Theme::terminal_native(), 120, 40);
    assert!(!rows.join("\n").contains("custom branch"));
    assert_eq!(
        (0..40)
            .filter(|y| matches!(
                hits.hit_target_at(2, *y),
                Some(HitTarget::MachineWorkspaceRow { .. })
            ))
            .count(),
        2
    );
}
#[test]
fn workspace_sidebar_degrades_across_tiers_and_preserves_header_keyboard_route() {
    for (width, height) in [(120, 40), (80, 24), (60, 12), (50, 10), (15, 4)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let mut ascii = Theme::terminal_native();
            ascii.glyphs = crate::tui_theme::GlyphSet::ASCII;
            for theme in [
                Theme::terminal_native(),
                Theme::rgb_test_dark(),
                Theme::one_light(),
                Theme::one_dark(),
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
                        Some(HitTarget::OpenMachineWorkspaces)
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
fn compact_sidebar_yields_heading_space_before_clipping_current_workspace_or_agent() {
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
        let member = crate::terminal_session::NativeMemberTarget {
            tab: agent.target.tab.clone(),
            panel: agent.target.panel.clone(),
            presentation: agent.target.member.clone(),
        };
        view.machines.observe_agents(vec![(
            Choice::Local,
            crate::machine_agents::Observation::Ready(crate::machine_agents::tests::checkpoint(
                vec![agent],
            )),
        )]);
        view.machines
            .observe_focus(Choice::Local, Some(target.clone()), Some(member));
        expand_context_agents(&mut view, target);
        let (_, hits) = paint(&view, locale, &Theme::terminal_native(), 60, 14);
        assert_eq!((0..14).filter(|y| matches!(hits.hit_target_at(2, *y), Some(HitTarget::MachineWorkspaceRow { selected: actual, .. }) if actual == &selected)).count(), 3);
        assert_eq!(
            (0..14)
                .filter(|y| matches!(
                    hits.hit_target_at(2, *y),
                    Some(HitTarget::MachineAgentRow { .. })
                ))
                .count(),
            2
        );
    }
}
#[test]
fn workspace_gap_surfaces_do_not_activate_rows_and_style_patches_preserve_count_roles() {
    let mut view = view(120, 40);
    let mut git = token("git_status");
    git["style"] = serde_json::json!({"fg":[16,32,48],"bold":true,"dim":false});
    configure(
        &mut view,
        serde_json::json!([[token("workspace")], [git]]),
        1,
    );
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
        crate::machine_spaces::Observation::Ready(std::sync::Arc::new(
            crate::machine_spaces::Checkpoint {
                site_id: SiteId::parse(&id(1)).unwrap(),
                revision: WorkbenchRevision::new(1),
                metadata_revision: WorkbenchRevision::new(2),
                git_requested: true,
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

fn paint_navigation(
    view: &ClientView,
    locale: Locale,
    theme: &Theme,
    width: u16,
    height: u16,
) -> (ratatui::buffer::Buffer, crate::tui_pointer::FrameHitMap) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut hits = HitMapBuilder::new();
    let rows = view.machines.sidebar_navigation_rows();
    terminal
        .draw(|frame| {
            render(
                frame,
                Rect::new(0, 0, width, height),
                RenderContext {
                    view,
                    locale,
                    theme,
                    layer: HitLayer::Chrome,
                },
                &rows,
                Some(&mut hits),
            );
        })
        .unwrap();
    (terminal.backend().buffer().clone(), hits.finish())
}

#[test]
fn project_disclosure_rows_and_menus_have_disjoint_hits_and_focus_is_not_active_context() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [
            Theme::terminal_native(),
            Theme::rgb_test_dark(),
            Theme::one_light(),
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
                assert!(view.machines.focus_sidebar_target(&focused));
                let project = view.machines.sidebar_parent(&active).unwrap();
                let (buffer, hits) = paint_navigation(&view, locale, &theme, width, 10);
                let menu_start = width - menu_width(&view, &project, &theme);
                for x in 2..4 {
                    assert_eq!(
                        hits.hit_target_at(x, 2),
                        Some(&HitTarget::SidebarDisclosure(project.clone()))
                    );
                }
                let create_start = menu_start - 3;
                for x in (0..2).chain(4..create_start) {
                    assert!(matches!(
                        hits.hit_target_at(x, 2),
                        Some(HitTarget::MachineProjectRow { target, .. }) if target == &project
                    ));
                }
                for x in create_start..menu_start {
                    assert_eq!(
                        hits.hit_target_at(x, 2),
                        Some(&HitTarget::CreateMenu(Some(project.clone())))
                    );
                }
                for x in menu_start..width {
                    assert_eq!(
                        hits.hit_target_at(x, 2),
                        Some(&HitTarget::SidebarMenu(project.clone()))
                    );
                    assert_eq!(
                        hits.hit_target_at(x, 3),
                        Some(&HitTarget::SidebarMenu(active.clone()))
                    );
                    assert_eq!(
                        hits.hit_target_at(x, 5),
                        Some(&HitTarget::SidebarMenu(focused.clone()))
                    );
                }
                assert_eq!(buffer[(0, 3)].symbol(), " ");
                assert_eq!(buffer[(0, 5)].symbol(), ">");
                assert_eq!(buffer[(0, 5)].fg, theme.color(Role::AccentFocus));
                assert_eq!(buffer[(4, 2)].symbol(), "R");
                assert_eq!(buffer[(6, 3)].symbol(), "s");
                assert_eq!(buffer[(6, 4)].symbol(), "f");
                for y in [3, 4] {
                    assert_eq!(buffer[(0, y)].bg, theme.color(Role::SurfaceCurrent));
                }
                for y in [5, 6] {
                    assert_ne!(buffer[(0, y)].bg, theme.color(Role::SurfaceCurrent));
                    for x in 0..menu_start {
                        assert!(matches!(
                            hits.hit_target_at(x, y),
                            Some(HitTarget::MachineWorkspaceRow { selected, .. })
                                if selected == &view.machines.space_rows()[1].selection()
                        ));
                    }
                }
                assert_ne!(buffer[(0, 3)].bg, buffer[(0, 5)].bg);
                view.machines.leave_sidebar_navigation();
                let (ordinary, _) = paint_navigation(&view, locale, &theme, width, 10);
                for y in [3, 4] {
                    assert_eq!(ordinary[(0, y)].bg, theme.color(Role::SurfaceCurrent));
                }
                assert_eq!(ordinary[(0, 5)].symbol(), " ");
                assert!(view.machines.focus_sidebar_target(&project));
                assert!(view.machines.toggle_sidebar_project(&project));
                let (collapsed_buffer, collapsed) =
                    paint_navigation(&view, locale, &theme, width, 10);
                assert_eq!(
                    collapsed_buffer[(0, 2)].bg,
                    theme.color(Role::SurfaceCurrent)
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
            crate::machine_agents::Observation::Ready(crate::machine_agents::tests::checkpoint(
                vec![agent_for(target.clone(), 23), agent_for(target.clone(), 24)],
            )),
        )]);

        let (_, collapsed_hits) = paint(&view, locale, &Theme::terminal_native(), 120, 40);
        assert!((0..40).any(|y| matches!(
            collapsed_hits.hit_target_at(2, y),
            Some(HitTarget::MachineAgentRow { .. })
        )));
        assert!((0..40).any(|y| (0..30).any(|x| matches!(
            collapsed_hits.hit_target_at(x, y),
            Some(HitTarget::SidebarDisclosure(actual)) if actual == &active
        ))));
        assert!((0..40).all(|y| (0..30).all(|x| !matches!(
            collapsed_hits.hit_target_at(x, y),
            Some(HitTarget::SidebarDisclosure(actual)) if actual == &other
        ))));

        assert!(view.machines.sidebar_context_agents_expanded(&active));
        let (rows, expanded_hits) = paint(&view, locale, &Theme::terminal_native(), 120, 40);
        assert_eq!(
            (0..40)
                .filter(|y| matches!(
                    expanded_hits.hit_target_at(2, *y),
                    Some(HitTarget::MachineAgentRow { .. })
                ))
                .count(),
            4
        );
        assert!(
            !rows
                .join("\n")
                .contains(locale.machine_picker_text("no-agents"))
        );

        let project = view.machines.sidebar_parent(&active).unwrap();
        assert!(view.machines.toggle_sidebar_project(&project));
        assert!(view.machines.sidebar_context_agents_expanded(&active));
        let (_, hidden_hits) = paint(&view, locale, &Theme::terminal_native(), 120, 40);
        assert!((0..40).all(|y| !matches!(
            hidden_hits.hit_target_at(2, y),
            Some(HitTarget::MachineAgentRow { .. })
        )));
        assert!(view.machines.toggle_sidebar_project(&project));
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
fn collapsed_rows_keep_localized_agent_counts_waiting_and_current_context() {
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
                        crate::machine_agents::tests::checkpoint(vec![
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
                        assert!(view.machines.toggle_sidebar_project(target));
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
                    let total = if matches!(target, SidebarTarget::Project { .. }) {
                        2
                    } else {
                        1
                    };
                    assert!(text.contains(&locale.sidebar_agent_count(total)), "{text}");
                    assert!(text.contains(&locale.sidebar_waiting_count(1)), "{text}");
                    let y = (0..20).find(|y| (0..width).any(|x| matches!(hits.hit_target_at(x, *y), Some(HitTarget::SidebarDisclosure(actual)) if actual == target))).unwrap();
                    assert_eq!(buffer[(0, y)].symbol(), ">");
                    assert_eq!(buffer[(0, y)].bg, theme.color(Role::SurfaceCurrent));
                    for x in 0..width {
                        assert!(
                            match (target, hits.hit_target_at(x, y + 1)) {
                                (
                                    SidebarTarget::Project { .. },
                                    Some(HitTarget::MachineProjectRow { target: actual, .. }),
                                ) => actual == target,
                                (
                                    SidebarTarget::Context { target, .. },
                                    Some(HitTarget::MachineWorkspaceRow { selected, .. }),
                                ) =>
                                    selected.context.as_ref()
                                        == Some(
                                            &crate::terminal_session::NativeStartupTarget::Context(
                                                target.clone()
                                            )
                                        ),
                                _ => false,
                            },
                            "count continuation lost its row hit at {x}, {}",
                            y + 1
                        );
                    }
                    assert_eq!(view.machines.current_context.as_ref(), Some(&contexts[0]));
                    assert!(view.machines.request.is_none());
                    assert_eq!(view.machines.sidebar_workspace_count(), 2);
                }
            }
        }
    }
}

#[test]
fn ungrouped_workspace_is_a_root_peer_and_empty_project_keeps_menu_hit() {
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
        crate::machine_spaces::Observation::Ready(std::sync::Arc::new(
            crate::machine_spaces::Checkpoint {
                site_id: current.site_id.clone(),
                revision: WorkbenchRevision::new(3),
                metadata_revision: WorkbenchRevision::new(2),
                git_requested: true,
                rows: rows.into(),
                projects: vec![crate::machine_spaces::ProjectSummary {
                    project_id: WorkbenchProjectId::parse(&id(70)).unwrap(),
                    label: "Empty group".into(),
                }]
                .into(),
            },
        )),
    )]);
    view.machines
        .observe_focus(Choice::Local, Some(current), None);
    let tree = view.machines.sidebar_navigation_rows();
    assert!(matches!(tree[0], SidebarRow::Context { .. }));
    assert!(matches!(tree[1], SidebarRow::Context { .. }));
    assert!(matches!(tree[2], SidebarRow::Project { .. }));
    let workspace = tree[0].target();
    let project = tree[2].target();
    assert!(view.machines.sidebar_parent(&workspace).is_none());
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let (_, hits) = paint_navigation(&view, locale, &Theme::terminal_native(), 40, 20);
        assert!(
            (0..20).any(|y| (0..40).any(
                |x| hits.hit_target_at(x, y) == Some(&HitTarget::SidebarMenu(project.clone()))
            ))
        );
        assert!((0..20).any(|y| (0..40).any(
            |x| hits.hit_target_at(x, y) == Some(&HitTarget::CreateMenu(Some(project.clone())))
        )));
    }
}

#[test]
fn shortest_sidebar_drops_context_and_separator_before_complete_current_rows() {
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
            let member = crate::terminal_session::NativeMemberTarget {
                tab: agent.target.tab.clone(),
                panel: agent.target.panel.clone(),
                presentation: agent.target.member.clone(),
            };
            view.machines.observe_agents(vec![(
                Choice::Local,
                crate::machine_agents::Observation::Ready(
                    crate::machine_agents::tests::checkpoint(vec![agent]),
                ),
            )]);
            view.machines
                .observe_focus(Choice::Local, Some(target.clone()), Some(member));
            expand_context_agents(&mut view, target);
            let (rows, hits) = paint(&view, locale, &theme, 60, 12);
            assert_eq!((0..12).filter(|y| matches!(hits.hit_target_at(2,*y), Some(HitTarget::MachineWorkspaceRow { selected: actual, .. }) if actual == &selected)).count(), 3);
            assert_eq!(
                (0..12)
                    .filter(|y| matches!(
                        hits.hit_target_at(2, *y),
                        Some(HitTarget::MachineAgentRow { .. })
                    ))
                    .count(),
                2
            );
            assert!(!rows.join("\n").contains('─'));
            assert!(
                rows.join("\n")
                    .contains(locale.machine_picker_text("agent-sidebar-open"))
            );
        }
    }
}

#[test]
fn duplicate_project_and_workspace_labels_keep_complete_distinguishing_ids_when_narrow() {
    let mut view = view(120, 40);
    let mut rows: Vec<_> = view
        .machines
        .space_rows()
        .iter()
        .map(|row| view.machines.space_checkpoint_row(row).unwrap().clone())
        .collect();
    for (index, row) in rows.iter_mut().enumerate() {
        let NativeContextLocation::Workspace { workspace_id, .. } = &mut row.target.location else {
            unreachable!()
        };
        *workspace_id =
            WorkbenchWorkspaceId::parse(&format!("{}{}", index + 1, "0".repeat(31))).unwrap();
        row.label = Some("same 测试🙂".repeat(20));
    }
    view.machines.observe_spaces(vec![(
        Choice::Local,
        crate::machine_spaces::Observation::Ready(std::sync::Arc::new(
            crate::machine_spaces::Checkpoint {
                site_id: SiteId::parse(&id(1)).unwrap(),
                revision: WorkbenchRevision::new(2),
                metadata_revision: WorkbenchRevision::new(2),
                git_requested: false,
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
        let navigation = view.machines.sidebar_navigation_rows();
        let annotations = annotations(&view, &navigation, locale);
        assert_eq!(annotations[0], " [00000046]");
        assert_eq!(annotations[3], " [00000047]");
        for (index, row) in view.machines.space_rows().iter().enumerate() {
            assert_eq!(
                annotations[index + 1],
                format!(" [{}]", identity(&row.sidebar_target().unwrap()))
            );
        }
        let (buffer, hits) = paint_navigation(&view, locale, &theme, 16, 30);
        for row in view.machines.space_rows() {
            let selection = row.selection();
            let displayed: String = (0..30)
                .filter(|y| matches!(
                    hits.hit_target_at(0, *y),
                    Some(HitTarget::MachineWorkspaceRow { selected, .. }) if selected == &selection
                ))
                .map(|y| {
                    (4..16)
                        .map(|x| buffer[(x, y)].symbol())
                        .collect::<String>()
                        .trim_end()
                        .to_owned()
                })
                .collect();
            assert!(
                displayed.contains(identity(&row.sidebar_target().unwrap())),
                "{displayed}"
            );
        }
        assert!((0..30).any(|y| matches!(
            hits.hit_target_at(4, y),
            Some(HitTarget::MachineProjectRow { target: SidebarTarget::Project { project_id, .. }, .. })
                if project_id.as_str() == id(71)
        )));
    }
}

#[test]
fn duplicate_agent_titles_keep_member_identity_inline_and_on_narrow_continuations() {
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
        crate::machine_agents::Observation::Ready(crate::machine_agents::tests::checkpoint(vec![
            first, second,
        ])),
    )]);
    expand_context_agents(&mut view, target);

    for width in [30, 16] {
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
        assert!(text.contains("[00000017]"), "{width}: {text}");
        assert!(text.contains("[00000018]"), "{width}: {text}");
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
fn monochrome_current_context_survives_explicit_plain_text_and_simultaneous_focus() {
    let mut view = view(120, 40);
    let mut workspace = token("workspace");
    workspace["style"]["bold"] = serde_json::json!(false);
    configure(&mut view, serde_json::json!([[workspace]]), 0);
    let active = view.machines.space_rows()[0].sidebar_target().unwrap();
    let SidebarTarget::Context { target, .. } = &active else {
        unreachable!()
    };
    view.machines
        .observe_focus(Choice::Local, Some(target.clone()), None);
    assert!(view.machines.focus_sidebar_target(&active));
    let theme = Theme::terminal_native().quantize(crate::tui_theme::ColorLevel::Monochrome);
    let (buffer, _) = paint_navigation(&view, Locale::EnUs, &theme, 26, 10);
    assert_eq!(buffer[(0, 3)].symbol(), ">");
    assert_eq!(buffer[(1, 3)].symbol(), "*");
    assert!(!buffer[(6, 3)].modifier.contains(Modifier::BOLD));
}
