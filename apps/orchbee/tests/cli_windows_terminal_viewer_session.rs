//! Terminal viewers through a real local Session: the CLI, Server, Worker and Zig Host under test,
//! a real TUI Client that runs in a terminal of a second Session, and a terminal that prints in
//! another Workspace.
//!
//! The Client drives a split Workspace with pointer input as a user would: a click whose pointer
//! moves before release focuses a pane and leaves a selection in it, which copy-on-select, turned
//! on in this Session's preferences, exports. A divider drag about 400 ms later resizes that pane once per column.
//! The Host reports the selection again at each new generation, faster than a busy Client polls,
//! and refuses the export's cancellation or later chunk acknowledgements once the export is over.
//! None of that may cost a pane its viewer: the TUI never shows a terminal as unavailable.
//!
//! The TUI's window also changes size, alternately narrow and wide, while both panes print lines
//! that soft-wrap. The Client redraws through a real ConPTY, which erases and rewrites soft-wrapped
//! rows, and each pane's terminal is resized with the window. Every frame of every terminal must
//! still decode for every viewer: the TUI keeps serving reads, the printing pane keeps advancing,
//! and each pane's terminal still serves a read.
#![cfg(windows)]

#[allow(dead_code)]
#[path = "support/windows_tui_session.rs"]
mod session;

#[path = "cli_windows_terminal_viewer_session/focus_control.rs"]
mod focus_control;

use orchbee_local_runtime::{UserPreferencesScope, UserPreferencesStore};
use orchbee_workbench_protocol::{
    PreferenceKey, UserPreferencesPatch, UserPreferencesRequest, UserPreferencesResponse,
};
use session::{Control, Fixture, OBSERVATION};
use std::{
    thread,
    time::{Duration, Instant},
};

/// The TUI's window, in cells.
const COLUMNS: u16 = 112;
const ROWS: u16 = 32;
/// The pause between the click that focuses a pane and the next gesture.
const FOCUS_TO_DRAG: Duration = Duration::from_millis(400);
/// Divider columns per drag; each column resizes both panes once.
const DRAG_COLUMNS: u16 = 6;
/// Focus-and-drag rounds, alternating the focused pane.
const ROUNDS: usize = 4;
/// How long the TUI is watched after each gesture.
const WATCH: Duration = Duration::from_millis(2_500);
/// The TUI's window sizes, in cells, alternately narrow and wide.
const WINDOW_SIZES: [(u16, u16); 10] = [
    (60, 24),
    (112, 32),
    (70, 28),
    (140, 40),
    (56, 20),
    (112, 32),
    (64, 30),
    (130, 36),
    (58, 22),
    (112, 32),
];
/// A PowerShell loop that prints numbered lines, longer than any pane is wide, twice a second.
/// Between lines most rows of the TUI stay unchanged, so a row that one frame misrepresents stays
/// in every later frame until it changes.
const NUMBERED_LINES: &str = "$i = 0; while ($true) { $i++; \
    [Console]::Out.WriteLine(('LINE-{0:D6}-' -f $i) + ('abcdefghijklmnopqrstuvwxyz0123456789' * 3)); \
    Start-Sleep -Milliseconds 500 }";
const LINE_MARK: &str = "LINE-";

