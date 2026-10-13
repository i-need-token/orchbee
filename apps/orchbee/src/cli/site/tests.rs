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
