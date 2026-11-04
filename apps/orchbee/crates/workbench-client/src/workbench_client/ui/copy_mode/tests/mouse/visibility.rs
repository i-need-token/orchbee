//! Search completion must expose the matching served glyph, not a cursor on shell chrome.
use super::*;
use crate::{
    terminal_copy::{CopySearchResult, TerminalTextRange},
    terminal_text::display_width,
    terminal_viewport::ViewportWindow,
    tui_theme::Role,
};
use orchbee_terminal_protocol::{TerminalRowWindowRoute, TerminalTextCell, TerminalTextRow};
use ratatui::{backend::Backend, layout::Position};

const SOURCE_COLUMNS: u16 = 89;
const SOURCE_ROWS: u16 = 38;

fn source_rows(query: &str, tail_column: u16) -> Vec<TerminalTextRow> {
    let blank = TerminalTextCell {
        text: " ".into(),
        width: 0,
        style: orchbee_terminal_protocol::TerminalTextStyle {
            foreground: 0xffffff,
            background: 0,
            underline_color: 0x1000000,
            flags: 0,
            underline: 0,
        },
    };
    (0..SOURCE_ROWS)
        .map(|index| {
            let mut row = TerminalTextRow {
                index,
                flags: 0,
                prompt_class: 0,
                cells: vec![blank.clone(); usize::from(SOURCE_COLUMNS)],
            };
            let (column, text) = if index == 2 {
                for (column, glyph) in "PS> ".chars().enumerate() {
                    row.cells[column].text = glyph.to_string().into();
                }
                (4, query.to_owned())
            } else if index == SOURCE_ROWS - 1 {
                (tail_column, query.to_owned())
            } else {
                (0, format!("HISTORY-{index:04}"))
            };
            let mut column = usize::from(column);
            for glyph in text.chars() {
                let text = glyph.to_string();
                let width = display_width(&text);
                row.cells[column].text = text.into();
                row.cells[column].width = if width == 2 { 1 } else { 0 };
                if width == 2 {
                    row.cells[column + 1].text = Default::default();
                    row.cells[column + 1].width = 2;
                }
                column += width;
            }
            row
        })
        .collect()
}

fn search_fixture(
    primary: bool,
    width: u16,
    query: &str,
    tail_column: u16,
) -> (Port, Arc<CommandMailbox>, InputDispatcher, ClientView) {
    let (mut ui, mailbox, _, _) = ready_ui();
    let key = install_projection(
        &mut ui,
        Some((if primary { 160 } else { 0 }, !primary, SOURCE_ROWS)),
        0,
    );
    let mut frame = ui.projections[0].value.frame().unwrap().clone();
    frame.geometry.columns = SOURCE_COLUMNS;
    frame.geometry.rows = SOURCE_ROWS;
    frame.rows = source_rows(query, tail_column);
    ui.projections[0]
        .value
        .install_frame_for_test(frame.clone());
    if primary {
        // Match the screenshot's 120×40 shell: sidebar 29, content starts at row 2,
        // and all 38 served rows belong to the Host window before the footer covers one.
        ui.state.as_mut().unwrap().sync_pane_set(
            &[(
                key.pane().panel().clone(),
                0,
                orchbee_client_server_protocol::Geometry::new(SOURCE_COLUMNS, SOURCE_ROWS, 8, 16)
                    .unwrap(),
            )],
            Instant::now(),
        );
    }
    ui.enter_copy_mode();
    if primary {
        let enter = mailbox.try_recv().unwrap().unwrap();
        assert!(
            matches!(enter.record(), QueuedCommand::Viewport(tagged) if tagged.value() == &ViewportStep::Enter)
        );
        mailbox.finish_lease(enter).unwrap();
        ui.projections[0]
            .viewport
            .as_mut()
            .unwrap()
            .facts(HistoryChanged {
                attachment_id: frame.route.attachment_id,
                sequence: identity(1),
                generation: frame.route.generation,
                revision: frame.route.revision,
                anchor_status: HistoryAnchorStatus::Valid,
                total_rows: 160 + u64::from(SOURCE_ROWS),
            });
        ui.admit_viewport_steps(&mailbox, &key);
        let read = mailbox.try_recv().unwrap().unwrap();
        let QueuedCommand::Viewport(tagged) = read.record() else {
            panic!("copy read")
        };
        let ViewportStep::Read {
            request_sequence, ..
        } = *tagged.value()
        else {
            panic!("copy read")
        };
        mailbox.finish_lease(read).unwrap();
        ui.projections[0].viewport.as_mut().unwrap().window(
            ViewportWindow {
                graphics_present: false,
                route: TerminalRowWindowRoute {
                    attachment_id: frame.route.attachment_id,
                    host_epoch: frame.route.host_epoch,
                    sequence: request_sequence,
                    generation: frame.route.generation,
                    revision: frame.route.revision,
                    start_row: 160,
                    total_rows: 160 + u64::from(SOURCE_ROWS),
                    columns: SOURCE_COLUMNS,
                    row_count: SOURCE_ROWS,
                    detail: orchbee_terminal_protocol::RowWindowDetail::Render,
                },
                foreground: frame.foreground,
                background: frame.background,
                rows: frame.rows,
            },
            Instant::now(),
        );
        ui.admit_viewport_steps(&mailbox, &key);
        let ack = mailbox.try_recv().unwrap().unwrap();
        assert!(
            matches!(ack.record(), QueuedCommand::Viewport(tagged) if matches!(tagged.value(), ViewportStep::Ack { .. }))
        );
        mailbox.finish_lease(ack).unwrap();
        ui.projections[0]
            .viewport
            .as_mut()
            .unwrap()
            .ack_outcome(request_sequence, true);
        ui.tick_copy_mode();
    }
    assert_eq!(ui.copy_mode_view().unwrap().status, CopyModeStatus::Ready);
    assert!(mailbox.try_recv().unwrap().is_none());
    let mut port = Port {
        ui,
        forwarded: Vec::new(),
    };
    let mut view = ClientView::new(width);
    view.settle_copy_mode(true);
    let mut dispatcher = InputDispatcher::default();
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    dispatcher
        .synchronize(&mut view, &mut port, Instant::now(), &mut |_| {
            Ok::<_, io::Error>(())
        })
        .unwrap();
    (port, mailbox, dispatcher, view)
}

