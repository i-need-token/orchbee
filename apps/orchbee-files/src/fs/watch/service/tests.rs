//! The watching rules on a fake clock: which paths are watched, root polling, restarts, missing
//! folders and repository reports.

use std::sync::{Arc, Mutex};

use orchbee_files_protocol::{FilePath, Separator};

use super::*;
use crate::scope::path;

/// `relative`, with `/` between segments, below this machine's `/w` or `C:\w`; `/` alone is the
/// drive or filesystem root.
fn native(relative: &str) -> NativePath {
    let (base, separator) = match path::SEPARATOR {
        Separator::Backslash => (r"C:", "\\"),
        Separator::Slash => ("", "/"),
    };
    let mut path = base.to_owned();
    for segment in relative.split('/').filter(|segment| !segment.is_empty()) {
        path.push_str(separator);
        path.push_str(segment);
    }
    if path.len() == base.len() {
        path.push_str(separator);
    }
    NativePath::parse(&path).unwrap()
}

fn workspace() -> WorkspaceId {
    WorkspaceId::from_bytes([7; 16]).unwrap()
}

/// One subscriber that loaded `loaded` and opened `opened`.
fn interest(loaded: &[&str], opened: &[&str]) -> Interest {
    let mut holder = stream::Holder::default();
    let mut interest = Interest::default();
    for folder in loaded {
        holder.loaded.insert(native(folder).key(), native(folder));
    }
    for file in opened {
        holder.opened.insert(native(file).key());
        interest.opened.insert(native(file).key(), native(file));
    }
    interest
        .holders
        .insert(ConnectionId::new(1).unwrap(), holder);
    interest
}

fn present(relative: &str, generation: u64) -> Option<RootFact> {
    Some(RootFact {
        generation,
        root: Root::Present(FilePath::new(native(relative).as_str()).unwrap()),
    })
}

fn order(watch: &[(&str, Need)], unwatch: &[(&str, Need)]) -> Order {
    let paths = |requests: &[(&str, Need)]| {
        requests
            .iter()
            .map(|(path, need)| (native(path), *need))
            .collect()
    };
    Order {
        watch: paths(watch),
        unwatch: paths(unwatch),
    }
}

fn watch_notice(state: WatchState) -> Notice {
    Notice::Watch {
        workspace: workspace(),
        state,
    }
}

#[test]
fn a_subscribed_workspace_watches_the_fewest_paths_that_cover_its_root_and_opened_files() {
    let mut core = Core::new(Timing::VS_CODE, Vec::new());
    core.interest(workspace(), interest(&[], &["w/in.txt", "o/f.txt"]), 1);
    let effects = core.effects();
    assert_eq!(effects.roots, [workspace()]);
    assert_eq!(
        effects.orders,
        [order(&[("o", Need::Folder), ("w", Need::Folder)], &[])]
    );

    // The root's recursive watch covers the folder of the file inside it, which stops first.
    core.root(workspace(), present("w", 1), Instant::now());
    let effects = core.effects();
    assert_eq!(
        effects.orders,
        [
            order(&[], &[("w", Need::Folder)]),
            order(&[("w", Need::Tree)], &[]),
        ]
    );
    assert!(effects.notices.is_empty());

    // A new scope keeps the old root until its fact arrives; the new root starts in the order
    // that stops the old one, and the adapter starts first.
    core.interest(workspace(), interest(&[], &["o/f.txt"]), 2);
    assert!(core.effects().orders.is_empty());
    core.root(workspace(), present("v", 2), Instant::now());
    assert_eq!(
        core.effects().orders,
        [order(&[("v", Need::Tree)], &[("w", Need::Tree)])]
    );
}

#[test]
fn a_special_root_watches_loaded_folders_and_a_refused_tree_degrades() {
    let mut core = Core::new(Timing::VS_CODE, vec![native("home")]);
    core.interest(workspace(), interest(&["home", "home/a", "w"], &[]), 1);
    core.root(workspace(), present("home", 1), Instant::now());
    assert_eq!(
        core.effects().orders,
        [order(
            &[("home", Need::Folder), ("home/a", Need::Folder)],
            &[]
        )]
    );

    core.interest(workspace(), interest(&["w", "w/a"], &[]), 2);
    core.root(workspace(), present("w", 2), Instant::now());
    core.effects();
    let refused = Native::Refused {
        path: native("w"),
        need: Need::Tree,
        why: Why::Limit,
    };
    core.native(refused, Instant::now());
    let effects = core.effects();
    assert_eq!(
        effects.orders,
        [
            order(&[], &[("w", Need::Tree)]),
            order(&[("w", Need::Folder), ("w/a", Need::Folder)], &[]),
        ]
    );
    assert_eq!(effects.notices, [watch_notice(WatchState::Degraded)]);
}

