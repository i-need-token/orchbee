//! Live launch-selection witness: one real `orchbee tab create --exec ...`
//! `--arg ...` invocation crosses the staged CLI, the real Server, Worker,
//! and Host, and the explicit program runs verbatim under the Workspace's
//! accepted directory — empty, dash-prefixed, and non-ASCII arguments
//! included. The committed request is then replayed over the public
//! `workbench.command` wire with the same request identity: an identical
//! command returns the retained outcome without a second spawn, while the
//! same identity carrying a changed selection is `request-identity-conflict`
//! before any effect. `panel split` carries the same contract on a smoke
//! pass. No internal API mutates business state; every observation is a
//! fixture-owned file written by the spawned process itself.

use super::{
    native_cli_query::{run_json, spawn_real_server},
    *,
};
use orchbee_workbench_protocol::{
    LaunchSelection, ProtocolVersion, SessionId, WorkbenchAction, WorkbenchCommand,
    WorkbenchCommandOutcome, WorkbenchLabel, WorkbenchRejectionReason, WorkbenchRequest,
    WorkbenchRequestId, WorkbenchResponse, WorkbenchRevision, WorkbenchTerminalContextId,
    WorkbenchTerminalGeometry, read_workbench_response, write_workbench_request,
};
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::path::Path;

const SPAWN_DEADLINE: Duration = Duration::from_secs(25);
const SETTLE_WINDOW: Duration = Duration::from_millis(1500);
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

/// Poll a fixture-owned proof file until the spawned program has written it.
/// The file is the only evidence: nothing here fabricates the process run.
fn wait_proof(path: &Path, deadline: Instant) -> String {
    loop {
        if let Ok(content) = fs::read_to_string(path)
            && !content.is_empty()
        {
            return content;
        }
        assert!(Instant::now() < deadline, "proof never landed: {path:?}");
        thread::sleep(Duration::from_millis(100));
    }
}

/// Admit one Client Connection and replay the exact `workbench.command` the
/// staged CLI reported in its receipt — the identity is the receipt's own
/// `source.requestId`, never a fixture-invented one.
fn replay_command(
    fixture: &Fixture,
    command: &WorkbenchCommand,
) -> Result<WorkbenchCommandOutcome, Box<dyn Error>> {
    let deadline = Instant::now() + SPAWN_DEADLINE;
    let cohort = Cohort::load(&fixture.cohort, deadline)?;
    let admitted = ClientConnection::connect_before(
        fixture.session().client_socket_path(),
        cohort.identity(),
        deadline,
    )?;
    let mut stream = UnixStream::from(admitted.as_fd().try_clone_to_owned()?);
    stream.set_read_timeout(Some(SPAWN_DEADLINE))?;
    stream.set_write_timeout(Some(SPAWN_DEADLINE))?;
    write_workbench_request(&mut stream, &WorkbenchRequest::Command(command.clone()))?;
    let WorkbenchResponse::Command(result) = read_workbench_response(&mut stream)? else {
        return Err("expected workbench.command.result".into());
    };
    assert_eq!(result.request_id, command.request_id);
    Ok(result.outcome)
}

fn receipt_field<'a>(document: &'a serde_json::Value, field: &str) -> &'a str {
    document["result"][field]
        .as_str()
        .unwrap_or_else(|| panic!("receipt carries {field}: {document}"))
}

