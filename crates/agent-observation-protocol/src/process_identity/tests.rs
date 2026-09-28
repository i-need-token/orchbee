use super::*;
fn p(
    pid: u32,
    parent: Option<u32>,
    provider: Option<AgentReportProvider>,
) -> AgentProcessCandidate {
    AgentProcessCandidate {
        pid,
        parent_pid: parent,
        creation_time: u64::from(pid),
        provider,
    }
}
#[test]
fn selection_requires_one_valid_outer_chain_and_creation_order() {
    let mut rows = vec![
        p(1, None, None),
        p(2, Some(1), Some(AgentReportProvider::Codex)),
        p(3, Some(2), Some(AgentReportProvider::Claude)),
    ];
    assert_eq!(
        select_agent_process(&rows),
        AgentProcessSelection::Selected {
            pid: 2,
            creation_time: 2,
            provider: AgentReportProvider::Codex
        }
    );
    rows[2].parent_pid = Some(1);
    assert_eq!(
        select_agent_process(&rows),
        AgentProcessSelection::Ambiguous
    );
    rows[2].parent_pid = Some(2);
    rows[1].creation_time = 4;
    assert_eq!(
        select_agent_process(&rows),
        AgentProcessSelection::Ambiguous
    );
    rows[1].creation_time = 3;
    rows[1].parent_pid = Some(3);
    assert_eq!(
        select_agent_process(&rows),
        AgentProcessSelection::Ambiguous
    );
    rows.truncate(1);
    assert_eq!(select_agent_process(&rows), AgentProcessSelection::None);
    rows.push(rows[0]);
    assert_eq!(
        select_agent_process(&rows),
        AgentProcessSelection::Ambiguous
    );
    assert_eq!(select_agent_process(&[]), AgentProcessSelection::None);
}
#[test]
fn only_actual_image_and_supported_entrypoint_establish_provider() {
    for (image, argv, want) in [
        (
            "node.exe",
            vec![
                "node.exe",
                "C:/node_modules/@anthropic-ai/claude-code/cli.js",
            ],
            Some("claude-code"),
        ),
        (
            "node.exe",
            vec!["node.exe", "--require", "codex.js", "ordinary.js"],
            None,
        ),
        ("node.exe", vec!["codex.exe", "codex.js"], None),
        ("node.exe", vec!["node.exe", "-e", "codex.js"], None),
        ("cmd.exe", vec!["cmd.exe", "/c", "codex.exe"], None),
        ("letta.exe", vec!["letta.exe", "--print=task"], None),
        ("C:/tools/CODEX.EXE", vec!["forged.exe"], Some("codex")),
        (
            "pwsh.exe",
            vec!["pwsh.exe", "-NoProfile", "-File", "C:/tools/codex.ps1"],
            Some("codex"),
        ),
    ] {
        let argv: Vec<String> = argv.into_iter().map(str::to_owned).collect();
        assert_eq!(
            identify_agent_process(Some(image), Some(&argv)),
            want,
            "{image}: {argv:?}"
        );
    }
    assert_eq!(
        identify_agent_process(None, Some(&["codex.exe".into()])),
        None
    );
}
#[test]
fn process_preprocessing_preserves_distinction_from_report_labels() {
    assert_eq!(native_agent_provider("C:/tools/agent.exe"), Some("cursor"));
    assert_eq!(native_agent_provider("codex.mjs"), Some("codex"));
    assert_eq!(native_agent_provider("codex.bat"), None);
    assert_eq!(
        crate::normalize_agent_report_label("agent").as_deref(),
        Some("agent")
    );
    assert_eq!(
        crate::normalize_agent_report_label("codex.mjs").as_deref(),
        Some("codex.mjs")
    );
}
