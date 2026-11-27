//! Read transfers against real files: what a read reports before its chunks, chunk order and
//! length, the final chunk's facts, and the transfer's lifetime.

use std::{
    fs,
    io::Write,
    path::Path,
    sync::mpsc::{self, Receiver},
    time::Duration,
};

use orchbee_files_protocol::{ChunkEnd, LineEnding, ReadMode};
use sha2::{Digest, Sha256};

use super::*;
use crate::fs::{encoding::Encoding, tests::Scratch};

const WAIT: Duration = Duration::from_secs(10);

fn transfers() -> Transfers {
    Transfers::new(Helpers::new())
}

fn connection() -> ConnectionId {
    ConnectionId::new(1).unwrap()
}

fn lane(value: u64) -> LaneId {
    LaneId::new(value).unwrap()
}

fn id(value: u64) -> TransferId {
    TransferId::new(value).unwrap()
}

fn text(encoding: Option<&str>) -> ReadRequest {
    ReadRequest {
        mode: ReadMode::Text,
        encoding: encoding.map(str::to_owned),
        force_text: false,
        limit: 1 << 30,
    }
}

fn open(
    transfers: &Transfers,
    transfer: u64,
    path: &Path,
    request: ReadRequest,
) -> Receiver<Result<ReadFacts, Unopened>> {
    let (send, receive) = mpsc::channel();
    transfers.open(
        connection(),
        lane(transfer),
        id(transfer),
        path.to_path_buf(),
        request,
        move |result| send.send(result).unwrap(),
    );
    receive
}

fn request_chunk(
    transfers: &Transfers,
    transfer: u64,
    offset: u64,
    length: usize,
) -> Receiver<Result<Chunk, FilesError>> {
    let (send, receive) = mpsc::channel();
    transfers.chunk(
        connection(),
        id(transfer),
        offset,
        length,
        Arc::new(AtomicBool::new(false)),
        move |result| send.send(result).unwrap(),
    );
    receive
}

/// Reads transfer `transfer` to its end in chunks of `length`, asking for every next offset as
/// soon as the previous answer arrives, and returns the chunks.
fn chunks(transfers: &Transfers, transfer: u64, length: usize) -> Vec<Chunk> {
    let mut chunks: Vec<Chunk> = Vec::new();
    let mut offset = 0;
    while chunks.last().is_none_or(|chunk| chunk.end.is_none()) {
        let chunk = request_chunk(transfers, transfer, offset, length)
            .recv_timeout(WAIT)
            .unwrap()
            .unwrap();
        assert_eq!(chunk.offset, offset);
        offset += chunk.bytes.len() as u64;
        chunks.push(chunk);
    }
    chunks
}

fn joined(chunks: &[Chunk]) -> String {
    String::from_utf8(
        chunks
            .iter()
            .flat_map(|chunk| chunk.bytes.clone())
            .collect(),
    )
    .unwrap()
}

fn read_text(path: &Path, encoding: Option<&str>, length: usize) -> (ReadFacts, Vec<Chunk>) {
    let transfers = transfers();
    let facts = open(&transfers, 1, path, text(encoding))
        .recv_timeout(WAIT)
        .unwrap()
        .unwrap();
    (facts, chunks(&transfers, 1, length))
}

fn end(chunks: &[Chunk]) -> ChunkEnd {
    chunks.last().unwrap().end.unwrap()
}

#[test]
fn a_text_read_serves_exact_lengths_in_order_without_the_byte_order_mark() {
    let scratch = Scratch::new("read-text");
    let path = scratch.path().join("text");
    let content = "é漢字\r\nline two\r\nend".repeat(2_000);
    let bytes = [&[0xEF, 0xBB, 0xBF][..], content.as_bytes()].concat();
    fs::write(&path, &bytes).unwrap();

    let (facts, chunks) = read_text(&path, Some("windows1252"), 16_384);
    assert_eq!(
        (
            facts.size,
            facts.encoding.map(Encoding::name),
            facts.bom,
            facts.binary
        ),
        (bytes.len() as u64, Some("utf8"), true, false)
    );
    assert_eq!(joined(&chunks), content);
    // Every chunk but the last is exactly the length asked, splitting characters as it must.
    for chunk in &chunks[..chunks.len() - 1] {
        assert_eq!(chunk.bytes.len(), 16_384);
    }
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    assert_eq!(
        end(&chunks),
        ChunkEnd {
            line_ending: LineEnding::CrLf,
            changed: false,
            digest: Some(digest),
        }
    );
}

