use super::native_cli_query::{expect_success, run_cli, run_json, validate, validators};
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

fn content_text(document: &serde_json::Value) -> &str {
    document["result"]["content"]["text"]
        .as_str()
        .expect("source text")
}

fn row_number(document: &serde_json::Value, field: &str) -> u64 {
    document["result"]["content"][field]
        .as_str()
        .expect("row coordinate")
        .parse()
        .expect("decimal row")
}

fn terminal_ids(fixture: &Fixture) -> Vec<String> {
    let (output, terminals) = run_json(fixture, &["terminal", "list", "--format", "json"]);
    expect_success(&["terminal", "list", "--format", "json"], &output);
    terminals["result"]["terminals"]
        .as_array()
        .expect("terminal list rows")
        .iter()
        .map(|row| row["terminalId"].as_str().expect("terminalId").to_string())
        .collect()
}

fn recent_read(fixture: &Fixture, terminal_id: &str, rows: u16) -> (Output, serde_json::Value) {
    let request = rows.to_string();
    run_json(
        fixture,
        &[
            "terminal",
            "read",
            terminal_id,
            "--source",
            "recent",
            "--lines",
            &request,
            "--format",
            "json",
        ],
    )
}

/// A live controlling TUI produces more than one screen. Recent reads span
/// retained history and the live screen, preserve exact coordinates, and
/// release their viewers without taking the TUI's input or resize control.
#[test]
fn recent_read_returns_recent_retained_tail_over_the_public_cli() -> TestResult {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::with_workspace();
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

    let (output, recent) = recent_read(&fixture, &terminal_id, 12);
    expect_success(
        &["terminal", "read", "--source", "recent", "--lines", "12"],
        &output,
    );
    validate(&validators, "orchbee.terminal.output", &output);
    let result = &recent["result"];
    assert_eq!(result["selection"]["source"], "recent");
    assert_eq!(result["selection"]["lines"], 12);
    assert_eq!(result["observed"]["windowRows"], 12);
    assert_eq!(result["observed"]["capacityOmittedRows"], "0");
    assert_eq!(result["observed"]["alternateScreen"], false);
    assert!(result["observed"]["revision"].as_str().is_some());
    assert!(result["observed"]["generation"].as_str().is_some());
    assert_eq!(result["content"]["physicalRows"], 12);
    let first = row_number(&recent, "firstRow");
    let last = row_number(&recent, "lastRow");
    assert!(first > 0, "producer scrolls beyond the selected tail");
    assert_eq!(last - first + 1, 12);
    assert_eq!(result["observed"]["windowStart"], first.to_string());
    let text = content_text(&recent);
    assert!(text.contains(DONE_SENTINEL), "{text}");
    let numbers: Vec<_> = text
        .lines()
        .filter_map(|line| marker_number(line, "HX7"))
        .collect();
    assert!(numbers.len() > 1, "{text}");
    assert_eq!(numbers.last(), Some(&150));
    for pair in numbers.windows(2) {
        assert_eq!(pair[1], pair[0] + 1);
    }

    // The visible source and recent source both include the last produced
    // marker: the current contract includes live output, not just scrollback.
    let (output, visible) = run_json(
        &fixture,
        &[
            "terminal",
            "read",
            &terminal_id,
            "--source",
            "visible",
            "--format",
            "json",
        ],
    );
    expect_success(&["terminal", "read", "--source", "visible"], &output);
    validate(&validators, "orchbee.terminal.output", &output);
    assert!(content_text(&visible).contains("HX7150"));

    let (output, repeat) = recent_read(&fixture, &terminal_id, 12);
    expect_success(&["terminal", "read"], &output);
    assert_eq!(repeat["result"]["content"], recent["result"]["content"]);
    assert_eq!(repeat["result"]["terminal"], recent["result"]["terminal"]);

    // A wider request includes every produced marker and ends at the same
    // current output boundary, with no imaginary rows before coordinate zero.
    let (output, capped) = recent_read(&fixture, &terminal_id, 1000);
    expect_success(&["terminal", "read", "--lines", "1000"], &output);
    validate(&validators, "orchbee.terminal.output", &output);
    assert_eq!(row_number(&capped, "firstRow"), 0);
    assert_eq!(row_number(&capped, "lastRow"), last);
    assert_eq!(capped["result"]["observed"]["windowRows"], last + 1);
    let all_numbers: Vec<_> = content_text(&capped)
        .lines()
        .filter_map(|line| marker_number(line, "HX7"))
        .collect();
    assert_eq!(all_numbers, (1..=150).collect::<Vec<u64>>());

    // The TUI still owns input: a fresh marker typed after all the reads is
    // accepted by the same shell and observed through the same viewport.
    let checkpoint = client.checkpoint();
    client.send(b"printf 'LV%sOK\\n' 'E'\r");
    client.wait_terminal_output_after(checkpoint, producer_deadline(), "LVEOK");
    assert_eq!(terminal_ids(&fixture), vec![terminal_id]);
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}

