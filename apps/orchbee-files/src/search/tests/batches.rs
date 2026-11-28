//! A search's queue between rg and `search.next`: batches that keep rg's order, group a file's
//! consecutive results and each fit one Result record; a full queue holding rg; cancellation;
//! one take at a time; and a failure reported after the results before it.

use std::{
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};

use orchbee_files_protocol::{
    ErrorCode, FilePath, FilesError, List, MAXIMUM_REPLY_BYTES, SafeInteger, SearchContextLine,
    SearchMatch, SearchNextResult, SearchRange,
};

use super::{
    super::queue::{Item, Queue, Taken},
    script::WAIT,
};
use crate::seeded::{self, SplitMix64};

const DEFAULT_SEED: u64 = 0x5eed_0b47_c4e5;

fn path(name: &str) -> FilePath {
    let root = if cfg!(windows) { r"C:\w\" } else { "/w/" };
    FilePath::new(format!("{root}{name}")).unwrap()
}

fn safe(value: usize) -> SafeInteger {
    SafeInteger::new(value as u64).unwrap()
}

fn range(at: usize) -> SearchRange {
    SearchRange {
        source: [safe(at), safe(0), safe(at), safe(3)],
        preview: [safe(0), safe(0), safe(0), safe(3)],
    }
}

fn found(preview: String, ranges: usize) -> SearchMatch {
    SearchMatch {
        ranges: List::new((0..ranges).map(range).collect()).unwrap(),
        preview,
    }
}

fn context(text: String) -> Item {
    Item::context(
        path("a.txt"),
        SearchContextLine {
            line: safe(0),
            text,
        },
    )
}

fn take(queue: &Queue) -> SearchNextResult {
    match queue.take(Instant::now() + WAIT, || false) {
        Taken::Batch(batch) => batch,
        Taken::Failed(error) => panic!("the search failed with {:?}", error.code),
        Taken::Concurrent | Taken::Abandoned => panic!("the take did not answer a batch"),
    }
}

fn failed(taken: Taken) -> FilesError {
    match taken {
        Taken::Failed(error) => error,
        _ => panic!("the take did not fail"),
    }
}

/// A pushed result: its file and what it should come out as, or nothing when it is too long.
struct Pushed {
    file: &'static str,
    out: Option<Out>,
}

/// One result as batches carry it: a match by its preview and its ranges' lines, all its parts
/// joined, or a context line by its text.
#[derive(Debug, Eq, PartialEq)]
enum Out {
    Match(&'static str, String, Vec<usize>),
    Context(&'static str, String),
}

/// The pushes of one round, and the items they push.
fn pushes(random: &mut SplitMix64) -> (Vec<Pushed>, Vec<Item>) {
    let (mut pushed, mut items) = (Vec::new(), Vec::new());
    for index in 0..random.below(60) + 1 {
        let file = ["a.txt", "b.txt", "c.txt"][random.below(3)];
        // Short, long, and too long for one record.
        let length = [10, 4000, 60_000, 140_000][random.below(4)];
        let text = format!("{index}:{}", "x".repeat(length));
        let fits = length < MAXIMUM_REPLY_BYTES;
        let out = if random.below(3) == 0 {
            let line = SearchContextLine {
                line: safe(index),
                text: text.clone(),
            };
            items.push(Item::context(path(file), line));
            fits.then_some(Out::Context(file, text))
        } else {
            // Up to 20,000 ranges, more than one record holds.
            let ranges = [1, 3, 20_000][random.below(3)];
            items.push(Item::matched(path(file), found(text.clone(), ranges)));
            fits.then(|| Out::Match(file, text, (0..ranges).collect()))
        };
        pushed.push(Pushed { file, out });
    }
    (pushed, items)
}

fn name(path: &FilePath) -> &'static str {
    let name = path.as_str().rsplit(['/', '\\']).next().unwrap();
    ["a.txt", "b.txt", "c.txt"]
        .into_iter()
        .find(|known| *known == name)
        .unwrap()
}

/// `files` with each run of one file as one.
fn runs(files: impl IntoIterator<Item = &'static str>) -> Vec<&'static str> {
    let mut runs: Vec<&'static str> = Vec::new();
    for file in files {
        if runs.last() != Some(&file) {
            runs.push(file);
        }
    }
    runs
}

#[test]
fn batches_keep_order_group_a_files_results_and_each_fit_a_record() {
    let seed = seeded::seed("ORCHBEE_FILES_SEARCH_BATCHES_SEED", DEFAULT_SEED);
    let mut random = SplitMix64::new(seed);
    for round in 0..8 {
        let context = format!("round {round}, seed {seed:#x}");
        let (pushed, items) = pushes(&mut random);
        let queue = Arc::new(Queue::new());
        let producer = {
            let queue = Arc::clone(&queue);
            thread::spawn(move || {
                for item in items {
                    queue.push(item);
                }
                queue.finish(Ok(false));
            })
        };
        let (mut entries, mut matches, mut lines) = (Vec::new(), Vec::new(), Vec::new());
        let last = loop {
            let batch = take(&queue);
            let bytes = serde_json::to_vec(&batch).unwrap().len();
            assert!(bytes <= MAXIMUM_REPLY_BYTES, "{context}");
            for pair in batch.files.windows(2) {
                assert_ne!(pair[0].path, pair[1].path, "{context}");
            }
            for file in &batch.files {
                let name = name(&file.path);
                entries.push(name);
                for found in &file.matches {
                    let at = found.ranges.as_slice().iter();
                    let at: Vec<usize> = at.map(|range| range.source[0].get() as usize).collect();
                    // The parts of one match follow each other with its preview.
                    if let Some(Out::Match(file, preview, joined)) = matches.last_mut()
                        && *file == name
                        && *preview == found.preview
                    {
                        joined.extend(at);
                    } else {
                        matches.push(Out::Match(name, found.preview.clone(), at));
                    }
                }
                for line in &file.context {
                    lines.push(Out::Context(name, line.text.clone()));
                }
            }
            if batch.done {
                break batch;
            }
        };
        producer.join().unwrap();
        // A file's entry holds its consecutive results, its matches and its context lines each
        // in order.
        let kept: Vec<&Pushed> = pushed.iter().filter(|push| push.out.is_some()).collect();
        assert_eq!(
            runs(entries),
            runs(kept.iter().map(|push| push.file)),
            "{context}"
        );
        let (expected_matches, expected_lines): (Vec<&Out>, Vec<&Out>) = kept
            .iter()
            .filter_map(|push| push.out.as_ref())
            .partition(|out| matches!(out, Out::Match(..)));
        assert!(matches.iter().eq(expected_matches), "{context}");
        assert!(lines.iter().eq(expected_lines), "{context}");
        let omitted = pushed.len() - kept.len();
        assert_eq!(last.omitted_long_lines.get(), omitted as u64, "{context}");
    }
}

#[test]
fn a_full_queue_holds_rg_until_a_batch_is_taken() {
    let queue = Arc::new(Queue::new());
    let (pushed, acknowledged) = mpsc::channel();
    let producer = {
        let queue = Arc::clone(&queue);
        thread::spawn(move || {
            // Each line about 100 KB: 21 of them pass 2 MiB.
            for index in 0..22 {
                queue.push(context(format!("{index}:{}", "x".repeat(100_000))));
                pushed.send(index).unwrap();
            }
        })
    };
    for index in 0..21 {
        assert_eq!(acknowledged.recv_timeout(WAIT).unwrap(), index);
    }
    assert!(
        acknowledged
            .recv_timeout(Duration::from_millis(100))
            .is_err()
    );
    assert_eq!(take(&queue).files.len(), 1);
    assert_eq!(acknowledged.recv_timeout(WAIT).unwrap(), 21);
    producer.join().unwrap();
}

#[test]
fn cancelling_drops_what_waits_and_frees_rg_and_a_waiting_take() {
    let full = Arc::new(Queue::new());
    let (pushed, acknowledged) = mpsc::channel();
    let producer = {
        let queue = Arc::clone(&full);
        thread::spawn(move || {
            for index in 0..22 {
                queue.push(context("x".repeat(100_000)));
                pushed.send(index).unwrap();
            }
        })
    };
    for index in 0..21 {
        assert_eq!(acknowledged.recv_timeout(WAIT).unwrap(), index);
    }
    assert!(
        acknowledged
            .recv_timeout(Duration::from_millis(100))
            .is_err()
    );
    full.cancel();
    assert_eq!(acknowledged.recv_timeout(WAIT).unwrap(), 21);
    producer.join().unwrap();
    let error = failed(full.take(Instant::now() + WAIT, || false));
    assert_eq!(error.code, ErrorCode::Cancelled);

    let empty = Arc::new(Queue::new());
    let (entered, waiting) = mpsc::channel();
    let taker = {
        let queue = Arc::clone(&empty);
        thread::spawn(move || {
            let taken = queue.take(Instant::now() + WAIT, move || {
                let _ = entered.send(());
                false
            });
            failed(taken).code
        })
    };
    waiting.recv_timeout(WAIT).unwrap();
    empty.cancel();
    assert_eq!(taker.join().unwrap(), ErrorCode::Cancelled);
}

#[test]
fn one_take_waits_at_a_time_and_an_abandoned_take_takes_nothing() {
    let queue = Arc::new(Queue::new());
    // At its deadline a take answers what there is.
    let early = match queue.take(Instant::now(), || false) {
        Taken::Batch(batch) => batch,
        _ => panic!("the take did not answer a batch"),
    };
    assert!(early.files.is_empty() && !early.done);

    let (entered, waiting) = mpsc::channel();
    let first = {
        let queue = Arc::clone(&queue);
        thread::spawn(move || {
            let taken = queue.take(Instant::now() + WAIT, move || {
                let _ = entered.send(());
                false
            });
            match taken {
                Taken::Batch(batch) => batch.files.len(),
                _ => panic!("the waiting take did not answer a batch"),
            }
        })
    };
    waiting.recv_timeout(WAIT).unwrap();
    assert!(matches!(
        queue.take(Instant::now() + WAIT, || false),
        Taken::Concurrent
    ));
    queue.push(context("first".to_owned()));
    assert_eq!(first.join().unwrap(), 1);

    queue.push(context("kept".to_owned()));
    assert!(matches!(
        queue.take(Instant::now() + WAIT, || true),
        Taken::Abandoned
    ));
    assert_eq!(take(&queue).files[0].context[0].text, "kept");
}

#[test]
fn a_failure_follows_the_results_before_it_and_loops_are_reported_once() {
    let queue = Queue::new();
    queue.symlink_loops(path("linked"));
    queue.push(context("before".to_owned()));
    queue.finish(Err(FilesError::new(ErrorCode::Failed)));
    let batch = take(&queue);
    assert_eq!(batch.symlink_loops, [path("linked")]);
    assert_eq!(batch.files[0].context[0].text, "before");
    assert!(!batch.done);
    let error = failed(queue.take(Instant::now() + WAIT, || false));
    assert_eq!(error.code, ErrorCode::Failed);

    let limited = Queue::new();
    limited.finish(Ok(true));
    let last = take(&limited);
    assert!(last.done && last.limit_hit && last.symlink_loops.is_empty());
}
