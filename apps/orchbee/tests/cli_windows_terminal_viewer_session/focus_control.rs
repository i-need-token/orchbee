//! Real Server, Worker, Host and native Clients: host focus transfers one existing terminal's
//! input and geometry, and a restored focus after closing a Client does the same without a chord.
//! Host focus events are controlled here; the operating-system emulator owns their delivery.
#![cfg(windows)]

use super::session;

use serde_json::Value;
use session::{Control, Fixture, Tui};
use std::{
    thread,
    time::{Duration, Instant},
};

// The state-color probe locates this text beside the provider in the 30-column sidebar; it must
// remain unique and fit there after terminal titles replace the initial Panel-header label.
const STATE_ROW_LABEL: &str = "job";

fn read(fixture: &Fixture, terminal: &str) -> Value {
    let read = fixture.run(&["terminal", "read", terminal, "--source", "visible"]);
    assert_eq!(read.code, Some(0), "{read}");
    read.document["result"].clone()
}

fn wait<T>(description: &str, mut observe: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(value) = observe() {
            return value;
        }
        assert!(Instant::now() < deadline, "{description}");
        thread::sleep(Duration::from_millis(100));
    }
}

/// One input submission must execute in the shared shell; echoed command text is insufficient.
fn probe(fixture: &Fixture, control: &mut Control, terminal: &str, marker: &str) {
    control.input(format!("echo {marker}\r").as_bytes());
    executed(fixture, terminal, marker);
}

fn executed(fixture: &Fixture, terminal: &str, marker: &str) {
    wait(
        &format!("focused Client failed to execute {marker}"),
        || {
            let result = read(fixture, terminal);
            result["content"]["text"]
                .as_str()
                .unwrap()
                .lines()
                .any(|line| line.trim() == marker)
                .then_some(())
        },
    );
}

fn activate(
    fixture: &Fixture,
    tui: &Tui,
    control: &mut Control,
    terminal: &str,
    marker: &str,
) -> Value {
    // Real Windows Terminal can route keys before its FocusIn record.
    control.input(format!("echo {marker}\r\x1b[I").as_bytes());
    executed(fixture, terminal, marker);
    fixture.settle_ready(&tui.client);
    let observed = read(fixture, terminal)["observed"].clone();
    assert!(observed["columns"].as_u64().is_some(), "{observed}");
    assert!(observed["screenRows"].as_u64().is_some(), "{observed}");
    serde_json::json!({"columns":observed["columns"], "rows":observed["screenRows"]})
}

