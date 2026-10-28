use super::*;

#[test]
fn rejected_second_plan_leaves_both_ready_state_and_mailbox_cursor_unchanged() {
    let mailbox = bootstrapped_mailbox(1, 9);
    let mut canonical = ready_state(2);
    let baseline = canonical.cache().clone();
    let old_key = canonical.desired().unwrap().clone();
    let candidate = baseline.clone();
    let prepared = canonical
        .prepare_membership_candidate(candidate, Some(old_key.tab_id()), Some(0), None, None)
        .unwrap();
    let mut locked = mailbox.lock();
    let mut transaction = UiReplyTransaction {
        state: &mut locked,
        observed_at: Instant::now(),
    };
    let first = transaction
        .prepare_desired_install(DesiredInstall::Fresh {
            tab_id: key(1, 999).tab_id().clone(),
            panel: panel(),
        })
        .unwrap();
    assert!(matches!(
        canonical.prepare_membership_install(
            prepared,
            first.desired().map(|pane| pane.anchor().clone()),
            None
        ),
        Err(CarrierFailure::Protocol)
    ));
    drop(first);
    assert_eq!(canonical.cache(), &baseline);
    assert_eq!(canonical.desired(), Some(&old_key));
    assert!(canonical.feedback_accepted());
    assert!(canonical.invariant_holds());
    assert_eq!(locked.desired.key(), Some(&old_key));
    assert_eq!(locked.attachment_generation, 1);
}

#[test]
fn dropping_prepared_desired_or_exhausted_allocation_cannot_split_cursor_and_key() {
    let mailbox = bootstrapped_mailbox(7, 9);
    let mut locked = mailbox.lock();
    {
        let mut transaction = UiReplyTransaction {
            state: &mut locked,
            observed_at: Instant::now(),
        };
        let plan: PreparedDesiredInstall<'_> = transaction
            .prepare_desired_install(DesiredInstall::Fresh {
                tab_id: key(1, 2).tab_id().clone(),
                panel: panel(),
            })
            .unwrap();
        assert_eq!(plan.desired().map(|pane| pane.anchor()), Some(&key(8, 2)));
        drop(plan);
    }
    assert_eq!(
        locked.desired.key().map(|pane| pane.anchor()),
        Some(&key(7, 1))
    );
    assert_eq!(locked.attachment_generation, 7);
    locked.attachment_generation = u64::MAX;
    let mut transaction = UiReplyTransaction {
        state: &mut locked,
        observed_at: Instant::now(),
    };
    assert!(matches!(
        transaction.prepare_desired_install(DesiredInstall::Fresh {
            tab_id: key(1, 2).tab_id().clone(),
            panel: panel()
        }),
        Err(CarrierFailure::IdentityExhausted)
    ));
    assert_eq!(
        locked.desired.key().map(|pane| pane.anchor()),
        Some(&key(7, 1))
    );
    assert_eq!(locked.attachment_generation, u64::MAX);
}

#[test]
fn empty_preserves_burned_cursor_and_next_selection_uses_its_checked_successor() {
    let mailbox = bootstrapped_mailbox(7, 9);
    let mut locked = mailbox.lock();
    let mut transaction = UiReplyTransaction {
        state: &mut locked,
        observed_at: Instant::now(),
    };
    transaction
        .prepare_desired_install(DesiredInstall::Empty)
        .unwrap()
        .commit();
    assert_eq!(transaction.state.desired.key(), None);
    assert!(!transaction.state.desired.awaiting_bootstrap());
    transaction
        .prepare_desired_install(DesiredInstall::Preserve)
        .unwrap()
        .commit();
    let fresh = transaction
        .prepare_desired_install(DesiredInstall::Fresh {
            tab_id: key(1, 2).tab_id().clone(),
            panel: panel(),
        })
        .unwrap();
    assert_eq!(fresh.desired().map(|pane| pane.anchor()), Some(&key(8, 2)));
    fresh.commit();
    assert_eq!(transaction.state.attachment_generation, 8);
    assert_eq!(
        transaction.state.desired.key().map(|pane| pane.anchor()),
        Some(&key(8, 2))
    );
}

#[test]
fn reserved_selection_reuses_only_the_current_nonzero_reservation() {
    let mailbox = bootstrapped_mailbox(7, 9);
    let mut locked = mailbox.lock();
    let mut transaction = UiReplyTransaction {
        state: &mut locked,
        observed_at: Instant::now(),
    };
    for generation in [0, 6, 8] {
        assert!(matches!(
            transaction.prepare_desired_install(DesiredInstall::ReuseReserved {
                key: key(generation, 2),
                panel: panel()
            }),
            Err(CarrierFailure::Protocol)
        ));
    }
    transaction
        .prepare_desired_install(DesiredInstall::ReuseReserved {
            key: key(7, 2),
            panel: panel(),
        })
        .unwrap()
        .commit();
    assert_eq!(transaction.state.attachment_generation, 7);
    assert_eq!(
        transaction.state.desired.key().map(|pane| pane.anchor()),
        Some(&key(7, 2))
    );
}
