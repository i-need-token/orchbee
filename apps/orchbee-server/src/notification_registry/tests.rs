use super::*;
use orchbee_workbench_protocol::{NotificationPosition, NotificationSound};
fn id(n: u64) -> WorkbenchRequestId {
    WorkbenchRequestId::parse(&format!("{n:032x}")).unwrap()
}
fn message(title: &str) -> NotificationAction {
    NotificationAction::Show {
        notification: Notification::normalize(
            title,
            Some("body"),
            NotificationPosition::TopRight,
            NotificationSound::None,
        )
        .unwrap(),
    }
}
fn run(
    registry: &mut Registry,
    generation: u64,
    request: u64,
    action: NotificationAction,
    at: Instant,
) -> NotificationOutcome {
    registry.execute(
        NonZeroU64::new(generation).unwrap(),
        id(request),
        action,
        at,
    )
}
#[test]
fn fanout_is_once_with_exact_replay_conflict_rate_limit_and_disconnect() {
    let mut r = Registry::default();
    let now = Instant::now();
    assert_eq!(
        run(&mut r, 1, 1, message("first"), now),
        outcome(Reason::NoForegroundClient)
    );
    for g in [2, 3] {
        run(
            &mut r,
            g,
            9,
            NotificationAction::Listen { accepting: true },
            now,
        );
    }
    assert_eq!(
        run(&mut r, 1, 2, message("next"), now),
        outcome(Reason::Shown)
    );
    assert_eq!(
        run(&mut r, 1, 3, message("limited"), now),
        outcome(Reason::RateLimited)
    );
    for g in [2, 3] {
        assert!(matches!(
            run(
                &mut r,
                g,
                9,
                NotificationAction::Listen { accepting: true },
                now
            ),
            NotificationOutcome::Listening {
                notification: Some(_)
            }
        ));
        assert!(matches!(
            run(
                &mut r,
                g,
                9,
                NotificationAction::Listen { accepting: true },
                now
            ),
            NotificationOutcome::Listening { notification: None }
        ));
    }
    assert_eq!(
        run(&mut r, 1, 2, message("next"), now),
        outcome(Reason::Shown)
    );
    assert!(matches!(
        run(&mut r, 1, 2, message("different"), now),
        NotificationOutcome::Failed {
            reason: NotificationFailure::Conflict
        }
    ));
    r.disconnect(NonZeroU64::new(2).unwrap());
    r.disconnect(NonZeroU64::new(3).unwrap());
    assert_eq!(
        run(&mut r, 1, 4, message("empty"), now),
        outcome(Reason::NoForegroundClient)
    );
}
#[test]
fn paused_or_full_inbox_refuses_new_work_and_stale_messages_never_reappear() {
    let mut r = Registry::default();
    let now = Instant::now();
    run(
        &mut r,
        2,
        9,
        NotificationAction::Listen { accepting: false },
        now,
    );
    assert_eq!(
        run(&mut r, 1, 1, message("busy"), now),
        outcome(Reason::Busy)
    );
    run(
        &mut r,
        2,
        9,
        NotificationAction::Listen { accepting: true },
        now,
    );
    assert_eq!(
        run(&mut r, 1, 2, message("one"), now),
        outcome(Reason::Shown)
    );
    assert_eq!(
        run(&mut r, 1, 3, message("two"), now + Duration::from_secs(1)),
        outcome(Reason::Busy)
    );
    assert!(matches!(
        run(
            &mut r,
            2,
            9,
            NotificationAction::Listen { accepting: true },
            now + LIVE
        ),
        NotificationOutcome::Listening { notification: None }
    ));
    assert_eq!(
        run(&mut r, 1, 2, message("one"), now + LIVE),
        outcome(Reason::Shown)
    );
    assert!(matches!(
        run(
            &mut r,
            2,
            9,
            NotificationAction::Listen { accepting: true },
            now + LIVE
        ),
        NotificationOutcome::Listening { notification: None }
    ));
    // Listener lifetime belongs to its connection; only a disconnect removes it.
    r.disconnect(NonZeroU64::new(2).unwrap());
    assert_eq!(
        run(&mut r, 1, 5, message("stale"), now + LIVE + LIVE),
        outcome(Reason::NoForegroundClient)
    );
}
#[test]
fn registry_capacity_is_bounded_and_reclaimed_after_disconnect_and_expiry() {
    let mut r = Registry::default();
    let now = Instant::now();
    for g in 1..=32 {
        run(
            &mut r,
            g,
            1,
            NotificationAction::Listen { accepting: false },
            now,
        );
    }
    assert!(matches!(
        run(
            &mut r,
            33,
            1,
            NotificationAction::Listen { accepting: true },
            now
        ),
        NotificationOutcome::Failed { .. }
    ));
    r.disconnect(NonZeroU64::MIN);
    assert!(matches!(
        run(
            &mut r,
            33,
            1,
            NotificationAction::Listen { accepting: true },
            now
        ),
        NotificationOutcome::Listening { .. }
    ));
    for n in 1..=256 {
        run(&mut r, 99, n, message("bounded"), now);
    }
    assert_eq!(r.receipts.len(), 256);
    assert_eq!(
        run(&mut r, 99, 257, message("overflow"), now),
        outcome(Reason::Busy)
    );
    run(&mut r, 99, 258, message("expired"), now + RETAIN);
    assert_eq!(r.receipts.len(), 1);
    // Time reclaims receipts; listeners stay until their connections disconnect.
    assert_eq!(r.listeners.len(), 32);
    for generation in 2..=33 {
        r.disconnect(NonZeroU64::new(generation).unwrap());
    }
    assert!(r.listeners.is_empty());
}

#[test]
fn notification_disabled_fallback_never_blocks_independent_client_policy_or_replays() {
    let mut r = Registry {
        disabled: true,
        ..Default::default()
    };
    let now = Instant::now();
    assert_eq!(
        run(&mut r, 1, 1, message("First"), now),
        outcome(Reason::Disabled)
    );
    r.disabled = false;
    assert_eq!(
        run(&mut r, 1, 1, message("First"), now),
        outcome(Reason::Disabled)
    );
    assert_eq!(
        run(&mut r, 1, 2, message("Second"), now),
        outcome(Reason::NoForegroundClient)
    );
    r.disabled = true;
    run(
        &mut r,
        2,
        8,
        NotificationAction::Listen { accepting: true },
        now,
    );
    assert_eq!(
        run(&mut r, 1, 3, message("Third"), now),
        outcome(Reason::Shown)
    );
    assert!(matches!(
        run(
            &mut r,
            2,
            8,
            NotificationAction::Listen { accepting: true },
            now
        ),
        NotificationOutcome::Listening {
            notification: Some(_)
        }
    ));
}
