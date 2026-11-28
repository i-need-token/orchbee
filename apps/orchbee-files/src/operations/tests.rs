use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

use orchbee_files_protocol::{
    CallClose, CallId, ChangeKind, Conflict, ConnectionId, CreateKind, DeleteMode, EncodingName,
    EntryKind, ErrorCode, FileName, FilePath, FilesNotice, FilesRecord, FilesReply, FilesRequest,
    FrameKind, FsClipboard, FsCreate, FsDelete, FsExists, FsExistsResult, FsGrant, FsGrantResult,
    FsList, FsRename, FsRestore, FsSnapshot, FsTransfer, FsUpdate, FsWrite, GitPath, GrantClass,
    GrantSource, LaneId, LinkFrame, LinkMessage, LinkRequest, LinkSide, List, MutationReply,
    MutationState, NoticeKind, Operation, PathsRequest, PickerList, PickerRequest, Put, PutEnd,
    RecordPlace, RequestId, RequestOutcome, RequestOutcomeResult, Root, SafeInteger, ScopeUpdate,
    SessionId, TransferId, WatchState, WorkspaceId, WorkspaceRequest, WorkspaceScope,
    WritePrecondition, decode_files_record, decode_link_message, encode_files_record,
    encode_request, parse_notice, parse_reply,
};

use super::Services;
use crate::{
    calls::Calls,
    outbound::Outbound,
    scope::path::{self, NativePath},
};

/// The process's side of the link: its services and the frames they queue for the Server.
struct Link {
    services: Arc<Services>,
    outbound: Arc<Outbound>,
    frames: mpsc::Receiver<Vec<u8>>,
}

