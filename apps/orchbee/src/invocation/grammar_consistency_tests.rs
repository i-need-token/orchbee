//! Public spellings are closed before any runtime access.
use super::*;
use std::ffi::OsString;

fn parse(words: &[&str]) -> Result<Request, ParseFailure> {
    parse_request(words.iter().map(OsString::from))
}

#[test]
fn output_waits_are_finite_without_an_explicit_forever() {
    let id = "@terminal:0x1";
    for (extra, expected) in [(vec![], Some(30_000)), (vec!["--forever"], None)] {
        let mut words = vec!["terminal", "wait-output", id, "--match", "ready"];
        words.extend(extra);
        assert!(matches!(parse(&words), Ok(Request::Query {
            command: QueryCommand::TerminalSourceWait { timeout_ms, .. }, ..
        }) if timeout_ms == expected));
    }
}

#[test]
fn terminal_agent_targets_are_explicit_and_exclusive() {
    for words in [
        vec!["agent", "get", "@terminal:0x1"],
        vec!["agent", "get", "--terminal", "@terminal:0x1"],
    ] {
        assert!(
            matches!(
                parse(&words),
                Ok(Request::Query {
                    command: QueryCommand::AgentGet(AgentTarget::Terminal { .. }),
                    ..
                })
            ),
            "{words:?}"
        );
    }
    assert!(
        parse(&[
            "agent",
            "get",
            "--terminal",
            "@terminal:0x1",
            "--panel",
            "@panel:0x1"
        ])
        .is_err()
    );
}

#[cfg(windows)]
#[test]
fn panel_targets_and_direction_domains_have_one_spelling() {
    for verb in ["layout", "current", "focus", "edges"] {
        assert!(parse(&["panel", verb, "@panel:0x1"]).is_ok(), "{verb}");
    }
    for direction in ["left", "right", "up", "down"] {
        assert!(parse(&["panel", "split", "@panel:0x1", "--direction", direction]).is_ok());
    }
    for direction in ["above", "below"] {
        assert!(parse(&["panel", "split", "@panel:0x1", "--direction", direction]).is_err());
    }
    assert!(parse(&["tab", "move", "@tab:0x1", "--order", "next"]).is_ok());
    assert!(parse(&["tab", "move", "@tab:0x1", "--direction", "next"]).is_err());
}

#[cfg(windows)]
#[test]
fn public_events_use_panel_and_translate_only_at_the_boundary() {
    assert!(parse(&["events", "wait", "--event", "panel.created"]).is_ok());
    assert!(parse(&["events", "wait", "--event", "pane.created"]).is_err());
    assert!(
        parse(&[
            "events",
            "wait",
            "--filter",
            r#"{"kind":"event","event":"panel.created","scope":{"kind":"all"},"condition":{"kind":"any"}}"#
        ])
        .is_ok()
    );
    assert!(
        parse(&[
            "events",
            "wait",
            "--filter",
            r#"{"kind":"event","event":"pane.created","scope":{"kind":"all"},"condition":{"kind":"any"}}"#
        ])
        .is_err()
    );
}

#[test]
fn provider_identity_is_canonical() {
    assert!(parse(&["agent", "integration", "list", "--provider", "antigravity"]).is_ok());
    assert!(
        parse(&[
            "agent",
            "integration",
            "list",
            "--provider",
            "antigravity-cli"
        ])
        .is_err()
    );
    assert!(parse(&["agent", "get", "--kind", "codex"]).is_err());
}
