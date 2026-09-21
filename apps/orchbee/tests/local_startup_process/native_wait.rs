//! Real-Site witnesses for `terminal wait-output`: the public CLI holds a
//! read-only viewer against a real Worker/Host and resolves on literal text
//! already retained, text produced later, scrollback-only rows, and
//! UTF-8 soft-wrapped logical lines — with the timeout and signal-cancelled
//! exits the command contracts. The controlling TUI is never disturbed and
//! the session stays usable for further queries afterwards.

use super::native_cli_query::{expect_success, run_json, validate, validators};
use super::*;
use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;
use std::process::{Child, Output};

/// A spawned CLI child that is always reaped: any `?` early return still
/// kills and waits it instead of leaking a zombie.
struct Reap(Child);

impl Reap {
    fn spawn(fixture: &Fixture, arguments: &[&str]) -> Self {
        let child = Command::new(fixture.cohort.join("orchbee"))
            .args(arguments)
            .env("ORCHBEE_RUNTIME_BASE", &fixture.base)
            .env("LANG", "en_US.UTF-8")
            .env_remove("LC_ALL")
            .env_remove("LC_MESSAGES")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|error| panic!("spawn wait CLI {arguments:?}: {error}"));
        Self(child)
    }

    /// Wait for the child up to `deadline`; a still-running child at the
    /// deadline is a test failure, not a hang.
    fn wait_with_output(mut self, deadline: Instant) -> Output {
        loop {
            if let Some(status) = self.0.try_wait().expect("poll wait CLI") {
                let mut output = Output {
                    status,
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                };
                if let Some(mut stdout) = self.0.stdout.take() {
                    io::Read::read_to_end(&mut stdout, &mut output.stdout)
                        .expect("drain wait CLI stdout");
                }
                if let Some(mut stderr) = self.0.stderr.take() {
                    io::Read::read_to_end(&mut stderr, &mut output.stderr)
                        .expect("drain wait CLI stderr");
                }
                self.0.wait().expect("reap wait CLI");
                return output;
            }
            assert!(Instant::now() < deadline, "wait CLI did not exit");
            thread::sleep(Duration::from_millis(25));
        }
    }
}