/// Suspend transport only while the real input path emits its intent, then complete that exact
/// query deterministically. Read transport/fence failures have separate owner coverage.
fn search_completion(port: &mut Port, current: usize, invoke: impl FnOnce(&mut Port)) {
    let mailbox = port.ui.mailbox.take();
    invoke(port);
    port.ui.mailbox = mailbox;
    let mode = port.ui.copy_mode.mode_mut().unwrap();
    let Effect::Search {
        identity, query, ..
    } = mode.take_effect().expect("search intent")
    else {
        panic!("search intent")
    };
    let start = u32::try_from(mode.surface().start_row).unwrap();
    let query_width = u16::try_from(display_width(&query)).unwrap();
    let matches = [
        TerminalTextRange {
            start: TerminalTextPoint {
                row: start + 2,
                col: 4,
            },
            end: TerminalTextPoint {
                row: start + 2,
                col: 4 + query_width - 1,
            },
        },
        TerminalTextRange {
            start: TerminalTextPoint {
                row: start + u32::from(SOURCE_ROWS - 1),
                col: 0,
            },
            end: TerminalTextPoint {
                row: start + u32::from(SOURCE_ROWS - 1),
                col: query_width - 1,
            },
        },
    ];
    assert!(
        mode.query_search_completed(
            identity,
            query,
            CopySearchResult {
                matches: matches.into(),
                total: 2,
                current: Some(current),
                current_global: Some(u64::try_from(current).unwrap()),
            }
        )
        .unwrap()
    );
}

fn assert_matching_cursor(
    terminal: &mut Terminal<TestBackend>,
    map: &FrameHitMap,
    query: &str,
    point: TerminalTextPoint,
    height: u16,
    theme: &Theme,
) -> Position {
    let cursor = terminal.backend_mut().get_cursor_position().unwrap();
    assert!(
        cursor.y < height - 1,
        "matching glyph and cursor must be above the footer"
    );
    let mut column = cursor.x;
    for glyph in query.chars() {
        let text = glyph.to_string();
        assert_eq!(
            terminal.backend().buffer()[(column, cursor.y)].symbol(),
            text
        );
        column += u16::try_from(display_width(&text)).unwrap();
    }
    assert_eq!(
        terminal.backend().buffer()[(cursor.x, cursor.y)].bg,
        theme.color(Role::AccentFocus)
    );
    let Some(HitTarget::CopySurface {
        token,
        area,
        origin,
    }) = map.hit_target_at(cursor.x, cursor.y)
    else {
        panic!("matching cursor must be on served content")
    };
    assert_eq!(
        token.surface.point(CopyModeCursor {
            column: origin.column + cursor.x - area.x,
            row: origin.row + cursor.y - area.y,
        }),
        Some(point)
    );
    assert!(!matches!(
        map.hit_target_at(cursor.x, height - 1),
        Some(HitTarget::CopySurface { .. })
    ));
    cursor
}

