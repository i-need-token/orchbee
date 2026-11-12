use super::{InputPort, InputRecord, KeyRecord, read_ready_records};
use std::{collections::VecDeque, io};

#[derive(Default)]
struct FakeInput {
    counts: VecDeque<io::Result<u32>>,
    reads: VecDeque<io::Result<Vec<InputRecord>>>,
    capacities: Vec<usize>,
}

impl InputPort for FakeInput {
    fn ready_count(&mut self) -> io::Result<u32> {
        self.counts
            .pop_front()
            .unwrap_or_else(|| Err(io::Error::other("unexpected readiness query")))
    }

    fn read(&mut self, capacity: usize) -> io::Result<Vec<InputRecord>> {
        self.capacities.push(capacity);
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

    assert_eq!(read_ready_records(&mut input, 8).unwrap(), Vec::new());
    assert_eq!(read_ready_records(&mut input, 0).unwrap(), Vec::new());
    assert!(input.capacities.is_empty());
}

#[test]
fn a_read_requests_no_more_than_is_ready_or_allowed() {
    let mut input = FakeInput {
        counts: [Ok(7), Ok(3)].into(),
        reads: [Ok(vec![key_record(); 4]), Ok(vec![key_record(); 3])].into(),
        ..FakeInput::default()
    };

    assert_eq!(read_ready_records(&mut input, 4).unwrap().len(), 4);
    assert_eq!(read_ready_records(&mut input, 4).unwrap().len(), 3);
    assert_eq!(input.capacities, [4, 3]);
}

#[test]
fn hostile_read_counts_are_rejected() {
    for records in [0, 3] {
        let mut input = FakeInput {
            counts: [Ok(2)].into(),
            reads: [Ok(vec![key_record(); records])].into(),
            ..FakeInput::default()
        };

        assert_eq!(
            read_ready_records(&mut input, 8).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }
}
