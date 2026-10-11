//! Composed render and decoded pointer traces through the real viewport owner.
use super::*;
use crate::{
    localization::Locale,
    terminal_session::TerminalSessionView,
    tui_pointer::{HitMapBuilder, HitTarget},
    tui_theme::{Role, Theme},
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

fn painted_map(ui: &NativeUi, key: &PaneRef, content: Rect) -> crate::tui_pointer::FrameHitMap {
    let mut hits = HitMapBuilder::new();
    hits.record(
        crate::tui_pointer::HitLayer::Foreground,
        content,
        HitTarget::Pane {
            pane: key.clone(),
            content,
        },
    );
    hits.record(
        crate::tui_pointer::HitLayer::Foreground,
        Rect::new(content.right(), content.y, 1, content.height),
        HitTarget::Scrollbar {
            pane: key.clone(),
            content,
            source: ui.view().pane_scrollbar(key.pane().panel()).unwrap(),
        },
    );
    hits.finish()
}

fn fill_frame(ui: &mut NativeUi, key: &PaneRef, content: Rect) {
    let projection = projection_at(ui, key);
    let mut live = frame(&projection.value, 1000, false, content.height);
    live.geometry.columns = content.width;
    live.geometry.rows = content.height;
    for row in &mut live.rows {
        row.cells = vec![row.cells[0].clone(); usize::from(content.width)];
    }
    projection.value.install_frame_for_test(live);
    projection.status = NativeProjectionStatus::FrameAvailable;
}

fn serve_history(ui: &mut NativeUi, mailbox: &Arc<CommandMailbox>, key: &PaneRef) {
    let projection = projection_at(ui, key);
    let live = projection.value.frame().unwrap().clone();
    let mut changed = facts(&projection.value, 1, 1);
    changed.total_rows = live.scrollback_rows + u64::from(live.geometry.rows);
    projection.viewport.as_mut().unwrap().facts(changed);
    ui.admit_viewport_steps(mailbox, key);
    let steps = recv_viewports(mailbox);
    let [
        ViewportStep::Read {
            request_sequence,
            origin: ViewportRowOrigin::Tail { rows_before },
            ..
        },
    ] = steps.as_slice()
    else {
        panic!("expected one bounded tail read: {steps:?}");
    };
    let mut served = window(&projection_at(ui, key).value, request_sequence.get(), 1, 0);
    served.route.total_rows = changed.total_rows;
    served.route.start_row = live.scrollback_rows - u64::from(*rows_before);
    served.route.columns = live.geometry.columns;
    served.route.row_count = live.geometry.rows;
    served.rows = live.rows;
    ui.apply_viewport_outcome(
        mailbox,
        key.clone(),
        ViewportOutcome::Window {
            request_sequence: *request_sequence,
            window: Box::new(served),
        },
    );
    assert!(matches!(
        recv_viewports(mailbox).as_slice(),
        [ViewportStep::Ack { .. }]
    ));
    ui.apply_viewport_outcome(
        mailbox,
        key.clone(),
        ViewportOutcome::Ack {
            request_sequence: *request_sequence,
            accepted: true,
        },
    );
}

fn paint(
    ui: &NativeUi,
    view: &ClientView,
    width: u16,
    height: u16,
    locale: Locale,
    theme: &Theme,
) -> (ratatui::buffer::Buffer, crate::tui_pointer::FrameHitMap) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut hits = HitMapBuilder::new();
    terminal
        .draw(|frame| {
            crate::tui_render::render_terminal_shell(
                frame,
                view,
                locale,
                Some(TerminalSessionView::Native(ui.view())),
                theme,
                &mut hits,
            );
        })
        .unwrap();
    (terminal.backend().buffer().clone(), hits.finish())
}

