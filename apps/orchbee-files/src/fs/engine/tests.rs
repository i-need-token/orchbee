//! The engine's answers as the link carries them: a listing page within a result record, a stat
//! answer per path, a read's facts and Chunk records, and a cancelled call's close.

use std::{
    fs::{self, File},
    path::Path,
    sync::mpsc,
    thread,
    time::Duration,
};

use orchbee_files_protocol::{
    CallId, Chunk, ChunkLength, Counter, FilesRecord, FrameKind, FsListResult, LineEnding,
    LinkFrame, LinkMessage, LinkSide, MAXIMUM_REPLY_BYTES, PageCursor, ReadMode, ReadSource,
    RecordPlace, SafeInteger, TransferId, WorkspaceId, decode_files_record, decode_link_message,
    encode_reply, parse_reply,
};

use super::*;
use crate::{calls::Calls, fs::tests::Scratch, outbound::Outbound};

const WAIT: Duration = Duration::from_secs(20);

/// The engine and the calls it answers, with the frames they queue for the Server.
struct Link {
    engine: Engine,
    calls: Arc<Calls>,
    outbound: Arc<Outbound>,
    frames: mpsc::Receiver<Vec<u8>>,
}

impl Link {
    fn new() -> Self {
        let outbound = Arc::new(Outbound::new());
        let (sent, frames) = mpsc::channel();
        let writer = Arc::clone(&outbound);
        thread::spawn(move || {
            while let Some(frame) = writer.next() {
                writer.written();
                if sent.send(frame).is_err() {
                    return;
                }
            }
        });
        Self {
            engine: Engine::new(
                Helpers::new(),
                Arc::new(Scopes::new()),
                Arc::new(Subscriptions::new()),
            ),
            calls: Calls::new(Arc::clone(&outbound)),
            outbound,
            frames,
        }
    }

    /// Opens call `id` on lane 1 of connection 1.
    fn call(&self, id: u64) -> Call {
        let connection = ConnectionId::new(1).unwrap();
        let lane = LaneId::new(1).unwrap();
        self.calls
            .open(CallId::new(id).unwrap(), connection, lane)
            .unwrap()
            .unwrap()
    }

    fn next(&self) -> LinkMessage {
        let frame = self.frames.recv_timeout(WAIT).unwrap();
        decode_link_message(&frame, LinkSide::Files).unwrap()
    }

    /// The one record of call `id`, which completes it.
    fn record(&self, id: u64) -> FilesRecord {
        let LinkMessage::Frame(LinkFrame {
            call,
            kind: FrameKind::Result,
            record,
        }) = self.next()
        else {
            panic!("expected the result of call {id}");
        };
        assert_eq!(call, CallId::new(id).unwrap());
        assert_eq!(self.closed(id), CallClose::Complete);
        decode_files_record(&record, RecordPlace::ResultFrame).unwrap()
    }

    fn reply(&self, id: u64, operation: Operation) -> FilesReply {
        let FilesRecord::Result(json) = self.record(id) else {
            panic!("expected a Result record");
        };
        parse_reply(operation, &json).unwrap()
    }

    fn chunk(&self, id: u64) -> Chunk {
        let FilesRecord::Chunk(chunk) = self.record(id) else {
            panic!("expected a Chunk record");
        };
        chunk
    }

    fn closed(&self, id: u64) -> CallClose {
        match self.next() {
            LinkMessage::Close { call, reason } if call == CallId::new(id).unwrap() => reason,
            other => panic!("expected the close of call {id}, got {other:?}"),
        }
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        self.outbound.close();
    }
}

fn workspace() -> WorkspaceId {
    WorkspaceId::from_bytes([1; 16]).unwrap()
}

fn native(path: &Path) -> NativePath {
    NativePath::parse(path.to_str().unwrap()).unwrap()
}

fn wire(path: &Path) -> FilePath {
    FilePath::new(path.to_str().unwrap()).unwrap()
}

fn list_request(path: &Path, cursor: Option<PageCursor>) -> FsList {
    FsList {
        workspace_id: workspace(),
        path: wire(path),
        cursor,
        single_child: false,
    }
}

fn listed(reply: FilesReply) -> FsListResult {
    match reply {
        FilesReply::FsList(page) => page,
        other => panic!("expected a page, got {other:?}"),
    }
}

