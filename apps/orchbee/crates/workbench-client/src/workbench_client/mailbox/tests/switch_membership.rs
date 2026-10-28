use super::*;

fn admit(
    mailbox: &CommandMailbox,
    state: &mut WorkbenchClientState,
    index: usize,
    base: Instant,
) -> AttachmentKey {
    mailbox
        .try_enqueue_switch_for(
            state,
            |state, generation| {
                let key =
                    AttachmentKey::new(generation, state.tab(index).unwrap().tab_id().clone());
                let target = state
                    .target_for_switch(index, key, OperationDeadlines::from_sample(base).unwrap())
                    .unwrap();
                assert!(state.begin_switch(target.clone()));
                Ok::<_, ()>(SwitchMarker::new(target))
            },
            |state, result, facts| {
                assert!(result.is_ok());
                state.finish_switch_membership_admission(
                    facts.discarded(),
                    facts.inherited(),
                    facts.authorized_cleanup(),
                );
            },
        )
        .unwrap();
    state.desired().unwrap().anchor().clone()
}

fn take_fact(
    mailbox: &CommandMailbox,
    marker: &AttachmentKey,
    base: Instant,
) -> Option<AttachmentKey> {
    let mut locked = mailbox.lock();
    let pane = locked
        .desired
        .key()
        .filter(|key| key.anchor() == marker)
        .cloned()?;
    UiReplyTransaction {
        state: &mut locked,
        observed_at: base,
    }
    .take_switch_anchor_invalidation(&pane)
    .map(|pane| pane.anchor().clone())
}

#[test]
fn admission_invalidates_only_actual_old_event_or_causal_cell_and_missing_callback_is_inert() {
    for cell in ["event", "awaiting", "settled", "missing"] {
        let base = Instant::now();
        let clock = FakeClock::new(base);
        let mut state = ready_state(2);
        let old = state.desired().unwrap().clone();
        let mailbox = mailbox_for_desired(old.clone(), 5);
        mailbox.try_enqueue(poll(&mailbox)).unwrap();
        let lease = mailbox.try_recv().unwrap().unwrap();
        let poll = lease.poll_key().unwrap();
        if cell != "missing" {
            mailbox
                .publish_poll_result_before_using(
                    poll.clone(),
                    None,
                    base + Duration::from_secs(30),
                    &clock,
                )
                .unwrap();
            if cell != "event" {
                mailbox.take_event_before_using(&clock).unwrap().unwrap();
            }
            if cell == "settled" {
                mailbox
                    .deposit_causal(&poll, CausalSettlement::NoFeedback)
                    .unwrap();
            }
        }
        let marker = admit(&mailbox, &mut state, 1, base);
        let cursors = mailbox.cursors();
        assert_eq!(mailbox.cells(), ("empty", "empty"));
        assert_eq!(
            mailbox
                .take_causal_before_using(&poll, base + Duration::from_secs(30), &clock)
                .unwrap(),
            None
        );
        assert_eq!(mailbox.switch_anchor_invalidation_for_test(), None);
        assert_eq!(mailbox.cursors(), cursors);
        assert_eq!(
            state
                .prepare_switch_restore_k(&marker, Some(old.anchor()))
                .is_ok(),
            cell == "missing"
        );
        mailbox.finish_lease(lease).unwrap();
    }
}

#[test]
fn late_fact_survives_build_and_full_refusal_and_queued_marker_becoming_active() {
    let base = Instant::now();
    let clock = FakeClock::new(base);
    let mut state = ready_state(2);
    let old = state.desired().unwrap().clone();
    let mailbox = mailbox_for_desired(old.clone(), 5);
    mailbox.try_enqueue(poll(&mailbox)).unwrap();
    let lease = mailbox.try_recv().unwrap().unwrap();
    let poll = lease.poll_key().unwrap();
    let marker = admit(&mailbox, &mut state, 1, base);
    let cursors = mailbox.cursors();
    assert_eq!(
        mailbox
            .publish_poll_result_before_using(poll, None, base, &clock)
            .unwrap(),
        MutationResult::Stale
    );
    assert_eq!(mailbox.cursors(), cursors);
    let refused = mailbox.try_enqueue_switch_for(
        &mut (),
        |_, _| Err::<SwitchMarker, _>("build refused"),
        |_, result, facts| {
            assert!(matches!(
                result,
                Err(AdmissionError::Build("build refused"))
            ));
            assert_eq!(facts.inherited(), None);
            assert_eq!(facts.discarded(), None);
        },
    );
    assert!(matches!(
        refused,
        Err(AdmissionError::Build("build refused"))
    ));
    assert_eq!(mailbox.cursors(), cursors);
    while mailbox.lock().queue.len() < COMMAND_CAPACITY {
        let columns = 40 + u16::try_from(mailbox.lock().queue.len()).unwrap();
        mailbox.try_enqueue(resize(&mailbox, columns)).unwrap();
    }
    let refused = mailbox.try_enqueue_switch_for(
        &mut (),
        |_, _| -> Result<SwitchMarker, ()> { panic!("full admission never builds") },
        |_, result, facts| {
            assert!(matches!(result, Err(AdmissionError::Full)));
            assert_eq!(facts.inherited(), None);
        },
    );
    assert!(matches!(refused, Err(AdmissionError::Full)));
    assert_eq!(
        mailbox.switch_anchor_invalidation_for_test(),
        Some((marker.clone(), old.anchor().clone()))
    );
    mailbox.finish_lease(lease).unwrap();
    let current = mailbox.try_recv().unwrap().unwrap();
    assert!(matches!(current.record(), QueuedCommand::Switch(_)));
    assert_eq!(
        take_fact(&mailbox, &marker, base),
        Some(old.anchor().clone())
    );
    assert_eq!(take_fact(&mailbox, &marker, base), None);
    assert_eq!(mailbox.cursors(), cursors);
}

#[test]
fn successful_successor_admission_transfers_only_matching_anchor_lineage() {
    let base = Instant::now();
    let clock = FakeClock::new(base);
    let mut state = ready_state(2);
    let old = state.desired().unwrap().clone();
    let mailbox = mailbox_for_desired(old.clone(), 5);
    mailbox.try_enqueue(poll(&mailbox)).unwrap();
    let lease = mailbox.try_recv().unwrap().unwrap();
    let first = admit(&mailbox, &mut state, 1, base);
    mailbox
        .publish_poll_result_before_using(lease.poll_key().unwrap(), Some(vec![1]), base, &clock)
        .unwrap();
    let second = admit(&mailbox, &mut state, 0, base);
    assert_eq!(mailbox.switch_anchor_invalidation_for_test(), None);
    assert_eq!(take_fact(&mailbox, &first, base), None);
    assert!(
        state
            .prepare_switch_restore_k(&second, Some(old.anchor()))
            .is_err()
    );
    assert_eq!(state.desired().map(|pane| pane.anchor()), Some(&second));
    mailbox.finish_lease(lease).unwrap();

    let mut fresh = ready_state(2);
    let mailbox = mailbox_for_desired(old.clone(), 5);
    let marker = admit(&mailbox, &mut fresh, 1, base);
    fresh.invalidate_switch_anchor_for_k(&key(99, 99));
    assert!(
        fresh
            .prepare_switch_restore_k(&marker, Some(old.anchor()))
            .is_ok()
    );
}
