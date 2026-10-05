use super::*;
use orchbee_terminal_protocol::{SelectionExportFormat, SelectionExportStatus, SelectionPoint};

fn driver() -> Driver {
    Driver::new(AttachmentId::parse("00000000000000000000000000000007").unwrap())
}
fn cell(column: u16) -> PointerSelectionCell {
    PointerSelectionCell { column, row: 0 }
}
fn changed(driver: &Driver) -> Reply {
    Reply::Changed(SelectionChanged {
        attachment_id: driver.attachment,
        state: SelectionChangedState::Active {
            start: SelectionPoint { column: 0, row: 0 },
            end: SelectionPoint { column: 3, row: 0 },
            rectangle: false,
            preview: None,
        },
    })
}
fn changed_none(driver: &Driver) -> Reply {
    Reply::Changed(SelectionChanged {
        attachment_id: driver.attachment,
        state: SelectionChangedState::None,
    })
}
fn accept(driver: &mut Driver) -> Command {
    let command = driver.pending().unwrap();
    driver.admitted(command).unwrap();
    command
}

fn establish_copy_selection(driver: &mut Driver, generation: NonZeroU64) {
    driver
        .semantic(generation, 8, 2, SemanticIntent::Start(cell(2)))
        .unwrap();
    let start = accept(driver);
    let response = changed_none(driver);
    driver.reply(start, response).unwrap();
    driver
        .semantic(generation, 8, 2, SemanticIntent::Update(cell(5)))
        .unwrap();
    let update = accept(driver);
    let response = changed(driver);
    driver.reply(update, response).unwrap();
    assert!(driver.copy_selection_owned());
}

fn export_with_replacement_start(copy_owned: bool) -> (Driver, NonZeroU64) {
    let mut driver = driver();
    let generation = NonZeroU64::new(4).unwrap();
    driver
        .pointer(
            generation,
            8,
            2,
            PointerSelectionIntent::Start {
                click_count: 1,
                cell: cell(1),
            },
        )
        .unwrap();
    let start = accept(&mut driver);
    let response = changed_none(&driver);
    driver.reply(start, response).unwrap();
    driver
        .pointer(
            generation,
            8,
            2,
            PointerSelectionIntent::End { cell: cell(5) },
        )
        .unwrap();
    for _ in 0..2 {
        let command = accept(&mut driver);
        let response = changed(&driver);
        driver.reply(command, response).unwrap();
    }
    let export = accept(&mut driver);
    assert!(matches!(export.action, Action::Export { .. }));
    if copy_owned {
        driver
            .semantic(generation, 8, 2, SemanticIntent::Start(cell(2)))
            .unwrap();
    } else {
        driver
            .pointer(
                generation,
                8,
                2,
                PointerSelectionIntent::Start {
                    click_count: 1,
                    cell: cell(2),
                },
            )
            .unwrap();
    }
    assert!(driver.pending().is_none(), "the export remains in flight");
    assert!(matches!(
        driver.queue[0].action,
        Action::CancelExport { .. }
    ));
    assert!(matches!(
        driver.queue[1].action,
        Action::Gesture {
            gesture: ViewerSelectionGestureAction::Start { .. },
            ..
        }
    ));
    (driver, generation)
}

#[test]
fn final_update_precedes_end_and_accepted_end_copies_only_completed_host_text() {
    let mut driver = driver();
    let generation = NonZeroU64::new(3).unwrap();
    assert_eq!(
        driver.pointer(
            generation,
            80,
            24,
            PointerSelectionIntent::Start {
                click_count: 1,
                cell: cell(0)
            }
        ),
        Ok(true)
    );
    let start = accept(&mut driver);
    driver
        .pointer(
            generation,
            80,
            24,
            PointerSelectionIntent::Update { cell: cell(1) },
        )
        .unwrap();
    driver
        .pointer(
            generation,
            80,
            24,
            PointerSelectionIntent::End { cell: cell(3) },
        )
        .unwrap();
    assert!(driver.pending().is_none());
    let response = changed(&driver);
    driver.reply(start, response).unwrap();
    let update = accept(&mut driver);
    assert!(
        matches!(update.action, Action::Gesture { generation: same, gesture: ViewerSelectionGestureAction::Update { point, .. } } if same == generation && point.column == 3)
    );
    let response = changed(&driver);
    driver.reply(update, response).unwrap();
    let end = accept(&mut driver);
    assert!(matches!(
        end.action,
        Action::Gesture {
            gesture: ViewerSelectionGestureAction::End { .. },
            ..
        }
    ));
    let response = changed(&driver);
    driver.reply(end, response).unwrap();
    let request = accept(&mut driver);
    let Action::Export { sequence } = request.action else {
        panic!("export after accepted End")
    };
    let attachment_id = driver.attachment;
    driver
        .chunk(SelectionExportChunk {
            attachment_id,
            export_sequence: sequence,
            chunk_sequence: 0,
            bytes: b"text".to_vec(),
            final_chunk: true,
        })
        .unwrap();
    driver
        .reply(
            request,
            Reply::Started(SelectionExportStarted {
                attachment_id,
                export_sequence: sequence,
                byte_length: 4,
                format: SelectionExportFormat::Plain,
            }),
        )
        .unwrap();
    let ack = accept(&mut driver);
    driver
        .finished(SelectionExportFinished {
            attachment_id,
            export_sequence: sequence,
            byte_length: 4,
            status: SelectionExportStatus::Completed,
        })
        .unwrap();
    assert!(driver.take_copy().is_none());
    driver.reply(ack, Reply::Ack).unwrap();
    assert_eq!(driver.take_copy(), Some((sequence, "text".into())));
    assert!(driver.take_copy().is_none());
}

