use super::*;
use crate::invocation::parse_request;
#[test]
fn routed_paths_retain_the_exact_unresolved_operand() {
    for arguments in [
        ["plugin", "link", r"~\remote", "--format=json"].as_slice(),
        ["workspace", "create", "--cwd", r"~\remote", "--format=json"].as_slice(),
        ["worktree", "list", "--cwd", r"~\remote", "--format=json"].as_slice(),
        ["worktree", "create", "--cwd", r"~\remote", "--format=json"].as_slice(),
        ["worktree", "create", "--path", r"~\remote", "--format=json"].as_slice(),
        ["worktree", "open", "--path", r"~\remote", "--format=json"].as_slice(),
    ] {
        let request = parse_request(
            ["--machine", "work"]
                .into_iter()
                .chain(arguments.iter().copied())
                .map(Into::into),
        )
        .unwrap();
        let Request::MachineRoute(route) = request else {
            panic!("expected machine route")
        };
        assert_eq!(route.arguments, arguments);
        assert_eq!(route.format, OutputFormat::Json);
        assert!(route.input.is_none());
    }
}

#[test]
fn prefix_preserves_sources_and_rejects_local_and_conflicting_modes() {
    for args in [
        vec![
            "--machine",
            "work",
            "terminal",
            "send",
            "@terminal:0x22",
            "--file=x",
            "--format=json",
        ],
        vec!["--machine=中文", "workspace", "list"],
        vec!["--machine=work", "client", "list"],
        vec!["--machine=work", "panel", "focus", "@panel:0x22"],
    ] {
        assert!(matches!(
            parse_request(args.into_iter().map(Into::into)).unwrap(),
            Request::MachineRoute(_)
        ));
    }
    for args in [
        vec!["--machine", "work"],
        vec!["--machine=work", "--session=x", "workspace", "list"],
        vec!["--machine=work", "machine", "list"],
        vec!["--machine=work", "status", "client"],
        vec!["--machine=work", "tui"],
        vec!["--machine=work", "update"],
        vec!["--machine=work", "--machine=x", "status"],
        vec!["status", "--machine=work"],
        vec!["--machine=work", "agent", "doctor"],
        vec!["--machine=work", "terminal", "attach", "@terminal:0x22"],
    ] {
        assert!(
            parse_request(args.iter().map(Into::into)).is_err(),
            "{args:?}"
        );
    }
    assert!(matches!(
        parse_request(
            ["--machine=no-such-host", "workspace", "--help"]
                .into_iter()
                .map(Into::into)
        )
        .unwrap(),
        Request::Help(_)
    ));
}

#[test]
fn a_remote_stop_is_confirmed_here_and_refuses_the_local_stop_options() {
    for (args, format) in [
        (
            vec!["--machine", "work", "server", "stop", "--confirm"],
            OutputFormat::Text,
        ),
        (
            vec![
                "--machine=work",
                "server",
                "stop",
                "--confirm",
                "--format=json",
            ],
            OutputFormat::Json,
        ),
    ] {
        assert_eq!(
            parse_request(args.into_iter().map(Into::into)).unwrap(),
            Request::MachineStop(RemoteStop {
                selector: "work".into(),
                confirm: true,
                format,
            })
        );
    }
    // A test build requires `--confirm` before anything is read or asked, as a local stop does.
    assert!(
        parse_request(
            ["--machine=work", "server", "stop"]
                .into_iter()
                .map(Into::into)
        )
        .is_err()
    );
    for (option, extra) in [
        (
            "--request-id",
            ["--request-id", "0123456789abcdef0123456789abcdef"].as_slice(),
        ),
        (
            "--retry",
            [
                "--retry",
                "--request-id",
                "0123456789abcdef0123456789abcdef",
            ]
            .as_slice(),
        ),
        ("--detach", ["--detach"].as_slice()),
        ("--timeout", ["--timeout", "65000"].as_slice()),
        ("--timeout", ["--timeout=1000"].as_slice()),
    ] {
        let failure = parse_request(
            ["--machine=work", "server", "stop", "--confirm"]
                .into_iter()
                .chain(extra.iter().copied())
                .map(Into::into),
        )
        .unwrap_err();
        assert!(
            matches!(
                failure.usage,
                Some(UsageReason::UnexpectedOption { option: refused, .. }) if refused == option
            ),
            "{extra:?}: {:?}",
            failure.usage
        );
    }
    // Neither side runs a stop as a routed command line; reading a stop's status still routes.
    let Ok(Request::ServerStop { command, format }) = crate::invocation::parse_request(
        ["server", "stop", "--confirm"].into_iter().map(Into::into),
    ) else {
        panic!("a local stop parses");
    };
    assert!(admit(&Request::ServerStop { command, format }).is_none());
    assert!(matches!(
        parse_request(
            [
                "--machine=work",
                "server",
                "status",
                "0123456789abcdef0123456789abcdef"
            ]
            .into_iter()
            .map(Into::into)
        )
        .unwrap(),
        Request::MachineRoute(_)
    ));
}
