use super::*;
fn parse(args: &[&str]) -> Result<Request, ParseFailure> {
    super::super::parse_request(args.iter().map(std::ffi::OsString::from))
}
#[test]
fn admits_member_intent_and_literal_launch_options() {
    for (args, expected) in [
        (vec!["panel", "create-terminal"], Member::Primary),
        (
            vec![
                "panel",
                "create-terminal",
                "--child",
                "--label",
                "helper",
                "--cwd",
                "current",
                "--exec",
                "C:/tools/program.exe",
                "--arg",
                "a & b",
                "--env",
                "X=a=b",
            ],
            Member::Child,
        ),
        (
            vec![
                "panel",
                "create-terminal",
                "--presentation",
                "00000000000000000000000000000007",
            ],
            Member::Existing(
                TerminalPresentationId::parse("00000000000000000000000000000007").unwrap(),
            ),
        ),
    ] {
        let Request::PanelTerminalCreation { command, .. } = parse(&args).unwrap() else {
            panic!("member creation");
        };
        assert_eq!(command.member, expected);
        assert_eq!(command.timeout_ms, 30_000);
        if command.member == Member::Child {
            assert_eq!(
                command.options.environment.entries()[0],
                ("X".to_owned(), "a=b".to_owned())
            );
        }
    }
}
#[test]
fn refuses_conflicts_and_invalid_values_before_discovery() {
    for suffix in [
        vec!["--child", "--presentation", "bad"],
        vec!["--label", "x"],
        vec!["--presentation", "bad"],
        vec!["--child", "--child"],
        vec!["--focus"],
        vec!["--exec", "C:/app.exe", "--profile", "x"],
        vec!["--env", "TERM=x"],
        vec!["--panel", "00000000000000000000000000000007", "--current"],
        vec![
            "--panel",
            "00000000000000000000000000000007",
            "--client",
            "00000000000000000000000000000008",
        ],
        vec!["--ratio", "0.4"],
        vec!["--timeout", "0"],
        vec!["--cwd", "relative"],
    ] {
        let mut args = vec!["panel", "create-terminal"];
        args.extend(suffix);
        assert!(parse(&args).is_err(), "{args:?}");
    }
    assert!(parse(&["panel", "split", "--child"]).is_err());
    assert!(parse(&["tab", "create", "--child"]).is_err());
}
