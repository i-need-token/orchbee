use super::*;

#[test]
fn delays_are_bounded_and_only_sustained_online_forgets_failures() {
    let start = Instant::now();
    let mut backoff = Backoff::new();
    let mut previous = Duration::ZERO;
    for _ in 0..12 {
        backoff.online(start);
        let delay = backoff.fail(start + Duration::from_secs(1));
        assert!(delay >= previous && delay <= LONGEST_DELAY);
        previous = delay;
    }
    assert_eq!(previous, LONGEST_DELAY);
    backoff.online(start);
    backoff.online(start + Duration::from_secs(30));
    assert_eq!(backoff.fail(start + SUSTAINED_ONLINE), FIRST_DELAY);
    assert!(backoff.fail(start + SUSTAINED_ONLINE) > FIRST_DELAY);
    backoff.reset();
    assert_eq!(backoff.fail(start), FIRST_DELAY);
}
