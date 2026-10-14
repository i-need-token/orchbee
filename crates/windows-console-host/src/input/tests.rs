use super::{InputPort, InputRecord, KeyRecord, read_ready_record};
use std::{collections::VecDeque, io};

#[derive(Default)]
struct FakeInput {
    counts: VecDeque<io::Result<u32>>,
    reads: VecDeque<io::Result<(InputRecord, u32)>>,
    read_calls: usize,
}

impl InputPort for FakeInput {
    fn ready_count(&mut self) -> io::Result<u32> {
        self.counts
            .pop_front()
            .unwrap_or_else(|| Err(io::Error::other("unexpected readiness query")))
    }

    fn read_one(&mut self) -> io::Result<(InputRecord, u32)> {
        self.read_calls += 1;
        self.reads
            .pop_front()
            .unwrap_or_else(|| Err(io::Error::other("unexpected record read")))
    }
}

fn key_record() -> InputRecord {
    InputRecord::Key(KeyRecord {
        key_down: true,
        repeat_count: 2,
        virtual_key_code: 0x41,
        virtual_scan_code: 0x1e,
        utf16_unit: 0x03bb,
        control_key_state: 0x10,
    })
}

#[test]
fn zero_readiness_never_starts_a_blocking_read() {
    let mut input = FakeInput {
        counts: [Ok(0)].into(),
        ..FakeInput::default()
    };

    assert_eq!(read_ready_record(&mut input).unwrap(), None);
    assert_eq!(input.read_calls, 0);
}

#[test]
fn positive_readiness_returns_exactly_one_plain_record() {
    let record = key_record();
    let mut input = FakeInput {
        counts: [Ok(7)].into(),
        reads: [Ok((record, 1))].into(),
        ..FakeInput::default()
    };

    assert_eq!(read_ready_record(&mut input).unwrap(), Some(record));
    assert_eq!(input.read_calls, 1);
}

#[test]
fn hostile_read_counts_are_rejected() {
    for count in [0, 2, u32::MAX] {
        let mut input = FakeInput {
            counts: [Ok(1)].into(),
            reads: [Ok((key_record(), count))].into(),
            ..FakeInput::default()
        };

        assert_eq!(
            read_ready_record(&mut input).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(input.read_calls, 1);
    }
}
