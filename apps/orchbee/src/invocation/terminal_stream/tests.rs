use super::*;
#[test]
fn attach_targets_are_explicit_and_options_are_scoped_before_effects() {
    let parse = |words: &[&str]| parse_request(words.iter().map(OsString::from));
    let Request::TerminalAttach { command } = parse(&[
        "--mouse=off",
        "terminal",
        "attach",
        "@terminal:ab",
        "--takeover",
        "--timeout=50",
        "--format=json",
    ])
    .unwrap() else {
        panic!("attach")
    };
    assert!(!command.mouse);
    assert!(command.takeover);
    assert_eq!(command.timeout_ms, Some(50));
    assert!(matches!(command.target, AttachTarget::Terminal(_)));
    for words in [
        vec!["agent", "attach", "alpha"],
        vec!["agent", "attach", "--name=alpha"],
        vec!["agent", "attach", "--session=session-1"],
        vec!["agent", "attach", "--panel=@panel:ab"],
    ] {
        let Request::TerminalAttach { command } = parse(&words).unwrap() else {
            panic!("agent attach")
        };
        assert!(command.mouse);
        assert!(!command.takeover);
        assert_eq!(command.timeout_ms, None);
        assert!(matches!(command.target, AttachTarget::Agent(_)));
    }
    for words in [
        vec!["terminal", "attach"],
        vec!["terminal", "attach", "@terminal:ab", "extra"],
        vec!["terminal", "attach", "@terminal:ab", "--cols=80"],
        vec!["terminal", "attach", "@terminal:ab", "--mouse=invalid"],
        vec![
            "terminal",
            "attach",
            "@terminal:ab",
            "--mouse=on",
            "--mouse=off",
        ],
        vec!["terminal", "attach", "@terminal:ab", "--timeout=0"],
        vec!["terminal", "attach", "@terminal:ab", "--name=alpha"],
        vec!["agent", "attach"],
        vec!["agent", "attach", "alpha", "--name=alpha"],
        vec![
            "agent",
            "attach",
            "--panel=@panel:ab",
            "--session=session-1",
        ],
        vec!["agent", "attach", "--name=alpha", "--stdin"],
    ] {
        assert!(parse(&words).is_err(), "{words:?}");
    }
}

fn parse(extra: &[&str]) -> Result<Request, ParseFailure> {
    parse_request(
        ["terminal", "session", "observe"]
            .into_iter()
            .chain(extra.iter().copied())
            .map(OsString::from),
    )
}
#[test]
fn terminal_control_scopes_takeover_and_reuses_checked_dimensions() {
    let parse = |words: &[&str]| parse_request(words.iter().map(OsString::from));
    let Request::TerminalControl { command } = parse(&[
        "terminal",
        "session",
        "control",
        "@terminal:ab",
        "--takeover",
        "--cols=100",
    ])
    .unwrap() else {
        panic!("control")
    };
    assert!(command.takeover);
    assert_eq!((command.stream.columns, command.stream.rows), (100, 40));
    for words in [
        vec![
            "terminal",
            "session",
            "observe",
            "@terminal:ab",
            "--takeover",
        ],
        vec![
            "terminal",
            "session",
            "control",
            "@terminal:ab",
            "--takeover",
            "--takeover",
        ],
        vec!["terminal", "session", "control", "@terminal:ab", "--cols=0"],
        vec!["terminal", "session", "control", "@terminal:ab", "--stdin"],
    ] {
        assert!(parse(&words).is_err());
    }
}
#[test]
fn terminal_observe_defaults_and_scoped_options() {
    let Request::TerminalObserve { command } = parse(&["@terminal:ab"]).expect("parse") else {
        panic!("stream request")
    };
    assert_eq!(
        (command.columns, command.rows, command.timeout_ms),
        (120, 40, None)
    );
    let Request::TerminalObserve { command } = parse(&[
        "@terminal:ab",
        "--cols=256",
        "--rows=1024",
        "--timeout=50",
        "--format=json",
    ])
    .expect("parse") else {
        panic!("stream request")
    };
    assert_eq!(
        (command.columns, command.rows, command.timeout_ms),
        (256, 1024, Some(50))
    );
    for extra in [
        vec![],
        vec!["@terminal:ab", "extra"],
        vec!["@terminal:ab", "--cols=0"],
        vec!["@terminal:ab", "--cols=+1"],
        vec!["@terminal:ab", "--cols=4096", "--rows=4096"],
        vec!["@terminal:ab", "--timeout=0"],
        vec!["@terminal:ab", "--cell-width=8"],
        vec!["@terminal:ab", "--stdin"],
        vec!["@terminal:ab", "--enter"],
        vec!["@terminal:ab", "--history"],
    ] {
        assert!(parse(&extra).is_err(), "{extra:?}");
    }
    assert_eq!(
        help_topic(&["terminal", "session", "observe"]),
        Some(HelpTopic::Command(CommandPath::TerminalObserve))
    );
}
