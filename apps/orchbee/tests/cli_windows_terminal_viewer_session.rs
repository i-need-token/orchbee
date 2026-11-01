//! Terminal viewers through a real local Session: the CLI, Server, Worker and Zig Host under test,
//! a real TUI Client that runs in a terminal of a second Session, and a terminal that prints in
//! another Workspace. The Client drives a split Workspace with pointer input as a user would: a
//! click whose pointer moves before release focuses a pane and leaves a selection in it, which the
//! default copy-on-select exports. A divider drag about 400 ms later resizes that pane once per
//! column. The Host reports the selection again at each new generation, faster than a busy Client
//! polls, and refuses the export's cancellation or later chunk acknowledgements once the export
//! is over. None of that may cost a pane its viewer: the TUI never shows a terminal as unavailable.
#![cfg(windows)]

#[allow(dead_code)]
#[path = "support/windows_tui_session.rs"]
mod session;

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

#[test]
fn a_divider_drag_after_a_selecting_click_keeps_every_viewer() {
    let fixture = Fixture::new("orchbee-terminal-viewer-", &[]);
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

        let after = watch(&fixture, &tui.terminal, &format!("divider drag {round}"));
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

/// Reads the TUI until `WATCH` passes: no read may show a terminal without its viewer.
fn watch(fixture: &Fixture, terminal: &str, gesture: &str) -> Layout {
    let deadline = Instant::now() + WATCH;
    loop {
        let text = fixture.screen(terminal);
        assert!(
            !text.to_lowercase().contains("terminal unavailable"),
            "{gesture} cost a pane its viewer:\n{text}"
        );
        if Instant::now() >= deadline {
            return Layout::read(fixture, terminal);
        }
    }
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
