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

/// Every reported process keeps its role and version in the schema-bound document, and an unknown
/// role or state stays readable.
#[test]
fn reported_processes_keep_their_order_versions_and_unknown_values() {
    use orchbee_connection_handshake::{
        Build, Extensions, ProcessEntry, ProcessRole, ProcessState, ProductVersion, QueryResult,
    };
    let entry = |role, version: &str, pid, state| ProcessEntry {
        role,
        product_version: ProductVersion::new(version).unwrap(),
        build: Build::default(),
        environment: orchbee_local_runtime::environment_name(),
        pid: std::num::NonZeroU32::new(pid).unwrap(),
        state,
        terminals: 0,
        connections: Vec::new(),
    };
    let result = QueryResult::new(
        vec![
            entry(ProcessRole::Server, "2.0.0", 7, ProcessState::Ready),
            entry(ProcessRole::Worker, "2.0.0", 8, ProcessState::Draining),
            entry(ProcessRole::TerminalHost, "2.1.0", 10, ProcessState::Ready),
            entry(ProcessRole::Unknown, "3.0.0", 9, ProcessState::Unknown),
        ],
        Some(false),
        Extensions::new(),
    )
    .unwrap();
    let report = || {
        Report::Server(ServerReport {
            scope: format!("orchbee-native-{}", "1".repeat(32)),
            runtime: PathBuf::from(r"C:\runtime"),
            endpoint: Some(PathBuf::from(r"\\.\pipe\test")),
            server: Server::Observed(result.clone()),
        })
    };
    let doc = report().document();
    let schema: Value =
        serde_json::from_str(include_str!("../../schemas/orchbee.status.server.json")).unwrap();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    validator.validate(&doc).unwrap();
    let roles = doc["result"]["server"]["processes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|process| (process["role"].clone(), process["version"].clone()))
        .collect::<Vec<_>>();
    assert_eq!(
        roles,
        [
            (json!("server"), json!("2.0.0")),
            (json!("worker"), json!("2.0.0")),
            (json!("terminal-host"), json!("2.1.0")),
            (json!("unknown"), json!("3.0.0")),
        ]
    );
    assert_eq!(doc["result"]["server"]["processes"][3]["state"], "unknown");
    let mut empty = doc.clone();
    empty["result"]["server"]["processes"] = json!([]);
    assert!(!validator.is_valid(&empty));
    // Text lists one line per process in both locales, each labelled by its role.
    for (locale, host) in [
        (Locale::EnUs, "Terminal Host process"),
        (Locale::ZhCn, "Terminal Host 进程"),
    ] {
        let text = String::from_utf8(emit(report(), OutputFormat::Text, locale).stdout).unwrap();
        assert_eq!(text.matches("2.0.0").count(), 2, "{text}");
        assert_eq!(text.matches("3.0.0").count(), 1, "{text}");
        let host_line = text.lines().find(|line| line.contains("2.1.0")).unwrap();
        assert!(
            host_line.contains(host) && host_line.contains("10"),
            "{text}"
        );
    }
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