#[test]
fn cli_tab_create_launch_selection_runs_verbatim_and_replays_by_identity() -> TestResult {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::new();
    let _server = spawn_real_server(&fixture);

    // Fixture-owned workspace directory carrying a space and non-ASCII text;
    // canonicalized once so `pwd -P` evidence compares byte-exact.
    let directory = fixture.base.join("launch 目录 case");
    fs::create_dir(&directory)?;
    let directory = fs::canonicalize(directory)?;
    let directory_text = directory.to_str().ok_or("directory path UTF-8")?;

    let (output, project) = run_json(
        &fixture,
        &["project", "create", "--label", "launch", "--format", "json"],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "project create: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let project_id = receipt_field(&project, "projectId").to_owned();
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
    let workspace_id = receipt_field(&workspace, "workspaceId").to_owned();
    let context_id = receipt_field(&workspace, "terminalContextId").to_owned();
    let (output, accepted) = run_json(
        &fixture,
        &[
            "workspace",
            "accept-directory",
            workspace_id.as_str(),
            "--path",
            directory_text,
            "--format",
            "json",
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "accept-directory: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(accepted["result"]["outcome"], "committed");

    // The witness program records one `RUN` block per spawn: its positional
    // parameters verbatim, then the resolved cwd. `$0` is the first --arg
    // after the script; the empty, dash-prefixed, and non-ASCII arguments all
    // ride argv[1..] untouched by any shell parsing on the way to execve.
    let spawns = directory.join("spawns.txt");
    let script = format!(
        "{{ printf 'RUN\\n'; printf '%s\\n' \"$0\" \"$@\"; pwd -P; }} >> '{}'",
        spawns.display()
    );
    let mut args = vec![
        "tab",
        "create",
        "--workspace",
        workspace_id.as_str(),
        "--label",
        "launch tab",
    ];
    args.extend(GEOMETRY);
    args.extend([
        "--exec",
        "/bin/sh",
        "--arg",
        "-c",
        "--arg",
        script.as_str(),
        "--arg",
        "tag0",
        "--arg",
        "",
        "--arg",
        "-x",
        "--arg",
        "漢字",
        "--format",
        "json",
    ]);
    let (output, receipt) = run_json(&fixture, &args);
    assert_eq!(
        output.status.code(),
        Some(0),
        "tab create --exec: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(receipt["result"]["action"], "tab.create");
    let request_id = receipt["source"]["requestId"]
        .as_str()
        .expect("receipt requestId")
        .to_owned();
    let session_id = receipt["source"]["sessionId"].as_str().expect("sessionId");
    let expected_revision = receipt["source"]["expectedRevision"]
        .as_str()
        .expect("expectedRevision");
    let tab_id = receipt_field(&receipt, "tabId").to_owned();
    let panel_id = receipt_field(&receipt, "panelId").to_owned();

    // The real Host ran the explicit program under the accepted directory.
    let observed = wait_proof(&spawns, Instant::now() + SPAWN_DEADLINE);
    assert_eq!(
        observed,
        format!("RUN\ntag0\n\n-x\n漢字\n{directory_text}\n"),
        "argv/cwd block"
    );

    // Rebuild the exact command the CLI submitted: same request identity,
    // same resolved context, same launch selection.
    let command = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&request_id)?,
        session_id: SessionId::parse(session_id)?,
        expected_revision: WorkbenchRevision::parse(expected_revision)?,
        action: WorkbenchAction::TabCreate {
            automatic_label: false,
            placement: None,
            terminal_context_id: WorkbenchTerminalContextId::parse(&context_id)?,
            label: WorkbenchLabel::parse("launch tab")?,
            geometry: WorkbenchTerminalGeometry::new(80, 24, 9, 18)?,
            launch: Some(LaunchSelection::new(
                "/bin/sh".to_owned(),
                vec![
                    "-c".to_owned(),
                    script.clone(),
                    "tag0".to_owned(),
                    String::new(),
                    "-x".to_owned(),
                    "漢字".to_owned(),
                ],
            )?),
            profile: None,
            options: Default::default(),
        },
    };

    // Identical replay: the retained outcome echoes the same tab — no second
    // spawn. Pending and settled forms both carry the identity.
    let outcome = replay_command(&fixture, &command)?;
    let replayed_tab = match outcome {
        WorkbenchCommandOutcome::TabCreatePending { tab_id, .. }
        | WorkbenchCommandOutcome::TabCreated { tab_id, .. } => tab_id,
        other => panic!("identical replay returned {other:?}"),
    };
    assert_eq!(replayed_tab.as_str(), tab_id.as_str());
    thread::sleep(SETTLE_WINDOW);
    assert_eq!(
        fs::read_to_string(&spawns)?.matches("RUN\n").count(),
        1,
        "identical replay must not spawn again"
    );

    // Same request identity, changed selection: conflict before any effect.
    // The conflicting program would record its own marker if it ever ran —
    // a plain `true` could not be observed either way.
    let conflict_proof = directory.join("conflict.txt");
    let conflict_script = format!("printf 'CONFLICT\\n' >> '{}'", conflict_proof.display());
    let mut conflicted = command.clone();
    conflicted.action = WorkbenchAction::TabCreate {
        automatic_label: false,
        placement: None,
        terminal_context_id: WorkbenchTerminalContextId::parse(&context_id)?,
        label: WorkbenchLabel::parse("launch tab")?,
        geometry: WorkbenchTerminalGeometry::new(80, 24, 9, 18)?,
        launch: Some(LaunchSelection::new(
            "/bin/sh".to_owned(),
            vec!["-c".to_owned(), conflict_script],
        )?),
        profile: None,
        options: Default::default(),
    };
    let outcome = replay_command(&fixture, &conflicted)?;
    assert!(
        matches!(
            outcome,
            WorkbenchCommandOutcome::Rejected {
                reason: WorkbenchRejectionReason::RequestIdentityConflict,
                ..
            }
        ),
        "changed selection under one identity: {outcome:?}"
    );
    thread::sleep(SETTLE_WINDOW);
    assert_eq!(
        fs::read_to_string(&spawns)?.matches("RUN\n").count(),
        1,
        "conflicting replay must not spawn the original program again"
    );
    assert!(
        !conflict_proof.exists(),
        "conflicting selection executed within the observation window"
    );

    // `panel split` carries the same selection contract: the explicit program
    // runs verbatim against the same workspace directory.
    let split_proof = directory.join("split.txt");
    let split_script = format!(
        "{{ printf 'SPLIT\\n'; pwd -P; }} >> '{}'",
        split_proof.display()
    );
    let mut args = vec![
        "panel",
        "split",
        panel_id.as_str(),
        "--direction",
        "right",
        "--label",
        "split pane",
    ];
    args.extend(GEOMETRY);
    args.extend([
        "--exec",
        "/bin/sh",
        "--arg",
        "-c",
        "--arg",
        split_script.as_str(),
        "--format",
        "json",
    ]);
    let (output, split) = run_json(&fixture, &args);
    assert_eq!(
        output.status.code(),
        Some(0),
        "panel split --exec: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(split["result"]["action"], "panel.split");
    let observed = wait_proof(&split_proof, Instant::now() + SPAWN_DEADLINE);
    assert_eq!(observed, format!("SPLIT\n{directory_text}\n"), "split cwd");

    Ok(())
}
