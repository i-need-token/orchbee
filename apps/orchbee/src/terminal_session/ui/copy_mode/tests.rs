use super::*;
mod mouse;
use crate::{
    local_client::test_server::adjustable_operation_clock,
    terminal_input::{
        DecodedKind, LegacyDecoder, PointerSelectionAdmission, PointerSelectionCell,
        PointerSelectionIntent,
    },
    terminal_session::state::{AttachmentKey, PaneKey},
};
use orchbee_client_server_protocol::{
    TerminalViewerAction, TerminalViewerOutcome, ViewerId, ViewerRejectedReason,
    ViewerSelectionGestureAction,
};
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, CorrelationId, Envelope, HistoryAnchorStatus, HistoryChanged,
    MessageRole, PayloadEncoding, SelectionChanged, SelectionChangedState, SelectionControl,
    SelectionExportControl, SelectionExportFinished, SelectionExportFormat, SelectionExportStarted,
    SelectionExportStatus, SelectionPoint, encode_selection_control,
    encode_selection_export_control, encode_transport_envelope,
};
use orchbee_workbench_protocol::{WorkbenchPanelId, WorkbenchTabId};
use std::{
    sync::{Arc, atomic::AtomicU64},
    time::Instant,
};

use super::super::tests::move_membership::ready_ui;
use super::super::tests::viewport::{controlling_ui, install_projection, settle_viewing};

fn id(value: u128) -> String {
    format!("{value:032x}")
}

fn pane() -> PaneRef {
    PaneRef::new(
        PaneKey::new(
            AttachmentKey::new(1, WorkbenchTabId::parse(&id(1)).unwrap()),
            WorkbenchPanelId::parse(&id(2)).unwrap(),
        ),
        0,
    )
}

fn identity(value: u64) -> NonZeroU64 {
    NonZeroU64::new(value).unwrap()
}

fn coordinator() -> Coordinator {
    Coordinator::new(adjustable_operation_clock(Arc::new(AtomicU64::new(0))))
}

fn take_selection(
    ui: &mut NativeUi,
    mailbox: &Arc<CommandMailbox>,
) -> (PaneRef, terminal_selection::Command, TerminalViewerAction) {
    ui.admit_selections(mailbox);
    let lease = mailbox.try_recv().unwrap().expect("selection command");
    let QueuedCommand::Selection(tagged) = lease.record() else {
        panic!("selection command")
    };
    let key = tagged.key().clone();
    let command = *tagged.value();
    let attachment = ui
        .projections
        .iter()
        .find(|projection| projection.key == key)
        .map(|projection| *projection.value.attachment())
        .unwrap();
    let viewer = ViewerId::parse(&attachment.to_string()).unwrap();
    let action = command.viewer_action(viewer);
    mailbox.finish_lease(lease).unwrap();
    (key, command, action)
}

fn reply_selection(
    ui: &mut NativeUi,
    mailbox: &Arc<CommandMailbox>,
    key: PaneRef,
    command: terminal_selection::Command,
    state: SelectionChangedState,
) {
    let projection = ui
        .projections
        .iter()
        .find(|projection| projection.key == key)
        .unwrap();
    let response = encode_selection_control(&SelectionControl::ChangedResponse(SelectionChanged {
        attachment_id: *projection.value.attachment(),
        state,
    }))
    .unwrap();
    let record = encode_transport_envelope(&Envelope {
        correlation_id: Some(CorrelationId::from_bytes([9; 16]).unwrap()),
        host_epoch: Some(*projection.value.epoch()),
        kind: response.kind,
        optional_message: false,
        payload: &response.payload,
        payload_encoding: PayloadEncoding::ControlJson,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Response,
    })
    .unwrap();
    assert!(ui.apply_selection_outcome(
        mailbox,
        key,
        command,
        TerminalViewerOutcome::Record(record),
    ));
}

fn reject_selection(
    ui: &mut NativeUi,
    mailbox: &Arc<CommandMailbox>,
    key: PaneRef,
    command: terminal_selection::Command,
) -> bool {
    ui.apply_selection_outcome(
        mailbox,
        key,
        command,
        TerminalViewerOutcome::Rejected {
            reason: ViewerRejectedReason::HostRejected,
        },
    )
}

fn reply_export_control(
    ui: &mut NativeUi,
    mailbox: &Arc<CommandMailbox>,
    key: PaneRef,
    command: terminal_selection::Command,
    control: SelectionExportControl,
) {
    let projection = ui
        .projections
        .iter()
        .find(|projection| projection.key == key)
        .unwrap();
    let response = encode_selection_export_control(&control).unwrap();
    let record = encode_transport_envelope(&Envelope {
        correlation_id: Some(CorrelationId::from_bytes([10; 16]).unwrap()),
        host_epoch: Some(*projection.value.epoch()),
        kind: response.kind,
        optional_message: false,
        payload: &response.payload,
        payload_encoding: PayloadEncoding::ControlJson,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Response,
    })
    .unwrap();
    assert!(ui.apply_selection_outcome(
        mailbox,
        key,
        command,
        TerminalViewerOutcome::Record(record),
    ));
}

fn active_selection() -> SelectionChangedState {
    SelectionChangedState::Active {
        start: SelectionPoint { column: 0, row: 0 },
        end: SelectionPoint { column: 2, row: 0 },
        rectangle: false,
        preview: None,
    }
}

fn install_wide_tail(ui: &mut NativeUi, row: u16, lead_column: u16, cursor: Option<(u16, u16)>) {
    let projection = &mut ui.projections[0];
    let mut frame = projection.value.frame().unwrap().clone();
    let template = frame.rows[usize::from(row)].cells[0].clone();
    let mut cells = vec![template; usize::from(frame.geometry.columns)];
    for cell in &mut cells {
        cell.text = " ".into();
        cell.width = 0;
    }
    cells[usize::from(lead_column)].text = "中".into();
    cells[usize::from(lead_column)].width = 1;
    cells[usize::from(lead_column + 1)].text = "".into();
    cells[usize::from(lead_column + 1)].width = 2;
    frame.rows[usize::from(row)].cells = cells;
    if let Some((column, row)) = cursor {
        frame.cursor.column = column;
        frame.cursor.row = row;
    }
    projection.value.install_frame_for_test(frame);
}

