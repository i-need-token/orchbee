//! Replacements against real files, named by what the search lane itself reported: a match is
//! found again only when this module counts lines and UTF-16 code units exactly as a search
//! preview does, and a mistake there replaces nothing, or the wrong text, without any error.
use super::pattern::{Replacement, with_case_of};
use crate::{BlobCache, SearchCommand, SearchReport, answer, serve_search};
use orchbee_workbench_protocol::{
    WorkspaceFilesFailure, WorkspaceFilesOperation, WorkspaceFilesOutcome, WorkspaceReplaceFile,
    WorkspaceReplaceMatch, WorkspaceReplaceQuery, WorkspaceReplaceResult, WorkspaceSearchFile,
    WorkspaceSearchQuery,
};
use std::io::{BufReader, Cursor, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// A temporary directory removed on drop.
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "orchbee-workspace-files-replace-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _removed = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(std::fs::canonicalize(path).unwrap())
    }
    fn write(&self, relative: &str, bytes: impl AsRef<[u8]>) {
        std::fs::write(self.0.join(relative), bytes).unwrap();
    }
    fn read(&self, relative: &str) -> Vec<u8> {
        std::fs::read(self.0.join(relative)).unwrap()
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

fn query(pattern: &str, replacement: &str) -> WorkspaceReplaceQuery {
    WorkspaceReplaceQuery {
        pattern: pattern.to_owned(),
        regex: false,
        match_case: false,
        whole_word: false,
        replacement: replacement.to_owned(),
        preserve_case: false,
    }
}

/// Input that ends only once its sender drops, as the Server's pipe does, so the lane does not
/// stop a search when its one command has been read.
struct Open(mpsc::Receiver<()>);
impl Read for Open {
    fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
        let _ended = self.0.recv();
        Ok(0)
    }
}

