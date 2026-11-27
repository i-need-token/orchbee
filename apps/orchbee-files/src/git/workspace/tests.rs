//! The display set, the warm pool, re-rooting, consent and status pages, driven with a fake clock
//! and fake Git: each test feeds discoveries and status ends as Git would, and checks the effects.

use super::*;
use std::{path::PathBuf, sync::OnceLock};

/// The one Client connection these tests subscribe from.
fn client() -> ConnectionId {
    ConnectionId::new(10).unwrap()
}

fn at(milliseconds: u64) -> Instant {
    static BASE: OnceLock<Instant> = OnceLock::new();
    *BASE.get_or_init(Instant::now) + Duration::from_millis(milliseconds)
}

fn path(name: &str) -> String {
    let base = if native_paths::SEPARATOR == '/' {
        "/w"
    } else {
        "C:\\w"
    };
    if name.is_empty() {
        base.to_owned()
    } else {
        native_paths::join(base, name)
    }
}

fn found(name: &str) -> Found {
    let git_dir = PathBuf::from(path(name)).join(".git");
    Found {
        toplevel: path(name),
        common_dir: git_dir.clone(),
        git_dir,
        safe_directory: None,
    }
}

fn discovered(root: Option<&str>, nested: &[&str], parent: Option<&str>) -> Discovered {
    Discovered {
        root: root.map(found),
        nested: nested.iter().map(|name| found(name)).collect(),
        parent: parent.map(found),
        ..Discovered::default()
    }
}

/// Moves the root to `root` and answers the discovery that follows with `answer`.
fn reroot(workspace: &mut Workspace, now: u64, root: &str, answer: Discovered) -> Vec<Effect> {
    assert!(workspace.set_root(at(now), Some(path(root))).is_empty());
    let due = workspace.next_wake().unwrap();
    assert_eq!(due, at(now) + DISCOVERY_DEBOUNCE);
    let effects = workspace.wake(due);
    let [
        Effect::Discover {
            generation,
            root: discovering,
        },
    ] = effects.as_slice()
    else {
        panic!("{effects:?}");
    };
    assert_eq!(*discovering, path(root));
    workspace.discovered(due, *generation, answer)
}

fn statuses(effects: &[Effect]) -> Vec<(String, u64)> {
    effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::Status {
                repository,
                generation,
            } => Some((repository.clone(), *generation)),
            _ => None,
        })
        .collect()
}

/// Ends every status `effects` started, as Git would at `now`.
fn settle(workspace: &mut Workspace, now: u64, effects: &[Effect]) {
    for (repository, generation) in statuses(effects) {
        let more = workspace.status_ended(at(now), &repository, generation, Ok(snapshot(1)));
        settle(workspace, now, &more);
    }
}

fn snapshot(entries: usize) -> Snapshot {
    Snapshot {
        entries: (0..entries)
            .map(|index| Entry {
                kind: super::super::porcelain::EntryKind::Untracked,
                x: b'?',
                y: b'?',
                submodule: false,
                path: format!("file {index}").into_bytes(),
                original: None,
            })
            .collect(),
        head: Head::default(),
        upstream: None,
        operation: RepositoryOperation::None,
        counts: Counts::default(),
        truncated: false,
    }
}

/// The display set, then the warm pool, each with whether it is in the pool and marked stale.
fn shown(workspace: &Workspace) -> Vec<(String, bool, bool)> {
    workspace
        .displayed
        .iter()
        .chain(&workspace.warm)
        .filter_map(|path| workspace.repository(path))
        .map(|repository| {
            (
                repository.found.toplevel.clone(),
                repository.warm,
                repository.stale,
            )
        })
        .collect()
}