#[test]
fn primary_zero_scrollback_enters_tail_zero_without_exposing_a_cursor() {
    let (mut ui, mailbox, _, _) = ready_ui();
    install_projection(&mut ui, Some((0, false, 24)), 0);

    assert_eq!(
        ui.enter_copy_mode(),
        crate::client_view::SessionActionDisposition::DismissNavigator
    );
    let view = ui.copy_mode_view().unwrap();
    assert_eq!(
        view.status,
        crate::terminal_copy_mode::CopyModeStatus::Entering
    );
    assert_eq!(view.cursor, None);
    assert!(ui.capture_copy_mode_key(&KeyEvent::press(KeyCode::Up, Modifiers::NONE)));
    assert_eq!(
        ui.view().notice(),
        Some(super::super::NativeSessionNotice::CopyModeBusy)
    );
    let lease = mailbox.try_recv().unwrap().unwrap();
    assert!(matches!(
        lease.record(),
        QueuedCommand::Viewport(tagged) if tagged.value() == &ViewportStep::Enter
    ));
    mailbox.finish_lease(lease).unwrap();
}

#[test]
fn alternate_screen_enters_on_the_exact_live_frame_without_history_demand() {
    let (mut ui, mailbox, _, _) = ready_ui();
    install_projection(&mut ui, Some((0, true, 24)), 0);

    assert_eq!(
        ui.enter_copy_mode(),
        crate::client_view::SessionActionDisposition::DismissNavigator
    );
    let view = ui.copy_mode_view().unwrap();
    assert_eq!(
        view.status,
        crate::terminal_copy_mode::CopyModeStatus::Ready
    );
    assert_eq!(
        view.cursor,
        Some(crate::terminal_copy_mode::CopyModeCursor { column: 0, row: 0 })
    );
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn alternate_entry_normalizes_an_application_cursor_on_a_wide_tail() {
    let (mut ui, _, _, _) = ready_ui();
    install_projection(&mut ui, Some((0, true, 24)), 0);
    install_wide_tail(&mut ui, 0, 0, Some((1, 0)));

    ui.enter_copy_mode();
    assert_eq!(
        ui.copy_mode_view().unwrap().cursor,
        Some(crate::terminal_copy_mode::CopyModeCursor { column: 0, row: 0 })
    );
}

#[test]
fn local_page_and_buffer_end_navigation_normalize_wide_tail_targets() {
    for (key, cursor_row, wide_row) in [
        (KeyCode::PageDown, 0, 23),
        (KeyCode::Char('g'), 12, 0),
        (KeyCode::Char('G'), 12, 23),
    ] {
        let (mut ui, _, _, _) = ready_ui();
        install_projection(&mut ui, Some((0, true, 24)), 0);
        install_wide_tail(&mut ui, wide_row, 1, Some((2, cursor_row)));
        ui.enter_copy_mode();

        assert!(ui.capture_copy_mode_key(&KeyEvent::press(key, Modifiers::NONE)));
        assert_eq!(
            ui.copy_mode_view().unwrap().cursor,
            Some(crate::terminal_copy_mode::CopyModeCursor {
                column: 1,
                row: wide_row,
            })
        );
    }
}

#[test]
fn revision_rebase_normalizes_the_rebased_cursor_against_the_new_rows() {
    let (mut ui, _, _, _) = ready_ui();
    install_projection(&mut ui, Some((0, true, 24)), 0);
    {
        let projection = &mut ui.projections[0];
        let mut frame = projection.value.frame().unwrap().clone();
        frame.cursor.column = 2;
        frame.cursor.row = 5;
        projection.value.install_frame_for_test(frame);
    }
    ui.enter_copy_mode();
    assert_eq!(
        ui.copy_mode_view().unwrap().cursor,
        Some(crate::terminal_copy_mode::CopyModeCursor { column: 2, row: 5 })
    );

    install_wide_tail(&mut ui, 5, 1, Some((2, 5)));
    {
        let projection = &mut ui.projections[0];
        let mut frame = projection.value.frame().unwrap().clone();
        frame.route.sequence = identity(2);
        frame.route.revision = 2;
        projection.value.install_frame_for_test(frame);
    }
    assert!(ui.tick_copy_mode());
    assert_eq!(
        ui.copy_mode_view().unwrap().cursor,
        Some(crate::terminal_copy_mode::CopyModeCursor { column: 1, row: 5 })
    );
}

#[test]
fn host_none_keeps_keyboard_selection_until_a_later_update_becomes_active() {
    let (mut ui, mailbox, _, _) = ready_ui();
    install_projection(&mut ui, Some((0, true, 24)), 0);
    ui.enter_copy_mode();

    assert!(ui.capture_copy_mode_key(&KeyEvent::press(KeyCode::Char('v'), Modifiers::NONE,)));
    let (key, start, action) = take_selection(&mut ui, &mailbox);
    assert!(matches!(
        action,
        TerminalViewerAction::SelectionGesture {
            gesture: ViewerSelectionGestureAction::Start { click_count: 1, .. },
            ..
        }
    ));
    reply_selection(
        &mut ui,
        &mailbox,
        key.clone(),
        start,
        SelectionChangedState::None,
    );
    ui.tick_copy_mode();
    assert!(ui.copy_mode_view().unwrap().selecting);

    assert!(ui.capture_copy_mode_key(&KeyEvent::press(KeyCode::Right, Modifiers::NONE,)));
    let (same, update, action) = take_selection(&mut ui, &mailbox);
    assert_eq!(same, key);
    assert!(matches!(
        action,
        TerminalViewerAction::SelectionGesture {
            gesture: ViewerSelectionGestureAction::Update { .. },
            ..
        }
    ));
    reply_selection(&mut ui, &mailbox, same, update, SelectionChangedState::None);
    ui.tick_copy_mode();
    assert!(ui.copy_mode_view().unwrap().selecting);

    assert!(ui.capture_copy_mode_key(&KeyEvent::press(KeyCode::Right, Modifiers::NONE,)));
    let (same, update, _) = take_selection(&mut ui, &mailbox);
    reply_selection(&mut ui, &mailbox, same, update, active_selection());
    ui.tick_copy_mode();
    assert!(ui.copy_mode_view().unwrap().selecting);

    assert!(ui.capture_copy_mode_key(&KeyEvent::press(KeyCode::Char('y'), Modifiers::NONE,)));
    for expected_end in [false, true] {
        let (same, command, action) = take_selection(&mut ui, &mailbox);
        assert_eq!(same, key);
        assert_eq!(
            matches!(
                action,
                TerminalViewerAction::SelectionGesture {
                    gesture: ViewerSelectionGestureAction::End { .. },
                    ..
                }
            ),
            expected_end,
        );
        reply_selection(&mut ui, &mailbox, same, command, active_selection());
    }
    ui.tick_copy_mode();
    let (_, _, action) = take_selection(&mut ui, &mailbox);
    assert!(matches!(
        action,
        TerminalViewerAction::SelectionExport { .. }
    ));
}

#[test]
fn end_none_reports_no_selection_and_returns_to_ready_without_export() {
    let (mut ui, mailbox, _, _) = ready_ui();
    install_projection(&mut ui, Some((0, true, 24)), 0);
    ui.enter_copy_mode();

    assert!(ui.capture_copy_mode_key(&KeyEvent::press(KeyCode::Char('v'), Modifiers::NONE,)));
    let (key, start, _) = take_selection(&mut ui, &mailbox);
    reply_selection(
        &mut ui,
        &mailbox,
        key.clone(),
        start,
        SelectionChangedState::None,
    );
    ui.tick_copy_mode();

    assert!(ui.capture_copy_mode_key(&KeyEvent::press(KeyCode::Char('y'), Modifiers::NONE,)));
    for _ in 0..2 {
        let (same, command, action) = take_selection(&mut ui, &mailbox);
        assert!(matches!(
            action,
            TerminalViewerAction::SelectionGesture {
                gesture: ViewerSelectionGestureAction::Update { .. }
                    | ViewerSelectionGestureAction::End { .. },
                ..
            }
        ));
        reply_selection(
            &mut ui,
            &mailbox,
            same,
            command,
            SelectionChangedState::None,
        );
    }
    ui.tick_copy_mode();

    let view = ui.copy_mode_view().unwrap();
    assert_eq!(
        view.status,
        crate::terminal_copy_mode::CopyModeStatus::Ready
    );
    assert!(!view.selecting);
    assert_eq!(
        ui.view().notice(),
        Some(super::super::NativeSessionNotice::CopyModeNoSelection)
    );
    assert!(mailbox.try_recv().unwrap().is_none(), "no export command");
}

#[test]
fn a_pre_entry_pointer_selection_is_preserved_but_cannot_be_exported_by_copy_mode() {
    let (mut ui, mailbox, _, _) = ready_ui();
    install_projection(&mut ui, Some((0, true, 24)), 0);
    let attachment = *ui.projections[0].value.attachment();
    let generation = ui.projections[0].value.frame().unwrap().route.generation;
    let mut selection = terminal_selection::Driver::new(attachment);
    selection.observe_generation(generation).unwrap();
    selection
        .changed(SelectionChanged {
            attachment_id: attachment,
            state: active_selection(),
        })
        .unwrap();
    ui.projections[0].selection = Some(selection);

    ui.enter_copy_mode();
    assert!(ui.capture_copy_mode_key(&KeyEvent::press(KeyCode::Char('y'), Modifiers::NONE,)));
    assert_eq!(
        ui.view().notice(),
        Some(super::super::NativeSessionNotice::CopyModeNoSelection)
    );
    assert!(mailbox.try_recv().unwrap().is_none());
    assert!(
        ui.projections[0]
            .selection
            .as_ref()
            .unwrap()
            .host_selection_active()
    );
}

#[test]
fn copy_exit_replaces_its_owned_selection_then_cancels_the_cleanup_gesture() {
    let (mut ui, mailbox, _, _) = ready_ui();
    install_projection(&mut ui, Some((0, true, 24)), 0);
    ui.enter_copy_mode();
    assert!(ui.capture_copy_mode_key(&KeyEvent::press(KeyCode::Char('v'), Modifiers::NONE,)));
    let (key, start, _) = take_selection(&mut ui, &mailbox);
    reply_selection(&mut ui, &mailbox, key, start, SelectionChangedState::None);
    ui.tick_copy_mode();

    assert!(ui.capture_copy_mode_key(&KeyEvent::press(KeyCode::Esc, Modifiers::NONE,)));
    assert!(!ui.copy_mode_active());
    let (key, clear, action) = take_selection(&mut ui, &mailbox);
    assert!(matches!(
        action,
        TerminalViewerAction::SelectionGesture {
            gesture: ViewerSelectionGestureAction::Start {
                click_count: 1,
                point,
                select_output: false,
                ..
            },
            ..
        } if point.column == 0 && point.row == 0
    ));
    reply_selection(&mut ui, &mailbox, key, clear, SelectionChangedState::None);
    let (_, _, action) = take_selection(&mut ui, &mailbox);
    assert!(matches!(
        action,
        TerminalViewerAction::SelectionGesture {
            gesture: ViewerSelectionGestureAction::Cancel,
            ..
        }
    ));
}

#[test]
fn copy_entry_places_viewport_enter_after_an_already_admitted_pointer_start_or_update() {
    for update_in_flight in [false, true] {
        let (mut ui, mailbox, _, _) = ready_ui();
        let key = install_projection(&mut ui, Some((0, false, 24)), 0);
        assert_eq!(
            ui.pointer_selection(
                &key,
                PointerSelectionIntent::Start {
                    click_count: 1,
                    cell: PointerSelectionCell { column: 1, row: 0 },
                },
            ),
            PointerSelectionAdmission::Admitted,
        );
        if update_in_flight {
            let (same, start, action) = take_selection(&mut ui, &mailbox);
            assert!(matches!(
                action,
                TerminalViewerAction::SelectionGesture {
                    gesture: ViewerSelectionGestureAction::Start { .. },
                    ..
                }
            ));
            reply_selection(&mut ui, &mailbox, same, start, SelectionChangedState::None);
            assert_eq!(
                ui.pointer_selection(
                    &key,
                    PointerSelectionIntent::Update {
                        cell: PointerSelectionCell { column: 2, row: 0 },
                    },
                ),
                PointerSelectionAdmission::Admitted,
            );
        }

        assert_eq!(
            ui.enter_copy_mode(),
            crate::client_view::SessionActionDisposition::DismissNavigator
        );
        assert_eq!(
            ui.copy_mode_view().unwrap().status,
            crate::terminal_copy_mode::CopyModeStatus::Entering
        );
        let (same, flight, action) = take_selection(&mut ui, &mailbox);
        assert_eq!(same, key);
        assert_eq!(
            matches!(
                action,
                TerminalViewerAction::SelectionGesture {
                    gesture: ViewerSelectionGestureAction::Update { .. },
                    ..
                }
            ),
            update_in_flight,
        );
        assert!(
            mailbox.try_recv().unwrap().is_none(),
            "viewport admission must wait for the queued cancellation"
        );
        reply_selection(&mut ui, &mailbox, same, flight, SelectionChangedState::None);
        ui.tick_copy_mode();

        let (_, _, cancel) = take_selection(&mut ui, &mailbox);
        assert!(matches!(
            cancel,
            TerminalViewerAction::SelectionGesture {
                gesture: ViewerSelectionGestureAction::Cancel,
                ..
            }
        ));
        let viewport = mailbox.try_recv().unwrap().expect("viewport enter");
        assert!(matches!(
            viewport.record(),
            QueuedCommand::Viewport(tagged) if tagged.key() == &key && tagged.value() == &ViewportStep::Enter
        ));
        mailbox.finish_lease(viewport).unwrap();
    }
}

#[test]
fn rejected_current_start_releases_copy_entry_but_an_old_generation_reply_does_not() {
    let (mut ui, mailbox, _, _) = ready_ui();
    let key = install_projection(&mut ui, Some((9, false, 24)), 0);
    assert_eq!(
        ui.pointer_selection(
            &key,
            PointerSelectionIntent::Start {
                click_count: 1,
                cell: PointerSelectionCell { column: 1, row: 0 },
            },
        ),
        PointerSelectionAdmission::Admitted,
    );
    let (same, old_start, _) = take_selection(&mut ui, &mailbox);
    assert_eq!(same, key);

    let mut frame = ui.projections[0].value.frame().unwrap().clone();
    frame.route.sequence = identity(2);
    frame.route.generation = identity(2);
    frame.route.revision = 2;
    ui.projections[0].value.install_frame_for_test(frame);
    assert_eq!(
        ui.pointer_selection(
            &key,
            PointerSelectionIntent::Start {
                click_count: 1,
                cell: PointerSelectionCell { column: 2, row: 0 },
            },
        ),
        PointerSelectionAdmission::Admitted,
    );
    let (same, current_start, _) = take_selection(&mut ui, &mailbox);
    assert_eq!(same, key);

    assert_eq!(
        ui.enter_copy_mode(),
        crate::client_view::SessionActionDisposition::DismissNavigator
    );
    assert_eq!(
        ui.copy_mode_view().unwrap().status,
        crate::terminal_copy_mode::CopyModeStatus::Entering
    );
    assert!(!reject_selection(&mut ui, &mailbox, key.clone(), old_start));
    assert!(
        mailbox.try_recv().unwrap().is_none(),
        "an old generation reply cannot release the current cleanup boundary"
    );
    assert_eq!(
        ui.copy_mode_view().unwrap().status,
        crate::terminal_copy_mode::CopyModeStatus::Entering
    );

    assert!(reject_selection(
        &mut ui,
        &mailbox,
        key.clone(),
        current_start
    ));
    assert_eq!(
        ui.view().notice(),
        Some(super::super::NativeSessionNotice::CopyFailed)
    );
    assert!(mailbox.try_recv().unwrap().is_none());
    ui.tick_copy_mode();
    let enter = mailbox.try_recv().unwrap().expect("enter after rejection");
    assert!(matches!(
        enter.record(),
        QueuedCommand::Viewport(tagged)
            if tagged.key() == &key && tagged.value() == &ViewportStep::Enter
    ));
    mailbox.finish_lease(enter).unwrap();
}

#[test]
fn copy_entry_preserves_a_pointer_exports_queued_cancellation_behind_its_flight() {
    let (mut ui, mailbox, _, _) = ready_ui();
    let key = install_projection(&mut ui, Some((0, true, 24)), 0);
    assert_eq!(
        ui.pointer_selection(
            &key,
            PointerSelectionIntent::Start {
                click_count: 1,
                cell: PointerSelectionCell { column: 1, row: 0 },
            },
        ),
        PointerSelectionAdmission::Admitted,
    );
    let (same, start, _) = take_selection(&mut ui, &mailbox);
    reply_selection(&mut ui, &mailbox, same, start, SelectionChangedState::None);
    assert_eq!(
        ui.pointer_selection(
            &key,
            PointerSelectionIntent::End {
                cell: PointerSelectionCell { column: 4, row: 0 },
            },
        ),
        PointerSelectionAdmission::Admitted,
    );
    for _ in 0..2 {
        let (same, command, _) = take_selection(&mut ui, &mailbox);
        reply_selection(&mut ui, &mailbox, same, command, active_selection());
    }
    let (same, export, action) = take_selection(&mut ui, &mailbox);
    let TerminalViewerAction::SelectionExport {
        export_sequence, ..
    } = action
    else {
        panic!("selection export")
    };

    assert_eq!(
        ui.pointer_selection(
            &key,
            PointerSelectionIntent::Start {
                click_count: 1,
                cell: PointerSelectionCell { column: 2, row: 0 },
            },
        ),
        PointerSelectionAdmission::Admitted,
    );
    assert_eq!(
        ui.enter_copy_mode(),
        crate::client_view::SessionActionDisposition::DismissNavigator
    );
    assert_eq!(
        ui.copy_mode_view().unwrap().status,
        crate::terminal_copy_mode::CopyModeStatus::Entering
    );

    let attachment = *ui.projections[0].value.attachment();
    reply_export_control(
        &mut ui,
        &mailbox,
        same,
        export,
        SelectionExportControl::StartedResponse(SelectionExportStarted {
            attachment_id: attachment,
            export_sequence,
            byte_length: 4,
            format: SelectionExportFormat::Plain,
        }),
    );
    let (same, cancel_export, action) = take_selection(&mut ui, &mailbox);
    assert!(matches!(
        action,
        TerminalViewerAction::SelectionExportCancel {
            export_sequence: observed,
            ..
        } if observed == export_sequence
    ));
    reply_export_control(
        &mut ui,
        &mailbox,
        same,
        cancel_export,
        SelectionExportControl::FinishedResponse(SelectionExportFinished {
            attachment_id: attachment,
            export_sequence,
            byte_length: 0,
            status: SelectionExportStatus::Cancelled,
        }),
    );
    let (_, _, action) = take_selection(&mut ui, &mailbox);
    assert!(matches!(
        action,
        TerminalViewerAction::SelectionGesture {
            gesture: ViewerSelectionGestureAction::Cancel,
            ..
        }
    ));
}

#[test]
fn ordinary_input_waits_for_copy_cleanup_admission_then_recovers_before_forwarding() {
    let (mut ui, mailbox, _, _) = controlling_ui();
    let key = install_projection(&mut ui, Some((9, false, 24)), 0);
    ui.enter_copy_mode();
    let enter = mailbox.try_recv().unwrap().expect("copy viewport enter");
    assert!(matches!(
        enter.record(),
        QueuedCommand::Viewport(tagged) if tagged.key() == &key && tagged.value() == &ViewportStep::Enter
    ));
    mailbox.finish_lease(enter).unwrap();
    settle_viewing(&mut ui, &mailbox, &key, 1);
    ui.tick_copy_mode();
    assert_eq!(
        ui.copy_mode_view().unwrap().status,
        crate::terminal_copy_mode::CopyModeStatus::Ready
    );

    assert!(ui.capture_copy_mode_key(&KeyEvent::press(KeyCode::Char('v'), Modifiers::NONE,)));
    let (same, start, _) = take_selection(&mut ui, &mailbox);
    assert_eq!(same, key);
    assert!(ui.capture_copy_mode_key(&KeyEvent::press(KeyCode::Esc, Modifiers::NONE,)));
    assert!(
        !ui.forward(b"a"),
        "the first byte is dropped behind the fence"
    );
    assert!(mailbox.try_recv().unwrap().is_none());

    reply_selection(&mut ui, &mailbox, same, start, SelectionChangedState::None);
    let (same, clear, action) = take_selection(&mut ui, &mailbox);
    assert!(matches!(
        action,
        TerminalViewerAction::SelectionGesture {
            gesture: ViewerSelectionGestureAction::Start { .. },
            ..
        }
    ));
    reply_selection(&mut ui, &mailbox, same, clear, SelectionChangedState::None);
    let (_, _, cancel) = take_selection(&mut ui, &mailbox);
    assert!(matches!(
        cancel,
        TerminalViewerAction::SelectionGesture {
            gesture: ViewerSelectionGestureAction::Cancel,
            ..
        }
    ));

    assert!(ui.forward(b"b"));
    let recover = mailbox.try_recv().unwrap().expect("recover after cleanup");
    assert!(matches!(
        recover.record(),
        QueuedCommand::Viewport(tagged) if tagged.key() == &key && tagged.value() == &ViewportStep::Recover
    ));
    mailbox.finish_lease(recover).unwrap();
    let input = mailbox.try_recv().unwrap().expect("input after recover");
    assert!(matches!(
        input.record(),
        QueuedCommand::Input(tagged) if tagged.key() == &key && tagged.value().as_bytes() == b"b"
    ));
    mailbox.finish_lease(input).unwrap();
}

#[test]
fn rejected_early_cleanup_reports_failure_then_releases_staged_recovery() {
    let (mut ui, mailbox, _, _) = controlling_ui();
    let key = install_projection(&mut ui, Some((9, false, 24)), 0);
    ui.enter_copy_mode();
    let enter = mailbox.try_recv().unwrap().expect("copy viewport enter");
    assert!(matches!(
        enter.record(),
        QueuedCommand::Viewport(tagged)
            if tagged.key() == &key && tagged.value() == &ViewportStep::Enter
    ));
    mailbox.finish_lease(enter).unwrap();
    settle_viewing(&mut ui, &mailbox, &key, 1);
    ui.tick_copy_mode();

    assert!(ui.capture_copy_mode_key(&KeyEvent::press(KeyCode::Char('v'), Modifiers::NONE,)));
    let (same, start, _) = take_selection(&mut ui, &mailbox);
    reply_selection(&mut ui, &mailbox, same, start, SelectionChangedState::None);
    ui.tick_copy_mode();
    assert!(ui.capture_copy_mode_key(&KeyEvent::press(KeyCode::Esc, Modifiers::NONE,)));
    let (same, clear, action) = take_selection(&mut ui, &mailbox);
    assert!(matches!(
        action,
        TerminalViewerAction::SelectionGesture {
            gesture: ViewerSelectionGestureAction::Start { .. },
            ..
        }
    ));
    assert!(
        mailbox.try_recv().unwrap().is_none(),
        "recovery remains staged behind the cleanup boundary"
    );

    assert!(reject_selection(&mut ui, &mailbox, same, clear));
    assert_eq!(
        ui.view().notice(),
        Some(super::super::NativeSessionNotice::CopyFailed)
    );
    assert!(mailbox.try_recv().unwrap().is_none());

    assert!(ui.forward(b"b"));
    let recover = mailbox
        .try_recv()
        .unwrap()
        .expect("recover after rejection");
    assert!(matches!(
        recover.record(),
        QueuedCommand::Viewport(tagged)
            if tagged.key() == &key && tagged.value() == &ViewportStep::Recover
    ));
    mailbox.finish_lease(recover).unwrap();
    let input = mailbox.try_recv().unwrap().expect("input after recover");
    assert!(matches!(
        input.record(),
        QueuedCommand::Input(tagged)
            if tagged.key() == &key && tagged.value().as_bytes() == b"b"
    ));
    mailbox.finish_lease(input).unwrap();
}

#[test]
fn reveal_of_the_final_retained_row_uses_copy_newest_instead_of_live_recovery() {
    let surface =
        Surface::checked(TerminalScreen::Primary, identity(11), 13, 8, 40, 20, 4).unwrap();
    assert_eq!(
        copy_viewport_nav(
            surface,
            Navigation::Reveal(TerminalTextPoint { row: 39, col: 1 })
        ),
        Some(ViewportNav::Newest)
    );
}

#[test]
fn entering_projects_no_cursor_and_cancellation_releases_the_episode() {
    let (mut ui, _, _, _) = ready_ui();
    install_projection(&mut ui, Some((0, false, 24)), 0);
    ui.enter_copy_mode();
    let view = ui.copy_mode_view().unwrap();
    assert_eq!(
        view.status,
        crate::terminal_copy_mode::CopyModeStatus::Entering
    );
    assert_eq!(view.cursor, None);
    assert!(ui.cancel_copy_mode());
    assert!(!ui.copy_mode_active());
}

#[test]
fn clipboard_effect_identity_is_exact_across_episode_generation_and_export() {
    let mut coordinator = coordinator();
    let pane = pane();
    let episode = EpisodeId::new(identity(7));
    let generation = identity(11);
    let surface = Surface::checked(TerminalScreen::Primary, generation, 13, 8, 24, 20, 4).unwrap();
    let mut mode = Mode::new(episode, surface, TerminalTextPoint { row: 22, col: 2 }).unwrap();
    mode.toggle_selection().unwrap();
    mode.take_effect();
    mode.selection_settled(true).unwrap();
    mode.request_copy(true).unwrap();
    mode.take_effect();
    let export = identity(17);
    mode.export_ready(export).unwrap();
    coordinator.episode = Some(Episode {
        key: pane.clone(),
        id: episode,
        source: None,
        state: EpisodeState::Active(mode),
        pending_query: None,
    });

    assert!(coordinator.effect_current(&pane, generation, export, episode.get()));
    assert!(!coordinator.effect_current(&pane, generation, identity(18), episode.get()));
    assert!(!coordinator.effect_current(&pane, identity(12), export, episode.get()));
    assert!(!coordinator.effect_current(&pane, generation, export, identity(8)));
}

#[test]
fn motion_and_valid_search_read_limits_report_query_failure_not_draft_limit() {
    for failure in [
        QueryFailure::LimitExceeded,
        QueryFailure::PatternTooComplex,
        QueryFailure::TransportUnavailable,
    ] {
        assert_eq!(
            query_failure_notice(failure),
            SessionNotice::CopyModeQueryFailed
        );
    }
}

#[test]
fn raw_forward_search_and_carriage_return_submit_without_pane_input() {
    let (mut ui, mailbox, _, _) = ready_ui();
    install_projection(&mut ui, Some((0, true, 24)), 0);
    ui.set_agent_board_origin(crate::local_client::test_server::disconnected_origin());
    assert_eq!(
        ui.enter_copy_mode(),
        crate::client_view::SessionActionDisposition::DismissNavigator
    );

    let mut decoder = LegacyDecoder::default();
    let now = Instant::now();
    let mut captured = Vec::new();
    for byte in b"/foo\r" {
        for item in decoder.push(*byte, now, false) {
            captured.extend_from_slice(&item.raw);
            let DecodedKind::Key(event) = item.kind else {
                panic!("search input must decode to one typed key per packet")
            };
            assert!(ui.capture_copy_mode_key(&event));
        }
    }

    assert_eq!(captured, b"/foo\r");
    let view = ui.copy_mode_view().unwrap();
    assert_eq!(
        view.status,
        crate::terminal_copy_mode::CopyModeStatus::Querying
    );
    assert!(view.search.is_none(), "Enter must close the search draft");
    let pending = ui
        .copy_mode
        .episode
        .as_ref()
        .and_then(|episode| episode.pending_query.as_ref())
        .expect("one exact search query flight");
    assert_eq!(pending.search.as_deref(), Some("foo"));
    assert!(
        mailbox.try_recv().unwrap().is_none(),
        "no pane input command"
    );
    ui.release_agent_lane();
}

#[test]
fn raw_ctrl_h_del_and_tab_follow_inline_search_editing() {
    let (mut ui, mailbox, _, _) = ready_ui();
    install_projection(&mut ui, Some((0, true, 24)), 0);
    assert_eq!(
        ui.enter_copy_mode(),
        crate::client_view::SessionActionDisposition::DismissNavigator
    );

    let mut decoder = LegacyDecoder::default();
    let now = Instant::now();
    for byte in b"/ab\x08c\t\x7f" {
        for item in decoder.push(*byte, now, false) {
            let DecodedKind::Key(event) = item.kind else {
                panic!("inline search bytes must decode to typed keys")
            };
            assert!(ui.capture_copy_mode_key(&event));
        }
    }

    let search = ui.copy_mode_view().unwrap().search.unwrap();
    assert_eq!(search.draft, "a");
    assert_eq!(search.cursor, 1);
    assert!(
        mailbox.try_recv().unwrap().is_none(),
        "no pane input command"
    );
    assert!(ui.cancel_copy_mode());
}

struct PrimaryRefresh {
    ui: NativeUi,
    mailbox: Arc<CommandMailbox>,
    key: PaneRef,
    sequence: NonZeroU64,
    prior: Surface,
}

/// Exercise the actual committed read and installed-window acknowledgement
/// handshake that ordinary output triggers, rather than setting a copy flag.
fn primary_refresh(searching: bool) -> PrimaryRefresh {
    let (mut ui, mailbox, _, _) = ready_ui();
    let key = install_projection(&mut ui, Some((9, false, 24)), 0);
    ui.enter_copy_mode();
    settle_viewing(&mut ui, &mailbox, &key, 1);
    ui.tick_copy_mode();
    let prior = ui.copy_mode.mode().unwrap().surface();
    assert_eq!(
        ui.copy_mode_view().unwrap().status,
        crate::terminal_copy_mode::CopyModeStatus::Ready
    );
    if searching {
        assert!(ui.capture_copy_mode_key(&KeyEvent::press(KeyCode::Char('/'), Modifiers::NONE)));
        assert!(ui.capture_copy_mode_paste(Some("retained draft")));
    }

    let projection = &mut ui.projections[0];
    let committed = projection
        .viewport
        .as_ref()
        .unwrap()
        .committed_window()
        .unwrap();
    let mut window = crate::terminal_session::terminal_viewport::ViewportWindow {
        graphics_present: false,
        route: committed.route,
        foreground: committed.foreground,
        background: committed.background,
        rows: committed.rows.clone(),
    };
    let changed = HistoryChanged {
        attachment_id: *projection.value.attachment(),
        sequence: identity(2),
        generation: identity(1),
        revision: 2,
        anchor_status: HistoryAnchorStatus::Valid,
        total_rows: prior.total_rows + 5,
    };
    projection.viewport.as_mut().unwrap().facts(changed);
    ui.admit_viewport_steps(&mailbox, &key);
    let lease = mailbox.try_recv().unwrap().expect("output refresh read");
    let QueuedCommand::Viewport(tagged) = lease.record() else {
        panic!("refresh must read the viewport")
    };
    let ViewportStep::Read {
        request_sequence,
        origin,
        generation,
        revision,
        ..
    } = *tagged.value()
    else {
        panic!("committed refresh read")
    };
    assert_eq!(
        origin,
        orchbee_client_server_protocol::ViewportRowOrigin::Committed { delta: 0 }
    );
    assert_eq!(generation, identity(1));
    assert_eq!(revision, 2);
    mailbox.finish_lease(lease).unwrap();
    assert!(mailbox.try_recv().unwrap().is_none());

    window.route.sequence = request_sequence;
    window.route.revision = 2;
    window.route.total_rows += 5;
    window.route.start_row += 2;
    ui.projections[0]
        .viewport
        .as_mut()
        .unwrap()
        .window(window, Instant::now());
    assert!(
        ui.projections[0]
            .viewport
            .as_ref()
            .unwrap()
            .committed_window()
            .is_none()
    );
    assert!(ui.tick_copy_mode());
    assert!(
        ui.copy_mode_active(),
        "ordinary output must preserve the copy episode"
    );
    assert_eq!(
        ui.copy_mode_view().unwrap().status,
        crate::terminal_copy_mode::CopyModeStatus::Moving
    );
    assert_eq!(ui.copy_mode_view().unwrap().cursor, None);
    PrimaryRefresh {
        ui,
        mailbox,
        key,
        sequence: request_sequence,
        prior,
    }
}

#[test]
fn primary_output_refresh_waits_for_rows_applied_then_rebases_without_queued_input() {
    for searching in [false, true] {
        let mut fixture = primary_refresh(searching);
        let desired = fixture.ui.copy_mode.mode().unwrap().desired();
        let keys: &[KeyCode] = if searching {
            &[KeyCode::Enter]
        } else {
            &[
                KeyCode::Right,
                KeyCode::Char('v'),
                KeyCode::Char('y'),
                KeyCode::Enter,
                KeyCode::Char('/'),
                KeyCode::Char('w'),
            ]
        };
        for &code in keys {
            assert!(
                fixture
                    .ui
                    .capture_copy_mode_key(&KeyEvent::press(code, Modifiers::NONE))
            );
        }
        assert!(fixture.ui.capture_copy_mode_key(&KeyEvent {
            kind: KeyKind::Release,
            ..KeyEvent::press(KeyCode::Right, Modifiers::NONE)
        }));
        assert!(fixture.ui.capture_copy_mode_paste(Some(if searching {
            ""
        } else {
            "must not queue"
        })));
        assert_eq!(fixture.ui.copy_mode.mode().unwrap().desired(), desired);
        assert_eq!(
            fixture.ui.copy_mode.mode().unwrap().surface(),
            fixture.prior
        );
        assert!(
            fixture
                .ui
                .copy_mode
                .mode_mut()
                .unwrap()
                .take_effect()
                .is_none()
        );
        assert!(
            fixture
                .ui
                .copy_mode
                .episode
                .as_ref()
                .unwrap()
                .pending_query
                .is_none()
        );
        if searching {
            assert_eq!(
                fixture.ui.copy_mode_view().unwrap().search.unwrap().draft,
                "retained draft"
            );
        }
        assert!(
            fixture.mailbox.try_recv().unwrap().is_none(),
            "captured input must not queue a pane, query or selection command"
        );

        fixture
            .ui
            .admit_viewport_steps(&fixture.mailbox, &fixture.key);
        let lease = fixture
            .mailbox
            .try_recv()
            .unwrap()
            .expect("RowsApplied acknowledgement");
        assert!(
            matches!(lease.record(), QueuedCommand::Viewport(tagged) if matches!(tagged.value(), ViewportStep::Ack { request_sequence, generation, revision } if *request_sequence == fixture.sequence && *generation == identity(1) && *revision == 2))
        );
        fixture.mailbox.finish_lease(lease).unwrap();
        assert!(fixture.mailbox.try_recv().unwrap().is_none());
        fixture.ui.projections[0]
            .viewport
            .as_mut()
            .unwrap()
            .ack_outcome(fixture.sequence, true);
        assert!(fixture.ui.tick_copy_mode());
        let view = fixture.ui.copy_mode_view().unwrap();
        assert_eq!(
            view.status,
            crate::terminal_copy_mode::CopyModeStatus::Ready
        );
        assert_eq!(
            view.cursor,
            Some(crate::terminal_copy_mode::CopyModeCursor { column: 0, row: 23 })
        );
        let surface = fixture.ui.copy_mode.mode().unwrap().surface();
        assert_eq!(surface.revision, 2);
        assert_eq!(surface.start_row, fixture.prior.start_row + 2);
        assert_eq!(surface.total_rows, fixture.prior.total_rows + 5);
        assert_eq!(
            fixture.ui.copy_mode.mode().unwrap().desired().row,
            desired.row + 2
        );
        assert!(
            fixture
                .ui
                .copy_mode
                .mode_mut()
                .unwrap()
                .take_effect()
                .is_none()
        );
        assert!(
            fixture.mailbox.try_recv().unwrap().is_none(),
            "acknowledgement must not replay captured keys"
        );
    }
}

#[test]
fn primary_pending_output_refresh_keeps_structural_invalidation_authoritative() {
    for invalidation in ["generation", "geometry", "anchor"] {
        let mut fixture = primary_refresh(false);
        fixture
            .ui
            .admit_viewport_steps(&fixture.mailbox, &fixture.key);
        let lease = fixture
            .mailbox
            .try_recv()
            .unwrap()
            .expect("in-flight RowsApplied");
        assert!(
            matches!(lease.record(), QueuedCommand::Viewport(tagged) if matches!(tagged.value(), ViewportStep::Ack { request_sequence, .. } if *request_sequence == fixture.sequence))
        );
        fixture.mailbox.finish_lease(lease).unwrap();
        let projection = &mut fixture.ui.projections[0];
        match invalidation {
            "geometry" => {
                projection
                    .viewport
                    .as_mut()
                    .unwrap()
                    .resize(81, 24, Instant::now());
            }
            _ => {
                projection.viewport.as_mut().unwrap().facts(HistoryChanged {
                    attachment_id: *projection.value.attachment(),
                    sequence: identity(3),
                    generation: identity(if invalidation == "generation" { 2 } else { 1 }),
                    revision: 3,
                    anchor_status: if invalidation == "anchor" {
                        HistoryAnchorStatus::Evicted
                    } else {
                        HistoryAnchorStatus::Valid
                    },
                    total_rows: fixture.prior.total_rows + 5,
                });
            }
        }
        assert!(fixture.ui.tick_copy_mode());
        assert!(
            !fixture.ui.copy_mode_active(),
            "{invalidation} must still cancel the pending episode"
        );
        assert_eq!(
            fixture.ui.view().notice(),
            Some(super::super::NativeSessionNotice::CopyModeInvalidated)
        );
        fixture.ui.projections[0]
            .viewport
            .as_mut()
            .unwrap()
            .ack_outcome(fixture.sequence, true);
        fixture.ui.tick_copy_mode();
        assert!(
            !fixture.ui.copy_mode_active(),
            "late acknowledgement must not restore a cancelled episode"
        );
    }
}

#[test]
fn primary_pending_output_refresh_allows_q_and_escape_to_exit() {
    for (searching, code) in [
        (false, KeyCode::Char('q')),
        (false, KeyCode::Esc),
        (true, KeyCode::Esc),
    ] {
        let mut fixture = primary_refresh(searching);
        assert!(
            fixture
                .ui
                .capture_copy_mode_key(&KeyEvent::press(code, Modifiers::NONE))
        );
        assert!(!fixture.ui.copy_mode_active());
        assert!(fixture.ui.copy_mode_view().is_none());
        assert!(!fixture.ui.copy_mode.surface_pending());
    }
}

#[test]
fn primary_pending_output_refresh_preserves_local_search_editing_without_submission() {
    let mut fixture = primary_refresh(true);
    let desired = fixture.ui.copy_mode.mode().unwrap().desired();
    assert!(
        fixture
            .ui
            .capture_copy_mode_key(&KeyEvent::press(KeyCode::Char('q'), Modifiers::NONE))
    );
    assert!(
        fixture.ui.copy_mode_active(),
        "q in the search draft is a printable character"
    );
    assert!(fixture.ui.capture_copy_mode_paste(Some("中文e\u{301}")));
    assert!(
        fixture
            .ui
            .capture_copy_mode_key(&KeyEvent::press(KeyCode::Backspace, Modifiers::NONE))
    );
    assert_eq!(
        fixture.ui.copy_mode_view().unwrap().search.unwrap().draft,
        "retained draftq中文"
    );
    assert!(
        fixture
            .ui
            .capture_copy_mode_key(&KeyEvent::press(KeyCode::Left, Modifiers::NONE))
    );
    assert!(
        fixture
            .ui
            .capture_copy_mode_key(&KeyEvent::press(KeyCode::Delete, Modifiers::NONE))
    );
    assert!(fixture.ui.capture_copy_mode_paste(Some("🧪")));
    assert_eq!(
        fixture.ui.copy_mode_view().unwrap().search.unwrap().draft,
        "retained draftq中🧪"
    );
    for event in [
        KeyEvent::press(KeyCode::Enter, Modifiers::NONE),
        KeyEvent::press(KeyCode::Char('m'), Modifiers::CTRL),
        KeyEvent {
            kind: KeyKind::Release,
            ..KeyEvent::press(KeyCode::Char('q'), Modifiers::NONE)
        },
    ] {
        assert!(fixture.ui.capture_copy_mode_key(&event));
    }
    assert!(fixture.ui.capture_copy_mode_paste(Some("\u{1b}")));
    let view = fixture.ui.copy_mode_view().unwrap();
    assert_eq!(
        view.status,
        crate::terminal_copy_mode::CopyModeStatus::Moving
    );
    assert_eq!(view.cursor, None);
    assert_eq!(view.search.unwrap().draft, "retained draftq中🧪");
    assert_eq!(fixture.ui.copy_mode.mode().unwrap().desired(), desired);
    assert!(
        fixture
            .ui
            .copy_mode
            .mode_mut()
            .unwrap()
            .take_effect()
            .is_none()
    );
    assert!(
        fixture
            .ui
            .copy_mode
            .episode
            .as_ref()
            .unwrap()
            .pending_query
            .is_none()
    );
    assert!(
        fixture.mailbox.try_recv().unwrap().is_none(),
        "local editing and refused submit must not enqueue any terminal effect"
    );
}
