//! History reads against a fixed repository whose commits are dated, so every run names them the
//! same: the Graph walks named revisions in topological order with parents, full references and
//! first-parent statistics, a page says whether more follow, a file's history follows its rename
//! with the path of each commit, a message keeps its bytes, and changes, merge bases and stashes
//! read as VS Code reads them.

use super::{
    super::{
        command,
        history::{
            Change, ChangeKind, ChangesQuery, LogQuery, Statistics, changes, log, merge_base,
            stashes,
        },
    },
    fixture::Fixture,
};

/// main: A adds `a.txt`; B renames it to `b.txt` and adds a line, tagged `v1`; D adds a line; M
/// merges `side`, whose C adds `c.txt` on B. `other` holds E on A alone.
struct History {
    fixture: Fixture,
    a: String,
    b: String,
    c: String,
    d: String,
    e: String,
    m: String,
}

fn history() -> History {
    let fixture = Fixture::new();
    fixture.repository("repo");
    fixture.write("repo/a.txt", "one\ntwo\n");
    fixture.git("repo", &["add", "a.txt"]);
    fixture.git_at("repo", &["commit", "-q", "-m", "first"], 0);
    let a = fixture.commit_of("repo", "HEAD");
    fixture.git("repo", &["mv", "a.txt", "b.txt"]);
    fixture.write("repo/b.txt", "one\ntwo\nthree\n");
    fixture.git("repo", &["add", "b.txt"]);
    fixture.git_at("repo", &["commit", "-q", "-m", "rename\n\nwith a body"], 1);
    let b = fixture.commit_of("repo", "HEAD");
    fixture.git("repo", &["tag", "v1"]);
    fixture.git("repo", &["checkout", "-q", "-b", "side"]);
    fixture.write("repo/c.txt", "x\n");
    fixture.git("repo", &["add", "c.txt"]);
    fixture.git_at("repo", &["commit", "-q", "-m", "side"], 2);
    let c = fixture.commit_of("repo", "HEAD");
    fixture.git("repo", &["checkout", "-q", "main"]);
    fixture.write("repo/b.txt", "one\ntwo\nthree\nfour\n");
    fixture.git_at("repo", &["commit", "-q", "-a", "-m", "four"], 3);
    let d = fixture.commit_of("repo", "HEAD");
    fixture.git_at(
        "repo",
        &["merge", "-q", "--no-ff", "-m", "merge", "side"],
        4,
    );
    let m = fixture.commit_of("repo", "HEAD");
    fixture.git("repo", &["checkout", "-q", "-b", "other", &a]);
    fixture.write("repo/e.txt", "e\n");
    fixture.git("repo", &["add", "e.txt"]);
    fixture.git_at("repo", &["commit", "-q", "-m", "other"], 5);
    let e = fixture.commit_of("repo", "HEAD");
    fixture.git("repo", &["checkout", "-q", "main"]);
    History {
        fixture,
        a,
        b,
        c,
        d,
        e,
        m,
    }
}

fn version(fixture: &Fixture) -> (u32, u32, u32) {
    command::version(fixture.git("", &["--version"]).as_bytes()).unwrap()
}

fn names(page: &[super::super::history::Commit]) -> Vec<&str> {
    page.iter().map(|commit| commit.commit.as_str()).collect()
}

#[test]
fn graph_pages_walk_named_revisions_in_topological_order() {
    let history = history();
    let fixture = &history.fixture;
    let repository = fixture.found("repo");
    let version = version(fixture);
    let query = LogQuery {
        revisions: vec!["refs/heads/main".to_owned()],
        statistics: true,
        limit: 3,
        ..LogQuery::default()
    };

    let first = log(
        &repository,
        version,
        &query,
        &fixture.limits(),
        &fixture.output,
    )
    .unwrap();
    assert_eq!(names(&first.commits), [&history.m, &history.c, &history.d]);
    assert!(first.more);
    let merge = &first.commits[0];
    assert_eq!(merge.parents, [history.d.clone(), history.c.clone()]);
    assert_eq!(merge.references, ["HEAD -> refs/heads/main"]);
    assert_eq!(merge.author_name, "Orchbee Fixture");
    assert_eq!(merge.author_email, "fixture@example.invalid");
    assert_eq!(merge.commit_time, 1_700_000_000 + 4 * 60);
    // A merge counts what it brought against its first parent.
    let added = Statistics {
        files: 1,
        insertions: 1,
        deletions: 0,
    };
    assert_eq!(merge.statistics, Some(added));
    assert_eq!(first.commits[1].references, ["refs/heads/side"]);

    let rest = LogQuery { skip: 3, ..query };
    let second = log(
        &repository,
        version,
        &rest,
        &fixture.limits(),
        &fixture.output,
    )
    .unwrap();
    assert_eq!(names(&second.commits), [&history.b, &history.a]);
    assert!(!second.more);
    assert_eq!(second.commits[0].references, ["tag: refs/tags/v1"]);
    assert_eq!(second.commits[0].message, "rename\n\nwith a body");
    assert!(second.commits[1].parents.is_empty());

    let everything = LogQuery {
        all: true,
        limit: 10,
        ..LogQuery::default()
    };
    let all = log(
        &repository,
        version,
        &everything,
        &fixture.limits(),
        &fixture.output,
    )
    .unwrap();
    assert_eq!(all.commits.len(), 6);
    assert!(names(&all.commits).contains(&history.e.as_str()));

    fixture.repository("empty");
    let unborn = log(
        &fixture.found("empty"),
        version,
        &LogQuery {
            limit: 10,
            ..LogQuery::default()
        },
        &fixture.limits(),
        &fixture.output,
    )
    .unwrap();
    assert!(unborn.commits.is_empty() && !unborn.more);
}