#[test]
fn re_rooting_hands_watches_over_and_keeps_the_last_three_repositories_warm() {
    let mut workspace = Workspace::new();
    assert!(workspace.subscribe(at(0), 1, client()).is_empty());
    workspace.focus(at(0), client(), true);

    let effects = reroot(
        &mut workspace,
        0,
        "a",
        discovered(Some("a"), &["a/n"], None),
    );
    assert_eq!(
        effects[..2],
        [
            Effect::Watch {
                repository: path("a")
            },
            Effect::Watch {
                repository: path("a/n")
            },
        ]
    );
    let started = statuses(&effects);
    assert_eq!(started.len(), 2);
    for (repository, generation) in started {
        let ended = workspace.status_ended(at(600), &repository, generation, Ok(snapshot(1)));
        assert!(ended.contains(&Effect::Publish));
    }

    // Moving into a folder of the repository with consent keeps it, with nothing to rebuild; only
    // the nested repository left behind stops watching.
    workspace.consent(at(700), &path("a"));
    let effects = reroot(
        &mut workspace,
        700,
        "a/sub",
        discovered(None, &[], Some("a")),
    );
    assert!(statuses(&effects).is_empty(), "{effects:?}");
    let watching = effects
        .iter()
        .filter(|effect| matches!(effect, Effect::Watch { .. } | Effect::Unwatch { .. }))
        .collect::<Vec<_>>();
    assert_eq!(
        watching,
        [&Effect::Unwatch {
            repository: path("a/n")
        }]
    );
    assert_eq!(
        workspace.summary().repositories[0].kind,
        RepositoryKind::Parent
    );

    // A new root's watches start before the repositories left behind stop theirs.
    let effects = reroot(&mut workspace, 2000, "b", discovered(Some("b"), &[], None));
    settle(&mut workspace, 2600, &effects);
    let watch = effects.iter().position(|effect| {
        *effect
            == Effect::Watch {
                repository: path("b"),
            }
    });
    let unwatch = effects.iter().position(|effect| {
        *effect
            == Effect::Unwatch {
                repository: path("a"),
            }
    });
    assert!(watch.unwrap() < unwatch.unwrap(), "{effects:?}");
    assert_eq!(
        shown(&workspace),
        [
            (path("b"), false, false),
            (path("a/n"), true, false),
            (path("a"), true, false),
        ]
    );

    for (index, root) in ["c", "d", "e"].into_iter().enumerate() {
        let now = 3000 + 1000 * index as u64;
        let effects = reroot(&mut workspace, now, root, discovered(Some(root), &[], None));
        settle(&mut workspace, now + 600, &effects);
    }
    // `a/n` and `a` were left first, so they are the ones past the pool's three.
    assert_eq!(
        shown(&workspace),
        [
            (path("e"), false, false),
            (path("b"), true, false),
            (path("c"), true, false),
            (path("d"), true, false),
        ]
    );

    // Agent completion marks the warm repositories stale; a returning one keeps its last state
    // and refreshes once.
    let effects = workspace.agent_settled(at(7000));
    settle(&mut workspace, 7000, &effects);
    assert!(
        shown(&workspace)[1..]
            .iter()
            .all(|(_, warm, stale)| *warm && *stale)
    );
    let effects = reroot(&mut workspace, 8000, "b", discovered(Some("b"), &[], None));
    let started = statuses(&effects);
    assert_eq!(started.len(), 1);
    let summarized = |workspace: &Workspace| {
        let summary = workspace.summary();
        assert_eq!(summary.repositories.len(), 1);
        let returned = &summary.repositories[0];
        (returned.revision, returned.warm, returned.stale)
    };
    assert_eq!(summarized(&workspace), (1, true, true));
    workspace.status_ended(at(9000), &path("b"), started[0].1, Ok(snapshot(2)));
    assert_eq!(summarized(&workspace), (2, true, false));
}

