use super::*;
#[test]
fn release_and_background_gates_use_monotonic_intervals() {
    let now = Instant::now();
    for background in [false, true] {
        for release in [false, true] {
            let mut schedule = Schedule::new(background, release, now);
            assert!(!schedule.admit(now, false));
            assert_eq!(schedule.admit(now, true), background && release);
            assert!(!schedule.admit(now + INTERVAL - Duration::from_secs(1), true));
            assert_eq!(schedule.admit(now + INTERVAL, true), background && release);
            schedule.project(true).validate().unwrap();
        }
    }
}
