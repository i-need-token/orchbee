use super::{
    Answer, Backend, BlobCache, COALESCE, Coalescer, Notice, Request, WatchEvent, WatchRoot,
    WatchSet, answer, serve, serve_watch,
};
use base64::Engine as _;
use orchbee_workbench_protocol::{
    WORKSPACE_FILE_CHUNK_BYTES, WorkspaceEntryKind, WorkspaceFileContent,
    WorkspaceFileEncoding as Encoding, WorkspaceFileSource, WorkspaceFileVersion,
    WorkspaceFilesFailure, WorkspaceFilesOperation, WorkspaceFilesOutcome,
    WorkspaceGitStatusCode as Code,
};
use sha2::Digest as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// A temporary directory removed on drop.
struct Scratch(PathBuf);
impl Scratch {
    fn new(name: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "orchbee-server-workspace-files-{name}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _removed = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(std::fs::canonicalize(path).unwrap())
    }
    fn write(&self, relative: &str, bytes: impl AsRef<[u8]>) {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    fn root(&self) -> String {
        self.0.to_str().unwrap().to_owned()
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _removed = std::fs::remove_dir_all(&self.0);
    }
}

fn git(directory: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .current_dir(directory)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

/// A repository with one commit, then a modified, a staged, a renamed, a deleted and an
/// untracked file, and ignore rules for a file and a directory.
fn repository() -> Scratch {
    let scratch = Scratch::new("repo");
    git(&scratch.0, &["init", "-q"]);
    scratch.write(".gitignore", "*.log\nbuild/\n");
    scratch.write("README.md", "one\n");
    scratch.write("src/main.rs", "fn main() {}\n");
    scratch.write("src/old.rs", "// old\n");
    scratch.write("gone.txt", "bye\n");
    git(&scratch.0, &["add", "-A"]);
    git(&scratch.0, &["commit", "-q", "-m", "first"]);
    scratch.write("README.md", "one\ntwo\n");
    scratch.write("added.txt", "new\n");
    git(&scratch.0, &["add", "added.txt"]);
    git(&scratch.0, &["mv", "src/old.rs", "src/new.rs"]);
    std::fs::remove_file(scratch.0.join("gone.txt")).unwrap();
    scratch.write("notes/todo.md", "later\n");
    scratch.write("debug.log", "noise\n");
    scratch.write("build/out.bin", [0_u8, 1, 2]);
    scratch
}

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(10)
}

fn list(root: &str, path: &str) -> WorkspaceFilesOutcome {
    answer(
        root,
        &WorkspaceFilesOperation::ListDirectory { path: path.into() },
        &BlobCache::default(),
        deadline(),
    )
}

fn read_at(
    root: &str,
    path: &str,
    source: WorkspaceFileSource,
    offset: u64,
    encoding: Encoding,
    cache: &BlobCache,
) -> WorkspaceFilesOutcome {
    let outcome = answer(
        root,
        &WorkspaceFilesOperation::ReadFile {
            path: path.into(),
            source,
            offset,
            encoding,
        },
        cache,
        deadline(),
    );
    outcome.validate().unwrap();
    outcome
}

fn read(root: &str, path: &str, source: WorkspaceFileSource) -> WorkspaceFilesOutcome {
    read_at(root, path, source, 0, Encoding::Text, &BlobCache::default())
}

/// The text of a file that fits one chunk.
fn text(outcome: &WorkspaceFilesOutcome) -> Option<&str> {
    match outcome {
        WorkspaceFilesOutcome::File {
            content:
                WorkspaceFileContent::Text {
                    text,
                    next_offset: None,
                    ..
                },
            ..
        } => Some(text),
        _ => None,
    }
}

/// Every chunk of a file, followed from offset to next offset, and the bytes they join to.
fn chunks(
    root: &str,
    path: &str,
    source: WorkspaceFileSource,
    encoding: Encoding,
) -> (Vec<WorkspaceFilesOutcome>, Vec<u8>) {
    let cache = BlobCache::default();
    let mut outcomes = Vec::new();
    let mut joined = Vec::new();
    let mut offset = Some(0);
    while let Some(at) = offset {
        let outcome = read_at(root, path, source, at, encoding, &cache);
        offset = match &outcome {
            WorkspaceFilesOutcome::File {
                content:
                    WorkspaceFileContent::Text {
                        offset,
                        text,
                        next_offset,
                    },
                ..
            } => {
                assert_eq!(*offset, at);
                joined.extend_from_slice(text.as_bytes());
                *next_offset
            }
            WorkspaceFilesOutcome::File {
                content:
                    WorkspaceFileContent::Bytes {
                        offset,
                        base64,
                        next_offset,
                    },
                ..
            } => {
                assert_eq!(*offset, at);
                joined.extend(
                    base64::engine::general_purpose::STANDARD
                        .decode(base64)
                        .unwrap(),
                );
                *next_offset
            }
            other => panic!("not a chunk: {other:?}"),
        };
        outcomes.push(outcome);
    }
    (outcomes, joined)
}

fn version(outcome: &WorkspaceFilesOutcome) -> Option<&WorkspaceFileVersion> {
    match outcome {
        WorkspaceFilesOutcome::File { version, .. } => version.as_ref(),
        _ => None,
    }
}

/// A file of `length` bytes that occupies no disk, for size limits.
fn sparse(scratch: &Scratch, relative: &str, length: u64) {
    std::fs::File::create(scratch.0.join(relative))
        .unwrap()
        .set_len(length)
        .unwrap();
}

#[test]
fn lists_directories_first_with_ignore_facts_and_without_git_metadata() {
    let scratch = repository();
    let WorkspaceFilesOutcome::Directory { entries, truncated } = list(&scratch.root(), "") else {
        panic!("not a listing");
    };
    assert!(!truncated);
    let names: Vec<_> = entries.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "build",
            "notes",
            "src",
            ".gitignore",
            "added.txt",
            "debug.log",
            "README.md"
        ]
    );
    let ignored: Vec<_> = entries
        .iter()
        .filter(|entry| entry.ignored)
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(ignored, ["build", "debug.log"]);
    assert_eq!(entries[0].entry_kind, WorkspaceEntryKind::Directory);
    let WorkspaceFilesOutcome::Directory { entries, .. } = list(&scratch.root(), "src") else {
        panic!("not a listing");
    };
    let names: Vec<_> = entries.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(names, ["main.rs", "new.rs"]);
}

