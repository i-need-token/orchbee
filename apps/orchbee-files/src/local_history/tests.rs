//! The store keeps VS Code's Local History rules, its own total, and one lock that every Session
//! sharing the data root waits for.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

use orchbee_local_runtime::DataRoot;

use super::store::{Entry, Limits, Source, Store, StoreError, create_folder};
use crate::scope::path::NativePath;

/// A private folder for one test, removed when the test ends.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        // The store admits only private folders, reached without a link on the way.
        let base = orchbee_git_process::canonical_path(&std::env::temp_dir()).unwrap();
        let path = Path::new(base.as_native()).join(format!(
            "orchbee-files-history-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        create_folder(&path).unwrap();
        Self(path)
    }

    /// The store of this folder's data root.
    fn store(&self, limits: Limits) -> Store {
        Store::new(self.root(), limits)
    }

    fn root(&self) -> PathBuf {
        DataRoot::at(self.0.join("data")).local_history()
    }

    /// A file path in this folder, which need not exist.
    fn file(&self, relative: &str) -> NativePath {
        let path = relative
            .split('/')
            .fold(self.0.join("work"), |path, part| path.join(part));
        NativePath::parse(path.to_str().unwrap()).unwrap()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn entries(store: &Store, path: &NativePath) -> Vec<Entry> {
    store
        .read(|hold| hold.entries(path))
        .unwrap()
        .unwrap_or_default()
}

fn record(store: &Store, path: &NativePath, source: Source, content: &[u8], timestamp: i64) {
    store
        .write(|hold| hold.record(path, source, content, timestamp))
        .unwrap();
}

fn content(store: &Store, path: &NativePath, id: &str) -> Vec<u8> {
    store.read(|hold| hold.content(path, id)).unwrap().unwrap()
}

/// The names in the folder holding `path`'s history.
fn copies(scratch: &Scratch, store: &Store, path: &NativePath) -> Vec<String> {
    let id = entries(store, path)[0].id.clone();
    let root = scratch.root();
    for folder in fs::read_dir(&root).unwrap() {
        let folder = folder.unwrap().path();
        if !folder.is_dir() {
            continue;
        }
        let names: Vec<String> = fs::read_dir(&folder)
            .unwrap()
            .map(|item| item.unwrap().file_name().into_string().unwrap())
            .collect();
        let record = fs::read_to_string(folder.join("entries.json")).unwrap_or_default();
        if record.contains(&id) {
            return names;
        }
    }
    panic!("no folder holds {id}");
}

const T: i64 = 1_800_000_000_000;

#[test]
fn records_keep_vs_code_defaults() {
    let scratch = Scratch::new();
    let store = scratch.store(Limits::DEFAULT);
    let file = scratch.file("a.ts");
    assert!(entries(&store, &file).is_empty());
    assert!(!scratch.root().exists(), "a read creates nothing");

    // A save within ten seconds of the newest entry from the same source replaces it.
    record(&store, &file, Source::Save, b"one", T);
    record(&store, &file, Source::Save, b"two", T + 10_000);
    let listed = entries(&store, &file);
    assert_eq!(listed.len(), 1);
    assert_eq!((listed[0].timestamp, listed[0].size), (T + 10_000, 3));
    assert_eq!(content(&store, &file, &listed[0].id), b"two");
    let first = listed[0].id.clone();

    // A later save, another source and a labeled entry each keep their own.
    record(&store, &file, Source::Save, b"three", T + 20_001);
    record(&store, &file, Source::Restore, b"four", T + 20_002);
    let restored = entries(&store, &file)[2].id.clone();
    store
        .write(|hold| hold.rename(&file, &restored, "checkpoint".to_owned()))
        .unwrap();
    record(&store, &file, Source::Restore, b"five", T + 20_003);
    let listed = entries(&store, &file);
    let sources: Vec<(Source, Option<&str>)> = listed
        .iter()
        .map(|entry| (entry.source, entry.label.as_deref()))
        .collect();
    assert_eq!(
        sources,
        [
            (Source::Save, None),
            (Source::Save, None),
            (Source::Restore, Some("checkpoint")),
            (Source::Restore, None),
        ]
    );
    assert!(matches!(
        store.write(|hold| hold.rename(&file, "missing", "x".to_owned())),
        Err(StoreError::NotFound)
    ));

    // A file over 256 KiB records nothing.
    let large = vec![b'x'; 256 * 1024 + 1];
    record(&store, &file, Source::Save, &large, T + 40_000);
    assert_eq!(entries(&store, &file).len(), 4);

    // Fifty entries stay, the oldest leaving first with their copies.
    for step in 0..50 {
        let text = format!("save {step}");
        record(
            &store,
            &file,
            Source::Save,
            text.as_bytes(),
            T + 60_000 * (step + 1),
        );
    }
    let listed = entries(&store, &file);
    assert_eq!(listed.len(), 50);
    assert!(listed.iter().all(|entry| entry.id != first));
    assert!(
        listed
            .windows(2)
            .all(|pair| pair[0].timestamp < pair[1].timestamp)
    );
    assert_eq!(content(&store, &file, &listed[49].id), b"save 49");
    let names = copies(&scratch, &store, &file);
    assert_eq!(names.len(), 51, "{names:?}");
    assert!(
        names
            .iter()
            .all(|name| name == "entries.json" || name.ends_with("Z.ts"))
    );

    // Deleting entries removes their copies; deleting the last removes the file's folder.
    let ids: Vec<String> = listed[..49].iter().map(|entry| entry.id.clone()).collect();
    store.write(|hold| hold.delete(&file, &ids)).unwrap();
    assert_eq!(copies(&scratch, &store, &file).len(), 2);
    store
        .write(|hold| hold.delete(&file, &[listed[49].id.clone()]))
        .unwrap();
    assert!(entries(&store, &file).is_empty());
    let folders = fs::read_dir(scratch.root())
        .unwrap()
        .filter(|item| item.as_ref().unwrap().path().is_dir())
        .count();
    assert_eq!(folders, 0);
}

#[test]
fn history_follows_moves_and_renames_and_merges_with_the_target() {
    let scratch = Scratch::new();
    let store = scratch.store(Limits::DEFAULT);
    let (x, y) = (scratch.file("a/x.ts"), scratch.file("a/sub/y.ts"));
    record(&store, &x, Source::Save, b"x saved", T);
    record(&store, &y, Source::Save, b"y saved", T);

    // A folder moves with every file below it; each records what it holds at its new path.
    let (moved_x, moved_y) = (scratch.file("c/x.ts"), scratch.file("c/sub/y.ts"));
    store
        .write(|hold| {
            hold.relocate(&scratch.file("a"), &scratch.file("c"), T + 1, &mut |path| {
                (*path == moved_x).then(|| b"x now".to_vec())
            })
        })
        .unwrap();
    assert!(entries(&store, &x).is_empty() && entries(&store, &y).is_empty());
    let listed = entries(&store, &moved_x);
    let sources: Vec<Source> = listed.iter().map(|entry| entry.source).collect();
    assert_eq!(sources, [Source::Save, Source::Move]);
    assert_eq!(content(&store, &moved_x, &listed[1].id), b"x now");
    // A file whose content cannot be read keeps its history and records nothing.
    assert_eq!(entries(&store, &moved_y).len(), 1);

    // Within its folder the file is renamed.
    let renamed = scratch.file("c/w.ts");
    store
        .write(|hold| hold.relocate(&moved_x, &renamed, T + 20_000, &mut |_| Some(b"w".to_vec())))
        .unwrap();
    let sources: Vec<Source> = entries(&store, &renamed)
        .iter()
        .map(|entry| entry.source)
        .collect();
    assert_eq!(sources, [Source::Save, Source::Move, Source::Rename]);

    // Onto a file with history of its own, both histories stay, in time order.
    let target = scratch.file("d/w.ts");
    record(&store, &target, Source::Save, b"d saved", T + 5);
    store
        .write(|hold| hold.relocate(&renamed, &target, T + 40_000, &mut |_| Some(b"d".to_vec())))
        .unwrap();
    let listed = entries(&store, &target);
    let timestamps: Vec<i64> = listed.iter().map(|entry| entry.timestamp).collect();
    assert_eq!(timestamps, [T, T + 1, T + 5, T + 20_000, T + 40_000]);
    assert_eq!(content(&store, &target, &listed[2].id), b"d saved");
    assert_eq!(content(&store, &target, &listed[0].id), b"x saved");
    assert!(entries(&store, &renamed).is_empty());
    let folders = fs::read_dir(scratch.root())
        .unwrap()
        .filter(|item| item.as_ref().unwrap().path().is_dir())
        .count();
    assert_eq!(folders, 2, "the merged folder is gone");
}

#[test]
fn the_total_removes_the_oldest_entries_of_any_file_first() {
    let scratch = Scratch::new();
    let store = scratch.store(Limits {
        total: 10,
        ..Limits::DEFAULT
    });
    let (first, second, third) = (
        scratch.file("1.txt"),
        scratch.file("2.txt"),
        scratch.file("3.txt"),
    );
    record(&store, &first, Source::Save, b"1111", T);
    record(&store, &second, Source::Save, b"2222", T + 1);
    record(&store, &third, Source::Save, b"3333", T + 2);
    assert!(entries(&store, &first).is_empty());
    assert_eq!(entries(&store, &second).len(), 1);

    record(&store, &second, Source::Save, b"2-2-", T + 60_000);
    let listed = entries(&store, &second);
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].timestamp, T + 60_000);
    assert_eq!(entries(&store, &third).len(), 1);
}

