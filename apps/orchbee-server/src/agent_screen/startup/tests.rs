use super::super::{Detection, PanelAgentState, Stability};
use super::admit;
use std::time::Duration;

fn evidence(state: PanelAgentState, visible: bool, skip: bool) -> Detection {
    Detection {
        state,
        skip,
        visible_idle: visible && state == PanelAgentState::Idle,
        visible_blocker: visible && state == PanelAgentState::Blocked,
        visible_working: visible && state == PanelAgentState::Working,
    }
}

/// One sample of a run that reads as opening, or that a rule already recognised as idle, and the
/// state the run publishes after it. Only rule evidence ever publishes blocked.
#[test]
fn opening_lasts_until_recognition_or_settles_to_unknown() {
    use PanelAgentState::{Blocked, Idle, Opening, Unknown, Working};
    let unreadable = Some(evidence(Unknown, false, false));
    let cases = [
        // (recognised before, seconds since first sight, evidence, published after)
        (false, 1, unreadable, Opening),
        (false, 1, Some(evidence(Idle, false, false)), Opening),
        (false, 1, Some(evidence(Working, true, false)), Working),
        (false, 1, Some(evidence(Blocked, true, false)), Blocked),
        (false, 5, unreadable, Opening),
        (false, 5, None, Opening),
        (false, 5, Some(evidence(Unknown, false, true)), Opening),
        (false, 5, Some(evidence(Idle, false, false)), Idle),
        (false, 15, unreadable, Unknown),
        (false, 15, None, Unknown),
        (false, 60, Some(evidence(Blocked, true, false)), Blocked),
        (true, 5, unreadable, Unknown),
        (true, 20, None, Idle),
        (true, 20, Some(evidence(Working, false, false)), Working),
    ];
    for (recognised, seconds, detection, expected) in cases {
        let start = if recognised {
            evidence(Idle, true, false)
        } else {
            Detection::opening()
        };
        let mut screen = Stability::new(start);
        let since = Duration::from_secs(seconds);
        if let Some(admitted) = admit(detection, since, recognised) {
            let tick = u64::try_from(since.as_millis()).unwrap();
            screen = screen.prepare(admitted, tick, false).unwrap().next;
        }
        let published = screen.published().state;
        assert_eq!(published, expected, "{recognised} {seconds}s {detection:?}");
        assert!(
            published != Blocked || detection.is_some_and(|d| d.visible_blocker),
            "blocked without a rule: {detection:?}"
        );
    }
}