#[test]
fn refuses_paths_that_leave_the_workspace() {
    let scratch = repository();
    let outside = Scratch::new("outside");
    outside.write("secret.txt", "no\n");
    for path in ["../x", "src/../../x", "/etc", "a\\b", ""] {
        assert_eq!(
            read(&scratch.root(), path, WorkspaceFileSource::Worktree),
            WorkspaceFilesOutcome::Failed {
                reason: WorkspaceFilesFailure::Malformed
            },
            "{path}"
        );
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&outside.0, scratch.0.join("escape")).unwrap();
        std::os::unix::fs::symlink(outside.0.join("secret.txt"), scratch.0.join("leak.txt"))
            .unwrap();
        let denied = WorkspaceFilesOutcome::Failed {
            reason: WorkspaceFilesFailure::Denied,
        };
        assert_eq!(list(&scratch.root(), "escape"), denied);
        assert_eq!(
            read(
                &scratch.root(),
                "escape/secret.txt",
                WorkspaceFileSource::Worktree
            ),
            denied
        );
        assert_eq!(
            read(&scratch.root(), "leak.txt", WorkspaceFileSource::Worktree),
            denied
        );
        let WorkspaceFilesOutcome::Directory { entries, .. } = list(&scratch.root(), "") else {
            panic!("not a listing");
        };
        let escape = entries.iter().find(|entry| entry.name == "escape").unwrap();
        assert!(escape.symbolic_link);
        assert_eq!(escape.entry_kind, WorkspaceEntryKind::Directory);
    }
    assert_eq!(
        list("relative/root", ""),
        WorkspaceFilesOutcome::Failed {
            reason: WorkspaceFilesFailure::Denied
        }
    );
}

