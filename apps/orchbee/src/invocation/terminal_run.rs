//! Opaque command submission through the existing ordered paste/Enter path.
use super::{
    ExpectedValue, FailureEnvelope, MAXIMUM_SEND_PASTE_BYTES, MutationCommand, NO_OPTIONS, Options,
    ParseFailure, Request, Resource, TerminalSendInput, UsageContext, UsageReason, check_scope,
    missing_required, mutation, target, unexpected_argument, usage_with,
};

pub(super) fn request(
    operands: &[&str],
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let context = UsageContext::Path("terminal run");
    check_scope(options, NO_OPTIONS, context, envelope)?;
    let Some(selector) = operands.first() else {
        return Err(missing_required(context, "<target>", envelope));
    };
    let Some(command) = operands.get(1) else {
        return Err(missing_required(context, "<command>", envelope));
    };
    if let Some(extra) = operands.get(2) {
        return Err(unexpected_argument(context, extra, envelope));
    }
    if command.is_empty() || command.len() > MAXIMUM_SEND_PASTE_BYTES {
        return Err(usage_with(
            UsageReason::InvalidValue {
                context,
                option: "<command>",
                value: String::new(),
                expected: ExpectedValue::CommandText,
            },
            envelope,
        ));
    }
    Ok(mutation(
        MutationCommand::TerminalSend {
            target: target((*selector).to_owned(), Resource::Terminal, envelope)?,
            input: TerminalSendInput::Paste {
                content: command.as_bytes().to_vec(),
                enter: true,
            },
        },
        options,
    ))
}

#[cfg(test)]
mod tests {
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
}
