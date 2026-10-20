use super::*;
use std::{os::windows::process::CommandExt, path::PathBuf, process::Command};

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
