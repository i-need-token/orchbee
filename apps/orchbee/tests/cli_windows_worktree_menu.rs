//! The Worktree rows of an ungrouped Workspace's sidebar menu: the CLI, Server, Worker and Zig Host
//! under test, a real Git repository with one linked checkout, and a TUI Client running in a
//! terminal of a second Session, driven by pointer. Opening the linked checkout from Open Worktree
//! creates the Project that holds the Workspace and the opened one, and a Worktree created from
//! New Worktree… joins it. The same holds wherever the first terminal stands, by process cwd or by a
//! directory its shell only reports, and for a checkout another Workspace already holds. A checkout
//! belongs to the Workspace the sidebar shows there, not to the one created there.
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

#[test]
fn a_first_terminal_in_a_subdirectory_groups_its_workspace_with_the_worktree() {
    let fixture = Fixture::new("orchbee-worktree-subdirectory-", &["repo"]);
    repository(&fixture);
    std::fs::create_dir_all(fixture.path("repo/sub")).unwrap();
    let created = fixture.workspace_in("source", "repo");
    let source = created["workspaceId"].as_str().unwrap();
    let tui = fixture.start_tui(COLUMNS, ROWS);
    fixture.settle_ready(&tui.client);
    let mut control = Control::hold(&fixture, &tui.terminal, COLUMNS, ROWS);
    control.input(b"\x1b[I");
    let command = format!("cd \"{}\"", fixture.path("repo/sub"));
    enter(&fixture, &terminal_of(&fixture, &created), &command);
    stands_in(&fixture, source, "repo/sub");

    let known = workspace_ids(&fixture);
    open_menu(&fixture, &tui, &mut control, "source");
    // One movement report, as a terminal sends for a real pointer. The row may still wait for Git.
    rest_on(&fixture, &tui, &mut control, OPEN_WORKTREE, |screen| {
        screen.contains(LINKED_BRANCH)
    });
    choose(&fixture, &tui, &mut control, LINKED_BRANCH, menu_closed);
    let opened = added(&fixture, &tui, &known);
    assert_same_new_project(&fixture, source, &opened);
    control.release();
}

/// A menu opened outside any repository stays open when the first terminal enters one, and its rows
/// follow.
#[test]
fn a_directory_change_under_the_open_menu_keeps_it_and_asks_git_again() {
    let fixture = Fixture::new("orchbee-worktree-moved-", &["repo", "launch"]);
    repository(&fixture);
    let created = fixture.workspace_in("source", "launch");
    let source = created["workspaceId"].as_str().unwrap();
    let tui = fixture.start_tui(COLUMNS, ROWS);
    fixture.settle_ready(&tui.client);
    let mut control = Control::hold(&fixture, &tui.terminal, COLUMNS, ROWS);
    control.input(b"\x1b[I");

    open_menu(&fixture, &tui, &mut control, "source");
    let deadline = Instant::now() + DISCOVERY;
    while !fixture.screen(&tui.terminal).contains(OPEN_WORKTREE) {
        assert!(Instant::now() < deadline, "the Workspace menu never opened");
        thread::sleep(Duration::from_millis(100));
    }
    let command = format!("cd \"{}\"", fixture.path("repo"));
    enter(&fixture, &terminal_of(&fixture, &created), &command);
    stands_in(&fixture, source, "repo");
    choose(&fixture, &tui, &mut control, OPEN_WORKTREE, |screen| {
        screen.contains(LINKED_BRANCH)
    });
    control.release();
}

