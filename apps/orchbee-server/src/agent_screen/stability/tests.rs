use super::*;
fn signal(state: PanelAgentState, visible: bool) -> Detection {
    Detection {
        state,
        skip: false,
        visible_idle: visible && state == PanelAgentState::Idle,
        visible_blocker: visible && state == PanelAgentState::Blocked,
        visible_working: visible && state == PanelAgentState::Working,
    }
}
#[test]
fn plain_idle_requires_three_more_confirmations_or_elapsed_cap() {
    let original = Stability::new(signal(PanelAgentState::Working, true));
    let idle = signal(PanelAgentState::Idle, false);
    let mut state = original.clone();
    for t in [10, 110, 210] {
        let p = state.prepare(idle, t, false).unwrap();
        assert_eq!(p.publish, None);
        state = p.next;
    }
    assert_eq!(state.prepare(idle, 310, false).unwrap().publish, Some(idle));
    let p = original.prepare(idle, 10, false).unwrap();
    assert_eq!(p.next.prepare(idle, 709, false).unwrap().publish, None);
    assert_eq!(
        p.next.prepare(idle, 710, false).unwrap().publish,
        Some(idle)
    );
    assert_eq!(
        original.pending_idle, None,
        "preparation must not mutate owner"
    );
}
#[test]
fn visible_idle_blocker_identity_change_and_exit_bypass_idle_hold() {
    let initial = Stability::new(signal(PanelAgentState::Working, true));
    let held = initial
        .prepare(signal(PanelAgentState::Idle, false), 1, false)
        .unwrap()
        .next;
    for (d, reset) in [
        (signal(PanelAgentState::Idle, true), false),
        (signal(PanelAgentState::Blocked, true), false),
        (signal(PanelAgentState::Idle, false), true),
    ] {
        let p = held.prepare(d, 2, reset).unwrap();
        assert_eq!(p.publish, Some(d));
        assert_eq!(p.next.pending_idle, None);
    }
}
#[test]
fn stable_blocker_refresh_is_bounded_and_skip_or_stale_cannot_publish() {
    let block = signal(PanelAgentState::Blocked, true);
    let initial = Stability::new(signal(PanelAgentState::Idle, true));
    let state = initial.prepare(block, 100, false).unwrap().next;
    assert_eq!(state.prepare(block, 899, false).unwrap().publish, None);
    assert_eq!(
        state.prepare(block, 900, false).unwrap().publish,
        Some(block)
    );
    let before = state.clone();
    assert!(state.prepare(block, 99, false).is_err());
    assert_eq!(state, before);
    let skip = Detection {
        skip: true,
        ..signal(PanelAgentState::Unknown, false)
    };
    let p = state.prepare(skip, 1000, false).unwrap();
    assert_eq!(p.publish, None);
    assert_eq!(p.next.published, block);
}
#[test]
fn transcript_viewer_clears_idle_confirmation_without_publishing() {
    let working = signal(PanelAgentState::Working, true);
    let idle = signal(PanelAgentState::Idle, false);
    let held = Stability::new(working)
        .prepare(idle, 1, false)
        .unwrap()
        .next;
    let skipped = held
        .prepare(
            Detection {
                skip: true,
                ..signal(PanelAgentState::Unknown, false)
            },
            900,
            false,
        )
        .unwrap();
    assert_eq!(skipped.publish, None);
    assert_eq!(skipped.next.published, working);
    assert_eq!(skipped.next.pending_idle, None);
    let resumed = skipped.next.prepare(idle, 1000, false).unwrap();
    assert_eq!(resumed.publish, None);
    assert_eq!(resumed.next.pending_idle, Some((1000, 0)));
}
