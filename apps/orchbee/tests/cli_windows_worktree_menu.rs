//! The Worktree rows of an ungrouped Workspace's sidebar menu: the CLI, Server, Worker and Zig Host
//! under test, a real Git repository with one linked checkout, and a TUI Client running in a
//! terminal of a second Session, driven by pointer. Opening the linked checkout from Open Worktree
//! creates the Project that holds the Workspace and the opened one, and a Worktree created from
//! the form New Worktree… opens joins it. The same holds wherever the first terminal stands, by process cwd or by a
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
const NEW_WORKTREE: &str = "New Worktree…";
const OPEN_WORKTREE: &str = "Open Worktree ▸";

#[test]
fn a_plain_workspace_first_menu_hides_git_actions_and_moves_between_new_projects() {
    let fixture = Fixture::new("orchbee-workspace-project-menu-", &["plain"]);
    let created = fixture.workspace_in("plain-source", "plain");
    let workspace = created["workspaceId"].as_str().unwrap();
    let contexts = snapshot(&fixture)["terminalContexts"].clone();
    let tui = fixture.start_tui(COLUMNS, ROWS);
    fixture.settle_ready(&tui.client);
    let mut control = Control::hold(&fixture, &tui.terminal, COLUMNS, ROWS);
    control.input(b"\x1b[I");
    let mut previous = Value::Null;
    for label in ["first-project", "second-project"] {
        open_menu(&fixture, &tui, &mut control, "plain-source");
        no_worktree_rows(&fixture, &tui);
        choose(&fixture, &tui, &mut control, "Move to Project", |screen| {
            screen.contains("New Project…")
        });
        choose(&fixture, &tui, &mut control, "New Project…", |screen| {
            !screen.contains("Move to Project") && screen.contains("New Project")
        });
        control.input(format!("{label}\r").as_bytes());
        let deadline = Instant::now() + DISCOVERY;
        loop {
            let project = record(&fixture, workspace)["projectId"].clone();
            if project.is_string() && project != previous {
                previous = project;
                break;
            }
            assert!(
                Instant::now() < deadline,
                "New Project did not move Workspace:\n{}",
                fixture.screen(&tui.terminal)
            );
            thread::sleep(Duration::from_millis(100));
        }
        fixture.settle_ready(&tui.client);
        assert_eq!(
            snapshot(&fixture)["terminalContexts"],
            contexts,
            "grouping preserves every terminal"
        );
    }
    open_menu(&fixture, &tui, &mut control, "plain-source");
    choose(&fixture, &tui, &mut control, "Move to Project", |screen| {
        screen.contains("New Project…")
    });
    choose(&fixture, &tui, &mut control, "New Project…", |screen| {
        !screen.contains("Move to Project") && screen.contains("New Project")
    });
    control.input(b"\x1b");
    fixture.settle_ready(&tui.client);
    assert_eq!(
        record(&fixture, workspace)["projectId"],
        previous,
        "cancel preserves membership"
    );
    fixture.workspace_in("fresh-plain", "plain");
    let deadline = Instant::now() + DISCOVERY;
    while !fixture.screen(&tui.terminal).contains("fresh-plain") {
        assert!(
            Instant::now() < deadline,
            "new non-Git Workspace never reached the sidebar"
        );
        thread::sleep(Duration::from_millis(100));
    }
    open_menu(&fixture, &tui, &mut control, "fresh-plain");
    no_worktree_rows(&fixture, &tui);
    control.release();
}

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
        menu_row(screen, LINKED_BRANCH).is_some()
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
    // Known Git facts enable New Worktree… while Open Worktree ▸ still checks. Waiting for the
    // answer keeps the click on the settled menu: an answer that recomposes the rows voids a click
    // spanning it, while one that only enables Open Worktree ▸ does not.
    enabled_row(&fixture, &tui, OPEN_WORKTREE);
    choose(&fixture, &tui, &mut control, NEW_WORKTREE, form_ready);
    submit_form(&fixture, &tui, &mut control);
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
        menu_row(screen, LINKED_BRANCH).is_some()
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
    no_worktree_rows(&fixture, &tui);
    let command = format!("cd \"{}\"", fixture.path("repo"));
    enter(&fixture, &terminal_of(&fixture, &created), &command);
    stands_in(&fixture, source, "repo");
    let deadline = Instant::now() + DISCOVERY;
    loop {
        let read = MenuRead::first(&fixture, &tui);
        if read.row(NEW_WORKTREE).is_some()
            && read.row(OPEN_WORKTREE).is_some()
            && read.enabled(NEW_WORKTREE)
            && read.enabled(OPEN_WORKTREE)
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the rows never enabled:\n{}",
            read.text()
        );
        thread::sleep(Duration::from_millis(100));
    }
    choose(&fixture, &tui, &mut control, OPEN_WORKTREE, |screen| {
        menu_row(screen, LINKED_BRANCH).is_some()
    });
    control.release();
}

