use super::*;

#[test]
fn named_session_report_uses_the_shared_schema() {
    let scope = "work".to_owned();
    let mut result = collect_at(
        Path::new("never-created-relative-path"),
        false,
        None,
        &scope,
        Instant::now(),
    )
    .unwrap()
    .result;
    let doctor: Value =
        serde_json::from_str(include_str!("../../schemas/orchbee.doctor.json")).unwrap();
    let schema = json!({"$ref": "#/$defs/runtimeFacts", "$defs": doctor["$defs"]});
    let validator = jsonschema::validator_for(&schema).unwrap();
    validator.validate(&result).unwrap();
    result["scope"] = json!("a".repeat(64));
    assert!(validator.is_valid(&result));
    // A Windows scope is its Session's name, so one no Session name spells is refused.
    result["scope"] = json!("a".repeat(65));
    assert!(!validator.is_valid(&result));
}

#[test]
fn spent_budget_and_namespace_limit_do_not_manufacture_absence() {
    assert_eq!(
        error_row(&io::ErrorKind::NotFound.into())["state"],
        "uninspectable"
    );
    let mut complete = true;
    let result = step(Instant::now(), &mut complete, || {
        panic!("no I/O after deadline")
    });
    assert!(!complete);
    assert_eq!(result["reason"], "deadline");
    complete = true;
    let result = step(
        Instant::now() + Duration::from_secs(1),
        &mut complete,
        || row("uninspectable", Some("observation-limit")),
    );
    assert!(!complete);
    assert_eq!(result["state"], "uninspectable");
    let result = collect_at(
        Path::new("never-created-relative-path"),
        false,
        None,
        "default",
        Instant::now(),
    )
    .unwrap();
    assert_eq!(result.result["complete"], false);
    assert_eq!(result.result["runtimeDirectory"]["reason"], "deadline");
    assert_eq!(result.result["marker"]["reason"], "parent-unavailable");
}