#[test]
fn scrollbar_composed_render_preserves_terminal_cells_geometry_and_localized_chrome() {
    for (width, height) in [(80, 24), (120, 40), (70, 16), (40, 8), (19, 4)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for theme in [Theme::terminal_native(), Theme::rgb_test_dark()] {
                let (mut ui, _mailbox, _, _) = ready_ui();
                let key = install_projection(&mut ui, Some((1000, false, 24)), 0);
                let view = ClientView::new(width);
                let area = Rect::new(0, 0, width, height);
                let areas = crate::tui_layout::project_shell_areas(area, view.sidebar_surface());
                let content = crate::tui_layout::native_terminal_area(
                    areas.panel,
                    areas.tier.shows_panel_frame(),
                    false,
                );
                fill_frame(&mut ui, &key, content);
                let (buffer, map) = paint(&ui, &view, width, height, locale, &theme);
                let mut count = 0;
                for y in 0..height {
                    for x in 0..width {
                        if let Some(HitTarget::Scrollbar {
                            pane,
                            content: painted,
                            ..
                        }) = map.hit_target_at(x, y)
                        {
                            assert_eq!(pane, &key);
                            assert_eq!(*painted, content);
                            assert_eq!(x, content.right());
                            count += 1;
                        }
                    }
                }
                if areas.tier == crate::tui_layout::ViewportTier::TooSmall {
                    assert_eq!(count, 0);
                    continue;
                }
                assert_eq!(count, usize::from(content.height));
                let gutter: String = (content.y..content.bottom())
                    .map(|y| buffer[(content.right(), y)].symbol())
                    .collect();
                assert_eq!(
                    gutter,
                    format!(
                        "{}{}",
                        theme
                            .glyphs
                            .scrollbar_track
                            .repeat(usize::from(content.height - 1)),
                        theme.glyphs.scrollbar_thumb
                    )
                );
                for y in content.y..content.bottom() {
                    let menu =
                        map.hit_target_at(content.right() - 1, y) == Some(&HitTarget::MenuButton);
                    if menu {
                        assert_eq!(areas.tier, crate::tui_layout::ViewportTier::Minimal);
                        assert_eq!(y, content.y);
                    }
                    assert_eq!(
                        buffer[(content.right() - 1, y)].symbol(),
                        if menu { "]" } else { "x" },
                        "only the Minimal Actions overlay may cover terminal cells"
                    );
                }
                assert_eq!(
                    buffer[(content.right(), content.bottom() - 1)].symbol(),
                    theme.glyphs.scrollbar_thumb
                );
                assert_eq!(
                    buffer[(content.right(), content.bottom() - 1)].fg,
                    theme
                        .style(Role::AccentFocus)
                        .fg
                        .unwrap_or(ratatui::style::Color::Reset)
                );
                assert_eq!(
                    buffer[(content.right(), content.bottom() - 1)].bg,
                    theme
                        .style(Role::SurfaceCanvas)
                        .bg
                        .unwrap_or(ratatui::style::Color::Reset)
                );
                assert_eq!(
                    buffer[(content.right(), content.y)].symbol(),
                    theme.glyphs.scrollbar_track
                );
            }
        }
    }
}

#[test]
#[cfg(windows)]
fn scrollbar_thumb_focus_follows_the_shell_input_owner() {
    let (mut ui, _, _, _) = ready_ui();
    let key = install_projection(&mut ui, Some((1000, false, 24)), 0);
    let mut view = ClientView::new(80);
    let areas =
        crate::tui_layout::project_shell_areas(Rect::new(0, 0, 80, 24), view.sidebar_surface());
    let content = crate::tui_layout::native_terminal_area(areas.panel, true, false);
    fill_frame(&mut ui, &key, content);
    view.machines.available = true;
    assert!(view.machines.enter_sidebar_navigation());
    let theme = Theme::terminal_native();
    let (buffer, _) = paint(&ui, &view, 80, 24, Locale::EnUs, &theme);
    assert_eq!(
        buffer[(content.right(), content.bottom() - 1)].fg,
        theme
            .style(Role::TextSecondary)
            .fg
            .unwrap_or(ratatui::style::Color::Reset)
    );
}