impl Drop for Reap {
    fn drop(&mut self) {
        if self.0.try_wait().is_ok_and(|status| status.is_none()) {
            let _result = self.0.kill();
        }
        let _result = self.0.wait();
    }
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

fn wait_output(
    fixture: &Fixture,
    terminal_id: &str,
    arguments: &[&str],
) -> (Output, serde_json::Value) {
    let mut argv = vec!["terminal", "wait-output", terminal_id];
    argv.extend_from_slice(arguments);
    argv.extend_from_slice(&["--format", "json"]);
    run_json(fixture, &argv)
}

/// Text already retained matches immediately — the common first case. The
/// result envelope validates against the published schema and carries the
/// honest observation record; the controlling TUI is untouched.
#[test]
fn wait_output_matches_existing_retained_text_over_the_public_cli() -> TestResult {
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
    // The marker is produced output, never part of the typed command line.
    let checkpoint = client.checkpoint();
    client.send(b"printf 'EX%sISTING\\n' 'E'\r");
    client.wait_terminal_output_after(
        checkpoint,
        Instant::now() + OBSERVATION_TIMEOUT,
        "EXEISTING",
    );
    let terminal_id = terminal_ids(&fixture).remove(0);

    let (output, document) = wait_output(&fixture, &terminal_id, &["--match", "EXEISTING"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "wait on existing text: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    validate(
        &validators(&fixture),
        "orchbee.terminal.wait-output",
        &output,
    );
    let result = &document["result"];
    assert_eq!(result["matched"], true);
    assert_eq!(result["match"]["excerpt"], "EXEISTING");
    assert_eq!(result["match"]["excerptTruncated"], false);
    assert_eq!(result["observed"]["timeoutMs"], 30_000);
    assert_eq!(result["observed"]["requestedRows"], 1000);
    assert!(
        result["observed"]["completedObservations"]
            .as_u64()
            .unwrap()
            >= 1
    );
    assert!(result["observed"]["attachmentId"].as_str().is_some());

    // Read-only: the same shell still takes input afterwards.
    let checkpoint = client.checkpoint();
    client.send(b"printf 'AF%sER\\n' 'T'\r");
    client.wait_terminal_output_after(checkpoint, Instant::now() + OBSERVATION_TIMEOUT, "AFTER");
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}

/// The wait CLI is spawned before the producer runs: a miss-then-hit path
/// is expected but this margin is no proof the first observation already
/// completed — that case is carried by the scripted-peer
/// `terminal_wait_polls_until_the_text_appears` witness.
#[test]
fn wait_output_returns_when_the_text_appears_later() -> TestResult {
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

    let child = Reap::spawn(
        &fixture,
        &[
            "terminal",
            "wait-output",
            &terminal_id,
            "--match",
            "LATEOK",
            "--timeout",
            "20000",
            "--format",
            "json",
        ],
    );
    // A pragmatic margin so the producer does not outrun the spawn itself;
    // it is not a readiness proof — the CLI may legitimately match on its
    // first window read if the text lands early.
    thread::sleep(Duration::from_millis(1200));
    let checkpoint = client.checkpoint();
    client.send(b"printf 'LA%sEOK\\n' 'T'\r");
    client.wait_terminal_output_after(checkpoint, Instant::now() + OBSERVATION_TIMEOUT, "LATEOK");

    let output = child.wait_with_output(Instant::now() + Duration::from_secs(25));
    assert_eq!(
        output.status.code(),
        Some(0),
        "late match: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(document["result"]["match"]["excerpt"], "LATEOK");
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}

/// A marker retained only in scrollback — long since off the visible grid —
/// still matches: the searched window spans visible plus history as one
/// absolute range, not just the screen.
#[test]
fn wait_output_matches_a_marker_scrolled_into_history() -> TestResult {
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
    // 150 numbered markers scroll the head far above the 32-row viewport.
    let checkpoint = client.checkpoint();
    client.send(b"printf 'SC7%03d\\n' $(seq 1 150); printf 'SC%sDONE\\n' '7'\r");
    client.wait_terminal_output_after(
        checkpoint,
        Instant::now() + OBSERVATION_TIMEOUT * 4,
        "SC7DONE",
    );
    let terminal_id = terminal_ids(&fixture).remove(0);
    assert!(
        !client.contains("SC7001"),
        "the head marker must be off the visible grid"
    );

    let (output, document) = wait_output(&fixture, &terminal_id, &["--match", "SC7001"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "scrollback match: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    validate(
        &validators(&fixture),
        "orchbee.terminal.wait-output",
        &output,
    );
    assert_eq!(document["result"]["match"]["excerpt"], "SC7001");
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}

/// UTF-8 CJK output wide enough to soft-wrap joins into one logical line
/// before matching: the pattern straddles the wrap boundary, so no single
/// physical row contains it.
#[test]
fn wait_output_joins_a_utf8_soft_wrapped_logical_line() -> TestResult {
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
    // The real content width comes from the live frame geometry — the TUI
    // outer frame is wider than the terminal panel. Exactly `columns` W's
    // fill the first physical row with no half-wide slot left, then the
    // CJK tail lands on the next row: 'W目' exists only across the wrap.
    let terminal_id = terminal_ids(&fixture).remove(0);
    let (output, read) = run_json(
        &fixture,
        &["terminal", "read", &terminal_id, "--format", "json"],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "terminal read for geometry: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let columns = read["result"]["frame"]["geometry"]["columns"]
        .as_u64()
        .expect("frame geometry columns") as usize;
    let checkpoint = client.checkpoint();
    client.send(format!("printf 'W%.0s' $(seq 1 {columns}); printf '目标OK\\n'\r").as_bytes());
    // The observer stores wide-character continuation cells as blanks, so
    // the rendered frame shows the CJK output spaced; the ASCII tail is the
    // reliable arrival needle while the product assertion stays on the
    // complete logical line below.
    client.wait_terminal_output_after(checkpoint, Instant::now() + OBSERVATION_TIMEOUT, "OK");

    let (output, document) = wait_output(&fixture, &terminal_id, &["--match", "W目标OK"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "wrap-boundary match: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    validate(
        &validators(&fixture),
        "orchbee.terminal.wait-output",
        &output,
    );
    let result = &document["result"];
    assert_eq!(
        result["match"]["excerpt"],
        format!("{}目标OK", "W".repeat(columns))
    );
    // The joined logical line spans two physical rows.
    let start: u64 = result["match"]["rowStart"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let end: u64 = result["match"]["rowEnd"].as_str().unwrap().parse().unwrap();
    assert_eq!(end, start + 1, "the match straddles the wrap boundary");
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}

/// A marker that never arrives ends in the typed timeout: exit 7, the wait
/// stage, the caller's total budget, and the honest cleanup fact.
#[test]
fn wait_output_short_timeout_is_exit_seven() -> TestResult {
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

    let (output, failure) = wait_output(
        &fixture,
        &terminal_id,
        &["--match", "NEVER-APPEARS-7X", "--timeout", "5000"],
    );
    assert_eq!(output.status.code(), Some(7));
    validate(&validators(&fixture), "orchbee.cli.failure", &output);
    assert_eq!(failure["failure"]["kind"], "timeout");
    let details = &failure["failure"]["details"];
    assert_eq!(details["stage"], "wait");
    assert_eq!(details["timeoutMs"], 5000);
    assert!(
        details["observed"]["completedObservations"]
            .as_u64()
            .is_some(),
        "timeout details carry the observation count: {details}"
    );
    assert!(
        matches!(
            details["cleanup"].as_str(),
            Some("requested") | Some("unconfirmed") | Some("confirmed")
        ),
        "cleanup fact is one of the honest states: {details}"
    );
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}

/// SIGINT cancels the wait cooperatively: the process exits 130 with the
/// `cancelled` envelope — never killed by the signal — and the session
/// answers a fresh query afterwards. A successful query is not a
/// slot-release proof; cleanup facts are guarded by the scripted-peer
/// detach evidence instead.
#[test]
fn wait_output_sigint_exits_130_and_the_session_stays_usable() -> TestResult {
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

    let mut child = Reap::spawn(
        &fixture,
        &[
            "terminal",
            "wait-output",
            &terminal_id,
            "--match",
            "NEVER-APPEARS-9X",
            "--timeout",
            "60000",
            "--format",
            "json",
        ],
    );
    // The preamble and handler install complete well inside this margin.
    thread::sleep(Duration::from_millis(1500));
    assert!(child.0.try_wait()?.is_none(), "wait CLI is still waiting");
    kill(Pid::from_raw(child.0.id() as i32), Signal::SIGINT)?;
    let output = child.wait_with_output(Instant::now() + Duration::from_secs(15));
    assert_eq!(
        output.status.code(),
        Some(130),
        "cancelled wait exits 130, not killed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let failure: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(failure["schema"], "orchbee.cli.failure");
    assert_eq!(failure["failure"]["kind"], "cancelled");
    assert_eq!(failure["failure"]["details"]["stage"], "wait");

    // The session answers a fresh query after the cancel — evidence the
    // carrier survived, not that the viewer slot itself was released.
    let (output, _) = run_json(&fixture, &["terminal", "list", "--format", "json"]);
    assert_eq!(output.status.code(), Some(0), "session usable after cancel");
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}
