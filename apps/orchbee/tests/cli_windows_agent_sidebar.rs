//! The sidebar's Agent list while its Agents work: the CLI, Server, Worker and Zig Host under test,
//! a TUI Client that runs in a terminal of a second Session, and two fake Codex processes that keep
//! printing while their window titles flip between working and idle, each in a Workspace of its own
//! inside a real Git repository. Once a row appears it must stay, whichever commits, output and Git
//! reads land meanwhile. Completion and input-driven display transitions have separate viewer witnesses.
//! Before the fix a row vanished about every two seconds.
#![cfg(windows)]

#[allow(dead_code)]
#[path = "support/windows_tui_session.rs"]
mod session;

use session::Fixture;
use std::{
    fs,
    process::Command,
    time::{Duration, Instant},
};

const COLUMNS: u16 = 112;
const ROWS: u16 = 32;
/// The branch both repositories check out.
const BRANCH: &str = "agent-sidebar";
/// Bounds how long a started Agent takes to reach the list through the Server's process sampler.
const LISTED: Duration = Duration::from_secs(20);
/// Long enough for several state changes of each Agent.
const WATCH: Duration = Duration::from_secs(12);
/// A title flip every 1.5 s and a line every 100 ms, as a working Codex does.
const FAKE_CODEX: &str = "$spin = [char]0x280B\r\n\
    while ($true) {\r\n  \
      $Host.UI.RawUI.WindowTitle = \"$spin fake\"\r\n  \
      1..15 | ForEach-Object { Write-Host \"working $_\"; Start-Sleep -Milliseconds 100 }\r\n  \
      $Host.UI.RawUI.WindowTitle = 'fake'\r\n  \
      1..15 | ForEach-Object { Write-Host \"idle $_\"; Start-Sleep -Milliseconds 100 }\r\n\
    }\r\n";

#[test]
fn listed_agents_stay_listed_and_follow_their_terminals_while_they_work() {
    let fixture = Fixture::new("orchbee-agent-sidebar-", &["work/alpha", "work/beta"]);
    fs::write(fixture.path("work/codex.ps1"), FAKE_CODEX).unwrap();
    repository(&fixture, "work/alpha");
    repository(&fixture, "work/beta");
    let tui = fixture.start_tui(COLUMNS, ROWS);
    fixture.settle_ready(&tui.client);
    let mut sidebar = Sidebar {
        fixture: &fixture,
        terminal: &tui.terminal,
        listed: 0,
        samples: 0,
        failures: Vec::new(),
    };

    // The first row must also hold while the second Agent is being created.
    start_codex(&fixture, "alpha", "work/alpha");
    sidebar.until_listed(1);
    // One Agent must visibly change color before another row can supply a different state.
    // Compare chromatic families so a Working pulse cannot masquerade as a state change.
    let mut state_colors = std::collections::BTreeSet::new();
    let deadline = Instant::now() + LISTED;
    while !(state_colors.contains(&1) && (state_colors.contains(&0) || state_colors.contains(&2))) {
        sidebar.sample();
        if let Some(color) = fixture.state_color(&tui.terminal, "fake")
            && let Some(family) = chromatic_family(&color)
        {
            state_colors.insert(family);
        }
        assert!(
            Instant::now() < deadline,
            "the same Agent's state cell never followed Working and completion colors: {state_colors:?}"
        );
    }
    start_codex(&fixture, "beta", "work/beta");
    sidebar.until_listed(2);

    let started = Instant::now();
    while started.elapsed() < WATCH {
        sidebar.sample();
    }
    assert!(
        sidebar.failures.is_empty(),
        "{} of {} samples failed\n{}",
        sidebar.failures.len(),
        sidebar.samples,
        sidebar.failures.join("\n")
    );
    let screen = fixture.screen(&tui.terminal);
    assert!(
        screen.contains(BRANCH),
        "Git never reached the sidebar:\n{screen}"
    );
}

/// Ignore brightness animation while distinguishing green Done from native or blue Working.
fn chromatic_family(color: &[u16]) -> Option<usize> {
    match color {
        [32 | 92] => Some(1),
        [34 | 94] => Some(2),
        [39 | 30 | 37 | 90 | 97] => Some(0),
        [31 | 91] => Some(3),
        [38, 5, 2 | 10] => Some(1),
        [38, 5, 4 | 12] => Some(2),
        [38, 5, 0 | 7 | 8 | 15] => Some(0),
        [38, 2, r, g, b] => {
            let channels = [*r, *g, *b];
            let maximum = *channels.iter().max()?;
            let minimum = *channels.iter().min()?;
            if maximum.saturating_sub(minimum) <= 16 {
                Some(0)
            } else {
                channels
                    .iter()
                    .position(|c| *c == maximum)
                    .map(|channel| if channel == 0 { 3 } else { channel })
            }
        }
        _ => None,
    }
}

/// Reads one TUI's screen and records every listed row that went missing.
struct Sidebar<'a> {
    fixture: &'a Fixture,
    terminal: &'a str,
    listed: usize,
    samples: usize,
    failures: Vec<String>,
}

impl Sidebar<'_> {
    fn sample(&mut self) -> String {
        let screen = self.fixture.screen(self.terminal);
        let rows = agent_rows(&screen).count();
        self.samples += 1;
        if rows < self.listed {
            self.failures.push(format!(
                "{rows} Agent rows after {} were listed:\n{screen}",
                self.listed
            ));
        }
        self.listed = self.listed.max(rows);
        screen
    }

    fn until_listed(&mut self, expected: usize) {
        let started = Instant::now();
        while self.listed < expected {
            let screen = self.sample();
            assert!(
                started.elapsed() < LISTED,
                "never listed {expected} Agents:\n{screen}"
            );
        }
    }
}

fn agent_rows(screen: &str) -> impl Iterator<Item = &str> {
    screen.lines().filter(|line| line.contains(" codex"))
}

fn repository(fixture: &Fixture, directory: &str) {
    let path = fixture.path(directory);
    for words in [
        &["init", "--quiet", "--initial-branch=agent-sidebar"][..],
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
        let output = Command::new("git")
            .args(words)
            .current_dir(&path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {words:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Starts the fake Codex in a Workspace of its own, leaving the TUI's Workspace selected.
fn start_codex(fixture: &Fixture, label: &str, directory: &str) {
    let script = fixture.path("work/codex.ps1");
    let system = std::env::var("SystemRoot").unwrap();
    let powershell = format!("{system}\\System32\\WindowsPowerShell\\v1.0\\powershell.exe");
    let cwd = fixture.path(directory);
    let environment = fixture.terminal_environment();
    let mut words = vec![
        "workspace",
        "create",
        "--label",
        label,
        "--cwd",
        &cwd,
        "--no-focus",
        "--exec",
        &powershell,
        "--arg",
        "-NoProfile",
        "--arg",
        "-ExecutionPolicy",
        "--arg",
        "Bypass",
        "--arg",
        "-File",
        "--arg",
        &script,
    ];
    words.extend(environment.iter().map(String::as_str));
    let created = fixture.run(&words);
    assert_eq!(created.code, Some(0), "{created}");
}
