//! Local History through the dispatch: a save that names a reason is recorded, an entry reads back
//! through a read transfer, a restore writes an entry over its file under its identity and is
//! recorded in turn, a named entry takes no later save, deletions remove what they name, and a
//! rename takes the history along. A clone reaches beyond the root only through an open picker.

use std::{
    fs,
    path::{MAIN_SEPARATOR, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

use orchbee_files_protocol::{
    CallClose, Chunk, ChunkLength, Counter, Empty, ErrorCode, FilePath, FilesRecord, FilesReply,
    FilesRequest, FrameKind, FsRead, FsReadChunk, FsRename, GitClone, GitName, HistoryDelete,
    HistoryEntry, HistoryEntryId, HistoryRename, HistoryRestore, HistorySource, HistoryTarget,
    LinkFrame, LinkMessage, List, MutationReply, MutationState, Operation, PathRequest,
    PickerRequest, ReadMode, ReadSource, RecordPlace, RequestId, RequestOutcome,
    RequestOutcomeResult, SafeInteger, SessionId, Text, TransferId, WorkspaceScope, WriteHistory,
    decode_files_record,
};
use orchbee_local_runtime::DataRoot;

use super::{Fixture, Link, call, error, workspace};

/// A private folder for a data root, reached without a link on the way as the store requires,
/// removed when the test ends.
struct Private(PathBuf);

impl Private {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let base = orchbee_git_process::canonical_path(&std::env::temp_dir()).unwrap();
        let path = Path::new(base.as_native()).join(format!(
            "orchbee-files-history-link-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut fs::DirBuilder::new(), 0o700)
            .create(&path)
            .unwrap();
        #[cfg(not(unix))]
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Private {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Call identities, each used once; every request below takes at most two.
struct Calls(u64);

impl Calls {
    fn next(&mut self) -> u64 {
        self.0 += 2;
        self.0
    }
}

fn identity(byte: u8) -> RequestId {
    RequestId::from_bytes([byte; 16]).unwrap()
}

/// The entries of the file at `path`, oldest first, once `settled` holds of them: a save, a
/// restore or a move records after it answers, so `history.list` is asked again until then.
fn listed(
    link: &Link,
    calls: &mut Calls,
    path: &FilePath,
    settled: impl Fn(&[HistoryEntry]) -> bool,
) -> Vec<HistoryEntry> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let id = calls.next();
        let list = FilesRequest::HistoryList(PathRequest {
            workspace_id: workspace(),
            path: path.clone(),
        });
        link.request(id, &list);
        let FilesReply::HistoryList(listed) = link.reply(id, Operation::HistoryList) else {
            panic!("expected the entries");
        };
        let entries = listed.entries.as_slice().to_vec();
        if settled(&entries) {
            return entries;
        }
        assert!(Instant::now() < deadline, "the history never settled");
        thread::sleep(Duration::from_millis(10));
    }
}

/// Each entry's source, size and label.
fn shape(entries: &[HistoryEntry]) -> Vec<(HistorySource, u64, Option<&str>)> {
    entries
        .iter()
        .map(|entry| {
            let label = entry.label.as_ref().map(Text::as_str);
            (entry.source, entry.size.get(), label)
        })
        .collect()
}

/// The content entry `entry` of the file at `path` keeps, through a read transfer.
fn content(link: &Link, calls: &mut Calls, path: &FilePath, entry: HistoryEntryId) -> Vec<u8> {
    let id = calls.next();
    let transfer = TransferId::new(id).unwrap();
    let read = FilesRequest::FsRead(FsRead {
        workspace_id: workspace(),
        transfer,
        source: ReadSource::History {
            path: path.clone(),
            entry,
        },
        mode: ReadMode::Bytes,
        encoding: None,
        force_text: false,
        limit: SafeInteger::new(1 << 20).unwrap(),
    });
    link.request(id, &read);
    let FilesReply::Read(facts) = link.reply(id, Operation::FsRead) else {
        panic!("expected the read's facts");
    };
    let chunk = FilesRequest::FsReadChunk(FsReadChunk {
        workspace_id: workspace(),
        transfer,
        offset: Counter::new(0),
        length: ChunkLength::new(16_384).unwrap(),
    });
    link.request(id + 1, &chunk);
    let LinkMessage::Frame(LinkFrame {
        call: answered,
        kind: FrameKind::Result,
        record,
    }) = link.next()
    else {
        panic!("expected the chunk");
    };
    assert_eq!(answered, call(id + 1));
    let Ok(FilesRecord::Chunk(Chunk { bytes, end, .. })) =
        decode_files_record(&record, RecordPlace::ResultFrame)
    else {
        panic!("expected a Chunk record");
    };
    assert_eq!(link.closed(id + 1), CallClose::Complete);
    assert!(end.is_some(), "the whole content in one chunk");
    assert_eq!(facts.size.get(), bytes.len() as u64);
    bytes
}

#[test]
fn saves_are_recorded_read_restored_named_deleted_and_follow_a_rename() {
    let data = Private::new();
    let fixture = Fixture::new();
    let link = Link::with_root(Some(DataRoot::at(data.0.join("data"))));
    link.scope(WorkspaceScope::Root(fixture.path("root")));
    let file = fixture.file(&format!("root{MAIN_SEPARATOR}inside.txt"));
    let on_disk = fixture.0.join("root").join("inside.txt");
    let mut calls = Calls(0);

    // A save without a reason records nothing.
    assert!(matches!(
        link.write(calls.next(), 1, &file, "unrecorded"),
        FilesReply::FsWrite(_)
    ));
    let save = |calls: &mut Calls, request: u8, text: &str| {
        let saved = link.save(calls.next(), request, &file, text, Some(WriteHistory::Save));
        assert!(matches!(saved, FilesReply::FsWrite(_)));
    };
    save(&mut calls, 2, "one");
    let entries = listed(&link, &mut calls, &file, |entries| !entries.is_empty());
    assert_eq!(shape(&entries), [(HistorySource::Save, 3, None)]);
    let one = entries[0].id;
    assert_eq!(content(&link, &mut calls, &file, one), b"one");

    // A named entry never takes a later save in its place.
    let name = FilesRequest::HistoryRename(HistoryRename {
        workspace_id: workspace(),
        request_id: identity(3),
        path: file.clone(),
        entry: one,
        label: Text::new("kept").unwrap(),
    });
    assert_eq!(link.mutate(calls.next(), &name), FilesReply::Empty);
    save(&mut calls, 4, "two!");
    let entries = listed(&link, &mut calls, &file, |entries| entries.len() == 2);
    assert_eq!(
        shape(&entries),
        [
            (HistorySource::Save, 3, Some("kept")),
            (HistorySource::Save, 4, None)
        ]
    );
    let two = entries[1].id;

    // A restore writes the entry over its file once under its identity, and is recorded.
    let restore = |request: u8, entry: HistoryEntryId| {
        FilesRequest::HistoryRestore(HistoryRestore {
            workspace_id: workspace(),
            request_id: identity(request),
            path: file.clone(),
            entry,
        })
    };
    assert_eq!(
        link.mutate(calls.next(), &restore(5, one)),
        FilesReply::Empty
    );
    assert_eq!(fs::read(&on_disk).unwrap(), b"one");
    let outcome = FilesRequest::RequestOutcome(RequestOutcome {
        workspace_id: workspace(),
        request_id: identity(5),
    });
    let id = calls.next();
    link.request(id, &outcome);
    assert_eq!(
        link.reply(id, Operation::RequestOutcome),
        FilesReply::RequestOutcome(RequestOutcomeResult {
            state: MutationState::Completed(MutationReply::Empty(Empty {})),
        })
    );
    let unknown = HistoryEntryId::from_bytes([9; 16]).unwrap();
    assert_eq!(
        error(link.mutate(calls.next(), &restore(6, unknown))),
        ErrorCode::NotFound
    );
    let entries = listed(&link, &mut calls, &file, |entries| entries.len() == 3);
    assert_eq!(shape(&entries)[2], (HistorySource::Restore, 3, None));

    // A rename takes the history along and records the file at its new path.
    let renamed = fixture.file(&format!("root{MAIN_SEPARATOR}renamed.txt"));
    let rename = FilesRequest::FsRename(FsRename {
        workspace_id: workspace(),
        request_id: identity(7),
        from: file.clone(),
        to: renamed.clone(),
    });
    assert!(matches!(
        link.mutate(calls.next(), &rename),
        FilesReply::FsEntry(_)
    ));
    let entries = listed(&link, &mut calls, &renamed, |entries| entries.len() == 4);
    let sources = entries.iter().map(|entry| entry.source).collect::<Vec<_>>();
    assert_eq!(
        sources,
        [
            HistorySource::Save,
            HistorySource::Save,
            HistorySource::Restore,
            HistorySource::Rename
        ]
    );
    assert_eq!((entries[0].id, entries[1].id), (one, two));
    assert!(listed(&link, &mut calls, &file, |_| true).is_empty());

    // Deleting entries keeps the others; deleting every entry leaves no history.
    let delete = FilesRequest::HistoryDelete(HistoryDelete {
        workspace_id: workspace(),
        request_id: identity(8),
        target: HistoryTarget::Entries {
            path: renamed.clone(),
            entries: List::new(vec![two]).unwrap(),
        },
    });
    assert_eq!(link.mutate(calls.next(), &delete), FilesReply::Empty);
    let entries = listed(&link, &mut calls, &renamed, |_| true);
    assert!(entries.iter().all(|entry| entry.id != two) && entries.len() == 3);
    let clear = FilesRequest::HistoryDelete(HistoryDelete {
        workspace_id: workspace(),
        request_id: identity(9),
        target: HistoryTarget::All,
    });
    assert_eq!(link.mutate(calls.next(), &clear), FilesReply::Empty);
    assert!(listed(&link, &mut calls, &renamed, |_| true).is_empty());
}

#[test]
fn a_clone_reaches_beyond_the_root_only_through_an_open_picker() {
    let fixture = Fixture::new();
    let link = Link::new();
    link.scope(WorkspaceScope::Root(fixture.path("root")));
    let picker = SessionId::new(1).unwrap();
    let clone = |request: u8| {
        FilesRequest::GitClone(GitClone {
            workspace_id: workspace(),
            request_id: identity(request),
            picker,
            parent: fixture.file("folder"),
            url: GitName::new(fixture.path("missing")).unwrap(),
            recursive: false,
        })
    };
    assert_eq!(error(link.mutate(1, &clone(1))), ErrorCode::InvalidRequest);
    let opened = PickerRequest {
        workspace_id: workspace(),
        picker,
    };
    link.request(2, &FilesRequest::PickerOpen(opened));
    link.reply(2, Operation::PickerOpen);
    // Granted, the clone runs, and Git finds nothing to clone.
    assert_eq!(error(link.mutate(3, &clone(3))), ErrorCode::GitFailed);
}