/// A `cd` inside the repository, with the menu open, leaves the rows the menu already recognized
/// enabled while Git is asked again.
#[test]
fn a_directory_change_under_the_open_menu_leaves_its_worktree_rows_enabled() {
    let fixture = Fixture::new("orchbee-worktree-refresh-", &["repo"]);
    repository(&fixture);
    std::fs::create_dir_all(fixture.path("repo/sub")).unwrap();
    let created = fixture.workspace_in("source", "repo");
    let source = created["workspaceId"].as_str().unwrap();
    let tui = fixture.start_tui(COLUMNS, ROWS);
    fixture.settle_ready(&tui.client);
    let mut control = Control::hold(&fixture, &tui.terminal, COLUMNS, ROWS);
    control.input(b"\x1b[I");

    open_menu(&fixture, &tui, &mut control, "source");
    let deadline = Instant::now() + DISCOVERY;
    loop {
        let screen = fixture.screen(&tui.terminal);
        if every_row_enabled(&screen) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the rows never enabled:\n{screen}"
        );
        thread::sleep(Duration::from_millis(100));
    }
    let command = format!("cd \"{}\"", fixture.path("repo/sub"));
    enter(&fixture, &terminal_of(&fixture, &created), &command);
    // Spans the commit of the new directory and the Git read after it.
    let watch = Instant::now() + Duration::from_secs(2);
    while Instant::now() < watch {
        let screen = fixture.screen(&tui.terminal);
        assert!(
            every_row_enabled(&screen),
            "the rows left their enabled state:\n{screen}"
        );
    }
    stands_in(&fixture, source, "repo/sub");
    choose(&fixture, &tui, &mut control, OPEN_WORKTREE, |screen| {
        screen.contains(LINKED_BRANCH)
    });
    control.release();
}

/// PowerShell reports its directory and leaves its process cwd in the launch directory, here outside
/// any repository, while the sidebar line follows the report.
#[test]
fn a_directory_the_shell_only_reports_enables_the_rows_and_groups_the_workspace() {
    let fixture = Fixture::new("orchbee-worktree-reported-", &["repo", "launch"]);
    repository(&fixture);
    let report = fixture.path("report.txt");
    let sequence = format!("\x1b]9;9;{}\x1b\\", fixture.path("repo"));
    std::fs::write(&report, sequence).unwrap();
    let created = fixture.workspace_in("source", "launch");
    let source = created["workspaceId"].as_str().unwrap();
    let tui = fixture.start_tui(COLUMNS, ROWS);
    fixture.settle_ready(&tui.client);
    let mut control = Control::hold(&fixture, &tui.terminal, COLUMNS, ROWS);
    control.input(b"\x1b[I");
    let command = format!("type \"{report}\"");
    enter(&fixture, &terminal_of(&fixture, &created), &command);
    stands_in(&fixture, source, "repo");

    let known = workspace_ids(&fixture);
    open_menu(&fixture, &tui, &mut control, "source");
    choose(&fixture, &tui, &mut control, OPEN_WORKTREE, |screen| {
        screen.contains(LINKED_BRANCH)
    });
    choose(&fixture, &tui, &mut control, LINKED_BRANCH, menu_closed);
    let opened = added(&fixture, &tui, &known);
    assert_same_new_project(&fixture, source, &opened);
    control.release();
}

#[test]
fn opening_a_checkout_another_workspace_holds_groups_that_workspace_too() {
    let fixture = Fixture::new("orchbee-worktree-reuse-", &["repo"]);
    repository(&fixture);
    let source = fixture.workspace_in("source", "repo")["workspaceId"]
        .as_str()
        .unwrap()
        .to_owned();
    let holder = fixture.workspace_in("holder", "linked-checkout")["workspaceId"]
        .as_str()
        .unwrap()
        .to_owned();
    let tui = fixture.start_tui(COLUMNS, ROWS);
    fixture.settle_ready(&tui.client);
    let mut control = Control::hold(&fixture, &tui.terminal, COLUMNS, ROWS);
    control.input(b"\x1b[I");

    let known = workspace_ids(&fixture);
    open_menu(&fixture, &tui, &mut control, "source");
    choose(&fixture, &tui, &mut control, OPEN_WORKTREE, |screen| {
        screen.contains(LINKED_BRANCH)
    });
    choose(&fixture, &tui, &mut control, LINKED_BRANCH, menu_closed);
    let deadline = Instant::now() + OPERATION;
    while !record(&fixture, &holder)["projectId"].is_string() {
        assert!(
            Instant::now() < deadline,
            "the Workspace holding the checkout stayed ungrouped: {}",
            snapshot(&fixture)
        );
        thread::sleep(Duration::from_millis(250));
    }
    assert_same_new_project(&fixture, &source, &holder);
    assert_eq!(
        workspace_ids(&fixture),
        known,
        "the checkout's Workspace is reused"
    );
    control.release();
}

