//! Public headless chain witnesses for `project create`: the real staged
//! `orchbee` binary drives the accepted `ProjectCreate` action against a live
//! isolated Site — first an empty Project object, then the JSON-id chained
//! `workspace create` and `tab create`, ending at a real bound terminal.
//! The workspace-bound tab honestly exercises the Server's existing
//! primary-directory precondition; the duplicate-label contract mirrors the
//! Server: both Projects commit, label targets go `ambiguous`, and typed ids
//! stay exact.

use super::{
    native_cli_query::{run_json, spawn_real_server, validate, validators},
    native_workspace_fixture::{
        DIRECTORY_ACCEPTED, DIRECTORY_NEEDED, DIRECTORY_PATH, DIRECTORY_READY, EMPTY, TEXT_ACTIONS,
        modal,
    },
    *,
};

/// The tab's terminal spawn is asynchronous; `pending` is not a live shell, so
/// the witness polls the real binding with a bounded deadline instead of
/// trusting the mutation receipt.
const BIND_DEADLINE: Duration = Duration::from_secs(20);
const BIND_POLL: Duration = Duration::from_millis(150);
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
const PROJECT_LABEL: &str = "chain";
const WORKSPACE_LABEL: &str = "main";

/// Empty Project space → `project create` → `workspace create --project <id>`.
/// The first `tab create --workspace` hits the Server's existing boundary —
/// a workspace without a primary directory cannot host a terminal — then the
/// TUI's own directory-accept surface satisfies it and the same CLI command
/// commits, binds a real terminal, and reads a live frame.
#[test]
fn project_workspace_tab_chain_binds_a_real_terminal() -> TestResult {
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
    let validators = validators(&fixture);

    // 1. Empty object space: the only pre-existing Project surface is empty.
    let (output, projects) = run_json(&fixture, &["project", "list", "--format", "json"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        projects["result"]["projects"].as_array().unwrap().len(),
        0,
        "the chain starts from an empty Project space"
    );

    // 2. `project create` — the receipt's projectId is the Server-allocated
    // commit identity, the label echoes back, and no directory exists.
    let (output, created) = run_json(
        &fixture,
        &[
            "project",
            "create",
            "--label",
            PROJECT_LABEL,
            "--format",
            "json",
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "project create: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    validate(&validators, "orchbee.mutation.result", &output);
    assert_eq!(created["result"]["action"], "project.create");
    assert_eq!(created["result"]["outcome"], "committed");
    assert_eq!(created["result"]["label"], PROJECT_LABEL);
    let project_id = created["result"]["projectId"]
        .as_str()
        .expect("commit-allocated projectId")
        .to_owned();
    assert_eq!(project_id.len(), 32, "typed project id: {project_id}");
    let (output, project) = run_json(
        &fixture,
        &["project", "get", &project_id, "--format", "json"],
    );
    assert_eq!(output.status.code(), Some(0));
    let record = &project["result"]["project"];
    assert_eq!(record["label"], PROJECT_LABEL);
    assert_eq!(
        record["directories"].as_array().unwrap().len(),
        0,
        "a created Project owns no directory"
    );
    assert_eq!(record["workspaces"].as_array().unwrap().len(), 0);

    // 3. `workspace create` inside the new Project, addressed by the committed
    // id (not the label — labels are allowed to repeat).
    let (output, workspace) = run_json(
        &fixture,
        &[
            "workspace",
            "create",
            "--project",
            &project_id,
            "--label",
            WORKSPACE_LABEL,
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
    validate(&validators, "orchbee.mutation.result", &output);
    assert_eq!(workspace["result"]["action"], "workspace.create");
    assert_eq!(workspace["result"]["projectId"], project_id.as_str());
    let workspace_id = workspace["result"]["workspaceId"]
        .as_str()
        .expect("workspace id")
        .to_owned();

    let workspace_header = format!("WORKBENCH · {PROJECT_LABEL} / {WORKSPACE_LABEL}");
    let mut tab_create = vec![
        "tab",
        "create",
        "--workspace",
        workspace_id.as_str(),
        "--label",
        "sh",
    ];
    tab_create.extend(GEOMETRY);
    tab_create.extend(["--format", "json"]);

    // 4. Reach the CLI-created workspace context in the TUI. The committed
    //    publication propagates asynchronously, so step the context selector
    //    only while the workspace header is absent; a switch completing must
    //    then hold for a short settle — a queued key flipping it back simply
    //    restarts the loop, and every sent key is consumed exactly once, so
    //    this converges instead of oscillating.
    let deadline = Instant::now() + OBSERVATION_TIMEOUT;
    let checkpoint = client.checkpoint();
    let mut settle: Option<Instant> = None;
    loop {
        client.collect();
        let now = Instant::now();
        if client.contains(&workspace_header) {
            match settle {
                Some(until) if now >= until => break,
                None => settle = Some(now + Duration::from_millis(150)),
                Some(_) => {}
            }
        } else {
            settle = None;
            assert!(now < deadline, "workspace context never stayed selected");
            client.send(b"\x02]");
            thread::sleep(Duration::from_millis(400));
        }
        thread::yield_now();
    }
    client.wait_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &[workspace_header.as_str(), EMPTY, DIRECTORY_NEEDED],
        &["[*1"],
        &[],
        &mut [],
    );

    // 5. The honest boundary: the Server rejects a workspace-bound tab whose
    //    workspace has no primary directory — an existing precondition the CLI
    //    reports, not a scope this task expands.
    let (output, failure) = run_json(&fixture, &tab_create);
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(failure["failure"]["kind"], "rejected");
    assert_eq!(failure["failure"]["details"]["stage"], "command");
    assert_eq!(
        failure["failure"]["details"]["reason"],
        "workspace-directory-unavailable"
    );

    // 6. Satisfy the precondition through the only existing path — the TUI
    //    directory-accept surface — then the same CLI command must commit.
    let directory = fixture.base.join("cli chain dir");
    fs::create_dir(&directory)?;
    let directory = fs::canonicalize(directory)?;
    let checkpoint = client.checkpoint();
    client.send(b"\x02e");
    let hint = [
        "Literal absolute path, at most 4,096 UTF-8 bytes, without NUL. No expansion or",
        "directory creation.",
    ];
    modal(
        &mut client,
        checkpoint,
        workspace_header.as_str(),
        &[
            DIRECTORY_PATH,
            "",
            &format!("Project: {PROJECT_LABEL}"),
            "",
            &format!("Workspaces: {WORKSPACE_LABEL}"),
            "",
            "",
            "",
            hint[0],
            hint[1],
            "",
            TEXT_ACTIONS,
        ],
    );
    let checkpoint = client.checkpoint();
    let path = directory.to_str().ok_or("directory path UTF-8")?;
    client.send(path.as_bytes());
    modal(
        &mut client,
        checkpoint,
        workspace_header.as_str(),
        &[
            DIRECTORY_PATH,
            "",
            &format!("Project: {PROJECT_LABEL}"),
            "",
            &format!("Workspaces: {WORKSPACE_LABEL}"),
            "",
            path,
            "",
            hint[0],
            hint[1],
            "",
            TEXT_ACTIONS,
        ],
    );
    let checkpoint = client.checkpoint();
    client.send(b"\r");
    client.wait_frame_text_after(
        checkpoint,
        Instant::now() + SWITCH_TIMEOUT,
        &[
            workspace_header.as_str(),
            EMPTY,
            DIRECTORY_READY,
            DIRECTORY_ACCEPTED,
        ],
        &["[*1"],
        &[],
        &mut [],
    );

    // 7. `tab create --workspace` now commits; the tab's terminal context is
    //    the one `workspace create` allocated.
    let (output, tab) = run_json(&fixture, &tab_create);
    assert_eq!(
        output.status.code(),
        Some(0),
        "tab create: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    validate(&validators, "orchbee.mutation.result", &output);
    assert_eq!(tab["result"]["action"], "tab.create");
    let context_id = tab["result"]["terminalContextId"]
        .as_str()
        .expect("terminal context id")
        .to_owned();
    let tab_id = tab["result"]["tabId"].as_str().expect("tab id").to_owned();

    // 8. The real binding: poll `terminal list` until the new tab's panel
    //    carries a bound terminal — `pending` in the receipt is not a live shell.
    let deadline = Instant::now() + BIND_DEADLINE;
    let bound = loop {
        let (output, terminals) = run_json(&fixture, &["terminal", "list", "--format", "json"]);
        assert_eq!(output.status.code(), Some(0));
        if let Some(row) = terminals["result"]["terminals"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["boundTo"]["tabId"] == tab_id)
        {
            break row.clone();
        }
        assert!(
            Instant::now() < deadline,
            "tab {tab_id} never bound a terminal: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        thread::sleep(BIND_POLL);
    };
    let terminal_id = bound["terminalId"].as_str().unwrap().to_owned();
    assert_eq!(bound["boundTo"]["terminalContextId"], context_id.as_str());

    // 9. The bound terminal reads a real frame — a live shell's grid, not a
    //    pending placeholder.
    let (output, read) = run_json(
        &fixture,
        &["terminal", "read", &terminal_id, "--format", "json"],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "terminal read: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        read["result"]["terminal"]["terminalId"],
        terminal_id.as_str()
    );
    assert_eq!(
        read["result"]["terminal"]["boundTo"]["tabId"],
        tab_id.as_str()
    );
    assert!(
        !read["result"]["content"]["rows"]
            .as_array()
            .expect("rows")
            .is_empty(),
        "a live bound terminal emits grid rows"
    );
    validate(&validators, "orchbee.terminal.read", &output);

    client.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}

/// Duplicate labels follow Server semantics: both `project create` invocations
/// commit with distinct allocated ids; a label target then resolves
/// `ambiguous` rather than silently picking one, while each typed id still
/// selects its own Project exactly.
#[test]
fn duplicate_project_labels_stay_id_addressable() -> TestResult {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::new();
    let _server = spawn_real_server(&fixture);

    let mut project_ids = Vec::new();
    for _ in 0..2 {
        let (output, document) = run_json(
            &fixture,
            &["project", "create", "--label", "dup", "--format", "json"],
        );
        assert_eq!(
            output.status.code(),
            Some(0),
            "duplicate project create must commit: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(document["result"]["outcome"], "committed");
        project_ids.push(
            document["result"]["projectId"]
                .as_str()
                .expect("allocated id")
                .to_owned(),
        );
    }
    assert_ne!(
        project_ids[0], project_ids[1],
        "each commit allocates a fresh id"
    );

    // Label targets are honestly ambiguous — zero commands are submitted.
    let (output, failure) = run_json(&fixture, &["project", "get", "dup", "--format", "json"]);
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(failure["failure"]["kind"], "ambiguous");
    assert_eq!(failure["failure"]["details"]["resource"], "project");
    let (output, failure) = run_json(
        &fixture,
        &[
            "workspace",
            "create",
            "--project",
            "dup",
            "--label",
            "w",
            "--format",
            "json",
        ],
    );
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(failure["failure"]["kind"], "ambiguous");

    // Each allocated id still addresses exactly its own Project.
    for id in &project_ids {
        let (output, project) = run_json(
            &fixture,
            &["project", "get", id.as_str(), "--format", "json"],
        );
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(project["result"]["project"]["projectId"], id.as_str());
        assert_eq!(project["result"]["project"]["label"], "dup");
        let (output, workspace) = run_json(
            &fixture,
            &[
                "workspace",
                "create",
                "--project",
                id.as_str(),
                "--label",
                "w",
                "--format",
                "json",
            ],
        );
        assert_eq!(
            output.status.code(),
            Some(0),
            "id-addressed workspace create: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(workspace["result"]["projectId"], id.as_str());
    }

    // A label-filtered list stays usable for operators disambiguating by id.
    let (output, projects) = run_json(&fixture, &["project", "list", "--format", "json"]);
    assert_eq!(output.status.code(), Some(0));
    let rows = projects["result"]["projects"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    for id in &project_ids {
        assert!(
            rows.iter().any(|row| row["projectId"] == id.as_str()),
            "{id} listed"
        );
    }
    Ok(())
}
