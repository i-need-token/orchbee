use super::*;
use crate::invocation::{CommandPath, HelpTopic, help_topic, parse_request};
use std::ffi::OsString;
#[test]
fn attach_targets_are_explicit_and_options_are_scoped_before_effects() {
    let parse = |words: &[&str]| parse_request(words.iter().map(OsString::from));
    let Request::TerminalAttach { command } = parse(&[
        "--mouse=off",
        "terminal",
        "attach",
        "@terminal:0xab",
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
        vec!["agent", "attach", "--agent-session=session-1"],
        vec!["agent", "attach", "--panel=@panel:0xab"],
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
        vec!["terminal", "attach", "@terminal:0xab", "extra"],
        vec!["terminal", "attach", "@terminal:0xab", "--cols=80"],
        vec!["terminal", "attach", "@terminal:0xab", "--mouse=invalid"],
        vec![
            "terminal",
            "attach",
            "@terminal:0xab",
            "--mouse=on",
            "--mouse=off",
        ],
        vec!["terminal", "attach", "@terminal:0xab", "--timeout=0"],
        vec!["terminal", "attach", "@terminal:0xab", "--name=alpha"],
        vec!["agent", "attach"],
        vec!["agent", "attach", "alpha", "--name=alpha"],
        vec![
            "agent",
            "attach",
            "--panel=@panel:0xab",
            "--agent-session=session-1",
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
        "@terminal:0xab",
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
            "@terminal:0xab",
            "--takeover",
        ],
        vec![
            "terminal",
            "session",
            "control",
            "@terminal:0xab",
            "--takeover",
            "--takeover",
        ],
        vec![
            "terminal",
            "session",
            "control",
            "@terminal:0xab",
            "--cols=0",
        ],
        vec![
            "terminal",
            "session",
            "control",
            "@terminal:0xab",
            "--stdin",
        ],
    ] {
        assert!(parse(&words).is_err());
    }
}
#[test]
fn terminal_observe_defaults_and_scoped_options() {
    let Request::TerminalObserve { command } = parse(&["@terminal:0xab"]).expect("parse") else {
        panic!("stream request")
    };
    assert_eq!(
        (command.columns, command.rows, command.timeout_ms),
        (120, 40, None)
    );
    let Request::TerminalObserve { command } = parse(&[
        "@terminal:0xab",
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
        vec!["@terminal:0xab", "extra"],
        vec!["@terminal:0xab", "--cols=0"],
        vec!["@terminal:0xab", "--cols=+1"],
        vec!["@terminal:0xab", "--cols=4096", "--rows=4096"],
        vec!["@terminal:0xab", "--timeout=0"],
        vec!["@terminal:0xab", "--cell-width=8"],
        vec!["@terminal:0xab", "--stdin"],
        vec!["@terminal:0xab", "--enter"],
    ] {
        assert!(parse(&extra).is_err(), "{extra:?}");
    }
    assert_eq!(
        help_topic(&["terminal", "session", "observe"]),
        Some(HelpTopic::Command(CommandPath::TerminalObserve))
    );
}
