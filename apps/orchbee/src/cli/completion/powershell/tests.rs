//! Actual PowerShell completion exercises grammar scope without a Site.
use super::render;
use std::{
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn real_powershell_completes_scoped_grammar_without_runtime() {
    let mut cases = super::super::tests::cases::CASES.to_vec();
    cases.extend([
        ("orchbee config import-herdr --se", "--session", false),
        ("orchbee --session work config im", "import-herdr", false),
        ("orchbee --machine work config im", "import-herdr", false),
        ("orchbee --ma", "--machine", true),
        ("orchbee --machine work workspace l", "list", true),
        ("orchbee --machine=work workspace l", "list", true),
        ("orchbee --machine work ma", "machine", false),
        ("orchbee --machine work tu", "tui", false),
        ("orchbee --machine work --se", "--session", false),
        ("orchbee --machine work status c", "client", false),
        ("orchbee --machine work status s", "server", true),
        ("orchbee workspace --ma", "--machine", false),
        ("orchbee --machine work terminal a", "attach", false),
        (
            "orchbee --machine work --session local workspace l",
            "list",
            false,
        ),
        ("orchbee terminal ti", "title", true),
        ("orchbee terminal title s", "set", true),
        ("orchbee help terminal title c", "clear", true),
        (
            "orchbee terminal title set 'Build title' --fo",
            "--format",
            true,
        ),
        ("orchbee terminal title clear --fo", "--format", true),
        ("orchbee terminal title clear extra --fo", "--format", false),
        ("orchbee terminal title set x --bo", "--body", false),
        ("orchbee notification sh", "show", true),
        ("orchbee help notification sh", "show", true),
        ("orchbee notification show hello --po", "--position", true),
        (
            "orchbee notification show hello --position b",
            "bottom-left",
            true,
        ),
        (
            "orchbee notification show hello --sound=",
            "--sound=done",
            true,
        ),
        (
            "orchbee notification show hello --sound nope --b",
            "--body",
            false,
        ),
        (
            "orchbee notification show hello --body literal --bo",
            "--body",
            false,
        ),
        ("orchbee server config --so", "--sound", false),
        ("orchbee panel create-terminal abc --cl", "--client", false),
        ("orchbee panel create-terminal abc --cu", "--current", false),
        (
            "orchbee panel create-terminal --client abc target --ch",
            "--child",
            false,
        ),
        ("orchbee panel create-t", "create-terminal", true),
        ("orchbee panel create-terminal --ch", "--child", true),
        (
            "orchbee panel create-terminal --child --p",
            "--presentation",
            false,
        ),
        (
            "orchbee panel create-terminal --presentation 00000000000000000000000000000009 --ch",
            "--child",
            false,
        ),
        ("orchbee panel create-terminal --cwd w", "workspace", true),
        (
            "orchbee panel create-terminal --profile shell --e",
            "--exec",
            false,
        ),
        ("orchbee server agent-m", "agent-manifests", true),
        (
            "orchbee server reload-agent-manifests --r",
            "--request-id",
            true,
        ),
        ("orchbee server agent-manifests --r", "--request-id", false),
        (
            "orchbee server agent-manifests-status abc --f",
            "--format",
            true,
        ),
        (
            "orchbee panel report-agent abc --state done --m",
            "--message",
            false,
        ),
        ("orchbee panel report-a", "report-agent-session", true),
        ("orchbee panel clear-a", "clear-agent-authority", true),
        (
            "orchbee panel report-agent abc --source custom --a",
            "--agent",
            true,
        ),
        (
            "orchbee panel report-agent abc --source 'custom source' --s",
            "--state",
            true,
        ),
        ("orchbee panel report-agent abc --source ", "visible", false),
        (
            "orchbee panel report-agent abc --source=",
            "--source=visible",
            false,
        ),
        ("orchbee panel report-agent abc --state b", "blocked", true),
        (
            "orchbee panel report-agent abc --state=",
            "--state=working",
            true,
        ),
        (
            "orchbee panel report-agent abc --state working --s",
            "--state",
            false,
        ),
        (
            "orchbee panel report-agent-session abc --s",
            "--state",
            false,
        ),
        (
            "orchbee panel report-agent-session abc --s",
            "--session-start-source",
            true,
        ),
        (
            "orchbee panel clear-agent-authority abc --source '' --s",
            "--seq",
            true,
        ),
        ("orchbee panel release-agent abc --m", "--message", false),
        (
            "orchbee panel report-metadata abc --source hook --t",
            "--title",
            true,
        ),
        ("orchbee workspace mo", "move-block", true),
        ("orchbee workspace move alpha --i", "--insert-index", true),
        (
            "orchbee workspace move-block alpha beta --b",
            "--before",
            true,
        ),
        (
            "orchbee workspace move-block alpha --before=beta --b",
            "--before",
            false,
        ),
        ("orchbee workspace move alpha --b", "--before", false),
        ("orchbee workspace list --b", "--before", false),
        ("orchbee --se", "--session", true),
        ("orchbee --session work sta", "status", true),
        ("orchbee --session=work tui --m", "--mouse", true),
        ("orchbee --session=work tui --s", "--session", false),
        ("orchbee --session=.. status --f", "--format", false),
        (
            "orchbee --session=work --session=other status --f",
            "--format",
            false,
        ),
        ("orchbee --session work help terminal a", "attach", true),
        ("orchbee --session work completion p", "powershell", true),
        ("orchbee --session ", "status", false),
        (
            "orchbee agent get --agent-session abc --se",
            "--session",
            true,
        ),
        (
            "orchbee --session work agent get --ag",
            "--agent-session",
            true,
        ),
        (
            "orchbee --session work agent get --agent-session abc --se",
            "--session",
            false,
        ),
        ("orchbee --session work server st", "start", true),
        ("orchbee help ser", "server", true),
        ("orchbee si", "site", false),
        ("orchbee --site work status --f", "--format", false),
        (
            "orchbee tab move alpha --insert-index 2 --f",
            "--format",
            true,
        ),
        (
            "orchbee tab move alpha --insert-index $x --f",
            "--format",
            false,
        ),
        (
            "orchbee tab move alpha --insert-index (1+1) --f",
            "--format",
            false,
        ),
        (
            "orchbee tab move alpha --insert-index {2} --f",
            "--format",
            false,
        ),
    ]);
    let json = serde_json::to_string(&cases).expect("cases");
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