#[test]
fn a_stale_generation_clears_work_and_preserves_host_selection_without_copy() {
    let mut driver = driver();
    let response = changed(&driver);
    let Reply::Changed(selection) = response else {
        unreachable!()
    };
    driver.changed(selection.clone()).unwrap();
    driver
        .pointer(
            NonZeroU64::MIN,
            80,
            24,
            PointerSelectionIntent::Start {
                click_count: 1,
                cell: cell(0),
            },
        )
        .unwrap();
    let start = accept(&mut driver);
    driver
        .pointer(
            NonZeroU64::MIN,
            80,
            24,
            PointerSelectionIntent::End { cell: cell(3) },
        )
        .unwrap();
    assert_eq!(
        driver.reply(
            start,
            Reply::Failed {
                stale_generation: true
            }
        ),
        Ok(true)
    );
    assert_eq!(driver.selection(), &selection.state);
    assert!(driver.gesture.is_none());
    assert!(driver.pending().is_none());
    assert!(driver.take_copy().is_none());
}

#[test]
fn keyboard_semantic_selection_uses_the_host_gesture_without_pointer_policy() {
    let mut driver = driver();
    let generation = NonZeroU64::new(4).unwrap();
    assert_eq!(
        driver.semantic(generation, 8, 2, SemanticIntent::Start(cell(2))),
        Ok(true)
    );
    assert!(!driver.gesture_settled());
    let start = accept(&mut driver);
    let response = changed_none(&driver);
    driver.reply(start, response).unwrap();
    assert!(driver.gesture_settled());
    assert!(driver.gesture_intent_active());
    assert!(!driver.host_selection_active());

    driver
        .semantic(generation, 8, 2, SemanticIntent::Update(cell(5)))
        .unwrap();
    assert!(!driver.gesture_settled());
    let update = accept(&mut driver);
    assert!(matches!(
        update.action,
        Action::Gesture {
            gesture: ViewerSelectionGestureAction::Update { point, .. },
            ..
        } if point.column == 5
    ));
    let response = changed_none(&driver);
    driver.reply(update, response).unwrap();
    assert!(driver.gesture_intent_active());
    assert!(!driver.host_selection_active());

    driver
        .semantic(generation, 8, 2, SemanticIntent::Update(cell(6)))
        .unwrap();
    let update = accept(&mut driver);
    let response = changed(&driver);
    driver.reply(update, response).unwrap();
    assert!(driver.gesture_intent_active());
    assert!(driver.host_selection_active());
}

#[test]
fn end_with_no_host_selection_settles_without_starting_an_export() {
    let mut driver = driver();
    let generation = NonZeroU64::new(4).unwrap();
    driver
        .semantic(generation, 8, 2, SemanticIntent::Start(cell(2)))
        .unwrap();
    let start = accept(&mut driver);
    let response = changed_none(&driver);
    driver.reply(start, response).unwrap();
    driver
        .semantic(generation, 8, 2, SemanticIntent::End(cell(2)))
        .unwrap();
    let update = accept(&mut driver);
    let response = changed_none(&driver);
    driver.reply(update, response).unwrap();
    let end = accept(&mut driver);
    let response = changed_none(&driver);
    driver.reply(end, response).unwrap();

    assert!(driver.gesture_settled());
    assert!(!driver.gesture_intent_active());
    assert!(!driver.host_selection_active());
    assert!(!driver.export_pending());
}