#[test]
fn reads_the_worktree_index_and_head() {
    let scratch = repository();
    let root = scratch.root();
    assert_eq!(
        text(&read(&root, "README.md", WorkspaceFileSource::Worktree)),
        Some("one\ntwo\n")
    );
    assert_eq!(
        text(&read(&root, "README.md", WorkspaceFileSource::Head)),
        Some("one\n")
    );
    assert_eq!(
        text(&read(&root, "README.md", WorkspaceFileSource::Index)),
        Some("one\n")
    );
    assert_eq!(
        text(&read(&root, "added.txt", WorkspaceFileSource::Index)),
        Some("new\n")
    );
    let absent = WorkspaceFilesOutcome::File {
        byte_length: 0,
        version: None,
        content: WorkspaceFileContent::Absent {},
    };
    assert_eq!(read(&root, "added.txt", WorkspaceFileSource::Head), absent);
    assert_eq!(
        read(&root, "gone.txt", WorkspaceFileSource::Worktree),
        absent
    );
    assert_eq!(
        text(&read(&root, "gone.txt", WorkspaceFileSource::Head)),
        Some("bye\n")
    );
    assert!(matches!(
        read(&root, "build/out.bin", WorkspaceFileSource::Worktree),
        WorkspaceFilesOutcome::File {
            byte_length: 3,
            version: Some(_),
            content: WorkspaceFileContent::Binary {}
        }
    ));
    assert_eq!(
        read(&root, "src", WorkspaceFileSource::Worktree),
        WorkspaceFilesOutcome::Failed {
            reason: WorkspaceFilesFailure::NotFound
        }
    );
}

#[test]
fn reads_large_text_in_chunks_that_end_on_whole_characters() {
    let scratch = Scratch::new("chunks");
    let root = scratch.root();
    // Push a three-byte character across the first chunk boundary.
    let mut original = vec![b'a'; WORKSPACE_FILE_CHUNK_BYTES - 2];
    original.extend_from_slice("\u{6587}".as_bytes());
    for line in 0..40_000 {
        original.extend_from_slice(format!("{line} \u{6587}\n").as_bytes());
    }
    scratch.write("big.txt", &original);
    let (outcomes, joined) = chunks(
        &root,
        "big.txt",
        WorkspaceFileSource::Worktree,
        Encoding::Text,
    );
    assert_eq!(joined, original);
    assert!(outcomes.len() > 2);
    let WorkspaceFilesOutcome::File {
        content: WorkspaceFileContent::Text { next_offset, .. },
        ..
    } = &outcomes[0]
    else {
        unreachable!();
    };
    assert_eq!(*next_offset, Some(WORKSPACE_FILE_CHUNK_BYTES as u64 - 2));
    // The first chunk names the whole file's hash; every chunk names the same modification.
    let first = version(&outcomes[0]).unwrap();
    assert_eq!(first.sha256.as_ref().map(String::len), Some(64));
    for outcome in &outcomes[1..] {
        let later = version(outcome).unwrap();
        assert_eq!(
            (later.modified.as_str(), &later.sha256),
            (first.modified.as_str(), &None)
        );
    }
    // A chunk of control characters escapes to six times its size and still fits the result cap.
    scratch.write(
        "controls.txt",
        vec![0x01_u8; 2 * WORKSPACE_FILE_CHUNK_BYTES],
    );
    let (_, joined) = chunks(
        &root,
        "controls.txt",
        WorkspaceFileSource::Worktree,
        Encoding::Text,
    );
    assert_eq!(joined.len(), 2 * WORKSPACE_FILE_CHUNK_BYTES);
}

#[test]
fn hashes_only_what_can_be_edited_and_opens_nothing_past_its_limit() {
    let scratch = Scratch::new("limits");
    let root = scratch.root();
    let editable = orchbee_workbench_protocol::MAXIMUM_WORKSPACE_FILE_EDITABLE_BYTES;
    scratch.write("small.txt", "same\n");
    let expected: String = sha2::Sha256::digest(b"same\n")
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(
        version(&read(&root, "small.txt", WorkspaceFileSource::Worktree))
            .and_then(|version| version.sha256.as_deref()),
        Some(expected.as_str())
    );
    scratch.write(
        "read-only.txt",
        vec![b'a'; usize::try_from(editable).unwrap() + 1],
    );
    let read_only = read(&root, "read-only.txt", WorkspaceFileSource::Worktree);
    assert!(matches!(
        read_only,
        WorkspaceFilesOutcome::File {
            content: WorkspaceFileContent::Text { .. },
            ..
        }
    ));
    assert_eq!(version(&read_only).unwrap().sha256, None);
    let open = orchbee_workbench_protocol::MAXIMUM_WORKSPACE_FILE_OPEN_BYTES;
    sparse(&scratch, "huge.txt", open + 1);
    assert!(matches!(
        read(&root, "huge.txt", WorkspaceFileSource::Worktree),
        WorkspaceFilesOutcome::File {
            content: WorkspaceFileContent::TooLarge {},
            version: Some(_),
            ..
        }
    ));
    let bytes = orchbee_workbench_protocol::MAXIMUM_WORKSPACE_FILE_BYTES_READ;
    sparse(&scratch, "image.png", bytes + 1);
    assert!(matches!(
        read_at(
            &root,
            "image.png",
            WorkspaceFileSource::Worktree,
            0,
            Encoding::Bytes,
            &BlobCache::default()
        ),
        WorkspaceFilesOutcome::File {
            content: WorkspaceFileContent::TooLarge {},
            ..
        }
    ));
    assert_eq!(
        read_at(
            &root,
            "small.txt",
            WorkspaceFileSource::Worktree,
            6,
            Encoding::Text,
            &BlobCache::default()
        ),
        WorkspaceFilesOutcome::Failed {
            reason: WorkspaceFilesFailure::Malformed
        }
    );
}

