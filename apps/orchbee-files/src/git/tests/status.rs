//! Status reads every kind of change a repository can hold, in Git's order, counts each in the
//! group VS Code shows it in, and stops at its entry limit.

use super::{
    super::{
        porcelain::EntryKind,
        status::{Counts, status},
    },
    fixture::Fixture,
};
use orchbee_files_protocol::RepositoryOperation;

#[test]
fn status_reads_each_kind_of_change_and_counts_it_in_its_group() {
    let fixture = Fixture::new();
    fixture.repository("repo");
    let found = fixture.found("repo");
    let read = |limit| status(&found, limit, &fixture.limits(), &fixture.output);

    let unborn = read(100).unwrap();
    assert!(unborn.head.unborn && unborn.head.commit.is_none());
    assert_eq!(unborn.head.branch.as_deref(), Some("main"));

    for name in [
        "modified", "staged", "both", "deleted", "removed", "moved", "conflict",
    ] {
        fixture.write(&format!("repo/{name}"), name);
    }
    fixture.git("repo", &["add", "."]);
    fixture.git("repo", &["commit", "-q", "-m", "seed"]);
    fixture.git("repo", &["checkout", "-q", "-b", "side"]);
    fixture.write("repo/conflict", "side");
    fixture.git("repo", &["commit", "-q", "-am", "side"]);
    fixture.git("repo", &["checkout", "-q", "main"]);
    fixture.write("repo/conflict", "main");
    fixture.git("repo", &["commit", "-q", "-am", "main"]);
    let merged = fixture.run("repo", &["merge", "-q", "side"]);
    assert!(!merged.status.unwrap().success());

    fixture.write("repo/modified", "changed");
    fixture.write("repo/staged", "changed");
    fixture.git("repo", &["add", "staged"]);
    fixture.write("repo/both", "changed");
    fixture.git("repo", &["add", "both"]);
    fixture.write("repo/both", "changed again");
    std::fs::remove_file(fixture.path("repo/deleted")).unwrap();
    fixture.git("repo", &["rm", "-q", "removed"]);
    fixture.git("repo", &["mv", "moved", "moved too"]);
    fixture.write("repo/added é", "new");
    fixture.git("repo", &["add", "added é"]);
    fixture.write("repo/folder/untracked", "new");
    // Only Linux and macOS allow a line break in a name.
    let line_break = cfg!(unix).then(|| {
        fixture.write("repo/line\nbreak", "new");
        "line\nbreak"
    });

    let snapshot = read(100).unwrap();
    assert_eq!(snapshot.head.branch.as_deref(), Some("main"));
    assert!(!snapshot.head.detached && !snapshot.head.unborn);
    assert_eq!(snapshot.head.commit.as_ref().map(String::len), Some(40));
    assert_eq!(snapshot.operation, RepositoryOperation::Merge);
    let letters = |path: &str| {
        snapshot
            .entries
            .iter()
            .find(|entry| entry.path == path.as_bytes())
            .map(|entry| (entry.kind, [entry.x, entry.y]))
    };
    assert_eq!(letters("modified"), Some((EntryKind::Changed, *b".M")));
    assert_eq!(letters("staged"), Some((EntryKind::Changed, *b"M.")));
    assert_eq!(letters("both"), Some((EntryKind::Changed, *b"MM")));
    assert_eq!(letters("deleted"), Some((EntryKind::Changed, *b".D")));
    assert_eq!(letters("removed"), Some((EntryKind::Changed, *b"D.")));
    assert_eq!(letters("moved too"), Some((EntryKind::Renamed, *b"R.")));
    assert_eq!(letters("added é"), Some((EntryKind::Changed, *b"A.")));
    assert_eq!(letters("conflict"), Some((EntryKind::Unmerged, *b"UU")));
    assert_eq!(
        letters("folder/untracked"),
        Some((EntryKind::Untracked, *b"??"))
    );
    let renamed = snapshot
        .entries
        .iter()
        .find(|entry| entry.kind == EntryKind::Renamed)
        .unwrap();
    assert_eq!(renamed.original.as_deref(), Some(&b"moved"[..]));
    if let Some(line_break) = line_break {
        assert_eq!(letters(line_break), Some((EntryKind::Untracked, *b"??")));
    }
    assert_eq!(
        snapshot.counts,
        Counts {
            merge: 1,
            index: 5,
            working_tree: 3,
            untracked: 1 + usize::from(line_break.is_some()),
        }
    );
    assert!(!snapshot.truncated);

    let truncated = read(3).unwrap();
    assert!(truncated.truncated);
    assert_eq!(truncated.entries, snapshot.entries[..3]);
}
