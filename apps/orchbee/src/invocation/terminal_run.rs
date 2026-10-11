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
mod tests;
