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
            "@terminal:22",
            "--file=x",
            "--format=json",
        ],
        vec!["--machine=中文", "workspace", "list"],
        vec!["--machine=work", "client", "list"],
        vec!["--machine=work", "panel", "focus", "@panel:22"],
    ] {
        assert!(matches!(
            parse_request(args.into_iter().map(Into::into)).unwrap(),
            Request::MachineRoute(_)
        ));
    }
    for args in [
        vec!["--machine", "work"],
        vec!["--machine=work", "--site=x", "workspace", "list"],
        vec!["--machine=work", "machine", "list"],
        vec!["--machine=work", "status", "client"],
        vec!["--machine=work", "tui"],
        vec!["--machine=work", "update"],
        vec!["--machine=work", "--machine=x", "status"],
        vec!["status", "--machine=work"],
        vec!["--machine=work", "agent", "doctor"],
        vec!["--machine=work", "terminal", "attach", "@terminal:22"],
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
