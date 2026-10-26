use super::*;
use std::ffi::OsString;
fn args(words: &[&str]) -> Result<Request, ParseFailure> {
    crate::invocation::parse_request(words.iter().map(OsString::from))
}
const ID: &str = "33333333333333333333333333333333";
#[test]
fn copy_motion_grammar_admits_all_motions_and_full_buffer_fences() {
    for motion in CopyMotion::ALL {
        let Request::TerminalCopy { command, format } = args(&[
            "terminal",
            "copy-motion",
            ID,
            "--cursor=4294967295:65535",
            "--motion",
            motion.token(),
            "--content-revision=18446744073709551615",
            "--generation",
            "18446744073709551615",
            "--format",
            "json",
        ])
        .unwrap() else {
            panic!("copy command");
        };
        assert_eq!(
            command.cursor(),
            TerminalTextPoint {
                row: u32::MAX,
                col: u16::MAX
            }
        );
        assert_eq!(command.revision(), Some(u64::MAX));
        assert_eq!(command.generation, Some(u64::MAX));
        assert_eq!(format, OutputFormat::Json);
    }
}
#[test]
fn copy_search_grammar_preserves_literals_orders_previous_and_requires_revision() {
    let Request::TerminalCopy { command, .. } = args(&[
        "terminal",
        "copy-search",
        ID,
        "--cursor",
        "0:0",
        "--query",
        " café.*\n",
        "--search-direction",
        "backward",
        "--content-revision",
        "0",
        "--previous",
        "4:3-0:2",
    ])
    .unwrap() else {
        panic!("search");
    };
    let TerminalCopyAction::Search {
        query,
        previous,
        revision,
        ..
    } = command.action
    else {
        panic!("search action");
    };
    assert_eq!(query, " café.*\n");
    assert_eq!(revision, 0);
    assert_eq!(
        previous,
        Some(TerminalTextRange {
            start: TerminalTextPoint { row: 0, col: 2 },
            end: TerminalTextPoint { row: 4, col: 3 }
        })
    );
    assert!(
        args(&[
            "terminal",
            "copy-search",
            ID,
            "--cursor",
            "0:0",
            "--query",
            "",
            "--search-direction",
            "forward"
        ])
        .is_err()
    );
    assert!(
        args(&[
            "terminal",
            "copy-search",
            ID,
            "--cursor",
            "0:0",
            "--query",
            "",
            "--search-direction",
            "forward",
            "--content-revision",
            "0"
        ])
        .is_ok()
    );
}
#[test]
fn copy_requests_reject_bad_scopes_duplicates_queries_ranges_and_numbers_before_effects() {
    for extra in [
        vec!["--motion", "bad"],
        vec!["--query", "x"],
        vec!["--anchor", "0:0"],
        vec!["--generation", "0"],
        vec!["--content-revision", "+1"],
        vec!["--cursor", "0:0"],
    ] {
        let mut words = vec![
            "terminal",
            "copy-motion",
            ID,
            "--cursor",
            "0:0",
            "--motion",
            "line-end",
        ];
        words.extend(extra);
        assert!(args(&words).is_err(), "{words:?}");
    }
    for extra in [
        vec!["--previous", "1:0"],
        vec!["--previous", "1:0--2:0"],
        vec!["--previous", "0:65536-1:0"],
        vec!["--search-direction", "up"],
        vec!["--motion", "line-end"],
    ] {
        let mut words = vec![
            "terminal",
            "copy-search",
            ID,
            "--cursor",
            "0:0",
            "--query",
            "x",
            "--search-direction",
            "forward",
            "--content-revision",
            "0",
        ];
        words.extend(extra);
        assert!(args(&words).is_err(), "{words:?}");
    }
    let long = "中".repeat(1366);
    assert!(
        args(&[
            "terminal",
            "copy-search",
            ID,
            "--cursor",
            "0:0",
            "--query",
            &long,
            "--search-direction",
            "forward",
            "--content-revision",
            "0"
        ])
        .is_err()
    );
}