#[test]
fn scrollbar_split_and_zoom_keep_each_thumb_on_its_exact_pane_and_outside_dividers() {
    for zoom in [false, true] {
        for history in [false, true] {
            for locale in [Locale::EnUs, Locale::ZhCn] {
                let (mut ui, mailbox, clock, _) = ready_split_ui();
                super::super::super::tests::publication_hint::attach_split_peer(
                    &mut ui, &mailbox, &clock,
                );
                drain(&mailbox);
                if zoom {
                    ui.apply_client_action(ClientAction::ToggleZoom);
                    drain(&mailbox);
                }
                let view = ClientView::new(120);
                let areas = crate::tui_layout::project_shell_areas(
                    Rect::new(0, 0, 120, 40),
                    view.sidebar_surface(),
                );
                let native = ui.view();
                let tab = native.tab(0).unwrap();
                let focus = ui.state.as_ref().unwrap().focus_pane_ref().unwrap();
                let root = crate::tui_layout::native_terminal_area(areas.panel, true, !zoom);
                let panes: Vec<_> = if zoom {
                    vec![(focus.clone(), root)]
                } else {
                    crate::tui_layout::project_panel_tree(
                        root,
                        tab.panel_tree(),
                        tab.active_panel(),
                        &|split, first, second| tab.split_ratio(split, first, second),
                    )
                    .leaves
                    .into_iter()
                    .map(|leaf| {
                        (
                            native
                                .pane_ref_for_panel(tab.panel(leaf.panel_index).unwrap().panel_id())
                                .unwrap(),
                            leaf.content,
                        )
                    })
                    .collect()
                };
                for (key, content) in &panes {
                    fill_frame(&mut ui, key, *content);
                }
                if history {
                    let source = ui.view().pane_scrollbar(focus.pane().panel()).unwrap();
                    assert!(ui.scrollbar_scroll(&focus, source, 500));
                    assert_eq!(recv_viewports(&mailbox), [ViewportStep::Enter]);
                    serve_history(&mut ui, &mailbox, &focus);
                }
                let theme = Theme::terminal_native();
                let (buffer, map) = paint(&ui, &view, 120, 40, locale, &theme);
                let mut actual = 0;
                for y in 0..40 {
                    for x in 0..120 {
                        if let Some(HitTarget::Scrollbar { pane, content, .. }) =
                            map.hit_target_at(x, y)
                        {
                            assert!(panes.contains(&(pane.clone(), *content)));
                            assert_eq!(x, content.right());
                            actual += 1;
                        }
                    }
                }
                assert_eq!(
                    actual,
                    panes
                        .iter()
                        .map(|(_, content)| usize::from(content.height))
                        .sum::<usize>()
                );
                for (key, content) in &panes {
                    let source = ui.view().pane_scrollbar(key.pane().panel()).unwrap();
                    let thumb = source
                        .thumb(Rect::new(content.right(), content.y, 1, content.height))
                        .unwrap();
                    assert_eq!(
                        buffer[(thumb.x, thumb.y)].symbol(),
                        theme.glyphs.scrollbar_thumb
                    );
                    assert_eq!(buffer[(content.right() - 1, content.y + 1)].symbol(), "x");
                    let expected_role = if *key == focus {
                        Role::AccentFocus
                    } else {
                        Role::TextSecondary
                    };
                    assert_eq!(
                        buffer[(thumb.x, thumb.y)].fg,
                        theme
                            .style(expected_role)
                            .fg
                            .unwrap_or(ratatui::style::Color::Reset)
                    );
                    if history && *key == focus {
                        assert!(thumb.y > content.y && thumb.bottom() < content.bottom());
                    } else {
                        assert_eq!(thumb.bottom(), content.bottom());
                    }
                }
            }
        }
    }
}

