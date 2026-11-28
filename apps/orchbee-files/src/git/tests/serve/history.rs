//! The `files-history` Git operations as the dispatch reaches them: a log page's continuation
//! starts the next page where it stopped, with times in milliseconds; a blame describes each
//! commit it names once, however many ranges name it; a branching write answers under its
//! identity; and a clone fills a new folder of the chosen one, named after its URL.

use super::{super::fixture::Fixture, Harness, connection};
use crate::scope::path::NativePath;
use orchbee_files_protocol::{
    Empty, FilePath, FilesReply, GitBlame, GitBranchCreate, GitClone, GitLog, GitName, LogLimit,
    MutationReply, MutationState, Operation, RequestId, RequestOutcome, SessionId, WorkspaceId,
    WorkspaceScope,
};
use std::fs;

#[test]
fn history_pages_continue_where_they_stopped_and_a_branching_write_answers() {
    let fixture = Fixture::new();
    fixture.repository("repo");
    let mut commits = Vec::new();
    for (minutes, text) in [(0, "a\nb\nc\n"), (1, "a\nB\nc\n"), (2, "a\nB\nc\nd\n")] {
        fixture.write("repo/a.txt", text);
        fixture.git("repo", &["add", "a.txt"]);
        let message = format!("commit {minutes}");
        fixture.git_at("repo", &["commit", "-q", "-m", &message], minutes);
        commits.push(fixture.commit_of("repo", "HEAD"));
    }

    let harness = Harness::new();
    let workspace = WorkspaceId::from_bytes([4; 16]).unwrap();
    harness.state(workspace, WorkspaceScope::Root(fixture.path("repo")));
    harness.service().subscribe(harness.open(1), workspace);
    let repository = harness.summary(1, |summary| {
        let shown = summary.repositories.as_slice().first()?;
        Some(shown.repository.clone())
    });

    // One commit a page, newest first, until a page without a continuation.
    let (mut cursor, mut read) = (None, Vec::new());
    for id in 2..=4 {
        let log = GitLog {
            workspace_id: workspace,
            repository: repository.clone(),
            revisions: None,
            all: false,
            path: None,
            follow: false,
            author: None,
            grep: None,
            statistics: false,
            limit: LogLimit::new(1).unwrap(),
            cursor,
        };
        harness.service().log(harness.open(id), &log);
        let FilesReply::GitLog(page) = harness.reply(id, Operation::GitLog) else {
            panic!("expected a page of the log");
        };
        let [commit] = page.commits.as_slice() else {
            panic!("expected one commit a page");
        };
        read.push((
            commit.commit.as_str().to_owned(),
            commit.author_time_ms.get(),
        ));
        cursor = page.cursor;
    }
    assert_eq!(cursor, None, "the last page ends the log");
    assert_eq!(
        read,
        [
            (commits[2].clone(), 1_700_000_120_000),
            (commits[1].clone(), 1_700_000_060_000),
            (commits[0].clone(), 1_700_000_000_000),
        ]
    );

    let blame = GitBlame {
        workspace_id: workspace,
        repository: repository.clone(),
        path: FilePath::new(fixture.path("repo/a.txt")).unwrap(),
        commit: None,
        ignore_whitespace: false,
        cursor: None,
    };
    harness.service().blame(harness.open(5), &blame);
    let FilesReply::GitBlame(blamed) = harness.reply(5, Operation::GitBlame) else {
        panic!("expected the blame");
    };
    let mut ranges = blamed
        .ranges
        .as_slice()
        .iter()
        .map(|range| (range.start.get(), range.end.get(), range.commit.as_str()))
        .collect::<Vec<_>>();
    ranges.sort_unstable();
    let [first, second, third] = [&commits[0], &commits[1], &commits[2]].map(String::as_str);
    assert_eq!(
        ranges,
        [(1, 1, first), (2, 2, second), (3, 3, first), (4, 4, third)]
    );
    let mut described = blamed
        .commits
        .as_slice()
        .iter()
        .map(|commit| (commit.commit.as_str(), commit.author_time_ms.get()))
        .collect::<Vec<_>>();
    described.sort_unstable();
    let mut expected = vec![
        (first, 1_700_000_000_000),
        (second, 1_700_000_060_000),
        (third, 1_700_000_120_000),
    ];
    expected.sort_unstable();
    assert_eq!(described, expected);
    assert_eq!(blamed.cursor, None);

    let request_id = RequestId::from_bytes([6; 16]).unwrap();
    let view = harness
        .scopes
        .view(workspace, connection(), Operation::GitBranchCreate)
        .unwrap();
    let create = GitBranchCreate {
        workspace_id: workspace,
        request_id,
        repository,
        name: GitName::new("topic").unwrap(),
        start: Some(GitName::new(first).unwrap()),
        checkout: false,
    };
    harness
        .service()
        .branch_create(harness.open(6), view, &create);
    assert_eq!(
        harness.reply(6, Operation::GitBranchCreate),
        FilesReply::Empty
    );
    assert_eq!(fixture.commit_of("repo", "topic"), first);
    let outcome = RequestOutcome {
        workspace_id: workspace,
        request_id,
    };
    harness.service().outcome(harness.open(7), &outcome);
    let FilesReply::RequestOutcome(outcome) = harness.reply(7, Operation::RequestOutcome) else {
        panic!("expected an outcome");
    };
    assert_eq!(
        outcome.state,
        MutationState::Completed(MutationReply::Empty(Empty {}))
    );
}

#[test]
fn a_clone_fills_a_new_folder_named_after_its_url() {
    let fixture = Fixture::new();
    fixture.repository("source");
    fixture.write("source/a.txt", "a\n");
    fixture.git("source", &["add", "a.txt"]);
    fixture.git("source", &["commit", "-q", "-m", "first"]);
    fixture.git("", &["clone", "-q", "--bare", "source", "remote.git"]);
    fs::create_dir(fixture.path("chosen")).unwrap();

    let harness = Harness::new();
    let workspace = WorkspaceId::from_bytes([7; 16]).unwrap();
    harness.state(workspace, WorkspaceScope::Root(fixture.path("source")));
    let parent = NativePath::parse(&fixture.path("chosen")).unwrap();
    // A second clone of the same URL takes the next free name, as VS Code's does.
    for (id, folder) in [(1, "chosen/remote"), (2, "chosen/remote-1")] {
        let clone = GitClone {
            workspace_id: workspace,
            request_id: RequestId::from_bytes([u8::try_from(id).unwrap(); 16]).unwrap(),
            picker: SessionId::new(1).unwrap(),
            parent: FilePath::new(fixture.path("chosen")).unwrap(),
            url: GitName::new(fixture.path("remote.git")).unwrap(),
            recursive: false,
        };
        let view = harness
            .scopes
            .view(workspace, connection(), Operation::GitClone)
            .unwrap();
        harness
            .service()
            .clone_repository(harness.open(id), view, &clone, &parent);
        let FilesReply::GitInit(cloned) = harness.reply(id, Operation::GitClone) else {
            panic!("expected the clone's repository");
        };
        assert_eq!(cloned.repository.as_str(), fixture.path(folder));
        assert_eq!(
            fixture.commit_of(folder, "HEAD"),
            fixture.commit_of("source", "HEAD")
        );
    }
}
