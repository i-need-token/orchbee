//! The creation projection joins canonical rows only by exact Session and committed identity.
use super::*;
use crate::localization::WorkspaceCreationPhase;
use crate::workbench_state::workspace_setup::WorkspaceCreationPreview;

#[test]
fn creation_preview_keeps_current_fill_and_shows_only_its_name() {
    let session = SessionId::parse(&id(1)).unwrap();
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [
            Theme::terminal_native(),
            Theme::one_light(),
            Theme::one_dark(),
        ] {
            for current in [false, true] {
                for phase in [
                    WorkspaceCreationPhase::Preparing,
                    WorkspaceCreationPhase::Committed,
                ] {
                    let mut terminal = Terminal::new(TestBackend::new(64, 3)).unwrap();
                    terminal
                        .draw(|frame| {
                            render_creation(
                                frame,
                                Rect::new(0, 1, 64, 1),
                                64,
                                WorkspaceCreationPreview {
                                    session: &session,
                                    label: Some("Preview"),
                                    project_id: None,
                                    committed: None,
                                    phase,
                                    current,
                                },
                                locale,
                                &theme,
                            );
                        })
                        .unwrap();
                    let buffer = terminal.backend().buffer();
                    let surface = if current {
                        Role::SurfaceCurrent
                    } else {
                        Role::SurfaceChrome
                    };
                    let label_x = (0..64).find(|x| buffer[(*x, 1)].symbol() == "P").unwrap();
                    assert_eq!(buffer[(label_x, 1)].bg, theme.color(surface));
                    assert!(!buffer[(label_x, 1)].modifier.contains(Modifier::BOLD));
                    assert_eq!(buffer[(0, 1)].bg, theme.color(surface));
                    assert_eq!(buffer[(63, 1)].bg, theme.color(surface));
                    // Neither a pending glyph nor progress copy: the row shows its name alone.
                    let row: String = (0..64).map(|x| buffer[(x, 1)].symbol()).collect();
                    assert_eq!(row.trim(), "Preview", "{locale:?} {phase:?}");
                }
            }
        }
    }
}

fn rendered_text(buffer: &ratatui::buffer::Buffer) -> String {
    let mut text = String::new();
    for y in buffer.area.y..buffer.area.bottom() {
        let mut x = buffer.area.x;
        while x < buffer.area.right() {
            let symbol = buffer[(x, y)].symbol();
            text.push_str(symbol);
            x += display_width(symbol).max(1) as u16;
        }
        text.push('\n');
    }
    text
}

fn creation_member_view() -> (ClientView, SidebarTarget, [SidebarTarget; 2]) {
    let mut view = view(120, 40);
    configure(
        &mut view,
        serde_json::json!([[token("workspace")], [token("branch")]]),
        0,
    );
    let workspace = view.machines.space_rows()[0].sidebar_target().unwrap();
    let SidebarTarget::Context { target, .. } = &workspace else {
        unreachable!()
    };
    let agents: Vec<_> = [(23, "Alpha"), (24, "Beta")]
        .into_iter()
        .map(|(number, name)| {
            let mut agent = agent_for(target.clone(), number);
            agent.target.panel = WorkbenchPanelId::parse(&id(number + 10)).unwrap();
            agent.metadata.title = Some(MetadataTokenValue::parse(name).unwrap());
            agent
        })
        .collect();
    let members: [SidebarTarget; 2] = agents
        .iter()
        .map(|agent| SidebarTarget::Agent {
            machine: Choice::Local,
            target: Box::new(agent.target.clone()),
        })
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    view.machines.observe_agents(vec![(
        Choice::Local,
        crate::machine_agents::Observation::Ready(crate::machine_agents::test_support::checkpoint(
            agents,
        )),
    )]);
    expand_context_agents(&mut view, target.clone());
    (view, workspace, members)
}

