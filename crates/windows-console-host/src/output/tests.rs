use super::{
    ConsoleWriteBoundary, ConsoleWriteError, ConsoleWriteErrorKind, ConsoleWriteProgress,
    MAX_UTF16_UNITS_PER_CALL, UnicodeWriter, Utf16Port, inclusive_extent,
};
use std::{
    cell::RefCell,
    collections::VecDeque,
    io::{self, Write},
    rc::Rc,
};

enum Step {
    Accept(usize),
    Fail(i32),
}

#[derive(Clone)]
struct FakePort {
    state: Rc<RefCell<FakeState>>,
}

#[derive(Default)]
struct FakeState {
    steps: VecDeque<Step>,
    calls: Vec<Vec<u16>>,
}

impl Utf16Port for FakePort {
    fn write_utf16(&mut self, units: &[u16]) -> io::Result<usize> {
        let mut state = self.state.borrow_mut();
        state.calls.push(units.to_vec());
        match state.steps.pop_front() {
            Some(Step::Accept(count)) => Ok(count),
            Some(Step::Fail(code)) => Err(io::Error::from_raw_os_error(code)),
            None => Ok(units.len()),
        }
    }
}

fn writer(steps: impl IntoIterator<Item = Step>) -> (UnicodeWriter<FakePort>, FakePort) {
    let port = FakePort {
        state: Rc::new(RefCell::new(FakeState {
            steps: steps.into_iter().collect(),
            calls: Vec::new(),
        })),
    };
    (UnicodeWriter::new(port.clone()), port)
}

fn calls(port: &FakePort) -> Vec<Vec<u16>> {
    port.state.borrow().calls.clone()
}

fn detail(error: &io::Error) -> &ConsoleWriteError {
    error
        .get_ref()
        .and_then(|source| source.downcast_ref::<ConsoleWriteError>())
        .expect("writer errors retain typed detail")
}

#[test]
fn full_writes_preserve_ascii_bmp_and_non_bmp_units() {
    let (mut writer, port) = writer([]);

    writer.write_all("Aλ😀".as_bytes()).unwrap();
    writer.flush().unwrap();

    assert_eq!(calls(&port), vec![vec![0x0041, 0x03bb, 0xd83d, 0xde00]]);
}

#[test]
fn scalar_boundary_short_write_returns_only_the_matching_utf8_prefix() {
    let (mut writer, port) = writer([Step::Accept(2), Step::Accept(1)]);
    let bytes = "AλB".as_bytes();

    assert_eq!(writer.write(bytes).unwrap(), "Aλ".len());
    writer.write_all(&bytes["Aλ".len()..]).unwrap();
    writer.flush().unwrap();

    assert_eq!(calls(&port), vec![vec![0x41, 0x03bb, 0x42], vec![0x42]]);
}

#[test]
fn split_surrogate_consumes_one_scalar_and_writes_only_the_low_unit_next() {
    let (mut writer, port) = writer([Step::Accept(1), Step::Accept(1), Step::Accept(1)]);
    let bytes = "😀Z".as_bytes();

    assert_eq!(writer.write(bytes).unwrap(), "😀".len());
    writer.write_all(&bytes["😀".len()..]).unwrap();
    writer.flush().unwrap();

    assert_eq!(
        calls(&port),
        vec![vec![0xd83d, 0xde00, 0x005a], vec![0xde00], vec![0x005a]]
    );
}

#[test]
fn every_utf8_split_reconstructs_the_same_utf16_stream() {
    let text = "Aλ😀中Z";
    let expected: Vec<u16> = text.encode_utf16().collect();

    for split in 1..text.len() {
        let (mut writer, port) = writer([]);
        writer.write_all(&text.as_bytes()[..split]).unwrap();
        writer.write_all(&text.as_bytes()[split..]).unwrap();
        writer.flush().unwrap();
        let actual: Vec<u16> = calls(&port).into_iter().flatten().collect();
        assert_eq!(actual, expected, "split at byte {split}");
    }
}

#[test]
fn incomplete_utf8_is_bounded_and_flush_faults_without_native_replay() {
    let (mut writer, port) = writer([]);
    let emoji = "😀".as_bytes();

    assert_eq!(writer.write(&emoji[..3]).unwrap(), 3);
    let error = writer.flush().unwrap_err();
    assert_eq!(detail(&error).kind(), ConsoleWriteErrorKind::IncompleteUtf8);
    assert_eq!(
        detail(&error).boundary_before(),
        ConsoleWriteBoundary::IncompleteUtf8 { bytes: 3 }
    );
    assert!(calls(&port).is_empty());

    assert!(writer.write(b"cleanup").is_err());
    assert!(writer.flush().is_err());
    assert!(calls(&port).is_empty());
}

