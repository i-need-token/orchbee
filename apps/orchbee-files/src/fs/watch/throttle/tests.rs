//! The throttle's chunks, rests and bound on a fake clock.

use super::*;

fn batch(start: usize, count: usize) -> Vec<usize> {
    (start..start + count).collect()
}

#[test]
fn a_burst_passes_500_at_once_and_500_after_each_rest() {
    let start = Instant::now();
    let mut throttle = Throttle::new();
    assert_eq!(throttle.offer(batch(0, 1_200), start), Ok(batch(0, 500)));
    assert_eq!(throttle.due(), Some(start + REST));
    // While throttling, more changes wait behind the others.
    assert_eq!(throttle.offer(batch(1_200, 10), start), Ok(Vec::new()));
    let first_rest = start + REST;
    assert_eq!(throttle.resume(first_rest), batch(500, 500));
    assert_eq!(throttle.due(), Some(first_rest + REST));
    assert_eq!(throttle.resume(first_rest + REST), batch(1_000, 210));
    assert_eq!(throttle.due(), None);
    // Rested, the next change passes at once.
    let later = first_rest + REST * 2;
    assert_eq!(throttle.offer(vec![7], later), Ok(vec![7]));
}

#[test]
fn a_batch_past_30000_waiting_is_refused_whole() {
    let start = Instant::now();
    let mut throttle = Throttle::new();
    // Unthrottled, the first chunk passes at once, so it does not count as waiting.
    assert_eq!(
        throttle.offer(batch(0, BUFFERED + CHUNK + 1), start),
        Err(batch(0, BUFFERED + CHUNK + 1))
    );
    assert_eq!(
        throttle
            .offer(batch(0, BUFFERED + CHUNK), start)
            .map(|chunk| chunk.len()),
        Ok(CHUNK)
    );
    // Throttling with 30,000 waiting, one more is refused and what waits stays.
    assert_eq!(throttle.offer(vec![0], start), Err(vec![0]));
    let mut passed = 0;
    let mut now = start;
    while let Some(due) = throttle.due() {
        now = due;
        passed += throttle.resume(now).len();
    }
    assert_eq!(passed, BUFFERED);
    assert_eq!(now, start + REST * 60);
}
