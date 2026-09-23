use super::*;
use orchbee_terminal_protocol::{AttachmentId, TerminalId};

fn process(pid: u32, parent: Option<u32>, image: &str, command: &str) -> TerminalProcess {
    TerminalProcess {
        pid,
        parent_pid: parent,
        creation_time: u64::from(pid),
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

#[test]
fn executable_aliases_are_explicit_and_canonical() {
    for (name, expected) in [
        ("pi", "pi"),
        ("claude", "claude-code"),
        ("codex", "codex"),
        ("gemini", "gemini"),
        ("cursor-agent", "cursor"),
        ("devin-cli", "devin"),
        ("agy", "antigravity"),
        ("cline", "cline"),
        ("omp", "omp"),
        ("mastra-code", "mastracode"),
        ("opencode2", "opencode"),
        ("copilot", "github-copilot"),
        ("kimi-code", "kimi"),
        ("kiro-cli", "kiro"),
        ("droid", "droid"),
        ("amp-local", "amp"),
        ("grok-build", "grok"),
        ("hermes-agent", "hermes"),
        ("kilo-code", "kilo"),
        ("qoderclicn", "qodercli"),
        ("qwen", "qwen-code"),
        ("letta", "letta"),
        ("maki", "maki"),
        ("muse-bin-0.1.0-R708.1", "muse"),
    ] {
        assert_eq!(
            provider(&format!("C:\\工具\\{}.EXE", name.to_uppercase())),
            Some(expected)
        );
    }
    for name in [
        "not-codex",
        "muse-binary",
        "prefixclaude",
        "acme.agent",
        "codex.exe.txt",
    ] {
        assert_eq!(provider(name), None);
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
            r#"python3.12.exe -u C:\bin\hermes.py"#,
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
        ("pwsh.exe", r#"pwsh.exe -Command codex.exe"#, None),
        ("bash.exe", r#"bash.exe -c codex"#, None),
        ("letta.exe", r#"letta.exe --print task"#, None),
    ] {
        let argv = arguments(command).unwrap();
        assert_eq!(
            identity::identify(Some(image), Some(&argv)),
            expected,
            "{command}"
        );
    }
    assert_eq!(identity::identify(None, Some(&["codex.exe".into()])), None);
}
