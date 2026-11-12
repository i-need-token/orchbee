use super::*;
#[test]
fn permanent_failure_waits_for_explicit_retry() {
    let now = Instant::now();
    let mut p = Policy::new(now);
    p.begin();
    p.fail(true, now);
    assert!(!p.due(now + std::time::Duration::from_secs(3600)));
    p.retry(now);
    assert!(p.due(now));
}
