use super::*;
#[test]
fn release_background_and_settings_gates_use_monotonic_intervals() {
    let now = Instant::now();
    for background in [false, true] {
        for release in [false, true] {
            for check in [false, true] {
                let settings = UserSettings {
                    manifest_check: check,
                    manifest_check_source: "file",
                    ..UserSettings::default()
                };
                let mut schedule = Schedule::new(&settings, background, release, now);
                assert!(!schedule.admit(now, false));
                assert_eq!(schedule.admit(now, true), background && release && check);
                assert!(!schedule.admit(now + INTERVAL - Duration::from_secs(1), true));
                assert_eq!(
                    schedule.admit(now + INTERVAL, true),
                    background && release && check
                );
                let off = UserSettings {
                    manifest_check: false,
                    manifest_check_source: "file",
                    ..UserSettings::default()
                };
                schedule.apply(&off, now);
                assert!(!schedule.admit(now + INTERVAL * 2, true));
                let on = UserSettings::default();
                schedule.apply(&on, now + INTERVAL * 2);
                assert_eq!(
                    schedule.admit(now + INTERVAL * 2, true),
                    background && release
                );
                schedule.project(true).validate().unwrap();
            }
        }
    }
}
