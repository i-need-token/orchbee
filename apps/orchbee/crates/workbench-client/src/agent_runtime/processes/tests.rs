use super::*;
use orchbee_terminal_protocol::{AttachmentId, TerminalId};

fn process(pid: u32, parent: Option<u32>, image: &str, command: &str) -> TerminalProcess {
    TerminalProcess {
        pid,
        parent_pid: parent,
        creation_time: u64::from(pid),
        console: orchbee_terminal_protocol::TerminalProcessConsole::Attached,
        image: Some(image.into()),
        command_line: Some(command.into()),
        current_directory: None,
    }
}

fn sample(processes: Vec<TerminalProcess>) -> TerminalProcessInfo {
    TerminalProcessInfo {
        attachment_id: AttachmentId::from_bytes([1; 16]).unwrap(),
        terminal_id: TerminalId::from_bytes([2; 16]).unwrap(),
        root_pid: 10,
        processes,
    }
}

#[test]
fn agent_chain_selection_has_explicit_ambiguity_and_root_absence() {
    let shell = process(10, None, "C:\\Windows\\cmd.exe", "cmd.exe");
    let outer = process(20, Some(10), "C:\\bin\\codex.exe", "codex.exe");
    let inner = process(30, Some(20), "C:\\bin\\claude.exe", "claude.exe");
    let mut facts = sample(vec![shell, outer, inner]);
    let projection = interpret(&facts);
    assert_eq!(projection.foreground.selection, Selection::AgentChain);
    assert_eq!(projection.foreground.pid, Some(20));
    assert_eq!(projection.foreground.provider, Some("codex"));
    facts.processes[2].parent_pid = Some(10);
    assert_eq!(interpret(&facts).foreground.selection, Selection::Ambiguous);
    facts.processes.truncate(1);
    assert_eq!(interpret(&facts).foreground.selection, Selection::Root);
    facts.processes.clear();
    assert_eq!(
        interpret(&facts).foreground.selection,
        Selection::Unavailable
    );
    facts
        .processes
        .push(process(25, Some(21), "codex.exe", "codex.exe"));
    assert_eq!(
        interpret(&facts).foreground.selection,
        Selection::AgentChain
    );
    // An opaque member inside the anchor never holds an outer Agent; one outside leaves the
    // identity unresolved, which the Server reports, while the named chain stays.
    let mut opaque = process(26, Some(25), "", "");
    (opaque.image, opaque.command_line) = (None, None);
    facts.processes.push(opaque);
    assert_eq!(interpret(&facts).foreground.pid, Some(25));
    facts.processes[1].parent_pid = Some(99);
    assert_eq!(interpret(&facts).foreground.pid, Some(25));
    // Without a candidate, an opaque member is not a foreground Agent.
    facts.processes.remove(0);
    assert_eq!(
        interpret(&facts).foreground.selection,
        Selection::Unavailable
    );
}

#[test]
fn cycles_and_reused_parent_pids_never_select_an_agent_chain() {
    let mut facts = sample(vec![
        process(20, Some(30), "codex.exe", "codex"),
        process(30, Some(20), "claude.exe", "claude"),
    ]);
    facts.processes[1].creation_time = 20;
    assert_eq!(interpret(&facts).foreground.selection, Selection::Ambiguous);
    facts.processes[0].parent_pid = None;
    facts.processes[0].creation_time = 40;
    assert_eq!(interpret(&facts).foreground.selection, Selection::Ambiguous);
    facts.processes[0].creation_time = 10;
    assert_eq!(interpret(&facts).foreground.pid, Some(20));
}

#[cfg(windows)]
#[test]
fn unlisted_arguments_stay_candidates_and_noninteractive_forms_are_excluded() {
    let shell = process(10, None, r"C:\Windows\cmd.exe", "cmd.exe");
    let agent = |command| process(20, Some(10), r"C:\bin\claude.exe", command);
    let facts = sample(vec![shell.clone(), agent("claude --unlisted-flag")]);
    let unlisted = interpret(&facts);
    assert_eq!(unlisted.foreground.selection, Selection::AgentChain);
    assert_eq!(unlisted.foreground.provider, Some("claude-code"));
    for command in ["claude -p task", "claude --version"] {
        let facts = sample(vec![shell.clone(), agent(command)]);
        let excluded = interpret(&facts);
        assert_eq!(excluded.foreground.selection, Selection::Root, "{command}");
        assert_eq!(excluded.foreground.provider, None, "{command}");
    }
}

