//! Composed terminal chrome floats over the final content row and restores it after capture.
use super::*;
use crate::{
    client_view::{ClientActionDisposition, PrefixInput},
    localization::Locale,
    tui_layout::project_shell_areas,
    tui_pointer::{HitMapBuilder, HitTarget},
    tui_theme::Theme,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

#[test]
fn read_only_crop_keeps_complete_hints_beside_actions_and_prompt_above_status() {
    for (width, height) in [(80, 24), (40, 10)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let (mut ui, mailbox, _, _) = ready_ui();
            let view = ClientView::new(width);
            let area = Rect::new(0, 0, width, height);
            let areas = project_shell_areas(area, view.sidebar_surface());
            let pane = crate::tui_layout::active_pane_area(&areas, Some(ui.view())).unwrap();
            ui.observe_geometry(Geometry::new(pane.width, pane.height, 8, 16).unwrap());
            drain(&mailbox);
            let key = install_projection(&mut ui, None, 0);
            let projection = projection_at(&mut ui, &key);
            let mut source = frame(&projection.value, 0, false, 36);
            source.geometry.columns = 104;
            source.geometry.rows = 36;
            source.cursor.visible = true;
            source.cursor.column = 0;
            source.cursor.row = 35;
            source.rows[35].cells[0].text = ">".into();
            projection.value.install_frame_for_test(source);
            let crop = ui.view().pane_live_view(key.pane().panel()).unwrap();
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal
                .draw(|frame| {
                    crate::tui_render::render_terminal_shell(
                        frame,
                        &view,
                        locale,
                        Some(ui.view()),
                        &Theme::terminal_native(),
                        &mut HitMapBuilder::new(),
                    );
                })
                .unwrap();
            let cells_text = |row, columns: std::ops::Range<u16>| -> String {
                columns
                    .map(|column| terminal.backend().buffer()[(column, row)].symbol())
                    .flat_map(str::chars)
                    .filter(|character| !character.is_whitespace())
                    .collect()
            };
            let row_text = |row| cells_text(row, 0..width);
            let expected_header = match locale {
                Locale::EnUs => "LivecropCtrl+bPageUpHistory",
                Locale::ZhCn => "实时裁切Ctrl+bPageUp历史",
            };
            assert_eq!(
                cells_text(pane.y, pane.x..pane.right()),
                expected_header,
                "{width} {locale:?}"
            );
            let action_row = if areas.tabs.is_empty() {
                areas.workbench.y
            } else {
                areas.tabs.y
            };
            let action = match locale {
                Locale::EnUs => "[Actions]",
                Locale::ZhCn => "[动作菜单]",
            };
            // Global actions stay in the menu bindings, never in the top strip.
            assert!(
                !row_text(action_row).ends_with(action),
                "{width} {locale:?}"
            );
            assert_eq!(
                terminal.backend().buffer()[(pane.x, pane.bottom() - 2)].symbol(),
                ">"
            );
            assert_eq!(
                terminal.get_cursor_position().unwrap(),
                ratatui::layout::Position::new(pane.x, pane.bottom() - 2),
                "header chrome never moves the live cursor",
            );
            assert_eq!(ui.view().pane_live_view(key.pane().panel()), Some(crop));
            assert!(crop.clipped);
            assert!(
                mailbox.try_recv().unwrap().is_none(),
                "paint never requests a physical resize"
            );
            assert!(
                ui.history_wheel(&key, true),
                "a clipped current grid is readable even without older scrollback"
            );
            assert_eq!(recv_viewports(&mailbox), [ViewportStep::Enter]);
            mailbox.close(CarrierFailure::WorkerExited);
        }
    }
}

