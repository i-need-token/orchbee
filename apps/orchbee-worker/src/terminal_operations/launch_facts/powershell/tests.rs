use super::*;
use std::{
    io::Write,
    os::windows::process::CommandExt,
    path::PathBuf,
    process::{Command, Stdio},
};

#[test]
fn recognizes_only_plain_interactive_powershell_and_preserves_input_flags() {
    let prompt = windows_interactive_powershell_initialization();
    assert!(prompt.contains("UTF8Encoding"));
    assert!(prompt.contains("__OrchbeeDirectoryPrompt"));
    assert!(prompt.contains("PSConsoleHostReadLine"));
    for executable in [
        r"C:\Windows\PowerShell.EXE",
        r"C:/Program Files/PowerShell/7/pwsh.exe",
        "pwsh",
    ] {
        let args = vec!["-NoProfile".to_owned(), "-nOeXiT".to_owned()];
        let actual = interactive_arguments(executable, args.clone());
        assert_eq!(&actual[..2], &args);
        assert_eq!(&actual[2..], &["-Command", &prompt]);
        assert_eq!(
            interactive_arguments(executable, vec![]),
            ["-NoExit", "-Command", &prompt]
        );
    }
    for args in [
        vec!["-Command", "literal"],
        vec!["-File", "script.ps1"],
        vec!["-NonInteractive"],
        vec!["-nop"],
        vec!["-NoExit:1"],
        vec!["-NoLogo"; 4],
    ] {
        let args: Vec<_> = args.into_iter().map(str::to_owned).collect();
        assert_eq!(interactive_arguments("pwsh.exe", args.clone()), args);
    }
    for exe in ["cmd.exe", "powershell.exe.bak", "python.exe"] {
        assert!(interactive_arguments(exe, vec![]).is_empty());
    }
}

#[test]
fn actual_powershell_initialization_decodes_native_utf8_in_pipelines() {
    let node = std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .map(|directory| directory.join("node.exe"))
        .find(|path| path.is_file())
        .unwrap();
    let node = node.to_string_lossy().replace('\'', "''");
    let initialization = windows_interactive_powershell_initialization();
    for shell in [
        PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe"),
        PathBuf::from(std::env::var_os("ProgramFiles").unwrap()).join("PowerShell/7/pwsh.exe"),
    ] {
        if !shell.is_file() {
            continue;
        }
        let script = format!(
            "[Console]::OutputEncoding = [System.Text.Encoding]::GetEncoding(437); {initialization}; $record = & '{node}' -e 'process.stdout.write(JSON.stringify({{label:String.fromCodePoint(27979,35797,32,24037,20316,21306)}}))' | ConvertFrom-Json; $expected = -join @([char]27979,[char]35797,[char]32,[char]24037,[char]20316,[char]21306); if ($record.label -ne $expected) {{ throw 'native UTF-8 decoded incorrectly' }}; if ($global:OutputEncoding.CodePage -ne 65001 -or [Console]::OutputEncoding.CodePage -ne 65001) {{ throw 'pipeline encoding not UTF-8' }}; 'utf8-pipeline-ok'"
        );
        let output = Command::new(&shell)
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                &script,
            ])
            .creation_flags(0x0800_0000)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}: {}",
            shell.display(),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("utf8-pipeline-ok"));
    }
}

