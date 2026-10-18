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
