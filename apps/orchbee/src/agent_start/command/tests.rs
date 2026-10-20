use super::*;
#[cfg(windows)]
#[path = "tests/windows_argv.rs"]
mod windows_argv;

#[test]
fn launch_checks_provider_input_and_typed_command_budget() {
    assert!(launch("codex", &["a\nb".into()]).is_none());
    assert!(launch("codex", &["x".repeat(10000)]).is_none());
    assert!(launch("unknown", &[]).is_none());
}

#[test]
fn argument_free_interactive_launch_is_short_and_excludes_powershell_aliases() {
    assert_eq!(interactive("codex", false).as_deref(), Some("codex"));
    let text = interactive("claude-code", true).unwrap();
    assert!(text.len() < 160);
    assert!(text.contains("'claude'"));
    assert!(text.contains("-CommandType Application,ExternalScript"));
    assert!(!text.contains("EncodedCommand"));
    assert!(interactive("unknown", true).is_none());
}
