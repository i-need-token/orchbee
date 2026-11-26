//! The search lane against real directories, through its own command and report lines: what a
//! search finds and how it shows it depends on ripgrep's walker, matcher and searcher together,
//! and on the UTF-16 arithmetic behind each preview, which no single function shows alone.
use super::{SearchCommand, SearchReport, serve_search};
use orchbee_workbench_protocol::{
    MAXIMUM_WORKSPACE_FILE_LIST_PAGE, MAXIMUM_WORKSPACE_SEARCH_MATCHES, WorkspaceFileListQuery,
    WorkspaceFilesOperation, WorkspaceFilesOutcome, WorkspaceSearchFile, WorkspaceSearchQuery,
    WorkspaceSearchState,
};
use std::io::{BufReader, Read, Write};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

/// A temporary directory removed on drop.
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "orchbee-workspace-files-search-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _removed = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(std::fs::canonicalize(path).unwrap())
    }
    fn write(&self, relative: &str, bytes: impl AsRef<[u8]>) {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    fn git_init(&self) {
        let status = Command::new("git")
            .args(["init", "-q"])
            .current_dir(&self.0)
            .status()
            .unwrap();
        assert!(status.success());
    }
    fn root(&self) -> String {
        self.0.to_str().unwrap().to_owned()
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _removed = std::fs::remove_dir_all(&self.0);
    }
}

/// Input that stays open until its sender drops, as the Server's pipe does.
struct Pipe(Receiver<Vec<u8>>, Vec<u8>);
impl Read for Pipe {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        if self.1.is_empty() {
            match self.0.recv() {
                Ok(bytes) => self.1 = bytes,
                Err(_) => return Ok(0),
            }
        }
        let count = buffer.len().min(self.1.len());
        buffer[..count].copy_from_slice(&self.1[..count]);
        self.1.drain(..count);
        Ok(count)
    }
}

