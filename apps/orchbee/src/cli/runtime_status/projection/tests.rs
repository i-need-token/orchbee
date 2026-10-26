use super::*;
use std::io;
use std::{ffi::OsString, os::windows::ffi::OsStringExt};

#[test]
fn cohort_io_kind_uses_closed_stable_tokens() {
    for (kind, token) in [
        (io::ErrorKind::NotFound, "not-found"),
        (io::ErrorKind::PermissionDenied, "permission-denied"),
        (io::ErrorKind::InvalidInput, "invalid-input"),
        (io::ErrorKind::InvalidData, "invalid-data"),
        (io::ErrorKind::TimedOut, "timed-out"),
        (io::ErrorKind::Unsupported, "unsupported"),
        (io::ErrorKind::Interrupted, "interrupted"),
        (io::ErrorKind::UnexpectedEof, "unexpected-eof"),
        (io::ErrorKind::WouldBlock, "would-block"),
        (io::ErrorKind::OutOfMemory, "out-of-memory"),
        (io::ErrorKind::ConnectionRefused, "other"),
        (io::ErrorKind::Other, "other"),
    ] {
        assert_eq!(CohortIoKind::from_error_kind(kind).token(), token);
    }

    let schema: Value =
        serde_json::from_str(include_str!("../../schemas/orchbee.status.client.json")).unwrap();
    let schema_tokens = schema["$defs"]["cohortDiagnostic"]["properties"]["ioKind"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|token| token.as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        schema_tokens,
        vec![
            "not-found",
            "permission-denied",
            "invalid-input",
            "invalid-data",
            "timed-out",
            "unsupported",
            "interrupted",
            "unexpected-eof",
            "would-block",
            "out-of-memory",
            "other",
        ]
    );
}

#[test]
fn installed_channel_is_bound_to_release_observation_without_widening_v1() {
    let schema: Value =
        serde_json::from_str(include_str!("../../schemas/orchbee.status.client.json")).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let report = Report::Client {
        executable: PathBuf::from("C:/release/orchbee.exe"),
        cohort: Cohort::Available("a".repeat(64)),
        installation: crate::cli::update::Installation::Verified {
            channel: orchbee_local_runtime::UpdateChannel::Preview,
            version: "0.1.0".into(),
            build_id: "test".into(),
            manifest: "b".repeat(64),
        },
    };
    let doc = report.document();
    validator.validate(&doc).unwrap();
    assert_eq!(doc["version"], 1);
    assert_eq!(doc["result"]["delivery"], "native-release");
    assert_eq!(doc["result"]["updateChannel"], "preview");
    let mut bad = doc.clone();
    bad["result"]["updateChannel"] = json!("stable");
    assert!(!validator.is_valid(&bad));
}

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
        let make_report = || Report::Client {
            executable: PathBuf::from(OsString::from_wide(&units)),
            cohort: Cohort::Unavailable(
                issue,
                Some(CohortDiagnostic {
                    io_kind: CohortIoKind::NotFound,
                    os_error: Some(2),
                    missing_artifact: Some("orchbee-server.exe"),
                }),
            ),
            installation: crate::cli::update::Installation::Unavailable,
        };
        let report = make_report();
        let doc = report.document();
        validator.validate(&doc).unwrap();
        assert_eq!(doc["version"], 1);
        assert_eq!(doc["result"]["executable"], json!({"pathUtf16":units}));
        assert_eq!(
            doc["result"]["cohort"]["diagnostic"],
            json!({"ioKind":"not-found","osError":2,"missingArtifact":"orchbee-server.exe"})
        );
        let en_json = emit(make_report(), OutputFormat::Json, Locale::EnUs);
        let zh_json = emit(make_report(), OutputFormat::Json, Locale::ZhCn);
        assert_eq!(en_json.stdout, zh_json.stdout);
        assert_eq!(en_json.status, 0);
        validator
            .validate(&serde_json::from_slice::<Value>(&en_json.stdout).unwrap())
            .unwrap();
        let en = emit(report, OutputFormat::Text, Locale::EnUs);
        assert_eq!(en.status, 0);
        let text = String::from_utf8(en.stdout).unwrap();
        assert!(!text.contains('\u{fffd}'));
        assert!(!text.contains("NotFound"));
        assert!(!text.contains("missingArtifact"));
        let mut guessed = doc.clone();
        guessed["result"]["cohort"]["identity"] = json!("a".repeat(64));
        assert!(!validator.is_valid(&guessed));
        let mut missing_diagnostic = doc.clone();
        missing_diagnostic["result"]["cohort"]
            .as_object_mut()
            .unwrap()
            .remove("diagnostic");
        assert!(!validator.is_valid(&missing_diagnostic));
        let mut debug_kind = doc.clone();
        debug_kind["result"]["cohort"]["diagnostic"]["ioKind"] = json!("NotFound");
        assert!(!validator.is_valid(&debug_kind));
        let mut unknown_kind = doc.clone();
        unknown_kind["result"]["cohort"]["diagnostic"]["ioKind"] = json!("network-failure");
        assert!(!validator.is_valid(&unknown_kind));
        let mut oversized_os_error = doc.clone();
        oversized_os_error["result"]["cohort"]["diagnostic"]["osError"] = json!(2_147_483_648_i64);
        assert!(!validator.is_valid(&oversized_os_error));
        let mut invented_artifact = doc.clone();
        invented_artifact["result"]["cohort"]["diagnostic"]["missingArtifact"] =
            json!("orchbee-unexpected.exe");
        assert!(!validator.is_valid(&invented_artifact));
        let mut invented_channel = doc;
        invented_channel["result"]["updateChannel"] = json!("stable");
        assert!(!validator.is_valid(&invented_channel));
    }

    let unknown_kind = Report::Client {
        executable: PathBuf::from(r"C:\orchbee.exe"),
        cohort: Cohort::Unavailable(
            CohortIssue::Io,
            Some(CohortDiagnostic {
                io_kind: CohortIoKind::Other,
                os_error: None,
                missing_artifact: None,
            }),
        ),
        installation: crate::cli::update::Installation::Unavailable,
    }
    .document();
    validator.validate(&unknown_kind).unwrap();
    assert_eq!(
        unknown_kind["result"]["cohort"]["diagnostic"],
        json!({"ioKind":"other","osError":null})
    );
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
        cohort: Cohort::Unavailable(CohortIssue::TimedOut, None),
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