#[test]
fn reads_bytes_and_committed_blobs_in_chunks() {
    let scratch = Scratch::new("blobs");
    let root = scratch.root();
    git(&scratch.0, &["init", "-q"]);
    let image: Vec<u8> = (0..WORKSPACE_FILE_CHUNK_BYTES * 2 + 7)
        .map(|index| u8::try_from(index % 251).unwrap())
        .collect();
    scratch.write("image.png", &image);
    let committed = "line\n".repeat(WORKSPACE_FILE_CHUNK_BYTES / 4);
    scratch.write("long.txt", &committed);
    git(&scratch.0, &["add", "-A"]);
    git(&scratch.0, &["commit", "-q", "-m", "first"]);
    scratch.write("long.txt", "short\n");
    let (outcomes, joined) = chunks(
        &root,
        "image.png",
        WorkspaceFileSource::Worktree,
        Encoding::Bytes,
    );
    assert_eq!((outcomes.len(), joined), (3, image.clone()));
    assert!(matches!(
        read(&root, "image.png", WorkspaceFileSource::Worktree),
        WorkspaceFilesOutcome::File {
            content: WorkspaceFileContent::Binary {},
            ..
        }
    ));
    for source in [WorkspaceFileSource::Head, WorkspaceFileSource::Index] {
        let (outcomes, joined) = chunks(&root, "long.txt", source, Encoding::Text);
        assert_eq!(joined, committed.as_bytes());
        assert!(outcomes.len() > 1);
        assert!(outcomes.iter().all(|outcome| version(outcome).is_none()));
        let (_, joined) = chunks(&root, "image.png", source, Encoding::Bytes);
        assert_eq!(joined, image);
    }
}

#[test]
fn reports_the_change_list_relative_to_the_workspace() {
    let scratch = repository();
    let WorkspaceFilesOutcome::GitStatus {
        branch,
        entries,
        truncated,
    } = answer(
        &scratch.root(),
        &WorkspaceFilesOperation::GitStatus {},
        &BlobCache::default(),
        deadline(),
    )
    else {
        panic!("not a change list");
    };
    assert_eq!(branch.as_deref(), Some("main"));
    assert!(!truncated);
    let mut facts: Vec<_> = entries
        .iter()
        .map(|entry| {
            (
                entry.path.as_str(),
                entry.original_path.as_deref(),
                entry.index,
                entry.worktree,
            )
        })
        .collect();
    facts.sort_unstable_by_key(|fact| fact.0);
    assert_eq!(
        facts,
        [
            ("README.md", None, Code::Unmodified, Code::Modified),
            ("added.txt", None, Code::Added, Code::Unmodified),
            ("gone.txt", None, Code::Unmodified, Code::Deleted),
            ("notes/todo.md", None, Code::Untracked, Code::Untracked),
            (
                "src/new.rs",
                Some("src/old.rs"),
                Code::Renamed,
                Code::Unmodified
            ),
        ]
    );
    // From a subdirectory, paths are relative to it and changes elsewhere are left out.
    let WorkspaceFilesOutcome::GitStatus { entries, .. } = answer(
        &format!("{}/src", scratch.root()),
        &WorkspaceFilesOperation::GitStatus {},
        &BlobCache::default(),
        deadline(),
    ) else {
        panic!("not a change list");
    };
    let paths: Vec<_> = entries.iter().map(|entry| entry.path.as_str()).collect();
    assert_eq!(paths, ["new.rs"]);
    assert_eq!(entries[0].original_path.as_deref(), Some("old.rs"));
}