fn admit_creation_current(view: &mut ClientView, target: &SidebarTarget) -> bool {
    let (context, member) = match target {
        SidebarTarget::Context { target, .. } => (target.clone(), None),
        SidebarTarget::Agent { target, .. } => (
            target.context.clone(),
            Some(crate::workbench_projection::ClientMemberTarget {
                tab: target.tab.clone(),
                panel: target.panel.clone(),
                presentation: target.member.clone(),
            }),
        ),
        SidebarTarget::Project { .. } | SidebarTarget::Recents { .. } => unreachable!(),
    };
    view.machines
        .observe_focus(Choice::Local, Some(context), member)
}

fn paint_creation_members(
    view: &ClientView,
    locale: Locale,
    theme: &Theme,
    creation: WorkspaceCreationPreview<'_>,
    height: u16,
) -> (ratatui::buffer::Buffer, crate::tui_hit_map::FrameHitMap) {
    let mut terminal = Terminal::new(TestBackend::new(64, height)).unwrap();
    let mut hits = HitMapBuilder::new();
    let mut candidate = None;
    let rows = view.machines.sidebar_navigation_rows();
    let source = view.machines.sidebar_source();
    terminal
        .draw(|frame| {
            render(
                frame,
                frame.area(),
                RenderContext {
                    held: None,
                    view,
                    locale,
                    theme,
                    layer: HitLayer::Chrome,
                    sidebar_width: 64,
                    trailing_padding: 0,
                    owns_keyboard: view.machines.sidebar_navigation_active(),
                    source: &source,
                    creation: Some(creation),
                },
                &rows,
                Some(&mut hits),
            );
            candidate = Some(frame.buffer_mut().clone());
        })
        .unwrap();
    (candidate.unwrap(), hits.finish())
}

fn assert_creation_member_surface(
    buffer: &ratatui::buffer::Buffer,
    hits: &crate::tui_hit_map::FrameHitMap,
    target: &SidebarTarget,
    theme: &Theme,
    surface: Role,
) -> Vec<u16> {
    let ys = sidebar_target_rows(hits, buffer.area.width, buffer.area.height, target);
    assert!(!ys.is_empty(), "missing creation member {target:?}");
    // An Agent surface includes leading padding; the guide and its gap stay on chrome.
    let first = match target {
        SidebarTarget::Agent { .. } => {
            layout::Tracks::resolve(
                Rect::new(0, 0, buffer.area.width, 1),
                0,
                layout::AuxiliaryActions::default(),
                layout::RowKind::Agent,
            )
            .unwrap()
            .surface()
            .x
        }
        _ => 0,
    };
    for y in &ys {
        for x in 0..buffer.area.width {
            if x == 0 && first > 0 {
                continue;
            }
            let expected = if x < first {
                Role::SurfaceChrome
            } else {
                surface
            };
            assert_eq!(
                buffer[(x, *y)].bg,
                theme.color(expected),
                "{target:?}: {expected:?} at {x},{y}"
            );
        }
    }
    ys
}

