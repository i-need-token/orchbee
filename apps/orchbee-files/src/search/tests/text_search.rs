//! Text searches against a scripted rg: what reaches `search.next` from rg's output, the second
//! run for exclude exceptions, the run again without following links, the glob recheck, how rg's
//! failures fail the search, and how a cancelled search ends rg and gives its slot back.

use std::{
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use orchbee_files_protocol::{ErrorCode, FilesError, SearchNextResult};

use super::{
    super::{
        preview::PreviewOptions,
        queue::{Queue, Taken},
        rg::Ended,
        slots::Slots,
        text,
    },
    script::{Run, Script, WAIT, folder, matched, relative},
};
use crate::{fs::tests::Scratch, scope::path::NativePath};

fn request(folder: &NativePath) -> text::Request {
    text::Request {
        pattern: "foo".to_owned(),
        regex: false,
        case_sensitive: false,
        whole_word: false,
        multiline: false,
        encoding: None,
        context: 0,
        folders: vec![text::Folder {
            path: folder.clone(),
            includes: Vec::new(),
        }],
        includes: Vec::new(),
        excludes: Vec::new(),
        use_ignore_files: true,
        use_parent_ignore_files: false,
        follow_symlinks: false,
        max_results: 10_000,
        preview: PreviewOptions {
            match_lines: 1,
            chars_per_line: 1000,
        },
    }
}

/// Every batch of `queue` until its search ends, and how it ended.
fn drain(queue: &Queue) -> (Vec<SearchNextResult>, Result<(), FilesError>) {
    let mut batches = Vec::new();
    loop {
        match queue.take(Instant::now() + WAIT, || false) {
            Taken::Batch(batch) => {
                let done = batch.done;
                batches.push(batch);
                if done {
                    return (batches, Ok(()));
                }
            }
            Taken::Failed(error) => return (batches, Err(error)),
            Taken::Concurrent | Taken::Abandoned => panic!("the only take was abandoned"),
        }
    }
}

/// Each match in order: its file below `folder`, its 0-based line and how many ranges it holds.
fn matches(batches: &[SearchNextResult], folder: &NativePath) -> Vec<(String, u64, usize)> {
    batches
        .iter()
        .flat_map(|batch| &batch.files)
        .flat_map(|file| {
            file.matches.iter().map(|found| {
                let ranges = found.ranges.as_slice();
                (
                    relative(file.path.as_str(), folder),
                    ranges[0].source[0].get(),
                    ranges.len(),
                )
            })
        })
        .collect()
}

fn run(
    request: &text::Request,
    script: &Script,
) -> (Vec<SearchNextResult>, Result<(), FilesError>) {
    let queue = Queue::new();
    text::run(request, script, &Slots::new(), &queue);
    drain(&queue)
}

#[test]
fn results_join_their_folder_and_stop_rg_at_the_limit() {
    let scratch = Scratch::new("search-limit");
    let folder = folder(scratch.path());
    let script = Script::new([Run::new()
        .stdout(matched("a.txt", 3, "foo and foo", &[(0, 3), (8, 11)]))
        .stdout(matched("b.txt", 1, "foo", &[(0, 3)]))
        .hold()]);
    let mut request = request(&folder);
    request.max_results = 2;
    let (batches, ended) = run(&request, &script);
    assert!(ended.is_ok());
    assert_eq!(matches(&batches, &folder), [("a.txt".to_owned(), 2, 2)]);
    let path = batches[0].files[0].path.as_str();
    assert_eq!(path, scratch.path().join("a.txt").to_str().unwrap());
    assert!(batches.last().unwrap().limit_hit);
    assert_eq!(script.seen()[0].ended, Ended::Stopped);
}

#[test]
fn exceptions_add_what_they_reinclude_up_to_the_search_limit() {
    let scratch = Scratch::new("search-exceptions");
    let folder = folder(scratch.path());
    let script = Script::new([
        Run::new().stdout(matched("a.txt", 1, "foo", &[(0, 3)])),
        Run::new()
            .stdout(matched("a.txt", 1, "foo", &[(0, 3)]))
            .stdout(matched("gen/c.txt", 1, "foo", &[(0, 3)]))
            .stdout(matched("gen/keep/b.txt", 1, "foo foo", &[(0, 3), (4, 7)]))
            .hold(),
    ]);
    let mut request = request(&folder);
    request.excludes = vec!["**/gen/**".to_owned(), "!**/gen/keep/**".to_owned()];
    request.max_results = 2;
    let (batches, ended) = run(&request, &script);
    assert!(ended.is_ok());
    // The second run lists everything; the search's own limit cuts the last match to it.
    assert_eq!(
        matches(&batches, &folder),
        [
            ("a.txt".to_owned(), 0, 1),
            ("gen/keep/b.txt".to_owned(), 0, 1)
        ]
    );
    assert!(batches.last().unwrap().limit_hit);
    let seen = script.seen();
    assert!(!seen[0].names("gen/keep") && seen[1].names("gen/keep"));
    assert_eq!(seen[1].ended, Ended::Stopped);
}

#[test]
fn too_many_link_loops_search_the_folder_again_without_following_links() {
    let scratch = Scratch::new("search-loops");
    let folder = folder(scratch.path());
    let loops = "rg: File system loop found: a/b points to an ancestor a\n".repeat(1001);
    let script = Script::new([
        Run::new()
            .stdout(matched("a.txt", 1, "foo", &[(0, 3)]))
            .stderr(loops)
            .hold(),
        Run::new()
            .stdout(matched("a.txt", 1, "foo", &[(0, 3)]))
            .stdout(matched("a.txt", 2, "foo", &[(0, 3)]))
            .stdout(matched("b.txt", 1, "foo", &[(0, 3)])),
    ]);
    let mut request = request(&folder);
    request.follow_symlinks = true;
    let (batches, ended) = run(&request, &script);
    assert!(ended.is_ok());
    // The run again skips as many of a file's results as the first run reported.
    assert_eq!(
        matches(&batches, &folder),
        [
            ("a.txt".to_owned(), 0, 1),
            ("a.txt".to_owned(), 1, 1),
            ("b.txt".to_owned(), 0, 1),
        ]
    );
    let loops: Vec<&str> = batches
        .iter()
        .flat_map(|batch| &batch.symlink_loops)
        .map(|path| path.as_str())
        .collect();
    assert_eq!(loops, [folder.as_str()]);
    let seen = script.seen();
    assert_eq!(seen[0].ended, Ended::Stopped);
    assert!(seen[0].names("--follow") && !seen[1].names("--follow"));
}

#[test]
fn results_outside_the_globs_are_dropped() {
    let scratch = Scratch::new("search-globs");
    let folder = folder(scratch.path());
    let script = Script::new([Run::new()
        .stdout(matched("a.ts", 1, "foo", &[(0, 3)]))
        .stdout(matched("b.js", 1, "foo", &[(0, 3)]))
        .stdout(matched("skip/c.ts", 1, "foo", &[(0, 3)]))
        .stdout(matched("d/e.ts", 1, "foo", &[(0, 3)]))]);
    let mut request = request(&folder);
    request.includes = vec!["**/*.ts".to_owned()];
    request.excludes = vec!["**/skip/**".to_owned()];
    let (batches, ended) = run(&request, &script);
    assert!(ended.is_ok());
    assert_eq!(
        matches(&batches, &folder),
        [("a.ts".to_owned(), 0, 1), ("d/e.ts".to_owned(), 0, 1)]
    );
}

#[test]
fn rg_failures_fail_the_search() {
    let scratch = Scratch::new("search-failures");
    let folder = folder(scratch.path());
    let failed = |request: &text::Request, run: Run| {
        let script = Script::new([run]);
        let (batches, ended) = self::run(request, &script);
        assert!(batches.iter().all(|batch| batch.files.is_empty()));
        (ended.unwrap_err(), script.seen())
    };
    let request = request(&folder);

    let parse = "regex parse error:\n    (?<\n    ^\nerror: unclosed group\n";
    let (error, _) = failed(&request, Run::new().stderr(parse));
    assert_eq!(error.code, ErrorCode::InvalidRegex);
    assert_eq!(error.detail.unwrap().as_str(), "Regex parse error");

    let (error, _) = failed(&request, Run::new().ending(Ended::Unavailable));
    assert_eq!(error.code, ErrorCode::Unavailable);
    assert_eq!(error.reason.unwrap().as_str(), "search-binary");

    let (error, seen) = failed(&request, Run::new().stdout("not json\n").hold());
    assert_eq!(error.code, ErrorCode::Failed);
    assert_eq!(seen[0].ended, Ended::Stopped);

    // A whole-word expression JavaScript rejects never reaches rg.
    let mut rejected = request;
    rejected.regex = true;
    rejected.whole_word = true;
    rejected.pattern = "(".to_owned();
    let (error, seen) = failed(&rejected, Run::new());
    assert_eq!(error.code, ErrorCode::InvalidRegex);
    assert!(seen.is_empty());
}

#[test]
fn a_cancelled_search_ends_rg_and_gives_its_slot_back() {
    let scratch = Scratch::new("search-cancel");
    let request = Arc::new(request(&folder(scratch.path())));
    let script = Arc::new(Script::standing(Run::new().hold()));
    let slots = Arc::new(Slots::new());
    let queue = Arc::new(Queue::new());
    let worker = {
        let (request, script, slots, queue) = (
            Arc::clone(&request),
            Arc::clone(&script),
            Arc::clone(&slots),
            Arc::clone(&queue),
        );
        thread::spawn(move || text::run(&request, script.as_ref(), &slots, &queue))
    };
    script.wait_for(|script| script.running().len() == 1);
    queue.cancel();
    worker.join().unwrap();
    assert_eq!(script.seen()[0].ended, Ended::Stopped);
    let (batches, ended) = drain(&queue);
    assert!(batches.is_empty());
    assert_eq!(ended.unwrap_err().code, ErrorCode::Cancelled);
    let deadline = Instant::now() + WAIT;
    let free: Vec<_> = (0..4)
        .map(|_| slots.acquire(|| Instant::now() > deadline))
        .collect();
    assert!(free.iter().all(Option::is_some));
}

#[test]
fn at_most_four_rg_run_at_once_and_a_waiting_one_starts_when_a_slot_frees() {
    let scratch = Scratch::new("search-slots");
    let folder = folder(scratch.path());
    let script = Arc::new(Script::standing(Run::new().hold()));
    let slots = Arc::new(Slots::new());
    let searches: Vec<(String, Arc<Queue>)> = (0..5)
        .map(|index| (format!("pattern{index}"), Arc::new(Queue::new())))
        .collect();
    let workers: Vec<_> = searches
        .iter()
        .map(|(pattern, queue)| {
            let mut request = request(&folder);
            request.pattern.clone_from(pattern);
            let (script, slots, queue) =
                (Arc::clone(&script), Arc::clone(&slots), Arc::clone(queue));
            thread::spawn(move || text::run(&request, script.as_ref(), &slots, &queue))
        })
        .collect();
    script.wait_for(|script| script.running().len() == 4);
    thread::sleep(Duration::from_millis(50));
    assert_eq!((script.running().len(), script.seen().len()), (4, 0));
    let started = |pattern: &str| {
        script
            .running()
            .iter()
            .any(|arguments| arguments.iter().any(|argument| argument.contains(pattern)))
    };
    let (waiting, _) = searches
        .iter()
        .find(|(pattern, _)| !started(pattern))
        .unwrap();
    let (_, running) = searches
        .iter()
        .find(|(pattern, _)| started(pattern))
        .unwrap();
    running.cancel();
    script.wait_for(|_| started(waiting));
    assert_eq!(script.running().len(), 4);
    for (_, queue) in &searches {
        queue.cancel();
    }
    for worker in workers {
        worker.join().unwrap();
    }
    assert_eq!(script.peak(), 4);
}