#[test]
fn viewport_fence_releases_when_the_exact_final_cancel_enters_the_mailbox() {
    let mut driver = driver();
    let generation = NonZeroU64::new(4).unwrap();
    driver
        .pointer(
            generation,
            8,
            2,
            PointerSelectionIntent::Start {
                click_count: 1,
                cell: cell(2),
            },
        )
        .unwrap();
    let start = accept(&mut driver);
    driver.cancel_before_viewport().unwrap();
    assert!(driver.viewport_blocked());
    assert!(
        driver.pending().is_none(),
        "Start remains the single flight"
    );

    let response = changed_none(&driver);
    driver.reply(start, response).unwrap();
    assert!(driver.viewport_blocked());
    let cancel = driver.pending().unwrap();
    assert!(matches!(
        cancel.action,
        Action::Gesture {
            gesture: ViewerSelectionGestureAction::Cancel,
            ..
        }
    ));
    driver.admitted(cancel).unwrap();
    assert!(
        !driver.viewport_blocked(),
        "mailbox admission, not Host success, releases ordering"
    );
}

#[test]
fn rejected_or_replaced_selection_work_never_leaves_a_viewport_fence_stuck() {
    let generation = NonZeroU64::new(4).unwrap();
    let mut rejected = driver();
    rejected
        .pointer(
            generation,
            8,
            2,
            PointerSelectionIntent::Start {
                click_count: 1,
                cell: cell(2),
            },
        )
        .unwrap();
    let start = accept(&mut rejected);
    rejected.cancel_before_viewport().unwrap();
    assert!(rejected.viewport_blocked());
    assert_eq!(
        rejected.reply(
            start,
            Reply::Failed {
                stale_generation: false,
            },
        ),
        Err(Failure::Rejected)
    );
    assert!(!rejected.viewport_blocked());

    let mut replaced = driver();
    replaced
        .pointer(
            generation,
            8,
            2,
            PointerSelectionIntent::Start {
                click_count: 1,
                cell: cell(2),
            },
        )
        .unwrap();
    replaced.cancel_before_viewport().unwrap();
    assert!(replaced.viewport_blocked());
    replaced
        .observe_generation(NonZeroU64::new(5).unwrap())
        .unwrap();
    assert!(!replaced.viewport_blocked());
}

#[test]
fn pointer_entry_cancel_preserves_a_queued_export_cancellation_at_the_cleanup_head() {
    let (mut driver, _) = export_with_replacement_start(false);
    let cancellation = driver.queue[0];

    driver.cancel_before_viewport().unwrap();
    assert_eq!(driver.queue[0], cancellation);
    assert_eq!(driver.queue.len(), 2);
    assert!(matches!(
        driver.queue[1].action,
        Action::Gesture {
            gesture: ViewerSelectionGestureAction::Cancel,
            ..
        }
    ));
    assert_eq!(driver.viewport_fence, Some(driver.queue[1].serial));
}

#[test]
fn generation_replacement_preserves_a_queued_export_cancellation() {
    let (mut driver, _) = export_with_replacement_start(false);
    let cancellation = driver.queue[0];

    driver
        .observe_generation(NonZeroU64::new(5).unwrap())
        .unwrap();
    assert_eq!(
        driver.queue.iter().copied().collect::<Vec<_>>(),
        [cancellation]
    );
    assert!(!driver.viewport_blocked());
    assert!(driver.flight.is_none());
}

#[test]
fn copy_owned_clear_and_stale_abandon_preserve_a_queued_export_cancellation() {
    let (mut clear, generation) = export_with_replacement_start(true);
    let cancellation = clear.queue[0];
    clear
        .cancel_copy_selection_before_viewport(generation, 8, 2, cell(0))
        .unwrap();
    assert_eq!(clear.queue[0], cancellation);
    assert!(matches!(
        clear.queue[1].action,
        Action::Gesture {
            gesture: ViewerSelectionGestureAction::Start { .. },
            ..
        }
    ));
    assert!(matches!(
        clear.queue[2].action,
        Action::Gesture {
            gesture: ViewerSelectionGestureAction::Cancel,
            ..
        }
    ));
    assert_eq!(clear.viewport_fence, Some(clear.queue[2].serial));

    let (mut abandon, _) = export_with_replacement_start(true);
    let cancellation = abandon.queue[0];
    abandon.abandon_copy_selection_before_viewport().unwrap();
    assert_eq!(abandon.queue[0], cancellation);
    assert!(matches!(
        abandon.queue[1].action,
        Action::Gesture {
            gesture: ViewerSelectionGestureAction::Cancel,
            ..
        }
    ));
    assert_eq!(abandon.viewport_fence, Some(abandon.queue[1].serial));
}

