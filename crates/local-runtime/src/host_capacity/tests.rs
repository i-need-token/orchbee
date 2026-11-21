//! The free processor share between two readings: rounding, a reading with no time between, and,
//! on macOS, a 32-bit counter that wrapped between the readings.
use super::CpuTicks;

#[test]
fn the_free_share_is_idle_time_over_all_time_rounded() {
    let earlier = CpuTicks {
        idle: 100,
        busy: 50,
    };
    let later = CpuTicks {
        idle: 167,
        busy: 83,
    };
    // 67 idle of 100 ticks.
    assert_eq!(later.free_percent_since(earlier), Some(67));
    assert_eq!(earlier.free_percent_since(earlier), None);
}

#[cfg(target_os = "macos")]
#[test]
fn a_wrapped_macos_counter_still_gives_the_share() {
    let earlier = CpuTicks {
        idle: u64::from(u32::MAX - 9),
        busy: u64::from(u32::MAX - 4),
    };
    // Each counter wrapped: 30 idle and 10 busy ticks passed.
    let later = CpuTicks { idle: 20, busy: 5 };
    assert_eq!(later.free_percent_since(earlier), Some(75));
}
