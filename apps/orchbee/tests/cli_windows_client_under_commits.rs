//! A live TUI Client while unrelated commits keep refreshing it: the CLI, Server, Worker and Zig
//! Host under test, a TUI Client that runs in a terminal of a second Session, another Client that
//! renames a Workspace every 250 to 900 ms, and a terminal that prints in another Workspace. A
//! refresh between a pointer press and its release, between a prefix and the key that completes
//! its chord, or between a divider release and the Server's answer must not drop, leak or refuse
//! that input, nor may a divider drag that starts while the previous release still settles; a
//! Client-relative query must not refuse a Client whose selection held. Pointer input arrives as
//! a busy Client reads a real one: a press may share its read with the motion before it, and
//! time passes before its release.
#![cfg(windows)]

#[allow(dead_code)]
#[path = "support/windows_tui_session.rs"]
mod session;

use serde_json::Value;
use session::{Fixture, Tui, system_program};
use std::{
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    thread,
    time::{Duration, Instant},
};

/// Pauses between the other Client's renames, cycled: one rename every 250 to 900 ms.
const RENAME_PAUSES: [u64; 8] = [250, 640, 410, 900, 330, 780, 520, 260];
/// Renames that must land during each phase, so that no phase passes in a quiet Workbench.
const PHASE_COMMITS: usize = 3;
/// Sidebar switches per run; one lost click in about four failed before the fix.
const SWITCHES: usize = 20;
/// Prefix chords per kind of interruption; each one failed before the fix.
const CHORDS: usize = 4;
/// Divider drags per run, three per round trip of the divider.
const DRAGS: usize = 12;
/// The notice of a release that other commits kept outpacing until its bounded resubmissions ran
/// out. The renames here outpace a debug build's reads often enough that a run may show it.
const CONTENDED: &str = "Resize not applied; the Workbench kept changing";
/// Drags that may end under that notice per run; without resubmission most of them would.
const CONTENDED_DRAGS: usize = DRAGS / 4;
/// Client-relative reads per query; about one in four was refused before the fix.
const QUERIES: usize = 20;
/// How long a pointer button stays down after its press has been written.
const HOLD: Duration = Duration::from_millis(80);
/// Bounds how long a switch, chord or drag may take to show.
const LANDING: Duration = Duration::from_secs(4);
/// Pause after each landed input, so that the next one starts from a settled view.
const GAP: Duration = Duration::from_millis(400);
/// The status line's indicator while the Client's own resize settles.
const RESIZING: &str = "Resizing";
/// Bounds how long the last release may settle; the Client resubmits a contended one for 5 s.
const SETTLE: Duration = Duration::from_secs(10);
/// Columns one drag moves the divider.
const DRAG_COLUMNS: usize = 8;

#[test]
fn a_live_client_keeps_its_input_and_answers_queries_while_another_client_commits() {
    let fixture = Fixture::new(
        "orchbee-client-commits-",
        &["work/alpha", "work/beta", "work/gamma"],
    );
    let alpha = fixture.workspace_in("alpha", "work/alpha");
    let beta = fixture.workspace_in("beta", "work/beta");
    let renamed = fixture.workspace_in("gamma", "work/gamma");
    fixture.background_output();
    // A second Panel gives `Ctrl+B z` a Panel to zoom and the pointer a divider to drag.
    split(&fixture, text(&alpha, "panelId"));
    let tui = fixture.start_tui(112, 32);
    fixture.settle_ready(&tui.client);
    // The host terminal reports focus; the TUI starts without one.
    write(&fixture, &tui, "\x1b[I");

    let workspaces = [Workspace::of("alpha", &alpha), Workspace::of("beta", &beta)];
    let stop = AtomicBool::new(false);
    let commits = AtomicUsize::new(0);
    let failures = thread::scope(|scope| {
        let _stop = StopOnDrop(&stop);
        scope.spawn(|| rename(&fixture, text(&renamed, "workspaceId"), &stop, &commits));
        let mut failures = Vec::new();
        let mut phase = |name: &str, body: &mut dyn FnMut() -> Vec<String>| {
            let before = commits.load(Ordering::Relaxed);
            failures.extend(
                body()
                    .into_iter()
                    .map(|failure| format!("{name}: {failure}")),
            );
            let landed = commits.load(Ordering::Relaxed) - before;
            if landed < PHASE_COMMITS {
                failures.push(format!("{name}: only {landed} renames landed meanwhile"));
            }
        };
        phase("sidebar click", &mut || {
            switches(&fixture, &tui, &workspaces)
        });
        focus(&fixture, &tui, &workspaces[0]);
        phase("prefix chord", &mut || chords(&fixture, &tui));
        unzoom(&fixture, &tui, text(&alpha, "panelId"));
        phase("divider drag", &mut || drags(&fixture, &tui));
        phase("client query", &mut || {
            queries(&fixture, &tui, text(&alpha, "panelId"))
        });
        failures
    });
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// One Workspace the pointer switches to, by its sidebar label and TerminalContext.
#[derive(Clone, Copy)]
struct Workspace<'a> {
    label: &'a str,
    id: &'a str,
    context: &'a str,
}

