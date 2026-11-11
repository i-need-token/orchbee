use super::*;
#[cfg(windows)]
#[path = "tests/windows_argv.rs"]
mod windows_argv;

#[test]
fn launch_checks_provider_input_and_typed_command_budget() {
    assert!(launch("codex", &["a\nb".into()], None).is_none());
    assert!(launch("codex", &["x".repeat(10000)], None).is_none());
    assert!(launch("unknown", &[], None).is_none());
}

#[test]
fn argument_free_interactive_launch_is_short_and_excludes_powershell_aliases() {
    assert_eq!(interactive("codex", false, None).as_deref(), Some("codex"));
    let text = interactive("claude-code", true, None).unwrap();
    assert!(text.len() < 160);
    assert!(text.contains("'claude'"));
    assert!(text.contains("-CommandType Application,ExternalScript"));
    assert!(!text.contains("EncodedCommand"));
    assert!(interactive("unknown", true, None).is_none());
    // A provider whose bare command is not a chat starts with its fixed subcommand.
    assert_eq!(
        interactive("qwenpaw", false, None).as_deref(),
        Some("qwenpaw tui")
    );
    let text = interactive("kirocrew", true, None).unwrap();
    assert!(text.starts_with("& (Get-Command 'kirocrew' "));
    assert!(text.ends_with(") chat"));
}

/// A hyphenated version-4 UUID in lowercase.
fn uuid_v4(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => matches!(byte, b'0'..=b'9' | b'a'..=b'f'),
        })
        && id.as_bytes()[14] == b'4'
        && matches!(id.as_bytes()[19], b'8' | b'9' | b'a' | b'b')
}

/// The ids bound to `--session-id` in one argument list.
fn bound(arguments: &[String]) -> Vec<&str> {
    arguments
        .windows(2)
        .filter(|pair| pair[0] == "--session-id")
        .map(|pair| pair[1].as_str())
        .collect()
}

#[test]
fn a_start_gives_exactly_one_fresh_id_only_where_the_caller_chose_no_conversation() {
    let args = |line: &str| -> Vec<String> { line.split_whitespace().map(str::to_owned).collect() };
    for provider in ["grok", "gemini", "qwen-code", "github-copilot", "pi"] {
        let first = launch_session(provider, &[]).unwrap();
        assert!(uuid_v4(&first), "{provider}: {first}");
        assert_ne!(
            launch_session(provider, &[]).as_deref(),
            Some(first.as_str())
        );
        // Both launch forms carry exactly one valid id, ahead of the caller's arguments.
        let caller = args("--model m");
        let line = arguments(provider, &caller, Some(&first));
        assert_eq!(bound(&line), [first.as_str()], "{provider}: {line:?}");
        assert!(line.ends_with(&caller), "{provider}: {line:?}");
        assert!(launch(provider, &caller, Some(&first)).is_some());
        for powershell in [false, true] {
            let text = interactive(provider, powershell, Some(&first)).unwrap();
            let quoted = |token: &str| {
                if powershell {
                    format!("'{token}'")
                } else {
                    token.to_owned()
                }
            };
            let tail = format!(" {} {}", quoted("--session-id"), quoted(&first));
            assert!(text.ends_with(&tail), "{provider}: {text}");
            assert_eq!(text.matches(&first).count(), 1, "{provider}: {text}");
        }
        // A caller's own conversation choice suppresses the id, `=` form included; an option
        // word after `--` is not an option.
        for own in [
            "--session-id 0f8fad5b-d9cb-469f-a165-70867728950e",
            "--session-id=0f8fad5b-d9cb-469f-a165-70867728950e",
            "--resume",
        ] {
            assert_eq!(
                launch_session(provider, &args(own)),
                None,
                "{provider}: {own}"
            );
        }
        assert!(launch_session(provider, &args("-- --resume")).is_some());
        assert!(launch_session(provider, &args("--resumes")).is_some());
    }
    // Every conversation option the caller may write for each provider.
    for (provider, own) in [
        ("grok", "-s x"),
        ("grok", "-c"),
        ("grok", "--continue"),
        ("grok", "--fork-session"),
        ("grok", "--load x"),
        ("grok", "-r"),
        ("gemini", "-r latest"),
        ("qwen-code", "-c"),
        ("qwen-code", "--continue"),
        ("github-copilot", "--continue"),
        ("github-copilot", "-r"),
        ("pi", "--session x"),
        ("pi", "--no-session"),
        ("pi", "-c"),
    ] {
        assert_eq!(
            launch_session(provider, &args(own)),
            None,
            "{provider}: {own}"
        );
    }
    // Providers that take no id at launch get none, and a missing id adds nothing.
    for provider in [
        "claude-code",
        "codex",
        "omp",
        "cursor",
        "kirocrew",
        "unknown",
    ] {
        assert_eq!(launch_session(provider, &[]), None, "{provider}");
        assert!(
            bound(&arguments(provider, &[], Some("x"))).is_empty(),
            "{provider}"
        );
    }
    assert!(bound(&arguments("grok", &[], None)).is_empty());
    assert_eq!(
        interactive("kirocrew", false, Some("x")).as_deref(),
        Some("kirocrew chat")
    );
}

#[test]
fn the_budget_check_reserves_room_for_the_id() {
    let session = launch_session("grok", &[]).unwrap();
    let mut long = "x".repeat(1000);
    // Grow the argument until the line with an id no longer fits, then check the budget agrees.
    while launch("grok", std::slice::from_ref(&long), Some(&session)).is_some() {
        long.push('x');
    }
    assert!(launch("grok", std::slice::from_ref(&long), None).is_some());
    assert!(!fits("grok", std::slice::from_ref(&long)));
}
