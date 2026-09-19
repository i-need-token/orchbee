//! Public `terminal send` witnesses staged for the integrated run: each case
//! drives only the staged `orchbee` binary and a live controlling TUI on an
//! isolated cohort. The send invocation, `terminal read`, `terminal list`,
//! and `tab create` stay strictly on the shipped public surface; every
//! execution proof is an independent shell-produced file or the public read
//! surface — never a raw VT echo search and never a resend-until-green loop.
//! On this base the `send` subcommand does not exist yet, so this file is
//! compile-checked only and recorded EXPECTED-RED until A9 integrates Send.

use super::native_cli_query::{run_cli, run_json};
use super::*;
use std::process::Output;

/// One public `terminal send` invocation that must be admitted and accepted:
/// `--format json` keeps the receipt machine-readable while the exit code is
/// the authoritative acceptance signal.
fn send(fixture: &Fixture, terminal_id: &str, arguments: &[&str]) -> Output {
    let mut full: Vec<&str> = vec!["terminal", "send", terminal_id];
    full.extend_from_slice(arguments);
    full.push("--format");
    full.push("json");
    let output = run_cli(fixture, &full);
    assert_eq!(
        output.status.code(),
        Some(0),
        "terminal send {full:?} was not accepted: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    // SEAM: the receipt document is validated against A1's final send receipt
    // schema at integration (`validate(&validators(...), <send schema id>,
    // &output)`); this witness deliberately binds no invented field names.
    output
}

/// Bound terminal ids through the public `terminal list` surface.
fn terminal_ids(fixture: &Fixture) -> Vec<String> {
    let (output, document) = run_json(fixture, &["terminal", "list", "--format", "json"]);
    assert_eq!(output.status.code(), Some(0));
    document["result"]["terminals"]
        .as_array()
        .expect("terminal list rows")
        .iter()
        .map(|row| row["terminalId"].as_str().expect("terminalId").to_owned())
        .collect()
}

fn only_terminal(fixture: &Fixture) -> String {
    let ids = terminal_ids(fixture);
    assert_eq!(ids.len(), 1, "exactly one bound terminal: {ids:?}");
    ids.into_iter().next().expect("one terminal")
}

/// A second bound terminal created through the public `tab create` surface.
/// The command returns only once the launch reports `started`, so the new id
/// is ready for admission when it appears on the public list.
fn create_public_terminal(fixture: &Fixture, label: &str, excluded: &str) -> String {
    let (output, document) = run_json(
        fixture,
        &[
            "tab",
            "create",
            "--standalone",
            "--label",
            label,
            "--cols",
            "80",
            "--rows",
            "24",
            "--cell-width",
            "9",
            "--cell-height",
            "18",
            "--format",
            "json",
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "public tab create: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(document["result"]["terminal"]["outcome"], "started");
    let deadline = Instant::now() + OBSERVATION_TIMEOUT;
    loop {
        if let Some(id) = terminal_ids(fixture).into_iter().find(|id| id != excluded) {
            return id;
        }
        assert!(
            Instant::now() < deadline,
            "the publicly created terminal never became listable"
        );
        thread::yield_now();
    }
}

/// The live grid through the public `terminal read` surface — the sanctioned
/// observation channel for what the send made visible.
fn read_rows(fixture: &Fixture, terminal_id: &str) -> String {
    let (output, document) = run_json(
        fixture,
        &["terminal", "read", terminal_id, "--format", "json"],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "terminal read {terminal_id}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    document["result"]["content"]["rows"]
        .as_array()
        .expect("read rows")
        .iter()
        .filter_map(|row| row["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// One shell line that both prints a visible token and produces a proof file:
/// the file is the independent execution proof and the token lands on the
/// public read surface. Arguments are single-quoted literals, so the line
/// stays free of embedded double quotes inside the CLI `--text` argument.
fn producer(fixture: &Fixture, token: &str, filename: &str, content: &str) -> String {
    format!(
        "printf '%s\\n' '{token}'; printf '%s' '{content}' > {}",
        fixture.base.join(filename).display()
    )
}

/// A shell line that records the executing shell's pid — the honest proof
/// that the same shell kept running after an interruption.
fn pid_producer(fixture: &Fixture, filename: &str) -> String {
    format!(
        "printf '%s' \"$$\" > {}",
        fixture.base.join(filename).display()
    )
}

/// Bounded negative observation: the file must stay absent for the whole
/// window while the live client keeps its frames flowing.
fn assert_absent_while(fixture: &Fixture, filename: &str, client: &mut Client, window: Duration) {
    let end = Instant::now() + window;
    while Instant::now() < end {
        client.collect();
        assert!(
            !fixture.base.join(filename).exists(),
            "{filename} must not exist before the semantic key"
        );
        thread::yield_now();
    }
}

/// A public send of one text record while the controlling TUI holds the same
/// terminal executes against the real shell, and the TUI's interactive input
/// and resize keep working afterwards — the coexistence contract.
#[test]
fn send_text_executes_while_tui_keeps_input_and_resize() -> TestResult {
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

    // Baseline: the TUI-owned shell answers interactive input with its pid.
    client.send(shell_identity_command(&fixture, "interactive", "shell-pid", true).as_bytes());
    let identity = wait_file(
        &fixture.base.join("shell-pid"),
        &mut [&mut client],
        Instant::now() + OBSERVATION_TIMEOUT,
    );
    let shell_pid = require_shell_identity(&identity, "interactive");

    // Baseline: the TUI's own resize path settles before any send traffic.
    let checkpoint = client.checkpoint();
    client.resize_rows(30);
    client.wait_resized_ready_after(checkpoint, Instant::now() + OBSERVATION_TIMEOUT, 30, 1);

    let terminal = only_terminal(&fixture);
    send(
        &fixture,
        &terminal,
        &[
            "--text",
            &producer(&fixture, "OB_SEND_LANDED", "send-proof", "from-send"),
            "--enter",
        ],
    );

    // Real execution is the shell-produced file; the public read surface shows
    // the printed token through the same grid the controlling TUI draws.
    assert_eq!(
        wait_file(
            &fixture.base.join("send-proof"),
            &mut [&mut client],
            Instant::now() + OBSERVATION_TIMEOUT,
        ),
        "from-send"
    );
    assert!(
        read_rows(&fixture, &terminal).contains("OB_SEND_LANDED"),
        "the send's visible output must reach the public read surface"
    );

    // Afterwards the unchanged shell still answers the TUI's interactive input.
    client.send(pid_producer(&fixture, "shell-after").as_bytes());
    assert_eq!(
        wait_file(
            &fixture.base.join("shell-after"),
            &mut [&mut client],
            Instant::now() + OBSERVATION_TIMEOUT,
        ),
        shell_pid.to_string(),
        "the controlling TUI must keep the exact original shell"
    );

    // And interactive resize still settles after the send traffic.
    let checkpoint = client.checkpoint();
    client.resize_rows(26);
    client.wait_resized_ready_after(checkpoint, Instant::now() + OBSERVATION_TIMEOUT, 26, 1);
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}

/// A second terminal created through the public `tab create` surface takes its
/// own send: one send per terminal, each producing only its own marker.
#[test]
fn sends_to_two_terminals_keep_independent_markers() -> TestResult {
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
    let terminal_a = only_terminal(&fixture);
    let terminal_b = create_public_terminal(&fixture, "send-b", &terminal_a);
    assert_ne!(terminal_a, terminal_b);

    send(
        &fixture,
        &terminal_a,
        &[
            "--text",
            &producer(&fixture, "OB_SEND_A", "marker-a", "marker-a"),
            "--enter",
        ],
    );
    send(
        &fixture,
        &terminal_b,
        &[
            "--text",
            &producer(&fixture, "OB_SEND_B", "marker-b", "marker-b"),
            "--enter",
        ],
    );

    // Each marker lands only in its own terminal's file: no cross-writing.
    let deadline = Instant::now() + OBSERVATION_TIMEOUT;
    assert_eq!(
        wait_file(&fixture.base.join("marker-a"), &mut [&mut client], deadline),
        "marker-a"
    );
    assert_eq!(
        wait_file(&fixture.base.join("marker-b"), &mut [&mut client], deadline),
        "marker-b"
    );
    assert!(read_rows(&fixture, &terminal_a).contains("OB_SEND_A"));
    assert!(read_rows(&fixture, &terminal_b).contains("OB_SEND_B"));
    assert!(!read_rows(&fixture, &terminal_a).contains("OB_SEND_B"));
    assert!(!read_rows(&fixture, &terminal_b).contains("OB_SEND_A"));

    client.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}

/// A text record without the carriage return only stages the line: nothing
/// executes until one semantic `enter` key completes it, proving the send
/// boundary is one input record and not an implicit execution.
#[test]
fn text_without_enter_stages_then_one_semantic_enter_executes() -> TestResult {
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
    let terminal = only_terminal(&fixture);

    send(
        &fixture,
        &terminal,
        &[
            "--text",
            &producer(&fixture, "OB_STAGED_RAN", "staged-proof", "staged"),
        ],
    );
    // The staged text must not execute on its own across a bounded window.
    assert_absent_while(
        &fixture,
        "staged-proof",
        &mut client,
        Duration::from_millis(600),
    );

    send(&fixture, &terminal, &["--key", "enter"]);
    assert_eq!(
        wait_file(
            &fixture.base.join("staged-proof"),
            &mut [&mut client],
            Instant::now() + OBSERVATION_TIMEOUT,
        ),
        "staged"
    );
    assert!(read_rows(&fixture, &terminal).contains("OB_STAGED_RAN"));
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}

/// `ctrl-c` interrupts only the target terminal's fixture-owned foreground
/// command: the other terminal and the target's original shell both continue.
#[test]
fn ctrl_c_interrupts_only_the_target_terminal_foreground() -> TestResult {
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
    client.send(shell_identity_command(&fixture, "interrupt", "shell-pid", true).as_bytes());
    let identity = wait_file(
        &fixture.base.join("shell-pid"),
        &mut [&mut client],
        Instant::now() + OBSERVATION_TIMEOUT,
    );
    let shell_pid = require_shell_identity(&identity, "interrupt");
    let terminal_a = only_terminal(&fixture);
    let terminal_b = create_public_terminal(&fixture, "send-peer", &terminal_a);

    // A's shell runs a fixture-owned foreground command whose `;` tail only
    // executes when the foreground command actually terminates.
    send(
        &fixture,
        &terminal_a,
        &[
            "--text",
            &format!(
                "sleep 300; printf '%s' 'interrupted' > {}",
                fixture.base.join("a-interrupted").display()
            ),
            "--enter",
        ],
    );
    // Let the accepted input reach the shell and start the foreground job
    // before the interrupt — the same post-submit settle the harness uses.
    thread::sleep(Duration::from_millis(500));

    // While A is blocked, the independent terminal still takes its own send.
    send(
        &fixture,
        &terminal_b,
        &[
            "--text",
            &producer(&fixture, "OB_PEER_ALIVE", "b-alive", "peer-alive"),
            "--enter",
        ],
    );
    assert_eq!(
        wait_file(
            &fixture.base.join("b-alive"),
            &mut [&mut client],
            Instant::now() + OBSERVATION_TIMEOUT,
        ),
        "peer-alive"
    );

    send(&fixture, &terminal_a, &["--key", "ctrl-c"]);
    // The foreground job was genuinely interrupted: its tail produced the file
    // long before the 300-second natural exit could.
    assert_eq!(
        wait_file(
            &fixture.base.join("a-interrupted"),
            &mut [&mut client],
            Instant::now() + OBSERVATION_TIMEOUT,
        ),
        "interrupted"
    );

    // A's original shell — same pid — continues after the interrupt.
    send(
        &fixture,
        &terminal_a,
        &["--text", &pid_producer(&fixture, "a-after"), "--enter"],
    );
    assert_eq!(
        wait_file(
            &fixture.base.join("a-after"),
            &mut [&mut client],
            Instant::now() + OBSERVATION_TIMEOUT,
        ),
        shell_pid.to_string(),
        "the interrupted terminal keeps its exact original shell"
    );
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}