#[test]
fn foreground_creation_yields_to_the_admitted_member_after_commit() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [
            Theme::terminal_native(),
            Theme::one_light(),
            Theme::one_dark(),
        ] {
            let (mut view, workspace, [alpha, beta]) = creation_member_view();
            admit_creation_current(&mut view, &beta);
            let SidebarTarget::Context { target, .. } = &workspace else {
                unreachable!()
            };
            let ClientContextLocation::Workspace { workspace_id, .. } = &target.location else {
                unreachable!()
            };
            let project = WorkbenchProjectId::parse(&id(70)).unwrap();
            for (phase, current) in [
                (WorkspaceCreationPhase::Preparing, true),
                (WorkspaceCreationPhase::Starting, true),
                (WorkspaceCreationPhase::Committed, true),
                (WorkspaceCreationPhase::Starting, false),
            ] {
                let pending_current = phase != WorkspaceCreationPhase::Committed && current;
                let provisional = phase == WorkspaceCreationPhase::Preparing;
                let (buffer, hits) = paint_creation_members(
                    &view,
                    locale,
                    &theme,
                    WorkspaceCreationPreview {
                        session: &target.session_id,
                        label: Some("Creation preview"),
                        project_id: Some(&project),
                        committed: (!provisional)
                            .then_some((workspace_id, WorkbenchRevision::new(1))),
                        phase,
                        current,
                    },
                    20,
                );
                let owner_rows = assert_creation_member_surface(
                    &buffer,
                    &hits,
                    &workspace,
                    &theme,
                    if pending_current && provisional {
                        Role::SurfaceChrome
                    } else {
                        Role::SurfaceCurrent
                    },
                );
                assert_creation_member_surface(&buffer, &hits, &alpha, &theme, Role::SurfaceChrome);
                assert_creation_member_surface(
                    &buffer,
                    &hits,
                    &beta,
                    &theme,
                    if pending_current {
                        Role::SurfaceChrome
                    } else {
                        Role::SurfaceCurrent
                    },
                );
                let y = owner_rows[0];
                let tracks = layout::Tracks::resolve(
                    Rect::new(0, y, 64, 1),
                    0,
                    auxiliary_actions(&view.machines.sidebar_source(), &workspace),
                    layout::RowKind::for_target(&workspace),
                )
                .unwrap();
                let identity_x = (tracks.primary.x..tracks.primary.right())
                    .find(|x| buffer[(*x, y)].symbol() != " ")
                    .unwrap();
                assert!(!buffer[(identity_x, y)].modifier.contains(Modifier::BOLD));
                if provisional {
                    let y = rendered_text(&buffer)
                        .lines()
                        .position(|line| line.contains("Creation preview"))
                        .unwrap() as u16;
                    for x in 0..buffer.area.width {
                        assert_eq!(buffer[(x, y)].bg, theme.color(Role::SurfaceCurrent));
                    }
                }
                // Fills alone tie the Workspace to its current Agent; no row adds a text marker.
                assert!(
                    (0..buffer.area.width)
                        .all(|x| buffer[(x, y)].symbol() != theme.glyphs.active_marker)
                );
                assert!(!admit_creation_current(&mut view, &beta));
                assert!(view.machines.request.is_none());
            }
        }
    }
}

#[test]
fn retained_committed_preview_follows_member_switches_and_collapsed_ancestors() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [
            Theme::terminal_native(),
            Theme::one_light(),
            Theme::one_dark(),
        ] {
            let (mut view, workspace, [alpha, beta]) = creation_member_view();
            let other = view.machines.space_rows()[1].sidebar_target().unwrap();
            let SidebarTarget::Context { target, .. } = &workspace else {
                unreachable!()
            };
            let ClientContextLocation::Workspace { workspace_id, .. } = &target.location else {
                unreachable!()
            };
            let project = WorkbenchProjectId::parse(&id(70)).unwrap();
            // Keep even the old workspace-current bit while changing admitted focus. A retained
            // receipt cannot pin either the current fill or its viewport to the created Workspace.
            let preview = WorkspaceCreationPreview {
                session: &target.session_id,
                label: Some("same 工作区"),
                project_id: Some(&project),
                committed: Some((workspace_id, WorkbenchRevision::new(1))),
                phase: WorkspaceCreationPhase::Committed,
                current: true,
            };
            for current in [&beta, &alpha, &other, &beta] {
                admit_creation_current(&mut view, current);
                let (buffer, hits) = paint_creation_members(&view, locale, &theme, preview, 20);
                for row in [&workspace, &alpha, &beta, &other] {
                    assert_creation_member_surface(
                        &buffer,
                        &hits,
                        row,
                        &theme,
                        if row == current || row == &workspace && current != &other {
                            Role::SurfaceCurrent
                        } else {
                            Role::SurfaceChrome
                        },
                    );
                }
                let (narrow_viewport, hits) =
                    paint_creation_members(&view, locale, &theme, preview, 6);
                assert_creation_member_surface(
                    &narrow_viewport,
                    &hits,
                    current,
                    &theme,
                    Role::SurfaceCurrent,
                );
            }
            // Provisional sidebar navigation is separate from an admitted member switch.
            assert!(view.machines.focus_sidebar_target(&alpha));
            let (buffer, hits) = paint_creation_members(&view, locale, &theme, preview, 20);
            assert_creation_member_surface(&buffer, &hits, &alpha, &theme, Role::SurfaceSelection);
            assert_creation_member_surface(&buffer, &hits, &beta, &theme, Role::SurfaceCurrent);
            assert!(!admit_creation_current(&mut view, &beta));
            view.machines.leave_sidebar_navigation();
            for collapsed in [true, false] {
                assert!(view.machines.toggle_sidebar_context_agents(&workspace));
                let (buffer, hits) = paint_creation_members(&view, locale, &theme, preview, 20);
                assert_creation_member_surface(
                    &buffer,
                    &hits,
                    &workspace,
                    &theme,
                    Role::SurfaceCurrent,
                );
                if collapsed {
                    assert!(sidebar_target_rows(&hits, 64, 20, &alpha).is_empty());
                    assert!(sidebar_target_rows(&hits, 64, 20, &beta).is_empty());
                } else {
                    assert_creation_member_surface(
                        &buffer,
                        &hits,
                        &beta,
                        &theme,
                        Role::SurfaceCurrent,
                    );
                }
            }
            assert!(!view.machines.sidebar_navigation_active());
            assert!(view.machines.request.is_none());
        }
    }
}

