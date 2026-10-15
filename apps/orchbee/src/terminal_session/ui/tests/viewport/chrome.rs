//! Composed terminal chrome preserves the final content row across prefix capture.
use super::*;
use crate::{
    client_view::{PrefixInput, SessionActionDisposition},
    localization::Locale,
    terminal_session::TerminalSessionView,
    tui_layout::project_shell_areas,
    tui_pointer::{HitMapBuilder, HitTarget},
    tui_theme::Theme,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

#[test]
fn creation_admission_retires_old_map_before_mouse_events_in_the_same_or_next_packet() {
    use crate::terminal_input::{
        PointerSelectionAdmission, PointerSelectionCell, PointerSelectionIntent,
    };

    for same_packet in [true, false] {
        for history in [false, true] {
            let (mut ui, mailbox, _, _) = controlling_ui();
            let key = install_projection(&mut ui, Some((1000, false, 24)), 0);
            if history {
                ui.enter_history();
                assert_eq!(recv_viewports(&mailbox), [ViewportStep::Enter]);
                settle_viewing(&mut ui, &mailbox, &key, 1);
                recv_viewports(&mailbox);
            }
            let source = ui.view().pane_scrollbar(key.pane().panel()).unwrap();
            let content = Rect::new(0, 0, 80, 24);
            let mut view = ClientView::new(120);
            let mut dispatcher = InputDispatcher::default();
            dispatcher.set_pointer_capture(&mut view, &mut NativePort(&mut ui), true);
            let mut hits = HitMapBuilder::new();
            hits.record(
                crate::tui_pointer::HitLayer::Foreground,
                content,
                HitTarget::Pane {
                    pane: key.clone(),
                    content,
                },
            );
            dispatcher.publish_hit_map(hits.finish());
            assert!(ui.pointer_probe().selection_ready);
            // Ctrl+B a admits creation. No preview is drawn before this old-map
            // click, drag, release and wheel sequence is decoded.
            let mouse = b"\x1b[<0;2;2M\x1b[<32;3;3M\x1b[<0;3;3m\x1b[<64;2;2M";
            if same_packet {
                let mut packet = b"\x02a".to_vec();
                packet.extend_from_slice(mouse);
                dispatch(&mut dispatcher, &mut view, &mut ui, &packet);
            } else {
                dispatch(&mut dispatcher, &mut view, &mut ui, b"\x02a");
                dispatch(&mut dispatcher, &mut view, &mut ui, mouse);
            }
            assert!(ui.workspace_creation_replaces_content());
            let probe = ui.pointer_probe();
            assert!(!probe.input_captured, "sidebar navigation stays available");
            assert!(!probe.selection_ready && !probe.input_ready);
            assert!(probe.focus.is_none() && probe.panes.is_empty() && probe.frame.is_none());
            assert_eq!(
                ui.pointer_selection(
                    &key,
                    PointerSelectionIntent::Start {
                        click_count: 1,
                        cell: PointerSelectionCell { column: 1, row: 1 },
                    },
                ),
                PointerSelectionAdmission::Rejected,
            );
            assert!(!ui.history_wheel(&key, true));
            assert!(!ui.history_key(&KeyEvent::press(KeyCode::PageUp, Modifiers::NONE)));
            assert!(!ui.scrollbar_scroll(&key, source, 0));
            let lease = mailbox.try_recv().unwrap().unwrap();
            assert!(matches!(lease.record(), QueuedCommand::Refresh(_)));
            mailbox.finish_lease(lease).unwrap();
            assert!(
                mailbox.try_recv().unwrap().is_none(),
                "no hidden-pane commands"
            );
            // Retiring a gesture must still be able to clean up its old owner.
            assert_eq!(
                ui.pointer_selection(&key, PointerSelectionIntent::Cancel),
                PointerSelectionAdmission::Admitted,
            );
            mailbox.close(CarrierFailure::WorkerExited);
        }
    }
}

#[test]
fn prefix_overlays_and_restores_the_last_terminal_row_without_resizing() {
    for (width, height) in [(120, 40), (80, 24), (60, 12), (50, 10)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for theme in [Theme::terminal_native(), Theme::one_light()] {
                let (mut ui, _, _, _) = controlling_ui();
                let mut view = ClientView::new(width);
                view.machines.available = true;
                let area = Rect::new(0, 0, width, height);
                let areas = project_shell_areas(area, view.sidebar_surface());
                let pane = crate::tui_shell::active_pane_area(
                    &areas,
                    Some(TerminalSessionView::Native(ui.view())),
                )
                .unwrap();
                assert_eq!(
                    pane.bottom(),
                    height,
                    "ordinary content reaches the host bottom"
                );
                let key = install_projection(&mut ui, None, 0);
                let projection = projection_at(&mut ui, &key);
                let mut content = frame(&projection.value, 0, false, pane.height);
                content.geometry.columns = pane.width;
                content.geometry.rows = pane.height;
                content.cursor.row = pane.height - 1;
                projection.value.install_frame_for_test(content);
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                let paint = |terminal: &mut Terminal<TestBackend>, view: &ClientView| {
                    let mut hits = HitMapBuilder::new();
                    terminal
                        .draw(|frame| {
                            crate::tui_render::render_terminal_shell(
                                frame,
                                view,
                                locale,
                                Some(TerminalSessionView::Native(ui.view())),
                                &theme,
                                &mut hits,
                            );
                        })
                        .unwrap();
                    hits.finish()
                };
                let hits = paint(&mut terminal, &view);
                let ordinary = terminal.backend().buffer().clone();
                assert_eq!(ordinary[(pane.x, height - 1)].symbol(), "x");
                assert!(matches!(
                    hits.hit_target_at(pane.x, height - 1),
                    Some(HitTarget::Pane { .. })
                ));
                if areas.sidebar.is_some() {
                    assert!(matches!(
                        hits.hit_target_at(2, height - 1),
                        Some(HitTarget::SettingsButton)
                    ));
                }
                let transition = view.decide(PrefixInput::Key(KeyEvent::press(
                    KeyCode::Char('b'),
                    Modifiers::CTRL,
                )));
                view.commit(transition, SessionActionDisposition::KeepNavigator);
                let hits = paint(&mut terminal, &view);
                assert!(view.prefix_active());
                assert!(matches!(
                    hits.hit_target_at(pane.x, height - 1),
                    Some(HitTarget::OverlaySurface)
                ));
                assert!(
                    !terminal.backend().cursor_visible(),
                    "a covered terminal cursor stays hidden"
                );
                assert_ne!(
                    terminal.backend().buffer()[(pane.x, height - 1)].symbol(),
                    "x"
                );
                assert_eq!(project_shell_areas(area, view.sidebar_surface()), areas);
                let transition = view.decide(PrefixInput::Key(KeyEvent::press(
                    KeyCode::Esc,
                    Modifiers::NONE,
                )));
                view.commit(transition, SessionActionDisposition::KeepNavigator);
                paint(&mut terminal, &view);
                assert_eq!(
                    terminal.backend().buffer(),
                    &ordinary,
                    "the complete terminal and settings row restore"
                );
                assert_eq!(project_shell_areas(area, view.sidebar_surface()), areas);
            }
        }
    }
}