#[test]
fn search_and_toolbar_repeat_paint_the_actual_tail_match_above_the_full_shell_footer() {
    for (width, height) in [(120, 40), (80, 24), (70, 18), (40, 10)] {
        for primary in [true, false] {
            for locale in [Locale::EnUs, Locale::ZhCn] {
                for theme in [Theme::terminal_native(), Theme::one_dark()] {
                    for query in ["DOGFOOD-SEARCH-ONE", "你好"] {
                        let (mut port, mailbox, mut dispatcher, mut view) =
                            search_fixture(primary, width, query, 0);
                        let panel = port.ui.view().focus_panel_id().unwrap().clone();
                        let source = port
                            .ui
                            .admitted_copy_source(&port.ui.projections[0].key)
                            .unwrap();
                        let ordinary_crop = port.ui.view().pane_live_view(&panel);
                        let bookmark =
                            port.ui.projections[0]
                                .viewport
                                .as_ref()
                                .and_then(|viewport| match viewport.surface() {
                                    crate::terminal_viewport::ViewportSurface::Window(window) => {
                                        Some((window.route.start_row, window.route.total_rows))
                                    }
                                    _ => None,
                                });
                        let first = TerminalTextPoint {
                            row: u32::try_from(source.rows.start_row).unwrap() + 2,
                            col: 4,
                        };
                        let tail = TerminalTextPoint {
                            row: u32::try_from(source.rows.start_row).unwrap()
                                + u32::from(SOURCE_ROWS - 1),
                            col: 0,
                        };
                        search_completion(&mut port, 0, |port| {
                            dispatch(
                                &mut dispatcher,
                                &mut view,
                                port,
                                format!("/{query}\r").as_bytes(),
                            )
                        });
                        let (mut terminal, map) =
                            painted(&port, &view, width, height, locale, &theme);
                        assert_matching_cursor(&mut terminal, &map, query, first, height, &theme);
                        for (bytes, current, point) in
                            [(b"n".as_slice(), 1, tail), (b"N".as_slice(), 0, first)]
                        {
                            search_completion(&mut port, current, |port| {
                                dispatch(&mut dispatcher, &mut view, port, bytes)
                            });
                            let (mut terminal, map) =
                                painted(&port, &view, width, height, locale, &theme);
                            assert_matching_cursor(
                                &mut terminal,
                                &map,
                                query,
                                point,
                                height,
                                &theme,
                            );
                        }
                        let map = paint(&port, &view, width, height, locale, &theme);
                        let search = button(&map, width, height, CopyControl::Search);
                        dispatcher.publish_hit_map(map);
                        click(&mut dispatcher, &mut view, &mut port, search);
                        dispatch(&mut dispatcher, &mut view, &mut port, query.as_bytes());
                        let map = paint(&port, &view, width, height, locale, &theme);
                        let find = button(&map, width, height, CopyControl::SubmitSearch);
                        dispatcher.publish_hit_map(map);
                        search_completion(&mut port, 1, |port| {
                            click(&mut dispatcher, &mut view, port, find)
                        });
                        let (mut terminal, map) =
                            painted(&port, &view, width, height, locale, &theme);
                        assert_matching_cursor(&mut terminal, &map, query, tail, height, &theme);
                        if width >= 70 {
                            for (reverse, current, point) in [(true, 0, first), (false, 1, tail)] {
                                let map = paint(&port, &view, width, height, locale, &theme);
                                let repeat = button(
                                    &map,
                                    width,
                                    height,
                                    CopyControl::RepeatSearch { reverse },
                                );
                                dispatcher.publish_hit_map(map);
                                search_completion(&mut port, current, |port| {
                                    click(&mut dispatcher, &mut view, port, repeat)
                                });
                                let (mut terminal, map) =
                                    painted(&port, &view, width, height, locale, &theme);
                                assert_matching_cursor(
                                    &mut terminal,
                                    &map,
                                    query,
                                    point,
                                    height,
                                    &theme,
                                );
                            }
                        }
                        assert_eq!(
                            port.ui
                                .admitted_copy_source(&port.ui.projections[0].key)
                                .unwrap(),
                            source
                        );
                        assert_eq!(port.ui.view().pane_live_view(&panel), ordinary_crop);
                        assert_eq!(
                            port.ui.projections[0]
                                .viewport
                                .as_ref()
                                .and_then(|viewport| match viewport.surface() {
                                    crate::terminal_viewport::ViewportSurface::Window(window) =>
                                        Some((window.route.start_row, window.route.total_rows)),
                                    _ => None,
                                }),
                            bookmark
                        );
                        assert!(
                            mailbox.try_recv().unwrap().is_none(),
                            "local reveal must not resize or move the Host bookmark"
                        );
                        assert!(port.forwarded.is_empty());
                    }
                }
            }
        }
    }
}