/// An explicit zero count produces an empty sample even on a live terminal.
#[test]
fn recent_read_reports_an_empty_sample_for_zero_lines() -> TestResult {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::with_workspace();
    let mut client = Client::spawn(&fixture, "en_US.UTF-8");
    client.wait_ready_after(
        0,
        Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
        1,
        "1",
        ReadyMode::Controlling,
    );
    let terminal_id = terminal_ids(&fixture).remove(0);

    let (output, history) = recent_read(&fixture, &terminal_id, 0);
    assert_eq!(
        output.status.code(),
        Some(0),
        "empty history read: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    validate(&validators(&fixture), "orchbee.terminal.output", &output);
    let result = &history["result"];
    assert_eq!(result["selection"]["lines"], 0);
    assert_eq!(result["observed"]["windowRows"], 0);
    assert_eq!(result["content"]["physicalRows"], 0);
    assert_eq!(result["content"]["text"], "");
    assert!(result["content"]["firstRow"].is_null());
    assert!(result["content"]["lastRow"].is_null());
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}

/// Unknown (0,0) pixels: the host legally reports no pixel extents and the
/// read-only history path must not invent any — the same public flow is
/// exercised end to end under that geometry report.
#[test]
fn recent_read_succeeds_when_host_reports_unknown_pixels() -> TestResult {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::with_workspace();
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

    let (output, history) = recent_read(&fixture, &terminal_id, 8);
    assert_eq!(
        output.status.code(),
        Some(0),
        "unknown-pixel history read: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    validate(&validators(&fixture), "orchbee.terminal.output", &output);
    assert_eq!(history["result"]["observed"]["windowRows"], 8);
    assert_eq!(history["result"]["content"]["physicalRows"], 8);
    assert_eq!(
        row_number(&history, "lastRow") - row_number(&history, "firstRow") + 1,
        8
    );
    let text = content_text(&history);
    assert!(text.contains(DONE_SENTINEL), "{text}");
    let numbers: Vec<_> = text
        .lines()
        .filter_map(|line| marker_number(line, "PX4"))
        .collect();
    assert_eq!(numbers.last(), Some(&60));
    for pair in numbers.windows(2) {
        assert_eq!(pair[1], pair[0] + 1);
    }
    let checkpoint = client.checkpoint();
    client.send(b"printf 'PX%sD\\n' 'E'\r");
    client.wait_terminal_output_after(checkpoint, producer_deadline(), "PXED");
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}

/// Retired flags and malformed row counts fail before accessing a session.
#[test]
fn recent_read_rejects_retired_duplicate_and_out_of_range_options() -> TestResult {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::with_workspace();
    let id = "ffffffffffffffffffffffffffffffff";
    for options in [
        ["--history"].as_slice(),
        ["--rows", "5"].as_slice(),
        ["--unwrap"].as_slice(),
        ["--history", "--rows", "5"].as_slice(),
        ["--lines", "5", "--lines", "7"].as_slice(),
        ["--lines", "-1"].as_slice(),
        ["--lines", "4294967296"].as_slice(),
    ] {
        let mut arguments = vec!["terminal", "read", id];
        arguments.extend_from_slice(options);
        let output = run_cli(&fixture, &arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
    }
    Ok(())
}
