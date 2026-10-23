use super::*;
use crate::{
    client_view::Modifiers,
    terminal_input::{
        MouseAction, MouseButton, MouseEvent, TerminalInputPort, TerminalInputUnavailable,
    },
    tui_pointer::{MotionObservability, PointerOutcome, PointerState},
};
use std::time::Instant;

struct NoInput;
impl TerminalInputPort for NoInput {
    fn forward(&mut self, _: &[u8]) -> Result<(), TerminalInputUnavailable> {
        panic!("sidebar interaction leaked terminal input")
    }
}

fn event(column: u16, row: u16, action: MouseAction) -> MouseEvent {
    MouseEvent {
        column,
        row,
        action,
        button: if action == MouseAction::Motion {
            MouseButton::None
        } else {
            MouseButton::Left
        },
        mods: Modifiers::NONE,
    }
}

/// Row controls are pointer-revealed: hover the row's first line, then paint the frame that
/// publishes its disclosure and Create controls.
fn revealed_frame(
    view: &mut ClientView,
    pointer: &mut PointerState,
    (locale, theme): (Locale, &Theme),
    (width, height): (u16, u16),
    target: &SidebarTarget,
) -> (ratatui::buffer::Buffer, crate::tui_pointer::FrameHitMap) {
    let (_, _, map) = paint_navigation_buffers_with_pointer(
        view,
        locale,
        theme,
        width,
        height,
        pointer.projection(),
    );
    assert!(
        (0..height).all(|y| (0..width).all(
            |x| map.hit_target_at(x, y) != Some(&HitTarget::SidebarDisclosure(target.clone()))
        )),
        "a resting row publishes no disclosure"
    );
    let y = sidebar_target_rows(&map, width, height, target)
        .into_iter()
        .find(|y| {
            (0..width).any(|x| {
                matches!(
                    map.hit_target_at(x, *y),
                    Some(
                        HitTarget::MachineProjectRow { .. } | HitTarget::MachineWorkspaceRow { .. }
                    )
                )
            })
        })
        .unwrap();
    pointer.publish(map);
    let _ = pointer.event(
        view,
        &mut NoInput,
        Instant::now(),
        event(2, y, MouseAction::Motion),
    );
    let (candidate, _, map) = paint_navigation_buffers_with_pointer(
        view,
        locale,
        theme,
        width,
        height,
        pointer.projection(),
    );
    (candidate, map)
}

fn active_view() -> ClientView {
    let mut view = view(120, 40);
    let SidebarTarget::Context {
        target: context, ..
    } = view.machines.space_rows()[0].sidebar_target().unwrap()
    else {
        unreachable!()
    };
    view.machines
        .observe_focus(Choice::Local, Some(context), None);
    view
}

