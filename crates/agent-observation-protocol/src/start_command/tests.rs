//! The start command each startable provider runs, and admission by exact canonical id only.
use super::{STARTABLE_AGENT_PROVIDER_IDS, StartableAgent};

#[test]
fn a_start_runs_the_catalog_command_with_its_fixed_arguments_and_launch_session() {
    const ID: &str = "0b5d2c8e-3f41-4a6b-9c7d-1e2f3a4b5c6d";
    // (provider, the argv a start without caller arguments runs, `{id}` standing for the
    // fresh conversation id)
    let rows: &[(&str, &[&str])] = &[
        ("claude-code", &["claude"]),
        ("codex", &["codex"]),
        ("cursor", &["agent"]),
        ("qodercli", &["qoder"]),
        ("antigravity", &["agy"]),
        ("kiro", &["kiro-cli"]),
        ("minimax-code", &["mcode"]),
        ("openclaw", &["openclaw", "chat"]),
        ("grok", &["grok", "--session-id", "{id}"]),
        ("gemini", &["gemini", "--session-id", "{id}"]),
        ("qwen-code", &["qwen", "--session-id", "{id}"]),
        ("github-copilot", &["copilot", "--session-id", "{id}"]),
        ("pi", &["pi", "--session-id", "{id}"]),
        ("omp", &["omp"]),
    ];
    for (provider, want) in rows {
        let agent = StartableAgent::parse(provider).unwrap();
        let argv: Vec<String> = std::iter::once(agent.executable().to_owned())
            .chain(agent.start_arguments(Some(ID)))
            .collect();
        let want: Vec<String> = want.iter().map(|a| a.replace("{id}", ID)).collect();
        assert_eq!(argv, want, "{provider}");
        // Without a fresh id nothing names a conversation.
        assert!(!agent.start_arguments(None).iter().any(|a| a == ID));
    }
}

#[test]
fn only_an_exact_canonical_id_is_startable() {
    for id in STARTABLE_AGENT_PROVIDER_IDS {
        assert_eq!(StartableAgent::parse(id).map(StartableAgent::id), Some(*id));
    }
    for label in ["claude", "Claude Code", "CODEX", "copilot", "", "codex "] {
        assert_eq!(StartableAgent::parse(label), None, "{label:?}");
    }
}
