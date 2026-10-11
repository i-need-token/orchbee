use crate::invocation::{Request, parse_request};
#[test]
fn report_members_are_exact_and_rejected_outside_the_five_report_commands() {
    let parse = |args: &[&str]| parse_request(args.iter().map(std::ffi::OsString::from));
    let id = "00000000000000000000000000000007";
    for verb in [
        "report-agent",
        "report-agent-session",
        "clear-agent-authority",
        "release-agent",
        "report-metadata",
    ] {
        let mut args = vec!["panel", verb, "@panel:1", "--presentation", id];
        if verb == "report-metadata" {
            args.extend(["--source=hook", "--title=x"]);
        } else if verb != "clear-agent-authority" {
            args.extend(["--source=hook", "--agent=codex"]);
        }
        if verb == "report-agent" {
            args.push("--state=blocked");
        }
        let member = match parse(&args).unwrap() {
            Request::PanelAgent { command, .. } => command.presentation_id,
            Request::PanelMetadata { command, .. } => command.presentation_id,
            _ => panic!("report"),
        };
        assert_eq!(member.unwrap().as_str(), id);
        for bad in [
            "",
            "@presentation:7",
            "00000000000000000000000000000000",
            "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
        ] {
            args[4] = bad;
            assert!(parse(&args).is_err(), "{verb}/{bad}");
        }
    }
    for args in [
        vec!["panel", "get", "@panel:1", "--presentation", id],
        vec!["api", "schema", "--presentation", id],
    ] {
        assert!(parse(&args).is_err());
    }
}
