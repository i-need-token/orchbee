//! The local mutations act on temporary repositories as VS Code's Git extension acts: paths reach
//! Git literally, the index takes exactly the content sent, a commit takes what `all` asks and
//! continues a waiting rebase, a locked index is waited out, and a clean never reaches outside the
//! repository.

use super::{
    super::{
        discovery::Found,
        mutations::{
            Commit, Context, Message, clean, commit, discard, stage, stage_content, unstage,
        },
        ports::{Trash, TrashError},
        runner::Limits,
    },
    fixture::Fixture,
};
use orchbee_files_protocol::{CleanMode, CommitAll, ErrorCode};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    thread,
    time::Duration,
};

fn found(fixture: &Fixture, relative: &str) -> Found {
    let toplevel = fixture.path(relative);
    Found {
        git_dir: Path::new(&toplevel).join(".git"),
        toplevel,
        safe_directory: None,
    }
}

fn context<'a>(fixture: &'a Fixture, limits: &'a Limits) -> Context<'a> {
    Context {
        limits,
        output: &fixture.output,
        relay: None,
    }
}

fn staged(fixture: &Fixture) -> String {
    fixture.git("repo", &["diff", "--cached", "--name-only"])
}

#[test]
fn staging_takes_paths_literally_and_unstaging_works_before_and_after_the_first_commit() {
    let fixture = Fixture::new();
    fixture.repository("repo");
    let repository = found(&fixture, "repo");
    let limits = fixture.limits();
    let context = context(&fixture, &limits);
    for name in ["a", "b", "[ab]"] {
        fixture.write(&format!("repo/{name}"), name);
    }

    stage(&repository, &[fixture.path("repo/[ab]")], &context).unwrap();
    assert_eq!(staged(&fixture), "[ab]\n");
    unstage(&repository, &[fixture.path("repo/[ab]")], &context).unwrap();
    assert_eq!(staged(&fixture), "");

    let all = ["a", "b"].map(|name| fixture.path(&format!("repo/{name}")));
    stage(&repository, &all, &context).unwrap();
    fixture.git("repo", &["commit", "-q", "-m", "seed"]);
    fixture.write("repo/a", "changed");
    stage(&repository, &all[..1], &context).unwrap();
    assert_eq!(staged(&fixture), "a\n");
    unstage(&repository, &all[..1], &context).unwrap();
    assert_eq!(staged(&fixture), "");

    discard(&repository, &all[..1], &[], &context).unwrap();
    assert_eq!(fs::read_to_string(&all[0]).unwrap(), "a");
    let outside = fixture.path("elsewhere");
    assert_eq!(
        stage(&repository, &[outside], &context).unwrap_err().code,
        ErrorCode::OutsideScope
    );
}

#[test]
fn content_staging_keeps_the_recorded_mode_and_leaves_the_working_tree() {
    let fixture = Fixture::new();
    fixture.repository("repo");
    let repository = found(&fixture, "repo");
    let limits = fixture.limits();
    let context = context(&fixture, &limits);
    fixture.write("repo/tool", "old");
    fixture.git("repo", &["add", "tool"]);
    fixture.git("repo", &["update-index", "--chmod=+x", "tool"]);
    fixture.git("repo", &["commit", "-q", "-m", "seed"]);
    fixture.write("repo/tool", "working tree");

    stage_content(
        &repository,
        &fixture.path("repo/tool"),
        b"selected".to_vec(),
        &context,
    )
    .unwrap();
    stage_content(
        &repository,
        &fixture.path("repo/new file"),
        b"new".to_vec(),
        &context,
    )
    .unwrap();

    let listed = fixture.git("repo", &["ls-files", "--stage"]);
    assert!(
        listed
            .lines()
            .any(|line| line.starts_with("100755 ") && line.ends_with("\ttool"))
    );
    assert!(
        listed
            .lines()
            .any(|line| line.starts_with("100644 ") && line.ends_with("\tnew file"))
    );
    assert_eq!(fixture.git("repo", &["show", ":tool"]), "selected");
    assert_eq!(fixture.git("repo", &["show", ":new file"]), "new");
    assert_eq!(
        fs::read_to_string(fixture.path("repo/tool")).unwrap(),
        "working tree"
    );
}

