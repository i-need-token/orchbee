//! The engine as the dispatch reaches it: a subscription's first summary opens its stream and a
//! later one shows the discovered repository, a write answers with its Result record and its
//! outcome repeats it, and a cancelled subscription closes. A consented repository's working tree
//! is in scope from the consent's answer until the close's.

use super::{super::serve::Service, fixture::Fixture};
use crate::{calls::Calls, outbound::Outbound, scope::Scopes};
use orchbee_files_protocol::{
    CallClose, CallId, ConnectionId, Empty, FilePath, FilesNotice, FilesRecord, FilesReply,
    GitPaths, LaneId, LinkFrame, LinkMessage, LinkSide, List, MutationReply, MutationState,
    NoticeKind, Operation, RecordPlace, RepositoryKind, RepositoryRequest, RequestId,
    RequestOutcome, ScmSummary, ScopeUpdate, WorkspaceId, WorkspaceScope, decode_files_record,
    decode_link_message, parse_notice, parse_reply,
};
use std::{
    path::MAIN_SEPARATOR,
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};

const WAIT: Duration = Duration::from_secs(60);

fn connection() -> ConnectionId {
    ConnectionId::new(1).unwrap()
}

/// The service with the call queue it answers through, the scopes it states members to, and the
/// frames it queued for the Server, in order.
struct Harness {
    service: Option<Service>,
    scopes: Arc<Scopes>,
    calls: Arc<Calls>,
    outbound: Arc<Outbound>,
    frames: mpsc::Receiver<Vec<u8>>,
}

impl Harness {
    fn new() -> Self {
        let outbound = Arc::new(Outbound::new());
        let calls = Calls::new(Arc::clone(&outbound));
        let scopes = Arc::new(Scopes::new());
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
            service: Some(Service::start(None, Arc::clone(&scopes))),
            scopes,
            calls,
            outbound,
            frames,
        }
    }

    fn service(&self) -> &Service {
        self.service.as_ref().unwrap()
    }

    /// States `workspace`'s scope as the dispatch does: to the scopes, then to the engine.
    fn state(&self, workspace: WorkspaceId, scope: WorkspaceScope) {
        let update = ScopeUpdate { workspace, scope };
        self.scopes.update(update.clone());
        self.service().scope(&update);
    }

    fn open(&self, id: u64) -> crate::calls::Call {
        self.calls
            .open(
                CallId::new(id).unwrap(),
                connection(),
                LaneId::new(id).unwrap(),
            )
            .unwrap()
            .unwrap()
    }

    /// The next frame of call `id`; other calls' frames before it are dropped.
    fn next(&self, id: u64) -> LinkMessage {
        loop {
            let frame = self
                .frames
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

    /// Summaries of subscription call `id`, from its Snapshot on, until `found` finds what it
    /// waits for in one.
    fn summary<T>(&self, id: u64, found: impl Fn(&ScmSummary) -> Option<T>) -> T {
        let mut kind = NoticeKind::Snapshot;
        loop {
            let LinkMessage::Frame(LinkFrame { record, .. }) = self.next(id) else {
                panic!("expected a notice of call {id}");
            };
            let Ok(FilesRecord::Notice(json)) =
                decode_files_record(&record, RecordPlace::NoticeFrame)
            else {
                panic!("expected a Notice record");
            };
            let Ok(FilesNotice::ScmSummary(summary)) =
                parse_notice(Operation::ScmSubscribe, kind, &json)
            else {
                panic!("expected a summary");
            };
            kind = NoticeKind::Update;
            if let Some(value) = found(&summary) {
                return value;
            }
        }
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

impl Drop for Harness {
    fn drop(&mut self) {
        self.service.take();
        self.outbound.close();
    }
}

#[test]
fn requests_reach_the_engine_and_its_answers_return_as_records() {
    let fixture = Fixture::new();
    fixture.repository("repo");
    fixture.write("repo/seed", "seed");
    fixture.git("repo", &["add", "seed"]);
    fixture.git("repo", &["commit", "-q", "-m", "seed"]);

    let harness = Harness::new();
    let workspace = WorkspaceId::from_bytes([3; 16]).unwrap();
    harness.state(workspace, WorkspaceScope::Root(fixture.path("repo")));

    harness.service().subscribe(harness.open(1), workspace);
    let shown = harness.summary(1, |summary| {
        summary
            .repositories
            .as_slice()
            .first()
            .filter(|shown| shown.revision.get() >= 1)
            .cloned()
    });
    assert_eq!(shown.kind, RepositoryKind::Root);

    fixture.write("repo/added", "added");
    let request_id = RequestId::from_bytes([5; 16]).unwrap();
    harness.service().stage(
        harness.open(2),
        &GitPaths {
            workspace_id: workspace,
            request_id,
            repository: shown.repository,
            paths: List::new(vec![FilePath::new(fixture.path("repo/added")).unwrap()]).unwrap(),
        },
    );
    assert_eq!(harness.reply(2, Operation::GitStage), FilesReply::Empty);
    harness.service().outcome(
        harness.open(3),
        &RequestOutcome {
            workspace_id: workspace,
            request_id,
        },
    );
    let FilesReply::RequestOutcome(outcome) = harness.reply(3, Operation::RequestOutcome) else {
        panic!("expected an outcome");
    };
    assert_eq!(
        outcome.state,
        MutationState::Completed(MutationReply::Empty(Empty {}))
    );

    harness.calls.cancel(CallId::new(1).unwrap());
    assert_eq!(harness.closed(1), CallClose::Cancelled);
}

#[test]
fn a_consented_repository_is_in_scope_until_it_closes() {
    let fixture = Fixture::new();
    fixture.repository("repo");
    fixture.write("repo/root/inside", "inside");
    fixture.write("repo/beside", "beside");

    let harness = Harness::new();
    let workspace = WorkspaceId::from_bytes([4; 16]).unwrap();
    harness.state(workspace, WorkspaceScope::Root(fixture.path("repo/root")));
    harness.service().subscribe(harness.open(1), workspace);
    let parent = harness.summary(1, |summary| summary.candidates.as_slice().first().cloned());
    let beside = FilePath::new(format!("{}{MAIN_SEPARATOR}beside", parent.as_str())).unwrap();
    let readable = || {
        harness
            .scopes
            .view(workspace, connection(), Operation::FsRead)
            .unwrap()
            .read(&beside)
            .is_ok()
    };
    assert!(!readable());

    let request = RepositoryRequest {
        workspace_id: workspace,
        repository: parent,
    };
    harness.service().consent(harness.open(2), &request);
    assert_eq!(
        harness.reply(2, Operation::RepositoryConsent),
        FilesReply::Empty
    );
    assert!(readable());
    harness.service().close(harness.open(3), &request);
    assert_eq!(
        harness.reply(3, Operation::RepositoryClose),
        FilesReply::Empty
    );
    assert!(!readable());
}