#[test]
fn the_index_is_rebuilt_from_the_folders_and_a_newer_record_is_left_alone() {
    let scratch = Scratch::new();
    let store = scratch.store(Limits::DEFAULT);
    let (a, b) = (scratch.file("a.ts"), scratch.file("b.ts"));
    record(&store, &a, Source::Save, b"a", T);
    record(&store, &b, Source::Save, b"b", T);
    let index = scratch.root().join("index.json");

    // A missing or unreadable index is rebuilt; a read keeps the rebuild in memory.
    fs::remove_file(&index).unwrap();
    assert_eq!(entries(&store, &a).len(), 1);
    assert!(!index.exists());
    fs::write(&index, b"{not json").unwrap();
    assert_eq!(entries(&store, &b).len(), 1);
    record(&store, &a, Source::Save, b"a again", T + 60_000);
    assert_eq!(entries(&store, &a).len(), 2);
    assert!(
        fs::read_to_string(&index)
            .unwrap()
            .contains("\"version\":1")
    );

    // A folder a newer version wrote stays out of the index and in place.
    let newer = scratch.root().join("000009");
    fs::create_dir(&newer).unwrap();
    let foreign = format!(
        "{{\"version\":2,\"source\":{}}}",
        serde_json::to_string(scratch.file("c.ts").as_str()).unwrap()
    );
    fs::write(newer.join("entries.json"), &foreign).unwrap();
    fs::remove_file(&index).unwrap();
    assert!(entries(&store, &scratch.file("c.ts")).is_empty());
    store.write(|hold| hold.delete_all()).unwrap();
    assert!(entries(&store, &a).is_empty() && entries(&store, &b).is_empty());
    assert_eq!(
        fs::read_to_string(newer.join("entries.json")).unwrap(),
        foreign
    );

    // A newer index leaves the store to that version.
    let written = br#"{"version":2,"folders":[]}"#;
    fs::write(&index, written).unwrap();
    assert!(matches!(
        store.read(|hold| hold.entries(&a)),
        Err(StoreError::Newer)
    ));
    assert!(matches!(
        store.write(|hold| hold.record(&a, Source::Save, b"a", T)),
        Err(StoreError::Newer)
    ));
    assert_eq!(fs::read(&index).unwrap(), written);
}

