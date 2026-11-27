//! Coalescing against VS Code's own results, and against the state the raw events leave behind.

use std::collections::BTreeMap;

use serde::Deserialize;

use super::*;

#[derive(Deserialize)]
struct Corpus {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Case {
    case_sensitive: bool,
    separator: char,
    events: Vec<(String, String)>,
    coalesced: Vec<(String, String)>,
}

fn kind(name: &str) -> ChangeKind {
    match name {
        "added" => ChangeKind::Added,
        "changed" => ChangeKind::Changed,
        "deleted" => ChangeKind::Deleted,
        other => panic!("unknown kind {other}"),
    }
}

#[test]
fn coalescing_matches_vs_code() {
    let corpus: Corpus = serde_json::from_str(include_str!("corpus.json")).unwrap();
    for case in corpus.cases {
        let paths = Paths {
            case_sensitive: case.case_sensitive,
            separator: case.separator,
        };
        let events = case.events.iter().map(|(name, path)| Event {
            kind: kind(name),
            path: path.clone(),
            directory: false,
        });
        let coalesced: Vec<(ChangeKind, String)> = coalesce(events, paths)
            .into_iter()
            .map(|event| (event.kind, event.path))
            .collect();
        let expected: Vec<(ChangeKind, String)> = case
            .coalesced
            .iter()
            .map(|(name, path)| (kind(name), path.clone()))
            .collect();
        assert_eq!(coalesced, expected, "raw events {:?}", case.events);
    }
}

/// A small deterministic generator, so a failure reproduces.
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, bound: usize) -> usize {
        usize::try_from(self.next() % bound as u64).unwrap()
    }
}

const ROOT: &str = "/w";

/// Every path the generated trees may hold: up to three levels of `a`, `b` and `c` below the root.
fn universe() -> Vec<String> {
    let mut paths = vec![ROOT.to_owned()];
    let mut level = vec![ROOT.to_owned()];
    for _ in 0..3 {
        level = level
            .iter()
            .flat_map(|parent| ["a", "b", "c"].map(|name| format!("{parent}/{name}")))
            .collect();
        paths.extend(level.iter().cloned());
    }
    paths
}

fn parent(path: &str) -> &str {
    &path[..path.rfind('/').unwrap()]
}

fn within(path: &str, folder: &str) -> bool {
    path == folder || path.starts_with(&format!("{folder}/"))
}

/// One step on a model tree, where each path maps to whether it is a directory, and the complete
/// raw events it produces: a removed directory reports its descendants first, as a recursive
/// delete removes them.
fn step(
    tree: &mut BTreeMap<String, bool>,
    universe: &[String],
    random: &mut Random,
) -> Vec<Event<String>> {
    let path = universe[1 + random.below(universe.len() - 1)].clone();
    let event = |kind, path: &str, directory| Event {
        kind,
        path: path.to_owned(),
        directory,
    };
    match tree.get(&path).copied() {
        None if tree.get(parent(&path)) == Some(&true) => {
            let directory = random.below(2) == 0;
            tree.insert(path.clone(), directory);
            vec![event(ChangeKind::Added, &path, directory)]
        }
        None => Vec::new(),
        Some(directory) if random.below(3) == 0 => {
            vec![event(ChangeKind::Changed, &path, directory)]
        }
        Some(_) => {
            let mut removed: Vec<(String, bool)> = tree
                .iter()
                .filter(|(item, _)| within(item, &path))
                .map(|(item, directory)| (item.clone(), *directory))
                .collect();
            // Deepest first, so each directory empties before it goes.
            removed.sort_by_key(|(item, _)| std::cmp::Reverse(item.len()));
            removed
                .into_iter()
                .map(|(item, directory)| {
                    tree.remove(&item);
                    event(ChangeKind::Deleted, &item, directory)
                })
                .collect()
        }
    }
}

/// The tree a Client holds after applying `events` to `tree`: a deleted folder takes its
/// descendants with it, and an added or changed item exists.
fn apply(mut tree: BTreeMap<String, bool>, events: &[Event<String>]) -> Vec<String> {
    for event in events {
        if event.kind == ChangeKind::Deleted {
            tree.retain(|item, _| !within(item, &event.path));
        } else {
            tree.insert(event.path.clone(), event.directory);
        }
    }
    tree.into_keys().collect()
}

#[test]
fn coalesced_events_end_in_the_raw_events_state() {
    let universe = universe();
    let mut random = Random(0x0BEE_5EED_F11E_5A11);
    for paths in [
        Paths {
            case_sensitive: true,
            separator: '/',
        },
        Paths {
            case_sensitive: false,
            separator: '/',
        },
    ] {
        for _ in 0..2_000 {
            let mut tree = BTreeMap::from([(ROOT.to_owned(), true)]);
            for _ in 0..random.below(12) {
                step(&mut tree, &universe, &mut random);
            }
            let before = tree.clone();
            let raw: Vec<Event<String>> = (0..random.below(16))
                .flat_map(|_| step(&mut tree, &universe, &mut random))
                .collect();
            let coalesced = coalesce(raw.clone(), paths);
            assert_eq!(
                apply(before, &coalesced),
                tree.into_keys().collect::<Vec<_>>(),
                "raw {raw:?} coalesced {coalesced:?}"
            );
        }
    }
}