impl Link {
    fn new() -> Self {
        let outbound = Arc::new(Outbound::new());
        let services = Services::new(Calls::new(Arc::clone(&outbound)), None);
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
            services,
            outbound,
            frames,
        }
    }

    fn scope(&self, scope: WorkspaceScope) {
        self.services.scope(ScopeUpdate {
            workspace: workspace(),
            scope,
        });
    }

    fn send(&self, id: u64, grant: GrantClass, workspace: WorkspaceId, record: Vec<u8>) {
        self.services
            .request(LinkRequest {
                call: call(id),
                connection: ConnectionId::new(1).unwrap(),
                lane: LaneId::new(id).unwrap(),
                workspace: Some(workspace),
                grant,
                record,
            })
            .unwrap();
    }

    fn request(&self, id: u64, request: &FilesRequest) {
        let record = encode_files_record(&FilesRecord::Request(encode_request(request))).unwrap();
        self.send(id, GrantClass::TerminalCreate, request.workspace(), record);
    }

    /// Sends `put` as the Server relays one: on a call of its own, naming no Workspace.
    fn put(&self, id: u64, put: Put) {
        let record = encode_files_record(&FilesRecord::Put(put)).unwrap();
        self.services
            .request(LinkRequest {
                call: call(id),
                connection: ConnectionId::new(1).unwrap(),
                lane: LaneId::new(id).unwrap(),
                workspace: None,
                grant: GrantClass::TerminalCreate,
                record,
            })
            .unwrap();
    }

    fn next(&self) -> LinkMessage {
        self.within(Duration::from_secs(20))
            .expect("the process sends a frame")
    }

    /// The next frame, when one comes within `wait`.
    fn within(&self, wait: Duration) -> Option<LinkMessage> {
        let frame = self.frames.recv_timeout(wait).ok()?;
        Some(decode_link_message(&frame, LinkSide::Files).unwrap())
    }

    fn closed(&self, id: u64) -> CallClose {
        match self.next() {
            LinkMessage::Close {
                call: closed,
                reason,
            } if closed == call(id) => reason,
            other => panic!("expected the close of call {id}, got {other:?}"),
        }
    }

    /// The one Result of request call `id`, which completes it.
    fn reply(&self, id: u64, operation: Operation) -> FilesReply {
        let LinkMessage::Frame(LinkFrame {
            call: answered,
            kind: FrameKind::Result,
            record,
        }) = self.next()
        else {
            panic!("expected the result of call {id}");
        };
        assert_eq!(answered, call(id));
        let Ok(FilesRecord::Result(json)) = decode_files_record(&record, RecordPlace::ResultFrame)
        else {
            panic!("expected a Result record");
        };
        assert_eq!(self.closed(id), CallClose::Complete);
        parse_reply(operation, &json).unwrap()
    }

    /// The next notice of subscription call `id`.
    fn notice(&self, id: u64, kind: NoticeKind) -> FilesNotice {
        notice_of(self.next(), id, kind)
    }

    /// The Results of request calls that complete in either order, as `expected` names them.
    fn replies(&self, expected: &[(u64, Operation)]) -> Vec<FilesReply> {
        let (mut results, mut closed) = (HashMap::new(), 0);
        while results.len() < expected.len() || closed < expected.len() {
            match self.next() {
                LinkMessage::Frame(LinkFrame {
                    call: answered,
                    kind: FrameKind::Result,
                    record,
                }) => {
                    let id = answered.get();
                    let (_, operation) = expected.iter().find(|(call, _)| *call == id).unwrap();
                    let record = decode_files_record(&record, RecordPlace::ResultFrame);
                    let Ok(FilesRecord::Result(json)) = record else {
                        panic!("expected a Result record");
                    };
                    results.insert(id, parse_reply(*operation, &json).unwrap());
                }
                LinkMessage::Close { reason, .. } => {
                    assert_eq!(reason, CallClose::Complete);
                    closed += 1;
                }
                other => panic!("expected a Result or a close, got {other:?}"),
            }
        }
        expected
            .iter()
            .map(|(id, _)| results.remove(id).unwrap())
            .collect()
    }

    /// The reply to `request`, a mutation, sent on call `id` with the lifecycle grant it needs.
    fn mutate(&self, id: u64, request: &FilesRequest) -> FilesReply {
        let record = encode_files_record(&FilesRecord::Request(encode_request(request))).unwrap();
        self.send(id, GrantClass::Lifecycle, request.workspace(), record);
        self.reply(id, request.operation())
    }

    /// `fs.write` of `text` under `request`, on call `id`, with its one Put on call `id + 1`.
    fn write(&self, id: u64, request: u8, path: &FilePath, text: &str) -> FilesReply {
        let transfer = TransferId::new(id).unwrap();
        let write = FsWrite {
            workspace_id: workspace(),
            request_id: RequestId::from_bytes([request; 16]).unwrap(),
            transfer,
            path: path.clone(),
            size: SafeInteger::new(text.len() as u64).unwrap(),
            encoding: EncodingName::new("utf8").unwrap(),
            bom: false,
            precondition: WritePrecondition::None,
            unlock: false,
            history: None,
        };
        let record = FilesRecord::Request(encode_request(&FilesRequest::FsWrite(write)));
        let record = encode_files_record(&record).unwrap();
        // A mutation needs the lifecycle grant.
        self.send(id, GrantClass::Lifecycle, workspace(), record);
        let put = Put {
            transfer,
            offset: 0,
            end: PutEnd::Commit,
            bytes: text.as_bytes().to_vec(),
        };
        self.put(id + 1, put);
        // A Put's answer is the empty Result `transfer.close` also answers.
        let expected = [(id, Operation::FsWrite), (id + 1, Operation::TransferClose)];
        let mut replies = self.replies(&expected);
        assert_eq!(
            replies.pop(),
            Some(FilesReply::Empty),
            "the Put's own answer"
        );
        replies.pop().unwrap()
    }
}

/// The notice `message` carries for subscription call `id`.
fn notice_of(message: LinkMessage, id: u64, kind: NoticeKind) -> FilesNotice {
    let LinkMessage::Frame(LinkFrame {
        call: streamed,
        kind: frame_kind,
        record,
    }) = message
    else {
        panic!("expected a notice of call {id}");
    };
    assert_eq!(streamed, call(id));
    let expected = match kind {
        NoticeKind::Snapshot => FrameKind::Snapshot,
        NoticeKind::Update => FrameKind::Update,
    };
    assert_eq!(frame_kind, expected);
    let Ok(FilesRecord::Notice(json)) = decode_files_record(&record, RecordPlace::NoticeFrame)
    else {
        panic!("expected a Notice record");
    };
    parse_notice(Operation::FsSubscribe, kind, &json).unwrap()
}

