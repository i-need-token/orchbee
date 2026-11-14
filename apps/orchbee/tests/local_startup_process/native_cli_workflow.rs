//! Public end-to-end CLI workflow witness: the fixture only starts the
//! isolated Session's Server — no TUI is spawned anywhere in the scenario — and
//! every business step is one invocation of the real staged `orchbee` binary:
//! project create → workspace create → workspace set-directory → tab
//! create → terminal list → terminal send → terminal wait-output → terminal
//! read → terminal get → tab close --confirm → canonical snapshot. No internal API mutates
//! business state, no fixture file fakes a command, and no send is retried.
//! Read and wait-output steps only observe produced output; they can never
//! fabricate command completion.
//!
//! Frozen contracts asserted here come from CLI-WORKSPACE-DIRECTORY-ACCEPTED,
//! CLI-SEND-IMPLEMENT-FINAL, and CLI-WAIT-IMPLEMENT-FINAL/CLI-FIRST-FLOW; the
//! send and wait-output receipts are bound against their registered
//! `orchbee.terminal.send` / `orchbee.terminal.wait-output` schemas, and the
//! send receipt is cross-checked against the wait receipt's observed source.

use super::{
    native_cli_query::{run_json, spawn_real_server, spawn_real_server_env, validate, validators},
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
/// is a commit, not a live shell, so binding is polled with a bounded
/// deadline. Returns the row's `(terminalId, workerId)` plus the envelope's
/// `source.sessionId` so later receipts are cross-checked against the same list
/// observation instead of a second lookup.
fn bind_terminal(fixture: &Fixture, tab_id: &str) -> (String, String, String) {
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
            return (
                row["terminalId"].as_str().expect("terminalId").to_owned(),
                row["workerId"].as_str().expect("workerId").to_owned(),
                terminals["source"]["sessionId"]
                    .as_str()
                    .expect("sessionId")
                    .to_owned(),
            );
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
    document["result"]["content"]["text"]
        .as_str()
        .expect("source text")
        .lines()
        .map(str::to_owned)
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
    // uses; no TUI ever joins this Session.
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

    // 3. `workspace set-directory <id> --path <dir>` — the only directory
    //    path admitted to the workspace; the TUI directory modal is never
    //    touched. Frozen contract (CLI-WORKSPACE-DIRECTORY-ACCEPTED):
    //    action/outcome plus revision/directoryId/workerId and the resolved
    //    workspaceId on the mutation receipt.
    let (output, accepted) = run_json(
        &fixture,
        &[
            "workspace",
            "set-directory",
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
        "workspace set-directory: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(accepted["result"]["action"], "workspace.accept-directory");
    assert_eq!(accepted["result"]["outcome"], "committed");
    // The receipt names the Workspace; its Project stays with the Workspace record.
    assert!(accepted["result"].get("projectId").is_none(), "{accepted}");
    assert_eq!(accepted["result"]["workspaceId"], workspace_id.as_str());
    for field in ["revision", "directoryId", "workerId"] {
        assert!(
            accepted["result"][field].as_str().is_some(),
            "set-directory receipt carries {field}: {accepted}"
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
    let binding_a = bind_terminal(&fixture, &tab_ids[0]);
    let binding_b = bind_terminal(&fixture, &tab_ids[1]);
    let terminal_a = binding_a.0.clone();
    let terminal_b = binding_b.0.clone();
    assert_eq!(
        binding_a.2, binding_b.2,
        "both bindings observe the same Session"
    );
    assert_ne!(terminal_a, terminal_b, "independent terminal identities");

    // 5. One `terminal send` per terminal. The command never `cd`s: the shell
    //    writes the proof into its own initial cwd via a relative name, so the
    //    file's landing location — and the `pwd -P` line it records — attest
    //    the real cwd that set-directory/tab established instead of masking
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
    // Each send receipt is kept: the wait step below cross-checks its observed
    // source against the same receipt instead of re-querying or resending.
    let mut receipts = Vec::new();
    for (terminal, marker, proof, binding) in [
        (terminal_a.as_str(), marker_a.as_str(), proof_a, &binding_a),
        (terminal_b.as_str(), marker_b.as_str(), proof_b, &binding_b),
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
        // The frozen `orchbee.terminal.send` receipt: the mutation selected
        // the same physical target the binding list row reported, the host
        // accepted every submitted byte (UTF-8 text plus the --enter CR), and
        // cleanup is confirmed — the attachment did not linger.
        validate(&validators, "orchbee.terminal.send", &output);
        let result = &sent["result"];
        assert_eq!(result["action"], "terminal.send");
        assert_eq!(
            result["terminalId"], terminal,
            "receipt targets the sent terminal"
        );
        assert_eq!(
            result["workerId"],
            binding.1.as_str(),
            "receipt worker matches the binding list row"
        );
        assert_eq!(
            result["sessionId"],
            binding.2.as_str(),
            "receipt session matches the list envelope source"
        );
        assert_eq!(result["inputStatus"], "accepted");
        assert_eq!(result["cleanupStatus"], "confirmed");
        let record_bytes = text.len() + 1;
        assert_eq!(
            result["recordBytes"].as_u64(),
            Some(record_bytes as u64),
            "recordBytes is the UTF-8 text plus the --enter CR"
        );
        assert_eq!(
            result["acceptedBytes"].as_u64(),
            Some(record_bytes as u64),
            "host accepted exactly the submitted record"
        );
        let target = &result["target"];
        assert_ne!(
            target["workerGeneration"], "0",
            "echoed worker generation is a real positive id"
        );
        for field in ["hostEpoch", "hostTerminalId"] {
            assert!(
                target[field]
                    .as_str()
                    .is_some_and(|id| { id.len() == 32 && id.bytes().any(|byte| byte != b'0') }),
                "target {field} is a real nonzero identity: {sent}"
            );
        }
        receipts.push((terminal.to_owned(), sent.clone()));
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
        // The frozen `orchbee.terminal.wait-output` receipt, cross-checked
        // against the send receipt saved above: the observed source is the
        // same terminal/worker/session the send receipt recorded, hostEpoch is
        // the send's actual target epoch (one same-source scenario), the
        // match excerpt carries this run's marker, and at least one
        // observation completed. Row-window offsets stay unasserted — they
        // are frame geometry, not stable identities.
        validate(&validators, "orchbee.terminal.wait-output", &output);
        let result = &matched["result"];
        let (_, sent) = receipts
            .iter()
            .find(|(sent_terminal, _)| sent_terminal.as_str() == terminal)
            .expect("send receipt saved for this terminal");
        let sent_result = &sent["result"];
        assert_eq!(
            result["terminal"]["terminalId"], terminal,
            "wait observed the sent terminal"
        );
        assert_eq!(
            result["terminal"]["workerId"], sent_result["workerId"],
            "wait observed the same worker the send receipt recorded"
        );
        assert_eq!(
            matched["source"]["sessionId"], sent_result["sessionId"],
            "wait source session matches the send receipt's session"
        );
        assert_eq!(
            result["observed"]["hostEpoch"], sent_result["target"]["hostEpoch"],
            "wait attached on the same host epoch the send targeted"
        );
        assert!(
            result["match"]["excerpt"]
                .as_str()
                .expect("match excerpt")
                .contains(marker),
            "match excerpt carries this run's marker: {matched}"
        );
        assert!(
            result["observed"]["completedObservations"]
                .as_u64()
                .is_some_and(|done| done >= 1),
            "at least one observation completed before the match: {matched}"
        );
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
        if output.status.code() != Some(0) {
            // A handled failure still emits its typed document on stdout —
            // report it alongside stderr. The cohort revalidation below
            // describes only the fixture directory's LATER state; the
            // child's original io error kind is not recoverable from the
            // mapped failure.
            let recheck = Cohort::load(&fixture.cohort, Instant::now() + Duration::from_secs(20))
                .map(|_| "ok".to_owned())
                .unwrap_or_else(|error| format!("{error:?}"));
            panic!(
                "terminal read: {} failure document: {frame} \
                 fixture cohort state after the failure: {recheck}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        validate(&validators, "orchbee.terminal.output", &output);
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

    // 8.5. `terminal get` on the live first terminal — the Server-authoritative
    //     launch-read witness. The envelope is `version: 1` sourced by the
    //     Server-joined read (not a client snapshot), `launch` nests inside
    //     `result.terminal`, and the joined revision equals the revision an
    //     adjacent `terminal list` observed (nothing mutates between the two
    //     reads, so both report the same committed Workbench revision). The
    //     wait/read steps above already proved the shell runs, so `started`
    //     is a deterministic assertion here — not a race on create timing.
    let (output, listed) = run_json(&fixture, &["terminal", "list", "--format", "json"]);
    assert_eq!(output.status.code(), Some(0));
    let (output, got) = run_json(
        &fixture,
        &["terminal", "get", terminal_a.as_str(), "--format", "json"],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "terminal get: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    validate(&validators, "orchbee.terminal.get", &output);
    assert_eq!(
        got["version"], 1,
        "get answers with the current envelope: {got}"
    );
    assert_eq!(got["source"]["kind"], "terminal-launch-read");
    assert_eq!(
        got["source"]["sessionId"],
        binding_a.2.as_str(),
        "get source session matches the list envelope source"
    );
    let get_revision = got["source"]["revision"]
        .as_str()
        .expect("get source revision is a decimal string");
    assert!(
        get_revision.bytes().all(|byte| byte.is_ascii_digit())
            && get_revision.parse::<u64>().is_ok_and(|value| value > 0),
        "get source revision is a positive decimal string: {got}"
    );
    assert_eq!(
        Some(get_revision),
        listed["source"]["revision"].as_str(),
        "the joined read reports the same committed revision an adjacent list observed"
    );
    let terminal = &got["result"]["terminal"];
    assert_eq!(terminal["terminalId"], terminal_a.as_str());
    assert_eq!(terminal["workerId"], binding_a.1.as_str());
    assert!(
        got["result"].get("launch").is_none(),
        "launch nests inside `terminal`, never beside it: {got}"
    );
    let launch = &terminal["launch"];
    assert_eq!(launch["state"], "started");
    let generation = launch["workerGeneration"]
        .as_str()
        .expect("tracked launch carries a decimal workerGeneration");
    assert_ne!(
        generation.parse::<u64>().ok(),
        Some(0),
        "tracked generation is positive: {got}"
    );
    assert_eq!(launch["createOutcome"]["kind"], "accepted");
    assert_eq!(launch["createOutcome"]["state"]["kind"], "started");
    assert!(
        launch["geometry"]["columns"].as_u64().is_some(),
        "started launch carries its geometry: {got}"
    );

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
    // Removal is checked against the snapshot model's own kinds, never a raw
    // substring scan: closed tabs must vanish from every context's `tabs[]`,
    // and closed terminals from every presentation's `binding.terminalId`
    // (primary and child presentations, recursing through splits). Same-shape
    // ids in other namespaces — coordinator `terminalId` values collide
    // numerically with context/project ids — stay legal; the negative control
    // below proves both directions.
    let deadline = Instant::now() + CLOSE_DEADLINE;
    loop {
        let (output, snapshot) = run_json(&fixture, &["api", "snapshot", "--format", "json"]);
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(snapshot["result"]["snapshot"]["kind"], "workbench.snapshot");
        validate(&validators, "orchbee.api.snapshot", &output);
        if closed_resources_absent(
            &snapshot["result"]["snapshot"],
            &tab_ids,
            &[&terminal_a, &terminal_b],
        ) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "closed resources still exposed in the canonical snapshot"
        );
        thread::sleep(BIND_POLL);
    }
    Ok(())
}

/// True when none of `tab_ids` remain as `terminalContexts[].tabs[].tabId`
/// and none of `terminal_ids` remain as any presentation's
/// `binding.terminalId` (primary or child, through nested splits). Any other
/// field carrying the same string — labels, context/project identities —
/// does not count: ids are only compared inside their own model kinds.
fn closed_resources_absent(
    snapshot: &serde_json::Value,
    tab_ids: &[String],
    terminal_ids: &[&String],
) -> bool {
    let contexts = snapshot["terminalContexts"]
        .as_array()
        .expect("terminal contexts");
    let tabs_gone = contexts
        .iter()
        .flat_map(|context| context["tabs"].as_array().expect("context tabs"))
        .all(|tab| !tab_ids.iter().any(|id| tab["tabId"] == *id));
    if !tabs_gone {
        return false;
    }
    contexts
        .iter()
        .flat_map(|context| context["tabs"].as_array().expect("context tabs"))
        .all(|tab| presentations_clear(&tab["root"], terminal_ids))
}

/// True when no presentation reachable from `node` binds one of
/// `terminal_ids`: a panel's primary plus every child presentation, and all
/// descendants of a split's children.
fn presentations_clear(node: &serde_json::Value, terminal_ids: &[&String]) -> bool {
    match node["kind"].as_str() {
        Some("panel") => {
            let clear = |presentation: &serde_json::Value| {
                presentation["binding"]["terminalId"]
                    .as_str()
                    .is_none_or(|bound| !terminal_ids.iter().any(|id| bound == id.as_str()))
            };
            clear(&node["primaryPresentation"])
                && node["childPresentations"]
                    .as_array()
                    .expect("child presentations")
                    .iter()
                    .all(clear)
        }
        Some("split") => node["children"]
            .as_array()
            .expect("split children")
            .iter()
            .all(|child| presentations_clear(&child["node"], terminal_ids)),
        _ => panic!("unexpected panel node kind: {node}"),
    }
}

/// Negative control for the canonical close check: same-shape strings in a
/// label or another resource kind must not count as alive, while a real
/// `binding.terminalId` or `tabs[].tabId` residue must be detected.
#[test]
fn closed_resources_absent_negative_controls() {
    let tabs = vec!["0000000000000000000000000000000a".to_owned()];
    let terminals = ["0000000000000000000000000000000b".to_owned()];
    let terminal_refs: Vec<&String> = terminals.iter().collect();
    let empty_context = |label: &str| {
        serde_json::json!({
            "groups": [],
            "terminalContextId": "0000000000000000000000000000000c",
            "tabs": [{
                "groupId": null,
                "label": label,
                "pinned": false,
                "root": {
                    "kind": "panel",
                    "panelId": "0000000000000000000000000000000d",
                    "primaryPresentation": {"customLabel":null,
                        "presentationId": "0000000000000000000000000000000e",
                        "label": "shell",
                        "binding": null
                    },
                    "childPresentations": []
                },
                "tabId": "0000000000000000000000000000000f"
            }]
        })
    };
    // Same strings in a tab label and a presentation label are not residue.
    let mut snapshot = serde_json::json!({
        "kind": "workbench.snapshot",
        "terminalContexts": [empty_context("0000000000000000000000000000000a")]
    });
    snapshot["terminalContexts"][0]["tabs"][0]["root"]["primaryPresentation"]["label"] =
        serde_json::json!("0000000000000000000000000000000b");
    assert!(closed_resources_absent(&snapshot, &tabs, &terminal_refs));
    // A real tabs[].tabId residue is detected.
    snapshot["terminalContexts"][0]["tabs"][0]["tabId"] =
        serde_json::json!("0000000000000000000000000000000a");
    assert!(!closed_resources_absent(&snapshot, &tabs, &terminal_refs));
    // A real binding.terminalId residue — including a nested-split child —
    // is detected.
    snapshot["terminalContexts"][0]["tabs"][0]["tabId"] =
        serde_json::json!("0000000000000000000000000000000f");
    snapshot["terminalContexts"][0]["tabs"][0]["root"]["childPresentations"] = serde_json::json!([{"customLabel":null,
        "presentationId": "00000000000000000000000000000010",
        "label": "split-child",
        "binding": { "terminalId": "0000000000000000000000000000000b", "workerId": "00000000000000000000000000000011" }
    }]);
    assert!(!closed_resources_absent(&snapshot, &tabs, &terminal_refs));
    snapshot["terminalContexts"][0]["tabs"][0]["root"]["childPresentations"] =
        serde_json::json!([]);
    assert!(closed_resources_absent(&snapshot, &tabs, &terminal_refs));
    snapshot["terminalContexts"][0]["tabs"][0]["root"] = serde_json::json!({
        "kind": "split",
        "splitId": "00000000000000000000000000000012",
        "orientation": "horizontal",
        "children": [
            { "slotId": "00000000000000000000000000000013", "node": {
                "kind": "panel",
                "panelId": "00000000000000000000000000000014",
                "primaryPresentation": {"customLabel":null,
                    "presentationId": "00000000000000000000000000000015",
                    "label": "left",
                    "binding": null
                },
                "childPresentations": []
            }},
            { "slotId": "00000000000000000000000000000016", "node": {
                "kind": "panel",
                "panelId": "00000000000000000000000000000017",
                "primaryPresentation": {"customLabel":null,
                    "presentationId": "00000000000000000000000000000018",
                    "label": "right",
                    "binding": { "terminalId": "0000000000000000000000000000000b", "workerId": "00000000000000000000000000000011" }
                },
                "childPresentations": []
            }}
        ]
    });
    assert!(!closed_resources_absent(&snapshot, &tabs, &terminal_refs));
}

/// Real failed-launch witness, no TUI: the fixture Server inherits a `SHELL`
/// pointing at a fixture-private executable whose shebang names a guaranteed
/// nonexistent interpreter under the same disposable directory. Worker
/// default-shell admission accepts it (regular file + X_OK is the whole
/// admission surface — no interpreter/format check), Host `execve` then fails
/// and reports a real launch failure. The retained fact must read back
/// through the public CLI as `unavailable` with a `failed` create outcome —
/// never silently dropped, never a phantom `started`. `SHELL` is set only on
/// the spawned Server command (the managed Worker inherits it); nothing
/// process-global, no account shell, and no cohort binary is touched.
#[test]
fn cli_get_reads_the_retained_failed_launch_without_tui() -> TestResult {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::new();

    // Fixture-private executable: regular file, 0700, shebang interpreter that
    // does not exist — guaranteed ENOENT inside the Host's execve.
    let shell_dir = fixture.base.join("fixture-shell");
    fs::create_dir(&shell_dir)?;
    let shell = shell_dir.join("shell");
    let interpreter = shell_dir.join("no-such-interpreter");
    fs::write(&shell, format!("#!{}\n", interpreter.display()))?;
    fs::set_permissions(&shell, fs::Permissions::from_mode(0o700))?;
    let _server = spawn_real_server_env(&fixture, &[("SHELL", shell.as_path())]);
    let validators = validators(&fixture);

    // Same public workspace path as the happy workflow: project → workspace →
    // set-directory → tab create. The mutation commits the tab and reports
    // the terminal's typed spawn outcome.
    let directory = fixture.base.join("wf 目录 fail");
    fs::create_dir(&directory)?;
    let directory = fs::canonicalize(directory)?;
    let directory = directory.to_str().ok_or("directory path UTF-8")?;

    let (output, created) = run_json(
        &fixture,
        &["project", "create", "--label", "wf", "--format", "json"],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "project create: {:?}",
        output
    );
    let project_id = created["result"]["projectId"]
        .as_str()
        .expect("allocated projectId")
        .to_owned();
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
        "workspace create: {:?}",
        output
    );
    let workspace_id = workspace["result"]["workspaceId"]
        .as_str()
        .expect("workspace id")
        .to_owned();
    let (output, _accepted) = run_json(
        &fixture,
        &[
            "workspace",
            "set-directory",
            workspace_id.as_str(),
            "--path",
            directory,
            "--format",
            "json",
        ],
    );
    assert_eq!(output.status.code(), Some(0), "set-directory: {:?}", output);

    let mut args = vec![
        "tab",
        "create",
        "--workspace",
        workspace_id.as_str(),
        "--label",
        "fails",
    ];
    args.extend(GEOMETRY);
    args.extend(["--format", "json"]);
    let (output, tab) = run_json(&fixture, &args);
    // Preserve the actual mutation contract: the tab commit itself is
    // `committed`; the failed spawn rides inside the receipt's terminal fact
    // — a Host-reported exec failure, never a precommit or fallback outcome.
    assert_eq!(
        output.status.code(),
        Some(0),
        "tab create receipt: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(tab["result"]["action"], "tab.create");
    assert_eq!(tab["result"]["outcome"], "committed");
    assert_eq!(tab["result"]["terminal"]["outcome"], "failed");
    assert_eq!(
        tab["result"]["terminal"]["reason"], "terminal-create-failed",
        "the exec failure is the Host-reported kind, not admission/runtime: {tab}"
    );
    let tab_id = tab["result"]["tabId"].as_str().expect("tabId").to_owned();

    // The failed launch's binding was committed with the tab; discover the
    // terminal through the public list surface (no receipt id, no seeding).
    let (terminal_id, worker_id, session_id) = bind_terminal(&fixture, &tab_id);

    // `terminal get` answers the failed retained fact through the real Server
    // read: v1 envelope, Server-joined source, nested launch, unavailable
    // state with the failed create outcome and a real positive generation.
    let (output, got) = run_json(
        &fixture,
        &["terminal", "get", terminal_id.as_str(), "--format", "json"],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "terminal get on a failed launch still answers: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    validate(&validators, "orchbee.terminal.get", &output);
    assert_eq!(got["version"], 1);
    assert_eq!(got["source"]["kind"], "terminal-launch-read");
    assert_eq!(got["source"]["sessionId"], session_id.as_str());
    let terminal = &got["result"]["terminal"];
    assert_eq!(terminal["terminalId"], terminal_id.as_str());
    assert_eq!(terminal["workerId"], worker_id.as_str());
    assert_eq!(terminal["boundTo"]["tabId"], tab_id.as_str());
    let launch = &terminal["launch"];
    assert_eq!(
        launch["state"], "unavailable",
        "the failed launch settles to the retained unavailable phase: {got}"
    );
    assert_eq!(launch["createOutcome"]["kind"], "failed");
    assert_eq!(
        launch["createOutcome"]["reason"], "terminal-create-failed",
        "the retained outcome echoes the Host failure: {got}"
    );
    let generation = launch["workerGeneration"]
        .as_str()
        .expect("tracked launch carries a decimal workerGeneration");
    assert!(
        generation.parse::<u64>().is_ok_and(|value| value > 0),
        "tracked generation is positive: {got}"
    );
    assert!(
        launch["geometry"].is_null(),
        "a failed launch carries no started geometry: {got}"
    );
    Ok(())
}