#[test]
fn chunk_requests_sent_before_the_read_opens_are_served_in_order() {
    let scratch = Scratch::new("read-pipelined");
    let path = scratch.path().join("text");
    let content: String = (0..40_000)
        .map(|index| char::from(b'a' + (index % 26) as u8))
        .collect();
    fs::write(&path, &content).unwrap();
    let transfers = transfers();
    let opened = open(&transfers, 7, &path, text(None));
    let answers: Vec<_> = (0..3)
        .map(|index| request_chunk(&transfers, 7, index * 16_384, 16_384))
        .collect();
    assert!(opened.recv_timeout(WAIT).unwrap().is_ok());
    let served: Vec<Chunk> = answers
        .iter()
        .map(|answer| answer.recv_timeout(WAIT).unwrap().unwrap())
        .collect();
    assert_eq!(joined(&served), content);
    assert!(served[2].end.is_some());
    // An offset other than the next one is refused.
    assert_eq!(
        request_chunk(&transfers, 7, 0, 16_384)
            .recv_timeout(WAIT)
            .unwrap(),
        Err(FilesError::new(ErrorCode::InvalidRequest))
    );
    // Past the end, the next offset repeats the end without bytes.
    let again = request_chunk(&transfers, 7, content.len() as u64, 16_384)
        .recv_timeout(WAIT)
        .unwrap()
        .unwrap();
    assert!(again.bytes.is_empty());
    assert_eq!(again.end, served[2].end);
}

#[test]
fn utf16_is_detected_by_its_mark_or_its_zero_bytes_and_malformed_input_becomes_replacement() {
    let scratch = Scratch::new("read-utf16");
    let utf16 = |text: &str, big: bool| -> Vec<u8> {
        text.encode_utf16()
            .flat_map(|unit| {
                if big {
                    unit.to_be_bytes()
                } else {
                    unit.to_le_bytes()
                }
            })
            .collect()
    };
    let cases = [
        (
            [&[0xFF, 0xFE][..], &utf16("añb", false)].concat(),
            "utf16le",
            true,
            "añb",
        ),
        (
            [&[0xFE, 0xFF][..], &utf16("añb", true)].concat(),
            "utf16be",
            true,
            "añb",
        ),
        (utf16("plain", false), "utf16le", false, "plain"),
        (utf16("plain", true), "utf16be", false, "plain"),
        // An odd final byte cannot be a code unit.
        (
            [&[0xFF, 0xFE][..], &utf16("ab", false), &[0x63]].concat(),
            "utf16le",
            true,
            "ab\u{FFFD}",
        ),
    ];
    for (index, (bytes, encoding, bom, expected)) in cases.into_iter().enumerate() {
        let path = scratch.path().join(format!("case-{index}"));
        fs::write(&path, &bytes).unwrap();
        let (facts, chunks) = read_text(&path, None, 16_384);
        assert_eq!(
            (facts.encoding.map(Encoding::name), facts.bom, facts.binary),
            (Some(encoding), bom, false),
            "case {index}"
        );
        assert_eq!(joined(&chunks), expected, "case {index}");
    }
}