#[test]
fn a_divider_drag_after_a_selecting_click_keeps_every_viewer() {
    let fixture = Fixture::new("orchbee-terminal-viewer-", &[]);
    let (left, right) = split_workspace(&fixture);
    enable_copy_on_select(&fixture);
    let tui = fixture.start_tui(COLUMNS, ROWS);
    fixture.background_output();
    fixture.settle_ready(&tui.client);
    let mut control = Control::hold(&fixture, &tui.terminal, COLUMNS, ROWS);
    // The TUI ignores presses while its host reports that the window has no focus.
    control.input(b"\x1b[I");

    for round in 0..ROUNDS {
        let before = Layout::read(&fixture, &tui.terminal);
        // The drag always shrinks the pane that the click focused.
        let (panel, column, step) = if round % 2 == 0 {
            (&right, before.divider + 10, 1)
        } else {
            (&left, before.left + 10, -1)
        };
        control.input(&press(column, 12));
        thread::sleep(Duration::from_millis(60));
        control.input(&motion(column + 3, 12));
        thread::sleep(Duration::from_millis(20));
        control.input(&release(column + 3, 12));
        thread::sleep(FOCUS_TO_DRAG);

        control.input(&press(before.divider, 20));
        thread::sleep(Duration::from_millis(40));
        for offset in 1..=DRAG_COLUMNS {
            control.input(&motion(shift(before.divider, step, offset), 20));
            thread::sleep(Duration::from_millis(16));
        }
        thread::sleep(Duration::from_millis(40));
        control.input(&release(shift(before.divider, step, DRAG_COLUMNS), 20));

        watch(&fixture, &tui.terminal, &format!("divider drag {round}"));
        let after = Layout::read(&fixture, &tui.terminal);
        let moved = (i32::from(after.divider) - i32::from(before.divider)) * step;
        assert!(
            moved >= i32::from(DRAG_COLUMNS) - 1,
            "divider drag {round} moved the divider {moved} columns:\n{}",
            after.text
        );
        let view = fixture.client(&tui.client);
        assert_eq!(
            view["view"]["selection"]["panelId"],
            panel.as_str(),
            "the click of round {round} did not focus its pane: {view}"
        );
    }
    control.release();
}

#[test]
fn window_resizes_under_soft_wrapped_output_keep_every_viewer() {
    let fixture = Fixture::new("orchbee-terminal-viewer-", &[]);
    let (left, right) = split_workspace(&fixture);
    let tui = fixture.start_tui(COLUMNS, ROWS);
    fixture.background_output();
    fixture.settle_ready(&tui.client);
    let mut control = Control::hold(&fixture, &tui.terminal, COLUMNS, ROWS);
    control.input(b"\x1b[I");
    // The left pane keeps focus, so the TUI shows its numbered lines at every window size.
    for (panel, command) in [(&left, NUMBERED_LINES), (&right, "ping -t 127.0.0.1")] {
        let sent = fixture.run(&[
            "terminal", "send", "--panel", panel, "--text", command, "--enter",
        ]);
        assert_eq!(sent.code, Some(0), "{sent}");
    }
    let mut newest = advance(&fixture, &tui.terminal, 0, "starting the numbered lines");

    for (round, (columns, rows)) in WINDOW_SIZES.into_iter().enumerate() {
        control.resize(columns, rows);
        let gesture = format!("window resize {round} to {columns}x{rows}");
        watch(&fixture, &tui.terminal, &gesture);
        newest = advance(&fixture, &tui.terminal, newest, &gesture);
    }
    for panel in [&left, &right] {
        let read = fixture.run(&["terminal", "read", "--panel", panel, "--source", "visible"]);
        assert_eq!(read.code, Some(0), "{read}");
    }
    control.release();
}

/// Splits a new Workspace into a left and a right pane.
fn split_workspace(fixture: &Fixture) -> (String, String) {
    let left = fixture.workspace("work");
    let split = fixture.run(&[
        "panel",
        "split",
        &left,
        "--direction",
        "right",
        "--no-focus",
    ]);
    assert_eq!(split.code, Some(0), "{split}");
    let right = split.document["result"]["newPanelId"]
        .as_str()
        .unwrap_or_else(|| panic!("{split}"))
        .to_owned();
    (left, right)
}

/// Reads the TUI until `WATCH` passes. A read that fails, or that shows a terminal without its
/// viewer, fails the gesture.
fn watch(fixture: &Fixture, terminal: &str, gesture: &str) {
    let deadline = Instant::now() + WATCH;
    while Instant::now() < deadline {
        let text = screen(fixture, terminal, gesture);
        assert!(
            !text.to_lowercase().contains("terminal unavailable"),
            "{gesture} cost a pane its viewer:\n{text}"
        );
    }
}

