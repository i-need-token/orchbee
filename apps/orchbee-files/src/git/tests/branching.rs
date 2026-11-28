//! The branching writes act on temporary repositories, and on a local bare remote, as VS Code's
//! Git extension acts: checkout never takes a file for a revision, branch writes report Git's own
//! refusals, conflicts read as VS Code reads them and abort cleanly, an emptied cherry-pick is
//! aborted, an undo or a stash write refuses what the Client no longer sees, and remote deletes
//! remove exactly the named branch or tag.

use super::{
    super::{
        branching::{Abort, Branching, execute},
        errors::{GitError, GitErrorCode},
        mutations::Context,
        runner::Limits,
    },
    fixture::Fixture,
};
use orchbee_files_protocol::ErrorCode;
use std::{fs, path::Path};

fn run(fixture: &Fixture, relative: &str, branching: Branching) -> Result<(), GitError> {
    let limits: Limits = fixture.limits();
    let context = Context {
        limits: &limits,
        output: &fixture.output,
        relay: None,
    };
    execute(&fixture.found(relative), &branching, &context)
}

fn code(result: Result<(), GitError>) -> (ErrorCode, Option<GitErrorCode>) {
    let error = result.unwrap_err();
    (error.code, error.git_error_code)
}

fn commit(fixture: &Fixture, file: &str, content: &str, message: &str) -> String {
    fixture.write(&format!("repo/{file}"), content);
    fixture.git("repo", &["add", file]);
    fixture.git("repo", &["commit", "-q", "-m", message]);
    fixture.commit_of("repo", "HEAD")
}

fn current(fixture: &Fixture) -> String {
    fixture
        .git("repo", &["branch", "--show-current"])
        .trim()
        .to_owned()
}

fn git_dir_has(fixture: &Fixture, name: &str) -> bool {
    Path::new(&fixture.path("repo/.git")).join(name).exists()
}

#[test]
fn checkout_and_branch_writes_move_head_and_report_refusals() {
    let fixture = Fixture::new();
    fixture.repository("repo");
    let first = commit(&fixture, "a.txt", "base\n", "first");
    let checkout = |revision: &str, detached: bool| Branching::Checkout {
        revision: revision.to_owned(),
        detached,
        track: false,
    };

    let topic = Branching::BranchCreate {
        name: "topic".to_owned(),
        start: None,
        checkout: false,
    };
    run(&fixture, "repo", topic.clone()).unwrap();
    assert_eq!(current(&fixture), "main");
    assert_eq!(
        code(run(&fixture, "repo", topic)),
        (
            ErrorCode::GitFailed,
            Some(GitErrorCode::BranchAlreadyExists)
        )
    );
    let work = Branching::BranchCreate {
        name: "work".to_owned(),
        start: Some(first.clone()),
        checkout: true,
    };
    run(&fixture, "repo", work).unwrap();
    assert_eq!(current(&fixture), "work");
    let renamed = Branching::BranchRename {
        name: "renamed".to_owned(),
    };
    run(&fixture, "repo", renamed).unwrap();
    assert_eq!(current(&fixture), "renamed");

    // A file's name is no revision: the checkout fails and the change stays.
    fixture.write("repo/a.txt", "changed\n");
    assert_eq!(
        code(run(&fixture, "repo", checkout("a.txt", false))).0,
        ErrorCode::GitFailed
    );
    assert_eq!(
        fs::read_to_string(fixture.path("repo/a.txt")).unwrap(),
        "changed\n"
    );
    fixture.git("repo", &["checkout", "-q", "--", "a.txt"]);

    run(&fixture, "repo", checkout("topic", false)).unwrap();
    commit(&fixture, "a.txt", "topic\n", "on topic");
    run(&fixture, "repo", checkout("main", false)).unwrap();
    fixture.write("repo/a.txt", "dirty\n");
    assert_eq!(
        code(run(&fixture, "repo", checkout("topic", false))),
        (ErrorCode::GitFailed, Some(GitErrorCode::DirtyWorkTree))
    );
    fixture.git("repo", &["checkout", "-q", "--", "a.txt"]);
    run(&fixture, "repo", checkout(&first, true)).unwrap();
    assert_eq!(current(&fixture), "");
    run(&fixture, "repo", checkout("main", false)).unwrap();

    let delete = |force| Branching::BranchDelete {
        name: "topic".to_owned(),
        force,
    };
    assert_eq!(
        code(run(&fixture, "repo", delete(false))),
        (
            ErrorCode::GitFailed,
            Some(GitErrorCode::BranchNotFullyMerged)
        )
    );
    run(&fixture, "repo", delete(true)).unwrap();
    assert!(
        fixture
            .run("repo", &["rev-parse", "--verify", "-q", "topic"])
            .status
            .is_some_and(|status| !status.success())
    );
}