#[test]
fn a_missing_root_is_read_again_every_poll_and_reported_when_it_returns() {
    let start = Instant::now();
    let mut core = Core::new(Timing::VS_CODE, Vec::new());
    core.interest(workspace(), interest(&[], &[]), 1);
    let missing = Root::Missing(FilePath::new(native("w").as_str()).unwrap());
    core.root(
        workspace(),
        Some(RootFact {
            generation: 1,
            root: missing,
        }),
        start,
    );
    core.effects();
    assert_eq!(core.due(), Some(start + Timing::VS_CODE.poll));
    core.tick(start + Timing::VS_CODE.poll);
    assert_eq!(core.effects().roots, [workspace()]);

    core.root(workspace(), present("w", 1), start + Timing::VS_CODE.poll);
    let effects = core.effects();
    assert_eq!(effects.orders, [order(&[("w", Need::Tree)], &[])]);
    assert_eq!(
        effects.notices,
        [Notice::Root {
            workspace: workspace(),
            generation: 1,
            notice: FilesNotice::FsUpdate(FsUpdate::Root {
                root: present("w", 1).unwrap().root,
            }),
        }]
    );
    // The root's own deletion ends its watch and reads it again.
    let deleted = Change {
        kind: ChangeKind::Deleted,
        path: native("w"),
        directory: true,
    };
    core.native(Native::Changes(vec![deleted]), start);
    let effects = core.effects();
    assert_eq!(effects.orders, [order(&[], &[("w", Need::Tree)])]);
    assert_eq!(effects.roots, [workspace()]);
}

#[test]
fn a_failed_watch_restarts_and_past_five_restarts_its_workspace_stops() {
    let mut now = Instant::now();
    let mut core = Core::new(Timing::VS_CODE, Vec::new());
    core.interest(workspace(), interest(&[], &[]), 1);
    core.root(workspace(), present("w", 1), now);
    core.effects();
    for _ in 0..Timing::VS_CODE.restarts {
        core.native(Native::Failed(Some(native("w/deep"))), now);
        assert_eq!(core.effects().orders, [order(&[], &[("w", Need::Tree)])]);
        now += Timing::VS_CODE.restart;
        assert_eq!(core.due(), Some(now));
        core.tick(now);
        assert_eq!(core.effects().orders, [order(&[("w", Need::Tree)], &[])]);
    }
    core.native(Native::Failed(Some(native("w"))), now);
    let effects = core.effects();
    assert_eq!(effects.orders, [order(&[], &[("w", Need::Tree)])]);
    assert_eq!(effects.notices, [watch_notice(WatchState::Stopped)]);
    assert_eq!(core.due(), None);
    // Stopped stays: a new subscriber watches nothing.
    core.interest(workspace(), interest(&["w"], &[]), 1);
    assert!(core.effects().orders.is_empty());
}

#[test]
fn a_deleted_folder_is_checked_until_it_returns_with_the_opened_files_in_it() {
    let start = Instant::now();
    let millis = Duration::from_millis;
    let mut core = Core::new(Timing::VS_CODE, Vec::new());
    core.interest(workspace(), interest(&[], &["o/f.txt"]), 1);
    assert_eq!(core.effects().orders, [order(&[("o", Need::Folder)], &[])]);
    let deleted = Change {
        kind: ChangeKind::Deleted,
        path: native("o"),
        directory: true,
    };
    core.native(Native::Changes(vec![deleted]), start);
    assert_eq!(core.effects().orders, [order(&[], &[("o", Need::Folder)])]);
    // The opened file's deletion is held, checked and delivered.
    core.tick(start + millis(100));
    assert_eq!(core.effects().checks, [(workspace(), native("o/f.txt"))]);
    let checked = start + millis(100);
    core.resolved(workspace(), vec![(native("o/f.txt"), None)], checked);
    core.tick(checked + millis(75));
    assert_eq!(
        delivered(&mut core),
        [(ChangeKind::Deleted, native("o/f.txt"))]
    );

    let poll = start + Timing::VS_CODE.poll;
    core.tick(poll);
    assert_eq!(
        core.effects().polls,
        [(native("o"), vec![native("o/f.txt")])]
    );
    core.polled(&native("o"), true, vec![native("o/f.txt")], poll);
    assert_eq!(core.effects().orders, [order(&[("o", Need::Folder)], &[])]);
    core.tick(poll + millis(75));
    assert_eq!(
        delivered(&mut core),
        [(ChangeKind::Added, native("o/f.txt"))]
    );
}

