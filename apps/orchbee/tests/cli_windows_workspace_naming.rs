//! A Workspace follows its first terminal's directory: the CLI, Server, Worker and Zig Host under
//! test, with a TUI Client running in a terminal of a second Session. A `cd` in the first terminal
//! of a Workspace whose name was generated renames it until a rename fixes the name, the recorded
//! terminal directory and the sidebar's folder line follow it either way, and a new Workspace
//! created from the TUI starts where that terminal now is. A row's name and location line move once
//! per `cd`, together.
#![cfg(windows)]

#[allow(dead_code)]
#[path = "support/windows_tui_session.rs"]
mod session;

use serde_json::Value;
use session::{Fixture, OBSERVATION};
use std::{
    fs,
    process::Command,
    thread,
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
        until(OBSERVATION, || sidebar_folder_is(
            &fixture.screen(&tui.terminal),
            "beta",
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

/// Every visit but the last is the first to its directory, so no earlier Git answer helps: a lagging
/// line would name the last directory's branch or folder under the new name.
#[test]
fn a_first_visit_changes_the_sidebar_name_and_location_line_together() {
    let fixture = Fixture::new(
        "orchbee-first-visit-",
        &["start", "first", "second", "outside"],
    );
    repository(&fixture, "first", "main");
    repository(&fixture, "second", "topic");
    let created = create_unnamed(&fixture, "start");
    let terminal = terminal_of(&fixture, &created);
    let tui = fixture.start_tui(COLUMNS, ROWS);
    fixture.settle_ready(&tui.client);

    let mut painted = Vec::new();
    let mut sample = |until_step: Option<&(String, String)>| {
        let screen = fixture.screen(&tui.terminal);
        let row = ["start", "first", "second", "outside"]
            .into_iter()
            .find_map(|name| Some((name.to_owned(), location_line(&screen, name)?)));
        if let Some(row) = row
            && painted.last() != Some(&row)
        {
            painted.push(row);
        }
        (screen, painted.last() == until_step)
    };
    let mut reach = |name: &str, line: &str| {
        let step = (name.to_owned(), line.to_owned());
        let deadline = Instant::now() + FOLLOW;
        loop {
            let (screen, reached) = sample(Some(&step));
            if reached {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "never reached {step:?}:\n{screen}"
            );
        }
    };
    reach("start", "start");
    for (directory, line) in [
        ("first", "main"),
        ("second", "topic"),
        ("outside", "outside"),
        ("first", "main"),
    ] {
        change_directory(&fixture, &terminal, directory);
        reach(directory, line);
    }
    // Time for a later sampled Git read to change what the row already painted.
    let settled = Instant::now() + Duration::from_secs(5);
    while Instant::now() < settled {
        sample(None);
    }
    let expected = [
        ("start", "start"),
        ("first", "main"),
        ("second", "topic"),
        ("outside", "outside"),
        ("first", "main"),
    ]
    .map(|(name, line)| (name.to_owned(), line.to_owned()));
    assert_eq!(painted, expected);
}

#[test]
fn a_directory_change_moves_the_sidebar_location_line_in_one_step() {
    let fixture = Fixture::new("orchbee-location-", &["repo", "outside"]);
    repository(&fixture, "repo", "main");
    // Fixed names keep the rows findable while their terminals move.
    let created = create_unnamed(&fixture, "repo");
    let terminal = terminal_of(&fixture, &created);
    let id = created["workspaceId"].as_str().unwrap();
    let renamed = fixture.run(&["workspace", "rename", id, "--label", "moved"]);
    assert_eq!(renamed.code, Some(0), "{renamed}");
    let other = create_unnamed(&fixture, "repo");
    let id = other["workspaceId"].as_str().unwrap();
    let renamed = fixture.run(&["workspace", "rename", id, "--label", "other"]);
    assert_eq!(renamed.code, Some(0), "{renamed}");
    let tui = fixture.start_tui(COLUMNS, ROWS);
    fixture.settle_ready(&tui.client);

    let mut moved = Vec::new();
    let mut unmoved = Vec::new();
    let mut sample = || {
        let screen = fixture.screen(&tui.terminal);
        for (label, painted) in [("moved", &mut moved), ("other", &mut unmoved)] {
            if let Some(line) = location_line(&screen, label)
                && painted.last() != Some(&line)
            {
                painted.push(line);
            }
        }
        (screen.clone(), location_line(&screen, "moved"))
    };
    let reach = |value: &str, sample: &mut dyn FnMut() -> (String, Option<String>)| {
        let deadline = Instant::now() + FOLLOW;
        loop {
            let (screen, line) = sample();
            if line.as_deref() == Some(value) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "never reached {value}:
{screen}"
            );
        }
    };
    reach("main", &mut sample);
    change_directory(&fixture, &terminal, "outside");
    reach("outside", &mut sample);
    change_directory(&fixture, &terminal, "repo");
    reach("main", &mut sample);
    // Time for a later sampled Git read to change what the commit already painted.
    let settled = Instant::now() + Duration::from_secs(5);
    while Instant::now() < settled {
        sample();
    }
    assert_eq!(moved, ["main", "outside", "main"]);
    assert_eq!(unmoved, ["main"]);
}

/// Makes the fixture directory a repository with one commit on `branch`.
fn repository(fixture: &Fixture, directory: &str, branch: &str) {
    let initial_branch = format!("--initial-branch={branch}");
    for words in [
        &["init", "--quiet", &initial_branch][..],
        &[
            "-c",
            "user.name=Orchbee Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "seed",
        ],
    ] {
        let git = Command::new("git")
            .args(words)
            .current_dir(fixture.path(directory))
            .output()
            .unwrap();
        assert!(git.status.success(), "git {words:?}: {git:?}");
    }
}

/// The line below `label`'s sidebar row, with a folder reduced to its last component, which middle
/// truncation keeps. Anything else, a marker included, stays as painted.
fn location_line(screen: &str, label: &str) -> Option<String> {
    let cells = |line: &'_ str| line.split('│').next().unwrap_or_default().trim().to_owned();
    let mut lines = screen.lines();
    lines.find(|line| cells(line).starts_with(label))?;
    let line = cells(lines.next()?);
    Some(match line.rsplit(['\\', '/']).next() {
        Some(last) if line.contains(['\\', '/']) => last.to_owned(),
        _ => line,
    })
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

/// Whether the Workspace row named `label` has a folder line reading `folder`: outside Git the
/// second line names only the folder's last component.
fn sidebar_folder_is(screen: &str, label: &str, folder: &str) -> bool {
    let cells: Vec<&str> = screen
        .lines()
        .map(|line| line.split('│').next().unwrap_or_default().trim())
        .collect();
    cells.windows(2).any(|pair| {
        pair[0]
            .strip_prefix(label)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
            && pair[1] == folder
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