#[test]
fn the_requested_encoding_decodes_content_without_a_mark() {
    let scratch = Scratch::new("read-encoding");
    let path = scratch.path().join("text");
    fs::write(&path, [b'c', b'a', b'f', 0xE9, 0xFF]).unwrap();
    let (facts, chunks) = read_text(&path, Some("windows1252"), 16_384);
    assert_eq!(facts.encoding.map(Encoding::name), Some("windows1252"));
    assert_eq!(joined(&chunks), "caféÿ");
    // UTF-8 decodes the same bytes with replacements; an unknown name decodes UTF-8.
    let (facts, chunks) = read_text(&path, Some("nosuchencoding"), 16_384);
    assert_eq!(facts.encoding.map(Encoding::name), Some("utf8"));
    assert_eq!(joined(&chunks), "caf\u{FFFD}\u{FFFD}");
}

#[test]
fn a_binary_file_reports_binary_and_serves_text_only_when_forced() {
    let scratch = Scratch::new("read-binary");
    let path = scratch.path().join("binary");
    fs::write(&path, b"ab\0\0cd").unwrap();
    let transfers = transfers();
    let facts = open(&transfers, 1, &path, text(None))
        .recv_timeout(WAIT)
        .unwrap()
        .unwrap();
    assert_eq!(
        (facts.binary, facts.encoding, facts.bom),
        (true, None, false)
    );
    assert_eq!(
        request_chunk(&transfers, 1, 0, 16_384)
            .recv_timeout(WAIT)
            .unwrap(),
        Err(FilesError::new(ErrorCode::InvalidRequest))
    );
    assert_eq!(transfers.close(connection(), id(1)), Ok(()));

    let forced = ReadRequest {
        force_text: true,
        ..text(None)
    };
    let facts = open(&transfers, 2, &path, forced)
        .recv_timeout(WAIT)
        .unwrap()
        .unwrap();
    assert_eq!(
        (facts.binary, facts.encoding.map(Encoding::name)),
        (true, Some("utf8"))
    );
    assert_eq!(joined(&chunks(&transfers, 2, 16_384)), "ab\0\0cd");
}

#[test]
fn line_endings_follow_the_text_buffer_rule_across_chunk_boundaries() {
    let scratch = Scratch::new("read-line-endings");
    let cases = [
        ("one line", LineEnding::None),
        ("", LineEnding::None),
        ("a\nb\r\nc\n", LineEnding::Lf),
        ("a\r\nb\r\nc\n", LineEnding::CrLf),
        // Lone carriage returns count toward CRLF.
        ("a\rb\rc\n", LineEnding::CrLf),
        // A tie is not more than half.
        ("a\r\nb\n", LineEnding::Lf),
        ("a\r", LineEnding::CrLf),
    ];
    for (index, (content, expected)) in cases.into_iter().enumerate() {
        let path = scratch.path().join(format!("case-{index}"));
        fs::write(&path, content).unwrap();
        // One-byte chunks put every carriage return and line feed in separate chunks.
        let (_, chunks) = read_text(&path, None, 1);
        assert_eq!(joined(&chunks), content, "case {index}");
        assert_eq!(end(&chunks).line_ending, expected, "case {index}");
    }
}

#[test]
fn the_final_chunk_reports_a_file_that_changed_during_the_read() {
    let scratch = Scratch::new("read-changed");
    let path = scratch.path().join("text");
    fs::write(&path, "x".repeat(40_000)).unwrap();
    let transfers = transfers();
    open(&transfers, 1, &path, text(None))
        .recv_timeout(WAIT)
        .unwrap()
        .unwrap();
    let first = request_chunk(&transfers, 1, 0, 16_384)
        .recv_timeout(WAIT)
        .unwrap()
        .unwrap();
    assert!(first.end.is_none());
    fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"more")
        .unwrap();
    let mut offset = 16_384;
    let last = loop {
        let chunk = request_chunk(&transfers, 1, offset, 16_384)
            .recv_timeout(WAIT)
            .unwrap()
            .unwrap();
        offset += chunk.bytes.len() as u64;
        if let Some(end) = chunk.end {
            break end;
        }
    };
    assert!(last.changed);
}