impl<'a> Workspace<'a> {
    fn of(label: &'a str, created: &'a Value) -> Self {
        Self {
            label,
            id: text(created, "workspaceId"),
            context: text(created, "terminalContextId"),
        }
    }
}

/// Sets `stop` when the scenario ends, panicking or not, so that the renamer always returns.
struct StopOnDrop<'a>(&'a AtomicBool);

impl Drop for StopOnDrop<'_> {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// Another Client: renames `workspace` until `stop`, counting the renames that committed.
fn rename(fixture: &Fixture, workspace: &str, stop: &AtomicBool, commits: &AtomicUsize) {
    for (round, pause) in RENAME_PAUSES.iter().cycle().enumerate() {
        if stop.load(Ordering::Relaxed) {
            return;
        }
        let label = format!("gamma-{round}");
        if fixture
            .run(&["workspace", "rename", workspace, "--label", &label])
            .code
            == Some(0)
        {
            commits.fetch_add(1, Ordering::Relaxed);
        }
        thread::sleep(Duration::from_millis(*pause));
    }
}

/// Clicks the sidebar row of whichever Workspace is not current; every click must switch.
fn switches(fixture: &Fixture, tui: &Tui, workspaces: &[Workspace<'_>; 2]) -> Vec<String> {
    let mut failures = Vec::new();
    for round in 0..SWITCHES {
        let current = selection(fixture, tui)["terminalContextId"].clone();
        let target = if current == workspaces[0].context {
            workspaces[1]
        } else {
            workspaces[0]
        };
        let row = sidebar_row(fixture, tui, target.label);
        click(fixture, tui, 3, row);
        if !until(|| selection(fixture, tui)["terminalContextId"] == target.context) {
            failures.push(format!("click {round} on {} did not switch", target.label));
        }
        thread::sleep(GAP);
    }
    failures
}

/// Completes `Ctrl+B z` after a click that shares the prefix's input packet, and after pointer
/// motion between the prefix and `z`; each chord must toggle zoom and type nothing.
fn chords(fixture: &Fixture, tui: &Tui) -> Vec<String> {
    let mut failures = Vec::new();
    for round in 0..CHORDS {
        // A click on the current Workspace's row, then the prefix, in one write.
        let row = sidebar_row(fixture, tui, "alpha");
        let click = format!("\x1b[<0;3;{row}M\x1b[<0;3;{row}m\x02");
        if let Some(failure) = chord(fixture, tui, |fixture, tui| write(fixture, tui, &click)) {
            failures.push(format!(
                "{round} after a click in the prefix's packet: {failure}"
            ));
        }
        if let Some(failure) = chord(fixture, tui, |fixture, tui| {
            fixture.keys(&tui.terminal, &["ctrl+b"]);
            thread::sleep(Duration::from_millis(100));
            write(fixture, tui, "\x1b[<35;60;20M");
        }) {
            failures.push(format!(
                "{round} with pointer motion after the prefix: {failure}"
            ));
        }
    }
    failures
}

/// Arms the prefix through `prefix`, then types `z` on its own and waits for the zoom to toggle.
fn chord(fixture: &Fixture, tui: &Tui, prefix: impl Fn(&Fixture, &Tui)) -> Option<String> {
    let zoomed = selection(fixture, tui)["zoomed"].clone();
    prefix(fixture, tui);
    thread::sleep(Duration::from_millis(100));
    fixture.keys(&tui.terminal, &["z"]);
    let toggled = until(|| selection(fixture, tui)["zoomed"] != zoomed);
    let (terminal, prompt) = prompt(fixture, tui);
    let leaked = prompt.trim_end().ends_with('z');
    if leaked {
        // Clear the stray key from the shell's input line before the next chord.
        let cleared = fixture.run(&["terminal", "send", &terminal, "--key", "escape"]);
        assert_eq!(cleared.code, Some(0), "{cleared}");
    }
    thread::sleep(GAP);
    (!toggled || leaked)
        .then(|| format!("zoom toggled: {toggled}; the selected Panel shows `{prompt}`"))
}

/// Drags the divider between the two Panels in round trips of three stops; each release must
/// land, or end under the contention notice for at most a quarter of the drags. The second stop
/// is dragged from the first one's released draft as soon as that shows, while the first release
/// still settles.
fn drags(fixture: &Fixture, tui: &Tui) -> Vec<String> {
    let lines = screen(fixture, tui);
    let (start, border) =
        divider(&lines).unwrap_or_else(|| panic!("no divider:\n{}", lines.join("\n")));
    let row = border + 6;
    let stops = [start - DRAG_COLUMNS, start - 2 * DRAG_COLUMNS, start];
    let mut at = start;
    let mut failures = Vec::new();
    let mut contention = Contention::default();
    for round in 0..DRAGS {
        // A refusal that came after the previous drag settled is still on the status line.
        if let Some(notice) = contention.observe(notice(&screen(fixture, tui))) {
            failures.push(format!("after drag {}: {notice}", round.saturating_sub(1)));
        }
        let to = stops[round % stops.len()];
        write(fixture, tui, &format!("\x1b[<0;{at};{row}M"));
        thread::sleep(Duration::from_millis(50));
        write(
            fixture,
            tui,
            &format!("\x1b[<32;{};{row}M", at.midpoint(to)),
        );
        write(fixture, tui, &format!("\x1b[<32;{to};{row}M"));
        thread::sleep(Duration::from_millis(50));
        write(fixture, tui, &format!("\x1b[<0;{to};{row}m"));
        // The next drag starts on this release's draft before the Server has answered it.
        if round % stops.len() == 0
            && until(|| divider(&screen(fixture, tui)).is_some_and(|(column, _)| column == to))
        {
            at = to;
            continue;
        }
        let (column, shown) = landing(fixture, tui, to);
        let contended = shown
            .as_ref()
            .is_some_and(|notice| notice.contains(CONTENDED));
        if let Some(notice) = contention.observe(shown) {
            failures.push(format!("drag {round} from {at} to {to}: {notice}"));
        } else if !contended && column.abs_diff(to) > 1 {
            failures.push(format!(
                "drag {round} from {at} to {to}: the divider stayed at {column}"
            ));
        }
        at = column;
        thread::sleep(GAP);
    }
    // A Client refuses control steps as busy while its own resize settles; the next phase reads a
    // Client that only background refreshes keep busy.
    let deadline = Instant::now() + SETTLE;
    while screen(fixture, tui)
        .iter()
        .any(|line| line.contains(RESIZING))
    {
        if Instant::now() >= deadline {
            failures.push(format!("the last release still settled after {SETTLE:?}"));
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    eprintln!(
        "{} of {DRAGS} divider drags ended under the contention notice",
        contention.episodes
    );
    if contention.episodes > CONTENDED_DRAGS {
        failures.push(format!(
            "{} of {DRAGS} drags ended under the contention notice",
            contention.episodes
        ));
    }
    failures
}

/// Counts each showing of the contention notice once; any other resize notice is a failure.
#[derive(Default)]
struct Contention {
    episodes: usize,
    shown: bool,
}

impl Contention {
    /// Records the resize notice the status line shows, returning it unless it is contention.
    fn observe(&mut self, notice: Option<String>) -> Option<String> {
        let contended = notice
            .as_ref()
            .is_some_and(|notice| notice.contains(CONTENDED));
        if contended && !self.shown {
            self.episodes += 1;
        }
        self.shown = contended;
        notice.filter(|_| !contended)
    }
}

/// Waits until the divider shows one column on three reads in a row, and returns that column with
/// the resize notice the status line showed meanwhile, if any.
fn landing(fixture: &Fixture, tui: &Tui, to: usize) -> (usize, Option<String>) {
    let deadline = Instant::now() + LANDING;
    let mut shown = None;
    let mut columns = Vec::new();
    loop {
        let lines = screen(fixture, tui);
        shown = shown.or_else(|| notice(&lines));
        columns.push(divider(&lines).map_or(0, |(column, _)| column));
        // The released draft shows at once; only a refusal moves the divider back.
        if let [.., first, second, third] = columns[..]
            && first == second
            && second == third
            && (third.abs_diff(to) <= 1 || Instant::now() + LANDING / 2 >= deadline)
        {
            break;
        }
        if Instant::now() >= deadline {
            break;
        }
    }
    (columns.last().copied().unwrap_or_default(), shown)
}

/// The resize notice the status line shows, if any.
fn notice(lines: &[String]) -> Option<String> {
    lines
        .iter()
        .find(|line| line.contains("Resize"))
        .map(|line| line.trim().to_owned())
}

/// Reads the selected Panel through the Client. Its view revision steps on every refresh the
/// renames cause while its selection holds, so no read may fail on the Client; only the canonical
/// Workbench revision fence, which any commit during the read moves, may refuse one.
fn queries(fixture: &Fixture, tui: &Tui, panel: &str) -> Vec<String> {
    let mut failures = Vec::new();
    let client = tui.client.as_str();
    for words in [
        &["panel", "get", panel, "--client", client][..],
        &["panel", "current", "--client", client][..],
    ] {
        let mut answered = 0;
        let mut elapsed = Vec::new();
        for _ in 0..QUERIES {
            let run = fixture.run(words);
            elapsed.push(run.elapsed);
            if run.code == Some(0) {
                answered += 1;
            } else if run.code != Some(3)
                || run.document["failure"]["details"]["reason"] != "workbench-changed"
            {
                failures.push(run.to_string());
            }
        }
        elapsed.sort();
        eprintln!(
            "`orchbee {}` answered {answered} of {QUERIES} reads in a median {:?}",
            words.join(" "),
            elapsed[elapsed.len() / 2]
        );
    }
    failures
}

/// A pointer click as a mouse reports it to a busy Client: the motion onto the cell and the press
/// arrive in one read, and after a hold the release follows on its own.
fn click(fixture: &Fixture, tui: &Tui, column: usize, row: usize) {
    write(
        fixture,
        tui,
        &format!("\x1b[<35;{column};{row}M\x1b[<0;{column};{row}M"),
    );
    thread::sleep(HOLD);
    write(fixture, tui, &format!("\x1b[<0;{column};{row}m"));
}

/// Writes raw input to the TUI through its `host` terminal.
fn write(fixture: &Fixture, tui: &Tui, input: &str) {
    let sent = fixture.run(&[
        "terminal",
        "send",
        "--session",
        "host",
        &tui.terminal,
        "--text",
        input,
    ]);
    assert_eq!(sent.code, Some(0), "{sent}");
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

fn selection(fixture: &Fixture, tui: &Tui) -> Value {
    fixture.client(&tui.client)["view"]["selection"].clone()
}

/// The TUI's screen, one string per row.
fn screen(fixture: &Fixture, tui: &Tui) -> Vec<String> {
    fixture
        .screen(&tui.terminal)
        .lines()
        .map(str::to_owned)
        .collect()
}

/// The one-based screen row of `label` in the sidebar, which may show a hovered row's affordance
/// after the label.
fn sidebar_row(fixture: &Fixture, tui: &Tui, label: &str) -> usize {
    let lines = screen(fixture, tui);
    lines
        .iter()
        .position(|line| {
            line.split('│')
                .next()
                .and_then(|cell| cell.trim().strip_prefix(label))
                .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
        })
        .map(|index| index + 1)
        .unwrap_or_else(|| panic!("no sidebar row for {label}:\n{}", lines.join("\n")))
}

/// The one-based column of the divider between the Tab's two Panels and the row of their top
/// border.
fn divider(lines: &[String]) -> Option<(usize, usize)> {
    lines.iter().enumerate().find_map(|(index, line)| {
        let cells: Vec<char> = line.chars().collect();
        let frame = cells.iter().position(|cell| *cell == '┌')?;
        let offset = cells[frame..].iter().position(|cell| *cell == '┬')?;
        Some((frame + offset + 1, index + 1))
    })
}

/// The selected Panel's terminal in the observed Session and the last line it shows.
fn prompt(fixture: &Fixture, tui: &Tui) -> (String, String) {
    let panel = selection(fixture, tui)["panelId"].clone();
    let listed = fixture.run(&["terminal", "list"]);
    let terminal = listed.document["result"]["terminals"]
        .as_array()
        .and_then(|terminals| {
            terminals
                .iter()
                .find(|terminal| terminal["boundTo"]["panelId"] == panel)
        })
        .and_then(|terminal| terminal["terminalId"].as_str())
        .unwrap_or_else(|| panic!("{listed}"))
        .to_owned();
    let read = fixture.run(&["terminal", "read", &terminal]);
    assert_eq!(read.code, Some(0), "{read}");
    let last = read.document["result"]["content"]["text"]
        .as_str()
        .unwrap_or_default()
        .lines()
        .rfind(|line| !line.trim().is_empty())
        .unwrap_or_default()
        .to_owned();
    (terminal, last)
}

/// Splits `panel` to the right with a second shell, before any Client attaches.
fn split(fixture: &Fixture, panel: &str) {
    let shell = system_program("cmd.exe");
    let mut words = vec![
        "panel",
        "split",
        panel,
        "--direction",
        "right",
        "--no-focus",
        "--exec",
        &shell,
    ];
    let environment = fixture.terminal_environment();
    words.extend(environment.iter().map(String::as_str));
    let split = fixture.run(&words);
    assert_eq!(split.code, Some(0), "{split}");
}

/// Makes `workspace` current through the CLI, which other commits may refuse for a moment.
fn focus(fixture: &Fixture, tui: &Tui, workspace: &Workspace<'_>) {
    assert!(
        until(|| {
            selection(fixture, tui)["terminalContextId"] == workspace.context
                || fixture
                    .run(&["workspace", "focus", workspace.id, "--client", &tui.client])
                    .code
                    == Some(0)
        }),
        "the Client never showed {}",
        workspace.label
    );
    fixture.settle_ready(&tui.client);
}

/// Leaves `panel`'s Tab unzoomed through the CLI, whatever the chords left behind.
fn unzoom(fixture: &Fixture, tui: &Tui, panel: &str) {
    assert!(
        until(|| {
            selection(fixture, tui)["zoomed"] == false
                || fixture
                    .run(&["panel", "zoom", panel, "--off", "--client", &tui.client])
                    .code
                    == Some(0)
        }),
        "the Client stayed zoomed"
    );
    fixture.settle_ready(&tui.client);
}

fn text<'a>(value: &'a Value, field: &str) -> &'a str {
    value[field]
        .as_str()
        .unwrap_or_else(|| panic!("no {field} in {value}"))
}
