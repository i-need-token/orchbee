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
fn accept(driver: &mut Driver) -> Command {
    let command = driver.pending().unwrap();
    driver.admitted(command).unwrap();
    command
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