#[test]
fn a_background_project_creation_above_manual_reading_keeps_the_same_painted_rows_through_handoff()
{
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let mut view = view(120, 40);
        configure(&mut view, serde_json::json!([[token("workspace")]]), 0);
        let template = view
            .machines
            .space_checkpoint_row(&view.machines.space_rows()[0])
            .unwrap()
            .clone();
        let session = template.target.session_id.clone();
        let project = WorkbenchProjectId::parse(&id(70)).unwrap();
        let old_rows: Vec<_> = (0..20)
            .map(|index| {
                let mut row = template.clone();
                row.target.location = ClientContextLocation::Workspace {
                    workspace_id: WorkbenchWorkspaceId::parse(&id(100 + index)).unwrap(),
                    terminal_context_id: WorkbenchTerminalContextId::parse(&id(200 + index))
                        .unwrap(),
                };
                row.project_id = (index < 2).then_some(project.clone());
                row.label = Some(format!("old-{index:02}"));
                row
            })
            .collect();
        let workspace = WorkbenchWorkspaceId::parse(&id(130)).unwrap();
        let mut positions = Vec::new();
        for phase in 0..3 {
            let mut rows = old_rows.clone();
            if phase == 2 {
                let mut created = template.clone();
                created.target.location = ClientContextLocation::Workspace {
                    workspace_id: workspace.clone(),
                    terminal_context_id: WorkbenchTerminalContextId::parse(&id(230)).unwrap(),
                };
                created.label = Some("created workspace".into());
                rows.insert(2, created);
            }
            view.machines.observe_spaces(vec![(
                Choice::Local,
                crate::machine_spaces::checkpoint::Observation::Ready(std::sync::Arc::new(
                    crate::machine_spaces::Checkpoint {
                        session_id: session.clone(),
                        revision: WorkbenchRevision::new(if phase == 2 { 2 } else { 1 }),
                        metadata_revision: WorkbenchRevision::new(2),
                        git_requested: true,
                        known_git: Vec::new().into(),
                        projects: vec![crate::machine_spaces::ProjectSummary {
                            project_id: project.clone(),
                            label: "Repo".into(),
                        }]
                        .into(),
                        rows: rows.into(),
                    },
                )),
            )]);
            if phase == 0 {
                let rows = view.machines.sidebar_navigation_rows();
                let window = view.machines.space_window_for_row_gaps(
                    &rows,
                    &vec![1; rows.len()],
                    &vec![0; rows.len()],
                    7,
                );
                view.machines.queue_space_scroll(window, false);
                view.machines.settle_sidebar_scroll();
            }
            let preview = (phase > 0).then_some(WorkspaceCreationPreview {
                session: &session,
                label: Some("created workspace"),
                project_id: Some(&project),
                committed: (phase == 2).then_some((&workspace, WorkbenchRevision::new(2))),
                phase: if phase == 2 {
                    WorkspaceCreationPhase::Committed
                } else {
                    WorkspaceCreationPhase::Preparing
                },
                current: false,
            });
            let source = view.machines.sidebar_source();
            let rows = view.machines.sidebar_navigation_rows();
            let mut terminal = Terminal::new(TestBackend::new(50, 10)).unwrap();
            terminal
                .draw(|frame| {
                    render(
                        frame,
                        frame.area(),
                        RenderContext {
                            held: None,
                            view: &view,
                            locale,
                            theme: &Theme::terminal_native(),
                            layer: HitLayer::Chrome,
                            sidebar_width: 50,
                            trailing_padding: 0,
                            owns_keyboard: false,
                            source: &source,
                            creation: preview,
                        },
                        &rows,
                        None,
                    )
                })
                .unwrap();
            let text = rendered_text(terminal.backend().buffer());
            positions.push(
                text.lines()
                    .enumerate()
                    .filter(|(_, line)| line.contains("old-"))
                    .map(|(y, line)| (y, line.to_owned()))
                    .collect::<Vec<_>>(),
            );
        }
        assert!(!positions[0].is_empty());
        assert_eq!(positions[0], positions[1]);
        assert_eq!(positions[1], positions[2]);
        assert!(view.machines.request.is_none());
    }
}