#[test]
fn actual_windows_powershell_prompts_preserve_status_and_publish_provider_directory() {
    let prompt = windows_interactive_powershell_initialization();
    let legacy = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let modern =
        PathBuf::from(std::env::var_os("ProgramFiles").unwrap()).join("PowerShell/7/pwsh.exe");
    for shell in [legacy, modern] {
        if !shell.is_file() {
            continue;
        }
        // No files/profile are written. The command owns and reaps its child.
        let script = format!(
            r"function global:prompt {{ if ($?) {{ 'ok> ' }} else {{ 'failed> ' }} }}
{prompt}
{prompt}
$null = Get-Item -LiteralPath 'Z:\orchbee-cwd-absent' -ErrorAction SilentlyContinue
$answer = prompt
if ($answer -ne 'failed> ') {{ throw ('lost status: ' + $answer) }}
$global:__OrchbeeRunId = 'ffffffffffffffffffffffffffffffff'
$null = Get-Item -LiteralPath 'Z:\orchbee-cwd-absent' -ErrorAction SilentlyContinue
$answer = prompt
if ($answer -ne 'failed> ' -or $null -ne $global:__OrchbeeRunId) {{ throw 'lost failed command status' }}
$global:__OrchbeeRunId = 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'
Set-Location -LiteralPath $env:SystemRoot
$answer = prompt
if ($answer -ne 'ok> ' -or $null -ne $global:__OrchbeeRunId) {{ throw 'lost successful command status' }}
if ([Environment]::CurrentDirectory -ne $pwd.ProviderPath) {{ throw 'stale process cwd' }}
$last = [Environment]::CurrentDirectory
Set-Location HKCU:\
$null = prompt
if ([Environment]::CurrentDirectory -ne $last) {{ throw 'non-filesystem cwd' }}
'directory-prompt-ok'"
        );
        let output = Command::new(&shell)
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                &script,
            ])
            .creation_flags(0x0800_0000)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}: {}",
            shell.display(),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("directory-prompt-ok"));
        for (id, status) in [("f".repeat(32), 1), ("b".repeat(32), 0)] {
            assert_eq!(
                stdout
                    .matches(&format!("orchbee.command.v1;finish;{id};{status}"))
                    .count(),
                1,
                "{stdout}"
            );
        }
    }
}

#[test]
fn actual_interactive_powershell_session_history_holds_only_user_commands() {
    for shell in [
        PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe"),
        PathBuf::from(std::env::var_os("ProgramFiles").unwrap()).join("PowerShell/7/pwsh.exe"),
    ] {
        if !shell.is_file() {
            continue;
        }
        let mut arguments = interactive_arguments(
            &shell.to_string_lossy(),
            vec!["-NoLogo".to_owned(), "-NoProfile".to_owned()],
        );
        // Without module autoloading the reader leaves PSReadLine unloaded, so the host reads
        // the piped lines below and never opens a PSReadLine history file. The host may echo
        // those lines, so only the joined output contains the history: marker.
        let startup = arguments.pop().unwrap();
        arguments.push(format!(
            "$PSModuleAutoLoadingPreference = 'None'\n{startup}"
        ));
        let mut child = Command::new(&shell)
            .args(&arguments)
            .creation_flags(0x0800_0000)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"'typed'\nGet-History | ForEach-Object { 'history' + ':' + $_.CommandLine }\nexit\n")
            .unwrap();
        let output = child.wait_with_output().unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let history: Vec<_> = stdout
            .lines()
            .filter_map(|line| line.split_once("history:").map(|(_, entry)| entry))
            .collect();
        assert_eq!(history, ["'typed'"], "{}: {stdout}", shell.display());
    }
}

