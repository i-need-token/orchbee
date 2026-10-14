use super::{Capability, HostReply, ModeReportState, Probe, QUERY_DEADLINE, Query};
use std::time::{Duration, Instant};

#[test]
fn query_is_exact_and_can_begin_only_once() {
    use crossterm::Command;

    assert_eq!(Query.sequence(), "\x1b[?1003$p\x1b[c");
    let mut encoded = String::new();
    Query.write_ansi(&mut encoded).unwrap();
    assert_eq!(encoded, Query.sequence());
    let now = Instant::now();
    let mut probe = Probe::default();
    assert_eq!(probe.begin(now), Some(Query));
    assert_eq!(probe.begin(now), None);
    assert_eq!(probe.capability(), None);
}

#[test]
fn reset_after_the_button_baseline_admits_any_motion_once() {
    let now = Instant::now();
    let mut probe = Probe::default();
    probe.begin(now).unwrap();
    assert_eq!(
        probe.observe(
            HostReply::Mode1003(ModeReportState::Reset),
            now + Duration::from_millis(1),
        ),
        Some(Capability::Admitted)
    );
    assert!(probe.any_motion_admitted());
    assert_eq!(
        probe.observe(
            HostReply::Mode1003(ModeReportState::Reset),
            now + Duration::from_millis(2),
        ),
        None
    );
    assert_eq!(probe.begin(now + Duration::from_millis(2)), None);
}

#[test]
fn set_contradicts_the_completed_button_baseline() {
    let now = Instant::now();
    let mut probe = Probe::default();
    probe.begin(now).unwrap();
    assert_eq!(
        probe.observe(
            HostReply::Mode1003(ModeReportState::Set),
            now + Duration::from_millis(1),
        ),
        Some(Capability::Unknown)
    );
    assert!(!probe.any_motion_admitted());
}

#[test]
fn fixed_or_unsupported_states_are_unavailable() {
    for state in [
        ModeReportState::Unsupported,
        ModeReportState::PermanentlySet,
        ModeReportState::PermanentlyReset,
    ] {
        let now = Instant::now();
        let mut probe = Probe::default();
        probe.begin(now).unwrap();
        assert_eq!(
            probe.observe(HostReply::Mode1003(state), now),
            Some(Capability::Unavailable)
        );
        assert_eq!(probe.capability(), Some(Capability::Unavailable));
        assert_eq!(probe.begin(now), None);
    }
}

#[test]
fn primary_attributes_before_the_mode_report_settle_unknown() {
    let now = Instant::now();
    let mut probe = Probe::default();
    probe.begin(now).unwrap();
    assert_eq!(
        probe.observe(HostReply::PrimaryDeviceAttributes, now),
        Some(Capability::Unknown)
    );
    assert_eq!(
        probe.observe(HostReply::Mode1003(ModeReportState::Reset), now),
        None
    );
    assert_eq!(probe.capability(), Some(Capability::Unknown));
}

#[test]
fn deadline_is_inclusive_and_late_reset_cannot_reopen_admission() {
    let now = Instant::now();
    let mut probe = Probe::default();
    probe.begin(now).unwrap();
    assert_eq!(
        probe.tick(now + QUERY_DEADLINE - Duration::from_nanos(1)),
        None
    );
    assert_eq!(
        probe.observe(
            HostReply::Mode1003(ModeReportState::Reset),
            now + QUERY_DEADLINE,
        ),
        Some(Capability::Unknown)
    );
    assert_eq!(probe.capability(), Some(Capability::Unknown));
    assert_eq!(
        probe.observe(
            HostReply::Mode1003(ModeReportState::Reset),
            now + QUERY_DEADLINE + Duration::from_millis(1),
        ),
        None
    );
    assert_eq!(probe.begin(now + QUERY_DEADLINE), None);
}

#[test]
fn ticking_after_any_settlement_is_inert() {
    let now = Instant::now();
    let mut probe = Probe::default();
    probe.begin(now).unwrap();
    probe
        .observe(HostReply::Mode1003(ModeReportState::Reset), now)
        .unwrap();
    assert_eq!(probe.tick(now + QUERY_DEADLINE), None);
    assert_eq!(probe.capability(), Some(Capability::Admitted));
}
