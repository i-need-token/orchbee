use super::native_cli_query::{run_cli, run_json, validate, validators};
use super::pty_client::{COLUMNS, ROWS};
use super::*;
use std::process::Output;

/// One producer pass emits `count` numbered markers plus a completion
/// sentinel. The literal marker text never appears inside the typed command
/// (the shell expands the `seq` operands itself), so a screen-observer hit
/// always witnesses produced output rather than the echoed command line.
fn producer_command(prefix: &str, count: usize) -> String {
    format!("printf '{prefix}%03d\\n' $(seq 1 {count}) ; printf 'DN%s\\n' 'E'")
}

/// The completion sentinel; the typed fragment `DN%s` cannot collide.
const DONE_SENTINEL: &str = "DNE";

fn producer_deadline() -> Instant {
    Instant::now() + Duration::from_secs(30)
}

fn marker_number(text: &str, prefix: &str) -> Option<u64> {
    text.trim_end().strip_prefix(prefix)?.parse().ok()
}

fn scrollback_rows(document: &serde_json::Value) -> u64 {
    document["result"]["history"]["scrollbackRows"]
        .as_str()
        .expect("scrollbackRows decimal string")
        .parse()
        .expect("scrollbackRows parses")
}

fn terminal_ids(fixture: &Fixture) -> Vec<String> {
    let (output, terminals) = run_json(fixture, &["terminal", "list", "--format", "json"]);
    assert_eq!(output.status.code(), Some(0));
    terminals["result"]["terminals"]
        .as_array()
        .expect("terminal list rows")
        .iter()
        .map(|row| row["terminalId"].as_str().expect("terminalId").to_string())
        .collect()
}

fn history_read(fixture: &Fixture, terminal_id: &str, rows: u16) -> (Output, serde_json::Value) {
    let request = rows.to_string();
    run_json(
        fixture,
        &[
            "terminal",
            "read",
            terminal_id,
            "--history",
            "--rows",
            &request,
            "--format",
            "json",
        ],
    )
}