/// `arrived` was created elsewhere and stands in the checkout; `gone` was created in the checkout and
/// stands in a folder outside Git. The sidebar shows the first at the checkout.
#[test]
fn opening_a_checkout_reuses_the_workspace_shown_there_not_the_one_created_there() {
    let fixture = Fixture::new("orchbee-worktree-shown-", &["repo", "plain", "elsewhere"]);
    repository(&fixture);
    let source = fixture.workspace_in("source", "repo")["workspaceId"]
        .as_str()
        .unwrap()
        .to_owned();
    let gone = move_workspace(&fixture, "gone", "linked-checkout", "plain");
    let arrived = move_workspace(&fixture, "arrived", "elsewhere", "linked-checkout");
    let tui = fixture.start_tui(COLUMNS, ROWS);
    fixture.settle_ready(&tui.client);
    let mut control = Control::hold(&fixture, &tui.terminal, COLUMNS, ROWS);
    control.input(b"[I");

    let known = workspace_ids(&fixture);
    open_menu(&fixture, &tui, &mut control, "source");
    choose(&fixture, &tui, &mut control, OPEN_WORKTREE, |screen| {
        screen.contains(LINKED_BRANCH)
    });
    choose(&fixture, &tui, &mut control, LINKED_BRANCH, menu_closed);
    grouped(&fixture, &source);
    assert_same_new_project(&fixture, &source, &arrived);
    assert!(
        record(&fixture, &gone)["projectId"].is_null(),
        "a Workspace that left the checkout was grouped: {}",
        snapshot(&fixture)
    );
    assert_eq!(workspace_ids(&fixture), known, "the Workspace is reused");
    // The Client enters the Workspace it reused.
    let context = record(&fixture, &arrived)["terminalContextId"].clone();
    let deadline = Instant::now() + OPERATION;
    while fixture.client(&tui.client)["view"]["selection"]["terminalContextId"] != context {
        assert!(
            Instant::now() < deadline,
            "the Client never entered the reused Workspace:
{}",
            fixture.screen(&tui.terminal)
        );
        thread::sleep(Duration::from_millis(250));
    }
    control.release();
}