#[test]
fn scrollbar_decoded_drag_to_zero_runs_recovery_before_live_without_forwarding_mouse() {
    let (mut ui, mailbox, _, _) = controlling_ui();
    let key = install_projection(&mut ui, Some((1000, false, 24)), 0);
    let content = Rect::new(0, 0, 80, 24);
    let mut view = ClientView::new(120);
    let mut dispatcher = InputDispatcher::default();
    dispatcher.set_pointer_capture(&mut view, &mut NativePort(&mut ui), true);
    dispatcher.publish_hit_map(painted_map(&ui, &key, content));
    dispatch(&mut dispatcher, &mut view, &mut ui, b"\x1b[<0;81;24M");
    assert!(recv_wire(&mailbox).is_empty());
    dispatch(&mut dispatcher, &mut view, &mut ui, b"\x1b[<32;81;12M");
    assert_eq!(recv_viewports(&mailbox), [ViewportStep::Enter]);
    serve_history(&mut ui, &mailbox, &key);
    dispatcher.publish_hit_map(painted_map(&ui, &key, content));
    dispatch(
        &mut dispatcher,
        &mut view,
        &mut ui,
        b"\x1b[<32;81;24M\x1b[<0;81;24m",
    );
    assert_eq!(recv_wire(&mailbox), [WireCmd::Recover(key.clone())]);
    assert!(
        !projection_at(&mut ui, &key)
            .viewport
            .as_mut()
            .unwrap()
            .frame_applied()
            .live
    );
    ui.apply_viewport_outcome(&mailbox, key.clone(), ViewportOutcome::Recovered);
    let effect = projection_at(&mut ui, &key)
        .viewport
        .as_mut()
        .unwrap()
        .frame_applied();
    assert!(effect.live);
    ui.settle_viewport_effect(&mailbox, &key, effect);
    assert!(projection_at(&mut ui, &key).viewport.is_none());
    assert!(recv_wire(&mailbox).is_empty());
}

#[test]
fn scrollbar_native_owner_validates_identity_known_range_and_live_entry() {
    let (mut ui, mailbox, _, _) = controlling_ui();
    let key = install_projection(&mut ui, Some((1000, false, 24)), 0);
    let source = ui.view().pane_scrollbar(key.pane().panel()).unwrap();
    let wrong = PaneRef::new(key.pane().clone(), key.incarnation() + 1);
    assert!(!ui.scrollbar_scroll(&wrong, source, 500));
    assert!(mailbox.try_recv().unwrap().is_none());
    assert!(ui.scrollbar_scroll(&key, source, 500));
    assert_eq!(recv_viewports(&mailbox), [ViewportStep::Enter]);
    assert!(
        ui.view().pane_scrollbar(key.pane().panel()).is_none(),
        "loading never invents a thumb"
    );
    assert!(ui.scrollbar_scroll(&key, source, 900));
    assert!(
        recv_viewports(&mailbox).is_empty(),
        "facts still own read admission"
    );
    let mut changed = facts(&projection_at(&mut ui, &key).value, 1, 1);
    changed.total_rows = source.total_rows();
    projection_at(&mut ui, &key)
        .viewport
        .as_mut()
        .unwrap()
        .facts(changed);
    ui.admit_viewport_steps(&mailbox, &key);
    assert!(matches!(
        recv_viewports(&mailbox).as_slice(),
        [ViewportStep::Read {
            origin: ViewportRowOrigin::Tail { rows_before: 900 },
            ..
        }]
    ));
    assert!(ui.scrollbar_scroll(&key, source, 0));
    assert_eq!(recv_viewports(&mailbox), [ViewportStep::Recover]);
    assert!(!ui.scrollbar_scroll(&key, source, 100));
    let effect = projection_at(&mut ui, &key)
        .viewport
        .as_mut()
        .unwrap()
        .frame_applied();
    assert!(
        !effect.live,
        "a frame preceding recovery demand completion cannot resume Live"
    );
    ui.apply_viewport_outcome(&mailbox, key.clone(), ViewportOutcome::Recovered);
    assert!(projection_at(&mut ui, &key).viewport.is_some());
    let effect = projection_at(&mut ui, &key)
        .viewport
        .as_mut()
        .unwrap()
        .frame_applied();
    assert!(effect.live);
    ui.settle_viewport_effect(&mailbox, &key, effect);
    assert!(projection_at(&mut ui, &key).viewport.is_none());
    assert!(ui.view().pane_scrollbar(key.pane().panel()).is_some());
}