/// Output that hands each report line to the test.
struct Lines(Sender<SearchReport>, Vec<u8>);
impl Write for Lines {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.1.extend_from_slice(bytes);
        while let Some(end) = self.1.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = self.1.drain(..=end).collect();
            let report = serde_json::from_slice(&line).unwrap();
            let _closed = self.0.send(report);
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A running lane.
struct Lane {
    commands: Sender<Vec<u8>>,
    reports: Receiver<SearchReport>,
}
impl Lane {
    fn start() -> Self {
        let (commands, input) = mpsc::channel();
        let (sender, reports) = mpsc::channel();
        std::thread::spawn(move || {
            serve_search(
                BufReader::new(Pipe(input, Vec::new())),
                Lines(sender, Vec::new()),
            );
        });
        Self { commands, reports }
    }
    fn send(&self, command: &SearchCommand) {
        let mut line = serde_json::to_vec(command).unwrap();
        line.push(b'\n');
        self.commands.send(line).unwrap();
    }
    /// Everything search `id` reports, until it ends.
    fn collect(&self, id: u64) -> (Vec<WorkspaceSearchFile>, WorkspaceSearchState) {
        let mut files = Vec::new();
        loop {
            match self.reports.recv_timeout(Duration::from_secs(30)).unwrap() {
                SearchReport::Found {
                    id: found,
                    files: more,
                } if found == id => files.extend(more),
                SearchReport::Ended { id: ended, state } if ended == id => return (files, state),
                _ => {}
            }
        }
    }
    /// Every batch listing `id` reports, until it ends.
    fn collect_listed(&self, id: u64) -> (Vec<Vec<String>>, WorkspaceSearchState) {
        let mut batches = Vec::new();
        loop {
            match self.reports.recv_timeout(Duration::from_secs(30)).unwrap() {
                SearchReport::Listed { id: listed, paths } if listed == id => batches.push(paths),
                SearchReport::Ended { id: ended, state } if ended == id => {
                    return (batches, state);
                }
                _ => {}
            }
        }
    }
}

fn query(pattern: &str) -> WorkspaceSearchQuery {
    WorkspaceSearchQuery {
        pattern: pattern.to_owned(),
        regex: false,
        match_case: false,
        whole_word: false,
        includes: Vec::new(),
        excludes: Vec::new(),
        use_ignore_files: true,
        paths: None,
    }
}

fn search(
    scratch: &Scratch,
    query: WorkspaceSearchQuery,
) -> (Vec<WorkspaceSearchFile>, WorkspaceSearchState) {
    let lane = Lane::start();
    lane.send(&SearchCommand::Start {
        id: 1,
        root: scratch.root(),
        query,
    });
    lane.collect(1)
}

/// The paths found, each once, sorted.
fn paths(files: &[WorkspaceSearchFile]) -> Vec<&str> {
    let mut paths: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
    paths.sort_unstable();
    paths.dedup();
    paths
}

/// Each preview line's number and matched text.
fn matched(files: &[WorkspaceSearchFile], path: &str) -> Vec<(u64, Vec<String>)> {
    files
        .iter()
        .filter(|file| file.path == path)
        .flat_map(|file| &file.lines)
        .map(|line| {
            let units: Vec<u16> = line.preview.encode_utf16().collect();
            let texts = line
                .matches
                .iter()
                .map(|range| {
                    String::from_utf16(&units[range.start as usize..range.end as usize]).unwrap()
                })
                .collect();
            (line.line, texts)
        })
        .collect()
}

/// Every entry a search reports is a valid page entry of the protocol.
fn assert_valid(files: Vec<WorkspaceSearchFile>) {
    let outcome = WorkspaceFilesOutcome::SearchResults {
        from: 0,
        files,
        more: false,
        state: WorkspaceSearchState::Complete,
    };
    assert!(outcome.validate().is_ok());
    assert!(outcome.answers(&WorkspaceFilesOperation::Search { query: query("x") }));
}

#[test]
fn finds_lines_with_utf16_ranges_skipping_git_and_ignored_files() {
    let scratch = Scratch::new();
    scratch.git_init();
    scratch.write(".gitignore", "build/\n");
    scratch.write(
        "a.txt",
        "first\r\n😀 中文 Needle and NEEDLE\r\nlast needle\n",
    );
    scratch.write(".hidden/b.txt", "needle\n");
    scratch.write("build/c.txt", "needle\n");
    scratch.write(".git/d.txt", "needle\n");
    let (files, state) = search(&scratch, query("needle"));
    assert_eq!(state, WorkspaceSearchState::Complete);
    assert_eq!(paths(&files), [".hidden/b.txt", "a.txt"]);
    assert_eq!(
        matched(&files, "a.txt"),
        [
            (2, vec!["Needle".to_owned(), "NEEDLE".to_owned()]),
            (3, vec!["needle".to_owned()]),
        ]
    );
    let line = &files
        .iter()
        .find(|file| file.path == "a.txt")
        .unwrap()
        .lines[0];
    // The emoji takes two UTF-16 units, and the carriage return is not shown.
    assert_eq!(line.preview, "😀 中文 Needle and NEEDLE");
    assert_eq!(line.matches[0].start, 6);
    assert_valid(files);

    let (files, _) = search(
        &scratch,
        WorkspaceSearchQuery {
            use_ignore_files: false,
            ..query("needle")
        },
    );
    assert_eq!(paths(&files), [".hidden/b.txt", "a.txt", "build/c.txt"]);
}

#[test]
fn matches_case_words_and_regular_expressions() {
    let scratch = Scratch::new();
    scratch.write("a.txt", "foo food Foo a.c abc\n");
    let words = |query: WorkspaceSearchQuery| matched(&search(&scratch, query).0, "a.txt");
    assert_eq!(words(query("foo"))[0].1, ["foo", "foo", "Foo"]);
    assert_eq!(
        words(WorkspaceSearchQuery {
            match_case: true,
            whole_word: true,
            ..query("foo")
        })[0]
            .1,
        ["foo"]
    );
    // Not a regular expression: the dot is a dot.
    assert_eq!(words(query("a.c"))[0].1, ["a.c"]);
    assert_eq!(
        words(WorkspaceSearchQuery {
            regex: true,
            ..query("a.c")
        })[0]
            .1,
        ["a.c", "abc"]
    );
    for (pattern, state) in [
        ("(unclosed", WorkspaceSearchState::InvalidPattern),
        ("a\u{0}b", WorkspaceSearchState::InvalidPattern),
    ] {
        let (_, ended) = search(
            &scratch,
            WorkspaceSearchQuery {
                regex: true,
                ..query(pattern)
            },
        );
        assert_eq!(ended, state);
    }
}

#[test]
fn a_line_break_in_the_pattern_matches_across_lines_of_either_ending() {
    let scratch = Scratch::new();
    scratch.write("unix.txt", "one\nend of\nstart here\n");
    scratch.write("dos.txt", "end of\r\nstart here\r\n");
    let (files, state) = search(&scratch, query("of\nstart"));
    assert_eq!(state, WorkspaceSearchState::Complete);
    for path in ["unix.txt", "dos.txt"] {
        let lines: Vec<_> = files
            .iter()
            .filter(|file| file.path == path)
            .flat_map(|file| &file.lines)
            .collect();
        assert_eq!(lines.len(), 1, "{path}");
        assert_eq!(lines[0].preview, "end of\nstart here");
        assert_eq!(lines[0].line, if path == "unix.txt" { 2 } else { 1 });
    }
    assert_valid(files);
}

#[test]
fn includes_and_excludes_follow_the_workspace_glob_rules() {
    let scratch = Scratch::new();
    scratch.write("src/a.txt", "needle\n");
    scratch.write("lib/src/b.txt", "needle\n");
    scratch.write("lib/c.log", "needle\n");
    scratch.write("d.txt", "needle\n");
    let found = |includes: &[&str], excludes: &[&str]| {
        let (files, state) = search(
            &scratch,
            WorkspaceSearchQuery {
                includes: includes.iter().map(|glob| (*glob).to_owned()).collect(),
                excludes: excludes.iter().map(|glob| (*glob).to_owned()).collect(),
                ..query("needle")
            },
        );
        assert_eq!(state, WorkspaceSearchState::Complete);
        paths(&files)
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    assert_eq!(found(&["src"], &[]), ["lib/src/b.txt", "src/a.txt"]);
    assert_eq!(found(&["./src"], &[]), ["src/a.txt"]);
    assert_eq!(found(&["*.txt"], &["lib"]), ["d.txt", "src/a.txt"]);
    assert_eq!(found(&[], &["*.log", "./src/"]), ["d.txt", "lib/src/b.txt"]);
    let (_, state) = search(
        &scratch,
        WorkspaceSearchQuery {
            includes: vec!["{unclosed".to_owned()],
            ..query("needle")
        },
    );
    assert_eq!(state, WorkspaceSearchState::InvalidGlob);
}

#[test]
fn named_paths_are_searched_even_when_ignored_and_others_are_not() {
    let scratch = Scratch::new();
    scratch.git_init();
    scratch.write(".gitignore", "*.log\n");
    scratch.write("a.log", "needle\n");
    scratch.write("b.txt", "needle\n");
    scratch.write("c.txt", "needle\n");
    let (files, _) = search(
        &scratch,
        WorkspaceSearchQuery {
            paths: Some(vec![
                "a.log".to_owned(),
                "b.txt".to_owned(),
                "missing.txt".to_owned(),
            ]),
            ..query("needle")
        },
    );
    assert_eq!(paths(&files), ["a.log", "b.txt"]);
}

#[test]
fn binary_files_and_undecodable_bytes() {
    let scratch = Scratch::new();
    scratch.write("binary.bin", b"needle\0needle\n");
    scratch.write("latin.txt", b"caf\xe9 needle\n");
    let (files, _) = search(&scratch, query("needle"));
    assert_eq!(paths(&files), ["latin.txt"]);
    let line = &files[0].lines[0];
    // The byte that is not UTF-8 shows as one replacement character.
    assert_eq!(line.preview, "caf\u{fffd} needle");
    assert_eq!((line.matches[0].start, line.matches[0].end), (5, 11));
}

#[test]
fn a_long_line_shows_the_part_around_each_match() {
    let scratch = Scratch::new();
    let mut text = "a".repeat(3000);
    text.insert_str(2500, "needle");
    text.insert_str(10, "needle");
    scratch.write("long.txt", format!("{text}\n"));
    let (files, _) = search(&scratch, query("needle"));
    let lines = &files[0].lines;
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].preview_start, 0);
    assert_eq!(
        (lines[0].matches[0].start, lines[0].matches[0].end),
        (10, 16)
    );
    assert_eq!(lines[1].preview_start, 2506 - 40);
    assert_eq!(lines[1].matches[0].start, 40);
    assert!(lines.iter().all(|line| line.preview.len() <= 1000));
    assert_valid(files);
}

