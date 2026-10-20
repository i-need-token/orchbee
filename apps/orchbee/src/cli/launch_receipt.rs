//! Preserve resolved executable facts without changing submission or cleanup outcomes.
use super::{Failure, FailureKind, Output, failure_output, output};
use crate::{invocation::OutputFormat, localization::Locale};
use orchbee_workbench_protocol::LaunchSelection;
use serde_json::{Value, json};

pub(super) fn annotate(
    mut output: Output,
    launch: Option<&LaunchSelection>,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let Some(launch) = launch else {
        return output;
    };
    if format == OutputFormat::Text {
        let suffix = locale.cli_resolved_executable(&super::escape::unquoted(launch.executable()));
        if suffix.len() > output::LIMIT.saturating_sub(output.stdout.len()) {
            return oversized(output, format, locale);
        }
        output.stdout.extend(suffix.as_bytes());
        return output;
    }
    // Preflight failure envelopes carry no launch receipt and keep their existing contract.
    if let Ok(mut document) = serde_json::from_slice::<Value>(&output.stdout)
        && attach(&mut document, launch)
    {
        match output::encode_document(&document) {
            Ok(stdout) => output.stdout = stdout,
            Err(output::TooLarge) => return oversized(output, format, locale),
        }
    }
    output
}

fn oversized(mut original: Output, format: OutputFormat, locale: Locale) -> Output {
    let mut failure = failure_output(
        Failure::new(FailureKind::OutputTooLarge).details(serde_json::Map::from_iter([(
            "limitBytes".to_owned(),
            json!(output::LIMIT),
        )])),
        Some(format),
        locale,
    );
    failure.stderr.push_str(&original.stderr);
    failure.finalization = original.finalization.take();
    failure
}

fn attach(document: &mut Value, launch: &LaunchSelection) -> bool {
    let version = match (document["schema"].as_str(), document["version"].as_u64()) {
        (Some("orchbee.creation.result"), Some(version @ 3..=5)) => version + 3,
        (Some("orchbee.workspace.creation"), Some(2)) => 3,
        (Some("orchbee.panel.terminal-create"), Some(1)) => 2,
        (Some("orchbee.mutation.result"), Some(4)) => 7,
        _ => return false,
    };
    document["version"] = json!(version);
    document["resolvedLaunch"] = json!(launch);
    true
}

#[cfg(test)]
mod tests;
