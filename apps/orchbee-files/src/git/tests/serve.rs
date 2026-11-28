//! The engine as the dispatch reaches it: a subscription's first summary opens its stream and a
//! later one shows the discovered repository, a write answers with its Result record and its
//! outcome repeats it, and a cancelled subscription closes. A consented repository's working tree
//! is in scope from the consent's answer until the close's. A show's content reaches the chunk
//! requests sent behind it. An editor prompt opens only for a file in its repository's Git
//! directories, a linked worktree's included, which is in scope until the prompt closes.

use super::{
    super::{
        helper,
        ports::{NoTrash, NoWatches},
        relay::{self, Answer, Asked},
        serve::Service,
    },
    fixture::Fixture,
};
use crate::{
    blocking::Helpers, calls::Calls, fs::Engine, outbound::Outbound, scope::Scopes,
    subscriptions::Subscriptions,
};
use orchbee_files_protocol::{
    CallClose, CallId, Chunk, ChunkLength, CommitAll, CommitMessage, ConnectionId, Counter, Empty,
    FilePath, FilesNotice, FilesRecord, FilesReply, FsReadChunk, GitCommit, GitPaths, GitShow,
    LaneId, LinkFrame, LinkMessage, LinkSide, List, MutationReply, MutationState, NoticeKind,
    Operation, PromptAnswer, PromptKind, ReadMode, RecordPlace, RelayAnswer, RelayUpdate,
    RepositoryKind, RepositoryRequest, RequestId, RequestOutcome, SafeInteger, ScmSummary,
    ScopeUpdate, ShowRef, Text, TransferId, WorkspaceId, WorkspaceScope, decode_files_record,
    decode_link_message, parse_notice, parse_reply,
};
use std::{
    fs,
    path::{MAIN_SEPARATOR, PathBuf},
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};

mod history;

const WAIT: Duration = Duration::from_secs(60);

