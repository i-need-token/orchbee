use super::*;
use crate::invocation::parse_request;
use std::ffi::OsString;

fn parse(words: &[&str]) -> Result<Request, ParseFailure> {
    parse_request(words.iter().map(OsString::from))
}

#[test]
fn implicit_reads_and_waits_use_recent_with_a_finite_default() {
    let id = "11111111111111111111111111111111";
    for words in [vec!["terminal", "read", id], vec!["agent", "read", "alpha"]] {
        assert!(matches!(
            parse(&words),
            Ok(Request::Query {
                command: QueryCommand::TerminalOutput {
                    selection: OutputSelection {
                        source: OutputSource::Recent,
                        lines: None
                    },
                    plain: false,
                    ansi: false,
                    ..
                },
                ..
            })
        ));
        let mut unwrapped = words.clone();
        unwrapped.extend(["--source", "recent-unwrapped", "--lines", "0"]);
        assert!(matches!(
            parse(&unwrapped),
            Ok(Request::Query {
                command: QueryCommand::TerminalOutput {
                    selection: OutputSelection {
                        source: OutputSource::RecentUnwrapped,
                        lines: Some(0)
                    },
                    ..
                },
                ..
            })
        ));
    }
    for (extra, lines, timeout) in [
        (vec![], None, Some(30_000)),
        (vec!["--lines=3"], Some(3), Some(30_000)),
        (vec!["--lines=3", "--forever"], Some(3), None),
        (vec!["--timeout=30000"], None, Some(30_000)),
    ] {
        let mut words = vec!["terminal", "wait-output", id, "--match=done"];
        words.extend(extra);
        assert!(matches!(parse(&words), Ok(Request::Query {
                command: QueryCommand::TerminalSourceWait { selection: OutputSelection { source: OutputSource::Recent, lines: found }, timeout_ms, .. }, ..
            }) if found == lines && timeout_ms == timeout));
    }
}

#[test]
fn ansi_selects_recent_by_default_and_accepts_a_json_envelope() {
    for words in [
        vec![
            "terminal",
            "read",
            "11111111111111111111111111111111",
            "--ansi",
        ],
        vec![
            "agent",
            "read",
            "--name",
            "alpha",
            "--ansi",
            "--format=json",
        ],
    ] {
        let Request::Query {
            command:
                QueryCommand::TerminalOutput {
                    selection,
                    plain,
                    ansi,
                    ..
                },
            ..
        } = parse(&words).expect("ANSI read")
        else {
            panic!("output query")
        };
        assert!(ansi && !plain);
        assert_eq!(
            selection,
            OutputSelection {
                source: OutputSource::Recent,
                lines: None
            }
        );
    }
    for words in [
        vec!["status", "--ansi"],
        vec!["--version", "--ansi"],
        vec![
            "terminal",
            "wait-output",
            "11111111111111111111111111111111",
            "--ansi",
            "--match",
            "x",
        ],
    ] {
        assert!(parse(&words).is_err(), "{words:?}");
    }
}

#[test]
fn source_read_bounds_physical_lines() {
    let id = "11111111111111111111111111111111";
    for (argument, expected) in [("0", 0), ("17", 17), ("4294967295", 1000)] {
        let Request::Query {
            command:
                QueryCommand::TerminalOutput {
                    selection, plain, ..
                },
            ..
        } = parse(&[
            "terminal",
            "read",
            id,
            "--source=recent-unwrapped",
            "--lines",
            argument,
            "--plain",
        ])
        .expect("source read")
        else {
            panic!("source query")
        };
        assert_eq!(
            selection,
            OutputSelection {
                source: OutputSource::RecentUnwrapped,
                lines: Some(expected)
            }
        );
        assert!(plain);
    }
    #[cfg(not(windows))]
    assert!(matches!(
        parse(&["terminal", "read", id]),
        Ok(Request::Query {
            command: QueryCommand::TerminalOutput { .. },
            ..
        })
    ));
    assert!(matches!(
        parse(&["agent", "read", "--name", "worker", "--plain"]),
        Ok(Request::Query {
            command: QueryCommand::TerminalOutput {
                target: OutputTarget::Agent(_),
                selection: OutputSelection {
                    source: OutputSource::Recent,
                    lines: None
                },
                plain: true,
                ansi: false
            },
            ..
        })
    ));
}

#[test]
fn invalid_or_mixed_source_forms_fail_before_runtime() {
    let id = "11111111111111111111111111111111";
    for extra in [
        vec!["--ansi", "--plain"],
        vec!["--ansi", "--history", "--rows", "1"],
        vec!["--ansi", "--unwrap"],
        vec!["--ansi", "--ansi"],
        vec!["--ansi=yes"],
        vec!["--source", "bad"],
        vec!["--source", "visible", "--unwrap"],
        vec!["--plain", "--rows", "2"],
        vec!["--source", "recent", "--history"],
        vec!["--plain", "--format=json"],
        vec!["--plain", "--plain"],
        vec!["--plain", "--lines", "4294967296"],
        vec!["--plain", "--lines", "+1"],
        vec!["--plain", "--lines", "-1"],
    ] {
        let mut words = vec!["terminal", "read", id];
        words.extend(extra);
        assert!(parse(&words).is_err(), "{words:?}");
    }
    for words in [
        vec!["status", "--plain"],
        vec!["terminal", "wait-output", id, "--plain", "--match", "x"],
        vec!["tui", "--source", "visible"],
        vec!["--version", "--plain"],
    ] {
        assert!(parse(&words).is_err(), "{words:?}");
    }
}

#[test]
fn source_wait_defaults_bounds_and_forever_are_explicit() {
    let id = "11111111111111111111111111111111";
    for (extra, source, lines, timeout) in [
        (
            vec!["--source=visible"],
            OutputSource::Visible,
            None,
            Some(30_000),
        ),
        (
            vec!["--lines=0"],
            OutputSource::Recent,
            Some(0),
            Some(30_000),
        ),
        (
            vec!["--lines=4294967295", "--timeout=7"],
            OutputSource::Recent,
            Some(1000),
            Some(7),
        ),
        (vec!["--forever"], OutputSource::Recent, None, None),
        (
            vec!["--source=detection", "--forever"],
            OutputSource::Detection,
            None,
            None,
        ),
    ] {
        let mut words = vec!["terminal", "wait-output", id, "--match=x"];
        words.extend(extra);
        let Request::Query {
            command:
                QueryCommand::TerminalSourceWait {
                    selection,
                    timeout_ms,
                    ..
                },
            ..
        } = parse(&words).unwrap()
        else {
            panic!("source wait")
        };
        assert_eq!(selection, OutputSelection { source, lines });
        assert_eq!(timeout_ms, timeout);
    }
    #[cfg(not(windows))]
    assert!(matches!(
        parse(&["terminal", "wait-output", id, "--match=x"]),
        Ok(Request::Query {
            command: QueryCommand::TerminalWaitOutput {
                rows: 1000,
                timeout_ms: 30_000,
                ..
            },
            ..
        })
    ));
    for extra in [
        vec!["--source=visible", "--rows=2"],
        vec!["--lines=1", "--rows=2"],
        vec!["--forever", "--timeout=1"],
        vec!["--source=visible", "--timeout=0"],
        vec!["--source=bad"],
        vec!["--lines=+1"],
        vec!["--lines=4294967296"],
        vec!["--forever", "--forever"],
        vec!["--source=visible", "--history"],
    ] {
        let mut words = vec!["terminal", "wait-output", id, "--match=x"];
        words.extend(extra);
        assert!(parse(&words).is_err(), "{words:?}");
    }
}
