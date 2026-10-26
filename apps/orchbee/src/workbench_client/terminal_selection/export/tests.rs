use super::*;
use orchbee_terminal_protocol::{SelectionExportFormat, SelectionExportStatus};

fn attachment() -> AttachmentId {
    AttachmentId::parse("00000000000000000000000000000007").unwrap()
}

fn sequence() -> NonZeroU64 {
    NonZeroU64::new(9).unwrap()
}

fn started(length: usize) -> SelectionExportStarted {
    SelectionExportStarted {
        attachment_id: attachment(),
        byte_length: u32::try_from(length).unwrap(),
        export_sequence: sequence(),
        format: SelectionExportFormat::Plain,
    }
}

fn chunk(bytes: &[u8], number: u32, final_chunk: bool) -> SelectionExportChunk {
    SelectionExportChunk {
        attachment_id: attachment(),
        bytes: bytes.to_vec(),
        chunk_sequence: number,
        export_sequence: sequence(),
        final_chunk,
    }
}

fn finished(length: usize) -> SelectionExportFinished {
    SelectionExportFinished {
        attachment_id: attachment(),
        byte_length: u32::try_from(length).unwrap(),
        export_sequence: sequence(),
        status: SelectionExportStatus::Completed,
    }
}

#[test]
fn complete_host_text_waits_for_final_ack_and_finish_then_is_taken_once() {
    let text = "中文 e\u{301}\nnext";
    let bytes = text.as_bytes();
    let mut export = Export::new(attachment(), sequence());
    assert_eq!(export.started(started(bytes.len())), Ok(None));
    assert_eq!(export.chunk(chunk(&bytes[..2], 0, false)), Ok(Some(0)));
    assert_eq!(export.take_text(), Ok(None));
    export.acknowledged(0).unwrap();
    assert_eq!(export.chunk(chunk(&bytes[2..], 1, true)), Ok(Some(1)));
    // Host can publish completion before its final acknowledgement response.
    export.finished(finished(bytes.len())).unwrap();
    assert_eq!(export.take_text(), Ok(None));
    export.acknowledged(1).unwrap();
    assert_eq!(export.take_text(), Ok(Some(text.into())));
    assert_eq!(export.take_text(), Ok(None));
}

#[test]
fn a_zero_based_early_chunk_is_acknowledged_only_after_the_start_response() {
    let mut export = Export::new(attachment(), sequence());
    assert_eq!(export.chunk(chunk(b"abc", 0, true)), Ok(None));
    assert_eq!(export.take_text(), Ok(None));
    assert_eq!(export.started(started(3)), Ok(Some(0)));
    export.acknowledged(0).unwrap();
    assert_eq!(export.take_text(), Ok(None));
    export.finished(finished(3)).unwrap();
    assert_eq!(export.take_text(), Ok(Some("abc".into())));
}

#[test]
fn partial_foreign_and_invalid_utf8_exports_never_produce_text() {
    let mut partial = Export::new(attachment(), sequence());
    partial.started(started(4)).unwrap();
    partial.chunk(chunk(b"abc", 0, false)).unwrap();
    partial.acknowledged(0).unwrap();
    assert_eq!(partial.finished(finished(4)), Err(Failure::Invalid));
    assert_eq!(partial.take_text(), Ok(None));

    let mut foreign = Export::new(attachment(), sequence());
    foreign.started(started(1)).unwrap();
    let mut value = chunk(b"x", 0, true);
    value.export_sequence = NonZeroU64::new(10).unwrap();
    assert_eq!(foreign.chunk(value), Err(Failure::Invalid));
    assert_eq!(foreign.take_text(), Ok(None));

    let mut invalid = Export::new(attachment(), sequence());
    invalid.started(started(1)).unwrap();
    invalid.chunk(chunk(&[0xff], 0, true)).unwrap();
    invalid.acknowledged(0).unwrap();
    invalid.finished(finished(1)).unwrap();
    assert_eq!(invalid.take_text(), Err(Failure::Invalid));
    assert_eq!(invalid.take_text(), Ok(None));
}

#[test]
fn cancel_drops_an_early_chunk_and_a_second_early_chunk_is_rejected() {
    let mut cancelled = Export::new(attachment(), sequence());
    cancelled.chunk(chunk(b"abc", 0, true)).unwrap();
    let mut value = finished(3);
    value.status = SelectionExportStatus::Cancelled;
    cancelled.finished(value).unwrap();
    assert_eq!(cancelled.take_text(), Ok(None));
    assert_eq!(cancelled.started(started(3)), Err(Failure::Invalid));

    let mut repeated = Export::new(attachment(), sequence());
    repeated.chunk(chunk(b"abc", 0, true)).unwrap();
    assert_eq!(
        repeated.chunk(chunk(b"abc", 0, true)),
        Err(Failure::Invalid)
    );
}
