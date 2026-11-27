//! Uploads as the link carries them: Puts in order stage the data and the commit hands it to the
//! operation; a Put out of order or a commit before the data is complete ends the upload and its
//! operation's call; and an upload that receives nothing ends with `timeout`.

use std::{sync::mpsc, thread};

use orchbee_files_protocol::{
    CallId, FilesRecord, FrameKind, LinkFrame, LinkMessage, LinkSide, Operation, RecordPlace,
    decode_files_record, decode_link_message, parse_reply,
};

use super::*;
use crate::{calls::Calls, outbound::Outbound};

const WAIT: Duration = Duration::from_secs(20);

/// The calls uploads answer, with the frames they queue for the Server.
struct Link {
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
            calls: Calls::new(Arc::clone(&outbound)),
            outbound,
            frames,
        }
    }

    /// Opens call `id` on lane `id` of connection 1.
    fn call(&self, id: u64) -> Call {
        let connection = ConnectionId::new(1).unwrap();
        let lane = LaneId::new(id).unwrap();
        self.calls
            .open(CallId::new(id).unwrap(), connection, lane)
            .unwrap()
            .unwrap()
    }

    fn next(&self) -> LinkMessage {
        let frame = self.frames.recv_timeout(WAIT).unwrap();
        decode_link_message(&frame, LinkSide::Files).unwrap()
    }

    /// The Result of call `id`, which completes it: empty, or the error's code.
    fn reply(&self, id: u64) -> Result<(), ErrorCode> {
        let LinkMessage::Frame(LinkFrame {
            call,
            kind: FrameKind::Result,
            record,
        }) = self.next()
        else {
            panic!("expected the result of call {id}");
        };
        assert_eq!(call, CallId::new(id).unwrap());
        let LinkMessage::Close { call, reason } = self.next() else {
            panic!("expected the close of call {id}");
        };
        assert_eq!(
            (call, reason),
            (CallId::new(id).unwrap(), CallClose::Complete)
        );
        let Ok(FilesRecord::Result(json)) = decode_files_record(&record, RecordPlace::ResultFrame)
        else {
            panic!("expected a Result record");
        };
        match parse_reply(Operation::GitStageContent, &json).unwrap() {
            FilesReply::Empty => Ok(()),
            FilesReply::Error(error) => Err(error.code),
            other => panic!("unexpected {other:?}"),
        }
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        self.outbound.close();
    }
}

fn put(transfer: u64, offset: u64, end: PutEnd, bytes: &[u8]) -> Put {
    Put {
        transfer: TransferId::new(transfer).unwrap(),
        offset,
        end,
        bytes: bytes.to_vec(),
    }
}

fn never(_: Call, _: Vec<u8>) {
    panic!("an upload that ended never commits");
}

#[test]
fn puts_stage_in_order_and_only_a_complete_commit_reaches_the_operation() {
    let link = Link::new();
    let uploads = Uploads::new(Arc::new(Timer::new()));
    let transfer = |id| TransferId::new(id).unwrap();
    let (committed, received) = mpsc::channel();
    uploads.open(link.call(1), transfer(1), 6, move |call, data| {
        committed.send(data).unwrap();
        call.reply(&FilesReply::Empty);
    });
    uploads.put(link.call(2), put(1, 0, PutEnd::Continues, b"abc"));
    assert_eq!(link.reply(2), Ok(()));
    uploads.put(link.call(3), put(1, 3, PutEnd::Commit, b"def"));
    assert_eq!(received.recv_timeout(WAIT).unwrap(), b"abcdef");
    assert_eq!(link.reply(1), Ok(()));
    assert_eq!(link.reply(3), Ok(()));
    uploads.put(link.call(4), put(1, 6, PutEnd::Commit, b""));
    assert_eq!(link.reply(4), Err(ErrorCode::InvalidRequest));

    uploads.open(link.call(5), transfer(2), 6, never);
    uploads.put(link.call(6), put(2, 3, PutEnd::Continues, b"def"));
    assert_eq!(link.reply(6), Err(ErrorCode::InvalidRequest));
    assert_eq!(link.reply(5), Err(ErrorCode::InvalidRequest));
    uploads.open(link.call(7), transfer(3), 6, never);
    uploads.put(link.call(8), put(3, 0, PutEnd::Commit, b"abc"));
    assert_eq!(link.reply(8), Err(ErrorCode::InvalidRequest));
    assert_eq!(link.reply(7), Err(ErrorCode::InvalidRequest));

    uploads.open(link.call(9), transfer(4), STAGED_BYTES as u64 + 1, never);
    assert_eq!(link.reply(9), Err(ErrorCode::TooLarge));
}

#[test]
fn an_upload_that_receives_nothing_ends_with_timeout() {
    let link = Link::new();
    let uploads = Uploads::with_idle(Arc::new(Timer::new()), Duration::from_millis(50));
    uploads.open(link.call(1), TransferId::new(1).unwrap(), 1, never);
    assert_eq!(link.reply(1), Err(ErrorCode::Timeout));
}