/// Open Worktree ▸ depends on the checkout list, so a menu starts it from the last list Git gave for
/// the same checkout: a repository with one checkout never offers it, and one with a linked checkout
/// offers it from the first paint of every menu after its first.
#[test]
fn open_worktree_starts_from_the_last_list_for_the_same_checkout() {
    let fixture = Fixture::new("orchbee-worktree-list-", &["repo", "solo"]);
    repository(&fixture);
    only_checkout(&fixture, "solo");
    fixture.workspace_in("alone", "solo");
    fixture.workspace_in("pair", "repo");
    let tui = fixture.start_tui(COLUMNS, ROWS);
    fixture.settle_ready(&tui.client);
    let mut control = Control::hold(&fixture, &tui.terminal, COLUMNS, ROWS);
    control.input(b"\x1b[I");
    // Both rows show their branch, so New Worktree… starts enabled from the sidebar.
    let deadline = Instant::now() + DISCOVERY;
    while fixture.screen(&tui.terminal).matches("main").count() < 2 {
        assert!(
            Instant::now() < deadline,
            "the rows never showed their branch"
        );
        thread::sleep(Duration::from_millis(100));
    }

    for _ in 0..2 {
        open_menu(&fixture, &tui, &mut control, "alone");
        // Spans Git's answer.
        let watch = Instant::now() + Duration::from_secs(3);
        while Instant::now() < watch {
            let read = MenuRead::first(&fixture, &tui);
            assert!(!read.enabled(OPEN_WORKTREE), "{}", read.text());
        }
        close_menu(&fixture, &tui, &mut control);
    }

    open_menu(&fixture, &tui, &mut control, "pair");
    worktrees_enable(&fixture, &tui);
    close_menu(&fixture, &tui, &mut control);
    open_menu(&fixture, &tui, &mut control, "pair");
    let watch = Instant::now() + Duration::from_secs(2);
    while Instant::now() < watch {
        let read = MenuRead::first(&fixture, &tui);
        assert!(read.enabled(OPEN_WORKTREE), "{}", read.text());
    }
    control.release();
}

/// A Workspace that left its repository for a folder outside Git shows nothing Git said about the
/// repository: its first open menu removes Git-only actions after discovery, as do later menus.
#[test]
fn a_workspace_that_left_its_repository_offers_no_worktree_rows() {
    let fixture = Fixture::new("orchbee-worktree-left-", &["repo", "plain"]);
    repository(&fixture);
    let created = fixture.workspace_in("source", "repo");
    let source = created["workspaceId"].as_str().unwrap();
    let tui = fixture.start_tui(COLUMNS, ROWS);
    fixture.settle_ready(&tui.client);
    let mut control = Control::hold(&fixture, &tui.terminal, COLUMNS, ROWS);
    control.input(b"\x1b[I");
    shows_branch(&fixture, &tui, true);

    // The menu used there.
    open_menu(&fixture, &tui, &mut control, "source");
    worktrees_enable(&fixture, &tui);
    close_menu(&fixture, &tui, &mut control);

    let command = format!("cd \"{}\"", fixture.path("plain"));
    enter(&fixture, &terminal_of(&fixture, &created), &command);
    stands_in(&fixture, source, "plain");
    shows_branch(&fixture, &tui, false);
    let workspaces = workspace_ids(&fixture);
    let checkouts = checkouts(&fixture);

    open_menu(&fixture, &tui, &mut control, "source");
    no_worktree_rows(&fixture, &tui);
    assert_eq!(workspace_ids(&fixture), workspaces);
    assert_eq!(self::checkouts(&fixture), checkouts);
    close_menu(&fixture, &tui, &mut control);
    open_menu(&fixture, &tui, &mut control, "source");
    no_worktree_rows(&fixture, &tui);
    control.release();
}

