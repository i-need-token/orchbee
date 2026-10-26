use super::super::*;
use std::ffi::OsString;
fn parse(words: &[&str]) -> Result<Request, ParseFailure> {
    crate::invocation::parse_request(words.iter().map(OsString::from))
}

fn parsed_directory(value: &str) -> ParsedWorkspaceCreationCommand {
    let Request::WorkspaceCreation { command, .. } =
        parse(&["workspace", "create", "--cwd", value, "--format=json"]).unwrap()
    else {
        panic!("workspace creation command")
    };
    command
}

#[test]
fn explicit_operands_retain_raw_spelling_until_the_executing_context_is_supplied() {
    for (operand, home, cwd, expected) in [
        (r"C:\literal\..\remote", None, None, r"C:\literal\..\remote"),
        ("remote", None, Some(r"D:\executor"), r"D:\executor\remote"),
        (r"~\remote", Some(r"E:\home"), None, r"E:\home\remote"),
    ] {
        let command = parsed_directory(operand);
        assert!(command.options.cwd.is_none());
        assert!(
            matches!(&command.directory, ParsedWorkspaceCreationDirectory::Explicit(path) if path.as_str() == operand)
        );
        let resolved = command
            .into_resolved(
                OutputFormat::Json,
                |key| {
                    assert_eq!(key, "USERPROFILE");
                    Some(OsString::from(home.expect("this path must not read home")))
                },
                || {
                    Ok(std::path::PathBuf::from(
                        cwd.expect("this path must not read cwd"),
                    ))
                },
            )
            .unwrap();
        assert_eq!(resolved.directory, CreationDirectory::Explicit);
        assert_eq!(resolved.options.cwd.unwrap().as_str(), expected);
    }
}

#[test]
fn directory_policies_keep_their_meaning_without_acquiring_path_context() {
    for (value, expected) in [
        ("follow", CreationDirectory::Follow),
        ("home", CreationDirectory::Home),
        ("current", CreationDirectory::Current),
        ("workspace", CreationDirectory::Workspace),
    ] {
        let resolved = parsed_directory(value)
            .into_resolved(
                OutputFormat::Text,
                |_| panic!("policy must not read home"),
                || panic!("policy must not read cwd"),
            )
            .unwrap();
        assert_eq!(resolved.directory, expected);
        assert!(resolved.options.cwd.is_none());
    }
    let Request::WorkspaceCreation { command, .. } = parse(&["workspace", "create"]).unwrap()
    else {
        panic!("workspace creation command")
    };
    let resolved = command
        .into_resolved(
            OutputFormat::Text,
            |_| panic!("default policy"),
            || panic!("default policy"),
        )
        .unwrap();
    assert_eq!(resolved.directory, CreationDirectory::Default);
    assert!(resolved.options.cwd.is_none());
}

#[test]
fn deferred_path_failures_preserve_the_usage_envelope_and_context_redaction() {
    for format in [OutputFormat::Text, OutputFormat::Json] {
        let context_failure = parsed_directory("~")
            .into_resolved(
                format,
                |_| Some(OsString::from("private-invalid-home")),
                || panic!("home path"),
            )
            .unwrap_err();
        assert!(
            matches!(context_failure.usage, Some(UsageReason::InvalidValue { context: UsageContext::Path("workspace create"), option: "USERPROFILE", value, expected: ExpectedValue::DirectoryPath }) if value.is_empty())
        );
        assert_eq!(
            context_failure.kind,
            super::super::ParseFailureKind::InvalidUsage
        );
        assert_eq!(context_failure.envelope, selection_envelope(Some(format)));
        let operand = "x".repeat(4096);
        let input_failure = parsed_directory(&operand)
            .into_resolved(
                format,
                |_| panic!("relative path"),
                || Ok(std::path::PathBuf::from(r"C:\executor")),
            )
            .unwrap_err();
        assert!(
            matches!(input_failure.usage, Some(UsageReason::InvalidValue { context: UsageContext::Path("workspace create"), option: "--cwd", value, expected: ExpectedValue::WorktreePath }) if value == operand)
        );
        assert_eq!(input_failure.envelope, selection_envelope(Some(format)));
    }
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