#[test]
fn conflicts_read_as_vs_code_reads_them_and_each_abort_ends_its_operation() {
    let fixture = Fixture::new();
    fixture.repository("repo");
    commit(&fixture, "a.txt", "base\n", "base");
    fixture.git("repo", &["checkout", "-q", "-b", "side"]);
    let side = commit(&fixture, "a.txt", "side\n", "side");
    fixture.git("repo", &["checkout", "-q", "main"]);
    commit(&fixture, "a.txt", "main\n", "main");

    let merge = Branching::Merge {
        revision: "side".to_owned(),
    };
    assert_eq!(
        code(run(&fixture, "repo", merge)),
        (ErrorCode::GitFailed, Some(GitErrorCode::Conflict))
    );
    assert!(git_dir_has(&fixture, "MERGE_HEAD"));
    run(&fixture, "repo", Branching::Abort(Abort::Merge)).unwrap();
    assert!(!git_dir_has(&fixture, "MERGE_HEAD"));

    let pick = |revision: &str| Branching::CherryPick {
        revision: revision.to_owned(),
    };
    assert_eq!(
        code(run(&fixture, "repo", pick(&side))),
        (ErrorCode::GitFailed, Some(GitErrorCode::CherryPickConflict))
    );
    assert!(git_dir_has(&fixture, "CHERRY_PICK_HEAD"));
    run(&fixture, "repo", Branching::Abort(Abort::CherryPick)).unwrap();
    assert!(!git_dir_has(&fixture, "CHERRY_PICK_HEAD"));

    fixture.git("repo", &["checkout", "-q", "side"]);
    let rebase = Branching::Rebase {
        revision: "main".to_owned(),
    };
    assert_eq!(
        code(run(&fixture, "repo", rebase)),
        (ErrorCode::GitFailed, Some(GitErrorCode::Conflict))
    );
    assert!(git_dir_has(&fixture, "rebase-merge") || git_dir_has(&fixture, "rebase-apply"));
    run(&fixture, "repo", Branching::Abort(Abort::Rebase)).unwrap();
    assert!(!git_dir_has(&fixture, "rebase-merge") && !git_dir_has(&fixture, "rebase-apply"));
    assert_eq!(fixture.commit_of("repo", "HEAD"), side);

    // A pick whose change the branch already has empties, and is aborted.
    fixture.git("repo", &["checkout", "-q", "-b", "again", "main"]);
    let mine = commit(&fixture, "b.txt", "b\n", "b");
    fixture.git("repo", &["checkout", "-q", "-b", "copy", "main"]);
    commit(&fixture, "b.txt", "b\n", "b again");
    assert_eq!(
        code(run(&fixture, "repo", pick(&mine))),
        (ErrorCode::GitFailed, Some(GitErrorCode::CherryPickEmpty))
    );
    assert!(!git_dir_has(&fixture, "CHERRY_PICK_HEAD"));
    let clean = Branching::Merge {
        revision: "again".to_owned(),
    };
    run(&fixture, "repo", clean).unwrap();
}