/// Output that hands each report line to the test.
struct Reports(mpsc::Sender<SearchReport>, Vec<u8>);
impl Write for Reports {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.1.extend_from_slice(bytes);
        while let Some(end) = self.1.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = self.1.drain(..=end).collect();
            let _closed = self.0.send(serde_json::from_slice(&line).unwrap());
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// What the search lane reports for `query`, run to its end.
fn search(scratch: &Scratch, query: &WorkspaceReplaceQuery) -> Vec<WorkspaceSearchFile> {
    let mut command = serde_json::to_vec(&SearchCommand::Start {
        id: 1,
        root: scratch.root(),
        query: WorkspaceSearchQuery {
            pattern: query.pattern.clone(),
            regex: query.regex,
            match_case: query.match_case,
            whole_word: query.whole_word,
            includes: Vec::new(),
            excludes: Vec::new(),
            use_ignore_files: false,
            paths: None,
        },
    })
    .unwrap();
    command.push(b'\n');
    let (open, input) = mpsc::channel();
    let (sender, reports) = mpsc::channel();
    std::thread::spawn(move || {
        serve_search(
            BufReader::new(Cursor::new(command).chain(Open(input))),
            Reports(sender, Vec::new()),
        );
    });
    let mut files = Vec::new();
    while let SearchReport::Found { files: found, .. } =
        reports.recv_timeout(Duration::from_secs(30)).unwrap()
    {
        files.extend(found);
    }
    drop(open);
    files
}

/// Each reported file's matches, as a replacement names them.
fn reported(files: &[WorkspaceSearchFile]) -> Vec<WorkspaceReplaceFile> {
    let mut named: Vec<WorkspaceReplaceFile> = Vec::new();
    for file in files {
        let matches = file.lines.iter().flat_map(|line| {
            line.matches.iter().map(|range| WorkspaceReplaceMatch {
                line: line.line,
                column: line.preview_start + range.start,
            })
        });
        match named.iter_mut().find(|entry| entry.path == file.path) {
            Some(entry) => entry.matches.extend(matches),
            None => named.push(WorkspaceReplaceFile {
                path: file.path.clone(),
                matches: matches.collect(),
            }),
        }
    }
    named.sort_by(|left, right| left.path.cmp(&right.path));
    named
}

fn replace(
    scratch: &Scratch,
    query: WorkspaceReplaceQuery,
    files: Vec<WorkspaceReplaceFile>,
) -> Vec<(String, WorkspaceReplaceResult)> {
    let operation = WorkspaceFilesOperation::ReplaceMatches { query, files };
    let outcome = answer(
        &scratch.root(),
        &operation,
        &BlobCache::default(),
        Instant::now() + Duration::from_secs(40),
    );
    assert!(outcome.answers(&operation), "{outcome:?}");
    let WorkspaceFilesOutcome::Replaced { files } = outcome else {
        panic!("{outcome:?}");
    };
    files
        .into_iter()
        .map(|file| (file.path, file.result))
        .collect()
}

fn replaced(result: &WorkspaceReplaceResult) -> bool {
    matches!(result, WorkspaceReplaceResult::Replaced { version } if version.sha256.is_some())
}

#[test]
fn replaces_every_match_a_search_reported_however_its_line_counts_units() {
    let scratch = Scratch::new();
    // Characters outside the Basic Multilingual Plane count two units, CRLF ends lines, and a
    // line past a preview's length shows only the parts around its matches.
    let long = format!("{}needle{}needle\n", "x".repeat(1500), "y".repeat(1500));
    scratch.write("a.txt", "😀 中文 Needle and NEEDLE\r\nlast needle\r\n");
    scratch.write("b.txt", &long);
    let query = query("needle", "pin");
    let files = reported(&search(&scratch, &query));
    assert_eq!(files.len(), 2);
    let results = replace(&scratch, query, files);
    assert!(
        results.iter().all(|(_, result)| replaced(result)),
        "{results:?}"
    );
    assert_eq!(
        scratch.read("a.txt"),
        "😀 中文 pin and pin\r\nlast pin\r\n".as_bytes()
    );
    assert_eq!(
        scratch.read("b.txt"),
        format!("{}pin{}pin\n", "x".repeat(1500), "y".repeat(1500)).as_bytes()
    );
}

#[test]
fn replaces_matches_across_lines_with_groups_and_case_changes() {
    let scratch = Scratch::new();
    scratch.write("a.txt", "let first_id\r\n= 1;\nlet second_id\n= 2;\n");
    let query = WorkspaceReplaceQuery {
        regex: true,
        match_case: true,
        ..query(r"(?P<name>\w+)_id\r?\n=", r"\u${name}Id =")
    };
    let files = reported(&search(&scratch, &query));
    assert_eq!(files[0].matches.len(), 2);
    let results = replace(&scratch, query, files);
    assert!(replaced(&results[0].1), "{results:?}");
    assert_eq!(
        scratch.read("a.txt"),
        b"let FirstId = 1;\nlet SecondId = 2;\n"
    );
}

#[test]
fn leaves_out_dismissed_matches_and_refuses_a_file_that_moved_them() {
    let scratch = Scratch::new();
    scratch.write("a.txt", "one needle\ntwo needle\nthree needle\n");
    scratch.write("b.txt", "needle\n");
    let query = query("needle", "pin");
    let mut files = reported(&search(&scratch, &query));
    // The person dismissed the second line's match.
    files[0].matches.remove(1);
    // Another program added a line above the match since the search.
    scratch.write("b.txt", "added\nneedle\n");
    let results = replace(&scratch, query, files);
    assert!(replaced(&results[0].1), "{results:?}");
    assert_eq!(results[1].1, WorkspaceReplaceResult::Changed {});
    assert_eq!(scratch.read("a.txt"), b"one pin\ntwo needle\nthree pin\n");
    assert_eq!(scratch.read("b.txt"), b"added\nneedle\n");
}

#[test]
fn leaves_binary_and_missing_files_alone_and_stops_at_its_deadline() {
    let scratch = Scratch::new();
    scratch.write("a.bin", b"needle\0\n");
    scratch.write("b.txt", "needle\n");
    let at_start = vec![WorkspaceReplaceMatch { line: 1, column: 0 }];
    let file = |path: &str| WorkspaceReplaceFile {
        path: path.to_owned(),
        matches: at_start.clone(),
    };
    let results = replace(
        &scratch,
        query("needle", "pin"),
        vec![file("a.bin"), file("gone.txt")],
    );
    assert_eq!(results[0].1, WorkspaceReplaceResult::NotEditable {});
    assert_eq!(
        results[1].1,
        WorkspaceReplaceResult::Failed {
            reason: WorkspaceFilesFailure::NotFound
        }
    );
    let operation = WorkspaceFilesOperation::ReplaceMatches {
        query: query("needle", "pin"),
        files: vec![file("b.txt")],
    };
    let late = answer(
        &scratch.root(),
        &operation,
        &BlobCache::default(),
        Instant::now(),
    );
    assert_eq!(
        late,
        WorkspaceFilesOutcome::Replaced {
            files: vec![orchbee_workbench_protocol::WorkspaceReplacedFile {
                path: "b.txt".to_owned(),
                result: WorkspaceReplaceResult::Failed {
                    reason: WorkspaceFilesFailure::Timeout
                },
            }],
        }
    );
    assert_eq!(scratch.read("b.txt"), b"needle\n");
}

#[test]
fn reads_a_replacement_as_written_or_with_groups_and_escapes() {
    let groups = |group: &super::pattern::Group| match group {
        super::pattern::Group::Index(0) => Some(Some("foo-bar")),
        super::pattern::Group::Index(1) => Some(Some("foo")),
        super::pattern::Group::Index(2) => Some(None),
        super::pattern::Group::Name(name) if name == "tail" => Some(Some("bar")),
        _ => None,
    };
    let expand =
        |text: &str, regex: bool| Replacement::parse(text, regex).expand("foo-bar", false, groups);
    assert_eq!(expand(r"$1\n$$\t\\", false), r"$1\n$$\t\\");
    assert_eq!(expand(r"$1\n$$\t\\", true), "foo\n$\t\\");
    assert_eq!(
        expand(r"\U$1-\u${tail}-$<tail>-$&-$0", true),
        "FOO-Bar-bar-foo-bar-foo-bar"
    );
    // An absent two-digit group is its first digit's group and a digit; an unknown one stays.
    assert_eq!(expand("$10|$2|$9|${nope}|$x", true), "foo0||$9|${nope}|$x");
}

#[test]
fn keeps_the_replaced_texts_case_part_by_part() {
    assert_eq!(with_case_of("NEEDLE", "pin"), "PIN");
    assert_eq!(with_case_of("needle", "Pin"), "pin");
    assert_eq!(with_case_of("Needle", "pin"), "Pin");
    assert_eq!(with_case_of("nEEDLE", "Pin"), "pin");
    assert_eq!(with_case_of("Foo-BAR", "baz-qux"), "Baz-QUX");
    assert_eq!(with_case_of("foo_Bar", "baz_qux"), "baz_Qux");
    // Both separators, or different counts of parts, keep the whole match's case.
    assert_eq!(with_case_of("Foo-Bar_Baz", "a-b_c"), "A-b_c");
    assert_eq!(with_case_of("FOO-BAR", "a-b-c"), "A-B-C");
    // Groups never take the matched text's case.
    let groups = |_: &super::pattern::Group| Some(Some("x"));
    assert_eq!(
        Replacement::parse("$0y", true).expand("NEEDLE", true, groups),
        "xy"
    );
}