#[test]
fn invalid_utf8_after_a_valid_prefix_never_replays_that_prefix() {
    let (mut writer, port) = writer([]);
    let error = writer.write_all(&[b'A', 0xff]).unwrap_err();

    assert_eq!(detail(&error).kind(), ConsoleWriteErrorKind::InvalidUtf8);
    assert_eq!(
        detail(&error).progress(),
        ConsoleWriteProgress::CompleteScalars {
            utf8_bytes: 1,
            utf16_units: 1,
        }
    );
    assert_eq!(calls(&port), vec![vec![u16::from(b'A')]]);
    assert!(writer.write(b"B").is_err());
    assert_eq!(calls(&port), vec![vec![u16::from(b'A')]]);
}

#[test]
fn native_failure_after_a_committed_prefix_never_replays_that_prefix() {
    let (mut writer, port) = writer([Step::Accept(1), Step::Fail(1234)]);

    let error = writer.write_all(b"AB").unwrap_err();
    assert_eq!(detail(&error).kind(), ConsoleWriteErrorKind::Native);
    assert_eq!(detail(&error).progress(), ConsoleWriteProgress::Unknown);
    assert_eq!(detail(&error).raw_os_error(), Some(1234));
    assert_eq!(calls(&port), vec![vec![0x41, 0x42], vec![0x42]]);

    assert!(writer.write(b"B").is_err());
    assert_eq!(calls(&port), vec![vec![0x41, 0x42], vec![0x42]]);
}

#[test]
fn zero_and_out_of_range_counts_latch_the_writer() {
    for (step, expected_kind) in [
        (Step::Accept(0), ConsoleWriteErrorKind::WriteZero),
        (
            Step::Accept(2),
            ConsoleWriteErrorKind::InvalidAcceptedCount {
                accepted: 2,
                offered: 1,
            },
        ),
    ] {
        let (mut writer, port) = writer([step]);
        let error = writer.write(b"A").unwrap_err();
        assert_eq!(detail(&error).kind(), expected_kind);
        assert!(writer.write(b"B").is_err());
        assert_eq!(calls(&port).len(), 1);
    }
}

#[test]
fn low_surrogate_failure_reports_the_known_split_and_never_retries() {
    let (mut writer, port) = writer([Step::Accept(1), Step::Fail(1234)]);

    assert_eq!(writer.write("😀".as_bytes()).unwrap(), 4);
    let error = writer.flush().unwrap_err();
    let detail = detail(&error);
    assert_eq!(detail.kind(), ConsoleWriteErrorKind::Native);
    assert_eq!(detail.progress(), ConsoleWriteProgress::Unknown);
    assert_eq!(
        detail.boundary_before(),
        ConsoleWriteBoundary::PendingLowSurrogate {
            pending_low: 0xde00,
        }
    );
    assert_eq!(detail.raw_os_error(), Some(1234));

    assert!(writer.flush().is_err());
    assert_eq!(calls(&port), vec![vec![0xd83d, 0xde00], vec![0xde00]]);
}

#[test]
fn zero_while_draining_a_low_surrogate_retains_typed_split_progress() {
    let (mut writer, _) = writer([Step::Accept(1), Step::Accept(0)]);

    assert_eq!(writer.write("😀".as_bytes()).unwrap(), 4);
    let error = writer.flush().unwrap_err();
    assert_eq!(detail(&error).kind(), ConsoleWriteErrorKind::WriteZero);
    assert_eq!(
        detail(&error).progress(),
        ConsoleWriteProgress::SplitScalar {
            complete_utf8_bytes: 0,
            accepted_utf16_units: 1,
            pending_low: 0xde00,
        }
    );
}

#[test]
fn one_native_call_never_exceeds_the_utf16_bound() {
    let (mut writer, port) = writer([]);
    let bytes = vec![b'x'; MAX_UTF16_UNITS_PER_CALL + 19];

    writer.write_all(&bytes).unwrap();
    writer.flush().unwrap();

    let calls = calls(&port);
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].len(), MAX_UTF16_UNITS_PER_CALL);
    assert_eq!(calls[1].len(), 19);
    assert!(
        calls
            .iter()
            .all(|call| call.len() <= MAX_UTF16_UNITS_PER_CALL)
    );
}

#[test]
fn a_fresh_writer_on_the_same_port_can_attempt_ascii_cleanup() {
    let (mut failed, port) = writer([Step::Accept(0), Step::Accept(3)]);
    assert!(failed.write(b"frame").is_err());

    let mut cleanup = UnicodeWriter::new(port.clone());
    cleanup.write_all(b"end").unwrap();
    cleanup.flush().unwrap();

    assert_eq!(
        calls(&port),
        vec![
            b"frame".iter().map(|byte| u16::from(*byte)).collect(),
            vec![0x65, 0x6e, 0x64]
        ]
    );
    assert!(failed.write(b"later-frame").is_err());
    assert_eq!(calls(&port).len(), 2);
}

#[test]
fn inclusive_console_extents_preserve_origin_and_reject_unrepresentable_span() {
    assert_eq!(inclusive_extent(4, 4).unwrap(), 1);
    assert_eq!(inclusive_extent(-3, 6).unwrap(), 10);
    assert_eq!(inclusive_extent(7, 6).unwrap(), 0);
    assert_eq!(
        inclusive_extent(i16::MIN, i16::MAX).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
}