#[test]
fn scrollbar_live_output_range_changes_cancel_old_intent_without_moving_a_committed_anchor() {
    let (mut ui, mailbox, _, _) = controlling_ui();
    let key = install_projection(&mut ui, Some((376, false, 24)), 0);
    let source = ui.view().pane_scrollbar(key.pane().panel()).unwrap();
    let projection = projection_at(&mut ui, &key);
    projection
        .value
        .install_frame_for_test(frame(&projection.value, 400, false, 24));
    assert!(
        !ui.scrollbar_scroll(&key, source, 100),
        "growth before history entry cancels the old painted range"
    );
    assert!(recv_viewports(&mailbox).is_empty());
    // Enter with a new painted observation, then pin an acknowledged history window.
    let projection = projection_at(&mut ui, &key);
    projection
        .value
        .install_frame_for_test(frame(&projection.value, 376, false, 24));
    assert!(ui.scrollbar_scroll(&key, source, 100));
    assert_eq!(recv_viewports(&mailbox), [ViewportStep::Enter]);
    settle_viewing(&mut ui, &mailbox, &key, 1);
    let history = ui.view().pane_scrollbar(key.pane().panel()).unwrap();
    let mut changed = facts(&projection_at(&mut ui, &key).value, 2, 1);
    changed.total_rows = 500;
    projection_at(&mut ui, &key)
        .viewport
        .as_mut()
        .unwrap()
        .facts(changed);
    assert!(
        !ui.scrollbar_scroll(&key, history, 200),
        "growth during capture cancels before the next window paints"
    );
    ui.admit_viewport_steps(&mailbox, &key);
    assert!(
        matches!(
            recv_viewports(&mailbox).as_slice(),
            [ViewportStep::Read {
                origin: ViewportRowOrigin::Committed { delta: 0 },
                ..
            }]
        ),
        "live output only refreshes the committed anchor"
    );
}

#[test]
fn scrollbar_unknown_alternate_and_stale_sources_cannot_submit_history() {
    for spec in [
        None,
        Some((0, false, 24)),
        Some((100, true, 24)),
        Some((100, false, 0)),
    ] {
        let (mut ui, _, _, _) = ready_ui();
        let key = install_projection(&mut ui, spec, 0);
        assert!(ui.view().pane_scrollbar(key.pane().panel()).is_none());
    }
    let (mut ui, mailbox, _, _) = ready_ui();
    let key = install_projection(&mut ui, Some((100, false, 24)), 0);
    let source = ui.view().pane_scrollbar(key.pane().panel()).unwrap();
    let projection = projection_at(&mut ui, &key);
    let mut changed = frame(&projection.value, 200, false, 24);
    changed.route.generation = NonZeroU64::new(2).unwrap();
    projection.value.install_frame_for_test(changed);
    assert!(!ui.scrollbar_scroll(&key, source, 40));
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn scrollbar_decoded_drag_is_cancelled_by_host_focus_loss_and_keyboard_input() {
    for cancellation in [b"\x1b[O".as_slice(), b"z".as_slice()] {
        let (mut ui, mailbox, _, _) = controlling_ui();
        let key = install_projection(&mut ui, Some((1000, false, 24)), 0);
        let mut view = ClientView::new(120);
        let content = Rect::new(0, 0, 80, 24);
        let mut dispatcher = InputDispatcher::default();
        dispatcher.set_pointer_capture(&mut view, &mut NativePort(&mut ui), true);
        dispatcher.publish_hit_map(painted_map(&ui, &key, content));
        dispatch(&mut dispatcher, &mut view, &mut ui, b"\x1b[<0;81;24M");
        assert!(recv_wire(&mailbox).is_empty());
        dispatch(&mut dispatcher, &mut view, &mut ui, cancellation);
        let before = recv_wire(&mailbox);
        assert!(
            before
                .iter()
                .all(|command| matches!(command, WireCmd::Input(_, bytes) if bytes == b"z"))
        );
        dispatch(
            &mut dispatcher,
            &mut view,
            &mut ui,
            b"\x1b[<32;81;1M\x1b[<0;81;1m",
        );
        assert!(recv_wire(&mailbox).is_empty());
    }
}