#[test]
fn commit_stages_what_all_asks_amends_and_continues_a_waiting_rebase() {
    let fixture = Fixture::new();
    fixture.repository("repo");
    let repository = found(&fixture, "repo");
    let limits = fixture.limits();
    let context = context(&fixture, &limits);
    let text = |message: &str, all| Commit {
        message: Message::Text(message.to_owned()),
        amend: false,
        signoff: false,
        all,
        no_verify: false,
    };
    fixture.write("repo/tracked", "one");
    let first = commit(&repository, &text("first", CommitAll::All), &context).unwrap();
    assert_eq!(fixture.git("repo", &["rev-parse", "HEAD"]).trim(), first);

    fixture.write("repo/tracked", "two");
    fixture.write("repo/untracked", "new");
    commit(&repository, &text("second", CommitAll::Tracked), &context).unwrap();
    assert_eq!(
        fixture.git("repo", &["status", "--porcelain"]),
        "?? untracked\n"
    );
    let amended = Commit {
        amend: true,
        ..text("", CommitAll::None)
    };
    commit(&repository, &amended, &context).unwrap();
    assert_eq!(
        fixture.git("repo", &["log", "-1", "--format=%s"]),
        "second\n"
    );
    assert_eq!(fixture.git("repo", &["rev-list", "--count", "HEAD"]), "2\n");

    fixture.git("repo", &["checkout", "-q", "-b", "side", "HEAD~1"]);
    fixture.write("repo/tracked", "side");
    fixture.git("repo", &["commit", "-q", "-am", "side"]);
    let rebased = fixture.run("repo", &["rebase", "main"]);
    assert!(!rebased.status.unwrap().success());
    fixture.write("repo/tracked", "resolved");
    commit(&repository, &text("ignored", CommitAll::Tracked), &context).unwrap();
    assert!(!repository.git_dir.join("rebase-merge").exists());
    assert_eq!(fixture.git("repo", &["log", "-1", "--format=%s"]), "side\n");
    assert_eq!(fixture.git("repo", &["show", "HEAD:tracked"]), "resolved");
}

#[test]
fn a_locked_index_is_retried_until_the_lock_goes() {
    let fixture = Fixture::new();
    fixture.repository("repo");
    let repository = found(&fixture, "repo");
    let limits = fixture.limits();
    let context = context(&fixture, &limits);
    fixture.write("repo/file", "content");
    let lock = repository.git_dir.join("index.lock");
    fs::write(&lock, "").unwrap();
    thread::scope(|scope| {
        scope.spawn(|| {
            thread::sleep(Duration::from_millis(400));
            fs::remove_file(&lock).unwrap();
        });
        stage(&repository, &[fixture.path("repo/file")], &context).unwrap();
    });
    assert_eq!(staged(&fixture), "file\n");
}

/// A trash that records what it took.
struct Recorded(Mutex<Vec<PathBuf>>);

impl Trash for Recorded {
    fn trash(&self, path: &Path) -> Result<(), TrashError> {
        self.0.lock().unwrap().push(path.to_owned());
        Ok(())
    }
}

#[test]
fn clean_trashes_inside_the_repository_or_removes_permanently() {
    let fixture = Fixture::new();
    fixture.repository("repo");
    let repository = found(&fixture, "repo");
    let limits = fixture.limits();
    let context = context(&fixture, &limits);
    fixture.write("repo/kept", "");
    fixture.write("repo/gone", "");
    let trash = Recorded(Mutex::new(Vec::new()));

    let paths = [
        fixture.path("repo/kept"),
        fixture.path("repo/missing"),
        fixture.path("other"),
    ];
    let cleaned = clean(&repository, &paths, CleanMode::Trash, &trash, &context);
    let codes = cleaned
        .iter()
        .map(|item| item.error.as_ref().map(|error| error.code))
        .collect::<Vec<_>>();
    assert_eq!(
        codes,
        [
            None,
            Some(ErrorCode::NotFound),
            Some(ErrorCode::OutsideScope)
        ]
    );
    assert_eq!(*trash.0.lock().unwrap(), [PathBuf::from(&paths[0])]);

    let cleaned = clean(
        &repository,
        &[fixture.path("repo/gone")],
        CleanMode::Permanent,
        &trash,
        &context,
    );
    assert!(cleaned[0].error.is_none());
    assert!(!Path::new(&fixture.path("repo/gone")).exists());
}
