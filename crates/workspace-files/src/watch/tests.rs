//! The platform watcher and Git's ignore rules against real directories: the platform's events and
//! Git's own verdicts are the references, since neither can be judged from the code alone.
use super::platform::Platform;
use super::repository::Repository;
use super::{Backend, Notice, WatchRoot};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

/// A temporary repository removed on drop.
struct Scratch(PathBuf);
impl Scratch {
    fn repository() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "orchbee-workspace-files-watch-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _removed = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        let scratch = Self(std::fs::canonicalize(path).unwrap());
        scratch.git(&["init", "-q"]);
        scratch
    }
    fn write(&self, relative: &str, bytes: &str) {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    fn git(&self, args: &[&str]) {
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
            .current_dir(&self.0)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }
    /// Whether Git itself ignores `relative`, the reference the lane's rules must match.
    fn git_ignores(&self, relative: &str) -> bool {
        Command::new("git")
            .args(["check-ignore", "-q", "--", relative])
            .current_dir(&self.0)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .status()
            .unwrap()
            .success()
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _removed = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn ignore_rules_agree_with_git_and_git_directories_count_only_state() {
    let scratch = Scratch::repository();
    scratch.write(
        ".gitignore",
        "build/\n*.log\n!keep.log\nvendor/\n/top-only\n",
    );
    scratch.write("sub/.gitignore", "*.tmp\n!wanted.tmp\n");
    scratch.write("src/main.rs", "fn main() {}\n");
    scratch.write("vendor/tracked.txt", "kept\n");
    scratch.git(&["add", "-A"]);
    scratch.git(&["add", "-f", "vendor/tracked.txt"]);
    scratch.git(&["commit", "-q", "-m", "first"]);
    // Git reads whether a path is a directory from the disk, so every path asked about is there.
    for file in [
        "build/out.bin",
        "build/deep/x.o",
        "debug.log",
        "keep.log",
        "src/new.log",
        "vendor/untracked.txt",
        "sub/a.tmp",
        "sub/wanted.tmp",
        "a.tmp",
        "top-only",
        "sub/top-only",
    ] {
        scratch.write(file, "");
    }
    let mut repository = Repository::discover(&scratch.0).unwrap();
    let cases = [
        ("build", true),
        ("build/out.bin", false),
        ("build/deep/x.o", false),
        ("debug.log", false),
        ("keep.log", false),
        ("src/main.rs", false),
        ("src/new.log", false),
        // A directory holding a tracked file is not ignored, though rules match it.
        ("vendor", true),
        ("vendor/tracked.txt", false),
        ("vendor/untracked.txt", false),
        ("sub/a.tmp", false),
        ("sub/wanted.tmp", false),
        ("a.tmp", false),
        ("top-only", false),
        ("sub/top-only", false),
    ];
    for (relative, is_dir) in cases {
        assert_eq!(
            repository.ignored(&scratch.0.join(relative), is_dir),
            scratch.git_ignores(relative),
            "{relative}"
        );
    }
    // Changed rules are read again once the lane is told they changed.
    scratch.write("sub/.gitignore", "");
    repository.rules_changed();
    assert!(!repository.ignored(&scratch.0.join("sub/a.tmp"), false));
    assert!(!scratch.git_ignores("sub/a.tmp"));
    // A file staged under an ignored directory is tracked from then on.
    scratch.write("vendor/staged.txt", "now\n");
    scratch.git(&["add", "-f", "vendor/staged.txt"]);
    repository.index_changed();
    assert!(!repository.ignored(&scratch.0.join("vendor/staged.txt"), false));

    let git = scratch.0.join(".git");
    for state in ["index", "HEAD", "packed-refs", "refs/heads/main", "refs"] {
        assert_eq!(repository.git_path(&git.join(state)), Some(true), "{state}");
    }
    for work in [
        "objects/ab/cdef",
        "index.lock",
        "refs/heads/main.lock",
        "logs/HEAD",
        "",
    ] {
        assert_eq!(repository.git_path(&git.join(work)), Some(false), "{work}");
    }
    assert_eq!(repository.git_path(&scratch.0.join("src/main.rs")), None);
    assert!(repository.is_rule_file(&scratch.0.join("sub/.gitignore")));
    assert!(repository.is_rule_file(&git.join("info/exclude")));
    assert!(!repository.is_rule_file(&scratch.0.join("src/main.rs")));
}

/// Waits for a notice `wanted` accepts, skipping others.
fn expect(notices: &Receiver<Notice>, what: &str, wanted: impl Fn(&Notice) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match notices.recv_timeout(left) {
            Ok(notice) if wanted(&notice) => return,
            Ok(_) => {}
            Err(_) => break,
        }
    }
    panic!("no notice for {what}");
}

fn changed(path: PathBuf, repository: bool) -> impl Fn(&Notice) -> bool {
    move |notice| {
        *notice
            == Notice::Path {
                path: path.clone(),
                repository,
            }
    }
}

/// Waits until `check` holds.
fn eventually(what: &str, check: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !check() {
        assert!(Instant::now() < deadline, "never: {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn the_platform_watcher_reports_changes_by_what_git_follows() {
    let scratch = Scratch::repository();
    scratch.write(".gitignore", "build/\n*.log\n");
    scratch.write("src/main.rs", "fn main() {}\n");
    scratch.write("build/deep/out.o", "");
    scratch.git(&["add", "-A"]);
    scratch.git(&["commit", "-q", "-m", "first"]);
    let root = &scratch.0;
    let (sender, notices) = std::sync::mpsc::channel();
    let mut platform = Platform::start(sender);
    let watch_root = WatchRoot {
        root: root.to_string_lossy().into_owned(),
        directories: vec![String::new()],
        files: Vec::new(),
    };
    assert_eq!(platform.watch(std::slice::from_ref(&watch_root)), []);
    let watched = |path: &Path| platform.watched().iter().any(|watched| watched == path);
    if !super::platform::WATCHES_TREES {
        // Directories are watched one by one, leaving out what Git ignores and Git's own objects.
        assert!(watched(&root.join("src")));
        assert!(!watched(&root.join("build")));
        assert!(watched(&root.join(".git/refs/heads")));
        assert!(!watched(&root.join(".git/objects")));
    }

    scratch.write("src/main.rs", "fn main() { }\n");
    expect(
        &notices,
        "a tracked file",
        changed(root.join("src/main.rs"), true),
    );
    scratch.write("debug.log", "noise\n");
    expect(
        &notices,
        "an ignored file",
        changed(root.join("debug.log"), false),
    );

    // A directory that appears is followed, what lies in it included.
    std::fs::create_dir(root.join("src/new")).unwrap();
    if !super::platform::WATCHES_TREES {
        eventually("the new directory is watched", || {
            watched(&root.join("src/new"))
        });
    }
    scratch.write("src/new/lib.rs", "\n");
    expect(
        &notices,
        "a file in a new directory",
        changed(root.join("src/new/lib.rs"), true),
    );

    // Staging changes the index, which is the repository's own state.
    scratch.git(&["add", "src/main.rs"]);
    expect(&notices, "the index", |notice| {
        *notice == Notice::Repository(root.clone())
    });

    // A root no longer named holds no watches.
    assert_eq!(platform.watch(&[]), []);
    assert_eq!(platform.watched(), Vec::<PathBuf>::new());
}
