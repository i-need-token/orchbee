use super::*;
use crate::invocation::{SendSourceCause, UsageReason};

#[test]
fn text_failure_retains_localized_source_diagnostic_and_escapes_untrusted_paths() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let mut failure = Failure::new(FailureKind::InvalidUsage);
        failure.usage = Some(Box::new(UsageReason::SendSource {
            option: "--file",
            value: Some("C:\\bad\u{1b}[31m\tfile".into()),
            cause: SendSourceCause::NotRegularFile,
        }));
        let diagnostic = locale.cli_failure(&failure);
        let result = Receipt::new().emit(Some(failure), OutputFormat::Text, locale);
        assert_eq!(result.status, 2);
        assert!(result.stdout.is_empty());
        assert!(result.stderr.contains(&diagnostic));
        assert!(!result.stderr.contains('\u{1b}'));
    }
}

#[test]
fn repeated_close_observations_retain_latest_status_without_unbounded_output() {
    let mut receipt = Receipt::new();
    receipt.record("replace", json!({"request":"retained"}));
    for _ in 0..10000 {
        receipt.record("replace-status", json!({"status":{"kind":"closing"}}));
    }
    receipt.record(
        "replace-status",
        json!({"status":{"kind":"closed","revision":"8"}}),
    );
    assert_eq!(receipt.effects.len(), 2);
    assert_eq!(receipt.effects[0]["document"]["request"], "retained");
    assert_eq!(receipt.effects[1]["document"]["status"]["kind"], "closed");
}
