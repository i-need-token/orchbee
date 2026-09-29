use super::*;
use crate::invocation::{CommandPath, Family, HelpTopic, parse_request};
use std::ffi::OsString;
fn parse(words: &[&str]) -> Result<Request, ParseFailure> {
    parse_request(words.iter().map(OsString::from))
}
#[test]
fn machine_grammar_is_complete_before_any_effect() {
    assert!(matches!(
        parse(&["machine", "list", "--json"]).unwrap(),
        Request::Machine {
            command: MachineCommand::List,
            format: OutputFormat::Json
        }
    ));
    assert!(
        matches!(parse(&["machine","add","ssh://user@host:22","--label=工作","--remote-session=work"]).unwrap(),
        Request::Machine {command:MachineCommand::Add(draft),..} if draft.session.as_str()=="work")
    );
    let id = "11111111111111111111111111111111";
    for verb in ["enable", "disable", "remove"] {
        assert!(parse(&["machine", verb, id]).is_ok());
    }
    assert!(parse(&["machine", "rename", id, "--label", "renamed"]).is_ok());
    assert!(matches!(
        parse(&["machine", "--help"]).unwrap(),
        Request::Help(HelpTopic::Family(Family::Machine))
    ));
    assert!(matches!(
        parse(&["help", "machine", "add"]).unwrap(),
        Request::Help(HelpTopic::Command(CommandPath::MachineAdd))
    ));
    for args in [
        vec!["--remote-session", "work"],
        vec!["status", "--remote-session", "work"],
        vec!["tui", "--remote-session", "work"],
        vec!["machine"],
        vec!["machine", "list", "extra"],
        vec!["machine", "list", "--label", "x"],
        vec!["machine", "list", "--json", "--format=json"],
        vec!["machine", "list", "--json", "--json"],
        vec!["status", "--json"],
        vec!["machine", "remove", "host"],
        vec!["machine", "add", "host"],
        vec!["machine", "add", "host", "--label", " "],
        vec!["machine", "add", "host;calc", "--label", "x"],
        vec![
            "machine",
            "add",
            "host",
            "--label",
            "x",
            "--remote-session",
            "../x",
        ],
        vec!["machine", "add", "host", "--label", "x", "--label", "y"],
        vec![
            "machine",
            "add",
            "host",
            "--label",
            "x",
            "--remote-session",
            "x",
            "--remote-session",
            "y",
        ],
        vec![
            "machine",
            "rename",
            id,
            "--label",
            "x",
            "--remote-session",
            "y",
        ],
        vec!["machine", "enable", id, "--json"],
    ] {
        assert!(parse(&args).is_err(), "{args:?}");
    }
}