#[cfg(windows)]
#[test]
fn windows_argument_projection_preserves_quotes_empty_values_and_unicode() {
    assert_eq!(
        arguments(r#""C:\Program Files\node.exe" "C:\工具\codex.js" "a b" "" "中 😀""#).unwrap(),
        [
            r"C:\Program Files\node.exe",
            r"C:\工具\codex.js",
            "a b",
            "",
            "中 😀"
        ]
    );
    assert_eq!(
        arguments(r#"codex.exe "a\"b" "C:\end\\""#).unwrap(),
        ["codex.exe", "a\"b", r"C:\end\"]
    );
    for value in ["", " \t", "codex\0.exe", &"a".repeat(4097)] {
        assert!(arguments(value).is_none());
    }
}

#[cfg(windows)]
#[test]
fn runtime_entrypoints_exclude_eval_flags_prompt_mentions_and_forged_argv0() {
    for (image, command, expected) in [
        (
            "node.exe",
            r#"node.exe "C:\工具\codex.js" "task""#,
            Some("codex"),
        ),
        (
            "node.exe",
            r#"node.exe --require helper.js C:\node_modules\@anthropic-ai\claude-code\cli.js"#,
            Some("claude-code"),
        ),
        (
            "bun.exe",
            r#"bun.exe C:\node_modules\@qwen-code\qwen-code\dist\index.js"#,
            Some("qwen-code"),
        ),
        (
            "python3.12.exe",
            r#"python3.12.exe -u C:\uv\tools\hermes-agent\Scripts\hermes.py"#,
            Some("hermes"),
        ),
        (
            "pwsh.exe",
            r#"pwsh.exe -NoProfile -File C:\bin\codex.ps1"#,
            Some("codex"),
        ),
        ("node.exe", r#"node.exe -e "codex" codex.js"#, None),
        ("node.exe", r#"node.exe --eval=codex codex.js"#, None),
        (
            "node.exe",
            r#"node.exe --require codex.js unrelated.js"#,
            None,
        ),
        ("node.exe", r#"node.exe unrelated.js codex"#, None),
        ("node.exe", r#"codex.exe C:\bin\codex.js"#, None),
        ("cmd.exe", r#"cmd.exe /c echo codex.exe"#, None),
        ("pwsh.exe", r#"pwsh.exe -Command codex.exe"#, Some("codex")),
        ("pwsh.exe", r#"pwsh.exe -Command "codex.exe; rm x""#, None),
        (r"C:\bin\agent.exe", r#"agent.exe"#, None),
        ("bash.exe", r#"bash.exe -c codex"#, None),
        ("letta.exe", r#"letta.exe --print task"#, None),
    ] {
        let argv = arguments(command).unwrap();
        assert_eq!(
            classify_agent_process(Some(image), Some(&argv))
                .interactive_provider()
                .map(AgentReportProvider::native_provider_id),
            expected,
            "{command}"
        );
    }
}

#[test]
fn filetime_renders_as_utc_and_keeps_pre_epoch_values_raw() {
    assert_eq!(
        filetime_utc(116_444_736_000_000_000),
        "1970-01-01 00:00:00 UTC"
    );
    assert_eq!(
        filetime_utc(134_368_638_784_289_377),
        "2026-10-19 06:11:18 UTC"
    );
    assert_eq!(filetime_utc(5), "5");
}

#[test]
fn readable_commands_drop_the_injected_shell_initialization() {
    let script = orchbee_local_runtime::windows_interactive_powershell_initialization();
    let shell = process(
        10,
        None,
        r"C:\Program Files\PowerShell\pwsh.exe",
        "pwsh.exe",
    );
    let wrapped = Process {
        facts: &shell,
        argv: Some(vec![
            r"C:\Program Files\PowerShell\pwsh.exe".into(),
            "-NoLogo".into(),
            "-NoExit".into(),
            "-Command".into(),
            script,
        ]),
        provider: None,
    };
    assert_eq!(
        readable_command(&wrapped).unwrap(),
        "pwsh.exe -NoLogo -NoExit"
    );
    let explicit = Process {
        facts: &shell,
        argv: Some(vec![
            "pwsh.exe".into(),
            "-Command".into(),
            "Get-Date; exit".into(),
        ]),
        provider: None,
    };
    assert_eq!(
        readable_command(&explicit).unwrap(),
        "pwsh.exe -Command \"Get-Date; exit\""
    );
    assert_eq!(program_name(&shell), "pwsh.exe");
}
