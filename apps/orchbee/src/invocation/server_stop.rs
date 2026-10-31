//! `server stop` options: the stop request identity, confirmation, retry, and the stop wait.

use super::{
    ExpectedValue, FailureEnvelope, Options, ParseFailure, ServerStopCommand, UsageContext,
    UsageReason, ZERO_IDENTITY, check_scope, missing_required, usage_with, valid_typed_id,
    wait_timeout_option,
};

pub(super) fn stop_id(
    value: &str,
    context: UsageContext,
    option: &'static str,
    envelope: FailureEnvelope,
) -> Result<orchbee_local_client_protocol::StopId, ParseFailure> {
    let invalid = || {
        usage_with(
            UsageReason::InvalidValue {
                context,
                option,
                value: value.to_owned(),
                expected: ExpectedValue::ServerStopRequestId,
            },
            envelope,
        )
    };
    if !valid_typed_id(value) || value == ZERO_IDENTITY {
        return Err(invalid());
    }
    let mut bytes = [0; 16];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).map_err(|_| invalid())?;
    }
    orchbee_local_client_protocol::StopId::new(bytes).map_err(|_| invalid())
}

pub(super) fn stop_command(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<ServerStopCommand, ParseFailure> {
    check_scope(options, context, envelope)?;
    if !options.confirm {
        return Err(missing_required(context, "'--confirm'", envelope));
    }
    if options.retry && options.request_id.is_none() {
        return Err(missing_required(context, "'--request-id'", envelope));
    }
    let id = options
        .request_id
        .as_deref()
        .map(|value| stop_id(value, context, "--request-id", envelope))
        .transpose()?;
    let timeout_ms = if options.timeout.is_some() {
        wait_timeout_option(options, context, envelope)?
    } else {
        65_000
    };
    Ok(ServerStopCommand::Stop {
        id,
        retry: options.retry,
        detach: options.detach,
        timeout_ms,
    })
}
