use super::*;
use crate::invocation::{ParseFailureKind, Request, UsageReason, parse_request};

fn parse(words: &[&str]) -> Request {
    parse_request(words.iter().map(OsString::from)).unwrap()
}

fn start_operand<P>(command: &WorktreeCommand<P>, source: bool) -> &P {
    let WorktreeCommand::Start(command) = command else {
        panic!("start command")
    };
    if source {
        return command.cwd.as_ref().unwrap();
    }
    match &command.kind {
        WorktreeStartKind::Create {
            path: Some(path), ..
        }
        | WorktreeStartKind::Open {
            selector: WorktreeOpenSelector::Path { path },
        } => path,
        _ => panic!("explicit target path"),
    }
}

#[test]
fn all_four_path_slots_retain_spelling_and_resolve_only_needed_execution_context() {
    for (verb, option) in [
        ("list", "--cwd"),
        ("create", "--cwd"),
        ("create", "--path"),
        ("open", "--path"),
    ] {
        for (operand, home, cwd, expected) in [
            (r"C:\literal\..\remote", None, None, r"C:\literal\..\remote"),
            ("remote", None, Some(r"D:\executor"), r"D:\executor\remote"),
            (r"~\remote", Some(r"E:\home"), None, r"E:\home\remote"),
        ] {
            let environment = |name: &str| {
                assert_eq!(name, "USERPROFILE");
                Some(OsString::from(home.expect("must not read home")))
            };
            let current_directory = || Ok(PathBuf::from(cwd.expect("must not read cwd")));
            match parse(&["worktree", verb, option, operand]) {
                Request::WorktreeQuery { command, format } => {
                    assert_eq!(command.cwd.as_ref().unwrap().as_str(), operand);
                    let resolved = command
                        .into_resolved(format, environment, current_directory)
                        .unwrap();
                    assert_eq!(resolved.cwd.unwrap().as_str(), expected);
                }
                Request::Worktree { command, format } => {
                    assert_eq!(start_operand(&command, option == "--cwd").as_str(), operand);
                    let resolved = command
                        .into_resolved(format, environment, current_directory)
                        .unwrap();
                    assert_eq!(
                        start_operand(&resolved, option == "--cwd").as_str(),
                        expected
                    );
                }
                _ => panic!("Worktree request"),
            }
        }
    }
}

#[test]
fn resolution_preserves_launch_selection_identity_geometry_and_wait_policy() {
    let Request::Worktree { command, format } = parse(&[
        "worktree",
        "create",
        "--project=project",
        "--cwd=source",
        r"--path=~\target",
        "--branch=feature",
        "--base=HEAD~2",
        "--label=new",
        "--trust-repository",
        "--request-id=00000000000000000000000000000001",
        "--client=00000000000000000000000000000002",
        "--cols=100",
        "--rows=30",
        "--cell-width=9",
        "--cell-height=18",
        "--focus",
        "--timeout=7",
    ]) else {
        panic!("start")
    };
    let WorktreeCommand::Start(before) = command.clone() else {
        panic!("start")
    };
    let resolved = command
        .into_resolved(
            format,
            |_| Some(OsString::from(r"E:\home")),
            || Ok(PathBuf::from(r"D:\executor")),
        )
        .unwrap();
    let WorktreeCommand::Start(after) = resolved else {
        panic!("start")
    };
    assert_eq!(after.project, before.project);
    assert_eq!(after.workspace, before.workspace);
    assert_eq!(after.client, before.client);
    assert_eq!(after.label, before.label);
    assert_eq!(after.geometry, before.geometry);
    assert_eq!(after.trust_repository, before.trust_repository);
    assert_eq!(after.operation_id, before.operation_id);
    assert_eq!(after.focus, before.focus);
    assert_eq!(after.detach, before.detach);
    assert_eq!(after.timeout_ms, before.timeout_ms);
    assert_eq!(after.cwd.as_ref().unwrap().as_str(), r"D:\executor\source");
    assert!(
        matches!(&after.kind, WorktreeStartKind::Create { branch: Some(branch), base: Some(base), path: Some(path) }
        if branch.as_str() == "feature" && base.as_str() == "HEAD~2" && path.as_str() == r"E:\home\target")
    );
}