/// The changes delivered to Clients in the core's effects.
fn delivered(core: &mut Core) -> Vec<(ChangeKind, NativePath)> {
    updates(core).0
}

/// The changes, and the folders to list again, delivered to Clients in the core's effects.
fn updates(core: &mut Core) -> (Vec<(ChangeKind, NativePath)>, Vec<NativePath>) {
    let parse = |path: &FilePath| NativePath::parse(path.as_str()).unwrap();
    let mut changes = Vec::new();
    let mut rescan = Vec::new();
    for notice in core.effects().notices {
        match notice {
            Notice::To {
                notice: FilesNotice::FsUpdate(FsUpdate::Changes { changes: delivered }),
                ..
            } => changes.extend(
                delivered
                    .as_slice()
                    .iter()
                    .map(|change| (change.kind, parse(&change.path))),
            ),
            Notice::To {
                notice: FilesNotice::FsUpdate(FsUpdate::Rescan { paths }),
                ..
            } => rescan.extend(paths.as_slice().iter().map(parse)),
            _ => {}
        }
    }
    (changes, rescan)
}

/// The native watcher's report that it started `started` and stopped `stopped`.
fn applied(started: &[(&str, Need)], stopped: &[(&str, Need)]) -> Native {
    let order = order(started, stopped);
    Native::Applied {
        started: order.watch,
        stopped: order.unwatch,
    }
}

#[test]
fn a_folder_read_before_its_watch_takes_effect_is_listed_again_once_it_does() {
    let mut now = Instant::now();
    let window = Duration::from_millis(75);
    let mut core = Core::new(Timing::VS_CODE, Vec::new());
    // The folders load and are read before the root's watch takes effect, so a change between a
    // read and the watch has no event.
    core.interest(workspace(), interest(&["w", "w/a"], &[]), 1);
    core.root(workspace(), present("w", 1), now);
    assert_eq!(core.effects().orders, [order(&[("w", Need::Tree)], &[])]);
    core.native(applied(&[("w", Need::Tree)], &[]), now);
    now += window;
    core.tick(now);
    assert_eq!(
        updates(&mut core),
        (vec![], vec![native("w"), native("w/a")])
    );

    // A folder that loads while the watch is in effect is read after it took effect, and the
    // event of a later change finds it loaded.
    core.interest(workspace(), interest(&["w", "w/a", "w/b"], &[]), 1);
    let created = Change {
        kind: ChangeKind::Added,
        path: native("w/b/new.txt"),
        directory: false,
    };
    core.native(Native::Changes(vec![created]), now);
    now += window;
    core.tick(now);
    assert_eq!(
        updates(&mut core),
        (vec![(ChangeKind::Added, native("w/b/new.txt"))], vec![])
    );

    // A failed watch reported nothing until its restart took effect.
    core.native(Native::Failed(Some(native("w"))), now);
    assert_eq!(core.effects().orders, [order(&[], &[("w", Need::Tree)])]);
    core.native(applied(&[], &[("w", Need::Tree)]), now);
    now += Timing::VS_CODE.restart;
    core.tick(now);
    assert_eq!(core.effects().orders, [order(&[("w", Need::Tree)], &[])]);
    core.native(applied(&[("w", Need::Tree)], &[]), now);
    now += window;
    core.tick(now);
    assert_eq!(
        updates(&mut core).1,
        [native("w"), native("w/a"), native("w/b")]
    );

    // A watch that takes over from one in effect asks for no listing.
    core.interest(workspace(), interest(&["w/a"], &[]), 2);
    core.root(workspace(), present("w/a", 2), now);
    assert_eq!(
        core.effects().orders,
        [order(&[("w/a", Need::Tree)], &[("w", Need::Tree)])]
    );
    core.native(applied(&[("w/a", Need::Tree)], &[("w", Need::Tree)]), now);
    now += window;
    core.tick(now);
    assert_eq!(updates(&mut core), (vec![], vec![]));
}