#[test]
fn a_byte_read_serves_any_offset_and_ends_at_the_end_of_the_file() {
    let scratch = Scratch::new("read-bytes");
    let path = scratch.path().join("bytes");
    let bytes: Vec<u8> = (0..50_000u32).map(|index| (index % 251) as u8).collect();
    fs::write(&path, &bytes).unwrap();
    let transfers = transfers();
    let request = ReadRequest {
        mode: ReadMode::Bytes,
        ..text(Some("utf16le"))
    };
    let facts = open(&transfers, 3, &path, request)
        .recv_timeout(WAIT)
        .unwrap()
        .unwrap();
    assert_eq!(
        (facts.size, facts.encoding, facts.bom, facts.binary),
        (50_000, None, false, false)
    );
    let last = request_chunk(&transfers, 3, 32_768, 32_768)
        .recv_timeout(WAIT)
        .unwrap()
        .unwrap();
    let first = request_chunk(&transfers, 3, 0, 32_768)
        .recv_timeout(WAIT)
        .unwrap()
        .unwrap();
    assert_eq!(first.bytes, bytes[..32_768]);
    assert!(first.end.is_none());
    assert_eq!(last.bytes, bytes[32_768..]);
    assert_eq!(
        last.end,
        Some(ChunkEnd {
            line_ending: LineEnding::None,
            changed: false,
            digest: None,
        })
    );
}

#[test]
fn a_read_that_cannot_open_leaves_no_transfer() {
    let scratch = Scratch::new("read-refused");
    let path = scratch.path().join("text");
    fs::write(&path, "twelve bytes").unwrap();
    let transfers = transfers();
    let small = ReadRequest {
        limit: 11,
        ..text(None)
    };
    let cases = [
        (path.clone(), small, ErrorCode::TooLarge),
        (
            scratch.path().to_path_buf(),
            text(None),
            ErrorCode::IsDirectory,
        ),
        (
            scratch.path().join("missing"),
            text(None),
            ErrorCode::NotFound,
        ),
    ];
    for (path, request, code) in cases {
        let waiting = {
            let opened = open(&transfers, 1, &path, request);
            let waiting = request_chunk(&transfers, 1, 0, 16_384);
            assert!(matches!(
                opened.recv_timeout(WAIT).unwrap(),
                Err(Unopened::Failed(error)) if error.code == code
            ));
            waiting
        };
        // A chunk request that waited on the read fails with it.
        assert_eq!(
            waiting
                .recv_timeout(WAIT)
                .unwrap()
                .map_err(|error| error.code),
            Err(code)
        );
        assert_eq!(
            transfers.close(connection(), id(1)),
            Err(FilesError::new(ErrorCode::InvalidRequest))
        );
    }
}

#[test]
fn a_transfer_name_is_unique_until_its_transfer_or_lane_closes() {
    let scratch = Scratch::new("read-lifetime");
    let path = scratch.path().join("text");
    fs::write(&path, "x".repeat(40_000)).unwrap();
    let transfers = transfers();
    assert!(
        open(&transfers, 1, &path, text(None))
            .recv_timeout(WAIT)
            .unwrap()
            .is_ok()
    );
    assert_eq!(
        open(&transfers, 1, &path, text(None))
            .recv_timeout(WAIT)
            .unwrap(),
        Err(Unopened::Failed(FilesError::new(ErrorCode::InvalidRequest)))
    );
    transfers.lane_closed(connection(), lane(1));
    assert_eq!(
        request_chunk(&transfers, 1, 0, 16_384)
            .recv_timeout(WAIT)
            .unwrap(),
        Err(FilesError::new(ErrorCode::InvalidRequest))
    );
    assert!(
        open(&transfers, 1, &path, text(None))
            .recv_timeout(WAIT)
            .unwrap()
            .is_ok()
    );
    transfers.connection_lost(connection());
    assert_eq!(
        transfers.close(connection(), id(1)),
        Err(FilesError::new(ErrorCode::InvalidRequest))
    );
}
