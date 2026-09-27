//! Deadline and interrupt failures belong to the layout contract in either output format.
use super::*;

#[test]
fn timeout_and_cancelled_failures_preserve_codes_and_use_layout_details() {
    for (kind, status) in [(FailureKind::Timeout, 7), (FailureKind::Cancelled, 130)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let json = failure_output(
                classify(Failure::new(kind)),
                Some(OutputFormat::Json),
                locale,
            );
            assert_eq!(json.status, status);
            let value: serde_json::Value =
                serde_json::from_slice(&json.stdout).expect("failure JSON");
            assert_eq!(value["schema"], "orchbee.cli.failure.layout");
            assert_eq!(value["failure"]["kind"], kind.as_str());
            assert_eq!(value["failure"]["details"]["reason"], "layout-unavailable");
            let text = failure_output(
                classify(Failure::new(kind)),
                Some(OutputFormat::Text),
                locale,
            );
            assert_eq!(text.status, status);
            assert!(text.stdout.is_empty());
            assert!(!text.stderr.is_empty());
        }
    }
}
