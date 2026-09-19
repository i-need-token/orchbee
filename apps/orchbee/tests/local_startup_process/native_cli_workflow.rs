//! Public end-to-end CLI workflow witness: the fixture only starts the
//! isolated Site's Server — no TUI is spawned anywhere in the scenario — and
//! every business step is one invocation of the real staged `orchbee` binary:
//! project create → workspace create → workspace accept-directory → tab
//! create → terminal list → terminal send → terminal wait-output → terminal
//! read → tab close --confirm → canonical snapshot. No internal API mutates
//! business state, no fixture file fakes a command, and no send is retried.
//! Read and wait-output steps only observe produced output; they can never
//! fabricate command completion.
//!
//! EXPECTED-RED on this base: `workspace accept-directory` (a4 slice),
//! `terminal send` (a1 slice), and `terminal wait-output` (a5 slice) do not
//! exist yet, so the chain stops at the first missing command. Frozen
//! contracts asserted here come from CLI-WORKSPACE-DIRECTORY-ACCEPTED,
//! CLI-SEND-IMPLEMENT-FINAL, and CLI-WAIT-IMPLEMENT-FINAL/CLI-FIRST-FLOW;
//! result field names whose schemas are not frozen are marked `SEAM` and only
//! the exit/envelope contract is asserted.

use super::{
    native_cli_query::{run_json, spawn_real_server, validate, validators},
    *,
};
use std::path::Path;

const BIND_DEADLINE: Duration = Duration::from_secs(20);
const BIND_POLL: Duration = Duration::from_millis(150);
const CLOSE_DEADLINE: Duration = Duration::from_secs(10);
/// A short, legal wait budget for the never-produced marker: the command must
/// time out on its own, without a cancelled or truncated observation.
const MISS_TIMEOUT_MS: &str = "3000";
const GEOMETRY: [&str; 8] = [
    "--cols",
    "80",
    "--rows",
    "24",
    "--cell-width",
    "9",
    "--cell-height",
    "18",
];