#[test]
fn creation_keeps_its_final_group_order_and_scroll_window_through_observer_handoff() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for grouped in [false, true] {
            for count in [2, 20] {
                let mut view = view(120, 40);
                configure(&mut view, serde_json::json!([[token("workspace")]]), 0);
                let template = view
                    .machines
                    .space_checkpoint_row(&view.machines.space_rows()[0])
                    .unwrap()
                    .clone();
                let session = template.target.session_id.clone();
                let project = WorkbenchProjectId::parse(&id(70)).unwrap();
                let canonical: Vec<_> = (0..=count)
                    .map(|index| {
                        let mut row = template.clone();
                        row.target.location = ClientContextLocation::Workspace {
                            workspace_id: WorkbenchWorkspaceId::parse(&id(100 + index)).unwrap(),
                            terminal_context_id: WorkbenchTerminalContextId::parse(
                                &id(200 + index),
                            )
                            .unwrap(),
                        };
                        row.project_id = grouped.then_some(project.clone());
                        row.label = Some(if index == count {
                            "new 工作区".into()
                        } else {
                            format!("old-{index:02}")
                        });
                        row
                    })
                    .collect();
                let workspace = WorkbenchWorkspaceId::parse(&id(100 + count)).unwrap();
                let mut positions = Vec::new();
                for phase in [
                    WorkspaceCreationPhase::Preparing,
                    WorkspaceCreationPhase::Starting,
                    WorkspaceCreationPhase::Committed,
                ] {
                    let committed = phase == WorkspaceCreationPhase::Committed;
                    view.machines.observe_spaces(vec![(
                        Choice::Local,
                        crate::machine_spaces::checkpoint::Observation::Ready(std::sync::Arc::new(
                            crate::machine_spaces::Checkpoint {
                                session_id: session.clone(),
                                revision: WorkbenchRevision::new(if committed { 2 } else { 1 }),
                                metadata_revision: WorkbenchRevision::new(2),
                                git_requested: true,
                                known_git: Vec::new().into(),
                                projects: if grouped {
                                    vec![crate::machine_spaces::ProjectSummary {
                                        project_id: project.clone(),
                                        label: "Repo".into(),
                                    }]
                                } else {
                                    Vec::new()
                                }
                                .into(),
                                rows: canonical
                                    [..usize::try_from(count).unwrap() + usize::from(committed)]
                                    .to_vec()
                                    .into(),
                            },
                        )),
                    )]);
                    let source = view.machines.sidebar_source();
                    let rows = view.machines.sidebar_navigation_rows();
                    let preview = WorkspaceCreationPreview {
                        session: &session,
                        label: Some("new 工作区"),
                        project_id: grouped.then_some(&project),
                        committed: committed.then_some((&workspace, WorkbenchRevision::new(2))),
                        phase,
                        current: true,
                    };
                    let mut terminal = Terminal::new(TestBackend::new(50, 10)).unwrap();
                    terminal
                        .draw(|frame| {
                            render(
                                frame,
                                frame.area(),
                                RenderContext {
                                    held: None,
                                    view: &view,
                                    locale,
                                    theme: &Theme::terminal_native(),
                                    layer: HitLayer::Chrome,
                                    sidebar_width: 50,
                                    trailing_padding: 0,
                                    owns_keyboard: false,
                                    source: &source,
                                    creation: Some(preview),
                                },
                                &rows,
                                None,
                            )
                        })
                        .unwrap();
                    let text = rendered_text(terminal.backend().buffer());
                    let location = text
                        .lines()
                        .enumerate()
                        .filter_map(|(y, line)| {
                            if line.contains("new 工作区") {
                                Some(("new".to_owned(), y))
                            } else {
                                (0..count).find_map(|index| {
                                    line.contains(&format!("old-{index:02}"))
                                        .then(|| (format!("old-{index:02}"), y))
                                })
                            }
                        })
                        .collect::<Vec<_>>();
                    assert!(location.iter().any(|(label, _)| label == "new"), "{text}");
                    positions.push(location);
                }
                assert_eq!(positions[0], positions[1]);
                assert_eq!(positions[1], positions[2]);
            }
        }
    }
}