#[test]
fn a_file_opened_before_its_watch_takes_effect_is_read_again_once_it_does() {
    let mut now = Instant::now();
    let window = Duration::from_millis(75);
    let mut core = Core::new(Timing::VS_CODE, Vec::new());
    // The file outside the root is read before its folder's watch takes effect, and the file
    // inside it before the root's does.
    core.interest(workspace(), interest(&[], &["o/f.txt", "w/in.txt"]), 1);
    core.root(workspace(), present("w", 1), now);
    core.effects();
    core.native(applied(&[("o", Need::Folder), ("w", Need::Tree)], &[]), now);
    now += window;
    core.tick(now);
    assert_eq!(
        updates(&mut core),
        (vec![], vec![native("o/f.txt"), native("w/in.txt")])
    );

    // A file opened under a watch in effect is read after it took effect, and the event of a later
    // change finds it opened.
    core.interest(
        workspace(),
        interest(&[], &["o/f.txt", "w/in.txt", "w/deep/g.txt"]),
        1,
    );
    assert!(core.effects().orders.is_empty());
    let changed = Change {
        kind: ChangeKind::Changed,
        path: native("w/deep/g.txt"),
        directory: false,
    };
    core.native(Native::Changes(vec![changed]), now);
    now += window;
    core.tick(now);
    assert_eq!(
        updates(&mut core),
        (vec![(ChangeKind::Changed, native("w/deep/g.txt"))], vec![])
    );

    // A restarted watch reads again the files whose folders it alone covers.
    core.native(Native::Failed(Some(native("w"))), now);
    core.native(applied(&[], &[("w", Need::Tree)]), now);
    now += Timing::VS_CODE.restart;
    core.tick(now);
    core.effects();
    core.native(applied(&[("w", Need::Tree)], &[]), now);
    now += window;
    core.tick(now);
    assert_eq!(
        updates(&mut core).1,
        [native("w/deep/g.txt"), native("w/in.txt")]
    );
}

#[test]
fn a_repository_reports_its_working_tree_and_the_first_level_of_its_git_directory() {
    let mut core = Core::new(Timing::VS_CODE, Vec::new());
    let reported = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&reported);
    let repository = Repository {
        tree: native("r"),
        git_dir: Some(native("r/.git")),
        report: ports::Report::new(move |change| sink.lock().unwrap().push(change)),
    };
    core.watch_repository(workspace(), repository);
    assert_eq!(core.effects().orders, [order(&[("r", Need::Tree)], &[])]);
    let batch = |paths: &[&str]| {
        paths
            .iter()
            .map(|relative| Change {
                kind: ChangeKind::Changed,
                path: native(relative),
                directory: false,
            })
            .collect::<Vec<_>>()
    };
    let mut kinds = Vec::new();
    for paths in [
        &[
            "r/.git/objects/ab/cd",
            "r/.git/index.lock",
            "r/sub/.git/HEAD",
        ][..],
        &["r/src/a.rs", "r/.git/objects/ab"],
        &["r/.git/HEAD"],
        &["r/src/b.rs", "r/.git"],
    ] {
        core.native(Native::Changes(batch(paths)), Instant::now());
        for (report, change) in core.effects().reports {
            report.changed(change);
        }
        kinds.push(std::mem::take(&mut *reported.lock().unwrap()));
    }
    assert_eq!(
        kinds,
        [
            vec![],
            vec![ports::Change::Content],
            vec![ports::Change::Content],
            vec![ports::Change::Repository],
        ]
    );
}

#[test]
fn the_fewest_watches_cover_every_request() {
    let covered = cover(
        [
            (native("w/sub"), Need::Tree),
            (native("w"), Need::Tree),
            (native("w/sub/x"), Need::Folder),
            (native("o"), Need::Folder),
            (native("o"), Need::Folder),
        ]
        .into_iter(),
    );
    let covered: Vec<(NativePath, Need)> = covered
        .into_iter()
        .map(|((_, need), path)| (path, need))
        .collect();
    assert_eq!(
        covered,
        [(native("o"), Need::Folder), (native("w"), Need::Tree)]
    );
}