#[test]
fn outside_repositories_need_consent_and_closed_ones_stay_out_until_it() {
    let mut workspace = Workspace::new();
    reroot(
        &mut workspace,
        0,
        "p/root",
        discovered(None, &["p/root/n"], Some("p")),
    );
    let summary = workspace.summary();
    assert_eq!(summary.candidates, [path("p")]);
    assert_eq!(summary.repositories.len(), 1);
    assert!(workspace.consentable(&path("p")));

    workspace.consent(at(1000), &path("p"));
    let summary = workspace.summary();
    assert!(summary.candidates.is_empty());
    assert_eq!(summary.repositories[1].kind, RepositoryKind::Parent);

    workspace.close(at(2000), &path("p"));
    workspace.close(at(2000), &path("p/root/n"));
    let summary = workspace.summary();
    assert!(summary.candidates.is_empty() && summary.repositories.is_empty());
    assert!(workspace.consentable(&path("p/root/n")));
    workspace.consent(at(3000), &path("p/root/n"));
    assert!(workspace.displays(&path("p/root/n")));
}

#[test]
fn watches_and_refreshes_wait_for_a_subscriber_and_writes_supersede_status() {
    let mut workspace = Workspace::new();
    let effects = reroot(&mut workspace, 0, "a", discovered(Some("a"), &[], None));
    assert!(statuses(&effects).is_empty());
    assert!(
        !effects
            .iter()
            .any(|effect| matches!(effect, Effect::Watch { .. }))
    );

    let effects = workspace.subscribe(at(1000), 1, client());
    assert_eq!(
        effects[0],
        Effect::Watch {
            repository: path("a")
        }
    );
    let started = statuses(&effects);
    assert_eq!(started.len(), 1);

    // A write during the status cancels it and refreshes once the write ends.
    assert!(workspace.summary().repositories[0].stale);
    assert!(workspace.write_queued(&path("a")));
    let effects = workspace.write_ended(at(1100), &path("a"));
    assert!(effects.contains(&Effect::CancelStatus {
        repository: path("a"),
        generation: started[0].1,
    }));
    let again = statuses(&effects);
    assert_eq!(again.len(), 1);
    assert!(
        workspace
            .status_ended(at(1200), &path("a"), started[0].1, Ok(snapshot(5)))
            .is_empty()
    );
    assert_eq!(workspace.summary().repositories[0].revision, 0);
    workspace.status_ended(at(1300), &path("a"), again[0].1, Ok(snapshot(5)));
    assert_eq!(workspace.summary().repositories[0].revision, 1);

    let effects = workspace.unsubscribe(at(1400), 1);
    assert_eq!(
        effects,
        [Effect::Unwatch {
            repository: path("a")
        }]
    );
    assert!(
        workspace
            .changed(at(1500), &path("a"), Change::Content)
            .is_empty()
    );
    assert_eq!(workspace.next_wake(), None);
}

#[test]
fn status_pages_follow_one_revision_within_their_bounds() {
    let mut workspace = Workspace::new();
    workspace.subscribe(at(0), 1, client());
    let effects = reroot(&mut workspace, 0, "a", discovered(Some("a"), &[], None));
    let started = statuses(&effects);
    workspace.status_ended(
        at(600),
        &path("a"),
        started[0].1,
        Ok(snapshot(PAGE_ENTRIES + 1)),
    );

    let mut cursor = 0;
    let mut paths = Vec::new();
    loop {
        let page = workspace.page(&path("a"), 1, cursor).unwrap();
        assert!(!page.entries.is_empty() && page.entries.len() <= PAGE_ENTRIES);
        let bytes = page
            .entries
            .iter()
            .map(|entry| ENTRY_OVERHEAD + escaped_len(&entry.path))
            .sum::<usize>();
        assert!(bytes <= PAGE_BYTES);
        paths.extend(page.entries.into_iter().map(|entry| entry.path));
        match page.cursor {
            Some(next) => cursor = next,
            None => break,
        }
    }
    let expected = (0..=PAGE_ENTRIES)
        .map(|index| native_paths::join(&path("a"), &format!("file {index}")))
        .collect::<Vec<_>>();
    assert_eq!(paths, expected);
    assert_eq!(
        workspace.page(&path("a"), 0, 0).unwrap_err().code,
        ErrorCode::StaleRevision
    );
    assert_eq!(
        workspace.page(&path("b"), 1, 0).unwrap_err().code,
        ErrorCode::NotFound
    );
}
