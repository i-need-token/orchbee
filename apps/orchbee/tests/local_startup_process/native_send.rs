//! Public `terminal send` witnesses staged for the integrated run: each case
//! drives only the staged `orchbee` binary and a live controlling TUI on an
//! isolated cohort. The send invocation, `terminal read`, `terminal list`,
//! and `tab create` stay strictly on the shipped public surface; every
//! execution proof is an independent shell-produced file or the public read
//! surface — never a raw VT echo search and never a resend-until-green loop.
//! On this base the `send` subcommand does not exist yet, so this file is
//! compile-checked only and recorded EXPECTED-RED until A9 integrates Send.

use super::native_cli_query::{expect_success, run_cli, run_json, validate, validators};
use super::*;
use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
use rustix::process::{Pid, PidfdFlags, pidfd_open};
use std::os::fd::AsFd;

/// One public `terminal send` invocation that must be admitted and accepted:
/// the receipt is validated against the embedded `orchbee.terminal.send`
/// schema (A1's final contract, commit 8d4f7fd4) and must report the healthy
/// `accepted` + `confirmed` verdict with its physical incarnation.
fn send(fixture: &Fixture, terminal_id: &str, arguments: &[&str]) -> serde_json::Value {
    let mut full: Vec<&str> = vec!["terminal", "send", terminal_id];
    full.extend_from_slice(arguments);
    full.extend_from_slice(&["--format", "json"]);
    let output = run_cli(fixture, &full);
    assert_eq!(
        output.status.code(),
        Some(0),
        "terminal send {full:?} was not accepted: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    validate(&validators(fixture), "orchbee.terminal.send", &output);
    let document: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("send receipt document");
    let result = &document["result"];
    assert_eq!(result["action"], "terminal.send", "{document}");
    assert_eq!(result["inputStatus"], "accepted", "{document}");
    assert_eq!(result["cleanupStatus"], "confirmed", "{document}");
    assert_eq!(result["terminalId"], terminal_id, "{document}");
    let target = &result["target"];
    assert!(
        target["workerGeneration"]
            .as_str()
            .is_some_and(|generation| generation.parse::<u64>().is_ok_and(|value| value > 0)),
        "physical target must carry a positive worker generation: {document}"
    );
    for field in ["hostEpoch", "hostTerminalId"] {
        assert!(
            target[field]
                .as_str()
                .is_some_and(|id| id.len() == 32 && id.chars().all(|c| c.is_ascii_hexdigit())),
            "physical target must carry a 32-hex {field}: {document}"
        );
    }
    document
}

/// Text records admit exactly the UTF-8 text plus the folded carriage return;
/// both `recordBytes` and the Host-echoed `acceptedBytes` carry that count.
fn assert_text_bytes(receipt: &serde_json::Value, text: &str, enter: bool) {
    let result = &receipt["result"];
    let expected = (text.len() + usize::from(enter)) as u64;
    assert_eq!(result["payloadKind"], "text", "{receipt}");
    assert_eq!(result["recordBytes"], expected, "{receipt}");
    assert_eq!(result["acceptedBytes"], expected, "{receipt}");
}

/// Key records echo the semantic key name; `acceptedBytes` is the Host-encoded
/// count, which may legitimately be zero for a mode-ignored key.
fn assert_key_bytes(receipt: &serde_json::Value, key: &str) {
    let result = &receipt["result"];
    assert_eq!(result["payloadKind"], "key", "{receipt}");
    assert_eq!(result["key"], key, "{receipt}");
    assert!(
        result["acceptedBytes"].as_u64().is_some(),
        "accepted sends must echo a bounded byte count: {receipt}"
    );
}

/// Bound terminal ids through the public `terminal list` surface.
fn terminal_ids(fixture: &Fixture) -> Vec<String> {
    let (output, document) = run_json(fixture, &["terminal", "list", "--format", "json"]);
    expect_success(&["terminal", "list", "--format", "json"], &output);
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
/// public read surface. `visible` is a complete `printf` fragment whose final
/// marker is assembled from format + arguments — the input record itself
/// never contains the contiguous token, so an echo of the sent line can only
/// show the split scaffold, never satisfy the read assertion.
fn producer(fixture: &Fixture, visible: &str, filename: &str, content: &str) -> String {
    format!(
        "{visible}; printf '%s' '{content}' > {}",
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
    let line = producer(
        &fixture,
        "printf 'OB_%s_LANDED\\n' 'SEND'",
        "send-proof",
        "from-send",
    );
    let receipt = send(&fixture, &terminal, &["--text", &line, "--enter"]);
    assert_text_bytes(&receipt, &line, true);

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
    client.send(b"\r");
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

    let line_a = producer(
        &fixture,
        "printf 'OB_%s_A\\n' 'SEND'",
        "marker-a",
        "marker-a",
    );
    let receipt_a = send(&fixture, &terminal_a, &["--text", &line_a, "--enter"]);
    assert_text_bytes(&receipt_a, &line_a, true);
    let line_b = producer(
        &fixture,
        "printf 'OB_%s_B\\n' 'SEND'",
        "marker-b",
        "marker-b",
    );
    let receipt_b = send(&fixture, &terminal_b, &["--text", &line_b, "--enter"]);
    assert_text_bytes(&receipt_b, &line_b, true);

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

    let line = producer(
        &fixture,
        "printf 'OB_%s_RAN\\n' 'STAGED'",
        "staged-proof",
        "staged",
    );
    let receipt = send(&fixture, &terminal, &["--text", &line]);
    assert_text_bytes(&receipt, &line, false);
    // The staged text must not execute on its own: this is a bounded absence
    // window, not a claim that a delayed execution is impossible in principle.
    assert_absent_while(
        &fixture,
        "staged-proof",
        &mut client,
        Duration::from_millis(600),
    );

    let receipt = send(&fixture, &terminal, &["--key", "enter"]);
    assert_key_bytes(&receipt, "enter");
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

    // A's shell runs a fixture-owned foreground child that reports its own pid
    // and then stays blocked under the same pid: `exec` keeps the reported
    // identity as the actual sleep process, so the marker file's existence
    // plus liveness proves a real foreground job — no marker-before-start
    // window and no settled-sleep guess. A `sleep; tail` line is rejected as
    // oracle because an interactive shell discards the remainder on SIGINT.
    send(
        &fixture,
        &terminal_a,
        &[
            "--text",
            &format!(
                "/bin/sh -c 'printf \"%s\" \"$$\" > {}; exec sleep 300'",
                fixture.base.join("a-child-pid").display()
            ),
            "--enter",
        ],
    );
    let child_pid: u32 = wait_file(
        &fixture.base.join("a-child-pid"),
        &mut [&mut client],
        Instant::now() + OBSERVATION_TIMEOUT,
    )
    .parse()
    .expect("child pid");
    assert_ne!(
        child_pid, shell_pid,
        "the foreground job must be a distinct child process"
    );
    // Hold the child's exact process identity and confirm it is alive and
    // blocked before the interrupt.
    let child = pidfd_open(
        Pid::from_raw(i32::try_from(child_pid).expect("child pid range"))
            .ok_or("child pid not a valid process id")?,
        PidfdFlags::empty(),
    )?;
    let mut descriptor = [PollFd::new(child.as_fd(), PollFlags::POLLIN)];
    assert_eq!(
        poll(&mut descriptor, PollTimeout::ZERO)?,
        0,
        "the reported foreground child must be alive and blocked"
    );

    // While A is blocked, the independent terminal still takes its own send.
    let line_b = producer(
        &fixture,
        "printf 'OB_%s_ALIVE\\n' 'PEER'",
        "b-alive",
        "peer-alive",
    );
    let receipt_b = send(&fixture, &terminal_b, &["--text", &line_b, "--enter"]);
    assert_text_bytes(&receipt_b, &line_b, true);
    assert_eq!(
        wait_file(
            &fixture.base.join("b-alive"),
            &mut [&mut client],
            Instant::now() + OBSERVATION_TIMEOUT,
        ),
        "peer-alive"
    );

    // One ctrl-c through the public send path interrupts the foreground child.
    let receipt = send(&fixture, &terminal_a, &["--key", "ctrl-c"]);
    assert_key_bytes(&receipt, "ctrl-c");
    let deadline = Instant::now() + OBSERVATION_TIMEOUT;
    loop {
        poll(&mut descriptor, PollTimeout::ZERO)?;
        if descriptor[0]
            .revents()
            .is_some_and(|flags| flags.contains(PollFlags::POLLIN))
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the foreground child did not terminate"
        );
        client.collect();
        thread::yield_now();
    }

    // The next command on the original shell captures `$$` and `$?` together:
    // no intervening command may clobber the interrupt's exit status, so this
    // must be the first input the shell runs after the child's death.
    let status_line = format!(
        "printf '%s:%s' \"$$\" \"$?\" > {}",
        fixture.base.join("a-after").display()
    );
    send(&fixture, &terminal_a, &["--text", &status_line, "--enter"]);
    assert_eq!(
        wait_file(
            &fixture.base.join("a-after"),
            &mut [&mut client],
            Instant::now() + OBSERVATION_TIMEOUT,
        ),
        format!("{shell_pid}:130"),
        "the original shell survives with the SIGINT status 128+2"
    );
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}