#[test]
fn a_file_with_many_lines_comes_as_consecutive_entries() {
    let scratch = Scratch::new();
    scratch.write("many.txt", "needle\n".repeat(150));
    scratch.write("other.txt", "needle\n");
    let (files, _) = search(&scratch, query("needle"));
    let many: Vec<usize> = files
        .iter()
        .enumerate()
        .filter(|(_, file)| file.path == "many.txt")
        .map(|(index, _)| index)
        .collect();
    assert_eq!(many.len(), 3);
    assert!(many.windows(2).all(|pair| pair[1] == pair[0] + 1));
    let numbers: Vec<u64> = many
        .iter()
        .flat_map(|index| files[*index].lines.iter().map(|line| line.line))
        .collect();
    assert_eq!(numbers, (1..=150).collect::<Vec<u64>>());
}

#[test]
fn a_search_stops_at_its_most_matches() {
    let scratch = Scratch::new();
    let count = usize::try_from(MAXIMUM_WORKSPACE_SEARCH_MATCHES).unwrap() + 10;
    scratch.write("many.txt", "x ".repeat(count));
    let (files, state) = search(&scratch, query("x"));
    assert_eq!(state, WorkspaceSearchState::LimitReached);
    let total: usize = files
        .iter()
        .flat_map(|file| &file.lines)
        .map(|line| line.matches.len())
        .sum();
    assert_eq!(
        total,
        usize::try_from(MAXIMUM_WORKSPACE_SEARCH_MATCHES).unwrap()
    );
}

