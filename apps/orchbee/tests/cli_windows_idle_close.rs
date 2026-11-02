//! Closing a Tab or Panel of a real PowerShell: the CLI, Server, Worker and Zig Host under test and
//! a TUI Client running in a terminal of a second Session, driven like a person does. A shell at its
//! prompt closes without a confirmation from every route; a program running in it still asks, also
//! in a Tab this Client has no viewer of and in a Presentation it does not show.
#![cfg(windows)]

#[allow(dead_code)]
#[path = "support/windows_tui_session.rs"]
mod session;

use session::{Control, Fixture, Tui, system_program};
use std::{
    thread,
    time::{Duration, Instant},
};

const COLUMNS: u16 = 112;
const ROWS: u16 = 32;
const TAB_DIALOG: &str = "Close Tab?";
const PANEL_DIALOG: &str = "Close Panel?";
/// How long a pointer button stays down.
const HOLD: Duration = Duration::from_millis(80);
/// Bounds how long a shell, a program's output or a close may take to show.
const LANDING: Duration = Duration::from_secs(20);
const POLL: Duration = Duration::from_millis(50);
/// Outlasts a shell's return to its prompt after what it printed last.
const PROMPT_RETURN: Duration = Duration::from_secs(2);
const CLOSE_TAB: &[u8] = b"\x02x";
const CLOSE_PANEL: &[u8] = b"\x02X";

struct Driver {
    fixture: Fixture,
    tui: Tui,
    control: Control,
}

impl Driver {
    /// One Workspace of one idle PowerShell, and the TUI Client with its host terminal focused.
    fn start(prefix: &'static str, label: &str) -> (Self, String) {
        let fixture = Fixture::new(prefix, &[]);
        let panel = fixture.workspace(label);
        let tui = fixture.start_tui(COLUMNS, ROWS);
        fixture.settle_ready(&tui.client);
        let mut control = Control::hold(&fixture, &tui.terminal, COLUMNS, ROWS);
        control.input(b"\x1b[I");
        let driver = Self {
            fixture,
            tui,
            control,
        };
        driver.await_screen("the first prompt", |screen| screen.contains("PS "));
        (driver, panel)
    }

    fn screen(&self) -> String {
        self.fixture.screen(&self.tui.terminal)
    }