impl Drop for Link {
    fn drop(&mut self) {
        self.services.drain();
        self.outbound.close();
    }
}

fn workspace() -> WorkspaceId {
    WorkspaceId::from_bytes([7; 16]).unwrap()
}

fn call(id: u64) -> CallId {
    CallId::new(id).unwrap()
}

fn error(reply: FilesReply) -> ErrorCode {
    match reply {
        FilesReply::Error(error) => error.code,
        other => panic!("expected an error, got {other:?}"),
    }
}

fn grant(path: &FilePath, source: GrantSource) -> FilesRequest {
    FilesRequest::FsGrant(FsGrant {
        workspace_id: workspace(),
        path: path.clone(),
        source,
    })
}

#[test]
fn a_record_closes_by_its_validity_grant_class_and_workspace() {
    let link = Link::new();
    let some_path = FilePath::new(if cfg!(windows) { r"C:\w" } else { "/w" }).unwrap();
    link.send(1, GrantClass::Lifecycle, workspace(), b"OBFB".to_vec());
    assert_eq!(link.closed(1), CallClose::InvalidRequest);
    let unknown = format!(
        r#"{{"op":"fs.unknown","workspaceId":"{}"}}"#,
        "07".repeat(16)
    );
    let record = encode_files_record(&FilesRecord::Request(unknown.into_bytes())).unwrap();
    link.send(2, GrantClass::Lifecycle, workspace(), record);
    assert_eq!(link.closed(2), CallClose::Unsupported);

    let release = FilesRequest::FsUngrant(PathsRequest {
        workspace_id: workspace(),
        paths: List::new(vec![some_path.clone()]).unwrap(),
    });
    // A Workspace the Server never stated admits nothing.
    link.request(3, &release);
    assert_eq!(
        error(link.reply(3, Operation::FsUngrant)),
        ErrorCode::Denied
    );
    link.scope(WorkspaceScope::NoRoot);
    link.request(4, &release);
    assert_eq!(link.reply(4, Operation::FsUngrant), FilesReply::Empty);
    // The envelope names the Workspace the Server read from the record.
    let record = encode_files_record(&FilesRecord::Request(encode_request(&release))).unwrap();
    let other = WorkspaceId::from_bytes([8; 16]).unwrap();
    link.send(5, GrantClass::TerminalCreate, other, record);
    assert_eq!(link.closed(5), CallClose::InvalidRequest);

    let mark_safe = FilesRequest::GitMarkSafe(GitPath {
        workspace_id: workspace(),
        request_id: RequestId::from_bytes([9; 16]).unwrap(),
        path: some_path,
    });
    link.request(6, &mark_safe);
    assert_eq!(
        error(link.reply(6, Operation::GitMarkSafe)),
        ErrorCode::Denied
    );
    // The lifecycle grant admits it.
    let record = encode_files_record(&FilesRecord::Request(encode_request(&mark_safe))).unwrap();
    link.send(7, GrantClass::Lifecycle, workspace(), record);
    assert_ne!(
        error(link.reply(7, Operation::GitMarkSafe)),
        ErrorCode::Denied
    );

    // A Put answers on its own call; this one names no upload its connection opened.
    link.put(
        8,
        Put {
            transfer: TransferId::new(1).unwrap(),
            offset: 0,
            end: PutEnd::Commit,
            bytes: Vec::new(),
        },
    );
    assert_eq!(
        error(link.reply(8, Operation::GitStageContent)),
        ErrorCode::InvalidRequest
    );
}

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