#[test]
fn minimal_global_actions_leave_the_projected_first_row_and_live_cursor_to_the_terminal() {
    use crate::terminal_device::WindowDimensions;
    use crate::tui_layout::ViewportTier;

    for (width, height) in [(40, 14), (59, 11), (20, 5)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let (mut ui, mailbox, _, _) = ready_ui();
            let view = ClientView::new(width);
            let host = Rect::new(0, 0, width, height);
            let areas = project_shell_areas(host, view.sidebar_surface());
            assert_eq!(areas.tier, ViewportTier::Minimal, "{width}x{height}");
            assert!(areas.tabs.is_empty(), "{width}x{height}");

            let pane = crate::tui_layout::active_pane_area(&areas, Some(ui.view()))
                .expect("Minimal remains a presentable terminal viewport");
            let geometry = crate::tui_layout::terminal_geometry(
                WindowDimensions {
                    columns: width,
                    pixel_height: height * 16,
                    pixel_width: width * 8,
                    rows: height,
                },
                &view,
                Some(ui.view()),
            )
            .unwrap()
            .expect("Minimal publishes terminal geometry");
            assert_eq!(
                geometry,
                Geometry::new(pane.width, pane.height, 8, 16).unwrap()
            );
            assert_eq!(
                pane.y, host.y,
                "{width}x{height}: terminal starts at row zero"
            );
            assert!(!pane.intersects(areas.tabs), "{width}x{height}");
            ui.observe_geometry(geometry);
            drain(&mailbox);

            let key = install_projection(&mut ui, None, 0);
            let projection = projection_at(&mut ui, &key);
            let mut source = frame(&projection.value, 0, false, pane.height);
            source.geometry.columns = pane.width;
            source.geometry.rows = pane.height;
            source.cursor.visible = true;
            source.cursor.column = 0;
            source.cursor.row = 0;
            source.rows[0].cells[0].text = ">".into();
            projection.value.install_frame_for_test(source);

            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            let mut hits = HitMapBuilder::new();
            terminal
                .draw(|frame| {
                    crate::tui_render::render_terminal_shell(
                        frame,
                        &view,
                        locale,
                        Some(ui.view()),
                        &Theme::terminal_native(),
                        &mut hits,
                    );
                })
                .unwrap();
            let map = hits.finish();

            assert_eq!(terminal.backend().buffer()[(pane.x, pane.y)].symbol(), ">");
            let cursor = terminal.get_cursor_position().unwrap();
            assert!(
                pane.contains(cursor),
                "{width}x{height} {locale:?}: cursor is in terminal area"
            );
            assert_eq!(
                cursor,
                ratatui::layout::Position::new(pane.x, pane.y),
                "{width}x{height} {locale:?}: live cursor uses projected terminal origin"
            );
            assert!(matches!(
                map.hit_target_at(pane.x, pane.y),
                Some(HitTarget::Pane { content, .. }) if content == &pane
            ));
            assert!(
                (0..height).all(|y| (0..width)
                    .all(|x| { map.hit_target_at(x, y) != Some(&HitTarget::MenuButton) })),
                "{width}x{height} {locale:?}: Actions is not drawn over terminal content"
            );
            assert!(
                mailbox.try_recv().unwrap().is_none(),
                "paint does not resize the PTY"
            );
            mailbox.close(CarrierFailure::WorkerExited);
        }
    }
}

#[test]
fn creation_admission_retires_old_map_before_mouse_events_in_the_same_or_next_packet() {
    use crate::terminal_input::{PaneAdmission, PointerSelectionCell, PointerSelectionIntent};

    for same_packet in [true, false] {
        for history in [false, true] {
            let (mut ui, mailbox, clock, _) = controlling_ui();
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
            dispatcher.set_pointer_capture(&mut view, &mut ClientInputPort(&mut ui), true);
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
            // Only the explicit at-directory entry waits for an Enter-accepted review.
            assert_eq!(
                ui.apply_client_action(
                    crate::client_view::ClientAction::RequestWorkspaceAtDirectory
                ),
                ClientActionDisposition::LayoutChanged
            );
            let snapshot = ui
                .state
                .as_ref()
                .unwrap()
                .cache()
                .canonical_snapshot()
                .clone();
            super::super::workspace_setup::finish_preparation(
                &mut ui, &mailbox, &clock, &snapshot, false,
            );
            let token = ui.view().setup_draft().unwrap().token();
            let resumed = mailbox.try_recv().unwrap().unwrap();
            assert!(
                matches!(resumed.record(), QueuedCommand::Poll(_)),
                "the settled preparation resumes the controller poll"
            );
            mailbox.finish_lease(resumed).unwrap();
            ui.after_setup_draw(Some(ModalPresentation::Presented(token)));
            // Enter accepts the drawn review and admits creation. No preview is drawn before
            // this old-map click, drag, release and wheel sequence is decoded.
            let mouse = b"\x1b[<0;2;2M\x1b[<32;3;3M\x1b[<0;3;3m\x1b[<64;2;2M";
            if same_packet {
                let mut packet = b"\r".to_vec();
                packet.extend_from_slice(mouse);
                dispatch(&mut dispatcher, &mut view, &mut ui, &packet);
            } else {
                dispatch(&mut dispatcher, &mut view, &mut ui, b"\r");
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
                PaneAdmission::Rejected,
            );
            assert!(!ui.history_wheel(&key, true));
            assert!(!ui.history_key(&KeyEvent::press(KeyCode::PageUp, Modifiers::NONE)));
            assert!(!ui.scrollbar_scroll(&key, source, 0));
            let lease = mailbox.try_recv().unwrap().unwrap();
            assert!(matches!(lease.record(), QueuedCommand::WorkspaceSetup(_)));
            mailbox.finish_lease(lease).unwrap();
            assert!(
                mailbox.try_recv().unwrap().is_none(),
                "no hidden-pane commands"
            );
            // Retiring a gesture must still be able to clean up its old owner.
            assert_eq!(
                ui.pointer_selection(&key, PointerSelectionIntent::Cancel),
                PaneAdmission::Admitted,
            );
            mailbox.close(CarrierFailure::WorkerExited);
        }
    }
}