#[test]
fn a_wide_tail_match_keeps_both_cells_visible_and_pointer_selection_uses_source_coordinates() {
    for (width, height) in [(120, 40), (80, 24), (40, 10)] {
        for primary in [true, false] {
            let (mut port, mailbox, mut dispatcher, mut view) =
                search_fixture(primary, width, "中", SOURCE_COLUMNS - 2);
            let mode = port.ui.copy_mode.mode_mut().unwrap();
            let point = TerminalTextPoint {
                row: u32::try_from(mode.surface().start_row).unwrap() + u32::from(SOURCE_ROWS - 1),
                col: SOURCE_COLUMNS - 2,
            };
            mode.begin_search(crate::terminal_copy::CopySearchDirection::Forward)
                .unwrap();
            mode.edit_search(SearchEdit::Insert("中")).unwrap();
            mode.submit_search().unwrap();
            let Effect::Search {
                identity, query, ..
            } = mode.take_effect().unwrap()
            else {
                panic!("search intent")
            };
            mode.query_search_completed(
                identity,
                query,
                CopySearchResult {
                    matches: vec![TerminalTextRange {
                        start: point,
                        end: TerminalTextPoint {
                            col: SOURCE_COLUMNS - 1,
                            ..point
                        },
                    }],
                    total: 1,
                    current: Some(0),
                    current_global: Some(0),
                },
            )
            .unwrap();
            let theme = Theme::terminal_native();
            let (mut terminal, map) = painted(&port, &view, width, height, Locale::ZhCn, &theme);
            let cursor = assert_matching_cursor(&mut terminal, &map, "中", point, height, &theme);
            assert!(
                cursor.x + 1 < width,
                "the wide tail must fit next to its leading glyph"
            );
            assert!(matches!(
                map.hit_target_at(cursor.x + 1, cursor.y),
                Some(HitTarget::CopySurface { .. })
            ));
            dispatcher.publish_hit_map(map);
            dispatch(
                &mut dispatcher,
                &mut view,
                &mut port,
                report((cursor.x + 1, cursor.y), 0, false).as_bytes(),
            );
            let (key, command, action) = take_selection(&mut port.ui, &mailbox);
            assert!(matches!(action, TerminalViewerAction::SelectionGesture {
                gesture: ViewerSelectionGestureAction::Start { point, .. }, ..
            } if point.column == SOURCE_COLUMNS - 2 && point.row == SOURCE_ROWS - 1));
            reply_selection(
                &mut port.ui,
                &mailbox,
                key,
                command,
                SelectionChangedState::None,
            );
            port.ui.tick_copy_mode();
            // A drag over the footer clamps to served content; it never addresses footer text.
            dispatch(
                &mut dispatcher,
                &mut view,
                &mut port,
                report((cursor.x - 1, height - 1), 32, false).as_bytes(),
            );
            dispatch(
                &mut dispatcher,
                &mut view,
                &mut port,
                report((cursor.x - 1, height - 1), 0, true).as_bytes(),
            );
            let (_, _, action) = take_selection(&mut port.ui, &mailbox);
            assert!(matches!(action, TerminalViewerAction::SelectionGesture {
                gesture: ViewerSelectionGestureAction::Update { point, .. }, ..
            } if point.column == SOURCE_COLUMNS - 3 && point.row == SOURCE_ROWS - 1));
            assert_eq!(
                port.ui.copy_mode.mode().unwrap().cursor(),
                TerminalTextPoint {
                    col: SOURCE_COLUMNS - 3,
                    ..point
                }
            );
            assert!(port.forwarded.is_empty());
        }
    }
}
