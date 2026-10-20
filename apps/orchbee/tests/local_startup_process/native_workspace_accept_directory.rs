//! Public headless chain witnesses for `workspace accept-directory`: the real
//! staged `orchbee` binary drives the accepted `WorkspacePrimaryDirectoryAccept`
//! action against a live isolated Site — with no TUI client and no directory
//! wizard anywhere in the sequence. `project create → workspace create →
//! workspace accept-directory → tab create → terminal list/read → tab close
//! → bounded list observation` is the genuinely headless chain the accepted
//! slice authorizes; the negative matrix proves invalid paths exit at the CLI
//! boundary or through the Server's own probe (zero mkdir, zero committed
//! metadata), repeated acceptance stays the Server's conflict, and duplicate
//! labels stay ambiguous while typed ids resolve.

use super::{
    native_cli_query::{run_cli, run_json, spawn_real_server, validate, validators},
    *,
};

/// The tab's terminal spawn is asynchronous; poll the real binding instead of
/// trusting the mutation receipt — `pending` is not a live shell.
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

/// `project create` → `workspace create` → `workspace accept-directory` →
/// `tab create` → `terminal list`/`terminal read` → `tab close --confirm` →
/// bounded `tab list`/`terminal list` observation — every step a real CLI
/// invocation, every id chained out of the previous JSON receipt, zero TUI
/// keys, zero wizard surfaces. The close stays `accepted` (a receipt, not a
/// deletion); the bounded observation is what proves the tabId/terminalId
/// are gone, and the close is never re-sent.
#[test]
fn accept_directory_completes_the_headless_chain() -> TestResult {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::new();
    let _server = spawn_real_server(&fixture);
    let validators = validators(&fixture);

    // A fresh empty directory is the only filesystem fixture this slice
    // needs; its name deliberately carries CJK and spaces so the wire and
    // receipt prove the literal-path contract on non-ASCII input.
    let directory = fixture.base.join("cli 已接受 目录 dir");
    fs::create_dir(&directory)?;
    let path = directory
        .to_str()
        .ok_or("fixture directory UTF-8")?
        .to_owned();

    // 1. `project create` — the committed projectId chains the workspace.
    let (output, project) = run_json(
        &fixture,
        &["project", "create", "--label", "chain", "--format", "json"],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "project create: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    validate(&validators, "orchbee.mutation.result", &output);
    let project_id = project["result"]["projectId"]
        .as_str()
        .expect("commit-allocated projectId")
        .to_owned();

    // 2. `workspace create` — inside the new Project, addressed by id.
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
    validate(&validators, "orchbee.mutation.result", &output);
    let workspace_id = workspace["result"]["workspaceId"]
        .as_str()
        .expect("workspace id")
        .to_owned();
    let context_id = workspace["result"]["terminalContextId"]
        .as_str()
        .expect("terminal context id")
        .to_owned();

    // 3. `workspace accept-directory` — the committed receipt carries the
    //    Server-allocated directory and worker ids, the resolved project and
    //    workspace ids, and the literal request path.
    let (output, accepted) = run_json(
        &fixture,
        &[
            "workspace",
            "accept-directory",
            workspace_id.as_str(),
            "--path",
            path.as_str(),
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
    validate(&validators, "orchbee.mutation.result", &output);
    assert_eq!(accepted["result"]["action"], "workspace.accept-directory");
    assert_eq!(accepted["result"]["outcome"], "committed");
    assert_eq!(accepted["result"]["projectId"], project_id.as_str());
    assert_eq!(accepted["result"]["workspaceId"], workspace_id.as_str());
    assert_eq!(
        accepted["result"]["path"],
        path.as_str(),
        "the receipt echoes the literal request path"
    );
    let directory_id = accepted["result"]["directoryId"]
        .as_str()
        .expect("Server-allocated directoryId")
        .to_owned();
    let worker_id = accepted["result"]["workerId"]
        .as_str()
        .expect("Server-allocated workerId")
        .to_owned();
    assert_eq!(directory_id.len(), 32, "typed directory id: {directory_id}");
    assert_eq!(worker_id.len(), 32, "typed worker id: {worker_id}");

    // The Server's own association: `workspace get` reports the same
    // directory id the commit returned.
    let (output, record) = run_json(
        &fixture,
        &[
            "workspace",
            "get",
            workspace_id.as_str(),
            "--format",
            "json",
        ],
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        record["result"]["workspace"]["primaryDirectoryId"],
        directory_id.as_str()
    );

    // 4. `tab create --workspace` now commits — the directory precondition is
    //    satisfied without any TUI wizard.
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
    let (output, tab) = run_json(&fixture, &tab_create);
    assert_eq!(
        output.status.code(),
        Some(0),
        "tab create: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    validate(&validators, "orchbee.mutation.result", &output);
    assert_eq!(tab["result"]["action"], "tab.create");
    assert_eq!(tab["result"]["terminalContextId"], context_id.as_str());
    let tab_id = tab["result"]["tabId"].as_str().expect("tab id").to_owned();

    // 5. The real binding: poll `terminal list` until the new tab's panel
    //    carries a bound terminal — never assume the receipt means a shell.
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

    // 6. `terminal read` on the real bound id returns a live grid.
    let (output, read) = run_json(
        &fixture,
        &["terminal", "read", terminal_id.as_str(), "--format", "json"],
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
    assert!(
        !read["result"]["content"]["text"]
            .as_str()
            .expect("rows")
            .is_empty(),
        "a live bound terminal emits grid rows"
    );
    validate(&validators, "orchbee.terminal.output", &output);

    // 7. `tab close --confirm` — the destructive close completes the chain.
    let (output, closed) = run_json(
        &fixture,
        &[
            "tab",
            "close",
            tab_id.as_str(),
            "--confirm",
            "--format",
            "json",
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "tab close: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    validate(&validators, "orchbee.mutation.result", &output);
    assert_eq!(closed["result"]["action"], "tab.close");
    assert_eq!(closed["result"]["outcome"], "accepted");
    assert!(
        closed["result"]["closeRequestId"].as_str().is_some(),
        "the asynchronous close carries its request id"
    );

    // 8. `accepted` is a receipt, not proof of deletion: poll the public
    //    listings until neither the tabId nor the terminalId is exposed any
    //    more. The close is submitted exactly once — only bounded read-only
    //    observation follows, never a resend.
    let deadline = Instant::now() + BIND_DEADLINE;
    loop {
        let (output, tabs) = run_json(
            &fixture,
            &[
                "tab",
                "list",
                "--workspace",
                workspace_id.as_str(),
                "--format",
                "json",
            ],
        );
        assert_eq!(
            output.status.code(),
            Some(0),
            "tab list: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let tab_visible = tabs["result"]["tabs"]
            .as_array()
            .expect("tabs array")
            .iter()
            .any(|row| row["tabId"] == tab_id);
        let (output, terminals) = run_json(&fixture, &["terminal", "list", "--format", "json"]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "terminal list: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let terminal_visible = terminals["result"]["terminals"]
            .as_array()
            .expect("terminals array")
            .iter()
            .any(|row| row["terminalId"] == terminal_id);
        if !tab_visible && !terminal_visible {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "accepted close never settled: tab {tab_id} visible={tab_visible}, \
             terminal {terminal_id} visible={terminal_visible}"
        );
        thread::sleep(BIND_POLL);
    }
    Ok(())
}

/// The Server stays authoritative over the path probe and the conflict: a
/// missing path and a non-directory both fail `DirectoryUnavailable` with zero
/// mkdir and zero committed metadata, a repeated acceptance — even the same
/// path — is `WorkspaceDirectoryConflict`, and duplicate workspace labels are
/// `ambiguous` while each typed id still resolves.
#[test]
fn accept_directory_negative_matrix_stays_server_owned() -> TestResult {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::new();
    let _server = spawn_real_server(&fixture);
    let validators = validators(&fixture);

    let (output, project) = run_json(
        &fixture,
        &["project", "create", "--label", "matrix", "--format", "json"],
    );
    assert_eq!(output.status.code(), Some(0));
    let project_id = project["result"]["projectId"].as_str().unwrap().to_owned();
    let (output, workspace) = run_json(
        &fixture,
        &[
            "workspace",
            "create",
            "--project",
            project_id.as_str(),
            "--label",
            "one",
            "--format",
            "json",
        ],
    );
    assert_eq!(output.status.code(), Some(0));
    let workspace_id = workspace["result"]["workspaceId"]
        .as_str()
        .unwrap()
        .to_owned();

    // 1. A missing path: the Server's probe answers `DirectoryUnavailable`
    //    (exit 4) — nothing is created and nothing is committed.
    let missing = fixture.base.join("never-created-dir");
    let missing_path = missing.to_str().ok_or("missing path UTF-8")?.to_owned();
    let (output, failure) = run_json(
        &fixture,
        &[
            "workspace",
            "accept-directory",
            workspace_id.as_str(),
            "--path",
            missing_path.as_str(),
            "--format",
            "json",
        ],
    );
    assert_eq!(output.status.code(), Some(4));
    assert_eq!(failure["failure"]["kind"], "service-unavailable");
    assert_eq!(failure["failure"]["details"]["stage"], "command");
    assert_eq!(
        failure["failure"]["details"]["reason"],
        "directory-unavailable"
    );
    validate(&validators, "orchbee.cli.failure", &output);
    assert!(
        !missing.exists(),
        "neither the CLI nor the Server may create the path"
    );
    let (output, record) = run_json(
        &fixture,
        &[
            "workspace",
            "get",
            workspace_id.as_str(),
            "--format",
            "json",
        ],
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(
        record["result"]["workspace"]["primaryDirectoryId"].is_null(),
        "a failed probe commits no directory metadata"
    );

    // 2. A non-directory path probes the same way: regular file, exit 4.
    let file = fixture.base.join("a file, not a dir");
    fs::write(&file, b"not a directory")?;
    let file_path = file.to_str().ok_or("file path UTF-8")?.to_owned();
    let (output, failure) = run_json(
        &fixture,
        &[
            "workspace",
            "accept-directory",
            workspace_id.as_str(),
            "--path",
            file_path.as_str(),
            "--format",
            "json",
        ],
    );
    assert_eq!(output.status.code(), Some(4));
    assert_eq!(
        failure["failure"]["details"]["reason"],
        "directory-unavailable"
    );
    let (output, record) = run_json(
        &fixture,
        &[
            "workspace",
            "get",
            workspace_id.as_str(),
            "--format",
            "json",
        ],
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(record["result"]["workspace"]["primaryDirectoryId"].is_null());

    // 3. A valid accept commits — then a second acceptance, even of the same
    //    path under a fresh request, is the Server's `WorkspaceDirectoryConflict`
    //    (exit 3), never a client-side idempotent success. The accepted path
    //    again carries CJK and spaces through the literal contract.
    let directory = fixture.base.join("real 目录 dir");
    fs::create_dir(&directory)?;
    let path = directory.to_str().ok_or("dir path UTF-8")?.to_owned();
    let accept = [
        "workspace",
        "accept-directory",
        workspace_id.as_str(),
        "--path",
        path.as_str(),
        "--format",
        "json",
    ];
    let (output, committed) = run_json(&fixture, &accept);
    assert_eq!(
        output.status.code(),
        Some(0),
        "first accept: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(committed["result"]["outcome"], "committed");

    let (output, failure) = run_json(&fixture, &accept);
    assert_eq!(
        output.status.code(),
        Some(3),
        "repeated acceptance stays the Server conflict"
    );
    assert_eq!(failure["failure"]["kind"], "rejected");
    assert_eq!(
        failure["failure"]["details"]["reason"],
        "workspace-directory-conflict"
    );
    validate(&validators, "orchbee.cli.failure", &output);

    // 4. Two more workspaces sharing one label: the label target is honestly
    //    `ambiguous` (zero commands submitted), while each typed id still
    //    resolves — here onto a fresh workspace that commits.
    let mut dup_ids = Vec::new();
    for _ in 0..2 {
        let (output, dup) = run_json(
            &fixture,
            &[
                "workspace",
                "create",
                "--project",
                project_id.as_str(),
                "--label",
                "dup",
                "--format",
                "json",
            ],
        );
        assert_eq!(output.status.code(), Some(0));
        dup_ids.push(dup["result"]["workspaceId"].as_str().unwrap().to_owned());
    }
    let (output, failure) = run_json(
        &fixture,
        &[
            "workspace",
            "accept-directory",
            "dup",
            "--path",
            path.as_str(),
            "--format",
            "json",
        ],
    );
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(failure["failure"]["kind"], "ambiguous");
    assert_eq!(failure["failure"]["details"]["resource"], "workspace");

    let accept_dup = [
        "workspace",
        "accept-directory",
        dup_ids[0].as_str(),
        "--path",
        path.as_str(),
        "--format",
        "json",
    ];
    let (output, committed) = run_json(&fixture, &accept_dup);
    assert_eq!(
        output.status.code(),
        Some(0),
        "id-addressed accept: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(committed["result"]["workspaceId"], dup_ids[0].as_str());
    assert_eq!(committed["result"]["projectId"], project_id.as_str());
    validate(&validators, "orchbee.mutation.result", &output);

    // 5. The CLI-side boundary: a relative path is a usage failure (exit 2)
    //    before any runtime access — the Server sees nothing.
    let output = run_cli(
        &fixture,
        &[
            "workspace",
            "accept-directory",
            dup_ids[1].as_str(),
            "--path",
            "relative/dir",
            "--format",
            "json",
        ],
    );
    assert_eq!(output.status.code(), Some(2));
    let document: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(document["failure"]["kind"], "invalid-usage");
    let (output, record) = run_json(
        &fixture,
        &["workspace", "get", dup_ids[1].as_str(), "--format", "json"],
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(record["result"]["workspace"]["primaryDirectoryId"].is_null());
    Ok(())
}