/// The floating chip on the last row: the status cells that block the pane beneath.
fn chip_cells(hits: &crate::tui_pointer::FrameHitMap, status: Rect) -> Option<Rect> {
    let covered = (status.x..status.right())
        .filter(|&x| {
            matches!(
                hits.hit_target_at(x, status.y),
                Some(HitTarget::OverlaySurface)
            )
        })
        .collect::<Vec<_>>();
    let first = *covered.first()?;
    let last = *covered.last()?;
    assert_eq!(
        covered.len(),
        usize::from(last - first + 1),
        "one contiguous chip"
    );
    Some(Rect::new(first, status.y, last - first + 1, 1))
}

#[test]
fn prefix_status_floats_over_the_last_terminal_row_and_restores_it_without_resizing() {
    for (width, height) in [(120, 40), (80, 24), (60, 12), (50, 10)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for theme in [Theme::terminal_native(), Theme::one_light()] {
                let (mut ui, _, _, _) = controlling_ui();
                let mut view = taught(width);
                let area = Rect::new(0, 0, width, height);
                let areas = project_shell_areas(area, view.sidebar_surface());
                let pane = crate::tui_layout::active_pane_area(&areas, Some(ui.view())).unwrap();
                assert_eq!(
                    pane,
                    crate::tui_layout::native_terminal_area(
                        areas.panel,
                        crate::tui_layout::PaneFrame::Bare
                    ),
                    "a single pane fills the panel without a frame"
                );
                assert_eq!(
                    pane.bottom(),
                    areas.status.bottom(),
                    "terminal content reaches the last row"
                );
                // The shell observes the drawn pane before painting it.
                ui.observe_geometry(Geometry::new(pane.width, pane.height, 8, 16).unwrap());
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
                                Some(ui.view()),
                                &theme,
                                &mut hits,
                            );
                        })
                        .unwrap();
                    hits.finish()
                };
                let hits = paint(&mut terminal, &view);
                let ordinary = terminal.backend().buffer().clone();
                let last = pane.bottom() - 1;
                for y in [pane.y, last] {
                    assert_eq!(ordinary[(pane.x, y)].symbol(), "x");
                    assert!(matches!(
                        hits.hit_target_at(pane.x, y),
                        Some(HitTarget::Pane { content, .. }) if content == &pane
                    ));
                }
                // At rest nothing floats: the whole last row is terminal content.
                assert!(
                    (pane.x..pane.right()).all(|x| matches!(
                        hits.hit_target_at(x, last),
                        Some(HitTarget::Pane { .. })
                    ))
                );
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
                view.commit(transition, ClientActionDisposition::KeepNavigator);
                let hits = paint(&mut terminal, &view);
                assert!(view.prefix_active());
                assert!(
                    !terminal.backend().cursor_visible(),
                    "prefix input hides the terminal cursor"
                );
                let prefixed = terminal.backend().buffer();
                let chip = chip_cells(&hits, areas.status)
                    .expect("the prefix status floats on the last row");
                // End-aligned one cell in from the right; the row's first cell stays content.
                assert_eq!(chip.right(), areas.status.right() - 1);
                assert!(chip.x > areas.status.x);
                assert!((chip.x..chip.right()).any(|x| prefixed[(x, last)] != ordinary[(x, last)]));
                for y in pane.y..pane.bottom() {
                    for x in pane.x..pane.right() {
                        if !chip.contains((x, y).into()) {
                            assert_eq!(
                                prefixed[(x, y)],
                                ordinary[(x, y)],
                                "the chip covers only its own cells ({x}, {y})"
                            );
                        }
                    }
                }
                assert!(matches!(
                    hits.hit_target_at(areas.status.x, last),
                    Some(HitTarget::Pane { .. })
                ));
                assert_eq!(project_shell_areas(area, view.sidebar_surface()), areas);
                assert_eq!(
                    crate::tui_layout::active_pane_area(&areas, Some(ui.view())),
                    Some(pane),
                    "the chip never resizes the terminal"
                );
                let transition = view.decide(PrefixInput::Key(KeyEvent::press(
                    KeyCode::Esc,
                    Modifiers::NONE,
                )));
                view.commit(transition, ClientActionDisposition::KeepNavigator);
                paint(&mut terminal, &view);
                assert_eq!(
                    terminal.backend().buffer(),
                    &ordinary,
                    "the complete terminal row restores"
                );
                assert_eq!(project_shell_areas(area, view.sidebar_surface()), areas);
            }
        }
    }
}