/// Waits until the TUI shows a numbered line after `newest`, and returns the newest line shown.
fn advance(fixture: &Fixture, terminal: &str, newest: u64, gesture: &str) -> u64 {
    let deadline = Instant::now() + OBSERVATION;
    loop {
        let text = screen(fixture, terminal, gesture);
        if let Some(line) = newest_line(&text).filter(|line| *line > newest) {
            return line;
        }
        assert!(
            Instant::now() < deadline,
            "after {gesture} the printing pane stopped at line {newest}:\n{text}"
        );
        thread::sleep(Duration::from_millis(100));
    }
}

/// The TUI's visible text, read through a viewer of the `host` terminal it runs in. A read that
/// fails, as one does when that viewer cannot decode a frame, fails the gesture.
fn screen(fixture: &Fixture, terminal: &str, gesture: &str) -> String {
    let read = fixture.run(&[
        "terminal",
        "read",
        "--session",
        "host",
        terminal,
        "--source",
        "visible",
    ]);
    read.document["result"]["content"]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("after {gesture} the TUI could not be read: {read}"))
        .to_owned()
}

/// The highest line number that the TUI shows.
fn newest_line(text: &str) -> Option<u64> {
    text.match_indices(LINE_MARK)
        .filter_map(|(index, _)| {
            let digits = &text[index + LINE_MARK.len()..];
            let end = digits
                .find(|character: char| !character.is_ascii_digit())
                .unwrap_or(digits.len());
            digits[..end].parse().ok()
        })
        .max()
}

/// Where the split's panes sit in the TUI, as 1-based columns.
struct Layout {
    text: String,
    /// The first column inside the left pane's frame.
    left: u16,
    /// The divider between the panes.
    divider: u16,
}

impl Layout {
    /// Reads the panes' top frame row. English text keeps every cell of that row one column
    /// wide, so a glyph's position is its column. A read that lands inside a redraw is retried.
    fn read(fixture: &Fixture, terminal: &str) -> Self {
        let deadline = Instant::now() + OBSERVATION;
        loop {
            let text = fixture.screen(terminal);
            let found = text.lines().find_map(|row| {
                let column = |glyph| {
                    row.chars()
                        .position(|cell| cell == glyph)
                        .map(|index| u16::try_from(index + 1).unwrap())
                };
                Some((column('┌')? + 1, column('┬')?))
            });
            if let Some((left, divider)) = found {
                return Self {
                    text,
                    left,
                    divider,
                };
            }
            assert!(Instant::now() < deadline, "no split panes:\n{text}");
            thread::sleep(Duration::from_millis(100));
        }
    }
}

fn shift(column: u16, step: i32, columns: u16) -> u16 {
    u16::try_from(i32::from(column) + step * i32::from(columns)).unwrap()
}

/// SGR mouse reports at a 1-based cell.
fn press(column: u16, row: u16) -> Vec<u8> {
    format!("\x1b[<0;{column};{row}M").into_bytes()
}

fn motion(column: u16, row: u16) -> Vec<u8> {
    format!("\x1b[<32;{column};{row}M").into_bytes()
}

fn release(column: u16, row: u16) -> Vec<u8> {
    format!("\x1b[<0;{column};{row}m").into_bytes()
}

/// Copy-on-select is off by default; this test needs its exports, so it saves the preference in
/// the fixture's own configuration before the TUI starts.
fn enable_copy_on_select(fixture: &Fixture) {
    let scope = UserPreferencesScope::from_environment(fixture.state_environment()).unwrap();
    let response = UserPreferencesStore::new(scope).execute(&UserPreferencesRequest::Update {
        source: "missing".to_owned(),
        changes: UserPreferencesPatch::set(PreferenceKey::CopyOnSelect, true.into()).unwrap(),
    });
    assert!(
        matches!(
            response,
            UserPreferencesResponse::Snapshot { changed: true, .. }
        ),
        "{response:?}"
    );
}
