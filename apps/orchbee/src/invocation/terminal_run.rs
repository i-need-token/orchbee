//! Opaque command submission through the existing ordered paste/Enter path.
use super::{
    ExpectedValue, FailureEnvelope, MAXIMUM_SEND_PASTE_BYTES, MutationCommand, Options,
    ParseFailure, Request, TerminalSendInput, UsageContext, UsageReason, check_scope,
    missing_required, mutation, unexpected_argument, usage_with,
};

pub(super) fn request(
    operands: &[&str],
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let context = UsageContext::Path("terminal run");
    check_scope(options, context, envelope)?;
    if options.timeout.is_some() && !options.prompt_wait {
        return Err(missing_required(context, "--wait", envelope));
    }
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
    let Some(command) = command_words.first() else {
        return Err(missing_required(context, "<command>", envelope));
    };
    if let Some(extra) = command_words.get(1) {
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
                target: selected,
                command: (*command).to_owned(),
                timeout_ms: super::wait_timeout_option(options, context, envelope)?,
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
