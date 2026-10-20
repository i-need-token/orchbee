use super::super::{OutputFormat, Target, parse_request};
use super::*;

fn parse(args: &[&str]) -> Result<Request, ParseFailure> {
    parse_request(args.iter().map(std::ffi::OsString::from))
}

#[test]
fn command_is_opaque_single_operand_lowered_to_paste_then_enter() {
    for command in [
        "echo hello",
        "  中文 'quotes'\nsecond line  ",
        "--format=json",
    ] {
        let result = parse(&[
            "--format=json",
            "terminal",
            "run",
            "33333333333333333333333333333333",
            "--",
            command,
        ]);
        assert_eq!(
            result,
            Ok(Request::Mutation {
                command: MutationCommand::TerminalSend {
                    target: Target::Id("33333333333333333333333333333333".into()),
                    input: TerminalSendInput::Paste {
                        content: command.as_bytes().to_vec(),
                        enter: true
                    },
                },
                format: OutputFormat::Json,
            })
        );
    }
}

#[test]
fn invalid_command_forms_are_rejected_before_runtime() {
    for args in [
        vec!["terminal", "run"],
        vec!["terminal", "run", "33333333333333333333333333333333"],
        vec!["terminal", "run", "33333333333333333333333333333333", ""],
        vec![
            "terminal",
            "run",
            "33333333333333333333333333333333",
            "echo",
            "extra",
        ],
        vec![
            "terminal",
            "run",
            "33333333333333333333333333333333",
            "echo",
            "--enter",
        ],
        vec![
            "terminal",
            "run",
            "33333333333333333333333333333333",
            "echo",
            "--paste",
        ],
        vec![
            "terminal",
            "run",
            "33333333333333333333333333333333",
            "echo",
            "--text",
            "other",
        ],
        vec![
            "terminal",
            "run",
            "33333333333333333333333333333333",
            "echo",
            "--file",
            "secret",
        ],
        vec![
            "terminal",
            "run",
            "33333333333333333333333333333333",
            "echo",
            "--stdin",
        ],
        vec![
            "terminal",
            "run",
            "33333333333333333333333333333333",
            "echo",
            "--key",
            "enter",
        ],
    ] {
        assert!(parse(&args).is_err(), "{args:?}");
    }
    for args in [
        vec!["--version", "--paste"],
        vec!["status", "--paste"],
        vec!["tui", "--paste"],
        vec!["api", "snapshot", "--paste"],
    ] {
        assert!(parse(&args).is_err(), "{args:?}");
    }
    let oversized = "x".repeat(64 * 1024);
    assert!(
        parse(&[
            "terminal",
            "run",
            "33333333333333333333333333333333",
            &oversized
        ])
        .is_err()
    );
}

#[test]
fn waiting_is_explicit_bounded_and_keeps_the_original_command() {
    let target = "33333333333333333333333333333333";
    assert_eq!(
        parse(&[
            "terminal",
            "run",
            target,
            "Write-Output 'ok'",
            "--wait",
            "--timeout",
            "1200"
        ]),
        Ok(Request::Mutation {
            command: MutationCommand::TerminalRunWait {
                target: Target::Id(target.to_owned()),
                command: "Write-Output 'ok'".to_owned(),
                timeout_ms: 1200,
            },
            format: OutputFormat::Text,
        })
    );
    assert!(parse(&["terminal", "run", target, "echo", "--timeout", "1200"]).is_err());
    assert!(
        parse(&[
            "terminal",
            "run",
            target,
            "echo",
            "--wait",
            "--timeout",
            "0"
        ])
        .is_err()
    );
}
