//! A Workspace follows its first terminal's directory: the CLI, Server, Worker and Zig Host under
//! test, with a TUI Client running in a terminal of a second Session. A `cd` in the first terminal
//! of a Workspace whose name was generated renames it until a rename fixes the name, the recorded
//! terminal directory and the sidebar's folder line follow it either way, and a new Workspace
//! created from the TUI starts where that terminal now is.
#![cfg(windows)]

#[allow(dead_code)]
#[path = "support/windows_tui_session.rs"]
mod session;

use serde_json::Value;
use session::{Fixture, OBSERVATION};
use std::{
    fs, thread,
    time::{Duration, Instant},
};

const COLUMNS: u16 = 112;
const ROWS: u16 = 32;
/// Bounds how long a `cd` takes to reach a committed label through the Server's process sampler.
const FOLLOW: Duration = Duration::from_secs(20);

#[test]
fn a_generated_workspace_name_follows_the_first_terminal_until_a_rename_fixes_it() {
    let fixture = Fixture::new("orchbee-naming-", &["alpha", "beta", "gamma", "delta"]);
    let created = create_unnamed(&fixture, "alpha");
    let workspace = created["workspaceId"].as_str().unwrap().to_owned();
    assert_eq!(record(&fixture, &workspace)["label"], "alpha");
    assert_eq!(record(&fixture, &workspace)["automaticLabel"], true);
    let terminal = terminal_of(&fixture, &created);

    change_directory(&fixture, &terminal, "beta");
    assert!(
        until(FOLLOW, || record(&fixture, &workspace)["label"] == "beta"),
        "the name never followed the first terminal: {}",
        record(&fixture, &workspace)
    );

    // Confirming the same text is a rename, so it fixes the name.
    let renamed = fixture.run(&["workspace", "rename", &workspace, "--label", "beta"]);
    assert_eq!(renamed.code, Some(0), "{renamed}");
    assert_eq!(record(&fixture, &workspace)["automaticLabel"], Value::Null);

    // A second generated name proves the sampler has seen both later `cd`s.
    let witness = create_unnamed(&fixture, "alpha");
    let witness_id = witness["workspaceId"].as_str().unwrap().to_owned();
    let witness_terminal = terminal_of(&fixture, &witness);
    change_directory(&fixture, &terminal, "gamma");
    change_directory(&fixture, &witness_terminal, "delta");
    assert!(
        until(FOLLOW, || record(&fixture, &witness_id)["label"] == "delta"),
        "the witness never followed its first terminal: {}",
        record(&fixture, &witness_id)
    );
    thread::sleep(Duration::from_secs(1));
    let fixed = record(&fixture, &workspace);
    assert_eq!(fixed["label"], "beta");
    // A fixed name still records where its first terminal is.
    assert!(
        fixed["terminalDirectory"]
            .as_str()
            .is_some_and(|directory| directory.ends_with("gamma")),
        "the renamed Workspace never recorded its terminal directory: {fixed}"
    );
}

#[test]
fn a_new_workspace_starts_in_the_selected_workspace_first_terminal_directory() {
    let fixture = Fixture::new("orchbee-new-in-", &["alpha", "beta"]);
    let created = create_unnamed(&fixture, "alpha");
    let workspace = created["workspaceId"].as_str().unwrap().to_owned();
    let terminal = terminal_of(&fixture, &created);
    let tui = fixture.start_tui(COLUMNS, ROWS);
    fixture.settle_ready(&tui.client);

    change_directory(&fixture, &terminal, "beta");
    assert!(
        until(FOLLOW, || sidebar_shows(
            &fixture.screen(&tui.terminal),
            "beta"
        )),
        "the sidebar never showed the followed name:\n{}",
        fixture.screen(&tui.terminal)
    );
    assert!(
        until(OBSERVATION, || sidebar_folder_ends_with(
            &fixture.screen(&tui.terminal),
            "beta"
        )),
        "the sidebar's folder line never followed the first terminal:\n{}",
        fixture.screen(&tui.terminal)
    );

    fixture.keys(&tui.terminal, &["ctrl+b", "a"]);
    let mut added = Value::Null;
    assert!(
        until(OBSERVATION * 3, || {
            added = snapshot(&fixture)["navigation"]["workspaces"]
                .as_array()
                .and_then(|workspaces| {
                    workspaces
                        .iter()
                        .find(|listed| listed["workspaceId"] != workspace.as_str())
                        .cloned()
                })
                .unwrap_or(Value::Null);
            !added.is_null()
        }),
        "the TUI never created a Workspace:\n{}",
        fixture.screen(&tui.terminal)
    );
    let directory = directory_of(&fixture, &added);
    // The shell reports its directory with a trailing separator that the Workspace must not keep.
    assert!(
        !directory.ends_with(['\\', '/']),
        "the new Workspace kept a trailing separator: {directory}"
    );
    assert_eq!(
        fs::canonicalize(&directory).unwrap(),
        fs::canonicalize(fixture.path("beta")).unwrap(),
        "the new Workspace started in {directory}"
    );
    assert_eq!(added["label"], "beta (2)");
    assert_eq!(added["automaticLabel"], true);
}

