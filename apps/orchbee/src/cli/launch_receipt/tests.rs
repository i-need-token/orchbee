//! Receipt decoration retains outcomes, cleanup and the bounded output contract.
use super::*;
#[test]
fn annotates_supported_receipts_without_replacing_their_results() {
    let launch = LaunchSelection::new("/bin/tool".into(), vec!["literal $value".into()]).unwrap();
    for (schema, before, after) in [
        ("orchbee.creation.result", 3, 6),
        ("orchbee.creation.result", 4, 7),
        ("orchbee.creation.result", 5, 8),
        ("orchbee.workspace.creation", 2, 3),
        ("orchbee.panel.terminal-create", 1, 2),
        ("orchbee.mutation.result", 4, 7),
    ] {
        let mut document = json!({"schema":schema,"version":before,"result":{"outcome":"unknown"}});
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