#[test]
fn agent_start_reaches_only_shells_that_run_the_managed_startup() {
    const ID: &str = "0f8b1e8e-7c3a-4d55-9a26-5f0c2b9e4a11";
    let start = |program: &str, arguments: &[&str]| {
        AgentStartCommand::new(
            program.to_owned(),
            arguments
                .iter()
                .map(|argument| (*argument).to_owned())
                .collect(),
        )
        .unwrap()
    };
    let claude = start("claude", &["--session-id", ID]);
    let openclaw = start("openclaw", &["chat"]);
    let codex = start("codex", &[]);
    let resolve = |program: &str| {
        format!(
            "& (Get-Command '{program}' -CommandType Application,ExternalScript -ErrorAction Stop | Select-Object -First 1)"
        )
    };
    let claude_line = format!("{} '--session-id' '{ID}'", resolve("claude"));
    let openclaw_line = format!("{} 'chat'", resolve("openclaw"));
    let codex_line = resolve("codex");
    // Executable, launch arguments, requested start, expected first-command line.
    type Row<'a> = (
        &'a str,
        &'a [&'a str],
        Option<&'a AgentStartCommand>,
        Option<&'a str>,
    );
    let rows: [Row; 13] = [
        // Managed PowerShell: Windows PowerShell and PowerShell 7, bare or with plain flags,
        // including the login flags the Host completes with the managed startup itself.
        (
            r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe",
            &[],
            Some(&claude),
            Some(&claude_line),
        ),
        (
            "pwsh.exe",
            &["-NoLogo", "-NoProfile"],
            Some(&openclaw),
            Some(&openclaw_line),
        ),
        ("PWSH", &["-NoExit"], Some(&codex), Some(&codex_line)),
        ("pwsh", &["-l"], Some(&codex), Some(&codex_line)),
        (
            r"C:\Program Files\PowerShell\7\pwsh.exe",
            &["-NoLogo", "-NoProfile", "-NoExit", "-Login"],
            Some(&codex),
            Some(&codex_line),
        ),
        // No start command asked.
        ("powershell.exe", &[], None, None),
        // cmd and custom launch profiles that do not run the managed startup.
        (r"C:\Windows\System32\cmd.exe", &[], Some(&codex), None),
        ("cmd.exe", &["/k", "echo"], Some(&codex), None),
        ("pwsh.exe", &["-File", "start.ps1"], Some(&codex), None),
        ("pwsh.exe", &["-Command", "literal"], Some(&codex), None),
        ("pwsh.exe", &["-NonInteractive"], Some(&codex), None),
        (
            r"C:\Program Files\Git\bin\bash.exe",
            &["--login"],
            Some(&codex),
            None,
        ),
        ("nu.exe", &[], Some(&codex), None),
    ];
    for (executable, arguments, command, expected) in rows {
        let prepared = interactive_arguments(
            executable,
            arguments
                .iter()
                .map(|argument| (*argument).to_owned())
                .collect(),
        );
        assert_eq!(
            agent_start(executable, &prepared, command).as_deref(),
            expected,
            "{executable} {arguments:?}"
        );
    }
}

#[test]
fn actual_powershell_startup_runs_the_agent_once_and_keeps_the_shell() {
    let initialization = windows_interactive_powershell_initialization();
    let check = "if ($env:ORCHBEE_AGENT_START) { throw 'start variable kept' }; if (-not $env:ORCHBEE_RUN_TOKEN -or $env:ORCHBEE_RUN_TOKEN -ne $global:__OrchbeeRunId) { throw 'run not marked' }";
    for shell in [
        PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe"),
        PathBuf::from(std::env::var_os("ProgramFiles").unwrap()).join("PowerShell/7/pwsh.exe"),
    ] {
        if !shell.is_file() {
            continue;
        }
        for (program, arguments, expected) in [
            ("where.exe", vec!["powershell".to_owned()], "powershell.exe"),
            (
                "orchbee-agent-start-absent",
                vec![],
                "start-missing-kept-shell",
            ),
        ] {
            // The missing command's error ends the startup script, as it ends the
            // interactive startup before the first prompt; `catch` stands in for that prompt.
            let script = format!(
                "try {{ {initialization} }} catch {{ 'start-missing-kept-shell' }}; {check}; 'start-checked'"
            );
            let output = Command::new(&shell)
                .args([
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    &script,
                ])
                .env(
                    orchbee_local_runtime::WINDOWS_AGENT_START_VARIABLE,
                    windows_agent_start_command(program, &arguments).unwrap(),
                )
                .env_remove("ORCHBEE_RUN_TOKEN")
                .creation_flags(0x0800_0000)
                .output()
                .unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            let context = format!(
                "{} {program}: {stdout} {}",
                shell.display(),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.status.success(), "{context}");
            let marked = stdout
                .split_once("\u{1b}_orchbee.command.v1;start;")
                .map(|(_, rest)| rest);
            assert!(
                marked.is_some_and(|rest| rest.len() > 34
                    && rest[..32].bytes().all(|byte| byte.is_ascii_hexdigit())
                    && rest[32..].starts_with("\u{1b}\\")),
                "{context}"
            );
            assert!(stdout.to_ascii_lowercase().contains(expected), "{context}");
            assert!(stdout.contains("start-checked"), "{context}");
        }
    }
}