#[test]
fn a_plain_directory_lists_but_has_no_git_facts() {
    let scratch = Scratch::new("plain");
    scratch.write("a.txt", "a\n");
    let WorkspaceFilesOutcome::Directory { entries, .. } = list(&scratch.root(), "") else {
        panic!("not a listing");
    };
    assert!(!entries[0].ignored);
    let not_repository = WorkspaceFilesOutcome::Failed {
        reason: WorkspaceFilesFailure::NotRepository,
    };
    // A scratch directory under a repository's tree would be inside it; the system temporary
    // directory is not.
    assert_eq!(
        answer(
            &scratch.root(),
            &WorkspaceFilesOperation::GitStatus {},
            &BlobCache::default(),
            deadline()
        ),
        not_repository
    );
    assert_eq!(
        read(&scratch.root(), "a.txt", WorkspaceFileSource::Head),
        not_repository
    );
}

#[test]
fn parses_conflicts_and_drops_a_cut_record() {
    let output = b"# branch.oid (initial)\0# branch.head (detached)\0\
u UU N... 100644 100644 100644 100644 aaaa bbbb cccc both.txt\0\
1 .M N... 100644 100644 100644 aaaa aaaa with space.txt\0\
? sub/x.txt\0? outside.txt\0? sub/partial";
    let outcome = super::status::parse_status(output, "sub/", true).unwrap();
    assert_eq!(
        outcome,
        WorkspaceFilesOutcome::GitStatus {
            branch: None,
            entries: vec![orchbee_workbench_protocol::WorkspaceGitStatusEntry {
                path: "x.txt".into(),
                original_path: None,
                index: Code::Untracked,
                worktree: Code::Untracked,
                conflicted: false,
            }],
            truncated: true,
        }
    );
    let outcome = super::status::parse_status(&output[..output.len() - 13], "", false).unwrap();
    let WorkspaceFilesOutcome::GitStatus { entries, .. } = outcome else {
        panic!("not a change list");
    };
    assert!(entries[0].conflicted);
    assert_eq!(entries[0].index, Code::Unmerged);
    assert_eq!(entries[1].path, "with space.txt");
    assert_eq!(entries.len(), 4);
}

#[test]
fn serves_lines_in_order_and_skips_what_is_not_a_request() {
    let scratch = repository();
    let request = |id, path: &str| {
        let mut line = serde_json::to_vec(&Request {
            id,
            root: scratch.root(),
            operation: WorkspaceFilesOperation::ReadFile {
                path: path.into(),
                source: WorkspaceFileSource::Worktree,
                offset: 0,
                encoding: Encoding::Text,
            },
        })
        .unwrap();
        line.push(b'\n');
        line
    };
    let mut input = request(7, "README.md");
    input.extend_from_slice(b"not json\n");
    input.extend(request(8, "added.txt"));
    let mut output = Vec::new();
    let activity = AtomicU64::new(0);
    serve(&input[..], &mut output, &activity);
    assert_eq!(activity.load(Ordering::Relaxed) % 2, 0);
    let answers: Vec<Answer> = output
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).unwrap())
        .collect();
    assert_eq!(answers.len(), 2);
    assert_eq!((answers[0].id, answers[1].id), (7, 8));
    assert_eq!(text(&answers[1].outcome), Some("new\n"));
}

fn watch_root(root: &str, directories: &[&str], files: &[&str]) -> WatchRoot {
    WatchRoot {
        root: root.to_owned(),
        directories: directories.iter().map(|path| (*path).to_owned()).collect(),
        files: files.iter().map(|path| (*path).to_owned()).collect(),
    }
}

fn changed(root: &str, files: bool, repository: bool) -> WatchEvent {
    WatchEvent::Changed {
        root: root.to_owned(),
        files,
        repository,
    }
}

