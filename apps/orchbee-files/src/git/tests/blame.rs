//! Blame attributes every line of a dated fixture file: at a commit through the file's rename, in
//! the working tree with uncommitted lines under the all-zero name, and past whitespace changes
//! with `-w`; each commit's author and summary come with the first range that names it.

use super::{
    super::{
        blame::{Blame, BlameQuery, blame},
        errors::GitErrorCode,
    },
    fixture::Fixture,
};
use orchbee_files_protocol::ErrorCode;

fn query(path: &str, commit: Option<&str>, ignore_whitespace: bool) -> BlameQuery {
    BlameQuery {
        path: path.to_owned(),
        commit: commit.map(str::to_owned),
        ignore_whitespace,
    }
}

#[test]
fn blame_attributes_every_line_at_a_commit_or_in_the_working_tree() {
    let fixture = Fixture::new();
    fixture.repository("repo");
    let repository = fixture.found("repo");
    let limits = fixture.limits();
    let output = &fixture.output;
    fixture.write("repo/a.txt", "one\ntwo\n");
    fixture.git("repo", &["add", "a.txt"]);
    fixture.git_at("repo", &["commit", "-q", "-m", "first"], 0);
    let first = fixture.commit_of("repo", "HEAD");
    fixture.git("repo", &["mv", "a.txt", "b.txt"]);
    fixture.write("repo/b.txt", "one\ntwo\nthree\n");
    fixture.git_at("repo", &["commit", "-q", "-a", "-m", "rename"], 1);
    let renamed = fixture.commit_of("repo", "HEAD");
    fixture.write("repo/b.txt", "one  \ntwo\nthree\n");
    fixture.git_at("repo", &["commit", "-q", "-a", "-m", "spaces"], 2);
    let spaces = fixture.commit_of("repo", "HEAD");
    let path = fixture.path("repo/b.txt");

    let at = blame(
        &repository,
        &query(&path, Some(&renamed), false),
        &limits,
        output,
    )
    .unwrap();
    let mut ranges = at
        .ranges
        .iter()
        .map(|range| (range.start, range.end, range.commit.as_str()))
        .collect::<Vec<_>>();
    ranges.sort_unstable();
    assert_eq!(ranges, [(1, 2, first.as_str()), (3, 3, renamed.as_str())]);
    let author = &at.commits[&first];
    assert_eq!(author.author_name, "Orchbee Fixture");
    assert_eq!(author.author_email, "fixture@example.invalid");
    assert_eq!(author.author_time, 1_700_000_000);
    assert_eq!(author.summary, "first");

    let strict = blame(
        &repository,
        &query(&path, Some(&spaces), false),
        &limits,
        output,
    )
    .unwrap();
    let lenient = blame(
        &repository,
        &query(&path, Some(&spaces), true),
        &limits,
        output,
    )
    .unwrap();
    let line_one = |blamed: &Blame| {
        blamed
            .ranges
            .iter()
            .find(|range| range.start == 1)
            .map(|range| range.commit.clone())
    };
    assert_eq!(line_one(&strict), Some(spaces));
    assert_eq!(line_one(&lenient), Some(first));

    fixture.write("repo/b.txt", "one  \ntwo\nthree\nfour\n");
    let working = blame(&repository, &query(&path, None, false), &limits, output).unwrap();
    let uncommitted = "0".repeat(40);
    let last = working
        .ranges
        .iter()
        .find(|range| range.start == 4)
        .unwrap();
    assert_eq!((last.end, last.commit.as_str()), (4, uncommitted.as_str()));
    assert_eq!(
        working.commits[&uncommitted].author_name,
        "Not Committed Yet"
    );

    let missing = fixture.path("repo/missing.txt");
    let error = blame(
        &repository,
        &query(&missing, Some(&renamed), false),
        &limits,
        output,
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::GitFailed);
    assert_eq!(error.git_error_code, Some(GitErrorCode::NoPathFound));
    let outside = fixture.path("elsewhere.txt");
    let error = blame(&repository, &query(&outside, None, false), &limits, output).unwrap_err();
    assert_eq!(error.code, ErrorCode::OutsideScope);
}
