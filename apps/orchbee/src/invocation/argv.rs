//! Argv admission: the UTF-8 and total byte envelope checked before any option interpretation,
//! then machine routing.

use super::{
    FailureEnvelope, MAXIMUM_ARGUMENT_BYTES, OutputFormat, ParseFailure, SelectedRequest,
    UsageReason, machine_routing, usage_with,
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
    if let Some(request) = complete_install(&tokens).or_else(|| installer_uninstall(&tokens)) {
        return Ok(SelectedRequest {
            availability: super::grammar::Availability::Windows,
            format: None,
            request,
            session: None,
        });
    }
    if let Some(request) = automatic_update(&tokens) {
        // Its only reader is the Server, so a refusal before the update is a JSON document too.
        return Ok(SelectedRequest {
            availability: super::grammar::Availability::Windows,
            format: Some(OutputFormat::Json),
            request,
            session: None,
        });
    }
    machine_routing::parse(tokens)
}

/// The automatic helper `--automatic-update-v1`, alone. A build that does not run automatic
/// updates does not know it, so there it reaches the grammar as an unknown option. This helper
/// grammar is permanent; a change adds a new helper name.
fn automatic_update(tokens: &[String]) -> Option<super::Request> {
    let helper = orchbee_local_runtime::AUTOMATIC_UPDATE_HELPER;
    (orchbee_local_runtime::automatic_updates_live() && matches!(tokens, [only] if only == helper))
        .then_some(super::Request::AutomaticUpdate)
}

/// The completion helper `--complete-install-v1 [--no-path] [--allow-downgrade]`, each flag at
/// most once and in any order. This helper grammar is permanent; a change adds a new helper name.
fn complete_install(tokens: &[String]) -> Option<super::Request> {
    let (first, flags) = tokens.split_first()?;
    if first != "--complete-install-v1" {
        return None;
    }
    let (mut no_path, mut allow_downgrade) = (false, false);
    for flag in flags {
        let seen = match flag.as_str() {
            "--no-path" => &mut no_path,
            "--allow-downgrade" => &mut allow_downgrade,
            _ => return None,
        };
        if std::mem::replace(seen, true) {
            return None;
        }
    }
    Some(super::Request::CompleteInstall {
        no_path,
        allow_downgrade,
    })
}

/// The uninstaller helper `--installer-uninstall-v1 [--delete-data]`. This helper grammar is
/// permanent; a change adds a new helper name.
fn installer_uninstall(tokens: &[String]) -> Option<super::Request> {
    match tokens {
        [first] if first == "--installer-uninstall-v1" => Some(false),
        [first, flag] if first == "--installer-uninstall-v1" && flag == "--delete-data" => {
            Some(true)
        }
        _ => None,
    }
    .map(|delete_data| super::Request::InstallerUninstall { delete_data })
}

#[cfg(test)]
pub(crate) fn parse_request(
    arguments: impl Iterator<Item = OsString>,
) -> Result<super::Request, ParseFailure> {
    parse_selected_request(arguments).map(|selected| selected.request)
}