#[test]
fn file_history_follows_a_rename_with_the_path_of_each_commit() {
    let history = history();
    let fixture = &history.fixture;
    let repository = fixture.found("repo");
    let query = LogQuery {
        path: Some(fixture.path("repo/b.txt")),
        follow: true,
        statistics: true,
        limit: 10,
        ..LogQuery::default()
    };

    let page = log(
        &repository,
        version(fixture),
        &query,
        &fixture.limits(),
        &fixture.output,
    )
    .unwrap();
    assert_eq!(names(&page.commits), [&history.d, &history.b, &history.a]);
    let paths = page
        .commits
        .iter()
        .map(|commit| commit.path.clone())
        .collect::<Vec<_>>();
    let b = Some(fixture.path("repo/b.txt"));
    assert_eq!(paths, [b.clone(), b, Some(fixture.path("repo/a.txt"))]);
    let counted = page
        .commits
        .iter()
        .map(|commit| commit.statistics.map(|counted| counted.insertions))
        .collect::<Vec<_>>();
    assert_eq!(counted, [Some(1), Some(1), Some(2)]);

    let folder = LogQuery {
        path: Some(fixture.path("repo/c.txt")),
        limit: 10,
        ..LogQuery::default()
    };
    let page = log(
        &repository,
        version(fixture),
        &folder,
        &fixture.limits(),
        &fixture.output,
    )
    .unwrap();
    assert_eq!(names(&page.commits), [&history.c]);
    assert_eq!(page.commits[0].path, None);
}

#[test]
fn a_message_keeps_a_separator_that_starts_no_commit() {
    let fixture = Fixture::new();
    fixture.repository("repo");
    let lookalike = format!("subject\n\n\u{1e}{}\nstill the body", "a".repeat(40));
    fixture.git_at(
        "repo",
        &["commit", "-q", "--allow-empty", "-m", &lookalike],
        0,
    );
    fixture.git_at("repo", &["commit", "-q", "--allow-empty", "-m", "next"], 1);

    let page = log(
        &fixture.found("repo"),
        version(&fixture),
        &LogQuery {
            limit: 10,
            ..LogQuery::default()
        },
        &fixture.limits(),
        &fixture.output,
    )
    .unwrap();
    let messages = page
        .commits
        .iter()
        .map(|commit| commit.message.as_str())
        .collect::<Vec<_>>();
    assert_eq!(messages, ["next", lookalike.as_str()]);
}

#[test]
fn changes_merge_bases_and_stashes_read_as_vs_code_reads_them() {
    let history = history();
    let fixture = &history.fixture;
    let repository = fixture.found("repo");
    let limits = fixture.limits();
    let output = &fixture.output;

    let between = |from: Option<&String>, to: &String| ChangesQuery {
        from: from.cloned(),
        to: to.clone(),
    };
    let renamed = changes(
        &repository,
        &between(Some(&history.a), &history.b),
        &limits,
        output,
    )
    .unwrap();
    assert_eq!(
        renamed,
        [Change {
            kind: ChangeKind::Renamed,
            path: fixture.path("repo/b.txt"),
            original: Some(fixture.path("repo/a.txt")),
            insertions: Some(1),
            deletions: Some(0),
        }]
    );
    let first = changes(&repository, &between(None, &history.a), &limits, output).unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].kind, ChangeKind::Added);
    assert_eq!(first[0].insertions, Some(2));
    // Against D, C's changes start from their merge base, B.
    let side = changes(
        &repository,
        &between(Some(&history.d), &history.c),
        &limits,
        output,
    )
    .unwrap();
    assert_eq!(side.len(), 1);
    assert_eq!(side[0].path, fixture.path("repo/c.txt"));

    assert_eq!(
        merge_base(&repository, &history.d, &history.c, &limits, output).unwrap(),
        Some(history.b.clone())
    );
    fixture.git("repo", &["checkout", "-q", "--orphan", "unrelated"]);
    fixture.git("repo", &["rm", "-q", "-r", "--cached", "."]);
    fixture.git_at("repo", &["commit", "-q", "--allow-empty", "-m", "alone"], 6);
    let alone = fixture.commit_of("repo", "HEAD");
    assert_eq!(
        merge_base(&repository, &alone, &history.m, &limits, output).unwrap(),
        None
    );

    fixture.git("repo", &["checkout", "-q", "-f", "main"]);
    fixture.write("repo/b.txt", "changed\n");
    fixture.git("repo", &["stash", "push", "-q", "-m", "keep this"]);
    fixture.write("repo/b.txt", "again\n");
    fixture.write("repo/new.txt", "untracked\n");
    fixture.git("repo", &["stash", "push", "-q", "-u"]);
    let listed = stashes(&repository, &limits, output).unwrap();
    let read = listed
        .iter()
        .map(|stash| {
            (
                stash.index,
                stash.wip,
                stash.branch.as_str(),
                stash.parents.len(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(read, [(0, true, "main", 3), (1, false, "main", 2)]);
    assert_eq!(listed[1].description, "keep this");
    assert!(listed[0].description.ends_with(" merge"));
}
