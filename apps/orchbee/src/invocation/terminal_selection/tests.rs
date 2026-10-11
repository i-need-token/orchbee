use super::*;
use std::ffi::OsString;
fn parse_args(words: &[&str]) -> Result<Request, ParseFailure> {
    crate::invocation::parse_request(words.iter().map(OsString::from))
}
const ID: &str = "33333333333333333333333333333333";
#[test]
fn selection_read_admits_full_reference_coordinate_and_revision_ranges() {
    let Request::TerminalSelection { command, format } = parse_args(&[
        "terminal",
        "selection-read",
        ID,
        "--anchor=4294967295:65535",
        "--cursor",
        "4294967294:0",
        "--content-revision",
        "18446744073709551615",
        "--generation=18446744073709551615",
        "--format=json",
    ])
    .unwrap() else {
        panic!("selection command")
    };
    assert_eq!(command.anchor.row, u32::MAX);
    assert_eq!(command.anchor.col, u16::MAX);
    assert_eq!(command.revision, Some(u64::MAX));
    assert_eq!(command.generation, Some(u64::MAX));
    assert_eq!(format, OutputFormat::Json);
}
#[test]
fn selection_read_rejects_malformed_missing_duplicate_and_foreign_inputs() {
    for raw in [
        "",
        "0",
        "0:0:0",
        "-1:0",
        "+1:0",
        " 1:0",
        "4294967296:0",
        "0:65536",
    ] {
        assert!(
            parse_args(&[
                "terminal",
                "selection-read",
                ID,
                "--anchor",
                raw,
                "--cursor",
                "0:0"
            ])
            .is_err(),
            "{raw}"
        );
    }
    for extra in [
        vec!["--anchor", "0:0"],
        vec!["--source", "visible"],
        vec!["--generation", "0"],
        vec!["--content-revision", "18446744073709551616"],
        vec!["--content-revision", "+1"],
    ] {
        let mut words = vec![
            "terminal",
            "selection-read",
            ID,
            "--anchor",
            "0:0",
            "--cursor",
            "0:0",
        ];
        words.extend(extra);
        assert!(parse_args(&words).is_err(), "{words:?}");
    }
    assert!(parse_args(&["terminal", "selection-read", ID]).is_err());
    assert!(parse_args(&["terminal", "read", ID, "--anchor", "0:0"]).is_err());
}
#[test]
fn selection_read_span_is_bounded_before_discovery_in_either_direction() {
    for (anchor, cursor, ok) in [
        ("0:0", "4095:0", true),
        ("4095:0", "0:0", true),
        ("0:0", "4096:0", false),
        ("4294967295:0", "0:0", false),
    ] {
        assert_eq!(
            parse_args(&[
                "terminal",
                "selection-read",
                ID,
                "--anchor",
                anchor,
                "--cursor",
                cursor
            ])
            .is_ok(),
            ok
        );
    }
}