#[test]
fn a_listing_page_fits_a_result_record() {
    let scratch = Scratch::new("engine-page");
    // Long names fill the byte bound long before the entry bound.
    for index in 0..1_500 {
        File::create(
            scratch
                .path()
                .join(format!("{index:04}-{}", "n".repeat(180))),
        )
        .unwrap();
    }
    let link = Link::new();
    let mut cursor = None;
    let mut listed_entries = 0;
    let mut pages = 0;
    loop {
        pages += 1;
        let request = list_request(scratch.path(), cursor);
        link.engine
            .list(link.call(pages), native(scratch.path()), &request);
        let page = listed(link.reply(pages, Operation::FsList));
        let json = encode_reply(&FilesReply::FsList(page.clone()));
        assert!(json.len() <= MAXIMUM_REPLY_BYTES, "{} bytes", json.len());
        listed_entries += page.entries.as_slice().len();
        cursor = page.cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(listed_entries, 1_500);
    assert!(pages > 1);
}

#[test]
fn stat_answers_each_path() {
    let scratch = Scratch::new("engine-stat");
    let file = scratch.path().join("file");
    fs::write(&file, b"four").unwrap();
    let missing = scratch.path().join("missing");
    let outside = scratch.path().join("outside");
    let link = Link::new();
    link.engine.stat(
        link.call(1),
        vec![
            (wire(&file), Ok(native(&file))),
            (wire(&missing), Ok(native(&missing))),
            (
                wire(&outside),
                Err(FilesError::new(ErrorCode::OutsideScope)),
            ),
        ],
    );
    let FilesReply::FsStat(result) = link.reply(1, Operation::FsStat) else {
        panic!("expected a stat result");
    };
    let items = result.items.as_slice();
    assert_eq!(items[0].path, wire(&file));
    let entry = items[0].outcome.as_ref().unwrap();
    assert_eq!((entry.name.as_str(), entry.size.get()), ("file", 4));
    let code = |index: usize| items[index].outcome.as_ref().unwrap_err().code;
    assert_eq!(
        (code(1), code(2)),
        (ErrorCode::NotFound, ErrorCode::OutsideScope)
    );
}

#[test]
fn a_read_answers_its_facts_then_chunk_records_until_its_transfer_closes() {
    let scratch = Scratch::new("engine-read");
    let path = scratch.path().join("text");
    fs::write(&path, "line\n".repeat(5_000)).unwrap();
    let link = Link::new();
    let transfer = TransferId::new(9).unwrap();
    let request = FsRead {
        workspace_id: workspace(),
        transfer,
        source: ReadSource::Path(wire(&path)),
        mode: ReadMode::Text,
        encoding: None,
        force_text: false,
        limit: SafeInteger::new(1 << 20).unwrap(),
    };
    link.engine.read(link.call(1), native(&path), &request);
    let FilesReply::Read(facts) = link.reply(1, Operation::FsRead) else {
        panic!("expected the read's facts");
    };
    assert_eq!(
        (
            facts.size.get(),
            facts.encoding.as_ref().map(EncodingName::as_str)
        ),
        (25_000, Some("utf8"))
    );

    let pull = |id: u64, offset: u64| {
        let request = FsReadChunk {
            workspace_id: workspace(),
            transfer,
            offset: Counter::new(offset),
            length: ChunkLength::new(16_384).unwrap(),
        };
        link.engine.read_chunk(link.call(id), &request);
        link.chunk(id)
    };
    let first = pull(2, 0);
    assert_eq!(
        (first.transfer, first.offset, first.bytes.len()),
        (transfer, 0, 16_384)
    );
    assert!(first.end.is_none());
    let last = pull(3, 16_384);
    assert_eq!(last.bytes.len(), 25_000 - 16_384);
    assert_eq!(last.end.map(|end| end.line_ending), Some(LineEnding::Lf));

    let close = TransferRequest {
        workspace_id: workspace(),
        transfer,
    };
    link.engine.close_transfer(link.call(4), &close);
    assert_eq!(link.reply(4, Operation::TransferClose), FilesReply::Empty);
    link.engine.close_transfer(link.call(5), &close);
    let FilesReply::Error(error) = link.reply(5, Operation::TransferClose) else {
        panic!("expected an error");
    };
    assert_eq!(error.code, ErrorCode::InvalidRequest);
}

#[test]
fn a_call_cancelled_before_its_answer_closes_as_cancelled() {
    let scratch = Scratch::new("engine-cancel");
    let link = Link::new();
    let call = link.call(1);
    link.calls.cancel(CallId::new(1).unwrap());
    link.engine.list(
        call,
        native(scratch.path()),
        &list_request(scratch.path(), None),
    );
    assert_eq!(link.closed(1), CallClose::Cancelled);
}
