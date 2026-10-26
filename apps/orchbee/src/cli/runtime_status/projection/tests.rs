use super::*;
use std::io;
#[cfg(windows)]
mod windows;

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
        installation: crate::cli::installation::Installation::Verified {
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