    /// Adds a background Tab whose shell starts with the isolated environment.
    fn tab(&self, workspace: &str, label: &str) -> String {
        let environment = self.fixture.terminal_environment();
        let mut words = vec!["tab", "create", "--workspace", workspace, "--label", label];
        words.extend(environment.iter().map(String::as_str));
        let created = self.fixture.run(&words);
        assert_eq!(created.code, Some(0), "{created}");
        self.await_screen(label, |screen| {
            screen.lines().next().unwrap().contains(label)
        });
        created.document["result"]["panelId"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    fn await_screen(&self, what: &str, done: impl Fn(&str) -> bool) -> String {
        let deadline = Instant::now() + LANDING;
        loop {
            let screen = self.screen();
            if done(&screen) {
                return screen;
            }
            assert!(Instant::now() < deadline, "never saw {what}:\n{screen}");
            thread::sleep(POLL);
        }
    }

    fn pointer(&mut self, code: u8, column: usize, row: usize) {
        self.control
            .input(format!("\x1b[<{code};{column};{row}M").as_bytes());
    }

    fn click(&mut self, column: usize, row: usize) {
        self.pointer(35, column, row);
        self.pointer(0, column, row);
        thread::sleep(HOLD);
        self.control
            .input(format!("\x1b[<0;{column};{row}m").as_bytes());
    }

    /// Clicks the `[x]` of the Tab named `label`, resting on it first as a person does: a
    /// background Tab reveals its button only under the pointer.
    fn click_close_button(&mut self, label: &str) {
        let screen = self.screen();
        let strip = screen.lines().next().unwrap().to_owned();
        let start = strip
            .find(label)
            .unwrap_or_else(|| panic!("no Tab {label}:\n{screen}"));
        let hover = strip[..start].chars().count() + 1;
        self.pointer(35, hover, 1);
        let revealed = format!("{label} [x]");
        let screen = self.await_screen(&revealed, |screen| {
            screen.lines().next().unwrap().contains(&revealed)
        });
        let strip = screen.lines().next().unwrap();
        let button = strip.find(&revealed).unwrap() + label.len() + 1;
        // The middle of the three cells, in one-based columns.
        let column = strip[..button].chars().count() + 2;
        self.click(column, 1);
    }

    /// Types a line into the focused terminal and waits for what it prints.
    fn run_in_focus(&mut self, line: &str, printed: &str) {
        self.control.input(line.as_bytes());
        self.control.input(b"\r");
        self.await_screen(printed, |screen| screen.contains(printed));
    }

    /// The close must end in `gone` without its confirmation ever being presented. A presented
    /// confirmation stays until it is answered, so polling cannot miss one.
    fn assert_closes_without_asking(&self, what: &str, dialog: &str, gone: impl Fn(&str) -> bool) {
        let deadline = Instant::now() + LANDING;
        loop {
            let screen = self.screen();
            assert!(
                !screen.contains(dialog),
                "{what} presented its confirmation:\n{screen}"
            );
            assert!(
                !screen.contains("Closing Tabs"),
                "{what} counted one Tab as a set:\n{screen}"
            );
            if gone(&screen) {
                return;
            }
            assert!(Instant::now() < deadline, "{what} never closed:\n{screen}");
            thread::sleep(POLL);
        }
    }

    /// The close must present its confirmation; the answer here keeps the target.
    fn assert_asks_and_cancel(&mut self, what: &str, dialog: &str) {
        self.await_screen(&format!("the confirmation of {what}"), |screen| {
            screen.contains(dialog)
        });
        self.control.input(b"\x1b");
        self.await_screen("the cancelled confirmation", |screen| {
            !screen.contains(dialog)
        });
    }

    fn finish(self) {
        self.control.release();
    }
}

fn strip_lacks(label: &'static str) -> impl Fn(&str) -> bool {
    move |screen| !screen.lines().next().unwrap().contains(label)
}

#[test]
fn a_shell_at_its_prompt_closes_from_every_tab_route_without_asking() {
    let (mut driver, _) = Driver::start("orchbee-idle-tabs-", "idle");
    driver.tab("idle", "second");
    driver.tab("idle", "third");
    // Let every shell reach its prompt; the selected one already shows it.
    thread::sleep(Duration::from_secs(5));

    // The reported route: the [x] of the selected Tab at an idle prompt.
    driver.click_close_button("PowerShell");
    driver.assert_closes_without_asking(
        "the selected idle Tab's [x]",
        TAB_DIALOG,
        strip_lacks("PowerShell"),
    );

    // The [x] of a background Tab, whose terminal this Client holds no viewer of.
    let active = driver.await_screen("the next Tab selected", |screen| {
        screen.contains("PS ") && screen.lines().next().unwrap().contains("second [x]")
    });
    assert!(active.lines().next().unwrap().contains("third"), "{active}");
    driver.click_close_button("third");
    driver.assert_closes_without_asking(
        "a background idle Tab's [x]",
        TAB_DIALOG,
        strip_lacks("third"),
    );

    // A shell back at its prompt is as idle as a new one, after a program it interrupted, a program
    // that finished, a cmdlet and an empty line.
    driver.run_in_focus("ping -t 127.0.0.1", "Reply from");
    driver.control.input(b"\x03");
    driver.await_screen("the interrupted program's summary", |screen| {
        screen.contains("Ping statistics")
    });
    driver.run_in_focus("ping -n 1 127.0.0.1", "Lost = 0");
    driver.run_in_focus("Write-Output marker-$(1+1)", "marker-2");
    driver.control.input(b"\r");
    thread::sleep(PROMPT_RETURN);

    // The keyboard route closes the last Tab.
    driver.control.input(CLOSE_TAB);
    driver.assert_closes_without_asking("the idle Tab's key", TAB_DIALOG, strip_lacks("second"));
    driver.finish();
}

#[test]
fn a_program_running_in_the_shell_still_asks_from_every_tab_route() {
    let (mut driver, _) = Driver::start("orchbee-busy-tabs-", "busy");

    // A console program in the selected Tab.
    driver.run_in_focus("ping -t 127.0.0.1", "Reply from");
    driver.click_close_button("PowerShell");
    driver.assert_asks_and_cancel("the [x] over a running program", TAB_DIALOG);
    driver.control.input(CLOSE_TAB);
    driver.assert_asks_and_cancel("the key over a running program", TAB_DIALOG);
    driver.control.input(b"\x03");
    driver.await_screen("the prompt after the interrupt", |screen| {
        screen.lines().rev().any(|line| line.contains("PS ")) && screen.contains("Ping statistics")
    });

    // A cmdlet that runs inside the shell, with no child process to sample.
    driver.run_in_focus(
        "Write-Output marker-$(1+1); Start-Sleep -Seconds 600",
        "marker-2",
    );
    driver.click_close_button("PowerShell");
    driver.assert_asks_and_cancel("the [x] over a running cmdlet", TAB_DIALOG);

    // A background Tab whose shell runs a console program.
    let panel = driver.tab("busy", "second");
    thread::sleep(Duration::from_secs(4));
    let typed = driver
        .fixture
        .run(&["terminal", "send", &panel, "--text", "ping -t 127.0.0.1\r"]);
    assert_eq!(typed.code, Some(0), "{typed}");
    thread::sleep(Duration::from_secs(3));
    driver.click_close_button("second");
    driver.assert_asks_and_cancel("a background Tab's [x] over a running program", TAB_DIALOG);
    driver.finish();
}

#[test]
fn the_panel_route_and_hidden_presentations_decide_from_the_real_facts() {
    let (mut driver, panel) = Driver::start("orchbee-idle-panels-", "panels");
    let split = driver.fixture.terminal_environment();
    let mut words = vec!["panel", "split", "--direction=right", &panel, "--focus"];
    words.extend(split.iter().map(String::as_str));
    let created = driver.fixture.run(&words);
    assert_eq!(created.code, Some(0), "{created}");
    thread::sleep(Duration::from_secs(5));

    // An idle Panel of a split closes at once.
    driver.control.input(CLOSE_PANEL);
    let client = driver.tui.client.clone();
    let deadline = Instant::now() + LANDING;
    loop {
        let screen = driver.screen();
        assert!(!screen.contains(PANEL_DIALOG), "asked:\n{screen}");
        if driver.fixture.client(&client)["view"]["selection"]["panelCount"] == 1 {
            break;
        }
        assert!(Instant::now() < deadline, "never closed:\n{screen}");
        thread::sleep(POLL);
    }

    // A running child Presentation behind the idle shell the Panel shows still asks, in both the
    // Panel and the Tab route.
    let ping = system_program("PING.EXE");
    let mut words = vec![
        "panel",
        "create-terminal",
        &panel,
        "--child",
        "--exec",
        &ping,
        "--arg",
        "-t",
        "--arg",
        "127.0.0.1",
    ];
    words.extend(split.iter().map(String::as_str));
    let child = driver.fixture.run(&words);
    assert_eq!(child.code, Some(0), "{child}");
    thread::sleep(Duration::from_secs(3));
    driver.control.input(CLOSE_TAB);
    driver.assert_asks_and_cancel("a Tab with a running hidden Presentation", TAB_DIALOG);
    driver.control.input(CLOSE_PANEL);
    driver.assert_asks_and_cancel("a Panel with a running hidden Presentation", PANEL_DIALOG);
    driver.finish();
}