#[test]
fn undo_and_stash_writes_refuse_what_the_client_no_longer_sees() {
    let fixture = Fixture::new();
    fixture.repository("repo");
    let root = commit(&fixture, "a.txt", "one\n", "root");
    let second = commit(&fixture, "a.txt", "two\n", "second");

    let undo = |commit: &str| Branching::UndoCommit {
        commit: commit.to_owned(),
    };
    assert_eq!(
        code(run(&fixture, "repo", undo(&root))).0,
        ErrorCode::StaleRevision
    );
    assert_eq!(fixture.commit_of("repo", "HEAD"), second);
    run(&fixture, "repo", undo(&second)).unwrap();
    assert_eq!(fixture.commit_of("repo", "HEAD"), root);
    assert_eq!(
        fixture.git("repo", &["diff", "--cached", "--name-only"]),
        "a.txt\n"
    );
    fixture.git("repo", &["commit", "-q", "-m", "second"]);

    let push = |untracked, message: Option<&str>| Branching::StashPush {
        untracked,
        staged: false,
        message: message.map(str::to_owned),
    };
    assert_eq!(
        code(run(&fixture, "repo", push(false, None))),
        (ErrorCode::GitFailed, Some(GitErrorCode::NoLocalChanges))
    );
    fixture.write("repo/a.txt", "stashed\n");
    run(&fixture, "repo", push(false, Some("kept"))).unwrap();
    let kept = fixture.commit_of("repo", "stash@{0}");
    fixture.write("repo/new.txt", "untracked\n");
    run(&fixture, "repo", push(true, None)).unwrap();
    assert!(!Path::new(&fixture.path("repo/new.txt")).exists());
    let untracked = fixture.commit_of("repo", "stash@{0}");

    let apply = |index, commit: &str, pop| Branching::StashApply {
        index,
        commit: commit.to_owned(),
        pop,
        staged: false,
    };
    // `kept` moved to stash@{1}; the Client that still sees it at 0 is refused.
    assert_eq!(
        code(run(&fixture, "repo", apply(0, &kept, false))).0,
        ErrorCode::StaleRevision
    );
    assert_eq!(
        code(run(&fixture, "repo", apply(5, &kept, false))),
        (ErrorCode::GitFailed, Some(GitErrorCode::NoStashFound))
    );
    run(&fixture, "repo", apply(0, &untracked, true)).unwrap();
    assert!(Path::new(&fixture.path("repo/new.txt")).exists());
    assert_eq!(fixture.commit_of("repo", "stash@{0}"), kept);

    // A pop that conflicts keeps its entry.
    fixture.write("repo/a.txt", "other\n");
    fixture.git("repo", &["commit", "-q", "-a", "-m", "other"]);
    assert_eq!(
        code(run(&fixture, "repo", apply(0, &kept, true))),
        (ErrorCode::GitFailed, Some(GitErrorCode::StashConflict))
    );
    assert_eq!(fixture.commit_of("repo", "stash@{0}"), kept);
    fixture.git("repo", &["reset", "-q", "--hard"]);
    let drop = Branching::StashDrop {
        index: 0,
        commit: kept,
    };
    run(&fixture, "repo", drop).unwrap();
    fixture.write("repo/a.txt", "again\n");
    run(&fixture, "repo", push(false, None)).unwrap();
    run(&fixture, "repo", Branching::StashClear).unwrap();
    assert_eq!(fixture.git("repo", &["stash", "list"]), "");

    // Undoing the first commit leaves the branch unborn and nothing staged.
    fixture.repository("first");
    fixture.write("first/f.txt", "f\n");
    fixture.git("first", &["add", "f.txt"]);
    fixture.git("first", &["commit", "-q", "-m", "only"]);
    let only = fixture.commit_of("first", "HEAD");
    run(&fixture, "first", undo(&only)).unwrap();
    assert!(
        fixture
            .run("first", &["rev-parse", "--verify", "-q", "HEAD"])
            .status
            .is_some_and(|status| !status.success())
    );
    assert_eq!(fixture.git("first", &["ls-files"]), "");
    assert!(Path::new(&fixture.path("first/f.txt")).exists());
}