#[test]
fn copy_mode_exit_clears_a_settled_active_host_selection() {
    let mut driver = driver();
    let generation = NonZeroU64::new(4).unwrap();
    establish_copy_selection(&mut driver, generation);

    driver
        .cancel_copy_selection_before_viewport(generation, 8, 2, cell(0))
        .unwrap();
    assert!(driver.viewport_blocked());
    let clear = accept(&mut driver);
    assert!(matches!(
        clear.action,
        Action::Gesture {
            generation: observed,
            gesture: ViewerSelectionGestureAction::Start {
                click_count: 1,
                point,
                select_output: false,
                ..
            },
        } if observed == generation && point.column == 0 && point.row == 0
    ));
    assert!(driver.viewport_blocked());
    let response = changed_none(&driver);
    driver.reply(clear, response).unwrap();
    let cancel = driver.pending().unwrap();
    assert!(matches!(
        cancel.action,
        Action::Gesture {
            generation: observed,
            gesture: ViewerSelectionGestureAction::Cancel,
        } if observed == generation
    ));
    driver.admitted(cancel).unwrap();
    assert!(!driver.viewport_blocked());
    assert!(!driver.copy_selection_owned());
}

#[test]
fn entry_preserves_a_settled_pointer_selection_that_copy_mode_does_not_own() {
    let mut driver = driver();
    let generation = NonZeroU64::new(4).unwrap();
    driver.generation = Some(generation);
    let Reply::Changed(active) = changed(&driver) else {
        unreachable!()
    };
    let retained = active.state.clone();
    driver.changed(active).unwrap();

    driver.cancel_before_viewport().unwrap();
    assert!(!driver.copy_selection_owned());
    assert_eq!(driver.selection(), &retained);
    assert!(driver.pending().is_none());
    assert!(!driver.viewport_blocked());
}

#[test]
fn active_export_cleanup_is_cancel_export_then_clear_start_then_final_cancel() {
    let mut driver = driver();
    let generation = NonZeroU64::new(4).unwrap();
    establish_copy_selection(&mut driver, generation);
    driver
        .semantic(generation, 8, 2, SemanticIntent::End(cell(5)))
        .unwrap();
    for _ in 0..2 {
        let command = accept(&mut driver);
        let response = changed(&driver);
        driver.reply(command, response).unwrap();
    }
    let export = accept(&mut driver);
    assert!(matches!(export.action, Action::Export { .. }));

    driver
        .cancel_copy_selection_before_viewport(generation, 8, 2, cell(0))
        .unwrap();
    assert!(driver.viewport_blocked());
    assert!(
        driver.pending().is_none(),
        "the export remains the one flight"
    );
    assert!(matches!(
        driver.queue[0].action,
        Action::CancelExport { .. }
    ));
    assert!(matches!(
        driver.queue[1].action,
        Action::Gesture {
            gesture: ViewerSelectionGestureAction::Start { click_count: 1, .. },
            ..
        }
    ));
    assert!(matches!(
        driver.queue[2].action,
        Action::Gesture {
            gesture: ViewerSelectionGestureAction::Cancel,
            ..
        }
    ));
    assert_eq!(driver.viewport_fence, Some(driver.queue[2].serial));

    let retained = driver.queue.clone();
    let fence = driver.viewport_fence;
    driver.cancel_before_viewport().unwrap();
    assert_eq!(driver.queue, retained);
    assert_eq!(driver.viewport_fence, fence);
}

#[test]
fn explicit_retry_mints_a_fresh_export_sequence_without_replaying_bytes() {
    let mut driver = driver();
    let generation = NonZeroU64::new(4).unwrap();
    establish_copy_selection(&mut driver, generation);
    driver
        .semantic(generation, 8, 2, SemanticIntent::End(cell(5)))
        .unwrap();
    for _ in 0..2 {
        let command = accept(&mut driver);
        let response = changed(&driver);
        driver.reply(command, response).unwrap();
    }
    let first = accept(&mut driver);
    let Action::Export {
        sequence: first_sequence,
    } = first.action
    else {
        panic!("first export")
    };
    assert!(driver.export_pending());
    assert_eq!(
        driver.reply(
            first,
            Reply::Failed {
                stale_generation: false
            }
        ),
        Err(Failure::Rejected)
    );
    assert!(!driver.export_pending());
    assert!(driver.copy_selection_owned());
    assert_eq!(driver.export_active(), Ok(true));
    let second = driver.pending().unwrap();
    let Action::Export {
        sequence: second_sequence,
    } = second.action
    else {
        panic!("second export")
    };
    assert!(second_sequence > first_sequence);
    assert!(driver.take_copy().is_none());
}