#[test]
fn a_change_moves_what_it_can_change_and_git_at_work_moves_nothing() {
    let workspace = Path::new("/work/app");
    let root = workspace.to_string_lossy().into_owned();
    let nested = workspace.join("packages/lib");
    let nested_root = nested.to_string_lossy().into_owned();
    let mut coalescer = Coalescer::default();
    coalescer.replace(&[
        watch_root(&root, &["", "src"], &["docs/guide.md"]),
        watch_root(&nested_root, &[], &[]),
    ]);
    let after = |coalescer: &mut Coalescer, relative: &str| {
        coalescer.record(&Notice::Path(workspace.join(relative)));
        coalescer.drain()
    };
    // An entry of a shown directory, the shown directory itself, and an open file are shown.
    for relative in ["README.md", "src/main.rs", "src", "docs/guide.md"] {
        assert_eq!(
            after(&mut coalescer, relative),
            [changed(&root, true, true)],
            "{relative}"
        );
    }
    // A file in a directory nobody shows changes only the change list.
    assert_eq!(
        after(&mut coalescer, "docs/other.md"),
        [changed(&root, false, true)]
    );
    // The index, HEAD and references change the change list; Git's other work changes nothing.
    for relative in [
        ".git/index",
        ".git/HEAD",
        ".git/refs/heads/main",
        ".git/packed-refs",
    ] {
        assert_eq!(
            after(&mut coalescer, relative),
            [changed(&root, false, true)],
            "{relative}"
        );
    }
    for relative in [
        ".git/objects/ab/cdef",
        ".git/index.lock",
        ".git/refs/heads/main.lock",
        ".git/logs/HEAD",
        ".git",
    ] {
        assert_eq!(after(&mut coalescer, relative), [], "{relative}");
    }
    // A path outside every root is nobody's change; one inside a nested Workspace is both roots'.
    coalescer.record(&Notice::Path(PathBuf::from("/work/other/file")));
    assert_eq!(coalescer.drain(), []);
    assert_eq!(
        after(&mut coalescer, "packages/lib/index.ts"),
        [
            changed(&root, false, true),
            changed(&nested_root, false, true)
        ]
    );
    // Lost events may hide anything, and a burst is one report per root.
    coalescer.record(&Notice::Path(workspace.join("docs/other.md")));
    coalescer.record(&Notice::Overflow(workspace.join("src")));
    coalescer.record(&Notice::Path(workspace.join("src/lib.rs")));
    assert_eq!(coalescer.drain(), [changed(&root, true, true)]);
    // Pending changes of a root no longer watched are dropped; the root still watched keeps its.
    coalescer.record(&Notice::Path(nested.join("a")));
    coalescer.replace(&[watch_root(&root, &[""], &[])]);
    assert_eq!(coalescer.drain(), [changed(&root, false, true)]);
}

/// A backend that follows every root except those named `refused`, and keeps its notice sender.
struct Fake {
    notices: std::sync::mpsc::Sender<Notice>,
    handed: std::sync::mpsc::Sender<std::sync::mpsc::Sender<Notice>>,
}
impl Backend for Fake {
    fn watch(
        &mut self,
        roots: &[WatchRoot],
    ) -> Vec<(String, orchbee_workbench_protocol::WorkspaceFilesWatchState)> {
        let _handed = self.handed.send(self.notices.clone());
        roots
            .iter()
            .filter(|root| root.root.ends_with("refused"))
            .map(|root| {
                (
                    root.root.clone(),
                    orchbee_workbench_protocol::WorkspaceFilesWatchState::Unavailable,
                )
            })
            .collect()
    }
}

#[test]
fn the_watch_lane_reports_each_root_once_then_coalesced_changes() {
    use orchbee_workbench_protocol::WorkspaceFilesWatchState as State;
    let root = Path::new("/work/app").to_string_lossy().into_owned();
    let refused = Path::new("/work/refused").to_string_lossy().into_owned();
    let (mut writer, reader) = pipe();
    let (lines, output) = std::sync::mpsc::channel::<Vec<u8>>();
    let (handed, notices) = std::sync::mpsc::channel();
    let lane = std::thread::spawn(move || {
        serve_watch(
            std::io::BufReader::new(reader),
            LineSink(lines, Vec::new()),
            |sender| Fake {
                notices: sender,
                handed,
            },
        );
    });
    let next = || -> WatchEvent {
        let line = output.recv_timeout(Duration::from_secs(5)).unwrap();
        serde_json::from_slice(&line).unwrap()
    };
    let set = |roots: Vec<WatchRoot>| {
        let mut line = serde_json::to_vec(&WatchSet { roots }).unwrap();
        line.push(b'\n');
        line
    };
    std::io::Write::write_all(
        &mut writer,
        &set(vec![
            watch_root(&root, &[""], &[]),
            watch_root(&refused, &[""], &[]),
        ]),
    )
    .unwrap();
    let mut states = [next(), next()];
    states.sort_by_key(|event| format!("{event:?}"));
    assert_eq!(
        states,
        [
            WatchEvent::State {
                root: root.clone(),
                state: State::Watching
            },
            WatchEvent::State {
                root: refused.clone(),
                state: State::Unavailable
            },
        ]
    );
    let sender = notices.recv_timeout(Duration::from_secs(5)).unwrap();
    let started = Instant::now();
    for name in ["a", "b", "c"] {
        sender
            .send(Notice::Path(Path::new(&root).join(name)))
            .unwrap();
    }
    assert_eq!(next(), changed(&root, true, true));
    assert!(started.elapsed() >= COALESCE);
    // The same set again reports nothing new; a root that stops being watched is simply dropped.
    std::io::Write::write_all(&mut writer, &set(vec![watch_root(&root, &[""], &[])])).unwrap();
    sender
        .send(Notice::Path(Path::new(&root).join("d")))
        .unwrap();
    assert_eq!(next(), changed(&root, true, true));
    drop(writer);
    lane.join().unwrap();
}