/// A live controlling TUI produces more output than one screen holds; the
/// public `terminal read --history --rows` path then returns the most recent
/// retained tail (not an older window), honors the row pre-cap, reports exact
/// decimal-string coordinates, and releases its read viewer without touching
/// the TUI's input or resize control.
#[test]
fn history_read_returns_recent_retained_tail_over_the_public_cli() -> TestResult {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::new();
    let mut client = Client::spawn(&fixture, "en_US.UTF-8");
    client.wait_ready_after(
        0,
        Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
        1,
        "1",
        ReadyMode::Controlling,
    );

    // Produce 150 numbered markers so most of the run is retained above the
    // 32-row viewport, then emit the completion sentinel. The shell is proven
    // input-ready by the ready wait; the producer is sent exactly once.
    let checkpoint = client.checkpoint();
    client.send(producer_command("HX7", 150).as_bytes());
    client.send(b"\r");
    client.wait_terminal_output_after(checkpoint, producer_deadline(), DONE_SENTINEL);

    let ids = terminal_ids(&fixture);
    assert_eq!(ids.len(), 1, "the live shell binds one terminal: {ids:?}");
    let terminal_id = ids[0].clone();
    let validators = validators(&fixture);

    // The recent tail: the last returned row must be the retained row
    // immediately above the current viewport (index S - 1).
    let (output, history) = history_read(&fixture, &terminal_id, 12);
    assert_eq!(
        output.status.code(),
        Some(0),
        "history read: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    validate(&validators, "orchbee.terminal.history", &output);
    let result = &history["result"];
    let s = scrollback_rows(&history);
    assert!(s > 12, "the producer scrolled rows off the screen: S={s}");
    assert_eq!(result["history"]["startRow"], (s - 12).to_string());
    assert_eq!(result["history"]["windowRows"], 12);
    assert_eq!(result["history"]["requestedRows"], 12);
    assert!(result["history"]["revision"].as_str().is_some());
    assert!(result["history"]["generation"].as_str().is_some());
    assert_eq!(result["history"]["alternateScreen"], false);
    let rows = result["content"]["rows"].as_array().expect("history rows");
    assert_eq!(rows.len(), 12);
    assert_eq!(result["content"]["rowCount"], 12);
    assert_eq!(result["content"]["truncated"], false);
    assert_eq!(result["content"]["omittedRows"], 0);
    let mut numbers = Vec::with_capacity(12);
    for (offset, row) in rows.iter().enumerate() {
        assert_eq!(row["index"], (s - 12 + offset as u64).to_string());
        numbers.push(
            marker_number(row["text"].as_str().expect("row text"), "HX7")
                .unwrap_or_else(|| panic!("tail row is a produced marker: {row}")),
        );
    }
    assert_eq!(rows[11]["index"], (s - 1).to_string());
    for pair in numbers.windows(2) {
        assert_eq!(
            pair[1],
            pair[0] + 1,
            "contiguous retained rows: {numbers:?}"
        );
    }

    // The window is the *recent* tail, not an older slice: the last retained
    // row is the marker immediately preceding the topmost marker still
    // visible in the live frame.
    let (output, frame) = run_json(
        &fixture,
        &["terminal", "read", &terminal_id, "--format", "json"],
    );
    assert_eq!(output.status.code(), Some(0));
    let topmost_visible = frame["result"]["content"]["rows"]
        .as_array()
        .expect("frame rows")
        .iter()
        .filter_map(|row| marker_number(row["text"].as_str().unwrap_or(""), "HX7"))
        .min()
        .expect("the screen still shows produced markers");
    assert_eq!(*numbers.last().unwrap(), topmost_visible - 1);
    assert_eq!(
        frame["result"]["frame"]["revision"],
        result["history"]["revision"]
    );
    assert_eq!(
        frame["result"]["frame"]["generation"],
        result["history"]["generation"]
    );

    // A second identical read is a fresh viewer: same window, same identity,
    // no carried state and no stolen control.
    let (output, repeat) = history_read(&fixture, &terminal_id, 12);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(repeat["result"], history["result"]);

    // The 1000-row request is pre-capped to S: the whole retained buffer is
    // returned, ending at the same recent boundary.
    let (output, capped) = history_read(&fixture, &terminal_id, 1000);
    assert_eq!(output.status.code(), Some(0));
    let capped_rows = capped["result"]["content"]["rows"]
        .as_array()
        .expect("capped rows");
    assert_eq!(capped_rows.len() as u64, s);
    assert_eq!(capped["result"]["content"]["rowCount"], s);
    assert_eq!(capped["result"]["history"]["windowRows"], s);
    assert_eq!(capped_rows[0]["index"], "0");
    assert_eq!(capped_rows[s as usize - 1]["index"], (s - 1).to_string());

    // The TUI still owns input: a fresh marker typed after all the reads is
    // accepted by the same shell and observed through the same viewport.
    let checkpoint = client.checkpoint();
    client.send(b"printf 'LV%sOK\\n' 'E'\r");
    client.wait_terminal_output_after(checkpoint, producer_deadline(), "LVEOK");
    assert_eq!(terminal_ids(&fixture), vec![terminal_id]);
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}

/// A freshly bound terminal has an empty scrollback: `S = 0` returns the
/// empty window with exact zero coordinates.
#[test]
fn history_read_reports_empty_scrollback_for_a_fresh_terminal() -> TestResult {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::new();
    let mut client = Client::spawn(&fixture, "en_US.UTF-8");
    client.wait_ready_after(
        0,
        Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
        1,
        "1",
        ReadyMode::Controlling,
    );
    let terminal_id = terminal_ids(&fixture).remove(0);

    let (output, history) = history_read(&fixture, &terminal_id, 5);
    assert_eq!(
        output.status.code(),
        Some(0),
        "empty history read: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    validate(&validators(&fixture), "orchbee.terminal.history", &output);
    let result = &history["result"];
    assert_eq!(result["history"]["scrollbackRows"], "0");
    assert_eq!(result["history"]["startRow"], "0");
    assert_eq!(result["history"]["windowRows"], 0);
    assert_eq!(result["content"]["rows"].as_array().unwrap().len(), 0);
    assert_eq!(result["content"]["rowCount"], 0);
    assert_eq!(result["content"]["truncated"], false);
    assert_eq!(result["content"]["omittedRows"], 0);
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}

/// Unknown (0,0) pixels: the host legally reports no pixel extents and the
/// read-only history path must not invent any — the same public flow is
/// exercised end to end under that geometry report.
#[test]
fn history_read_succeeds_when_host_reports_unknown_pixels() -> TestResult {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::new();
    let mut client = Client::spawn_geometry(
        &fixture,
        "en_US.UTF-8",
        &fixture.base,
        COLUMNS as u16,
        ROWS as u16,
        false,
    );
    client.wait_ready_after(
        0,
        Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
        1,
        "1",
        ReadyMode::Controlling,
    );
    let checkpoint = client.checkpoint();
    client.send(producer_command("PX4", 60).as_bytes());
    client.send(b"\r");
    client.wait_terminal_output_after(checkpoint, producer_deadline(), DONE_SENTINEL);
    let terminal_id = terminal_ids(&fixture).remove(0);

    let (output, history) = history_read(&fixture, &terminal_id, 8);
    assert_eq!(
        output.status.code(),
        Some(0),
        "unknown-pixel history read: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    validate(&validators(&fixture), "orchbee.terminal.history", &output);
    let s = scrollback_rows(&history);
    assert!(s >= 8, "producer scrolled rows off: S={s}");
    let rows = history["result"]["content"]["rows"]
        .as_array()
        .expect("history rows");
    assert_eq!(rows.len(), 8);
    assert_eq!(rows[7]["index"], (s - 1).to_string());
    for row in rows {
        marker_number(row["text"].as_str().expect("row text"), "PX4")
            .unwrap_or_else(|| panic!("tail row is a produced marker: {row}"));
    }
    let checkpoint = client.checkpoint();
    client.send(b"printf 'PX%sD\\n' 'E'\r");
    client.wait_terminal_output_after(checkpoint, producer_deadline(), "PXED");
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}

/// The paired-flags contract rejects partial, duplicate, or out-of-range
/// history options during parse — before any session is touched.
#[test]
fn history_read_rejects_unpaired_duplicate_and_out_of_range_options() -> TestResult {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::new();
    let id = "ffffffffffffffffffffffffffffffff";

    let output = run_cli(&fixture, &["terminal", "read", id, "--history"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--rows"), "{stderr}");

    let output = run_cli(&fixture, &["terminal", "read", id, "--rows", "5"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--history"), "{stderr}");

    let output = run_cli(
        &fixture,
        &[
            "terminal",
            "read",
            id,
            "--history",
            "--rows",
            "5",
            "--rows",
            "7",
        ],
    );
    assert_eq!(output.status.code(), Some(2));

    for value in ["0", "1001"] {
        let output = run_cli(
            &fixture,
            &["terminal", "read", id, "--history", "--rows", value],
        );
        assert_eq!(output.status.code(), Some(2), "--rows {value}");
    }
    Ok(())
}
