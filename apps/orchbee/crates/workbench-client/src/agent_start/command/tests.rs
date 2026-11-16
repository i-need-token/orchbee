use super::*;
#[cfg(windows)]
#[path = "tests/windows_argv.rs"]
mod windows_argv;

#[test]
fn launch_checks_provider_input_and_typed_command_budget() {
    for shell in [Shell::Cmd, Shell::Posix] {
        assert!(launch("codex", &["a\nb".into()], None, shell).is_none());
        assert!(launch("codex", &["x".repeat(10000)], None, shell).is_none());
        assert!(launch("unknown", &[], None, shell).is_none());
    }
}

/// The run prefix is exactly the form each managed shell strips (`orchbee.zsh` and `orchbee.bash`
/// match `: orchbee-run:` and 32 lowercase hex digits followed by `; `), in front of the words
/// that start the provider; cmd takes none, and no token leaves the line as it is.
#[test]
fn a_typed_line_carries_the_prefix_its_shell_strips() {
    let token = AgentRunToken::parse("0123456789abcdef0123456789abcdef").unwrap();
    let words = launch(
        "openclaw",
        &["--model".into(), "two words".into()],
        None,
        Shell::Posix,
    )
    .unwrap();
    let literal = ["chat", "--model", "two words"].map(str::to_owned);
    assert_eq!(
        Some(words.as_str()),
        orchbee_local_runtime::posix_command_line("openclaw", &literal).as_deref()
    );
    let line = typed_line(&words, Shell::Posix, Some(&token));
    assert_eq!(
        line,
        format!(": orchbee-run:0123456789abcdef0123456789abcdef; {words}")
    );
    assert_eq!(
        typed_line("x", Shell::Powershell, Some(&token)),
        "<#orchbee-run:0123456789abcdef0123456789abcdef#> x"
    );
    assert_eq!(typed_line("x", Shell::Cmd, Some(&token)), "x");
    assert_eq!(typed_line("x", Shell::Posix, None), "x");
    assert!(interactive("openclaw", Shell::Posix, None).is_none());
}

#[test]
fn argument_free_interactive_launch_is_short_and_excludes_powershell_aliases() {
    assert_eq!(
        interactive("codex", Shell::Cmd, None).as_deref(),
        Some("codex")
    );
    let text = interactive("claude-code", Shell::Powershell, None).unwrap();
    assert!(text.len() < 160);
    assert!(text.contains("'claude'"));
    assert!(text.contains("-CommandType Application,ExternalScript"));
    assert!(!text.contains("EncodedCommand"));
    assert!(interactive("unknown", Shell::Powershell, None).is_none());
    // A provider whose bare command is not a chat starts with its fixed subcommand.
    assert_eq!(
        interactive("openclaw", Shell::Cmd, None).as_deref(),
        Some("openclaw chat")
    );
    let text = interactive("openclaw", Shell::Powershell, None).unwrap();
    assert!(text.starts_with("& (Get-Command 'openclaw' "));
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
        for shell in [Shell::Cmd, Shell::Posix] {
            assert!(launch(provider, &caller, Some(&first), shell).is_some());
        }
        for powershell in [false, true] {
            let shell = if powershell {
                Shell::Powershell
            } else {
                Shell::Cmd
            };
            let text = interactive(provider, shell, Some(&first)).unwrap();
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
        "openclaw",
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
        interactive("openclaw", Shell::Cmd, Some("x")).as_deref(),
        Some("openclaw chat")
    );
}

#[test]
fn the_budget_check_reserves_room_for_the_id() {
    let session = launch_session("grok", &[]).unwrap();
    let mut long = "x".repeat(1000);
    // Grow the argument until the line with an id no longer fits, then check the budget agrees.
    while launch(
        "grok",
        std::slice::from_ref(&long),
        Some(&session),
        Shell::NATIVE,
    )
    .is_some()
    {
        long.push('x');
    }
    assert!(launch("grok", std::slice::from_ref(&long), None, Shell::NATIVE).is_some());
    assert!(!fits("grok", std::slice::from_ref(&long)));
}