/// A menu opened again on a repository Git answered keeps its Git actions enabled from
/// its first paint, through Git's answer, and through a `cd` inside the repository with the Git read
/// it starts.
#[test]
fn a_menu_on_a_known_repository_keeps_its_actions_enabled_through_a_directory_change() {
    let fixture = Fixture::new("orchbee-worktree-refresh-", &["repo"]);
    repository(&fixture);
    std::fs::create_dir_all(fixture.path("repo/sub")).unwrap();
    let created = fixture.workspace_in("source", "repo");
    let source = created["workspaceId"].as_str().unwrap();
    let tui = fixture.start_tui(COLUMNS, ROWS);
    fixture.settle_ready(&tui.client);
    let mut control = Control::hold(&fixture, &tui.terminal, COLUMNS, ROWS);
    control.input(b"\x1b[I");
    shows_branch(&fixture, &tui, true);
    open_menu(&fixture, &tui, &mut control, "source");
    worktrees_enable(&fixture, &tui);
    close_menu(&fixture, &tui, &mut control);

    open_menu(&fixture, &tui, &mut control, "source");
    menu_rows_are_enabled(&fixture, &tui);
    // Spans Git's answer.
    let watch = Instant::now() + Duration::from_secs(3);
    while Instant::now() < watch {
        menu_rows_are_enabled(&fixture, &tui);
    }
    let command = format!("cd \"{}\"", fixture.path("repo/sub"));
    enter(&fixture, &terminal_of(&fixture, &created), &command);
    // Spans the commit of the new directory and the Git read after it.
    let watch = Instant::now() + Duration::from_secs(3);
    while Instant::now() < watch {
        menu_rows_are_enabled(&fixture, &tui);
    }
    stands_in(&fixture, source, "repo/sub");
    menu_rows_are_enabled(&fixture, &tui);
    choose(&fixture, &tui, &mut control, OPEN_WORKTREE, |screen| {
        menu_row(screen, LINKED_BRANCH).is_some()
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
        menu_row(screen, LINKED_BRANCH).is_some()
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

    // The same branch label behind the menu must not satisfy the submenu selector.
    let deadline = Instant::now() + DISCOVERY;
    while !fixture.screen(&tui.terminal).contains(LINKED_BRANCH) {
        assert!(
            Instant::now() < deadline,
            "the holder branch never appeared"
        );
        thread::sleep(Duration::from_millis(100));
    }
    assert!(
        menu_row(&fixture.screen(&tui.terminal), LINKED_BRANCH).is_none(),
        "a branch behind the menu is not a submenu row"
    );
    let known = workspace_ids(&fixture);
    open_menu(&fixture, &tui, &mut control, "source");
    choose(&fixture, &tui, &mut control, OPEN_WORKTREE, |screen| {
        menu_row(screen, LINKED_BRANCH).is_some()
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
        menu_row(screen, LINKED_BRANCH).is_some()
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
        menu_row(screen, LINKED_BRANCH).is_some()
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
    only_checkout(fixture, "repo");
    let linked = fixture.path("linked-checkout");
    git_in(
        fixture,
        "repo",
        &["worktree", "add", "--quiet", "-b", LINKED_BRANCH, &linked],
    );
}

/// A repository in the fixture's `name` with one commit on `main` and no other checkout.
fn only_checkout(fixture: &Fixture, name: &str) {
    let git = |words: &[&str]| git_in(fixture, name, words);
    git(&["init", "--quiet", "--initial-branch=main"]);
    git(&["config", "user.name", "Orchbee Fixture"]);
    git(&["config", "user.email", "fixture@example.invalid"]);
    git(&["config", "commit.gpgsign", "false"]);
    std::fs::write(fixture.path(&format!("{name}/seed.txt")), b"seed\n").unwrap();
    git(&["add", "seed.txt"]);
    git(&["commit", "--quiet", "-m", "seed"]);
}

fn git_in(fixture: &Fixture, name: &str, words: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(fixture.path(name))
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
}

/// Waits for the open menu's New Worktree… and Open Worktree ▸ to enable.
fn worktrees_enable(fixture: &Fixture, tui: &Tui) {
    let deadline = Instant::now() + DISCOVERY;
    loop {
        let read = MenuRead::first(fixture, tui);
        if read.enabled(NEW_WORKTREE) && read.enabled(OPEN_WORKTREE) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "the rows never enabled:\n{}",
            read.text()
        );
        thread::sleep(Duration::from_millis(100));
    }
}

/// Closes the open menu.
fn close_menu(fixture: &Fixture, tui: &Tui, control: &mut Control) {
    control.input(b"\x1b");
    let deadline = Instant::now() + DISCOVERY;
    while !menu_closed(&fixture.screen(&tui.terminal)) {
        assert!(Instant::now() < deadline, "the menu never closed");
        thread::sleep(Duration::from_millis(100));
    }
}

/// The checkouts Git lists for the repository.
fn checkouts(fixture: &Fixture) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(fixture.path("repo"))
        .args(["worktree", "list", "--porcelain"])
        .env("HOME", fixture.path("home"))
        .env("USERPROFILE", fixture.path("home"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap()
}

/// Waits for the sidebar to show, or no longer show, the repository's branch.
fn shows_branch(fixture: &Fixture, tui: &Tui, expected: bool) {
    let deadline = Instant::now() + DISCOVERY;
    loop {
        let screen = fixture.screen(&tui.terminal);
        if screen.contains("main") == expected {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "the row's branch never settled:\n{screen}"
        );
        thread::sleep(Duration::from_millis(100));
    }
}

/// The first observed menu paint and subsequent discovery paints must all omit Git-only actions.
fn no_worktree_rows(fixture: &Fixture, tui: &Tui) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let read = MenuRead::first(fixture, tui);
        assert!(
            [NEW_WORKTREE, OPEN_WORKTREE]
                .iter()
                .all(|text| read.row(text).is_none()),
            "non-Git menu advertised Git actions before discovery settled:\n{}",
            read.text()
        );
        if Instant::now() >= deadline {
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

/// Right-clicks the sidebar row of `label`.
fn open_menu(fixture: &Fixture, tui: &Tui, control: &mut Control, label: &str) {
    let row = sidebar_row(fixture, tui, label);
    control.input(format!("\x1b[<35;3;{row}M\x1b[<2;3;{row}M").as_bytes());
    thread::sleep(HOLD);
    control.input(format!("\x1b[<2;3;{row}m").as_bytes());
}

/// Waits for the menu row that shows `text` to color as enabled and returns its position.
fn enabled_row(fixture: &Fixture, tui: &Tui, text: &str) -> (usize, usize) {
    let deadline = Instant::now() + DISCOVERY;
    loop {
        let read = MenuRead::take(fixture, tui);
        if let Some(position) = read.enabled_row(text) {
            return position;
        }
        assert!(
            Instant::now() < deadline,
            "the menu row {text} never enabled:\n{}",
            read.text()
        );
        thread::sleep(Duration::from_millis(100));
    }
}

/// Clicks the menu row that shows `text` once it colors as enabled, then waits for `effect`. A row
/// that waits for Git discovery differs from an enabled one only in color, and a click on it does
/// nothing. One click: on a busy machine a click lands late, and a second one could act on
/// whatever the first opened.
fn choose(
    fixture: &Fixture,
    tui: &Tui,
    control: &mut Control,
    text: &str,
    effect: impl Fn(&str) -> bool,
) {
    let deadline = Instant::now() + DISCOVERY;
    let (row, column) = enabled_row(fixture, tui, text);
    control.input(format!("\x1b[<35;{column};{row}M\x1b[<0;{column};{row}M").as_bytes());
    thread::sleep(HOLD);
    control.input(format!("\x1b[<0;{column};{row}m").as_bytes());
    loop {
        let screen = fixture.screen(&tui.terminal);
        if effect(&screen) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "the menu row {text} never took effect:\n{screen}"
        );
        thread::sleep(Duration::from_millis(100));
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

/// One read of the open Workspace menu, with its colors.
struct MenuRead(String);

impl MenuRead {
    /// Waits for the menu's first paint.
    fn first(fixture: &Fixture, tui: &Tui) -> Self {
        let deadline = Instant::now() + DISCOVERY;
        loop {
            let read = Self::take(fixture, tui);
            if read.row("Move to Project").is_some() {
                return read;
            }
            assert!(Instant::now() < deadline, "no menu:\n{}", read.text());
            thread::sleep(Duration::from_millis(50));
        }
    }

    fn take(fixture: &Fixture, tui: &Tui) -> Self {
        let read = fixture.run(&[
            "terminal",
            "read",
            "--session",
            "host",
            &tui.terminal,
            "--source",
            "visible",
            "--ansi",
        ]);
        Self(
            read.document["result"]["content"]["text"]
                .as_str()
                .unwrap_or_else(|| panic!("{read}"))
                .to_owned(),
        )
    }

    fn row(&self, text: &str) -> Option<&str> {
        self.0.lines().find(|line| plain(line).contains(text))
    }

    fn text(&self) -> String {
        plain(&self.0)
    }

    /// Whether the row of `text` colors as Rename, which is always enabled here.
    fn enabled(&self, text: &str) -> bool {
        let style = |text: &str| {
            let row = self
                .row(text)
                .unwrap_or_else(|| panic!("no row {text}:\n{}", self.text()));
            style_of(row, text)
        };
        style(text) == style("Rename")
    }

    /// The position `menu_row` gives for `text`, once that row colors as Rename.
    fn enabled_row(&self, text: &str) -> Option<(usize, usize)> {
        let (row, column) = menu_row(&self.text(), text)?;
        let line = self.0.lines().nth(row - 1)?;
        let rename = self.row("Rename")?;
        (style_of(line, text) == style_of(rename, "Rename")).then_some((row, column))
    }
}

/// Known repository actions remain usable while a current-directory refresh runs.
fn menu_rows_are_enabled(fixture: &Fixture, tui: &Tui) {
    let read = MenuRead::first(fixture, tui);
    for text in ["Move to Project", NEW_WORKTREE, OPEN_WORKTREE] {
        assert!(
            read.enabled(text),
            "{text} is not enabled:\n{}",
            read.text()
        );
    }
}

/// `line` without its escape sequences.
fn plain(line: &str) -> String {
    let mut text = String::new();
    let mut characters = line.chars();
    while let Some(character) = characters.next() {
        if character == '\x1b' {
            characters.by_ref().find(char::is_ascii_alphabetic);
        } else {
            text.push(character);
        }
    }
    text
}

/// The color sequences in force where `text` starts in `line`, from the last reset on.
fn style_of(line: &str, text: &str) -> String {
    let start = plain(line).find(text).expect("the row is on this line");
    let mut style = String::new();
    let mut offset = 0;
    let mut characters = line.chars();
    while let Some(character) = characters.next() {
        if character != '\x1b' {
            if offset == start {
                break;
            }
            offset += character.len_utf8();
            continue;
        }
        let mut sequence = String::from(character);
        for next in characters.by_ref() {
            sequence.push(next);
            if next.is_ascii_alphabetic() {
                break;
            }
        }
        if sequence == "\x1b[0m" || sequence == "\x1b[m" {
            style.clear();
        } else {
            style.push_str(&sequence);
        }
    }
    style
}

/// The one-based row and a column inside a framed menu row that shows `text`. Sidebar branch
/// labels behind a menu are not evidence of a submenu and must never receive its clicks.
fn menu_row(screen: &str, text: &str) -> Option<(usize, usize)> {
    screen.lines().enumerate().find_map(|(index, line)| {
        let start = line.find(text)?;
        if !line[..start].contains('│') || !line[start + text.len()..].contains('│') {
            return None;
        }
        Some((index + 1, line[..start].chars().count() + 2))
    })
}

/// Whether the Workspace menu has closed.
fn menu_closed(screen: &str) -> bool {
    !screen.contains(OPEN_WORKTREE)
}

/// Whether the New Worktree form shows its Branch and Base fields, which follow source discovery.
fn form_ready(screen: &str) -> bool {
    menu_closed(screen)
        && menu_row(screen, "Branch").is_some()
        && menu_row(screen, "Base").is_some()
}

/// Presses Enter on the New Worktree form, with its defaults, until it leaves its fields for the
/// running operation; one press creates once.
fn submit_form(fixture: &Fixture, tui: &Tui, control: &mut Control) {
    let deadline = Instant::now() + DISCOVERY;
    loop {
        let screen = fixture.screen(&tui.terminal);
        if !form_ready(&screen) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "the New Worktree form never took Enter:\n{screen}"
        );
        control.input(b"\r");
        thread::sleep(Duration::from_millis(500));
    }
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
    // Client readiness does not establish that PowerShell finished initializing its input editor.
    let deadline = Instant::now() + DISCOVERY;
    loop {
        let read = fixture.run(&["terminal", "read", terminal, "--source", "visible"]);
        if read.code == Some(0)
            && read.document["result"]["content"]["text"]
                .as_str()
                .and_then(|text| text.lines().rfind(|line| !line.trim().is_empty()))
                .is_some_and(|line| line.trim_end().ends_with('>'))
        {
            break;
        }
        assert!(Instant::now() < deadline, "shell prompt not ready: {read}");
        thread::sleep(Duration::from_millis(100));
    }
    // Text and Enter belong to one controlling attachment, as one user command does.
    let sent = fixture.run(&["terminal", "send", terminal, "--text", command, "--enter"]);
    assert_eq!(sent.code, Some(0), "{sent}");
}

/// Waits for the Server to follow the Workspace's first terminal into `directory`.
fn stands_in(fixture: &Fixture, workspace: &str, directory: &str) {
    let expected = fixture.path(directory);
    let followed = |value: &Value| {
        value
            .as_str()
            .is_some_and(|path| fixture.is_directory(path, directory))
    };
    let deadline = Instant::now() + DISCOVERY;
    while !followed(&record(fixture, workspace)["terminalDirectory"]) {
        if Instant::now() >= deadline {
            let context = record(fixture, workspace)["terminalContextId"].clone();
            let listed = fixture.run(&["terminal", "list"]);
            let terminal = listed.document["result"]["terminals"]
                .as_array()
                .and_then(|terminals| {
                    terminals
                        .iter()
                        .find(|terminal| terminal["boundTo"]["terminalContextId"] == context)
                })
                .and_then(|terminal| terminal["terminalId"].as_str());
            let screen = terminal.map_or_else(
                || listed.to_string(),
                |terminal| {
                    fixture
                        .run(&["terminal", "read", terminal, "--source", "visible"])
                        .to_string()
                },
            );
            panic!(
                "the first terminal never reached {expected}: {}\nterminal: {screen}",
                snapshot(fixture)
            );
        }
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
