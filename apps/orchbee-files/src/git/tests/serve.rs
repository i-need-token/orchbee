//! The engine as the dispatch reaches it: a subscription's first summary opens its stream and a
//! later one shows the discovered repository, a write answers with its Result record and its
//! outcome repeats it, and a cancelled subscription closes.

use super::{super::serve::Service, fixture::Fixture};
use crate::{calls::Calls, outbound::Outbound};
use orchbee_files_protocol::{
    CallClose, CallId, ConnectionId, Empty, FilePath, FilesNotice, FilesRecord, FilesReply,
    GitPaths, LaneId, LinkFrame, LinkMessage, LinkSide, List, MutationReply, MutationState,
    NoticeKind, Operation, RecordPlace, RepositoryKind, RequestId, RequestOutcome, ScopeUpdate,
    WorkspaceId, WorkspaceScope, decode_files_record, decode_link_message, parse_notice,
    parse_reply,
};
use std::{
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};

const WAIT: Duration = Duration::from_secs(60);

/// The frames queued for the Server, in order.
struct Frames(mpsc::Receiver<Vec<u8>>);

impl Frames {
    fn new(outbound: &Arc<Outbound>) -> Self {
        let (sent, received) = mpsc::channel();
        let writer = Arc::clone(outbound);
        thread::spawn(move || {
            while let Some(frame) = writer.next() {
                writer.written();
                if sent.send(frame).is_err() {
                    return;
                }
            }
        });
        Self(received)
    }

    /// The next frame of call `id`; other calls' frames before it are dropped.
    fn next(&self, id: u64) -> LinkMessage {
        loop {
            let frame = self
                .0
                .recv_timeout(WAIT)
                .expect("the service sends a frame");
            let message = decode_link_message(&frame, LinkSide::Files).unwrap();
            let call = match &message {
                LinkMessage::Frame(frame) => frame.call,
                LinkMessage::Close { call, .. } => *call,
                other => panic!("unexpected {other:?}"),
            };
            if call == CallId::new(id).unwrap() {
                return message;
            }
        }
    }

    fn notice(&self, id: u64, kind: NoticeKind) -> FilesNotice {
        let LinkMessage::Frame(LinkFrame { record, .. }) = self.next(id) else {
            panic!("expected a notice of call {id}");
        };
        let Ok(FilesRecord::Notice(json)) = decode_files_record(&record, RecordPlace::NoticeFrame)
        else {
            panic!("expected a Notice record");
        };
        parse_notice(Operation::ScmSubscribe, kind, &json).unwrap()
    }

    fn reply(&self, id: u64, operation: Operation) -> FilesReply {
        let LinkMessage::Frame(LinkFrame { record, .. }) = self.next(id) else {
            panic!("expected the result of call {id}");
        };
        let Ok(FilesRecord::Result(json)) = decode_files_record(&record, RecordPlace::ResultFrame)
        else {
            panic!("expected a Result record");
        };
        assert_eq!(self.closed(id), CallClose::Complete);
        parse_reply(operation, &json).unwrap()
    }

    /// How call `id` closed, after any notices still before its close.
    fn closed(&self, id: u64) -> CallClose {
        loop {
            if let LinkMessage::Close { reason, .. } = self.next(id) {
                return reason;
            }
        }
    }
}

#[test]
fn requests_reach_the_engine_and_its_answers_return_as_records() {
    let fixture = Fixture::new();
    fixture.repository("repo");
    fixture.write("repo/seed", "seed");
    fixture.git("repo", &["add", "seed"]);
    fixture.git("repo", &["commit", "-q", "-m", "seed"]);

    let outbound = Arc::new(Outbound::new());
    let calls = Calls::new(Arc::clone(&outbound));
    let frames = Frames::new(&outbound);
    let service = Service::start(None);
    let workspace = WorkspaceId::from_bytes([3; 16]).unwrap();
    let open = |id: u64| {
        calls
            .open(
                CallId::new(id).unwrap(),
                ConnectionId::new(1).unwrap(),
                LaneId::new(id).unwrap(),
            )
            .unwrap()
            .unwrap()
    };
    service.scope(&ScopeUpdate {
        workspace,
        scope: WorkspaceScope::Root(fixture.path("repo")),
    });

    service.subscribe(open(1), workspace);
    let mut kind = NoticeKind::Snapshot;
    let shown = loop {
        let FilesNotice::ScmSummary(summary) = frames.notice(1, kind) else {
            panic!("expected a summary");
        };
        kind = NoticeKind::Update;
        if let Some(shown) = summary
            .repositories
            .as_slice()
            .first()
            .filter(|shown| shown.revision.get() >= 1)
        {
            break shown.clone();
        }
    };
    assert_eq!(shown.kind, RepositoryKind::Root);

    fixture.write("repo/added", "added");
    let request_id = RequestId::from_bytes([5; 16]).unwrap();
    service.stage(
        open(2),
        &GitPaths {
            workspace_id: workspace,
            request_id,
            repository: shown.repository,
            paths: List::new(vec![FilePath::new(fixture.path("repo/added")).unwrap()]).unwrap(),
        },
    );
    assert_eq!(frames.reply(2, Operation::GitStage), FilesReply::Empty);
    service.outcome(
        open(3),
        &RequestOutcome {
            workspace_id: workspace,
            request_id,
        },
    );
    let FilesReply::RequestOutcome(outcome) = frames.reply(3, Operation::RequestOutcome) else {
        panic!("expected an outcome");
    };
    assert_eq!(
        outcome.state,
        MutationState::Completed(MutationReply::Empty(Empty {}))
    );

    calls.cancel(CallId::new(1).unwrap());
    assert_eq!(frames.closed(1), CallClose::Cancelled);
    drop(service);
    outbound.close();
}
