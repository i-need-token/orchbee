use super::*;
use orchbee_local_ipc::ChildInputWriter;
use std::{
    io::Write,
    thread,
    time::{Duration, Instant},
};

#[test]
fn reader_preserves_fragmented_lines_bom_eof_and_reaps_stalled_input() {
    let (mut input, child_input) = ChildInputWriter::pipe().unwrap();
    let mut reader = Reader::with_input(child_input).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let records = thread::scope(|scope| {
        scope.spawn(move || {
            for part in [
                b"\xef\xbb\xbf{\"type\":\"terminal.release\"}\r".as_slice(),
                b"\n\nlast",
            ] {
                input.write_all(part).unwrap();
                thread::sleep(Duration::from_millis(20));
            }
            drop(input);
        });
        let mut records = Vec::new();
        loop {
            assert!(Instant::now() < deadline, "reader deadline");
            match reader.poll().unwrap() {
                Poll::Line(line) => records.push(line),
                Poll::Pending => thread::sleep(Duration::from_millis(2)),
                Poll::Eof => break,
            }
        }
        records
    });
    assert_eq!(
        records,
        vec![
            br#"{"type":"terminal.release"}"#.to_vec(),
            Vec::new(),
            b"last".to_vec()
        ]
    );
    drop(reader);
    let (_held, child_input) = ChildInputWriter::pipe().unwrap();
    let mut reader = Reader::with_input(child_input).unwrap();
    assert!(matches!(reader.poll().unwrap(), Poll::Pending));
    let started = Instant::now();
    drop(reader);
    assert!(started.elapsed() < Duration::from_secs(2));
}
