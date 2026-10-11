use super::*;

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
