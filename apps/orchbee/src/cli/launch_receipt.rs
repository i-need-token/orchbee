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
mod tests {
    use super::*;
    #[test]
    fn annotates_supported_receipts_without_replacing_their_results() {
        let launch =
            LaunchSelection::new("/bin/tool".into(), vec!["literal $value".into()]).unwrap();
        for (schema, before, after) in [
            ("orchbee.creation.result", 3, 6),
            ("orchbee.creation.result", 4, 7),
            ("orchbee.creation.result", 5, 8),
            ("orchbee.workspace.creation", 2, 3),
            ("orchbee.panel.terminal-create", 1, 2),
            ("orchbee.mutation.result", 4, 7),
        ] {
            let mut document =
                json!({"schema":schema,"version":before,"result":{"outcome":"unknown"}});
            assert!(attach(&mut document, &launch));
            assert_eq!(document["version"], after);
            assert_eq!(document["resolvedLaunch"]["exec"], "/bin/tool");
            assert_eq!(document["result"]["outcome"], "unknown");
        }
        let mut failure = json!({"schema":"orchbee.cli.failure","version":1});
        let original = failure.clone();
        assert!(!attach(&mut failure, &launch));
        assert_eq!(failure, original);
    }

    #[test]
    fn resolved_launch_annotation_obeys_the_existing_output_bound() {
        let launch = LaunchSelection::new("/bin/tool".into(), vec![]).unwrap();
        for format in [OutputFormat::Text, OutputFormat::Json] {
            let stdout = if format == OutputFormat::Text {
                vec![b'x'; output::LIMIT]
            } else {
                let document = json!({"schema":"orchbee.creation.result","version":3,
                    "result":"x".repeat(output::LIMIT - 100)});
                let bytes = document.to_string().into_bytes();
                assert!(bytes.len() <= output::LIMIT);
                bytes
            };
            let annotated = annotate(
                Output {
                    status: 8,
                    stdout,
                    stderr: "retained cleanup diagnostic\n".into(),
                    finalization: None,
                },
                Some(&launch),
                format,
                Locale::EnUs,
            );
            assert_eq!(annotated.status, FailureKind::OutputTooLarge.exit_code());
            assert!(annotated.stdout.len() <= output::LIMIT);
            assert!(annotated.stderr.contains("retained cleanup diagnostic"));
        }
    }
}
