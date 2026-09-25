use super::*;
use std::{ffi::OsString, os::windows::ffi::OsStringExt};

#[test]
fn local_metadata_preserves_non_unicode_paths_and_closed_unavailable_states() {
    let schema: Value =
        serde_json::from_str(include_str!("../../schemas/orchbee.status.client.json")).unwrap();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    let units = [67, 58, 92, 0xd800, 92, 120];
    for issue in [
        CohortIssue::Missing,
        CohortIssue::Invalid,
        CohortIssue::Denied,
        CohortIssue::TimedOut,
        CohortIssue::Io,
    ] {
        let report = Report::Client {
            executable: PathBuf::from(OsString::from_wide(&units)),
            cohort: Cohort::Unavailable(issue),
        };
        let doc = report.document();
        validator.validate(&doc).unwrap();
        assert_eq!(doc["result"]["executable"], json!({"pathUtf16":units}));
        let en = emit(report, OutputFormat::Text, Locale::EnUs);
        assert_eq!(en.status, 0);
        assert!(!String::from_utf8(en.stdout).unwrap().contains('\u{fffd}'));
        let mut guessed = doc.clone();
        guessed["result"]["cohort"]["identity"] = json!("a".repeat(64));
        assert!(!validator.is_valid(&guessed));
        let mut invented_channel = doc;
        invented_channel["result"]["updateChannel"] = json!("stable");
        assert!(!validator.is_valid(&invented_channel));
    }
}

#[test]
fn unavailable_local_cohort_cannot_erase_known_protocol_incompatibility() {
    let info = ServerMetadata::new(
        "1.0",
        orchbee_local_client_protocol::ClientHello::new("server").unwrap(),
        std::num::NonZeroU32::new(1).unwrap(),
        std::num::NonZeroU8::new(255).unwrap(),
        ServerMode::Unmanaged,
        u32::MAX,
    )
    .unwrap();
    let report = Report::Server(ServerReport {
        scope: format!("orchbee-native-{}", "1".repeat(32)),
        runtime: PathBuf::from(r"C:\runtime"),
        endpoint: Some(PathBuf::from(r"\\.\pipe\test")),
        server: Server::Observed(info),
        cohort: Cohort::Unavailable(CohortIssue::TimedOut),
    });
    let doc = report.document();
    let schema: Value =
        serde_json::from_str(include_str!("../../schemas/orchbee.status.server.json")).unwrap();
    jsonschema::draft202012::new(&schema)
        .unwrap()
        .validate(&doc)
        .unwrap();
    assert_eq!(
        doc["result"]["compatibility"],
        json!({"admissionProtocol":true,"terminalProtocol":false,"cohort":null,"compatible":false,"restartRequired":true})
    );
    assert_eq!(
        doc["result"]["server"]["capabilities"]["managedStop"],
        false
    );
}

#[test]
fn cancellation_and_timeout_use_their_own_localized_stage_without_activation_copy() {
    let schema: Value =
        serde_json::from_str(include_str!("../../schemas/orchbee.cli.failure.json")).unwrap();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    for (kind, status) in [(FailureKind::Cancelled, 130), (FailureKind::Timeout, 7)] {
        let en = failed(Failure::new(kind), OutputFormat::Json, Locale::EnUs);
        let zh = failed(Failure::new(kind), OutputFormat::Json, Locale::ZhCn);
        assert_eq!(en.stdout, zh.stdout);
        assert_ne!(en.stderr, zh.stderr);
        assert_eq!(en.status, status);
        assert!(!en.stderr.contains("start"));
        let doc: Value = serde_json::from_slice(&en.stdout).unwrap();
        validator.validate(&doc).unwrap();
        assert_eq!(doc["failure"]["details"], json!({"stage":"runtime-status"}));
        assert!(
            failed(Failure::new(kind), OutputFormat::Text, Locale::EnUs)
                .stdout
                .is_empty()
        );
    }
}
