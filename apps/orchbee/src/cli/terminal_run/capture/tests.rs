use super::{Captured, MAXIMUM_OUTPUT_BYTES, Markers, Scan, wrapped};
use crate::cli::terminal_output::OutputRow;
use crate::invocation::CaptureShell;

const RUN: &str = "0123456789abcdef0123456789abcdef";

/// Printed lines as screen rows `width` cells wide; `trimmed` drops the spaces a row ends
/// with, as a soft wrap that falls on a space can.
fn screen(lines: &[String], width: usize, trimmed: bool) -> Vec<OutputRow> {
    let mut rows = Vec::new();
    for line in lines {
        let characters = line.chars().collect::<Vec<_>>();
        let pieces = if characters.is_empty() {
            vec![String::new()]
        } else {
            characters
                .chunks(width)
                .map(|piece| piece.iter().collect::<String>())
                .collect()
        };
        let count = pieces.len();
        for (index, piece) in pieces.into_iter().enumerate() {
            rows.push(OutputRow {
                index: rows.len() as u64,
                text: if trimmed {
                    piece.trim_end().to_owned()
                } else {
                    piece
                },
                from_previous: index > 0,
                into_next: index + 1 < count,
            });
        }
    }
    rows
}

/// Scan `rows` from the bottom in windows of `window` rows, as the terminal reads them.
fn scanned(markers: &Markers, rows: &[OutputRow], window: usize) -> Captured {
    let mut scan = Scan::new(markers);
    let mut end = rows.len();
    loop {
        let start = end.saturating_sub(window);
        if let Some(found) = scan.push(rows[start..end].to_vec(), start == 0) {
            return found;
        }
        end = start;
    }
}

/// What a shell prints for one run: the echoed input, the markers around `printed`, a prompt.
fn printed(echo: &str, printed: &str, status: &str) -> Vec<String> {
    format!("{echo}\n[orchbee run {RUN}]\n{printed}\n[orchbee end {RUN}] {status}\n$ ")
        .split('\n')
        .map(str::to_owned)
        .collect()
}

#[test]
fn echoed_input_never_holds_either_marker() {
    let markers = Markers::new(RUN);
    for shell in [
        CaptureShell::Posix,
        CaptureShell::Fish,
        CaptureShell::PowerShell,
        CaptureShell::Cmd,
    ] {
        let commands: &[&str] = if shell == CaptureShell::Cmd {
            &["echo [orchbee end x] 0", "dir"]
        } else {
            &[
                "echo [orchbee end x] 0",
                "printf '%s\\n' \"$HOME\" 'it''s'",
                "first\n\nsecond # comment\n",
            ]
        };
        for command in commands {
            let input = wrapped(shell, RUN, command);
            for prompt in ["", "PS C:\\> ", "> "] {
                let lines = input
                    .split('\n')
                    .map(|line| format!("{prompt}{line}"))
                    .collect::<Vec<_>>();
                for (width, trimmed) in [(7, true), (47, true), (80, false), (4096, false)] {
                    let rows = screen(&lines, width, trimmed);
                    assert!(!markers.ended_in(&rows), "{shell:?} {command:?} {width}");
                    assert_eq!(
                        scanned(&markers, &rows, 3),
                        Captured {
                            status: None,
                            output: None,
                            truncated: false,
                        },
                        "{shell:?} {command:?} {width}"
                    );
                }
            }
        }
    }
}

#[test]
fn output_is_exactly_the_text_printed_between_the_markers() {
    let markers = Markers::new(RUN);
    let long = "a line long enough to wrap at every width below, with spaces in it";
    for (text, output) in [
        ("hi\n".to_owned(), "hi\n".to_owned()),
        ("hi".to_owned(), "hi".to_owned()),
        (String::new(), String::new()),
        (
            "a\n\n  indented\n".to_owned(),
            "a\n\n  indented\n".to_owned(),
        ),
        (format!("{long}\nnext\n"), format!("{long}\nnext\n")),
    ] {
        // The shell prints the end marker after a line break of its own.
        let lines = printed("$ echoed input", &text, "3");
        for width in [9, 23, 80] {
            let rows = screen(&lines, width, false);
            assert!(markers.ended_in(&screen(&lines, width, true)));
            for window in [1, 2, 5, 4096] {
                assert_eq!(
                    scanned(&markers, &rows, window),
                    Captured {
                        status: Some(3),
                        output: Some(output.clone()),
                        truncated: false,
                    },
                    "{text:?} {width} {window}"
                );
            }
        }
    }
}