#[test]
fn provisional_rows_survive_observer_lag_and_reconcile_without_label_deduplication_or_ghosts() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [Theme::terminal_native(), Theme::rgb_test_dark()] {
            for case in ["provisional", "observed", "other-session"] {
                let view = view(120, 40);
                let session =
                    SessionId::parse(&id(if case == "other-session" { 2 } else { 1 })).unwrap();
                let workspace = WorkbenchWorkspaceId::parse(&id(80)).unwrap();
                let project = WorkbenchProjectId::parse(&id(70)).unwrap();
                let preview = WorkspaceCreationPreview {
                    session: &session,
                    label: Some("same 工作区"),
                    project_id: Some(&project),
                    committed: (case != "provisional")
                        .then_some((&workspace, WorkbenchRevision::new(1))),
                    phase: WorkspaceCreationPhase::Starting,
                    current: true,
                };
                let source = view.machines.sidebar_source();
                let rows = view.machines.sidebar_navigation_rows();
                let mut terminal = Terminal::new(TestBackend::new(50, 18)).unwrap();
                terminal
                    .draw(|frame| {
                        render(
                            frame,
                            frame.area(),
                            RenderContext {
                                held: None,
                                view: &view,
                                locale,
                                theme: &theme,
                                layer: HitLayer::Chrome,
                                sidebar_width: 50,
                                trailing_padding: 0,
                                owns_keyboard: false,
                                source: &source,
                                creation: Some(preview),
                            },
                            &rows,
                            None,
                        );
                    })
                    .unwrap();
                let text = rendered_text(terminal.backend().buffer());
                let extra = matches!(case, "provisional" | "other-session");
                assert_eq!(
                    text.matches("same 工作区").count(),
                    if extra { 3 } else { 2 },
                    "{case}"
                );
                // The list opens with its Project heading; there is no counted list heading.
                assert!(text.trim_start().starts_with("Repo"), "{case}: {text}");
                assert!(!text.contains("Repo / same 工作区"), "{case}");
            }
        }
    }
}

