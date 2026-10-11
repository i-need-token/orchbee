use super::*;
fn parse(args: &[&str]) -> Result<Request, ParseFailure> {
    super::super::parse_request(args.iter().map(std::ffi::OsString::from))
}
#[test]
fn agent_view_commands_parse_full_json_and_source_guard_before_runtime() {
    assert!(matches!(
        parse(&["agent", "view", "get", "--format=json"]).unwrap(),
        Request::AgentView {
            action: AgentViewAction::Read {},
            format: OutputFormat::Json
        }
    ));
    let Request::AgentView{action:AgentViewAction::Set{spec},..}=parse(&["agent","view","set","--spec",r#"{"source":"  tools.views ","filter":{"op":"eq","field":"seen","value":false},"sort":[{"field":{"token":"phase"},"order":"desc"}]}"#]).unwrap() else {panic!()};
    assert_eq!(spec.source, "tools.views");
    assert_eq!(spec.sort.len(), 1);
    assert!(
        matches!(parse(&["agent","view","clear","--source= tools.views "]).unwrap(),Request::AgentView{action:AgentViewAction::Clear{source:Some(s)},..} if s=="tools.views")
    );
    for args in [
        vec!["agent", "view", "set"],
        vec!["agent", "view", "set", "--spec", "{bad}"],
        vec!["agent", "view", "clear", "--source", ""],
        vec!["agent", "view", "get", "--spec", "{}"],
        vec![
            "agent",
            "view",
            "set",
            "--spec",
            "{\"source\":\"x\"}",
            "--source",
            "x",
        ],
        vec!["agent", "view", "get", "extra"],
        vec!["agent", "list", "--spec", "{}"],
    ] {
        assert!(parse(&args).is_err(), "{args:?}");
    }
}
#[test]
fn agent_view_help_and_saved_machine_route_are_admitted() {
    for verb in ["get", "set", "clear"] {
        assert!(matches!(
            parse(&["help", "agent", "view", verb]).unwrap(),
            Request::Help(_)
        ));
    }
    assert!(matches!(
        parse(&[
            "--machine",
            "remote",
            "agent",
            "view",
            "set",
            "--spec",
            "{\"source\":\"x\"}"
        ])
        .unwrap(),
        Request::MachineRoute(_)
    ));
}