/// A commit hook that hands the test the relay variables of the commit's commands, as any of them
/// could read them, and holds the commit open until the test releases it.
const HOOK: &str = r#"#!/bin/sh
printf '%s\n%s\n' "$ORCHBEE_FILES_RELAY" "$ORCHBEE_FILES_RELAY_TOKEN" > ../relay.tmp
mv ../relay.tmp ../relay
i=0
while [ ! -e ../released ] && [ "$i" -lt 600 ]; do sleep 0.1; i=$((i + 1)); done
"#;

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
            service: Some(Service::start(
                None,
                Arc::clone(&scopes),
                Box::new(NoWatches),
                Arc::new(NoTrash),
            )),
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

    /// The next notice of call `id`, a subscription of `operation`, read as `kind`.
    fn notice(&self, id: u64, operation: Operation, kind: NoticeKind) -> FilesNotice {
        let LinkMessage::Frame(LinkFrame { record, .. }) = self.next(id) else {
            panic!("expected a notice of call {id}");
        };
        let Ok(FilesRecord::Notice(json)) = decode_files_record(&record, RecordPlace::NoticeFrame)
        else {
            panic!("expected a Notice record");
        };
        parse_notice(operation, kind, &json).unwrap()
    }

    /// Summaries of subscription call `id`, from its Snapshot on, until `found` finds what it
    /// waits for in one.
    fn summary<T>(&self, id: u64, found: impl Fn(&ScmSummary) -> Option<T>) -> T {
        let mut kind = NoticeKind::Snapshot;
        loop {
            let FilesNotice::ScmSummary(summary) = self.notice(id, Operation::ScmSubscribe, kind)
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

    /// The Chunk record of call `id`, which completes it.
    fn chunk(&self, id: u64) -> Chunk {
        let LinkMessage::Frame(LinkFrame { record, .. }) = self.next(id) else {
            panic!("expected the chunk of call {id}");
        };
        let Ok(FilesRecord::Chunk(chunk)) = decode_files_record(&record, RecordPlace::ResultFrame)
        else {
            panic!("expected a Chunk record");
        };
        assert_eq!(self.closed(id), CallClose::Complete);
        chunk
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
    let view = harness
        .scopes
        .view(workspace, connection(), Operation::GitStage)
        .unwrap();
    harness.service().stage(
        harness.open(2),
        view,
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

#[test]
fn a_show_serves_its_content_to_the_chunks_sent_behind_it() {
    let fixture = Fixture::new();
    fixture.repository("repo");
    fixture.write("repo/file", "committed");
    fixture.git("repo", &["add", "file"]);
    fixture.git("repo", &["commit", "-q", "-m", "first"]);

    let harness = Harness::new();
    let workspace = WorkspaceId::from_bytes([6; 16]).unwrap();
    harness.state(workspace, WorkspaceScope::Root(fixture.path("repo")));
    harness.service().subscribe(harness.open(1), workspace);
    let shown = harness.summary(1, |summary| {
        summary.repositories.as_slice().first().cloned()
    });

    let files = Engine::new(
        Helpers::new(),
        Arc::new(Scopes::new()),
        Arc::new(Subscriptions::new()),
    );
    let transfer = TransferId::new(1).unwrap();
    let request = GitShow {
        workspace_id: workspace,
        transfer,
        repository: shown.repository,
        reference: ShowRef::Head,
        path: FilePath::new(fixture.path("repo/file")).unwrap(),
        mode: ReadMode::Text,
        encoding: None,
        limit: SafeInteger::new(1024).unwrap(),
    };
    harness.service().show(harness.open(2), &request, |call| {
        files.show_read(call, transfer, request.mode, None, 1024)
    });
    let pull = FsReadChunk {
        workspace_id: workspace,
        transfer,
        offset: Counter::new(0),
        length: ChunkLength::new(16_384).unwrap(),
    };
    files.read_chunk(harness.open(3), &pull);

    let FilesReply::Read(facts) = harness.reply(2, Operation::GitShow) else {
        panic!("expected the content's facts");
    };
    assert_eq!(facts.size.get(), 9);
    let chunk = harness.chunk(3);
    assert_eq!(chunk.bytes, b"committed");
    assert!(chunk.end.is_some());
}

#[test]
fn an_editor_prompt_opens_only_for_a_file_in_its_repository_git_directories() {
    let fixture = Fixture::new();
    fixture.repository("repo");
    fixture.write("repo/seed", "seed");
    fixture.write("repo/outside", "outside");
    fixture.git("repo", &["add", "seed"]);
    fixture.git("repo", &["commit", "-q", "-m", "seed"]);
    fixture.git(
        "repo",
        &["worktree", "add", "-q", "-b", "side", &fixture.path("wt")],
    );
    fixture.write("hooks/prepare-commit-msg", HOOK);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let executable = fs::Permissions::from_mode(0o755);
        fs::set_permissions(fixture.path("hooks/prepare-commit-msg"), executable).unwrap();
    }
    fixture.git(
        "repo",
        &["config", "core.hooksPath", &fixture.path("hooks")],
    );
    fixture.write("wt/change", "change");
    fixture.git("wt", &["add", "change"]);

    let harness = Harness::new();
    let workspace = WorkspaceId::from_bytes([7; 16]).unwrap();
    harness.state(workspace, WorkspaceScope::Root(fixture.path("wt")));
    harness.service().subscribe(harness.open(1), workspace);
    let repository = harness.summary(1, |summary| {
        summary
            .repositories
            .as_slice()
            .first()
            .map(|shown| shown.repository.clone())
    });
    harness
        .service()
        .subscribe_relay(harness.open(2), workspace, true);
    let view = harness
        .scopes
        .view(workspace, connection(), Operation::GitCommit)
        .unwrap();
    harness.service().commit(
        harness.open(3),
        view,
        &GitCommit {
            workspace_id: workspace,
            request_id: RequestId::from_bytes([8; 16]).unwrap(),
            repository,
            message: CommitMessage::Text(Text::new("change").unwrap()),
            amend: false,
            signoff: false,
            all: CommitAll::None,
            no_verify: false,
        },
    );
    let deadline = Instant::now() + WAIT;
    let variables = loop {
        if let Ok(variables) = fs::read_to_string(fixture.path("relay")) {
            break variables;
        }
        assert!(Instant::now() < deadline, "the hook never ran");
        thread::sleep(Duration::from_millis(20));
    };
    let mut lines = variables.lines();
    let endpoint = PathBuf::from(lines.next().unwrap());
    let token = relay::token_from_hex(lines.next().unwrap()).unwrap();
    let editor = |path: &str| {
        let endpoint = endpoint.clone();
        let request = relay::encode_request(Asked::Editor, &token, path);
        thread::spawn(move || helper::ask(&endpoint, &request))
    };
    let readable = |path: &str| {
        harness
            .scopes
            .view(workspace, connection(), Operation::FsRead)
            .unwrap()
            .read(&FilePath::new(path).unwrap())
            .is_ok()
    };

    // Any other file fails the helper without a prompt, also when its name runs through the Git
    // directory.
    let outside = fixture.path("repo/outside");
    let through = format!(
        "{}{}{MAIN_SEPARATOR}outside",
        fixture.path("repo/.git/worktrees/wt"),
        format!("{MAIN_SEPARATOR}..").repeat(3)
    );
    for named in [&outside, &through] {
        assert_eq!(editor(named).join().unwrap().unwrap(), Answer::Cancelled);
    }
    assert!(!readable(&outside));

    // The message file in the worktree's own Git directory opens a prompt, the first the Client
    // sees, and is in scope until the prompt closes.
    let message = fixture.path("repo/.git/worktrees/wt/COMMIT_EDITMSG");
    let editing = editor(&message);
    let FilesNotice::RelaySnapshot(_) =
        harness.notice(2, Operation::RelaySubscribe, NoticeKind::Snapshot)
    else {
        panic!("expected the relay's snapshot");
    };
    let FilesNotice::RelayUpdate(RelayUpdate::Prompt { prompt }) =
        harness.notice(2, Operation::RelaySubscribe, NoticeKind::Update)
    else {
        panic!("expected a prompt");
    };
    assert_eq!(
        (prompt.kind, prompt.path),
        (
            PromptKind::Editor,
            Some(FilePath::new(message.clone()).unwrap())
        )
    );
    assert!(readable(&message));
    harness.service().answer(
        harness.open(4),
        &RelayAnswer {
            workspace_id: workspace,
            prompt: prompt.id,
            answer: PromptAnswer::Empty,
        },
    );
    assert_eq!(harness.reply(4, Operation::RelayAnswer), FilesReply::Empty);
    assert_eq!(editing.join().unwrap().unwrap(), Answer::Empty);
    let deadline = Instant::now() + WAIT;
    while readable(&message) {
        assert!(
            Instant::now() < deadline,
            "the closed prompt's file stays in scope"
        );
        thread::sleep(Duration::from_millis(10));
    }

    fs::write(fixture.path("released"), "").unwrap();
    let FilesReply::GitCommit(_) = harness.reply(3, Operation::GitCommit) else {
        panic!("expected the commit");
    };
}