#[test]
fn two_sessions_wait_for_one_lock() {
    let scratch = Scratch::new();
    // Two processes of two Sessions open the same data root.
    let (first, second) = (
        scratch.store(Limits::DEFAULT),
        scratch.store(Limits::DEFAULT),
    );
    let (a, b) = (scratch.file("a.ts"), scratch.file("b.ts"));

    // A hold the other Session keeps past half a second refuses the change.
    first
        .write(|hold| {
            hold.record(&a, Source::Save, b"a", T)?;
            thread::scope(|scope| {
                let waited = scope.spawn(|| {
                    let started = Instant::now();
                    let outcome = second.write(|hold| hold.record(&b, Source::Save, b"b", T));
                    (outcome, started.elapsed())
                });
                let (outcome, elapsed) = waited.join().unwrap();
                assert!(matches!(outcome, Err(StoreError::Busy)));
                assert!(elapsed >= Duration::from_millis(500));
            });
            Ok(())
        })
        .unwrap();

    // A shorter hold is waited for, and each Session reads what the other recorded.
    let (holding, held) = std::sync::mpsc::channel();
    thread::scope(|scope| {
        let holder = &first;
        scope.spawn(move || {
            holder
                .write(|_| {
                    holding.send(()).unwrap();
                    thread::sleep(Duration::from_millis(50));
                    Ok(())
                })
                .unwrap();
        });
        held.recv().unwrap();
        record(&second, &b, Source::Save, b"b", T);
    });
    assert_eq!(entries(&second, &a).len(), 1);
    assert_eq!(entries(&first, &b).len(), 1);
}
