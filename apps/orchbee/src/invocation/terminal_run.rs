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
    check_scope(
        options,
        super::Scope {
            prompt_wait: true,
            timeout: true,
            ..NO_OPTIONS
        },
        context,
        envelope,
    )?;
    if options.timeout.is_some() && !options.prompt_wait {
        return Err(missing_required(context, "--wait", envelope));
    }
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
    if options.prompt_wait {
        return Ok(mutation(
            MutationCommand::TerminalRunWait {
                target: target((*selector).to_owned(), Resource::Terminal, envelope)?,
                command: (*command).to_owned(),
                timeout_ms: super::wait_timeout_option(options, context, envelope)?,
            },
            options,
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
mod tests;