/// A Workspace root with one file, and beside it a file and a folder outside the root.
struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!(
            "orchbee-files-operations-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(base.join("root")).unwrap();
        fs::create_dir(base.join("folder")).unwrap();
        fs::write(base.join("root").join("inside.txt"), b"inside").unwrap();
        fs::write(base.join("outside.txt"), b"outside").unwrap();
        Self(base)
    }

    fn path(&self, name: &str) -> String {
        self.0.join(name).to_str().unwrap().to_owned()
    }

    fn file(&self, name: &str) -> FilePath {
        FilePath::new(self.path(name)).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn grants_and_root_facts_follow_the_scope() {
    let fixture = Fixture::new();
    let link = Link::new();
    let root = fixture.path("root");
    link.scope(WorkspaceScope::Root(root.clone()));
    link.request(
        1,
        &FilesRequest::FsSubscribe(WorkspaceRequest {
            workspace_id: workspace(),
        }),
    );
    let real = path::real(&NativePath::parse(&root).unwrap()).unwrap();
    assert_eq!(
        link.notice(1, NoticeKind::Snapshot),
        FilesNotice::FsSnapshot(FsSnapshot {
            root: Root::Present(FilePath::new(real.as_str()).unwrap()),
            os: path::OPERATING_SYSTEM,
            separator: path::SEPARATOR,
            case_sensitive: path::CASE_SENSITIVE,
            watch: WatchState::Full,
        })
    );

    let inside = FilePath::new(format!(
        "{}{}inside.txt",
        real.as_str(),
        std::path::MAIN_SEPARATOR
    ))
    .unwrap();
    link.request(2, &grant(&inside, GrantSource::Tree));
    let FilesReply::FsGrant(FsGrantResult {
        entry: Some(granted),
    }) = link.reply(2, Operation::FsGrant)
    else {
        panic!("expected the granted entry");
    };
    assert_eq!(
        (granted.name.as_str(), granted.kind, granted.size.get()),
        ("inside.txt", EntryKind::File, 6)
    );
    let outside = fixture.file("outside.txt");
    link.request(3, &grant(&outside, GrantSource::Tree));
    assert_eq!(
        error(link.reply(3, Operation::FsGrant)),
        ErrorCode::OutsideScope
    );
    link.request(4, &grant(&outside, GrantSource::TerminalLink));
    assert!(matches!(
        link.reply(4, Operation::FsGrant),
        FilesReply::FsGrant(_)
    ));
    link.request(
        5,
        &grant(&fixture.file("folder"), GrantSource::TerminalLink),
    );
    assert_eq!(
        error(link.reply(5, Operation::FsGrant)),
        ErrorCode::IsDirectory
    );
    link.request(6, &grant(&fixture.file("missing.txt"), GrantSource::OsOpen));
    assert_eq!(
        error(link.reply(6, Operation::FsGrant)),
        ErrorCode::NotFound
    );

    // A new root reaches the open subscription; this one does not exist.
    let moved = fixture.path("moved");
    link.scope(WorkspaceScope::Root(moved.clone()));
    assert_eq!(
        link.notice(1, NoticeKind::Update),
        FilesNotice::FsUpdate(FsUpdate::Root {
            root: Root::Missing(FilePath::new(moved).unwrap()),
        })
    );
    link.request(
        7,
        &FilesRequest::FsUngrant(PathsRequest {
            workspace_id: workspace(),
            paths: List::new(vec![outside]).unwrap(),
        }),
    );
    assert_eq!(link.reply(7, Operation::FsUngrant), FilesReply::Empty);
    link.services.cancel(call(1));
    assert_eq!(link.closed(1), CallClose::Cancelled);
}

#[test]
fn a_listed_folder_reports_the_files_created_in_it() {
    let fixture = Fixture::new();
    let link = Link::new();
    link.scope(WorkspaceScope::Root(fixture.path("root")));
    link.request(
        1,
        &FilesRequest::FsSubscribe(WorkspaceRequest {
            workspace_id: workspace(),
        }),
    );
    let FilesNotice::FsSnapshot(FsSnapshot {
        root: Root::Present(root),
        ..
    }) = link.notice(1, NoticeKind::Snapshot)
    else {
        panic!("expected the present root");
    };
    let list = FsList {
        workspace_id: workspace(),
        path: root.clone(),
        cursor: None,
        single_child: false,
    };
    link.request(2, &FilesRequest::FsList(list));
    assert!(matches!(
        link.reply(2, Operation::FsList),
        FilesReply::FsList(_)
    ));
    // The native watch starts on the process's own threads after the listing answers, so files
    // are created until the subscription reports one added.
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut created = Vec::new();
    loop {
        assert!(Instant::now() < deadline, "no creation was reported");
        let name = format!("created-{}.txt", created.len());
        fs::write(fixture.0.join("root").join(&name), b"created").unwrap();
        created.push(format!(
            "{}{}{name}",
            root.as_str(),
            std::path::MAIN_SEPARATOR
        ));
        let Some(message) = link.within(Duration::from_millis(500)) else {
            continue;
        };
        let FilesNotice::FsUpdate(FsUpdate::Changes { changes }) =
            notice_of(message, 1, NoticeKind::Update)
        else {
            continue;
        };
        if changes.as_slice().iter().any(|change| {
            change.kind == ChangeKind::Added
                && created.iter().any(|path| path == change.path.as_str())
        }) {
            break;
        }
    }
}

#[test]
fn a_write_runs_once_under_its_identity_and_only_in_scope() {
    let fixture = Fixture::new();
    let link = Link::new();
    link.scope(WorkspaceScope::Root(fixture.path("root")));
    let inside = fixture.file(&format!("root{}inside.txt", std::path::MAIN_SEPARATOR));
    let FilesReply::FsWrite(written) = link.write(1, 1, &inside, "saved") else {
        panic!("expected the written file");
    };
    assert_eq!((written.size.get(), written.replaced.get()), (5, 0));
    assert_eq!(
        fs::read(fixture.0.join("root").join("inside.txt")).unwrap(),
        b"saved"
    );

    // A repeat of the identity answers what it recorded, and writes nothing.
    let repeated = link.write(3, 1, &inside, "other");
    assert_eq!(repeated, FilesReply::FsWrite(written.clone()));
    assert_eq!(
        fs::read(fixture.0.join("root").join("inside.txt")).unwrap(),
        b"saved"
    );
    let outcome = FilesRequest::RequestOutcome(RequestOutcome {
        workspace_id: workspace(),
        request_id: RequestId::from_bytes([1; 16]).unwrap(),
    });
    link.request(5, &outcome);
    assert_eq!(
        link.reply(5, Operation::RequestOutcome),
        FilesReply::RequestOutcome(RequestOutcomeResult {
            state: MutationState::Completed(MutationReply::FsWrite(written)),
        })
    );

    let outside = fixture.file("outside.txt");
    assert_eq!(
        error(link.write(6, 2, &outside, "x")),
        ErrorCode::OutsideScope
    );
    assert_eq!(fs::read(fixture.0.join("outside.txt")).unwrap(), b"outside");
}

#[test]
fn items_change_only_where_their_folder_is_in_scope() {
    let fixture = Fixture::new();
    let link = Link::new();
    link.scope(WorkspaceScope::Root(fixture.path("root")));
    let inside =
        |relative: &str| fixture.file(&format!("root{}{relative}", std::path::MAIN_SEPARATOR));
    let identity = |request: u8| RequestId::from_bytes([request; 16]).unwrap();
    let create = |request: u8, path: FilePath| {
        FilesRequest::FsCreate(FsCreate {
            workspace_id: workspace(),
            request_id: identity(request),
            path,
            kind: CreateKind::Directory,
        })
    };
    let transfer = |request: u8, sources: Vec<FilePath>, target_directory: FilePath| FsTransfer {
        workspace_id: workspace(),
        request_id: identity(request),
        sources: List::new(sources).unwrap(),
        target_directory,
        conflict: Conflict::Fail,
    };
    let made = inside("made");
    let FilesReply::FsEntry(created) = link.mutate(1, &create(1, made.clone())) else {
        panic!("expected the created folder");
    };
    assert_eq!(created.entry.kind, EntryKind::Directory);

    let copy = transfer(2, vec![inside("inside.txt")], made.clone());
    let copied = transferred(link.mutate(2, &FilesRequest::FsCopy(copy)));
    let in_made = |name: &str| inside(&format!("made{}{name}", std::path::MAIN_SEPARATOR));
    assert_eq!(copied, in_made("inside.txt"));
    let rename = FilesRequest::FsRename(FsRename {
        workspace_id: workspace(),
        request_id: identity(3),
        from: copied,
        to: in_made("renamed.txt"),
    });
    let FilesReply::FsEntry(renamed) = link.mutate(3, &rename) else {
        panic!("expected the renamed file");
    };
    assert_eq!(renamed.entry.name.as_str(), "renamed.txt");
    let moved = transfer(4, vec![inside("inside.txt")], made.clone());
    assert_eq!(
        transferred(link.mutate(4, &FilesRequest::FsMove(moved))),
        in_made("inside.txt")
    );

    let names = ["inside.txt", "missing", "renamed.txt"].map(|name| FileName::new(name).unwrap());
    let exists = FilesRequest::FsExists(FsExists {
        workspace_id: workspace(),
        directory: made,
        names: List::new(names.to_vec()).unwrap(),
    });
    let existing = [names[0].clone(), names[2].clone()];
    assert_eq!(
        link.mutate(5, &exists),
        FilesReply::FsExists(FsExistsResult {
            existing: List::new(existing.to_vec()).unwrap(),
        })
    );

    // Neither an item whose folder lies outside the root nor the root itself is changed.
    let beyond = fixture.file(&format!("folder{}new", std::path::MAIN_SEPARATOR));
    assert_eq!(
        error(link.mutate(6, &create(6, beyond))),
        ErrorCode::OutsideScope
    );
    let root = fixture.file("root");
    assert_eq!(
        error(link.mutate(7, &create(7, root.clone()))),
        ErrorCode::OutsideScope
    );
    assert!(!fixture.0.join("folder").join("new").exists());

    // A captured delete is restored once, and the root itself is never deleted.
    let delete = |request: u8, path: FilePath| {
        FilesRequest::FsDelete(FsDelete {
            workspace_id: workspace(),
            request_id: identity(request),
            paths: List::new(vec![path]).unwrap(),
            mode: DeleteMode::Permanent,
            capture: true,
        })
    };
    let FilesReply::FsDelete(deleted) = link.mutate(8, &delete(8, in_made("renamed.txt"))) else {
        panic!("expected the deleted items");
    };
    assert_eq!(deleted.items.as_slice()[0].error, None);
    let restore = |request: u8| {
        FilesRequest::FsRestore(FsRestore {
            workspace_id: workspace(),
            request_id: identity(request),
            stash: deleted.stash.unwrap(),
        })
    };
    let FilesReply::FsRestore(restored) = link.mutate(9, &restore(9)) else {
        panic!("expected the restored items");
    };
    assert_eq!(restored.items.as_slice()[0].error, None);
    let renamed = fixture.0.join("root").join("made").join("renamed.txt");
    assert_eq!(fs::read(renamed).unwrap(), b"inside");
    assert_eq!(error(link.mutate(10, &restore(10))), ErrorCode::UndoExpired);
    let FilesReply::FsDelete(refused) = link.mutate(11, &delete(11, root)) else {
        panic!("expected the refused item");
    };
    let refusal = refused.items.as_slice()[0]
        .error
        .as_ref()
        .map(|error| error.code);
    assert_eq!(refusal, Some(ErrorCode::OutsideScope));
    assert!(refused.stash.is_none());
}

/// The final path of the one item a transfer answered without an error.
fn transferred(reply: FilesReply) -> FilePath {
    let FilesReply::FsTransfer(result) = reply else {
        panic!("expected the transferred items, got {reply:?}");
    };
    let [item] = result.items.as_slice() else {
        panic!("expected one item");
    };
    assert_eq!(item.error, None);
    item.path.clone().unwrap()
}

#[test]
fn a_picker_and_the_clipboard_reach_beyond_the_root_only_as_granted() {
    let fixture = Fixture::new();
    let link = Link::new();
    link.scope(WorkspaceScope::Root(fixture.path("root")));
    let separator = std::path::MAIN_SEPARATOR;
    fs::write(fixture.0.join("folder").join("listed.txt"), b"listed").unwrap();
    let picker = SessionId::new(1).unwrap();
    let list = FilesRequest::PickerList(PickerList {
        workspace_id: workspace(),
        picker,
        path: fixture.file("folder"),
        cursor: None,
    });
    let opened = PickerRequest {
        workspace_id: workspace(),
        picker,
    };
    // A picker lists any folder while it is open, and its list may follow its open at once.
    link.request(1, &list);
    assert_eq!(
        error(link.reply(1, Operation::PickerList)),
        ErrorCode::InvalidRequest
    );
    link.request(2, &FilesRequest::PickerOpen(opened.clone()));
    link.request(3, &list);
    let replies = link.replies(&[(2, Operation::PickerOpen), (3, Operation::PickerList)]);
    let FilesReply::PickerOpen(places) = &replies[0] else {
        panic!("expected where the picker starts, got {:?}", replies[0]);
    };
    assert!(!places.roots.as_slice().is_empty());
    let FilesReply::PickerList(listed) = &replies[1] else {
        panic!("expected the folder's entries, got {:?}", replies[1]);
    };
    let names: Vec<_> = listed
        .entries
        .as_slice()
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(names, ["listed.txt"]);
    link.request(4, &FilesRequest::PickerClose(opened));
    assert_eq!(link.reply(4, Operation::PickerClose), FilesReply::Empty);
    link.request(5, &list);
    assert_eq!(
        error(link.reply(5, Operation::PickerList)),
        ErrorCode::InvalidRequest
    );

    // A Save As destination anywhere is granted before it exists, and the save creates it.
    let saved = fixture.file(&format!("folder{separator}saved.txt"));
    link.request(6, &grant(&saved, GrantSource::SaveAs));
    assert_eq!(
        link.reply(6, Operation::FsGrant),
        FilesReply::FsGrant(FsGrantResult { entry: None })
    );
    assert!(matches!(
        link.write(7, 7, &saved, "saved"),
        FilesReply::FsWrite(_)
    ));
    assert_eq!(
        fs::read(fixture.0.join("folder").join("saved.txt")).unwrap(),
        b"saved"
    );

    // Clipboard sources stay readable after the root moves, and only cut ones move.
    let clipboard = |paths: Vec<FilePath>, cut: bool| {
        FilesRequest::FsClipboard(FsClipboard {
            workspace_id: workspace(),
            paths: List::new(paths).unwrap(),
            cut,
        })
    };
    link.request(9, &clipboard(vec![fixture.file("outside.txt")], true));
    assert_eq!(
        error(link.reply(9, Operation::FsClipboard)),
        ErrorCode::OutsideScope
    );
    let source = fixture.file(&format!("root{separator}inside.txt"));
    link.request(10, &clipboard(vec![source.clone()], false));
    assert_eq!(link.reply(10, Operation::FsClipboard), FilesReply::Empty);
    fs::create_dir_all(fixture.0.join("elsewhere").join("moved")).unwrap();
    link.scope(WorkspaceScope::Root(fixture.path("elsewhere")));
    let transfer = |request: u8, target_directory: FilePath| FsTransfer {
        workspace_id: workspace(),
        request_id: RequestId::from_bytes([request; 16]).unwrap(),
        sources: List::new(vec![source.clone()]).unwrap(),
        target_directory,
        conflict: Conflict::Fail,
    };
    let elsewhere = fixture.file("elsewhere");
    let copy = FilesRequest::FsCopy(transfer(11, elsewhere.clone()));
    let copied = fixture.file(&format!("elsewhere{separator}inside.txt"));
    assert_eq!(transferred(link.mutate(11, &copy)), copied);
    let FilesReply::FsTransfer(refused) =
        link.mutate(12, &FilesRequest::FsMove(transfer(12, elsewhere)))
    else {
        panic!("expected the refused move");
    };
    let refusal = refused.items.as_slice()[0]
        .error
        .as_ref()
        .map(|error| error.code);
    assert_eq!(refusal, Some(ErrorCode::OutsideScope));
    link.scope(WorkspaceScope::Root(fixture.path("root")));
    link.request(13, &clipboard(vec![source.clone()], true));
    assert_eq!(link.reply(13, Operation::FsClipboard), FilesReply::Empty);
    link.scope(WorkspaceScope::Root(fixture.path("elsewhere")));
    let moved = fixture.file(&format!("elsewhere{separator}moved"));
    let moved = transferred(link.mutate(14, &FilesRequest::FsMove(transfer(14, moved))));
    assert_eq!(
        moved,
        fixture.file(&format!("elsewhere{separator}moved{separator}inside.txt"))
    );
    assert!(!fixture.0.join("root").join("inside.txt").exists());
}
