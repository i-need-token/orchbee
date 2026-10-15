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

fn has_menu(
    hits: &crate::tui_pointer::FrameHitMap,
    width: u16,
    y: u16,
    target: &SidebarTarget,
) -> bool {
    (0..width).any(|x| matches!(hits.hit_target_at(x,y), Some(HitTarget::SidebarMenu(actual)) if actual == target))
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
fn project_group_gap_stays_inert_while_hover_targets_only_the_inline_disclosure_cell() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [Theme::one_light(), Theme::one_dark()] {
            for width in [24, 30, 60] {
                for label in ["Orchbee", "项目😀e\u{301} long identity that must truncate"] {
                    let mut view = ungrouped_view(label);
                    configure(
                        &mut view,
                        serde_json::json!([[token("workspace")], [token("branch")]]),
                        2,
                    );
                    let target = view
                        .machines
                        .sidebar_navigation_rows()
                        .last()
                        .unwrap()
                        .target();
                    let mut pointer = PointerState::default();
                    pointer.set_capture(true);
                    pointer.set_motion_observability(MotionObservability::AnyMotion);
                    let (_, _, map) = paint_navigation_buffers_with_pointer(
                        &view,
                        locale,
                        &theme,
                        width,
                        24,
                        pointer.projection(),
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
                        0,
                        auxiliary_actions(&view.machines.sidebar_source(), &target),
                        layout::RowKind::Owner,
                    )
                    .unwrap();
                    let expected_label = truncate_display(
                        label,
                        usize::from(tracks.text_width(true)),
                        theme.glyphs.ellipsis,
                    );
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
                    for (x, y, menu_visible) in [
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
                            assert_eq!(has_menu(&map, width, project_y, &target), menu_visible);
                            assert_eq!(
                                candidate[(disclosure_x, project_y)].symbol(),
                                theme.glyphs.disclosure_open
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
fn multirow_workspace_disclosure_has_no_hit_on_padding_or_continuations() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for width in [24, 30, 60] {
            let mut view = active_view();
            configure(
                &mut view,
                serde_json::json!([[token("workspace")], [token("branch")]]),
                2,
            );
            let target = view.machines.space_rows()[0].sidebar_target().unwrap();
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
            let (buffer, map) = paint_navigation(&view, locale, &theme, width, 24);
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
            assert!(
                rows.iter().any(|row| *row < y),
                "configured leading padding is present"
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

#[test]
fn row_and_child_hover_reveal_menu_without_repaint_flicker_or_focus_changes() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for width in [26, 40, 60] {
            let mut view = active_view();
            let target = view
                .machines
                .sidebar_parent(&view.machines.space_rows()[0].sidebar_target().unwrap())
                .unwrap();
            let theme = Theme::one_light();
            let mut pointer = PointerState::default();
            pointer.set_capture(true);
            pointer.set_motion_observability(MotionObservability::AnyMotion);
            let (_, _, hits) = paint_navigation_buffers_with_pointer(
                &view,
                locale,
                &theme,
                width,
                12,
                pointer.projection(),
            );
            let y = sidebar_target_rows(&hits, width, 12, &target)[0];
            assert!(!has_menu(&hits, width, y, &target));
            let disclosure_x = (0..width)
                .find(|x| {
                    hits.hit_target_at(*x, y) == Some(&HitTarget::SidebarDisclosure(target.clone()))
                })
                .expect("painted disclosure after the Project identity");
            pointer.publish(hits);
            let before = view.machines.sidebar_focused_target().cloned();
            for x in [2, disclosure_x, width - 2] {
                assert!(matches!(
                    pointer.event(
                        &mut view,
                        &mut NoInput,
                        Instant::now(),
                        event(x, y, MouseAction::Motion)
                    ),
                    PointerOutcome::IdleMotion { .. }
                ));
                for _ in 0..4 {
                    let (_, _, hits) = paint_navigation_buffers_with_pointer(
                        &view,
                        locale,
                        &theme,
                        width,
                        12,
                        pointer.projection(),
                    );
                    assert!(
                        has_menu(&hits, width, y, &target),
                        "{locale:?} width={width} x={x}"
                    );
                    pointer.publish(hits);
                }
            }
            assert_eq!(view.machines.sidebar_focused_target(), before.as_ref());
            assert!(!view.machines.sidebar_navigation_active());
            assert!(view.machines.request.is_none());
            let _ = pointer.event(
                &mut view,
                &mut NoInput,
                Instant::now(),
                event(width + 1, y, MouseAction::Motion),
            );
            let (_, _, hits) = paint_navigation_buffers_with_pointer(
                &view,
                locale,
                &theme,
                width,
                12,
                pointer.projection(),
            );
            assert!(
                !has_menu(&hits, width, y, &target),
                "leaving the row hides its menu"
            );
            pointer.publish(hits);
            let _ = pointer.event(
                &mut view,
                &mut NoInput,
                Instant::now(),
                event(2, y, MouseAction::Motion),
            );
            let (_, _, hits) = paint_navigation_buffers_with_pointer(
                &view,
                locale,
                &theme,
                width,
                12,
                pointer.projection(),
            );
            assert!(has_menu(&hits, width, y, &target));
            pointer.publish(hits);
            pointer.set_host_focus(false);
            let (_, _, hits) = paint_navigation_buffers_with_pointer(
                &view,
                locale,
                &theme,
                width,
                12,
                pointer.projection(),
            );
            assert!(!has_menu(&hits, width, y, &target));
            pointer.set_host_focus(true);
            pointer.publish(hits);
            let (_, _, hits) = paint_navigation_buffers_with_pointer(
                &view,
                locale,
                &theme,
                width,
                12,
                pointer.projection(),
            );
            assert!(
                !has_menu(&hits, width, y, &target),
                "focus restoration cannot synthesize hover"
            );
        }
    }
}

#[test]
fn an_occluding_owner_suppresses_row_hover_reveal() {
    let mut view = active_view();
    let target = view
        .machines
        .sidebar_parent(&view.machines.space_rows()[0].sidebar_target().unwrap())
        .unwrap();
    let theme = Theme::one_light();
    let mut pointer = PointerState::default();
    pointer.set_capture(true);
    pointer.set_motion_observability(MotionObservability::AnyMotion);
    let (_, _, map) = paint_navigation_buffers_with_pointer(
        &view,
        Locale::EnUs,
        &theme,
        40,
        12,
        pointer.projection(),
    );
    let y = sidebar_target_rows(&map, 40, 12, &target)[0];
    pointer.publish(map);
    let _ = pointer.event(
        &mut view,
        &mut NoInput,
        Instant::now(),
        event(2, y, MouseAction::Motion),
    );
    let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
    let mut hits = HitMapBuilder::with_pointer(pointer.projection());
    hits.record(
        HitLayer::Status,
        Rect::new(2, y, 1, 1),
        HitTarget::SettingsButton,
    );
    let rows = view.machines.sidebar_navigation_rows();
    let source = view.machines.sidebar_source();
    terminal
        .draw(|frame| {
            render(
                frame,
                frame.area(),
                RenderContext {
                    view: &view,
                    locale: Locale::EnUs,
                    theme: &theme,
                    layer: HitLayer::Chrome,
                    sidebar_width: 40,
                    owns_keyboard: false,
                    source: &source,
                },
                &rows,
                Some(&mut hits),
            )
        })
        .unwrap();
    let hits = hits.finish();
    assert!(!has_menu(&hits, 40, y, &target));
    assert!((37..40).all(|x| terminal.backend().buffer()[(x, y)].symbol() == " "));
}

#[test]
fn revealing_a_menu_never_changes_an_armed_rows_exposure() {
    for hovered in [false, true] {
        let mut view = active_view();
        let target = view
            .machines
            .sidebar_parent(&view.machines.space_rows()[0].sidebar_target().unwrap())
            .unwrap();
        let theme = Theme::one_light();
        let mut pointer = PointerState::default();
        pointer.set_capture(true);
        pointer.set_motion_observability(MotionObservability::AnyMotion);
        let (_, _, hits) = paint_navigation_buffers_with_pointer(
            &view,
            Locale::EnUs,
            &theme,
            40,
            12,
            pointer.projection(),
        );
        let y = sidebar_target_rows(&hits, 40, 12, &target)[0];
        pointer.publish(hits);
        if hovered {
            let _ = pointer.event(
                &mut view,
                &mut NoInput,
                Instant::now(),
                event(2, y, MouseAction::Motion),
            );
            let (_, _, hits) = paint_navigation_buffers_with_pointer(
                &view,
                Locale::EnUs,
                &theme,
                40,
                12,
                pointer.projection(),
            );
            assert!(has_menu(&hits, 40, y, &target));
            pointer.publish(hits);
        }
        assert_eq!(
            pointer.event(
                &mut view,
                &mut NoInput,
                Instant::now(),
                event(2, y, MouseAction::Press)
            ),
            PointerOutcome::Handled
        );
        for _ in 0..3 {
            let (_, _, hits) = paint_navigation_buffers_with_pointer(
                &view,
                Locale::EnUs,
                &theme,
                40,
                12,
                pointer.projection(),
            );
            assert_eq!(has_menu(&hits, 40, y, &target), hovered);
            pointer.publish(hits);
        }
        assert!(matches!(
            pointer.event(
                &mut view,
                &mut NoInput,
                Instant::now(),
                event(2, y, MouseAction::Release)
            ),
            PointerOutcome::Interaction {
                target: InteractionTarget::Sidebar(_),
                contract: PrimaryContract::ActivateOnRelease
            }
        ));
    }
}