#[test]
fn row_and_child_hover_share_the_whole_row_surface_without_an_overflow_slot() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for width in [24, 30, 60] {
            let mut view = active_view();
            let target = view
                .machines
                .sidebar_parent(&view.machines.space_rows()[0].sidebar_target().unwrap())
                .unwrap();
            let theme = Theme::one_light();
            let mut pointer = PointerState::default();
            pointer.set_capture(true);
            pointer.set_motion_observability(MotionObservability::AnyMotion);
            let (_, map) = revealed_frame(
                &mut view,
                &mut pointer,
                (locale, &theme),
                (width, 12),
                &target,
            );
            let y = sidebar_target_rows(&map, width, 12, &target)[0];
            let disclosure_x = (0..width)
                .find(|x| {
                    map.hit_target_at(*x, y) == Some(&HitTarget::SidebarDisclosure(target.clone()))
                })
                .unwrap();
            assert_eq!(
                map.hit_target_at(width - 1, y),
                Some(&HitTarget::CreateMenu(Some(target.clone())))
            );
            pointer.publish(map);
            let before = view.machines.sidebar_focused_target().cloned();
            for x in [2, disclosure_x, width - 2] {
                let _ = pointer.event(
                    &mut view,
                    &mut NoInput,
                    Instant::now(),
                    event(x, y, MouseAction::Motion),
                );
                for _ in 0..3 {
                    let (candidate, _, map) = paint_navigation_buffers_with_pointer(
                        &view,
                        locale,
                        &theme,
                        width,
                        12,
                        pointer.projection(),
                    );
                    assert!(
                        (0..width)
                            .all(|column| candidate[(column, y)].bg
                                == theme.color(Role::SurfaceHover))
                    );
                    assert!((0..12).all(|row| (0..width).all(|column| !matches!(
                        map.hit_target_at(column, row),
                        Some(HitTarget::SidebarMenu(_))
                    ))));
                    assert_eq!(
                        map.hit_target_at(width - 1, y),
                        Some(&HitTarget::CreateMenu(Some(target.clone())))
                    );
                    pointer.publish(map);
                }
            }
            let _ = pointer.event(
                &mut view,
                &mut NoInput,
                Instant::now(),
                event(disclosure_x, y, MouseAction::Motion),
            );
            let _ = pointer.event(
                &mut view,
                &mut NoInput,
                Instant::now(),
                event(disclosure_x, y, MouseAction::Press),
            );
            for _ in 0..3 {
                let (candidate, _, map) = paint_navigation_buffers_with_pointer(
                    &view,
                    locale,
                    &theme,
                    width,
                    12,
                    pointer.projection(),
                );
                for x in 0..width {
                    assert_eq!(
                        candidate[(x, y)].bg,
                        theme.color(if x == disclosure_x {
                            Role::SurfacePressed
                        } else {
                            Role::SurfaceHover
                        })
                    );
                }
                pointer.publish(map);
            }
            let outcome = pointer.event(
                &mut view,
                &mut NoInput,
                Instant::now(),
                event(disclosure_x, y, MouseAction::Release),
            );
            assert!(
                matches!(outcome, PointerOutcome::Interaction { target: InteractionTarget::Sidebar(ref drawn), contract: PrimaryContract::ActivateOnRelease } if drawn.target() == &target && drawn.part() == SidebarPart::Disclosure)
            );
            let _ = pointer.event(
                &mut view,
                &mut NoInput,
                Instant::now(),
                event(width + 1, y, MouseAction::Motion),
            );
            let (candidate, _, _) = paint_navigation_buffers_with_pointer(
                &view,
                locale,
                &theme,
                width,
                12,
                pointer.projection(),
            );
            assert!((0..width).all(|x| candidate[(x, y)].bg == theme.color(Role::SurfaceChrome)));
            assert_eq!(view.machines.sidebar_focused_target(), before.as_ref());
            assert!(!view.machines.sidebar_navigation_active());
            assert!(view.machines.request.is_none());
        }
    }
}

fn project_gap_view(label: &str) -> ClientView {
    let mut view = active_view();
    let rows = view
        .machines
        .space_rows()
        .iter()
        .map(|row| view.machines.space_checkpoint_row(row).unwrap().clone())
        .collect::<Vec<_>>();
    view.machines.observe_spaces(vec![(
        Choice::Local,
        crate::machine_spaces::Observation::Ready(std::sync::Arc::new(
            crate::machine_spaces::Checkpoint {
                site_id: SiteId::parse(&id(1)).unwrap(),
                revision: WorkbenchRevision::new(3),
                metadata_revision: WorkbenchRevision::new(2),
                git_requested: true,
                rows: rows.into(),
                projects: vec![
                    crate::machine_spaces::ProjectSummary {
                        project_id: WorkbenchProjectId::parse(&id(70)).unwrap(),
                        label: "First group".into(),
                    },
                    crate::machine_spaces::ProjectSummary {
                        project_id: WorkbenchProjectId::parse(&id(71)).unwrap(),
                        label: label.into(),
                    },
                ]
                .into(),
            },
        )),
    )]);
    view
}

