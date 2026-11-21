use super::*;

#[test]
fn missing_runtime_location_is_a_check_failure_before_activation() {
    let failure = classify(ActivationFailure {
        failure: StartupFailure::Runtime,
        cause: Some(ActivationCause::RuntimeLocation(
            std::io::ErrorKind::NotFound.into(),
        )),
    });
    assert_eq!(failure.kind, FailureKind::CheckFailed);
    assert_eq!(failure.details.as_ref().unwrap()["stage"], "runtime-base");
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../schemas/orchbee.cli.failure.json")).unwrap();
    jsonschema::validator_for(&schema)
        .unwrap()
        .validate(&failure.document())
        .unwrap();
}

/// A refused start keeps its kind and exit status, names what blocks it or what the system
/// answered in every failure document that can carry an activation refusal, and words it with the
/// sentence the TUI and the desktop window show.
#[test]
fn refused_starts_name_the_blocking_process_or_the_system_answer() {
    let blocker = orchbee_local_runtime::RecoveryBlocker {
        role: orchbee_local_runtime::BlockerRole::TerminalHost,
        process_id: 4242,
    };
    let spawn = crate::local_activation::ServerSpawnFailure {
        kind: std::io::ErrorKind::PermissionDenied,
        os_error: Some(5),
    };
    let cases = [
        (
            StartupFailure::RecoveryBlocked(blocker),
            json!({
                "stage": "activation",
                "reason": "recovery-required",
                "blockingProcess": {
                    "role": "terminal-host",
                    "program": blocker.role.program().file_name(),
                    "processId": 4242,
                },
            }),
        ),
        (
            StartupFailure::ServerNotStarted(spawn),
            json!({
                "stage": "activation",
                "reason": "server-not-started",
                "ioKind": "permission-denied",
                "osError": 5,
            }),
        ),
    ];
    let schemas = [
        (
            "orchbee.cli.failure",
            include_str!("../schemas/orchbee.cli.failure.json"),
        ),
        (
            "orchbee.cli.failure.tab-move",
            include_str!("../schemas/orchbee.cli.failure.tab-move.json"),
        ),
        (
            "orchbee.cli.failure.workspace-move",
            include_str!("../schemas/orchbee.cli.failure.workspace-move.json"),
        ),
    ]
    .map(|(id, text)| {
        let schema = serde_json::from_str(text).unwrap();
        (id, jsonschema::validator_for(&schema).unwrap())
    });
    for (startup, details) in cases {
        let failure = classify(ActivationFailure {
            failure: startup.clone(),
            cause: None,
        })
        .source(crate::cli_outcome::local_runtime_source());
        assert_eq!(failure.kind, FailureKind::Rejected);
        assert_eq!(
            serde_json::Value::Object(failure.details.clone().unwrap()),
            details
        );
        for (id, validator) in &schemas {
            let mut document = failure.document();
            document["schema"] = json!(id);
            validator.validate(&document).unwrap();
        }
        for locale in [Locale::EnUs, Locale::ZhCn] {
            assert_eq!(
                CliText::new(locale).cli_failure(&failure),
                locale.startup_failure(&startup)
            );
        }
    }
}
