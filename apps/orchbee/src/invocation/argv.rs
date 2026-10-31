//! Argv admission: the UTF-8 and total byte envelope checked before any option interpretation,
//! then machine routing.

use super::{
    FailureEnvelope, MAXIMUM_ARGUMENT_BYTES, ParseFailure, SelectedRequest, UsageReason,
    machine_routing, usage_with,
};
use std::ffi::OsString;

/// Parse the full argv tail into a typed request. The argv envelope (UTF-8 and
/// total byte budget) is validated before any option interpretation.
pub(crate) fn parse_selected_request(
    arguments: impl Iterator<Item = OsString>,
) -> Result<SelectedRequest, ParseFailure> {
    let mut tokens = Vec::new();
    let mut total_bytes = 0_usize;
    for argument in arguments {
        let Some(text) = argument.to_str() else {
            return Err(usage_with(
                UsageReason::ArgumentEnvelope,
                FailureEnvelope::Silent,
            ));
        };
        total_bytes += text.len();
        if total_bytes > MAXIMUM_ARGUMENT_BYTES {
            return Err(usage_with(
                UsageReason::ArgumentEnvelope,
                FailureEnvelope::Silent,
            ));
        }
        tokens.push(text.to_owned());
    }
    machine_routing::parse(tokens)
}

#[cfg(test)]
pub(crate) fn parse_request(
    arguments: impl Iterator<Item = OsString>,
) -> Result<super::Request, ParseFailure> {
    parse_selected_request(arguments).map(|selected| selected.request)
}