#[test]
fn five_clients_transfer_input_resize_and_restore_after_the_recent_client_closes() {
    let fixture = Fixture::new("orchbee-focus-control-", &[]);
    let command = session::system_program("cmd.exe");
    let cwd = fixture.path("work");
    let environment = fixture.terminal_environment();
    let mut words = vec![
        "workspace",
        "create",
        "--label",
        "shared",
        "--cwd",
        &cwd,
        "--no-focus",
        "--exec",
        &command,
        "--arg",
        "/d",
    ];
    words.extend(environment.iter().map(String::as_str));
    let created = fixture.run(&words);
    assert_eq!(created.code, Some(0), "{created}");
    let panel = created.document["workspace"]["result"]["panelId"]
        .as_str()
        .unwrap();
    let listed = fixture.run(&["terminal", "list"]);
    let terminal = listed.document["result"]["terminals"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["boundTo"]["panelId"] == panel)
        .unwrap()["terminalId"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut clients = Vec::new();
    let mut controls: Vec<Control> = Vec::new();
    let mut geometries = Vec::new();
    for (index, (columns, rows)) in [(160, 42), (148, 38), (136, 34), (124, 32), (112, 30)]
        .into_iter()
        .enumerate()
    {
        if let Some(previous) = controls.last_mut() {
            previous.input(b"\x1b[O");
        }
        let tui = fixture.start_tui(columns, rows);
        let mut control = Control::hold(&fixture, &tui.terminal, columns, rows);
        assert_eq!(
            fixture.client(&tui.client)["view"]["selection"]["panelId"],
            panel
        );
        let geometry = activate(
            &fixture,
            &tui,
            &mut control,
            &terminal,
            &format!("FOCUS-{index}"),
        );
        if let Some(previous) = geometries.last() {
            assert_ne!(&geometry, previous);
        }
        geometries.push(geometry);
        controls.push(control);
        clients.push(tui);
    }
    controls[4].input(b"\x1b[O");
    let geometry = activate(
        &fixture,
        &clients[3],
        &mut controls[3],
        &terminal,
        "RETURN-FOUR",
    );
    assert_eq!(geometry, geometries[3]);
    controls[3].input(b"\x02q");
    wait("closed Client stayed registered", || {
        (!fixture
            .clients()
            .iter()
            .any(|client| client["clientId"] == clients[3].client))
        .then_some(())
    });
    // No focus event: the Server must promote the most recently focused surviving Viewer.
    let geometry = wait(
        "Server did not restore the surviving Viewer's dimensions",
        || {
            let observed = read(&fixture, &terminal)["observed"].clone();
            let geometry =
                serde_json::json!({"columns":observed["columns"], "rows":observed["screenRows"]});
            (geometry == geometries[4]).then_some(geometry)
        },
    );
    fixture.settle_ready(&clients[4].client);
    probe(&fixture, &mut controls[4], &terminal, "RESTORED-FIVE");
    assert_eq!(
        geometry, geometries[4],
        "restored focus must restore its own dimensions"
    );

    controls[4].input(b"for /L %i in (1,1,100) do @echo HISTORY-LINE-%i\r");
    wait("shell history output missing", || {
        read(&fixture, &terminal)["content"]["text"]
            .as_str()
            .unwrap()
            .contains("HISTORY-LINE-100")
            .then_some(())
    });
    controls[4].input(b"\x1b[<64;90;10M");
    wait("wheel did not enter history", || {
        let screen = fixture.screen(&clients[4].terminal);
        screen.contains("HISTORY").then(|| {
            assert!(
                !screen.contains("Esc back to live"),
                "history footer covered content: {screen}"
            );
            assert!(
                !screen.contains("Row 0 of"),
                "history position footer remained: {screen}"
            );
        })
    });
    controls[4].input(b"\x1b");
    fixture.settle_ready(&clients[4].client);
    probe(&fixture, &mut controls[4], &terminal, "LIVE-AGAIN");
}

fn report(fixture: &Fixture, panel: &str, state: &str) {
    let result = fixture.run(&[
        "panel",
        "report-agent",
        panel,
        "--source",
        "controlled-agent",
        "--agent",
        "decision-agent",
        "--state",
        state,
    ]);
    assert_eq!(result.code, Some(0), "{result}");
}

fn attention(fixture: &Fixture, tui: &Tui, panel: &str, seen: bool) {
    wait(&format!("Panel {panel} never became seen={seen}"), || {
        let result = fixture.run(&["agent", "get", "--panel", panel, "--client", &tui.client]);
        let client = &result.document["result"]["inspection"]["clientView"];
        (result.code == Some(0) && client["seen"] == seen).then_some(())
    });
}

fn agent_color(fixture: &Fixture, tui: &Tui) -> Option<Vec<u16>> {
    fixture.state_color(&tui.terminal, STATE_ROW_LABEL)
}

#[test]
fn a_visible_split_stays_unread_until_focused_and_completion_survives_viewing_until_input() {
    let fixture = Fixture::new("orchbee-split-attention-", &[]);
    let first = fixture.workspace("decisions");
    let split = fixture.run(&[
        "panel",
        "split",
        &first,
        "--direction",
        "right",
        "--label",
        STATE_ROW_LABEL,
        "--no-focus",
    ]);
    assert_eq!(split.code, Some(0), "{split}");
    let second = split.document["result"]["newPanelId"]
        .as_str()
        .unwrap_or_else(|| panic!("{split}"))
        .to_owned();
    let tui = fixture.start_tui(160, 40);
    let mut control = Control::hold(&fixture, &tui.terminal, 160, 40);
    control.input(b"\x1b[I");
    fixture.settle_ready(&tui.client);
    assert_eq!(
        fixture.client(&tui.client)["view"]["selection"]["panelId"],
        first
    );
    report(&fixture, &second, "done");
    let completed_color = wait(
        "explicit completion never acquired a colored state cell",
        || agent_color(&fixture, &tui),
    );
    report(&fixture, &second, "working");
    wait("Working kept the completed cell color", || {
        agent_color(&fixture, &tui).filter(|color| color != &completed_color)
    });
    attention(&fixture, &tui, &second, false);
    report(&fixture, &second, "idle");
    let completed = fixture.run(&["agent", "get", "--panel", &second]);
    assert_eq!(
        completed.document["result"]["inspection"]["semantic"]["observation"]["view"]["state"],
        "idle",
        "{completed}"
    );
    attention(&fixture, &tui, &second, false);
    // An actual pointer gesture into the right-hand pane, rather than a read or a paint.
    control.input(b"\x1b[<0;130;10M\x1b[<0;130;10m");
    wait("pointer did not focus the right pane", || {
        (fixture.client(&tui.client)["view"]["selection"]["panelId"] == second).then_some(())
    });
    attention(&fixture, &tui, &second, true);
    wait(
        "quiescent completion never acquired the Done cell color",
        || agent_color(&fixture, &tui).filter(|color| color == &completed_color),
    );
    // This Client has never observed Working; it must still read the same completion from Server.
    let late = fixture.start_tui(160, 40);
    wait("late Client missed Server completion", || {
        agent_color(&fixture, &late).filter(|color| color == &completed_color)
    });
    control.input(b"\x1b[<0;60;10M\x1b[<0;60;10m");
    wait("pointer did not return to the left pane", || {
        (fixture.client(&tui.client)["view"]["selection"]["panelId"] == first).then_some(())
    });
    assert_eq!(
        agent_color(&fixture, &tui),
        Some(completed_color.clone()),
        "viewing must not clear completion"
    );
    control.input(b"\x1b[<0;130;10M\x1b[<0;130;10m");
    fixture.settle_ready(&tui.client);
    control.input(b"x");
    wait(
        "accepted terminal input did not clear completion for the typing Client",
        || agent_color(&fixture, &tui).filter(|color| color != &completed_color),
    );
    wait(
        "input in one Client did not clear the peer's completion",
        || agent_color(&fixture, &late).filter(|color| color != &completed_color),
    );
    report(&fixture, &second, "working");
    report(&fixture, &second, "idle");
    attention(&fixture, &tui, &second, true);
}
