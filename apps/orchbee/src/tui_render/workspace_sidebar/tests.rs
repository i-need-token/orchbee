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
            target: NativeContextTarget {
                site_id: site.clone(),
                location: NativeContextLocation::Workspace {
                    project_id: WorkbenchProjectId::parse(&id(70)).unwrap(),
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
                branch: Some("worktree/测试".into()),
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
            assert!(text.contains("worktree/测试"));
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
            assert!(
                (0..40).any(|y| matches!(
                    hits.hit_target_at(2, y),
                    Some(HitTarget::OpenMachineAgents)
                ))
            );
        }
    }
}
#[test]
fn workspace_raw_value_rules_and_empty_layouts_collapse_without_borrowing_custom_git() {
    let mut view = view(120, 40);
    let mut branch = token("branch");
    branch["rules"] = serde_json::json!([{"condition":{"kind":"Equals","value":"worktree/测试"},"ignore_case":false,"style":{"fg":null,"bold":null,"dim":null},"hide":true}]);
    configure(
        &mut view,
        serde_json::json!([[branch], [token("$branch")]]),
        0,
    );
    let (rows, hits) = paint(&view, Locale::EnUs, &Theme::terminal_native(), 120, 40);
    assert!(!rows.join("\n").contains("worktree/测试"));
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
    terminal
        .draw(|frame| {
            render(
                frame,
                Rect::new(0, 0, width, height),
                view,
                locale,
                theme,
                HitLayer::Chrome,
                Some(&mut hits),
            );
        })
        .unwrap();
    (terminal.backend().buffer().clone(), hits.finish())
}

#[test]
fn project_disclosure_rows_and_menus_have_disjoint_hits_and_focus_is_not_active_context() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [Theme::rgb_test_dark(), Theme::one_light()] {
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
                let project = active.parent().unwrap();
                let (buffer, hits) = paint_navigation(&view, locale, &theme, width, 10);
                let menu_start = width - menu_width(&view, &project, &theme);
                for x in 0..3 {
                    assert_eq!(
                        hits.hit_target_at(x, 1),
                        Some(&HitTarget::SidebarDisclosure(project.clone()))
                    );
                }
                for x in 3..menu_start {
                    assert!(matches!(
                        hits.hit_target_at(x, 1),
                        Some(HitTarget::MachineProjectRow { target, .. }) if target == &project
                    ));
                }
                for x in menu_start..width {
                    assert_eq!(
                        hits.hit_target_at(x, 1),
                        Some(&HitTarget::SidebarMenu(project.clone()))
                    );
                    assert_eq!(
                        hits.hit_target_at(x, 2),
                        Some(&HitTarget::SidebarMenu(active.clone()))
                    );
                    assert_eq!(
                        hits.hit_target_at(x, 4),
                        Some(&HitTarget::SidebarMenu(focused.clone()))
                    );
                }
                assert_eq!(buffer[(2, 2)].symbol(), theme.glyphs.active_marker);
                for y in [4, 5] {
                    assert_eq!(buffer[(0, y)].bg, theme.color(Role::SurfaceSelection));
                    for x in 0..menu_start {
                        assert!(matches!(
                            hits.hit_target_at(x, y),
                            Some(HitTarget::MachineWorkspaceRow { selected, .. })
                                if selected == &view.machines.space_rows()[1].selection()
                        ));
                    }
                }
                assert_ne!(buffer[(0, 2)].bg, buffer[(0, 4)].bg);
                assert!(view.machines.focus_sidebar_target(&project));
                assert!(view.machines.toggle_sidebar_project(&project));
                let (_, collapsed) = paint_navigation(&view, locale, &theme, width, 10);
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
        let annotations = annotations(&navigation, locale);
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
                    (2..16)
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
