use super::{Answer, Request, answer, serve};
use orchbee_workbench_protocol::{
    WorkspaceEntryKind, WorkspaceFileContent, WorkspaceFileSource, WorkspaceFilesFailure,
    WorkspaceFilesOperation, WorkspaceFilesOutcome, WorkspaceGitStatusCode as Code,
};
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
        deadline(),
    )
}

fn read(root: &str, path: &str, source: WorkspaceFileSource) -> WorkspaceFilesOutcome {
    answer(
        root,
        &WorkspaceFilesOperation::ReadFile {
            path: path.into(),
            source,
        },
        deadline(),
    )
}

fn text(outcome: &WorkspaceFilesOutcome) -> Option<&str> {
    match outcome {
        WorkspaceFilesOutcome::File {
            content: WorkspaceFileContent::Text { text },
            ..
        } => Some(text),
        _ => None,
    }
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
    assert_eq!(
        read(&root, "build/out.bin", WorkspaceFileSource::Worktree),
        WorkspaceFilesOutcome::File {
            byte_length: 3,
            content: WorkspaceFileContent::Binary {}
        }
    );
    scratch.write("big.txt", vec![b'a'; 2 * 1024 * 1024 + 1]);
    assert_eq!(
        read(&root, "big.txt", WorkspaceFileSource::Worktree),
        WorkspaceFilesOutcome::File {
            byte_length: 2 * 1024 * 1024 + 1,
            content: WorkspaceFileContent::TooLarge {}
        }
    );
    assert_eq!(
        read(&root, "src", WorkspaceFileSource::Worktree),
        WorkspaceFilesOutcome::Failed {
            reason: WorkspaceFilesFailure::NotFound
        }
    );
}

#[test]
fn text_whose_escapes_pass_the_result_cap_is_too_large() {
    let scratch = Scratch::new("escapes");
    // Each control byte encodes as six bytes of JSON.
    scratch.write("controls.txt", vec![0x01_u8; 2 * 1024 * 1024]);
    assert_eq!(
        read(
            &scratch.root(),
            "controls.txt",
            WorkspaceFileSource::Worktree
        ),
        WorkspaceFilesOutcome::File {
            byte_length: 2 * 1024 * 1024,
            content: WorkspaceFileContent::TooLarge {}
        }
    );
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