#[test]
fn commands_without_explicit_paths_never_read_path_context() {
    for words in [
        vec!["worktree", "status", "00000000000000000000000000000001"],
        vec![
            "worktree",
            "resume",
            "00000000000000000000000000000001",
            "--generation=2",
            "--confirm",
            "--detach",
            "--timeout=7",
        ],
        vec![
            "worktree",
            "remove",
            "--workspace=source",
            "--confirm",
            "--force",
            "--trust-repository",
            "--detach",
        ],
        vec!["worktree", "open", "--workspace=source", "--branch=main"],
        vec!["worktree", "create"],
    ] {
        let Request::Worktree { command, format } = parse(&words) else {
            panic!("operation")
        };
        let before = format!("{command:?}");
        let resolved = command
            .into_resolved(
                format,
                |_| panic!("no home operand"),
                || panic!("no relative operand"),
            )
            .unwrap();
        assert_eq!(format!("{resolved:?}"), before);
    }
    let Request::WorktreeQuery { command, format } = parse(&[
        "worktree",
        "list",
        "--workspace=source",
        "--client=00000000000000000000000000000002",
        "--trust-repository",
    ]) else {
        panic!("query")
    };
    let before = format!("{command:?}");
    let resolved = command
        .into_resolved(
            format,
            |_| panic!("no home operand"),
            || panic!("no relative operand"),
        )
        .unwrap();
    assert_eq!(format!("{resolved:?}"), before);
}

#[test]
fn target_context_failure_precedes_source_context_and_redacts_environment() {
    for (target, source, expected_option) in [
        ("~", "relative", "USERPROFILE"),
        ("relative", "~", "current-directory"),
    ] {
        let Request::Worktree { command, .. } =
            parse(&["worktree", "create", "--cwd", source, "--path", target])
        else {
            panic!("start")
        };
        let failure = command
            .into_resolved(
                OutputFormat::Json,
                |_| {
                    assert_eq!(target, "~");
                    Some(OsString::from("private-invalid-home"))
                },
                || {
                    assert_eq!(target, "relative");
                    Err(std::io::ErrorKind::NotFound.into())
                },
            )
            .unwrap_err();
        assert!(matches!(failure.usage, Some(UsageReason::InvalidValue {
            context: UsageContext::Path("worktree create"), option, value, expected: ExpectedValue::DirectoryPath,
        }) if option == expected_option && value.is_empty()));
    }
}

#[test]
fn every_path_slot_preserves_usage_context_format_and_input_vs_context_failure() {
    for (verb, option, context) in [
        ("list", "--cwd", "worktree list"),
        ("create", "--cwd", "worktree create"),
        ("create", "--path", "worktree create"),
        ("open", "--path", "worktree open"),
        ("open", "--cwd", "worktree open"),
    ] {
        for format in [OutputFormat::Text, OutputFormat::Json] {
            for operand in ["~".to_owned(), "x".repeat(4096)] {
                let environment = |_: &str| Some(OsString::from("private-invalid-home"));
                let cwd = || Ok(PathBuf::from(r"C:\executor"));
                let mut words = vec!["worktree", verb, option, &operand];
                if verb == "open" && option == "--cwd" {
                    words.push("--branch=main");
                }
                let failure = match parse(&words) {
                    Request::Worktree { command, .. } => {
                        command.into_resolved(format, environment, cwd).unwrap_err()
                    }
                    Request::WorktreeQuery { command, .. } => {
                        command.into_resolved(format, environment, cwd).unwrap_err()
                    }
                    _ => panic!("Worktree request"),
                };
                assert_eq!(failure.kind, ParseFailureKind::InvalidUsage);
                assert_eq!(failure.envelope, selection_envelope(Some(format)));
                let Some(UsageReason::InvalidValue {
                    context: actual_context,
                    option: actual_option,
                    value,
                    expected,
                }) = failure.usage
                else {
                    panic!("invalid path")
                };
                assert_eq!(actual_context, UsageContext::Path(context));
                if operand == "~" {
                    assert_eq!(actual_option, "USERPROFILE");
                    assert!(value.is_empty());
                    assert_eq!(expected, ExpectedValue::DirectoryPath);
                } else {
                    assert_eq!(actual_option, option);
                    assert_eq!(value, operand);
                    assert_eq!(expected, ExpectedValue::WorktreePath);
                }
            }
        }
    }
}
