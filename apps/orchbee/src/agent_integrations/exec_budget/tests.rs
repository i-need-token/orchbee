use super::*;

#[test]
fn invocation_slack_cannot_extend_one_command_slot() {
    let start = Instant::now();
    let deadlines = CommandDeadlines::new(start, start + Duration::from_secs(40)).unwrap();

    assert_eq!(deadlines.run.duration_since(start), RUN);
    assert_eq!(deadlines.slot.duration_since(start), SLOT);
    assert_eq!(deadlines.slot.duration_since(deadlines.run), TEARDOWN);
}

#[test]
fn short_caller_deadline_reduces_run_and_preserves_teardown() {
    let start = Instant::now();
    let caller_budget = Duration::from_millis(2_000);
    let deadlines = CommandDeadlines::new(start, start + caller_budget).unwrap();

    assert_eq!(deadlines.slot.duration_since(start), caller_budget);
    assert_eq!(deadlines.slot.duration_since(deadlines.run), TEARDOWN);
}

#[test]
fn caller_without_a_complete_teardown_phase_cannot_start_a_command() {
    let start = Instant::now();

    assert!(CommandDeadlines::new(start, start).is_none());
    assert!(CommandDeadlines::new(start, start + TEARDOWN).is_none());
}

#[test]
fn setup_time_consumes_run_instead_of_moving_teardown() {
    let start = Instant::now();
    let deadlines = CommandDeadlines::new(start, start + SLOT).unwrap();
    let after_slow_setup = start + Duration::from_millis(800);

    assert!(deadlines.run < after_slow_setup + RUN);
    assert_eq!(deadlines.run.duration_since(start), RUN);
    assert_eq!(deadlines.slot.duration_since(deadlines.run), TEARDOWN);
}
