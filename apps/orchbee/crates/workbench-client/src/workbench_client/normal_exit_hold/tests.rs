use super::*;

#[test]
fn only_a_sole_presentation_exiting_with_code_zero_holds_until_its_bound_or_departure() {
    let (pane, successor) = (1_u32, 2_u32);
    let now = Instant::now();
    let normal = TerminalEnd::Exited(ExitResult::ExitCode(0));
    let mut holds = NormalExitHolds::default();
    for (end, sole) in [
        (TerminalEnd::Exited(ExitResult::ExitCode(1)), true),
        (TerminalEnd::Exited(ExitResult::Signal(9)), true),
        (TerminalEnd::Exited(ExitResult::Unknown(0)), true),
        (TerminalEnd::Failed, true),
        (normal, false),
    ] {
        holds.observe(&pane, end, sole, now);
        assert!(!holds.active(), "{end:?} sole={sole}");
    }

    holds.observe(&pane, normal, true, now);
    assert!(holds.holds(&pane) && !holds.holds(&successor));
    assert!(!holds.release(now + BOUND - Duration::from_millis(1), |_| true));
    assert!(holds.release(now + BOUND, |_| true));
    assert!(!holds.active());

    holds.observe(&pane, normal, true, now);
    assert!(holds.release(now, |key| *key != pane));
    assert!(!holds.active());
}
