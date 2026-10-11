use super::*;
#[test]
fn unbounded_total_keeps_reads_bounded_and_finite_totals_never_renew() {
    let start = Instant::now();
    let infinite = WaitDeadline::new(start, None).unwrap();
    let later = start + Duration::from_secs(365 * 24 * 3600);
    assert!(!infinite.expired(later));
    assert_eq!(
        infinite.step(later, Duration::from_secs(5)).unwrap(),
        later + Duration::from_secs(5)
    );
    assert_eq!(
        infinite.backoff(later, Duration::from_millis(500)),
        Duration::from_millis(500)
    );
    let finite = WaitDeadline::new(start, Some(3000)).unwrap();
    let near_end = start + Duration::from_millis(2800);
    assert_eq!(
        finite.step(near_end, Duration::from_secs(5)).unwrap(),
        start + Duration::from_secs(3)
    );
    assert_eq!(
        finite.backoff(near_end, Duration::from_millis(500)),
        Duration::from_millis(200)
    );
    assert!(finite.expired(start + Duration::from_secs(3)));
}
