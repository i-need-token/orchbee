//! A key pressed right after a second sidebar click while the user's previous switch still lands:
//! the CLI, Server, Worker and Zig Host under test and a TUI Client running in a terminal of a
//! second Session, driven with a person's timing. `Ctrl+B D` pressed then either closes the
//! Workspace the last click asked for, after its confirmation, or is refused visibly; it never
//! closes the idle Workspace the earlier click is still landing on.
#![cfg(windows)]

#[allow(dead_code)]
#[path = "support/windows_tui_session.rs"]
mod session;

use serde_json::Value;
use session::{Control, Fixture, Tui};
use std::{
    thread,
    time::{Duration, Instant},
};

const COLUMNS: u16 = 112;
const ROWS: u16 = 32;
const ROUNDS: usize = 8;
/// Pauses between the two clicks, from the reported 700 ms down to a quick second choice. Which
/// of them meets the earlier switch's read or its terminal attach depends on the host's speed.
const BETWEEN_CLICKS: [Duration; 4] = [
    Duration::from_millis(700),
    Duration::from_millis(500),
    Duration::from_millis(300),
    Duration::from_millis(150),
];
/// How long a pointer button stays down.
const HOLD: Duration = Duration::from_millis(80);
/// Bounds how long the key's outcome may take to show.
const LANDING: Duration = Duration::from_secs(8);
const CONFIRMATION: &str = "Close Workspace?";
/// The refused key's notice, the notice of a click the held frame could not resolve, which takes
/// the status line first, and the busy refusal of a key whose hold outlived its bound.
const REFUSALS: [&str; 3] = ["Workspace changed", "input was not sent", "Client busy"];

#[test]
fn close_after_a_second_click_never_closes_the_earlier_destination() {
    let fixture = Fixture::new("orchbee-switch-keys-", &["work/alpha", "work/source"]);
    let alpha = fixture.workspace_in("alpha", "work/alpha");
    let mut source = fixture.workspace_in("source", "work/source");
    // Its running program makes the clicked Workspace ask before it closes, so each round can
    // cancel that close; the idle source would close at once.
    let clicked = fixture.background_output();
    let tui = fixture.start_tui(COLUMNS, ROWS);
    fixture.settle_ready(&tui.client);
    let mut control = Control::hold(&fixture, &tui.terminal, COLUMNS, ROWS);
    // The host terminal reports focus; the TUI starts without one.
    control.input(b"\x1b[I");

    let mut failures = Vec::new();
    for round in 0..ROUNDS {
        let gap = BETWEEN_CLICKS[round % BETWEEN_CLICKS.len()];
        focus(&fixture, &tui, &alpha);
        assert!(
            until(|| !refused(&fixture.screen(&tui.terminal))),
            "an earlier refusal stayed on screen"
        );
        let source_row = sidebar_row(&fixture, &tui, "source");
        let clicked_row = sidebar_row(&fixture, &tui, "noise");
        click(&mut control, source_row);
        thread::sleep(gap);
        click(&mut control, clicked_row);
        control.input(b"\x02D");

        let deadline = Instant::now() + LANDING;
        let outcome = loop {
            if !listed(&fixture, &source) {
                break "closed the idle Workspace the earlier click landed on".to_owned();
            }
            let screen = fixture.screen(&tui.terminal);
            if screen.contains(CONFIRMATION) {
                let shown = selection(&fixture, &tui);
                if shown == clicked["terminalContextId"] {
                    break String::new();
                }
                break format!("asked to close while the Client showed {shown}");
            }
            if refused(&screen) {
                // Nothing may close once the earlier switch lands either.
                fixture.settle_ready(&tui.client);
                thread::sleep(Duration::from_millis(500));
                break if listed(&fixture, &source) {
                    String::new()
                } else {
                    "closed the idle Workspace after a refusal".to_owned()
                };
            }
            if Instant::now() >= deadline {
                break format!(
                    "showed neither the clicked Workspace's close nor a refusal:\n{screen}"
                );
            }
            thread::sleep(Duration::from_millis(50));
        };
        let gap = gap.as_millis();
        eprintln!(
            "round {round} ({gap} ms): {}",
            if outcome.is_empty() { "ok" } else { &outcome }
        );
        if !outcome.is_empty() {
            failures.push(format!("round {round} ({gap} ms): {outcome}"));
        }
        // Cancel the clicked Workspace's confirmation; restore a closed source.
        if fixture.screen(&tui.terminal).contains(CONFIRMATION) {
            control.input(b"\x1b");
            assert!(
                until(|| !fixture.screen(&tui.terminal).contains(CONFIRMATION)),
                "the confirmation stayed open"
            );
        }
        assert!(listed(&fixture, &clicked), "the cancelled close closed it");
        if !listed(&fixture, &source) {
            source = fixture.workspace_in("source", "work/source");
        }
    }
    control.release();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A pointer click on the sidebar's first columns of `row`: the motion onto the cell and the
/// press arrive together, and the release follows after a hold.
fn click(control: &mut Control, row: usize) {
    control.input(format!("\x1b[<35;3;{row}M\x1b[<0;3;{row}M").as_bytes());
    thread::sleep(HOLD);
    control.input(format!("\x1b[<0;3;{row}m").as_bytes());
}

fn refused(screen: &str) -> bool {
    REFUSALS.iter().any(|notice| screen.contains(notice))
}

/// Makes `workspace` current through the CLI and waits until the Client settles there.
fn focus(fixture: &Fixture, tui: &Tui, workspace: &Value) {
    assert!(
        until(|| {
            selection(fixture, tui) == workspace["terminalContextId"]
                || fixture
                    .run(&[
                        "workspace",
                        "focus",
                        workspace["workspaceId"].as_str().unwrap(),
                        "--client",
                        &tui.client,
                    ])
                    .code
                    == Some(0)
        }),
        "the Client never showed {workspace}"
    );
    fixture.settle_ready(&tui.client);
}

/// Whether `workspace` is still open. A listing that meets a membership change, such as the close
/// under test, is asked again.
fn listed(fixture: &Fixture, workspace: &Value) -> bool {
    let deadline = Instant::now() + LANDING;
    loop {
        let listed = fixture.run(&["workspace", "list"]);
        if let Some(workspaces) = listed.document["result"]["workspaces"].as_array() {
            return workspaces
                .iter()
                .any(|listed| listed["workspaceId"] == workspace["workspaceId"]);
        }
        assert!(
            listed.document["failure"]["details"]["reason"] == "workbench-changed"
                && Instant::now() < deadline,
            "{listed}"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

fn selection(fixture: &Fixture, tui: &Tui) -> Value {
    fixture.client(&tui.client)["view"]["selection"]["terminalContextId"].clone()
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

/// Polls `observed` until it holds or the landing bound passes.
fn until(mut observed: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + LANDING;
    while Instant::now() < deadline {
        if observed() {
            return true;
        }
        thread::sleep(Duration::from_millis(50));
    }
    false
}