/// The cursor is placed after the floating status, so the chip hides only a cursor it covers.
#[test]
fn launch_chip_hides_only_the_terminal_cursor_beneath_it() {
    for (width, height) in [(120, 40), (60, 12)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let theme = Theme::terminal_native();
            let (mut ui, _, _, _) = controlling_ui();
            let mut view = ClientView::new(width);
            view.machines.available = true;
            assert!(view.launch_hint_pending());
            let areas = project_shell_areas(Rect::new(0, 0, width, height), view.sidebar_surface());
            let pane = crate::tui_layout::active_pane_area(&areas, Some(ui.view())).unwrap();
            ui.observe_geometry(Geometry::new(pane.width, pane.height, 8, 16).unwrap());
            let key = install_projection(&mut ui, None, 0);
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            let mut paint = |ui: &mut WorkbenchClientUi, column: u16| {
                let projection = projection_at(ui, &key);
                let mut content = frame(&projection.value, 0, false, pane.height);
                content.geometry.columns = pane.width;
                content.geometry.rows = pane.height;
                content.cursor.visible = true;
                content.cursor.row = pane.height - 1;
                content.cursor.column = column;
                projection.value.install_frame_for_test(content);
                let mut hits = HitMapBuilder::new();
                terminal
                    .draw(|frame| {
                        crate::tui_render::render_terminal_shell(
                            frame,
                            &view,
                            locale,
                            Some(ui.view()),
                            &theme,
                            &mut hits,
                        );
                    })
                    .unwrap();
                let cursor = terminal
                    .backend()
                    .cursor_visible()
                    .then(|| terminal.get_cursor_position().unwrap());
                (hits.finish(), cursor)
            };
            let (hits, cursor) = paint(&mut ui, 0);
            let last = pane.bottom() - 1;
            assert_eq!(
                cursor,
                Some((pane.x, last).into()),
                "{locale:?} {width}x{height}"
            );
            let chip = chip_cells(&hits, areas.status).expect("the launch hint floats at rest");
            assert!(chip.x > pane.x);
            let (_, covered) = paint(&mut ui, chip.x - pane.x + 1);
            assert_eq!(
                covered, None,
                "{locale:?} {width}x{height}: the chip hides the cursor"
            );
            let (_, beside) = paint(&mut ui, chip.x - pane.x - 1);
            assert_eq!(beside, Some((chip.x - 1, last).into()));
        }
    }
}