#[test]
fn tags_remotes_and_remote_deletes_reach_a_local_bare_remote() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.path("remote.git")).unwrap();
    fixture.git("remote.git", &["init", "-q", "--bare", "-b", "main"]);
    let bare = fixture.path("remote.git");
    fixture.repository("repo");
    commit(&fixture, "a.txt", "a\n", "first");

    let tag = |name: &str, message: Option<&str>| Branching::TagCreate {
        name: name.to_owned(),
        message: message.map(str::to_owned),
        target: None,
    };
    run(&fixture, "repo", tag("light", None)).unwrap();
    run(&fixture, "repo", tag("annotated", Some("notes"))).unwrap();
    assert_eq!(
        fixture.git("repo", &["cat-file", "-t", "light"]),
        "commit\n"
    );
    assert_eq!(
        fixture.git("repo", &["cat-file", "-t", "annotated"]),
        "tag\n"
    );
    let delete = Branching::TagDelete {
        name: "light".to_owned(),
    };
    run(&fixture, "repo", delete).unwrap();
    assert_eq!(fixture.git("repo", &["tag"]), "annotated\n");

    let add = Branching::RemoteAdd {
        name: "origin".to_owned(),
        url: bare.clone(),
    };
    run(&fixture, "repo", add).unwrap();
    fixture.git("repo", &["branch", "feature"]);
    fixture.git(
        "repo",
        &["push", "-q", "origin", "main", "feature", "annotated"],
    );
    // A tag and a branch of one name on the remote: each delete removes only its own kind.
    fixture.git("repo", &["push", "-q", "origin", "main:refs/tags/feature"]);
    let upstream = Branching::BranchUpstream {
        name: "main".to_owned(),
        upstream: "origin/main".to_owned(),
    };
    run(&fixture, "repo", upstream).unwrap();
    assert_eq!(
        fixture.git("repo", &["rev-parse", "--abbrev-ref", "main@{upstream}"]),
        "origin/main\n"
    );

    let remote_branch = Branching::BranchDeleteRemote {
        remote: "origin".to_owned(),
        name: "feature".to_owned(),
    };
    run(&fixture, "repo", remote_branch).unwrap();
    let remote_tag = Branching::TagDeleteRemote {
        remote: "origin".to_owned(),
        name: "annotated".to_owned(),
    };
    run(&fixture, "repo", remote_tag).unwrap();
    assert_eq!(
        fixture.git("remote.git", &["for-each-ref", "--format=%(refname)"]),
        "refs/heads/main\nrefs/tags/feature\n"
    );

    fixture.git(
        "repo",
        &[
            "push",
            "-q",
            "origin",
            "refs/heads/feature:refs/heads/feature",
        ],
    );
    fixture.git("repo", &["branch", "-q", "-D", "feature"]);
    let track = Branching::Checkout {
        revision: "origin/feature".to_owned(),
        detached: false,
        track: true,
    };
    run(&fixture, "repo", track).unwrap();
    assert_eq!(current(&fixture), "feature");
    assert_eq!(
        fixture.git("repo", &["rev-parse", "--abbrev-ref", "feature@{upstream}"]),
        "origin/feature\n"
    );

    let remove = Branching::RemoteRemove {
        name: "origin".to_owned(),
    };
    run(&fixture, "repo", remove).unwrap();
    assert_eq!(fixture.git("repo", &["remote"]), "");
    let invalid = Branching::RemoteAdd {
        name: "origin".to_owned(),
        url: "--upload-pack=x".to_owned(),
    };
    assert_eq!(
        code(run(&fixture, "repo", invalid)).0,
        ErrorCode::InvalidRequest
    );
}