/// Every Workspace created in the checkout has left it, so none is shown there.
#[test]
fn opening_a_checkout_every_workspace_left_starts_a_new_workspace_there() {
    let fixture = Fixture::new("orchbee-worktree-left-", &["repo", "plain"]);
    repository(&fixture);
    let source = fixture.workspace_in("source", "repo")["workspaceId"]
        .as_str()
        .unwrap()
        .to_owned();
    let gone = move_workspace(&fixture, "gone", "linked-checkout", "plain");
    let tui = fixture.start_tui(COLUMNS, ROWS);
    fixture.settle_ready(&tui.client);
    let mut control = Control::hold(&fixture, &tui.terminal, COLUMNS, ROWS);
    control.input(b"[I");

    let known = workspace_ids(&fixture);
    open_menu(&fixture, &tui, &mut control, "source");
    choose(&fixture, &tui, &mut control, OPEN_WORKTREE, |screen| {
        screen.contains(LINKED_BRANCH)
    });
    choose(&fixture, &tui, &mut control, LINKED_BRANCH, menu_closed);
    grouped(&fixture, &source);
    assert!(
        record(&fixture, &gone)["projectId"].is_null(),
        "a Workspace that left the checkout was grouped: {}",
        snapshot(&fixture)
    );
    let opened = added(&fixture, &tui, &known);
    assert_same_new_project(&fixture, &source, &opened);
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

/// Moves the pointer onto the menu row that shows `text` once, then only waits for `effect`.
fn rest_on(
    fixture: &Fixture,
    tui: &Tui,
    control: &mut Control,
    text: &str,
    effect: impl Fn(&str) -> bool,
) {
    let deadline = Instant::now() + DISCOVERY;
    let mut moved = false;
    loop {
        let screen = fixture.screen(&tui.terminal);
        if effect(&screen) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "resting on the menu row {text} opened nothing:\n{screen}"
        );
        if !moved && let Some((row, column)) = menu_row(&screen, text) {
            control.input(format!("\x1b[<35;{column};{row}M").as_bytes());
            moved = true;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

/// Whether the open Workspace menu ends with its last row, Close. A row that is unavailable differs
/// from an enabled one in color, and adds a reason row to the bottom of the frame.
fn every_row_enabled(screen: &str) -> bool {
    let lines: Vec<_> = screen.lines().collect();
    lines
        .iter()
        .position(|line| line.contains('└'))
        .is_some_and(|bottom| bottom > 0 && lines[bottom - 1].contains("Close"))
}

/// The one-based row and a column inside the first row that shows `text`.
fn menu_row(screen: &str, text: &str) -> Option<(usize, usize)> {
    screen.lines().enumerate().find_map(|(index, line)| {
        let start = line.find(text)?;
        Some((index + 1, line[..start].chars().count() + 2))
    })
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

/// Both Workspaces share one Project, which the repository named.
fn assert_same_new_project(fixture: &Fixture, source: &str, other: &str) {
    let project = record(fixture, source)["projectId"].clone();
    assert!(
        project.is_string(),
        "the Workspace stayed ungrouped: {}",
        snapshot(fixture)
    );
    assert_eq!(record(fixture, other)["projectId"], project);
    let projects = snapshot(fixture)["navigation"]["projects"].clone();
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
}

/// The terminal of the first Panel of a Workspace the fixture created.
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

/// Runs `command` in `terminal`'s shell.
fn enter(fixture: &Fixture, terminal: &str, command: &str) {
    for words in [
        ["terminal", "send", terminal, "--text", command],
        ["terminal", "send", terminal, "--key", "enter"],
    ] {
        let sent = fixture.run(&words);
        assert_eq!(sent.code, Some(0), "{sent}");
    }
}

/// Waits for the Server to follow the Workspace's first terminal into `directory`.
fn stands_in(fixture: &Fixture, workspace: &str, directory: &str) {
    let expected = fixture.path(directory);
    let followed = |value: &Value| {
        value.as_str().is_some_and(|path| {
            let spelled = |path: &str| path.replace('/', "\\").to_ascii_lowercase();
            spelled(path) == spelled(&expected)
        })
    };
    let deadline = Instant::now() + DISCOVERY;
    while !followed(&record(fixture, workspace)["terminalDirectory"]) {
        assert!(
            Instant::now() < deadline,
            "the first terminal never reached {expected}: {}",
            snapshot(fixture)
        );
        thread::sleep(Duration::from_millis(200));
    }
}

/// Opens a Workspace in `from` and walks its first terminal to `to`; returns the Workspace.
fn move_workspace(fixture: &Fixture, label: &str, from: &str, to: &str) -> String {
    let created = fixture.workspace_in(label, from);
    let workspace = created["workspaceId"].as_str().unwrap().to_owned();
    let command = format!("cd \"{}\"", fixture.path(to));
    enter(fixture, &terminal_of(fixture, &created), &command);
    stands_in(fixture, &workspace, to);
    workspace
}

/// Waits until the Worktree operation has put `workspace` in a Project.
fn grouped(fixture: &Fixture, workspace: &str) {
    let deadline = Instant::now() + OPERATION;
    while !record(fixture, workspace)["projectId"].is_string() {
        assert!(
            Instant::now() < deadline,
            "the Workspace stayed ungrouped: {}",
            snapshot(fixture)
        );
        thread::sleep(Duration::from_millis(250));
    }
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
