use super::{
    CapsuleStatus, IntegrationOperation, IntegrationReport, IntegrationRequest, decode_report,
    decode_request, encode_report, encode_request,
};
use crate::IntegrationId;
use std::path::PathBuf;

#[cfg(windows)]
fn non_unicode_path() -> PathBuf {
    use std::os::windows::ffi::OsStringExt;
    std::ffi::OsString::from_wide(&[u16::from(b'C'), 0xD800]).into()
}

#[cfg(unix)]
fn non_unicode_path() -> PathBuf {
    use std::os::unix::ffi::OsStrExt;
    std::ffi::OsStr::from_bytes(b"C\xff").into()
}

fn listed(root: PathBuf) -> IntegrationReport {
    IntegrationReport::List {
        items: Vec::new(),
        capsule: CapsuleStatus {
            ready: true,
            root,
            source_kind: "cohort-sidecar",
            binding_failure: false,
            payload_sha256: "0".repeat(64),
        },
    }
}

#[test]
fn requests_use_registry_provider_ids_and_bounded_budgets() {
    let request = IntegrationRequest {
        operation: IntegrationOperation::Enable {
            provider: IntegrationId::AntigravityCli,
        },
        adapter_source: None,
        budget_milliseconds: Some(40_000),
    };
    let bytes = encode_request(&request).unwrap();
    assert!(String::from_utf8_lossy(&bytes).contains("\"provider\":\"antigravity\""));
    assert_eq!(decode_request(&bytes).unwrap(), request);
    for invalid in [
        r#"{"operation":{"kind":"enable","provider":"antigravity-cli"}}"#,
        r#"{"operation":{"kind":"list"},"budgetMilliseconds":0}"#,
        r#"{"operation":{"kind":"list"},"budgetMilliseconds":40001}"#,
        r#"{"operation":{"kind":"list"},"extra":true}"#,
    ] {
        assert!(decode_request(invalid.as_bytes()).is_err(), "{invalid}");
    }
    assert!(decode_request(br#"{"operation":{"kind":"list"}}"#).is_ok());
}

#[test]
fn reports_round_trip_lossless_node_paths_and_reject_open_vocabularies() {
    for report in [
        IntegrationReport::HomeUnavailable {},
        listed(PathBuf::from("capsule")),
        listed(non_unicode_path()),
    ] {
        assert_eq!(
            decode_report(&encode_report(&report).unwrap()).unwrap(),
            report
        );
    }
    let mut document = serde_json::to_value(listed(PathBuf::from("capsule"))).unwrap();
    assert_eq!(
        document["capsule"]["root"],
        serde_json::json!({"text": "capsule"})
    );
    document["capsule"]["sourceKind"] = "other".into();
    assert!(decode_report(&serde_json::to_vec(&document).unwrap()).is_err());
}
