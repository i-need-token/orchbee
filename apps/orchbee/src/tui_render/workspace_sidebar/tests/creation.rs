//! The creation projection joins canonical rows only by exact Session and committed identity.
use super::*;
use crate::terminal_session::{WorkspaceCreationPhase, WorkspaceCreationPreview};

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
                let site = template.target.site_id.clone();
                let project = WorkbenchProjectId::parse(&id(70)).unwrap();
                let canonical: Vec<_> = (0..=count)
                    .map(|index| {
                        let mut row = template.clone();
                        row.target.location = NativeContextLocation::Workspace {
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
                        crate::machine_spaces::Observation::Ready(std::sync::Arc::new(
                            crate::machine_spaces::Checkpoint {
                                site_id: site.clone(),
                                revision: WorkbenchRevision::new(if committed { 2 } else { 1 }),
                                metadata_revision: WorkbenchRevision::new(2),
                                git_requested: true,
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
                        site: &site,
                        label: Some("new 工作区"),
                        project: grouped.then_some("Repo"),
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
                                    view: &view,
                                    locale,
                                    theme: &Theme::terminal_native(),
                                    layer: HitLayer::Chrome,
                                    sidebar_width: 50,
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
                let site = SiteId::parse(&id(if case == "other-session" { 2 } else { 1 })).unwrap();
                let workspace = WorkbenchWorkspaceId::parse(&id(80)).unwrap();
                let project = WorkbenchProjectId::parse(&id(70)).unwrap();
                let preview = WorkspaceCreationPreview {
                    site: &site,
                    label: Some("same 工作区"),
                    project: Some("Repo"),
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
                                view: &view,
                                locale,
                                theme: &theme,
                                layer: HitLayer::Chrome,
                                sidebar_width: 50,
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
                assert!(
                    text.starts_with(&format!(
                        "{} {}",
                        locale.shell().workspaces,
                        if extra { 3 } else { 2 }
                    )),
                    "{case}: {text}"
                );
                assert!(!text.contains("Repo / same 工作区"), "{case}");
            }
        }
    }
}

#[test]
fn canonical_handoff_then_higher_revision_removal_clears_the_row_and_collapsed_parent_marker() {
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
            let site = canonical[0].target.site_id.clone();
            let workspace = WorkbenchWorkspaceId::parse(&id(80)).unwrap();
            let project = WorkbenchProjectId::parse(&id(70)).unwrap();
            let parent = SidebarTarget::Project {
                machine: Choice::Local,
                site_id: site.clone(),
                project_id: project.clone(),
            };
            if collapsed {
                assert!(view.machines.toggle_sidebar_project(&parent));
            }
            let preview = WorkspaceCreationPreview {
                site: &site,
                label: Some("created workspace"),
                project: Some("Repo"),
                project_id: Some(&project),
                committed: Some((&workspace, WorkbenchRevision::new(2))),
                phase: WorkspaceCreationPhase::Committed,
                current: true,
            };
            // The same ID is first absent in the older checkpoint, then really
            // observed at its commit revision, then removed at a newer revision.
            for (revision, present) in [(1, false), (2, true), (3, false)] {
                view.machines.observe_spaces(vec![(
                    Choice::Local,
                    crate::machine_spaces::Observation::Ready(std::sync::Arc::new(
                        crate::machine_spaces::Checkpoint {
                            site_id: site.clone(),
                            revision: WorkbenchRevision::new(revision),
                            metadata_revision: WorkbenchRevision::new(revision),
                            git_requested: true,
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
                                view: &view,
                                locale,
                                theme: &theme,
                                layer: HitLayer::Chrome,
                                sidebar_width: 60,
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
                assert!(text.starts_with(&format!(
                    "{} {}",
                    locale.shell().workspaces,
                    if revision < 3 { 2 } else { 1 },
                )));
                assert_eq!(
                    text.matches("created workspace").count(),
                    usize::from(revision < 3 && !collapsed),
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
                assert_eq!(
                    current_bands.len(),
                    usize::from(revision < 3),
                    "revision {revision}, collapsed {collapsed}: {text}",
                );
                if let Some(y) = current_bands.first() {
                    let label: String = (0..buffer.area.width)
                        .map(|x| buffer[(x, *y)].symbol())
                        .collect();
                    let expected = if collapsed {
                        "Repo"
                    } else {
                        "created workspace"
                    };
                    assert!(label.trim_start().starts_with(expected), "{label}");
                }
            }
        }
    }
}
