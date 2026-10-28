use super::*;
#[test]
fn retry_is_bounded_and_only_sustained_online_resets_history() {
    let mut now = Instant::now();
    let mut p = Policy::new(now);
    for millis in [
        500, 1000, 2000, 4000, 8000, 16000, 32000, 64000, 120000, 120000,
    ] {
        assert!(p.due(now));
        p.begin();
        assert!(!p.due(now));
        p.healthy(now);
        p.fail(false, now + Duration::from_secs(1));
        now += Duration::from_secs(1) + Duration::from_millis(millis);
        assert!(!p.due(now - Duration::from_millis(1)));
        assert!(p.due(now));
    }
    p.begin();
    p.healthy(now);
    p.healthy(now + Duration::from_secs(30));
    p.fail(false, now + Duration::from_secs(60));
    assert!(p.due(now + Duration::from_millis(60_500)));
}
#[test]
fn permanent_failure_waits_for_explicit_retry() {
    let now = Instant::now();
    let mut p = Policy::new(now);
    p.begin();
    p.fail(true, now);
    assert!(!p.due(now + Duration::from_secs(3600)));
    p.retry(now);
    assert!(p.due(now));
}