#[test]
fn project_group_gap_stays_inert_while_hover_targets_only_the_inline_disclosure_cell() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [Theme::one_light(), Theme::one_dark()] {
            for width in [24, 30, 60] {
                for label in ["Orchbee", "项目😀e\u{301} long identity that must truncate"] {
                    let mut view = project_gap_view(label);
                    configure(
                        &mut view,
                        serde_json::json!([[token("workspace")], [token("branch")]]),
                        2,
                    );
                    let target = SidebarTarget::Project {
                        machine: Choice::Local,
                        site_id: SiteId::parse(&id(1)).unwrap(),
                        project_id: WorkbenchProjectId::parse(&id(71)).unwrap(),
                    };
                    let mut pointer = PointerState::default();
                    pointer.set_capture(true);
                    pointer.set_motion_observability(MotionObservability::AnyMotion);
                    let (_, map) = revealed_frame(
                        &mut view,
                        &mut pointer,
                        (locale, &theme),
                        (width, 24),
                        &target,
                    );
                    let project_y = sidebar_target_rows(&map, width, 24, &target)[0];
                    let last_workspace_y = (0..project_y)
                        .filter(|y| {
                            matches!(
                                map.hit_target_at(0, *y),
                                Some(HitTarget::MachineWorkspaceRow { .. })
                            )
                        })
                        .max()
                        .unwrap();
                    assert_eq!(
                        project_y - last_workspace_y,
                        3,
                        "two external group-gap rows"
                    );
                    let tracks = layout::Tracks::resolve(
                        Rect::new(0, project_y, width, 1),
                        0,
                        auxiliary_actions(&view.machines.sidebar_source(), &target),
                        layout::RowKind::Group,
                    )
                    .unwrap();
                    let count =
                        view.machines
                            .sidebar_navigation_rows()
                            .iter()
                            .find_map(|row| match row {
                                crate::machine_picker::SidebarRow::Project {
                                    workspaces, ..
                                } if row.target() == target => Some(format!(" {workspaces}")),
                                _ => None,
                            })
                            .unwrap();
                    // Only the name truncates; the member count stays whole before the disclosure.
                    let expected_label = truncate_display(
                        label,
                        usize::from(tracks.text_width(true)).saturating_sub(display_width(&count)),
                        theme.glyphs.ellipsis,
                    ) + &count;
                    let disclosure_x = tracks.primary.x + display_width(&expected_label) as u16 + 1;
                    let disclosure_cells = (0..24)
                        .flat_map(|y| (0..width).map(move |x| (x, y)))
                        .filter(|&(x, y)| {
                            map.hit_target_at(x, y)
                                == Some(&HitTarget::SidebarDisclosure(target.clone()))
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(disclosure_cells, [(disclosure_x, project_y)]);
                    pointer.publish(map);
                    for (x, y, on_row) in [
                        (2, project_y, true),
                        (disclosure_x, project_y, true),
                        (disclosure_x, project_y - 1, false),
                        (2, project_y - 2, false),
                    ] {
                        let _ = pointer.event(
                            &mut view,
                            &mut NoInput,
                            Instant::now(),
                            event(x, y, MouseAction::Motion),
                        );
                        for _ in 0..3 {
                            let (candidate, _, map) = paint_navigation_buffers_with_pointer(
                                &view,
                                locale,
                                &theme,
                                width,
                                24,
                                pointer.projection(),
                            );
                            assert!((0..24).all(|row| (0..width).all(|column| !matches!(
                                map.hit_target_at(column, row),
                                Some(HitTarget::SidebarMenu(_))
                            ))));
                            assert!((0..width).all(|column| candidate[(column, project_y)].bg
                                == theme.color(if on_row {
                                    Role::SurfaceHover
                                } else {
                                    Role::SurfaceChrome
                                })));
                            // The expanded Project's disclosure is shown only while the row is hovered.
                            assert_eq!(
                                candidate[(disclosure_x, project_y)].symbol(),
                                if on_row {
                                    theme.glyphs.disclosure_open
                                } else {
                                    " "
                                }
                            );
                            for gap_y in last_workspace_y + 1..project_y {
                                for gap_x in 0..width {
                                    assert_eq!(candidate[(gap_x, gap_y)].symbol(), " ");
                                    assert_eq!(
                                        candidate[(gap_x, gap_y)].bg,
                                        theme.color(Role::SurfaceChrome)
                                    );
                                    assert!(matches!(
                                        map.hit_target_at(gap_x, gap_y),
                                        Some(HitTarget::MachineWorkspaceSurface(_))
                                    ));
                                }
                            }
                            if x == disclosure_x && y == project_y {
                                assert_eq!(candidate[(x, y)].bg, theme.color(Role::SurfaceHover));
                            }
                            pointer.publish(map);
                        }
                    }
                    let before = view.machines.sidebar_focused_target().cloned();
                    for action in [MouseAction::Press, MouseAction::Release] {
                        let _ = pointer.event(
                            &mut view,
                            &mut NoInput,
                            Instant::now(),
                            event(disclosure_x, project_y - 1, action),
                        );
                    }
                    assert_eq!(view.machines.sidebar_focused_target(), before.as_ref());
                    assert!(!view.machines.sidebar_navigation_active());
                    assert!(view.machines.request.is_none());
                    assert!(view.machines.sidebar_project_expanded(&target));
                }
            }
        }
    }
}