/// An in-memory pipe: what the writer writes, the reader reads, until the writer drops.
fn pipe() -> (PipeWriter, PipeReader) {
    let (sender, receiver) = std::sync::mpsc::channel();
    (
        PipeWriter(sender),
        PipeReader {
            receiver,
            buffer: Vec::new(),
        },
    )
}
struct PipeWriter(std::sync::mpsc::Sender<Vec<u8>>);
impl std::io::Write for PipeWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0
            .send(bytes.to_vec())
            .map_err(|_| std::io::ErrorKind::BrokenPipe)?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
struct PipeReader {
    receiver: std::sync::mpsc::Receiver<Vec<u8>>,
    buffer: Vec<u8>,
}
impl std::io::Read for PipeReader {
    fn read(&mut self, into: &mut [u8]) -> std::io::Result<usize> {
        if self.buffer.is_empty() {
            match self.receiver.recv() {
                Ok(bytes) => self.buffer = bytes,
                Err(_) => return Ok(0),
            }
        }
        let count = into.len().min(self.buffer.len());
        into[..count].copy_from_slice(&self.buffer[..count]);
        self.buffer.drain(..count);
        Ok(count)
    }
}
/// Hands each complete line written to it to a channel.
struct LineSink(std::sync::mpsc::Sender<Vec<u8>>, Vec<u8>);
impl std::io::Write for LineSink {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.1.extend_from_slice(bytes);
        while let Some(end) = self.1.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = self.1.drain(..=end).collect();
            let _sent = self.0.send(line);
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn write(root: &str, path: &str, text: &str, expected: Option<&str>) -> WorkspaceFilesOutcome {
    let outcome = answer(
        root,
        &WorkspaceFilesOperation::WriteFile {
            path: path.into(),
            text: text.into(),
            expected: expected.map(str::to_owned),
        },
        &BlobCache::default(),
        deadline(),
    );
    outcome.validate().unwrap();
    outcome
}

fn hash(text: &str) -> String {
    sha2::Sha256::digest(text.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn saves_only_over_the_version_that_was_read() {
    let scratch = Scratch::new("write");
    let root = scratch.root();
    scratch.write("src/main.rs", "one\n");
    let read_hash = version(&read(&root, "src/main.rs", WorkspaceFileSource::Worktree))
        .and_then(|version| version.sha256.clone())
        .unwrap();
    // The written version is what the next read reports, so the next save expects it.
    let WorkspaceFilesOutcome::Written { version: saved } =
        write(&root, "src/main.rs", "two\n", Some(&read_hash))
    else {
        panic!("not written");
    };
    assert_eq!(saved.sha256.as_deref(), Some(hash("two\n").as_str()));
    let reread = read(&root, "src/main.rs", WorkspaceFileSource::Worktree);
    assert_eq!(text(&reread), Some("two\n"));
    assert_eq!(version(&reread), Some(&saved));
    // A save from the older read finds the newer file and leaves it.
    let WorkspaceFilesOutcome::Conflict { version: Some(now) } =
        write(&root, "src/main.rs", "three\n", Some(&read_hash))
    else {
        panic!("not a conflict");
    };
    assert_eq!(now.sha256, saved.sha256);
    assert_eq!(
        std::fs::read_to_string(scratch.0.join("src/main.rs")).unwrap(),
        "two\n"
    );
    // No temporary copy stays behind.
    let names: Vec<_> = std::fs::read_dir(scratch.0.join("src"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(names, ["main.rs"]);
}

#[test]
fn creates_only_a_file_that_is_not_there() {
    let scratch = Scratch::new("create");
    let root = scratch.root();
    scratch.write("notes/a.md", "a\n");
    assert!(matches!(
        write(&root, "notes/新.md", "新\n", None),
        WorkspaceFilesOutcome::Written { .. }
    ));
    assert_eq!(
        std::fs::read_to_string(scratch.0.join("notes/新.md")).unwrap(),
        "新\n"
    );
    // An existing file is never replaced by a create, and a file read before is gone.
    let WorkspaceFilesOutcome::Conflict { version: Some(now) } =
        write(&root, "notes/a.md", "b\n", None)
    else {
        panic!("not a conflict");
    };
    assert_eq!(now.sha256.as_deref(), Some(hash("a\n").as_str()));
    assert_eq!(
        write(&root, "notes/gone.md", "x", Some(&hash("x"))),
        WorkspaceFilesOutcome::Conflict { version: None }
    );
    // A save creates no directory.
    assert_eq!(
        write(&root, "missing/a.md", "x", None),
        WorkspaceFilesOutcome::Failed {
            reason: WorkspaceFilesFailure::NotFound
        }
    );
}

#[cfg(unix)]
#[test]
fn saves_nothing_outside_the_workspace_or_into_a_read_only_file() {
    use std::os::unix::fs::{PermissionsExt as _, symlink};
    let scratch = Scratch::new("write-escape");
    let outside = Scratch::new("write-outside");
    let root = scratch.root();
    outside.write("secret.txt", "no\n");
    symlink(&outside.0, scratch.0.join("escape")).unwrap();
    symlink(outside.0.join("secret.txt"), scratch.0.join("leak.txt")).unwrap();
    symlink(scratch.0.join("nowhere"), scratch.0.join("dangling.txt")).unwrap();
    let denied = WorkspaceFilesOutcome::Failed {
        reason: WorkspaceFilesFailure::Denied,
    };
    let no = hash("no\n");
    assert_eq!(write(&root, "escape/secret.txt", "x", Some(&no)), denied);
    assert_eq!(write(&root, "escape/new.txt", "x", None), denied);
    assert_eq!(write(&root, "leak.txt", "x", Some(&no)), denied);
    assert_eq!(write(&root, "dangling.txt", "x", None), denied);
    assert!(!scratch.0.join("nowhere").exists());
    assert_eq!(
        std::fs::read_to_string(outside.0.join("secret.txt")).unwrap(),
        "no\n"
    );
    // A link inside the Workspace is written through and stays a link.
    scratch.write("real.txt", "a\n");
    symlink(scratch.0.join("real.txt"), scratch.0.join("alias.txt")).unwrap();
    assert!(matches!(
        write(&root, "alias.txt", "b\n", Some(&hash("a\n"))),
        WorkspaceFilesOutcome::Written { .. }
    ));
    assert!(
        std::fs::symlink_metadata(scratch.0.join("alias.txt"))
            .unwrap()
            .is_symlink()
    );
    assert_eq!(
        std::fs::read_to_string(scratch.0.join("real.txt")).unwrap(),
        "b\n"
    );
    scratch.write("locked.txt", "a\n");
    let locked = scratch.0.join("locked.txt");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o444)).unwrap();
    assert_eq!(
        write(&root, "locked.txt", "b\n", Some(&hash("a\n"))),
        denied
    );
    assert_eq!(std::fs::read_to_string(&locked).unwrap(), "a\n");
}

#[cfg(unix)]
#[test]
fn a_save_keeps_the_file_mode_and_its_other_links() {
    use std::os::unix::fs::PermissionsExt as _;
    let scratch = Scratch::new("write-keep");
    let root = scratch.root();
    scratch.write("run.sh", "echo a\n");
    let script = scratch.0.join("run.sh");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o750)).unwrap();
    assert!(matches!(
        write(&root, "run.sh", "echo b\n", Some(&hash("echo a\n"))),
        WorkspaceFilesOutcome::Written { .. }
    ));
    let mode = std::fs::metadata(&script).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o750);
    // A hard-linked file is written in place, so the other name sees the save too.
    scratch.write("a.txt", "a\n");
    std::fs::hard_link(scratch.0.join("a.txt"), scratch.0.join("b.txt")).unwrap();
    assert!(matches!(
        write(&root, "a.txt", "c\n", Some(&hash("a\n"))),
        WorkspaceFilesOutcome::Written { .. }
    ));
    assert_eq!(
        std::fs::read_to_string(scratch.0.join("b.txt")).unwrap(),
        "c\n"
    );
}
