//! Captured runs in real shells started as plain programs, without shell integration: the CLI,
//! Server, Worker and Zig Host under test with cmd, Windows PowerShell and, where installed,
//! PowerShell 7. Each run returns the command's own output and status as the shell itself parses
//! and runs it, the echoed input never counts as a marker, and a run that times out still leaves
//! its end marker to wait for.
#![cfg(windows)]

#[allow(dead_code)]
#[path = "support/windows_tui_session.rs"]
mod session;

use serde_json::{Value, json};
use session::{Fixture, system_program};
use std::{
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

/// Bounds how long a shell may take to show its first prompt.
const PROMPT: Duration = Duration::from_secs(60);

/// Opens a Workspace whose terminal runs `program` and waits until it prompts; returns its Panel.
fn open(fixture: &Fixture, label: &str, program: &str, arguments: &[&str]) -> String {
    let work = fixture.path("work");
    let mut words = vec![
        "workspace",
        "create",
        "--label",
        label,
        "--cwd",
        &work,
        "--no-focus",
        "--exec",
        program,
    ];
    for argument in arguments {
        words.extend(["--arg", argument]);
    }
    let environment = fixture.terminal_environment();
    words.extend(environment.iter().map(String::as_str));
    let created = fixture.run(&words);
    assert_eq!(created.code, Some(0), "{created}");
    let panel = created.document["result"]["workspace"]["result"]["panelId"]
        .as_str()
        .unwrap_or_else(|| panic!("{created}"))
        .to_owned();
    let deadline = Instant::now() + PROMPT;
    loop {
        let read = fixture.run(&["terminal", "read", "--panel", &panel, "--source", "visible"]);
        let text = read.document["result"]["content"]["text"]
            .as_str()
            .unwrap_or_default();
        if text
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .is_some_and(|line| line.trim_end().ends_with('>'))
        {
            return panel;
        }
        assert!(Instant::now() < deadline, "{label} never prompted: {read}");
        thread::sleep(Duration::from_millis(200));
    }
}

/// Captures `command` in the Panel's shell; returns the CLI exit code and `result.capture`.
fn capture(fixture: &Fixture, panel: &str, shell: &str, command: &str) -> (Option<i32>, Value) {
    let run = fixture.run(&[
        "terminal",
        "run",
        "--panel",
        panel,
        command,
        "--capture",
        "--shell",
        shell,
        "--timeout",
        "20000",
    ]);
    (run.code, run.document["result"]["capture"].clone())
}

fn captured(shell: &str, status: i32, output: &str) -> Value {
    json!({"shell": shell, "exitCode": status, "output": output, "truncated": false})
}

#[test]
fn captured_runs_return_each_commands_own_output_and_status() {
    let fixture = Fixture::new("orchbee-run-capture-", &[]);

    let cmd = open(&fixture, "cmd", &system_program("cmd.exe"), &["/D"]);
    for (command, exit, status, output) in [
        ("set \"TOOLS=C:\\Program Files (x86)\\Tools\"", 0, 0, ""),
        // A bare line keeps a `)` that a value expands to literal.
        ("echo %TOOLS%", 0, 0, "C:\\Program Files (x86)\\Tools\n"),
        // `for` and `if` run as a block, so the end marker follows them once.
        ("for %i in (1 2 3) do @echo %i", 0, 0, "1\n2\n3\n"),
        ("if 1==2 (echo yes) else (echo no)", 0, 0, "no\n"),
        ("set /p =no newline<nul", 1, 1, "no newline"),
        ("cmd /c exit 4", 1, 4, ""),
    ] {
        assert_eq!(
            capture(&fixture, &cmd, "cmd", command),
            (Some(exit), captured("cmd", status, output)),
            "{command}"
        );
    }

    // A run that times out leaves the command running with the output so far, and its end
    // marker still arrives for `wait-output`.
    let run = fixture.run(&[
        "terminal",
        "run",
        "--panel",
        &cmd,
        "ping -n 5 127.0.0.1 >nul& echo late",
        "--capture",
        "--shell",
        "cmd",
        "--timeout",
        "2000",
    ]);
    assert_eq!(run.code, Some(7), "{run}");
    assert_eq!(
        run.document["result"]["capture"],
        json!({"shell": "cmd", "exitCode": null, "output": "", "truncated": false}),
        "{run}"
    );
    let marker = format!(
        "[orchbee end {}]",
        run.document["result"]["runId"].as_str().unwrap()
    );
    let waited = fixture.run(&[
        "terminal",
        "wait-output",
        "--panel",
        &cmd,
        "--match",
        &marker,
        "--source",
        "recent-unwrapped",
        "--timeout",
        "20000",
    ]);
    assert_eq!(waited.code, Some(0), "{waited}");
    assert_eq!(
        capture(&fixture, &cmd, "cmd", "echo next"),
        (Some(0), captured("cmd", 0, "next\n"))
    );

    let powershell = open(
        &fixture,
        "powershell",
        &system_program(r"WindowsPowerShell\v1.0\powershell.exe"),
        &["-NoLogo", "-NoProfile"],
    );
    for (command, exit, status, output) in [
        (
            "Write-Output 'it''s' \"$(1 + 1)\"\nWrite-Output two",
            0,
            0,
            "it's\n2\ntwo\n",
        ),
        ("[Console]::Write('no newline')", 0, 0, "no newline"),
        // A native command's exit code is the status.
        ("cmd /c exit 3", 1, 3, ""),
    ] {
        assert_eq!(
            capture(&fixture, &powershell, "powershell", command),
            (Some(exit), captured("powershell", status, output)),
            "{command}"
        );
    }
    // A terminating error ends the command with status 1 and shows the error as its output.
    let (exit, thrown) = capture(&fixture, &powershell, "powershell", "throw 'boom'; 'after'");
    assert_eq!(
        (exit, &thrown["exitCode"]),
        (Some(1), &json!(1)),
        "{thrown}"
    );
    let output = thrown["output"].as_str().unwrap_or_default();
    assert!(
        output.starts_with("boom\n") && !output.lines().any(|line| line == "after"),
        "{thrown}"
    );

    // PowerShell 7 predicts the shorter run from the longer one while it is typed, and the rows
    // the prediction took below the input still return the shorter run's lines exactly.
    let installed = std::env::var_os("ProgramFiles")
        .map(|folder| PathBuf::from(folder).join(r"PowerShell\7\pwsh.exe"))
        .filter(|program| program.is_file());
    if let Some(program) = installed {
        let pwsh = open(
            &fixture,
            "pwsh",
            program.to_str().unwrap(),
            &["-NoLogo", "-NoProfile"],
        );
        let long = "A".repeat(400);
        for (command, output) in [
            (format!("Write-Output '{long}'"), format!("{long}\n")),
            (
                "Write-Output one '' two".to_owned(),
                "one\n\ntwo\n".to_owned(),
            ),
        ] {
            assert_eq!(
                capture(&fixture, &pwsh, "powershell", &command),
                (Some(0), captured("powershell", 0, &output)),
                "{command}"
            );
        }
    }
}