#[test]
fn a_cancelled_search_still_ends() {
    let scratch = Scratch::new();
    for index in 0..200 {
        scratch.write(&format!("f{index}.txt"), "needle\n".repeat(200));
    }
    let lane = Lane::start();
    lane.send(&SearchCommand::Start {
        id: 7,
        root: scratch.root(),
        query: query("needle"),
    });
    lane.send(&SearchCommand::Cancel { id: 7 });
    let (_, state) = lane.collect(7);
    assert_ne!(state, WorkspaceSearchState::Searching);
}

#[test]
fn a_listing_names_the_files_a_search_would_read_in_pages() {
    let scratch = Scratch::new();
    scratch.git_init();
    scratch.write(".gitignore", "build/\n");
    scratch.write(".hidden/a.txt", "");
    scratch.write("build/b.txt", "");
    scratch.write("lib/c.log", "");
    scratch.write(".git/d.txt", "");
    for index in 0..MAXIMUM_WORKSPACE_FILE_LIST_PAGE {
        scratch.write(&format!("many/{index}.txt"), "");
    }
    let list = |excludes: &[&str], use_ignore_files: bool| {
        let lane = Lane::start();
        lane.send(&SearchCommand::List {
            id: 3,
            root: scratch.root(),
            query: WorkspaceFileListQuery {
                excludes: excludes.iter().map(|glob| (*glob).to_owned()).collect(),
                use_ignore_files,
            },
        });
        let (batches, state) = lane.collect_listed(3);
        assert_eq!(state, WorkspaceSearchState::Complete);
        assert!(
            batches
                .iter()
                .all(|batch| batch.len() <= MAXIMUM_WORKSPACE_FILE_LIST_PAGE)
        );
        let mut paths: Vec<String> = batches.into_iter().flatten().collect();
        paths.retain(|path| !path.starts_with("many/"));
        paths.sort_unstable();
        paths
    };
    assert_eq!(
        list(&[], true),
        [".gitignore", ".hidden/a.txt", "lib/c.log"]
    );
    assert_eq!(
        list(&["*.log"], false),
        [".gitignore", ".hidden/a.txt", "build/b.txt"]
    );
}
