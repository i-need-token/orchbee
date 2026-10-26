use super::*;

#[test]
fn catalog_checks_publish_the_failed_runtime_stage() {
    let schema: Value =
        serde_json::from_str(include_str!("../schemas/orchbee.cli.failure.json")).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    for stage in ["runtime-base", "runtime-directory", "socket"] {
        let failure = local_error_at(io::ErrorKind::NotFound.into(), stage);
        assert_eq!(failure.details.as_ref().unwrap()["stage"], stage);
        validator.validate(&failure.document()).unwrap();
    }
}

#[test]
fn unavailable_session_states_do_not_claim_a_running_server() {
    let schema: Value =
        serde_json::from_str(include_str!("../schemas/orchbee.session.list.json")).unwrap();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    let result = json!({"sessions":[{"name":"default","default":true,"state":"recoverable","running":false,"pid":null,"runtimeDirectory":{"path":"C:/runtime"},"endpoint":"pipe"}]});
    let output = emit(
        "orchbee.session.list",
        result,
        String::new(),
        0,
        OutputFormat::Json,
        Locale::EnUs,
    );
    let mut document: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(validator.is_valid(&document));
    document["result"]["sessions"][0]["state"] = json!("stopped");
    assert!(validator.is_valid(&document));
    for (state, running) in [("recoverable", json!(false)), ("unreadable", json!(null))] {
        document["result"]["sessions"][0]["state"] = json!(state);
        document["result"]["sessions"][0]["running"] = running;
        assert!(validator.is_valid(&document));
        document["result"]["sessions"][0]["running"] = json!(true);
        assert!(!validator.is_valid(&document));
    }
}

#[test]
fn deletion_receipts_validate_without_hiding_partial_effects_or_diagnostics() {
    let schema: Value =
        serde_json::from_str(include_str!("../schemas/orchbee.session.delete.json")).unwrap();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    for result in [
        Err(NamedSessionDeleteFailure {
            reason: Reason::Interrupted,
            pending: false,
        }),
        Err(NamedSessionDeleteFailure {
            reason: Reason::Interrupted,
            pending: true,
        }),
        Ok(NamedSessionDeleteOutcome::Deleted),
        Ok(NamedSessionDeleteOutcome::Absent),
        Err(NamedSessionDeleteFailure {
            reason: Reason::DefaultSession,
            pending: false,
        }),
        Err(NamedSessionDeleteFailure {
            reason: Reason::Filesystem,
            pending: true,
        }),
        Err(NamedSessionDeleteFailure {
            reason: Reason::TimedOut,
            pending: true,
        }),
    ] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let output = delete_output("work", result, OutputFormat::Json, locale);
            let doc: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert!(validator.is_valid(&doc), "{doc}");
            assert_eq!(output.status == 0, result.is_ok());
            assert_eq!(output.stderr.is_empty(), result.is_ok());
            let human = delete_output("work", result, OutputFormat::Text, locale);
            if result.is_err() {
                assert!(human.stdout.is_empty());
                assert!(!human.stderr.is_empty());
            }
            if doc["result"]["pending"] == true {
                let mut bad = doc.clone();
                bad["result"]["outcome"] = json!("deleted");
                assert!(!validator.is_valid(&bad));
            }
        }
    }
}
#[test]
fn absent_endpoint_never_turns_unknown_lifecycle_into_stopped() {
    assert_eq!(
        inactive_state(ActivationState::Active, SessionLifetimeObservation::Free),
        SessionState::RecoveryRequired
    );
    assert_eq!(
        inactive_state(ActivationState::Active, SessionLifetimeObservation::Held),
        SessionState::Starting
    );
    assert_eq!(
        inactive_state(
            ActivationState::Stopped {
                request_id: [1; 16]
            },
            SessionLifetimeObservation::Free
        ),
        SessionState::Stopped
    );
    for lifetime in [
        SessionLifetimeObservation::Held,
        SessionLifetimeObservation::Unsafe,
        SessionLifetimeObservation::Absent,
    ] {
        assert_eq!(
            inactive_state(
                ActivationState::Stopped {
                    request_id: [1; 16]
                },
                lifetime
            ),
            SessionState::RecoveryRequired
        );
    }
}
