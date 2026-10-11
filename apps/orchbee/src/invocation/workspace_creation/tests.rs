use super::*;
use std::ffi::OsString;
fn parse(words: &[&str]) -> Result<Request, ParseFailure> {
    super::super::parse_request(words.iter().map(OsString::from))
}
#[test]
fn every_workspace_create_launches_with_identical_defaults_with_or_without_no_focus() {
    for cwd in [
        None,
        Some("C:/repo"),
        Some("./repo"),
        Some("current"),
        Some("home"),
    ] {
        let mut args = vec!["workspace", "create", "--project=p", "--label=w"];
        if let Some(cwd) = cwd {
            args.extend(["--cwd", cwd]);
        }
        let ordinary = parse(&args).unwrap();
        args.push("--no-focus");
        assert_eq!(parse(&args).unwrap(), ordinary, "{args:?}");
        let Request::WorkspaceCreation { command, .. } = ordinary else {
            panic!("launch workflow");
        };
        assert_eq!(command.project, Some(Target::Label("p".into())));
        assert_eq!(command.label.unwrap().as_str(), "w");
        assert!(!command.focus);
        assert_eq!(command.timeout_ms, 30_000);
    }
    for tail in [
        vec![],
        vec!["--label=w"],
        vec!["--project=p"],
        vec!["--project=p", "--label=w", "--no-focus"],
        vec!["--project=p", "--label=w", "--env=TOKEN=value"],
        vec!["--cwd=follow", "--workspace=source"],
        vec!["--cwd=./repo"],
    ] {
        let mut args = vec!["workspace", "create"];
        args.extend(tail);
        let Request::WorkspaceCreation { command, .. } = parse(&args).unwrap() else {
            panic!("launch workflow")
        };
        assert!(!command.focus);
        assert_eq!(command.timeout_ms, 30_000);
    }
}
#[test]
fn invalid_creation_inputs_refuse_before_runtime() {
    for tail in [
        vec!["--focus", "--no-focus"],
        vec!["--focus", "--detach"],
        vec!["--client=bad"],
        vec!["--cols=80"],
        vec!["--cwd=C:relative"],
        vec!["--timeout=0"],
        vec!["--env=ORCHBEE_SECRET=private"],
        vec!["--profile=p", "--exec=C:/shell.exe"],
    ] {
        let mut args = vec!["workspace", "create"];
        args.extend(tail);
        assert!(parse(&args).is_err(), "{args:?}");
    }
}