/// `terminal list` until `tab_id` carries a bound terminal; the create receipt
/// is a commit, not a live shell, so binding is polled with a bounded deadline.
fn bind_terminal(fixture: &Fixture, tab_id: &str) -> String {
    let deadline = Instant::now() + BIND_DEADLINE;
    loop {
        let (output, terminals) = run_json(fixture, &["terminal", "list", "--format", "json"]);
        assert_eq!(output.status.code(), Some(0));
        if let Some(row) = terminals["result"]["terminals"]
            .as_array()
            .expect("terminal list rows")
            .iter()
            .find(|row| row["boundTo"]["tabId"] == tab_id)
        {
            return row["terminalId"].as_str().expect("terminalId").to_owned();
        }
        assert!(
            Instant::now() < deadline,
            "tab {tab_id} never bound a terminal: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        thread::sleep(BIND_POLL);
    }
}

fn frame_rows(document: &serde_json::Value) -> Vec<String> {
    document["result"]["content"]["rows"]
        .as_array()
        .expect("frame rows")
        .iter()
        .map(|row| row["text"].as_str().expect("frame row text").to_owned())
        .collect()
}

/// The full business loop over public CLI only. Two tabs bind two independent
/// terminals; each receives a single send whose produced marker is unique and
/// never contiguous inside the echoed command, each wait matches exactly once,
/// the proof files are independent shell-produced evidence, and the canonical
/// snapshot confirms the closed resources stop being exposed.
#[test]
fn cli_workflow_create_send_wait_read_close_completes_without_tui() -> TestResult {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::new();
    // Server-only startup — the same fixture surface A4's pure-CLI evidence
    // uses; no TUI ever joins this Site.
    let _server = spawn_real_server(&fixture);
    let validators = validators(&fixture);

    // Fixture-owned workspace directory: literal absolute path containing a
    // space and non-ASCII text, created by the fixture, never by the CLI.
    let directory = fixture.base.join("wf 目录 case");
    fs::create_dir(&directory)?;
    let directory = fs::canonicalize(directory)?;
    let directory = directory.to_str().ok_or("directory path UTF-8")?;

    // 1. `project create` — committed Server-allocated id.
    let (output, created) = run_json(
        &fixture,
        &["project", "create", "--label", "wf", "--format", "json"],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "project create: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    validate(&validators, "orchbee.mutation.result", &output);
    let project_id = created["result"]["projectId"]
        .as_str()
        .expect("allocated projectId")
        .to_owned();

    // 2. `workspace create --project <id>`.
    let (output, workspace) = run_json(
        &fixture,
        &[
            "workspace",
            "create",
            "--project",
            project_id.as_str(),
            "--label",
            "main",
            "--format",
            "json",
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "workspace create: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let workspace_id = workspace["result"]["workspaceId"]
        .as_str()
        .expect("workspace id")
        .to_owned();

    // 3. `workspace accept-directory <id> --path <dir>` — the only directory
    //    path admitted to the workspace; the TUI directory modal is never
    //    touched. Frozen contract (CLI-WORKSPACE-DIRECTORY-ACCEPTED):
    //    action/outcome plus revision/directoryId/workerId and the resolved
    //    projectId/workspaceId on the mutation receipt.
    let (output, accepted) = run_json(
        &fixture,
        &[
            "workspace",
            "accept-directory",
            workspace_id.as_str(),
            "--path",
            directory,
            "--format",
            "json",
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "workspace accept-directory: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(accepted["result"]["action"], "workspace.accept-directory");
    assert_eq!(accepted["result"]["outcome"], "committed");
    assert_eq!(accepted["result"]["projectId"], project_id.as_str());
    assert_eq!(accepted["result"]["workspaceId"], workspace_id.as_str());
    for field in ["revision", "directoryId", "workerId"] {
        assert!(
            accepted["result"][field].as_str().is_some(),
            "accept-directory receipt carries {field}: {accepted}"
        );
    }

    // 4. Two tabs in the directory-ready workspace → two independent shells.
    let mut tab_ids = Vec::new();
    for label in ["a", "b"] {
        let mut args = vec![
            "tab",
            "create",
            "--workspace",
            workspace_id.as_str(),
            "--label",
            label,
        ];
        args.extend(GEOMETRY);
        args.extend(["--format", "json"]);
        let (output, tab) = run_json(&fixture, &args);
        assert_eq!(
            output.status.code(),
            Some(0),
            "tab create {label}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(tab["result"]["action"], "tab.create");
        tab_ids.push(tab["result"]["tabId"].as_str().expect("tabId").to_owned());
    }
    let terminal_a = bind_terminal(&fixture, &tab_ids[0]);
    let terminal_b = bind_terminal(&fixture, &tab_ids[1]);
    assert_ne!(terminal_a, terminal_b, "independent terminal identities");

    // 5. One `terminal send` per terminal. The command never `cd`s: the shell
    //    writes the proof into its own initial cwd via a relative name, so the
    //    file's landing location — and the `pwd -P` line it records — attest
    //    the real cwd that accept-directory/tab established instead of masking
    //    a wrong one. The marker is split inside the typed text
    //    (`'WA22-''<nonce>'`), so the contiguous marker never appears in the
    //    echoed command line — only in produced output. The shell first
    //    writes a fixture-owned proof file containing its real PID, the
    //    marker, and its resolved cwd, then prints the marker: a wait match
    //    therefore guarantees the independent producer evidence already
    //    exists.
    let nonce = format!("{:05x}", std::process::id() & 0x0f_ffff);
    let marker_a = format!("WA22-{nonce}");
    let marker_b = format!("WB22-{nonce}");
    let proof_a = "proof-a.txt";
    let proof_b = "proof-b.txt";
    for (terminal, marker, proof) in [
        (terminal_a.as_str(), marker_a.as_str(), proof_a),
        (terminal_b.as_str(), marker_b.as_str(), proof_b),
    ] {
        let (prefix, tag) = marker.split_at(4);
        let text = format!(
            "printf '%s\\n%s\\n%s\\n' \"$$\" '{prefix}''{tag}' \"$(pwd -P)\" > '{proof}' && printf '{prefix}''{tag}\\n'"
        );
        let (output, sent) = run_json(
            &fixture,
            &[
                "terminal",
                "send",
                terminal,
                "--text",
                text.as_str(),
                "--enter",
                "--format",
                "json",
            ],
        );
        assert_eq!(
            output.status.code(),
            Some(0),
            "terminal send: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        // SEAM(a1): exit 0 alone is NOT the receipt validation — the frozen
        // contract additionally requires the receipt to record the
        // actually-selected physical target. Assert the receipt's
        // input-status/accepted-bytes/cleanup axes and the echoed physical
        // identity (worker id+generation, hostEpoch, hostTerminalId) once the
        // send result schema is frozen; A9's final combination must bind
        // these against the actual registered schema, not guessed names.
        let _ = sent;
    }

    // 6. One `terminal wait-output --match` per terminal: the match observes
    //    produced output only — the contiguous marker cannot be the echoed
    //    command, and the proof file below independently proves execution.
    for (terminal, marker) in [
        (terminal_a.as_str(), marker_a.as_str()),
        (terminal_b.as_str(), marker_b.as_str()),
    ] {
        let (output, matched) = run_json(
            &fixture,
            &[
                "terminal",
                "wait-output",
                terminal,
                "--match",
                marker,
                "--timeout",
                "30000",
                "--format",
                "json",
            ],
        );
        assert_eq!(
            output.status.code(),
            Some(0),
            "terminal wait-output: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        // SEAM(a5): exit 0 alone is NOT the match validation — assert the
        // observed source identity (terminalId, resolved worker/site,
        // hostEpoch/attachment), the bounded excerpt, and
        // completedObservations once the wait result schema is frozen; A9's
        // final combination must bind these against the actual registered
        // schema, not guessed names.
        let _ = matched;
    }

    // 7. Independent producer evidence: each proof file was written by the
    //    shell itself — a nonzero real PID, the unique marker, and the
    //    shell's resolved cwd — before the marker was ever printed. The wait
    //    match alone is never "task done", and the file's landing inside the
    //    accepted directory already proves the terminal's initial cwd; the
    //    recorded `pwd -P` is checked against it explicitly.
    let mut shell_pids = Vec::new();
    for (marker, proof) in [(marker_a.as_str(), proof_a), (marker_b.as_str(), proof_b)] {
        let content = fs::read_to_string(Path::new(directory).join(proof))?;
        let mut lines = content.lines();
        let pid = lines
            .next()
            .and_then(|pid| pid.parse::<u64>().ok())
            .filter(|pid| *pid != 0)
            .unwrap_or_else(|| panic!("proof carries a nonzero real PID: {content:?}"));
        assert_eq!(lines.next(), Some(marker), "proof carries the marker");
        assert_eq!(
            lines.next(),
            Some(directory),
            "the shell's own `pwd -P` is the accepted directory: {content:?}"
        );
        shell_pids.push(pid);
    }
    assert_ne!(
        shell_pids[0], shell_pids[1],
        "two independent shells report distinct PIDs"
    );

    // 8. Cross-write check over `terminal read`: each frame contains its own
    //    marker exactly once (the produced line; the echoed command holds the
    //    split spelling) and never the sibling marker.
    for (terminal, own, foreign) in [
        (terminal_a.as_str(), marker_a.as_str(), marker_b.as_str()),
        (terminal_b.as_str(), marker_b.as_str(), marker_a.as_str()),
    ] {
        let (output, frame) = run_json(
            &fixture,
            &["terminal", "read", terminal, "--format", "json"],
        );
        assert_eq!(
            output.status.code(),
            Some(0),
            "terminal read: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        validate(&validators, "orchbee.terminal.read", &output);
        let rows = frame_rows(&frame);
        assert_eq!(
            rows.iter().filter(|row| row.contains(own)).count(),
            1,
            "own marker appears once as produced output: {rows:?}"
        );
        assert!(
            rows.iter().all(|row| !row.contains(foreign)),
            "no cross-write from the sibling terminal: {rows:?}"
        );
    }

    // 9. A bounded wait for a marker that is never produced times out with
    //    the accepted `timeout` failure (exit 7) — and the same terminal
    //    still answers a subsequent query. Limited claim: this proves only
    //    that a read still succeeds after the timed-out wait; it does NOT
    //    prove the wait released its viewer slot (viewer-capacity accounting
    //    is owned by the a5 cleanup slice, not this witness).
    let never = format!("WN22-{nonce}");
    let (output, failure) = run_json(
        &fixture,
        &[
            "terminal",
            "wait-output",
            terminal_a.as_str(),
            "--match",
            never.as_str(),
            "--timeout",
            MISS_TIMEOUT_MS,
            "--format",
            "json",
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(7),
        "missed wait is the timeout failure: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(failure["failure"]["kind"], "timeout");
    let (output, after) = run_json(
        &fixture,
        &["terminal", "read", terminal_a.as_str(), "--format", "json"],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "terminal still answers after a timed-out wait"
    );
    assert!(frame_rows(&after).iter().any(|row| row.contains(&marker_a)));

    // 10. `tab close --confirm` for both tabs: the long-term contract is an
    //     *accepted* close request carrying a `closeRequestId` — removal is
    //     then proven by bounded canonical observation, not by the receipt
    //     alone.
    for (tab_id, terminal) in [
        (tab_ids[0].as_str(), terminal_a.as_str()),
        (tab_ids[1].as_str(), terminal_b.as_str()),
    ] {
        let (output, closed) = run_json(
            &fixture,
            &["tab", "close", tab_id, "--confirm", "--format", "json"],
        );
        assert_eq!(
            output.status.code(),
            Some(0),
            "tab close: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(closed["result"]["action"], "tab.close");
        assert_eq!(closed["result"]["outcome"], "accepted");
        assert!(
            closed["result"]["closeRequestId"].as_str().is_some(),
            "accepted close carries its request id: {closed}"
        );
        let deadline = Instant::now() + CLOSE_DEADLINE;
        loop {
            let (output, terminals) = run_json(&fixture, &["terminal", "list", "--format", "json"]);
            assert_eq!(output.status.code(), Some(0));
            let gone = terminals["result"]["terminals"]
                .as_array()
                .expect("terminal list rows")
                .iter()
                .all(|row| row["terminalId"] != terminal);
            if gone {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "closed tab's terminal {terminal} still exposed"
            );
            thread::sleep(BIND_POLL);
        }
    }
    let (output, snapshot) = run_json(&fixture, &["api", "snapshot", "--format", "json"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(snapshot["result"]["snapshot"]["kind"], "workbench.snapshot");
    validate(&validators, "orchbee.api.snapshot", &output);
    let canonical = serde_json::to_string(&snapshot["result"]["snapshot"])?;
    for id in [&tab_ids[0], &tab_ids[1], &terminal_a, &terminal_b] {
        assert!(
            !canonical.contains(id.as_str()),
            "closed resource {id} absent from the canonical snapshot"
        );
    }
    Ok(())
}