#[test]
fn ended_single_terminal_names_its_exit_in_the_floating_chip_beside_its_fix() {
    use orchbee_terminal_protocol::{
        CURRENT_PROTOCOL_VERSION, CreationId, Envelope, ExitResult, LifecycleControl, MessageRole,
        PayloadEncoding, TerminalExited, TerminalId, TerminalIdentity, encode_lifecycle_control,
        encode_transport_envelope,
    };

    for code in [0, 7, u32::MAX] {
        for (width, height) in [(120, 40), (80, 24), (60, 12), (50, 10), (20, 5)] {
            for locale in [Locale::EnUs, Locale::ZhCn] {
                for theme in [Theme::terminal_native(), Theme::one_light()] {
                    let (mut ui, _, _, _) = controlling_ui();
                    let mut view = ClientView::new(width);
                    view.machines.available = true;
                    let areas =
                        project_shell_areas(Rect::new(0, 0, width, height), view.sidebar_surface());
                    let pane =
                        crate::tui_layout::active_pane_area(&areas, Some(ui.view())).unwrap();
                    let key = install_projection(&mut ui, None, 0);
                    let projection = projection_at(&mut ui, &key);
                    let mut content = frame(&projection.value, 0, false, pane.height);
                    content.geometry.columns = pane.width;
                    content.geometry.rows = pane.height;
                    projection.value.install_frame_for_test(content);
                    let end = encode_lifecycle_control(&LifecycleControl::TerminalExited(
                        TerminalExited {
                            identity: TerminalIdentity {
                                creation_id: CreationId::from_bytes([9; 16]).unwrap(),
                                host_epoch: *projection.value.epoch(),
                                terminal_id: TerminalId::from_bytes(7_003u128.to_be_bytes())
                                    .unwrap(),
                            },
                            final_revision: 1,
                            exit: ExitResult::ExitCode(code),
                        },
                    ))
                    .unwrap();
                    let record = encode_transport_envelope(&Envelope {
                        protocol: CURRENT_PROTOCOL_VERSION,
                        role: MessageRole::Event,
                        kind: end.kind,
                        payload_encoding: PayloadEncoding::ControlJson,
                        host_epoch: Some(*projection.value.epoch()),
                        correlation_id: None,
                        optional_message: false,
                        payload: &end.payload,
                    })
                    .unwrap();
                    assert_eq!(projection.value.receive_lifecycle(&record), Ok(true));
                    assert_eq!(
                        ui.view().phase(),
                        crate::workbench_client::DisplayPhase::Ready
                    );
                    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                    for _ in 0..2 {
                        let mut hits = HitMapBuilder::new();
                        terminal
                            .draw(|frame| {
                                crate::tui_render::render_terminal_shell(
                                    frame,
                                    &view,
                                    locale,
                                    Some(ui.view()),
                                    &theme,
                                    &mut hits,
                                );
                            })
                            .unwrap();
                        let end = crate::terminal_projection::TerminalEnd::Exited(
                            ExitResult::ExitCode(code),
                        );
                        // Two words where they fit, then the compact exit; never the long fact.
                        let available = usize::from(areas.status.width) - 4;
                        let named = |text: &str| format!("{} {text}", theme.glyphs.status_bullet);
                        let short = named(locale.status().terminal_exited);
                        let compact = named(&locale.terminal_end_compact(end));
                        let fix = format!("Ctrl+b c {}", locale.status().create_tab);
                        let buffer = terminal.backend().buffer();
                        let mut status = String::new();
                        let mut x = areas.status.x;
                        while x < areas.status.right() {
                            let symbol = buffer[(x, areas.status.y)].symbol();
                            status.push_str(symbol);
                            x += u16::try_from(crate::terminal_text::display_width(symbol).max(1))
                                .unwrap();
                        }
                        let fits =
                            |text: &str| crate::terminal_text::display_width(text) <= available;
                        let expected = if fits(&short) {
                            Some(short.as_str())
                        } else if fits(&compact) {
                            Some(compact.as_str())
                        } else {
                            None
                        };
                        if let Some(expected) = expected {
                            assert!(
                                status.contains(expected),
                                "{locale:?} {width}x{height}: {status}"
                            );
                        }
                        assert!(
                            !status.contains(&locale.terminal_end(end)),
                            "{locale:?} {width}x{height}: {status}"
                        );
                        if fits(&format!("{short} · {fix}")) {
                            assert!(
                                status.contains(&fix),
                                "{locale:?} {width}x{height}: {status}"
                            );
                        }
                        assert_eq!(buffer[(pane.x, pane.y)].symbol(), "x");
                        assert_eq!(pane.bottom(), areas.status.bottom());
                        assert!(!terminal.backend().cursor_visible());
                        let hits = hits.finish();
                        let chip = (areas.status.x..areas.status.right())
                            .filter(|x| {
                                matches!(
                                    hits.hit_target_at(*x, areas.status.y),
                                    Some(HitTarget::OverlaySurface)
                                )
                            })
                            .collect::<Vec<_>>();
                        assert_eq!(chip.is_empty(), expected.is_none());
                        if let (Some(first), Some(last)) = (chip.first(), chip.last()) {
                            assert_eq!(*last + 2, areas.status.right());
                            assert_eq!(chip.len(), usize::from(last - first + 1));
                        }
                        assert_eq!(
                            crate::tui_layout::active_pane_area(&areas, Some(ui.view())),
                            Some(pane)
                        );
                    }
                }
            }
        }
    }
}