#[test]
fn only_a_decimal_status_of_this_run_ends_it() {
    let markers = Markers::new(RUN);
    let other = "fedcba9876543210fedcba9876543210";
    for (line, status) in [
        (format!("[orchbee end {RUN}] 0"), Some(0)),
        (
            format!("[orchbee end {RUN}] -1073741819"),
            Some(-1_073_741_819),
        ),
        (format!("[orchbee end {RUN}]"), None),
        (format!("[orchbee end {RUN}] ok"), None),
        (format!("[orchbee end {RUN}] 0x1"), None),
        (format!("[orchbee end {other}] 0"), None),
        (format!("> [orchbee end {RUN}] 0"), None),
    ] {
        let rows = screen(std::slice::from_ref(&line), 80, false);
        assert_eq!(markers.ended_in(&rows), status.is_some(), "{line}");
        assert_eq!(scanned(&markers, &rows, 80).status, status, "{line}");
    }
}

#[test]
fn rows_are_output_only_below_a_begin_marker_or_once_its_input_is_gone() {
    let markers = Markers::new(RUN);
    let end = format!("[orchbee end {RUN}] 2");
    let echo = wrapped(CaptureShell::Posix, RUN, "make");
    for (lines, status, output, truncated) in [
        // The history or a clear dropped the input and the begin marker: every row left above
        // the end marker was printed after them.
        (
            vec!["tail of the output", "", &end, "$ "],
            Some(2),
            Some("tail of the output\n"),
            true,
        ),
        // The input is still there but the begin marker is not, as when the command overwrote
        // its row: no row is surely output.
        (
            vec![&echo, "drawn over", "", &end, "$ "],
            Some(2),
            None,
            false,
        ),
        // A command still running reports what it printed so far, without the blank rows below.
        (
            vec![&echo, &format!("[orchbee run {RUN}]"), "so far", "", ""],
            None,
            Some("so far"),
            false,
        ),
        (vec!["earlier text", "", ""], None, None, false),
    ] {
        let rows = screen(
            &lines
                .iter()
                .map(|line| (*line).to_owned())
                .collect::<Vec<_>>(),
            80,
            false,
        );
        assert_eq!(
            scanned(&markers, &rows, 2),
            Captured {
                status,
                output: output.map(str::to_owned),
                truncated,
            },
            "{lines:?}"
        );
    }
}

#[test]
fn rows_a_redraw_left_wrapping_below_the_input_keep_their_lines() {
    // Under ConPTY, every row that a line editor's prediction took below the input, from the
    // input's last row down, stays marked as wrapping into the next, padded with spaces to the
    // edge, after the shell prints over it.
    let markers = Markers::new(RUN);
    let lines = printed("$ echoed input", "one\n\ntwo\n", "12");
    // At 9 columns each marker spans rows; at 48 the end marker wraps inside its status.
    for width in [9, 48, 80] {
        let clean = screen(&lines, width, false);
        let begin = clean
            .iter()
            .position(|row| row.text.starts_with("[orchbee"))
            .unwrap();
        for last in begin..clean.len() {
            let mut rows = clean.clone();
            for index in begin - 1..last {
                rows[index].text = format!("{:width$}", rows[index].text);
                rows[index].into_next = true;
                rows[index + 1].from_previous = true;
            }
            assert!(markers.ended_in(&rows), "{width} {last}");
            for window in [1, 2, 4096] {
                assert_eq!(
                    scanned(&markers, &rows, window),
                    Captured {
                        status: Some(12),
                        output: Some("one\n\ntwo\n".to_owned()),
                        truncated: false,
                    },
                    "{width} {last} {window}"
                );
            }
        }
    }
}

#[test]
fn cmd_runs_a_line_bare_unless_if_or_for_would_take_the_end_marker() {
    for (command, block) in [
        ("echo %PARENS%", false),
        ("echo (a)", false),
        ("dir missing || echo failed", false),
        ("echo \"if\" ^& echo verify", false),
        ("if 1==1 (echo yes) else (echo no)", true),
        ("IF not exist x echo missing", true),
        ("for %i in (1 2) do @echo %i", true),
        ("set x=1& if defined x echo set", true),
        ("echo ^\"if", true),
    ] {
        let run = if block {
            format!("({command})")
        } else {
            command.to_owned()
        };
        let line = wrapped(CaptureShell::Cmd, RUN, command);
        assert!(line.contains(&format!("]& {run}& echo.")), "{command}");
    }
}

#[test]
fn output_past_the_bound_keeps_its_newest_lines() {
    let markers = Markers::new(RUN);
    let text = (0..20_000)
        .map(|line| format!("{line:0>99}\n"))
        .collect::<String>();
    let rows = screen(&printed("$ echoed", &text, "0"), 80, false);
    let found = scanned(&markers, &rows, 3276);
    let output = found.output.unwrap();
    assert!(found.truncated);
    assert!(output.len() <= MAXIMUM_OUTPUT_BYTES);
    assert!(output.len() > MAXIMUM_OUTPUT_BYTES / 2);
    assert!(text.ends_with(&output));
    // The kept text starts at the beginning of a printed line.
    assert!(text.contains(&format!("\n{output}")));
}
