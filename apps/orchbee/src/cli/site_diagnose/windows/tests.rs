use super::*;

#[test]
fn named_session_diagnosis_validates_without_widening_legacy_scope() {
    let scope = format!("orchbee-session-{}", "a".repeat(47));
    let mut document = collect_at(
        Path::new("never-created-relative-path"),
        &scope,
        Instant::now(),
    )
    .unwrap()
    .envelope;
    let schema: Value = serde_json::from_str(include_str!(
        "../../schemas/orchbee.site.diagnose.windows.json"
    ))
    .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    validator.validate(&document).unwrap();
    assert_eq!(document["version"], 3);
    document["version"] = json!(2);
    assert!(!validator.is_valid(&document));
    document["result"]["scope"] = json!(format!("orchbee-native-{}", "a".repeat(32)));
    assert!(validator.is_valid(&document));
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
        "orchbee-native-00000000000000000000000000000000",
        Instant::now(),
    )
    .unwrap();
    assert_eq!(result.envelope["result"]["complete"], false);
    assert_eq!(
        result.envelope["result"]["runtimeDirectory"]["reason"],
        "deadline"
    );
    assert_eq!(
        result.envelope["result"]["marker"]["reason"],
        "parent-unavailable"
    );
}
