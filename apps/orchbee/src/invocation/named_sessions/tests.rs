use super::*;
use crate::invocation::{SelectedRequest, parse_selected_request};
use std::ffi::OsString;

fn selected(words: &[&str]) -> Result<SelectedRequest, ParseFailure> {
    parse_selected_request(words.iter().map(OsString::from))
}

#[test]
fn named_lifecycle_selects_one_exact_session_and_preserves_stop_options() {
    for verb in ["attach", "stop", "delete"] {
        let mut words = vec!["--session=Work", "session", verb, "Work"];
        if verb != "attach" {
            words.push("--confirm");
        }
        let result = selected(&words).unwrap();
        assert_eq!(result.session.unwrap().as_str(), "Work");
        assert!(matches!(result.request, Request::NamedSession { .. }));
    }
    let result = selected(&["session", "attach", "work", "--mouse=off"]).unwrap();
    assert_eq!(result.session.unwrap().as_str(), "work");
    assert!(matches!(
        result.request,
        Request::NamedSession {
            command: NamedSessionCommand::Attach { mouse: false, .. },
            ..
        }
    ));
    let result = selected(&[
        "session",
        "stop",
        "work",
        "--confirm",
        "--detach",
        "--timeout=1234",
        "--retry",
        "--request-id=11111111111111111111111111111111",
        "--format=json",
    ])
    .unwrap();
    assert!(matches!(
        result.request,
        Request::NamedSession {
            command: NamedSessionCommand::Stop {
                command: ServerStopCommand::Stop {
                    retry: true,
                    detach: true,
                    timeout_ms: 1234,
                    id: Some(_),
                    ..
                },
                ..
            },
            format: OutputFormat::Json
        }
    ));
    assert!(matches!(
        selected(&["session", "list", "--format=json"])
            .unwrap()
            .request,
        Request::NamedSession {
            command: NamedSessionCommand::List,
            format: OutputFormat::Json
        }
    ));
}

#[test]
fn invalid_lifecycle_grammar_fails_before_selection_or_runtime_effects() {
    for words in [
        vec!["session"],
        vec!["session", "attach"],
        vec!["session", "attach", "work", "--format=json"],
        vec!["session", "list", "work"],
        vec!["session", "list", "--mouse=off"],
        vec!["session", "stop", "work"],
        vec!["session", "delete", "work"],
        vec!["session", "delete", "..", "--confirm"],
        vec!["session", "delete", "work", "extra", "--confirm"],
        vec!["session", "delete", "work", "--confirm", "--timeout=1"],
        vec!["session", "stop", "work", "--confirm", "--retry"],
        vec!["--session=Work", "session", "delete", "work", "--confirm"],
    ] {
        assert!(selected(&words).is_err(), "{words:?}");
    }
    for verb in ["list", "attach", "stop", "delete"] {
        assert!(matches!(
            selected(&["help", "session", verb]).unwrap().request,
            Request::Help(_)
        ));
        assert!(matches!(
            selected(&["session", verb, "--help"]).unwrap().request,
            Request::Help(_)
        ));
    }
}
