//! Opaque command submission through the existing ordered paste/Enter path.
use super::{
    ExpectedValue, FailureEnvelope, MAXIMUM_SEND_PASTE_BYTES, MutationCommand, Options,
    ParseFailure, Request, TerminalSendInput, UsageContext, UsageReason, check_scope,
    missing_required, mutation, unexpected_argument, usage_with,
};

/// The shell syntax `--capture` writes its markers and status in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CaptureShell {
    Posix,
    Fish,
    PowerShell,
    Cmd,
}

impl CaptureShell {
    pub(crate) const fn token(self) -> &'static str {
        match self {
            Self::Posix => "posix",
            Self::Fish => "fish",
            Self::PowerShell => "powershell",
            Self::Cmd => "cmd",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        [Self::Posix, Self::Fish, Self::PowerShell, Self::Cmd]
            .into_iter()
            .find(|shell| shell.token() == value)
    }
}

/// Where a waiting or capturing run reads its command: the operand, or one declared source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RunCommand {
    Operand(String),
    File(String),
    Stdin,
}

pub(super) fn request(
    operands: &[&str],
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let context = UsageContext::Path("terminal run");
    check_scope(options, context, envelope)?;
    for (first, second, both) in [
        (
            "--wait",
            "--capture",
            options.prompt_wait && options.capture,
        ),
        ("--file", "--stdin", options.file.is_some() && options.stdin),
    ] {
        if both {
            return Err(usage_with(
                UsageReason::ConflictingOptions {
                    context,
                    first,
                    second,
                },
                envelope,
            ));
        }
    }
    let observed = options.prompt_wait || options.capture;
    let sourced = options.file.is_some() || options.stdin;
    if (options.timeout.is_some() || sourced) && !observed {
        return Err(missing_required(
            context,
            "'--wait' or '--capture'",
            envelope,
        ));
    }
    let capture = match (options.capture, options.capture_shell.as_deref()) {
        (false, None) => None,
        (false, Some(_)) => return Err(missing_required(context, "'--capture'", envelope)),
        (true, None) => return Err(missing_required(context, "'--shell'", envelope)),
        (true, Some(value)) => Some(CaptureShell::parse(value).ok_or_else(|| {
            usage_with(
                UsageReason::InvalidValue {
                    context,
                    option: "--shell",
                    value: value.to_owned(),
                    expected: ExpectedValue::CaptureShell,
                },
                envelope,
            )
        })?),
    };
    let (selectors, command_words) = if options.panel.is_some() {
        (&[][..], operands)
    } else {
        let Some((selector, rest)) = operands.split_first() else {
            return Err(missing_required(
                context,
                "<target> or --panel <panel>",
                envelope,
            ));
        };
        (std::slice::from_ref(selector), rest)
    };
    let selected = super::terminal_target::parse(selectors, options, context, envelope)?;
    if sourced {
        if let Some(extra) = command_words.first() {
            return Err(unexpected_argument(context, extra, envelope));
        }
        return Ok(mutation(
            MutationCommand::TerminalRunWait {
                target: selected,
                command: options
                    .file
                    .clone()
                    .map_or(RunCommand::Stdin, RunCommand::File),
                timeout_ms: super::wait_timeout_option(options, context, envelope)?,
                capture,
            },
            options,
        ));
    }
    let Some(command) = command_words.first() else {
        return Err(missing_required(context, "<command>", envelope));
    };
    if let Some(extra) = command_words.get(1) {
        return Err(unexpected_argument(context, extra, envelope));
    }
    let invalid = |expected| {
        usage_with(
            UsageReason::InvalidValue {
                context,
                option: "<command>",
                value: String::new(),
                expected,
            },
            envelope,
        )
    };
    if command.is_empty() || command.len() > MAXIMUM_SEND_PASTE_BYTES {
        return Err(invalid(ExpectedValue::CommandText));
    }
    if capture == Some(CaptureShell::Cmd) && command.contains(['\n', '\r']) {
        return Err(invalid(ExpectedValue::CmdCommandLine));
    }
    if observed {
        return Ok(mutation(
            MutationCommand::TerminalRunWait {
                target: selected,
                command: RunCommand::Operand((*command).to_owned()),
                timeout_ms: super::wait_timeout_option(options, context, envelope)?,
                capture,
            },
            options,
        ));
    }
    Ok(mutation(
        MutationCommand::TerminalSend {
            target: selected,
            input: TerminalSendInput::Paste {
                content: command.as_bytes().to_vec(),
                enter: true,
            },
        },
        options,
    ))
}

#[cfg(test)]
mod tests;
