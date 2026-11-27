use std::{
    fs,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

use orchbee_files_protocol::{
    CallClose, CallId, ConnectionId, EntryKind, ErrorCode, FilePath, FilesNotice, FilesRecord,
    FilesReply, FilesRequest, FrameKind, FsGrant, FsSnapshot, FsUpdate, GitPath, GrantClass,
    GrantSource, LaneId, LinkFrame, LinkMessage, LinkRequest, LinkSide, List, NoticeKind,
    Operation, PathsRequest, RecordPlace, RequestId, Root, ScopeUpdate, WatchState, WorkspaceId,
    WorkspaceRequest, WorkspaceScope, decode_files_record, decode_link_message,
    encode_files_record, encode_request, parse_notice, parse_reply,
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
        let services = Services::new(Calls::new(Arc::clone(&outbound)));
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

    fn next(&self) -> LinkMessage {
        let frame = self
            .frames
            .recv_timeout(Duration::from_secs(20))
            .expect("the process sends a frame");
        decode_link_message(&frame, LinkSide::Files).unwrap()
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
        let LinkMessage::Frame(LinkFrame {
            call: streamed,
            kind: frame_kind,
            record,
        }) = self.next()
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
    let record = encode_files_record(&FilesRecord::Request(encode_request(&mark_safe))).unwrap();
    link.send(7, GrantClass::Lifecycle, workspace(), record);
    assert_eq!(link.closed(7), CallClose::Unsupported);
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
            watch: WatchState::Stopped,
        })
    );

    let inside = FilePath::new(format!(
        "{}{}inside.txt",
        real.as_str(),
        std::path::MAIN_SEPARATOR
    ))
    .unwrap();
    link.request(2, &grant(&inside, GrantSource::Tree));
    let FilesReply::FsGrant(granted) = link.reply(2, Operation::FsGrant) else {
        panic!("expected the granted entry");
    };
    assert_eq!(
        (
            granted.entry.name.as_str(),
            granted.entry.kind,
            granted.entry.size.get()
        ),
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
