//! The delivery rules on a fake clock: the filter, the window, the hold, overflow and lost changes,
//! and the Update bounds.

use super::*;

/// `relative`, with `/` between segments, below this machine's `/w` or `C:\w`.
fn native(relative: &str) -> NativePath {
    let (base, separator) = match path::SEPARATOR {
        Separator::Backslash => (r"C:\w", "\\"),
        Separator::Slash => ("/w", "/"),
    };
    let mut path = base.to_owned();
    for segment in relative.split('/').filter(|segment| !segment.is_empty()) {
        path.push_str(separator);
        path.push_str(segment);
    }
    NativePath::parse(&path).unwrap()
}

fn change(kind: ChangeKind, relative: &str) -> Change {
    Change {
        kind,
        path: native(relative),
        directory: false,
    }
}

fn connection(number: u64) -> ConnectionId {
    ConnectionId::new(number).unwrap()
}

/// The interest of holders, each with its number, loaded folders and opened files.
fn interest(holders: &[(u64, &[&str], &[&str])]) -> Interest {
    let mut interest = Interest {
        root: Some(native("")),
        ..Interest::default()
    };
    for (number, loaded, opened) in holders {
        let holder = Holder {
            loaded: loaded
                .iter()
                .map(|folder| (native(folder).key(), native(folder)))
                .collect(),
            everything: false,
            opened: opened.iter().map(|file| native(file).key()).collect(),
        };
        interest.holders.insert(connection(*number), holder);
        for file in *opened {
            interest.opened.insert(native(file).key(), native(file));
        }
    }
    interest
}

fn kinds(changes: &[Change]) -> Vec<(ChangeKind, NativePath)> {
    changes
        .iter()
        .map(|change| (change.kind, change.path.clone()))
        .collect()
}

#[test]
fn a_client_receives_its_loaded_folders_and_opened_files_coalesced_after_75_ms() {
    let start = Instant::now();
    let interest = interest(&[(1, &["src"], &[]), (2, &[], &["other/x.txt"])]);
    let mut stream = Stream::default();
    for change in [
        change(ChangeKind::Added, "src/a.txt"),
        change(ChangeKind::Changed, "src/a.txt"),
        change(ChangeKind::Changed, "other/x.txt"),
        change(ChangeKind::Changed, "other/y.txt"),
        change(ChangeKind::Added, "src/deep/z.txt"),
    ] {
        stream.observe(change, &interest, start);
    }
    assert_eq!(stream.due(), Some(start + WINDOW));
    assert_eq!(stream.tick(start + WINDOW / 2, &interest), Due::default());
    let mut due = stream.tick(start + WINDOW, &interest);
    due.deliveries.sort_by_key(|delivery| delivery.connection);
    let delivered: Vec<(ConnectionId, Vec<(ChangeKind, NativePath)>)> = due
        .deliveries
        .iter()
        .map(|delivery| (delivery.connection, kinds(&delivery.changes)))
        .collect();
    assert_eq!(
        delivered,
        [
            (
                connection(1),
                vec![(ChangeKind::Added, native("src/a.txt"))]
            ),
            (
                connection(2),
                vec![(ChangeKind::Changed, native("other/x.txt"))]
            ),
        ]
    );
    assert_eq!(stream.due(), None);
}

#[test]
fn an_opened_files_deletion_is_held_and_reported_by_whether_it_exists_again() {
    let start = Instant::now();
    let interest = interest(&[(1, &[""], &["f.txt", "dir/g.txt"])]);
    let mut stream = Stream::default();
    stream.observe(change(ChangeKind::Deleted, "f.txt"), &interest, start);
    // The folder's deletion holds the opened file inside it and reaches the Client itself.
    stream.observe(change(ChangeKind::Deleted, "dir"), &interest, start);
    stream.observe(change(ChangeKind::Added, "f.txt"), &interest, start);
    let due = stream.tick(start + WINDOW, &interest);
    assert_eq!(due.checks, Vec::<NativePath>::new());
    assert_eq!(
        kinds(&due.deliveries[0].changes),
        [(ChangeKind::Deleted, native("dir"))]
    );
    let mut due = stream.tick(start + HOLD, &interest);
    due.checks.sort_by_key(NativePath::key);
    assert_eq!(due.checks, [native("dir/g.txt"), native("f.txt")]);
    let checked = start + HOLD + WINDOW / 3;
    stream.resolved(&native("f.txt"), Some(false), &interest, checked);
    stream.resolved(&native("dir/g.txt"), None, &interest, checked);
    let due = stream.tick(checked + WINDOW, &interest);
    assert_eq!(
        kinds(&due.deliveries[0].changes),
        [
            (ChangeKind::Deleted, native("dir/g.txt")),
            (ChangeKind::Changed, native("f.txt")),
        ]
    );
}

#[test]
fn an_overflow_and_lost_changes_become_rescans_of_loaded_folders() {
    let start = Instant::now();
    let interest = interest(&[(1, &["a", "a/b", "c"], &[])]);
    let mut stream = Stream::default();
    for index in 0..30_501 {
        let added = change(ChangeKind::Added, &format!("a/{index}"));
        stream.observe(added, &interest, start);
    }
    stream.lost(Some(&native("a")), &interest, start);
    let due = stream.tick(start + WINDOW, &interest);
    assert_eq!(due.deliveries.len(), 1);
    assert!(due.deliveries[0].changes.is_empty());
    assert_eq!(due.deliveries[0].rescan, [native("a"), native("a/b")]);
    // Everywhere lost reaches every loaded folder.
    stream.lost(None, &interest, start + WINDOW);
    let due = stream.tick(start + WINDOW * 2, &interest);
    assert_eq!(
        due.deliveries[0].rescan,
        [native("a"), native("a/b"), native("c")]
    );
}

#[test]
fn updates_hold_at_most_500_items_within_the_record_bound() {
    let short: Vec<Change> = (0..1_001)
        .map(|index| change(ChangeKind::Changed, &format!("{index}")))
        .collect();
    let sizes: Vec<usize> = updates(short, vec![native("a")])
        .iter()
        .map(|notice| match notice {
            FilesNotice::FsUpdate(FsUpdate::Changes { changes }) => changes.as_slice().len(),
            FilesNotice::FsUpdate(FsUpdate::Rescan { paths }) => paths.as_slice().len(),
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    assert_eq!(sizes, [500, 500, 1, 1]);

    let long_name = "n".repeat(4_000);
    let long: Vec<Change> = (0..100)
        .map(|index| change(ChangeKind::Changed, &format!("{long_name}{index}")))
        .collect();
    let notices = updates(long, Vec::new());
    assert!(notices.len() > 1);
    let mut carried = 0;
    for notice in &notices {
        assert!(encode_notice(notice).len() <= MAXIMUM_REPLY_BYTES);
        if let FilesNotice::FsUpdate(FsUpdate::Changes { changes }) = notice {
            carried += changes.as_slice().len();
        }
    }
    assert_eq!(carried, 100);
}