#[test]
fn canonical_handoff_then_higher_revision_removal_clears_the_row_and_never_fills_a_folded_parent() {
    for collapsed in [false, true] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let theme = Theme::terminal_native();
            let mut view = view(120, 40);
            let mut canonical: Vec<_> = view
                .machines
                .space_rows()
                .iter()
                .map(|row| view.machines.space_checkpoint_row(row).unwrap().clone())
                .collect();
            canonical[0].label = Some("created workspace".into());
            let session = canonical[0].target.session_id.clone();
            let workspace = WorkbenchWorkspaceId::parse(&id(80)).unwrap();
            let project = WorkbenchProjectId::parse(&id(70)).unwrap();
            let parent = SidebarTarget::Project {
                machine: Choice::Local,
                session_id: session.clone(),
                project_id: project.clone(),
            };
            if collapsed {
                assert!(view.machines.toggle_sidebar_group(&parent));
            }
            let preview = WorkspaceCreationPreview {
                session: &session,
                label: Some("created workspace"),
                project_id: Some(&project),
                committed: Some((&workspace, WorkbenchRevision::new(3))),
                phase: WorkspaceCreationPhase::Committed,
                current: true,
            };
            // The same ID is first absent in the older checkpoint, then really
            // observed at its commit revision, then removed at a newer revision.
            for (revision, present) in [(2, false), (3, true), (4, false)] {
                view.machines.observe_spaces(vec![(
                    Choice::Local,
                    crate::machine_spaces::checkpoint::Observation::Ready(std::sync::Arc::new(
                        crate::machine_spaces::Checkpoint {
                            session_id: session.clone(),
                            revision: WorkbenchRevision::new(revision),
                            metadata_revision: WorkbenchRevision::new(revision),
                            git_requested: true,
                            known_git: Vec::new().into(),
                            projects: vec![crate::machine_spaces::ProjectSummary {
                                project_id: project.clone(),
                                label: "Repo".into(),
                            }]
                            .into(),
                            rows: canonical[if present { 0 } else { 1 }..].to_vec().into(),
                        },
                    )),
                )]);
                let source = view.machines.sidebar_source();
                let rows = view.machines.sidebar_navigation_rows();
                let mut terminal = Terminal::new(TestBackend::new(60, 18)).unwrap();
                terminal
                    .draw(|frame| {
                        render(
                            frame,
                            frame.area(),
                            RenderContext {
                                held: None,
                                view: &view,
                                locale,
                                theme: &theme,
                                layer: HitLayer::Chrome,
                                sidebar_width: 60,
                                trailing_padding: 0,
                                owns_keyboard: false,
                                source: &source,
                                creation: Some(preview),
                            },
                            &rows,
                            None,
                        );
                    })
                    .unwrap();
                let text = rendered_text(terminal.backend().buffer());
                assert!(
                    text.trim_start().starts_with("Repo"),
                    "revision {revision}, collapsed {collapsed}: {text}"
                );
                assert_eq!(
                    text.matches("created workspace").count(),
                    usize::from(revision < 4 && !collapsed),
                    "revision {revision}, collapsed {collapsed}: {text}",
                );
                let buffer = terminal.backend().buffer();
                let current_bands: Vec<_> = (buffer.area.y..buffer.area.bottom())
                    .filter(|y| {
                        buffer[(0, *y)].bg == theme.color(Role::SurfaceCurrent)
                            && (*y == buffer.area.y
                                || buffer[(0, *y - 1)].bg != theme.color(Role::SurfaceCurrent))
                    })
                    .collect();
                // A Project heading is never current, folded or not; only the row fills.
                assert_eq!(
                    current_bands.len(),
                    usize::from(revision < 4 && !collapsed),
                    "revision {revision}, collapsed {collapsed}: {text}",
                );
                if let Some(y) = current_bands.first() {
                    let label: String = (0..buffer.area.width)
                        .map(|x| buffer[(x, *y)].symbol())
                        .collect();
                    assert!(
                        label.trim_start().starts_with("created workspace"),
                        "present {present}: {label}"
                    );
                }
            }
        }
    }
}

#[test]
fn creation_and_canonical_workspace_share_configured_metadata_tracks() {
    let (view, _, _) = creation_member_view();
    assert_eq!(configured_height(&view), 2);
    for row in view.machines.space_rows() {
        let projected = lines(
            &view,
            row,
            Locale::EnUs,
            &Theme::terminal_native(),
            48,
            LineShape::default(),
        );
        assert_eq!(projected.lines.len(), usize::from(configured_height(&view)));
    }
}
