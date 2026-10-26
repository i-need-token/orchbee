//! Receipt decoration retains outcomes, cleanup and the bounded output contract.
use super::*;
#[test]
fn annotates_supported_receipts_without_replacing_their_results() {
    let launch = LaunchSelection::new("/bin/tool".into(), vec!["literal $value".into()]).unwrap();
    for schema in [
        "orchbee.creation.result",
        "orchbee.workspace.creation",
        "orchbee.panel.terminal-create",
        "orchbee.mutation.result",
    ] {
        let mut document = json!({"schema":schema,"version":1,"resolvedLaunch":null,"result":{"outcome":"unknown"}});
        assert!(attach(&mut document, &launch));
        assert_eq!(document["version"], 1);
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
            let document = json!({"schema":"orchbee.creation.result","version":1,
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