#[test]
fn multirow_workspace_disclosure_has_no_hit_on_separator_or_continuations() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for width in [24, 30, 60] {
            let mut view = active_view();
            configure(
                &mut view,
                serde_json::json!([[token("workspace")], [token("branch")]]),
                2,
            );
            // A Project's first Workspace has no leading gap; the second follows a separator.
            let target = view.machines.space_rows()[1].sidebar_target().unwrap();
            let SidebarTarget::Context {
                target: context, ..
            } = &target
            else {
                unreachable!()
            };
            view.machines.observe_agents(vec![(
                Choice::Local,
                crate::machine_agents::Observation::Ready(
                    crate::machine_agents::tests::checkpoint(vec![agent_for(context.clone(), 23)]),
                ),
            )]);
            let theme = Theme::one_light();
            let mut pointer = PointerState::default();
            pointer.set_capture(true);
            pointer.set_motion_observability(MotionObservability::AnyMotion);
            let (buffer, map) = revealed_frame(
                &mut view,
                &mut pointer,
                (locale, &theme),
                (width, 24),
                &target,
            );
            let rows = sidebar_target_rows(&map, width, 24, &target);
            let cells = rows
                .iter()
                .flat_map(|y| (0..width).map(move |x| (x, *y)))
                .filter(|&(x, y)| {
                    map.hit_target_at(x, y) == Some(&HitTarget::SidebarDisclosure(target.clone()))
                })
                .collect::<Vec<_>>();
            assert_eq!(
                cells.len(),
                1,
                "one painted disclosure, even on a tall Workspace row"
            );
            let (x, y) = cells[0];
            assert_eq!(buffer[(x, y)].symbol(), theme.glyphs.disclosure_open);
            assert_eq!(
                rows.first(),
                Some(&y),
                "the entry starts at its identity line"
            );
            assert!(
                matches!(
                    map.hit_target_at(x, y - 1),
                    Some(HitTarget::MachineWorkspaceSurface(_))
                ),
                "the preceding separator is inert list surface"
            );
            assert!(
                rows.iter().any(|row| *row > y),
                "configured continuation is present"
            );
            for row in rows.into_iter().filter(|row| *row != y) {
                assert!(matches!(
                    map.hit_target_at(x, row),
                    Some(HitTarget::MachineWorkspaceRow { .. })
                ));
            }
        }
    }
}