/// Opens a Workspace without a label whose terminal starts in `directory`, and returns its
/// creation result.
fn create_unnamed(fixture: &Fixture, directory: &str) -> Value {
    let cwd = fixture.path(directory);
    let mut words = vec!["workspace", "create", "--cwd", &cwd, "--no-focus"];
    let environment = fixture.terminal_environment();
    words.extend(environment.iter().map(String::as_str));
    let created = fixture.run(&words);
    assert_eq!(created.code, Some(0), "{created}");
    created.document["workspace"]["result"].clone()
}

/// The terminal bound to the creation result's Panel.
fn terminal_of(fixture: &Fixture, created: &Value) -> String {
    let listed = fixture.run(&["terminal", "list"]);
    listed.document["result"]["terminals"]
        .as_array()
        .and_then(|terminals| {
            terminals
                .iter()
                .find(|terminal| terminal["boundTo"]["panelId"] == created["panelId"])
        })
        .and_then(|terminal| terminal["terminalId"].as_str())
        .unwrap_or_else(|| panic!("{listed}"))
        .to_owned()
}

/// Types `cd` into `terminal`'s shell, toward one of the fixture's directories.
fn change_directory(fixture: &Fixture, terminal: &str, directory: &str) {
    let command = format!("cd \"{}\"", fixture.path(directory));
    for words in [
        ["terminal", "send", terminal, "--text", &command],
        ["terminal", "send", terminal, "--key", "enter"],
    ] {
        let sent = fixture.run(&words);
        assert_eq!(sent.code, Some(0), "{sent}");
    }
}

fn snapshot(fixture: &Fixture) -> Value {
    let read = fixture.run(&["api", "snapshot"]);
    assert_eq!(read.code, Some(0), "{read}");
    read.document["result"]["snapshot"].clone()
}

/// The canonical Workspace record, including its optional `automaticLabel`.
fn record(fixture: &Fixture, workspace: &str) -> Value {
    let snapshot = snapshot(fixture);
    snapshot["navigation"]["workspaces"]
        .as_array()
        .and_then(|workspaces| {
            workspaces
                .iter()
                .find(|listed| listed["workspaceId"] == workspace)
                .cloned()
        })
        .unwrap_or_else(|| panic!("no Workspace {workspace}: {snapshot}"))
}

fn directory_of(fixture: &Fixture, workspace: &Value) -> String {
    let snapshot = snapshot(fixture);
    snapshot["navigation"]["directories"]
        .as_array()
        .and_then(|directories| {
            directories
                .iter()
                .find(|directory| directory["directoryId"] == workspace["primaryDirectoryId"])
        })
        .and_then(|directory| directory["path"].as_str())
        .unwrap_or_else(|| panic!("no directory for {workspace}: {snapshot}"))
        .to_owned()
}

/// Whether a sidebar row, the screen's first column, starts with `label`.
fn sidebar_shows(screen: &str, label: &str) -> bool {
    screen.lines().any(|line| {
        line.split('│')
            .next()
            .and_then(|cell| cell.trim().strip_prefix(label))
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
    })
}

/// Whether a sidebar row shows a folder whose last component is `name`; a folder truncates in
/// the middle, so its end stays visible.
fn sidebar_folder_ends_with(screen: &str, name: &str) -> bool {
    screen.lines().any(|line| {
        line.split('│').next().is_some_and(|cell| {
            cell.trim()
                .strip_suffix(name)
                .is_some_and(|rest| rest.ends_with(['\\', '/']))
        })
    })
}

/// Polls `observed` until it holds or `bound` passes.
fn until(bound: Duration, mut observed: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + bound;
    while Instant::now() < deadline {
        if observed() {
            return true;
        }
        thread::sleep(Duration::from_millis(200));
    }
    false
}
