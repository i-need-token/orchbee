//! The zoomed Panel going away on real terminals: the CLI, Server, Worker and Zig Host under test and
//! a TUI Client running in a terminal of a second Session. However the Panel goes, its zoom goes with
//! it and the remaining Panels return to the normal layout; no other Panel inherits the zoom.
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

const COLUMNS: u16 = 160;
const ROWS: u16 = 50;
const CLOSE_PANEL: &[u8] = b"\x02X";
/// Bounds how long a Panel's shell or its removal may take to show.
const LANDING: Duration = Duration::from_secs(25);

struct Driver {
    fixture: Fixture,
    tui: Tui,
    control: Control,
}

impl Driver {
    fn selection(&self) -> Value {
        self.fixture.client(&self.tui.client)["view"]["selection"].clone()
    }

    fn split(&self, panel: &str, direction: &str) -> String {
        let direction = format!("--direction={direction}");
        let environment = self.fixture.terminal_environment();
        let mut words = vec!["panel", "split", &direction, panel, "--focus"];
        words.extend(environment.iter().map(String::as_str));
        let created = self.fixture.run(&words);
        assert_eq!(created.code, Some(0), "{created}");
        created.document["result"]["panelId"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    fn zoom_current(&self, panels: u64) -> String {
        let client = self.tui.client.clone();
        let zoomed = self.fixture.run(&[
            "panel",
            "maximize",
            "--current",
            "--on",
            "--client",
            &client,
        ]);
        assert_eq!(zoomed.code, Some(0), "{zoomed}");
        let selection = self.await_panels(panels);
        assert_eq!(selection["maximized"], true, "{selection}");
        selection["panelId"].as_str().unwrap().to_owned()
    }

    fn await_panels(&self, panels: u64) -> Value {
        let deadline = Instant::now() + LANDING;
        loop {
            let selection = self.selection();
            if selection["panelCount"] == panels {
                return selection;
            }
            assert!(
                Instant::now() < deadline,
                "{panels} Panels never showed: {selection}\n{}",
                self.fixture.screen(&self.tui.terminal)
            );
            thread::sleep(Duration::from_millis(50));
        }
    }

    /// The zoomed Panel just went away by `route`: the Tab draws its normal layout.
    fn assert_normal_layout(&self, route: &str, panels: u64, gone: &str) {
        let selection = self.await_panels(panels);
        let screen = self.fixture.screen(&self.tui.terminal);
        assert_eq!(
            selection["maximized"], false,
            "{route} left another Panel zoomed: {selection}\n{screen}"
        );
        assert_ne!(selection["panelId"], gone, "{selection}");
        // Each remaining shell shows its prompt; a zoom would draw one of them.
        let shells = u64::try_from(screen.matches("PS ").count()).unwrap();
        assert!(shells >= panels, "{route} drew {shells} shells:\n{screen}");
    }
}

#[test]
fn the_zoomed_panel_going_away_exits_zoom_from_every_route() {
    let fixture = Fixture::new("orchbee-zoom-close-", &[]);
    let first = fixture.workspace("zoom");
    let tui = fixture.start_tui(COLUMNS, ROWS);
    fixture.settle_ready(&tui.client);
    let mut control = Control::hold(&fixture, &tui.terminal, COLUMNS, ROWS);
    control.input(b"\x1b[I");
    let mut driver = Driver {
        fixture,
        tui,
        control,
    };

    // Six Panels, so that each route still leaves a real choice of a Panel to inherit the zoom.
    let mut last = first.clone();
    for direction in ["right", "down", "right", "down", "right"] {
        last = driver.split(&last, direction);
    }
    driver.await_panels(6);
    thread::sleep(Duration::from_secs(6));

    // A hidden Panel going away keeps the zoom on the Panel that holds it.
    let zoomed = driver.zoom_current(6);
    let closed = driver.fixture.run(&["panel", "close", &first, "--confirm"]);
    assert_eq!(closed.code, Some(0), "{closed}");
    let selection = driver.await_panels(5);
    assert_eq!(selection["maximized"], true, "{selection}");
    assert_eq!(selection["panelId"], zoomed.as_str(), "{selection}");

    // The close action.
    driver.control.input(CLOSE_PANEL);
    driver.assert_normal_layout("the close action", 4, &zoomed);

    // The shell exiting by itself.
    let zoomed = driver.zoom_current(4);
    driver.control.input(b"exit\r");
    driver.assert_normal_layout("the shell's exit", 3, &zoomed);

    // Another Client's close of the same Panel.
    let zoomed = driver.zoom_current(3);
    let closed = driver
        .fixture
        .run(&["panel", "close", &zoomed, "--confirm"]);
    assert_eq!(closed.code, Some(0), "{closed}");
    driver.assert_normal_layout("a close from the CLI", 2, &zoomed);

    driver.control.release();
}
