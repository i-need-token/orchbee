//! The Worktree rows of an ungrouped Workspace's sidebar menu: the CLI, Server, Worker and Zig Host
//! under test, a real Git repository with one linked checkout, and a TUI Client running in a
//! terminal of a second Session, driven by pointer. Opening the linked checkout from Open Worktree
//! creates the Project that holds the Workspace and the opened one, and a Worktree created from
//! New Worktree… joins it.
#![cfg(windows)]

#[allow(dead_code)]
#[path = "support/windows_tui_session.rs"]
mod session;

use serde_json::Value;
use session::{Control, Fixture, Tui};
use std::{
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const COLUMNS: u16 = 112;
const ROWS: u16 = 32;
/// How long a pointer button stays down.
const HOLD: Duration = Duration::from_millis(80);
/// Bounds Git discovery for an open menu.
const DISCOVERY: Duration = Duration::from_secs(15);
/// Bounds one Worktree operation, from the click through Git to the committed Workspace.
const OPERATION: Duration = Duration::from_secs(60);
const LINKED_BRANCH: &str = "feature-linked";
const OPEN_WORKTREE: &str = "Open Worktree ▸";

#[test]
fn worktree_rows_of_an_ungrouped_workspace_group_it_with_every_worktree_they_add() {
    let fixture = Fixture::new("orchbee-worktree-menu-", &["repo"]);
    repository(&fixture);
    let source = fixture.workspace_in("source", "repo")["workspaceId"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(record(&fixture, &source)["projectId"].is_null());
    let tui = fixture.start_tui(COLUMNS, ROWS);
    fixture.settle_ready(&tui.client);
    let mut control = Control::hold(&fixture, &tui.terminal, COLUMNS, ROWS);
    // The host terminal reports focus; the TUI starts without one.
    control.input(b"\x1b[I");

    let mut known = workspace_ids(&fixture);
    open_menu(&fixture, &tui, &mut control, "source");
    choose(&fixture, &tui, &mut control, OPEN_WORKTREE, |screen| {
        screen.contains(LINKED_BRANCH)
    });
    choose(&fixture, &tui, &mut control, LINKED_BRANCH, menu_closed);
    let opened = added(&fixture, &tui, &known);
    let project = record(&fixture, &source)["projectId"].clone();
    assert!(
        project.is_string(),
        "the Workspace stayed ungrouped: {}",
        snapshot(&fixture)
    );
    assert_eq!(record(&fixture, &opened)["projectId"], project);
    let projects = snapshot(&fixture)["navigation"]["projects"].clone();
    assert_eq!(
        projects
            .as_array()
            .unwrap()
            .iter()
            .find(|listed| listed["projectId"] == project)
            .map(|listed| listed["label"].clone()),
        Some(Value::from("repo")),
        "{projects}"
    );

    known.push(opened);
    fixture.settle_ready(&tui.client);
    open_menu(&fixture, &tui, &mut control, "source");
    choose(&fixture, &tui, &mut control, "New Worktree…", menu_closed);
    let created = added(&fixture, &tui, &known);
    assert_eq!(record(&fixture, &created)["projectId"], project);
    assert_eq!(record(&fixture, &source)["projectId"], project);
    control.release();
}

/// A repository in the fixture's `repo` with one commit and a linked checkout beside it.
fn repository(fixture: &Fixture) {
    let git = |words: &[&str]| {
        let output = Command::new("git")
            .arg("-C")
            .arg(fixture.path("repo"))
            .args(words)
            .env("HOME", fixture.path("home"))
            .env("USERPROFILE", fixture.path("home"))
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {words:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    git(&["init", "--quiet", "--initial-branch=main"]);
    git(&["config", "user.name", "Orchbee Fixture"]);
    git(&["config", "user.email", "fixture@example.invalid"]);
    git(&["config", "commit.gpgsign", "false"]);
    std::fs::write(fixture.path("repo/seed.txt"), b"seed\n").unwrap();
    git(&["add", "seed.txt"]);
    git(&["commit", "--quiet", "-m", "seed"]);
    let linked = fixture.path("linked-checkout");
    git(&["worktree", "add", "--quiet", "-b", LINKED_BRANCH, &linked]);
}

/// Right-clicks the sidebar row of `label`.
fn open_menu(fixture: &Fixture, tui: &Tui, control: &mut Control, label: &str) {
    let row = sidebar_row(fixture, tui, label);
    control.input(format!("\x1b[<35;3;{row}M\x1b[<2;3;{row}M").as_bytes());
    thread::sleep(HOLD);
    control.input(format!("\x1b[<2;3;{row}m").as_bytes());
}

/// Clicks the menu row that shows `text` until `effect` shows. A row that waits for Git discovery
/// differs from an enabled one only in color, and a click on it does nothing, so the click repeats.
fn choose(
    fixture: &Fixture,
    tui: &Tui,
    control: &mut Control,
    text: &str,
    effect: impl Fn(&str) -> bool,
) {
    let deadline = Instant::now() + DISCOVERY;
    loop {
        let screen = fixture.screen(&tui.terminal);
        if effect(&screen) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "the menu row {text} never took effect:
{screen}"
        );
        if let Some((row, column)) = screen.lines().enumerate().find_map(|(index, line)| {
            let start = line.find(text)?;
            Some((index + 1, line[..start].chars().count() + 2))
        }) {
            control.input(format!("[<35;{column};{row}M[<0;{column};{row}M").as_bytes());
            thread::sleep(HOLD);
            control.input(format!("[<0;{column};{row}m").as_bytes());
        }
        thread::sleep(Duration::from_millis(500));
    }
}

/// Whether the Workspace menu has closed.
fn menu_closed(screen: &str) -> bool {
    !screen.contains(OPEN_WORKTREE)
}

/// Waits for the one Workspace the Worktree operation adds beside `known`.
fn added(fixture: &Fixture, tui: &Tui, known: &[String]) -> String {
    let deadline = Instant::now() + OPERATION;
    loop {
        if let Some(added) = workspace_ids(fixture)
            .into_iter()
            .find(|workspace| !known.contains(workspace))
        {
            return added;
        }
        assert!(
            Instant::now() < deadline,
            "the menu row never added a Workspace:\n{}",
            fixture.screen(&tui.terminal)
        );
        thread::sleep(Duration::from_millis(250));
    }
}

fn snapshot(fixture: &Fixture) -> Value {
    let read = fixture.run(&["api", "snapshot"]);
    assert_eq!(read.code, Some(0), "{read}");
    read.document["result"]["snapshot"].clone()
}

fn workspace_ids(fixture: &Fixture) -> Vec<String> {
    snapshot(fixture)["navigation"]["workspaces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|workspace| workspace["workspaceId"].as_str().unwrap().to_owned())
        .collect()
}

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

/// The one-based screen row of `label` in the sidebar, which may show a hovered row's affordance
/// after the label.
fn sidebar_row(fixture: &Fixture, tui: &Tui, label: &str) -> usize {
    let screen = fixture.screen(&tui.terminal);
    screen
        .lines()
        .position(|line| {
            line.split('│')
                .next()
                .and_then(|cell| cell.trim().strip_prefix(label))
                .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
        })
        .map(|index| index + 1)
        .unwrap_or_else(|| panic!("no sidebar row for {label}:\n{screen}"))
}
