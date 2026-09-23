//! Actual PowerShell completion exercises grammar scope without a Site.
use super::render;
use std::{
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn real_powershell_completes_scoped_grammar_without_runtime() {
    let cases = [
        ("orchbee workspace create --c", "--cwd", true),
        ("orchbee workspace create --cwd C:\\code --e", "--env", true),
        (
            "orchbee workspace create --cwd C:\\code --profile dev --e",
            "--exec",
            false,
        ),
        (
            "orchbee workspace create --cwd C:\\code --cell-",
            "--cell-width",
            true,
        ),
        ("orchbee panel ", "current", true),
        ("orchbee help panel ", "current", true),
        ("orchbee agent ", "rename", true),
        ("orchbee agent rename a alpha --f", "--format", true),
        ("orchbee agent rename a alpha --c", "--clear", false),
        ("orchbee agent rename a --c", "--clear", true),
        ("orchbee agent rename --name a alpha --f", "--format", true),
        ("orchbee agent resolve --name a --s", "--session", false),
        ("orchbee agent resolve a --n", "--name", false),
        ("orchbee panel current --p", "--panel", true),
        ("orchbee panel current -- ", "--panel", false),
        (
            "orchbee panel current --panel=@panel:ab --c",
            "--current",
            false,
        ),
        ("orchbee panel current --current --p", "--panel", false),
        (
            "orchbee panel current --panel p --current --f",
            "--format",
            false,
        ),
        ("orchbee panel get p --c", "--current", false),
        ("orchbee --current panel ", "current", true),
        ("orchbee tab create --profile dev --e", "--env", true),
        ("orchbee tab create --env A=b --e", "--env", true),
        ("orchbee panel split p --env A=b --c", "--cwd", true),
        ("orchbee tab create --cwd C:\\code --c", "--cwd", false),
        ("orchbee launch-profile create --e", "--env", false),
        ("orchbee status --c", "--cwd", false),
        ("orchbee ", "--skill", true),
        ("orchbee completion ", "powershell", true),
        ("orchbee help agent integration ", "enable", true),
        ("orchbee terminal ", "send", true),
        ("orchbee terminal send terminal-1 --p", "--paste", true),
        (
            "orchbee terminal send terminal-1 --key enter --k",
            "--key",
            true,
        ),
        (
            "orchbee terminal send terminal-1 --key enter --s",
            "--stdin",
            false,
        ),
        (
            "orchbee terminal send terminal-1 --stdin --p",
            "--paste",
            true,
        ),
        (
            "orchbee terminal send terminal-1 --text 'two words' --p",
            "--paste",
            false,
        ),
        ("orchbee terminal read terminal-1 --p", "--paste", false),
        ("orchbee tab create --standalone --w", "--workspace", false),
        (
            "orchbee panel split panel-1 --direction=",
            "--direction=left",
            true,
        ),
        ("orchbee tab move tab-1 --direction ", "left", false),
        ("orchbee tab move tab-1 --direction ", "next", true),
        ("orchbee --format json agent ", "list", true),
        ("orchbee --format json ", "tui", false),
        (
            "orchbee agent integration enable --provider ",
            "codex",
            true,
        ),
        ("orchbee --mouse off ", "tui", true),
        ("orchbee status --format=", "--format=json", true),
        ("orchbee STATUS --f", "--format", false),
        ("orchbee -- --s", "--skill", false),
        ("orchbee --skill --f", "--format", false),
        (
            "orchbee terminal send terminal-1 --file ",
            "--format",
            false,
        ),
        (
            "orchbee tab create --exec 'C:\\Program Files\\shell.exe' --a",
            "--arg",
            true,
        ),
        (
            "orchbee tab create --profile profile-1 --e",
            "--exec",
            false,
        ),
    ];
    let json = serde_json::to_string(cases.as_slice()).expect("cases");
    // Windows PowerShell 5.1 only invokes a native argument completer for
    // option prefixes after resolving an actual executable. Use this test
    // executable as a name-only target; completion never runs it.
    let executable = std::env::current_exe().expect("test executable");
    let name = executable
        .file_name()
        .expect("name")
        .to_str()
        .expect("Unicode name");
    let mut paths = vec![executable.parent().expect("directory").to_path_buf()];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let search_path = std::env::join_paths(paths).expect("PATH");
    for shell in ["powershell.exe", "pwsh.exe"] {
        let script = format!(
            "$ErrorActionPreference='Stop'\n[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)\n{}\n$cases=ConvertFrom-Json @'\n{json}\n'@\nforeach ($case in $cases) {{\n$line=$case[0]\n$matches=@([System.Management.Automation.CommandCompletion]::CompleteInput($line,$line.Length,$null).CompletionMatches | ForEach-Object {{ $_.CompletionText }})\nif (($matches -ccontains $case[1]) -ne $case[2]) {{ throw ('Case failed: '+$line+' expected '+$case[1]+'='+$case[2]+' got '+($matches -join ',')) }}\n}}\nWrite-Output 'completion-ok'\n",
            render()
        );
        let script = script
            .replace(
                "-CommandName orchbee, orchbee.exe",
                &format!("-CommandName orchbee, orchbee.exe, {name}"),
            )
            .replace(
                "$line=$case[0]",
                &format!("$line='{name} '+$case[0].Substring(8)"),
            );
        let mut child = match Command::new(shell)
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "& ([scriptblock]::Create([Console]::In.ReadToEnd()))",
            ])
            .env("PATH", &search_path)
            .env("ORCHBEE_RUNTIME_BASE", "invalid-relative-runtime")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(error) if shell == "pwsh.exe" && error.kind() == std::io::ErrorKind::NotFound => {
                continue;
            }
            Err(error) => panic!("{shell}: {error}"),
        };
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(script.as_bytes())
            .expect("script");
        let result = child.wait_with_output().expect("shell result");
        assert!(
            result.status.success()
                && result.stderr.is_empty()
                && String::from_utf8_lossy(&result.stdout).contains("completion-ok"),
            "{shell}: {} {}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
}
